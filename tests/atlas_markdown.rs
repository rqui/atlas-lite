use waveshare_epd397_rust_app::app::display::{DisplayPreferences, UiFontFamily, UiFontSize};
use waveshare_epd397_rust_app::atlas_markdown::{
    AtlasMarkdownLayout, AtlasMarkdownLineKind, AtlasMarkdownOverflow, AtlasMarkdownPages,
    MAX_ATLAS_MARKDOWN_INPUT_BYTES,
};

fn render(body: &str) -> AtlasMarkdownPages {
    AtlasMarkdownPages::parse(body, AtlasMarkdownLayout::for_note_reader())
}

fn page_text(pages: &AtlasMarkdownPages) -> String {
    pages
        .pages()
        .iter()
        .flat_map(|page| page.lines())
        .map(|line| line.text())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn empty_body_has_one_readable_empty_page() {
    let pages = render("");

    assert_eq!(pages.page_count(), 1);
    assert_eq!(pages.pages()[0].lines()[0].text(), "EMPTY NOTE");
    assert_eq!(pages.overflow(), AtlasMarkdownOverflow::None);
}

#[test]
fn headings_paragraphs_lists_and_tasks_have_stable_readable_lines() {
    let pages = render(
        "# One\n## Two\n### Three\n\nA short paragraph.\n\n- alpha\n* beta\n1. first\n2. second\n- [ ] open\n- [x] done",
    );
    let lines = &pages.pages()[0].lines();
    let text = page_text(&pages);

    assert_eq!(lines[0].kind(), AtlasMarkdownLineKind::Heading1);
    assert!(text.contains("One"));
    assert!(text.contains("Two"));
    assert!(text.contains("Three"));
    assert!(text.contains("- alpha"));
    assert!(text.contains("1. first"));
    assert!(text.contains("[ ] open"));
    assert!(text.contains("[x] done"));
}

#[test]
fn links_and_wikilinks_keep_labels_but_not_destinations() {
    let pages = render("[Atlas docs](https://example.test/private) and [[Vault/Plan|Plan]].");
    let text = page_text(&pages);

    assert!(text.contains("Atlas docs"));
    assert!(text.contains("Plan"));
    assert!(!text.contains("https://example.test/private"));
    assert!(!text.contains("Vault/Plan"));
}

#[test]
fn emphasis_and_unclosed_lightweight_syntax_remain_bounded_and_readable() {
    let pages = render("**bold** and _italic_ and **unclosed and [[missing");
    let text = page_text(&pages);

    assert!(text.contains("bold"));
    assert!(text.contains("italic"));
    assert!(text.contains("unclosed"));
    assert!(text.len() <= 512);
}

#[test]
fn long_paragraphs_wrap_and_cross_page_boundaries() {
    let layout = AtlasMarkdownLayout::new(12, 2, 3);
    let pages = AtlasMarkdownPages::parse(
        "one two three four five six seven eight nine ten eleven twelve thirteen fourteen",
        layout,
    );

    assert_eq!(pages.page_count(), 3);
    assert!(pages.pages().iter().all(|page| page.lines().len() <= 2));
    assert!(pages
        .pages()
        .iter()
        .flat_map(|page| page.lines())
        .all(|line| line.text().chars().count() <= 12));
    assert_eq!(pages.overflow(), AtlasMarkdownOverflow::Truncated);
}

#[test]
fn unicode_malformed_and_unsafe_blocks_are_harmless() {
    let pages = render(
        "Café 📚\n\n<script>alert('no')</script>\n<iframe src=\"bad\"></iframe>\n![embed](https://bad.test/a)\n\n---",
    );
    let text = page_text(&pages);

    assert!(text.contains("Café"));
    assert!(!text.contains("alert"));
    assert!(!text.contains("iframe"));
    assert!(!text.contains("embed"));
    assert!(pages
        .pages()
        .iter()
        .flat_map(|page| page.lines())
        .any(|line| line.kind() == AtlasMarkdownLineKind::Separator));
}

#[test]
fn too_large_input_is_an_explicit_non_allocating_overflow_state() {
    let body = "x".repeat(MAX_ATLAS_MARKDOWN_INPUT_BYTES + 1);
    let pages = render(&body);

    assert_eq!(pages.overflow(), AtlasMarkdownOverflow::InputTooLarge);
    assert_eq!(pages.page_count(), 1);
    assert_eq!(pages.pages()[0].lines()[0].text(), "NOTE TOO LARGE");
}

#[test]
fn page_and_line_limits_are_never_exceeded() {
    let layout = AtlasMarkdownLayout::new(4, 1, 2);
    let pages = AtlasMarkdownPages::parse("a b c d e f g h i j k l", layout);

    assert!(pages.page_count() <= layout.max_pages());
    assert!(pages
        .pages()
        .iter()
        .all(|page| page.lines().len() <= layout.lines_per_page()));
    assert!(pages
        .pages()
        .iter()
        .flat_map(|page| page.lines())
        .all(|line| line.text().chars().count() <= layout.columns()));
}

#[test]
fn reader_layout_width_is_safe_for_every_supported_display_profile() {
    const VIEWPORT_WIDTH: i32 = 436;
    const VIEWPORT_HEIGHT: i32 = 608;
    let source = "# Wide heading with W characters\n\n- café 1000 with a verylongunbrokenwordthatmustwrapsafely\n\nNormal paragraph with accented text and enough words to span several display lines.";

    for family in [UiFontFamily::Inter, UiFontFamily::AtkinsonHyperlegible] {
        for size in [UiFontSize::Compact, UiFontSize::Standard, UiFontSize::Large] {
            let preferences = DisplayPreferences {
                font_family: family,
                font_size: size,
            };
            let pages = AtlasMarkdownPages::parse(
                source,
                AtlasMarkdownLayout::for_note_reader_with_preferences(preferences),
            );
            assert!(pages
                .pages()
                .iter()
                .all(|page| page.used_height() <= VIEWPORT_HEIGHT));
            for line in pages.pages().iter().flat_map(|page| page.lines()) {
                let style = match line.kind() {
                    AtlasMarkdownLineKind::Heading1
                    | AtlasMarkdownLineKind::Heading2
                    | AtlasMarkdownLineKind::Heading3 => preferences.heading_style(),
                    AtlasMarkdownLineKind::Body
                    | AtlasMarkdownLineKind::List
                    | AtlasMarkdownLineKind::Separator => preferences.body_style(),
                };
                assert!(
                    style.text_width(line.text()) <= VIEWPORT_WIDTH,
                    "{family:?}/{size:?}: {}",
                    line.text()
                );
            }
        }
    }
}

#[test]
fn comparison_text_with_a_less_than_sign_is_not_treated_as_html() {
    let pages = render("The simple comparison 1 < 2 is readable.");
    assert!(page_text(&pages).contains("1 < 2"));
}

#[test]
fn oversized_notice_is_wrapped_to_a_narrow_caller_layout() {
    let body = "x".repeat(MAX_ATLAS_MARKDOWN_INPUT_BYTES + 1);
    let layout = AtlasMarkdownLayout::new(5, 2, 1);
    let pages = AtlasMarkdownPages::parse(&body, layout);

    assert_eq!(pages.overflow(), AtlasMarkdownOverflow::InputTooLarge);
    assert!(pages
        .pages()
        .iter()
        .flat_map(|page| page.lines())
        .all(|line| line.text().chars().count() <= layout.columns()));
}
