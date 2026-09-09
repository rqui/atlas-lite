//! Bounded Markdown-to-page model for the Atlas Note reader.
//!
//! This deliberately recognizes a small display subset instead of constructing
//! a general Markdown AST. Its input, wrapped lines and pages are capped before
//! any reader/UI code sees them.

use crate::app::{display::DisplayPreferences, typography::UiTextRole};

/// Maximum source bytes accepted by the standalone Markdown model.
///
/// This matches the retained Note-body budget, while keeping this module safe
/// when used independently in host tests or a future cache adapter.
pub const MAX_ATLAS_MARKDOWN_INPUT_BYTES: usize = 16 * 1024;
/// Hard ceiling on retained pages regardless of a caller-provided layout.
pub const MAX_ATLAS_MARKDOWN_PAGES: usize = 32;
/// Hard ceiling on retained lines per page.
pub const MAX_ATLAS_MARKDOWN_LINES_PER_PAGE: usize = 32;
/// Hard ceiling on Unicode scalar values per display line.
pub const MAX_ATLAS_MARKDOWN_COLUMNS: usize = 56;

/// A bounded, reader-sized page layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtlasMarkdownLayout {
    columns: usize,
    lines_per_page: usize,
    max_pages: usize,
    pixel_width: Option<i32>,
    pixel_height: Option<i32>,
    preferences: Option<DisplayPreferences>,
}

impl AtlasMarkdownLayout {
    #[must_use]
    pub const fn new(columns: usize, lines_per_page: usize, max_pages: usize) -> Self {
        Self {
            columns: clamp(columns, 1, MAX_ATLAS_MARKDOWN_COLUMNS),
            lines_per_page: clamp(lines_per_page, 1, MAX_ATLAS_MARKDOWN_LINES_PER_PAGE),
            max_pages: clamp(max_pages, 1, MAX_ATLAS_MARKDOWN_PAGES),
            pixel_width: None,
            pixel_height: None,
            preferences: None,
        }
    }

    /// Pixel-accurate geometry shared with the portrait Note renderer.
    #[must_use]
    pub fn for_note_reader() -> Self {
        // Compatibility layout for host callers that do not own a display
        // profile. Product AppState always supplies the pixel layout below.
        Self::new(18, 16, 24)
    }

    #[must_use]
    pub fn for_note_reader_with_preferences(preferences: DisplayPreferences) -> Self {
        Self {
            columns: MAX_ATLAS_MARKDOWN_COLUMNS,
            lines_per_page: MAX_ATLAS_MARKDOWN_LINES_PER_PAGE,
            max_pages: 24,
            pixel_width: Some(
                crate::app::screens::atlas_note::NOTE_TEXT_RIGHT
                    - crate::app::screens::atlas_note::NOTE_TEXT_LEFT,
            ),
            pixel_height: Some(
                crate::app::screens::atlas_note::NOTE_TEXT_BOTTOM
                    - crate::app::screens::atlas_note::NOTE_TEXT_TOP,
            ),
            preferences: Some(preferences),
        }
    }

    #[must_use]
    pub const fn columns(self) -> usize {
        self.columns
    }

    #[must_use]
    pub const fn lines_per_page(self) -> usize {
        self.lines_per_page
    }

    #[must_use]
    pub const fn max_pages(self) -> usize {
        self.max_pages
    }
}

const fn clamp(value: usize, minimum: usize, maximum: usize) -> usize {
    if value < minimum {
        minimum
    } else if value > maximum {
        maximum
    } else {
        value
    }
}

/// Display treatment retained for each already-safe line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtlasMarkdownLineKind {
    Body,
    Heading1,
    Heading2,
    Heading3,
    List,
    Separator,
}

/// One bounded display line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasMarkdownLine {
    text: String,
    kind: AtlasMarkdownLineKind,
}

impl AtlasMarkdownLine {
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub const fn kind(&self) -> AtlasMarkdownLineKind {
        self.kind
    }
}

/// One fixed-capacity-in-practice Note page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasMarkdownPage {
    lines: Vec<AtlasMarkdownLine>,
    used_height: i32,
}

impl AtlasMarkdownPage {
    #[must_use]
    pub fn lines(&self) -> &[AtlasMarkdownLine] {
        &self.lines
    }

    #[must_use]
    pub const fn used_height(&self) -> i32 {
        self.used_height
    }
}

/// Explicit result when the source or generated display model reaches a cap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtlasMarkdownOverflow {
    None,
    InputTooLarge,
    Truncated,
}

/// Safe, pre-paginated Note content. There is always one page so callers can
/// render loading/error/empty states without special allocation paths.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasMarkdownPages {
    pages: Vec<AtlasMarkdownPage>,
    overflow: AtlasMarkdownOverflow,
}

impl AtlasMarkdownPages {
    #[must_use]
    pub fn parse(body: &str, layout: AtlasMarkdownLayout) -> Self {
        if body.len() > MAX_ATLAS_MARKDOWN_INPUT_BYTES {
            return Self::notice(
                "NOTE TOO LARGE",
                layout,
                AtlasMarkdownOverflow::InputTooLarge,
            );
        }

        let mut output = PageBuilder::new(layout);
        let mut paragraph = String::new();
        let mut in_unsafe_block = false;
        let mut in_fenced_block = false;

        for source_line in body.lines() {
            let trimmed = source_line.trim();
            if trimmed.starts_with("```") {
                flush_paragraph(&mut output, &mut paragraph);
                in_fenced_block = !in_fenced_block;
                continue;
            }
            if in_fenced_block {
                continue;
            }
            if is_unsafe_block_start(trimmed) {
                flush_paragraph(&mut output, &mut paragraph);
                in_unsafe_block = !is_unsafe_block_end(trimmed);
                continue;
            }
            if in_unsafe_block {
                if is_unsafe_block_end(trimmed) {
                    in_unsafe_block = false;
                }
                continue;
            }
            if is_unsupported_block(trimmed) {
                flush_paragraph(&mut output, &mut paragraph);
                continue;
            }
            if trimmed.is_empty() {
                flush_paragraph(&mut output, &mut paragraph);
                continue;
            }
            if let Some((kind, value)) = heading(trimmed) {
                flush_paragraph(&mut output, &mut paragraph);
                output.push_wrapped(&sanitize_inline(value), kind);
                continue;
            }
            if is_separator(trimmed) {
                flush_paragraph(&mut output, &mut paragraph);
                output.push_line(
                    "------------------------------------------",
                    AtlasMarkdownLineKind::Separator,
                );
                continue;
            }
            if let Some(value) = list_item(trimmed) {
                flush_paragraph(&mut output, &mut paragraph);
                output.push_wrapped(&value, AtlasMarkdownLineKind::List);
                continue;
            }
            append_paragraph(&mut paragraph, &sanitize_inline(trimmed));
        }
        flush_paragraph(&mut output, &mut paragraph);

        if output.is_empty() {
            output.push_line("EMPTY NOTE", AtlasMarkdownLineKind::Body);
        }
        output.finish()
    }

    fn notice(message: &str, layout: AtlasMarkdownLayout, overflow: AtlasMarkdownOverflow) -> Self {
        let mut output = PageBuilder::new(layout);
        output.push_wrapped(message, AtlasMarkdownLineKind::Body);
        let mut pages = output.finish();
        // Preserve the reason the source was rejected even if the notice itself
        // reaches a deliberately tiny caller page budget.
        pages.overflow = overflow;
        pages
    }

    #[must_use]
    pub fn pages(&self) -> &[AtlasMarkdownPage] {
        &self.pages
    }

    #[must_use]
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    #[must_use]
    pub const fn overflow(&self) -> AtlasMarkdownOverflow {
        self.overflow
    }
}

struct PageBuilder {
    layout: AtlasMarkdownLayout,
    pages: Vec<AtlasMarkdownPage>,
    overflow: AtlasMarkdownOverflow,
}

impl PageBuilder {
    fn new(layout: AtlasMarkdownLayout) -> Self {
        Self {
            layout,
            pages: Vec::with_capacity(layout.max_pages),
            overflow: AtlasMarkdownOverflow::None,
        }
    }

    fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    fn push_wrapped(&mut self, value: &str, kind: AtlasMarkdownLineKind) {
        let mut line = String::with_capacity(self.layout.columns);
        for word in value.split_whitespace() {
            if self.overflow == AtlasMarkdownOverflow::Truncated {
                return;
            }
            let line_len = line.chars().count();
            let with_word = if line.is_empty() {
                word.into()
            } else {
                format!("{line} {word}")
            };
            if line_len > 0 && !self.layout.fits(&with_word, kind) {
                self.push_line(&line, kind);
                line.clear();
            }
            if !self.layout.fits(word, kind) {
                if !line.is_empty() {
                    self.push_line(&line, kind);
                    line.clear();
                }
                for character in word.chars() {
                    line.push(character);
                    if !self.layout.fits(&line, kind) {
                        let character = line.pop().expect("line has character");
                        self.push_line(&line, kind);
                        line.clear();
                        line.push(character);
                        if self.overflow == AtlasMarkdownOverflow::Truncated {
                            return;
                        }
                    }
                }
            } else {
                if !line.is_empty() {
                    line.push(' ');
                }
                line.push_str(word);
            }
        }
        if !line.is_empty() {
            self.push_line(&line, kind);
        }
    }

    fn push_line(&mut self, value: &str, kind: AtlasMarkdownLineKind) {
        if self.overflow == AtlasMarkdownOverflow::Truncated {
            return;
        }
        let line_height = self.layout.line_height(kind);
        let needs_page = self.pages.last().is_none_or(|page| {
            page.lines.len() >= self.layout.lines_per_page
                || self
                    .layout
                    .pixel_height
                    .is_some_and(|height| page.used_height + line_height > height)
        });
        if needs_page {
            if self.pages.len() >= self.layout.max_pages {
                self.overflow = AtlasMarkdownOverflow::Truncated;
                return;
            }
            self.pages.push(AtlasMarkdownPage {
                lines: Vec::with_capacity(self.layout.lines_per_page),
                used_height: 0,
            });
        }
        let text = if self.layout.pixel_width.is_some() {
            value.into()
        } else {
            limit_chars(value, self.layout.columns)
        };
        let page = self
            .pages
            .last_mut()
            .expect("page exists after bounded allocation");
        page.lines.push(AtlasMarkdownLine { text, kind });
        page.used_height += line_height;
    }

    fn finish(self) -> AtlasMarkdownPages {
        AtlasMarkdownPages {
            pages: self.pages,
            overflow: self.overflow,
        }
    }
}

impl AtlasMarkdownLayout {
    fn style(self, kind: AtlasMarkdownLineKind) -> Option<crate::app::typography::UiTextStyle> {
        self.preferences.map(|preferences| match kind {
            AtlasMarkdownLineKind::Heading1
            | AtlasMarkdownLineKind::Heading2
            | AtlasMarkdownLineKind::Heading3 => preferences.heading_style(),
            AtlasMarkdownLineKind::Body
            | AtlasMarkdownLineKind::List
            | AtlasMarkdownLineKind::Separator => preferences.text_style(
                UiTextRole::Body,
                embedded_graphics::pixelcolor::BinaryColor::On,
            ),
        })
    }

    fn fits(self, text: &str, kind: AtlasMarkdownLineKind) -> bool {
        self.pixel_width.map_or_else(
            || text.chars().count() <= self.columns,
            |width| {
                self.style(kind)
                    .is_some_and(|style| style.text_width(text) <= width)
            },
        )
    }

    fn line_height(self, kind: AtlasMarkdownLineKind) -> i32 {
        self.style(kind)
            .map_or(1, |style| i32::from(style.line_height()))
    }
}

fn flush_paragraph(output: &mut PageBuilder, paragraph: &mut String) {
    if !paragraph.is_empty() {
        output.push_wrapped(paragraph, AtlasMarkdownLineKind::Body);
        paragraph.clear();
    }
}

fn append_paragraph(paragraph: &mut String, value: &str) {
    if value.is_empty() {
        return;
    }
    if !paragraph.is_empty() {
        paragraph.push(' ');
    }
    paragraph.push_str(value);
}

fn heading(value: &str) -> Option<(AtlasMarkdownLineKind, &str)> {
    for (prefix, kind) in [
        ("### ", AtlasMarkdownLineKind::Heading3),
        ("## ", AtlasMarkdownLineKind::Heading2),
        ("# ", AtlasMarkdownLineKind::Heading1),
    ] {
        if let Some(text) = value.strip_prefix(prefix) {
            return Some((kind, text));
        }
    }
    None
}

fn list_item(value: &str) -> Option<String> {
    let unordered = value
        .strip_prefix("- ")
        .or_else(|| value.strip_prefix("* "))
        .map(|text| ("- ", text));
    if let Some((prefix, text)) = unordered {
        return Some(format!("{prefix}{}", sanitize_inline(text)));
    }
    let digits = value.chars().take_while(char::is_ascii_digit).count();
    let remainder = value.get(digits..)?;
    let text = remainder.strip_prefix(". ")?;
    if digits > 0 {
        return Some(format!("{}{}", &value[..digits + 2], sanitize_inline(text)));
    }
    None
}

fn is_separator(value: &str) -> bool {
    value.len() >= 3
        && value
            .chars()
            .all(|character| matches!(character, '-' | '*' | '_'))
}

fn is_unsafe_block_start(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with("<script") || lower.starts_with("<style")
}

fn is_unsafe_block_end(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("</script>") || lower.contains("</style>")
}

fn is_unsupported_block(value: &str) -> bool {
    value.contains("![") || contains_html_tag(value)
}

fn contains_html_tag(value: &str) -> bool {
    let bytes = value.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'<' {
            continue;
        }
        let Some(next) = bytes.get(index + 1).copied() else {
            continue;
        };
        let tag_start = if next == b'/' { index + 2 } else { index + 1 };
        let Some(tag_first) = bytes.get(tag_start).copied() else {
            continue;
        };
        let looks_like_tag = tag_first.is_ascii_alphabetic() || matches!(tag_first, b'!' | b'?');
        if looks_like_tag && bytes[index + 1..].contains(&b'>') {
            return true;
        }
    }
    false
}

fn sanitize_inline(value: &str) -> String {
    let without_links = readable_links(value);
    without_links
        .replace("**", "")
        .replace("__", "")
        .replace(['*', '_'], "")
}

fn readable_links(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find("[[") {
        output.push_str(&rest[..start]);
        let tail = &rest[start + 2..];
        let Some(end) = tail.find("]]") else {
            output.push_str("[[");
            output.push_str(tail);
            return readable_markdown_links(&output);
        };
        let target = &tail[..end];
        output.push_str(target.rsplit_once('|').map_or(target, |(_, label)| label));
        rest = &tail[end + 2..];
    }
    output.push_str(rest);
    readable_markdown_links(&output)
}

fn readable_markdown_links(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find('[') {
        output.push_str(&rest[..start]);
        let tail = &rest[start + 1..];
        let Some(label_end) = tail.find("](") else {
            output.push('[');
            output.push_str(tail);
            return output;
        };
        let destination = &tail[label_end + 2..];
        let Some(destination_end) = destination.find(')') else {
            output.push('[');
            output.push_str(tail);
            return output;
        };
        output.push_str(&tail[..label_end]);
        rest = &destination[destination_end + 1..];
    }
    output.push_str(rest);
    output
}

fn limit_chars(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}
