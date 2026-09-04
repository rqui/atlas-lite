//! Bounded, e-paper-safe Atlas Markdown Note reader.

use core::convert::Infallible;

use embedded_graphics::prelude::Point;

use crate::{
    app::{
        state::AppState,
        typography::{Text, TextBounds},
        widgets::{
            footer::draw_footer,
            header::draw_header,
            status_row::{draw_status_row, StatusRow},
        },
    },
    orientation::OrientedFrameBuffer,
};

/// Renders retained Note state only; networking remains an explicit AppState action.
pub fn render_atlas_note(
    display: &mut OrientedFrameBuffer<'_>,
    state: &AppState,
) -> Result<(), Infallible> {
    let note = &state.atlas_note;
    let body = state.display.body_style();
    let heading = state.display.heading_style();
    let page = page_label(
        note.page_index(),
        note.page_count(),
        note.markdown_overflow(),
    );
    draw_header(display, state.display, "ATLAS LITE", "NOTE")?;
    draw_status_row(
        display,
        state.display,
        StatusRow {
            left: note.status().label(),
            middle: note.origin().map_or("", |origin| origin.route().label()),
            right: &page,
        },
    )?;

    if let Some(document) = note.document() {
        let title_bounds = TextBounds::new(22, 150, 458, 190);
        Text::new(document.title(), Point::new(22, 184), heading)
            .draw_clipped(display, title_bounds)?;
        let text_bounds = TextBounds::new(22, 212, 458, 722);
        if let Some(page) = note.current_page() {
            for (index, line) in page.lines().iter().enumerate() {
                let style = match line.kind() {
                    crate::atlas_markdown::AtlasMarkdownLineKind::Heading1
                    | crate::atlas_markdown::AtlasMarkdownLineKind::Heading2
                    | crate::atlas_markdown::AtlasMarkdownLineKind::Heading3 => heading,
                    crate::atlas_markdown::AtlasMarkdownLineKind::Body
                    | crate::atlas_markdown::AtlasMarkdownLineKind::List
                    | crate::atlas_markdown::AtlasMarkdownLineKind::Separator => body,
                };
                let baseline = 226 + index as i32 * i32::from(body.line_height());
                if baseline >= text_bounds.bottom {
                    break;
                }
                Text::new(line.text(), Point::new(22, baseline), style)
                    .draw_clipped(display, text_bounds)?;
            }
        }
    } else {
        Text::new(note.status().label(), Point::new(22, 208), heading).draw(display)?;
        Text::new(
            "Choose a note from Atlas and refresh.",
            Point::new(22, 252),
            body,
        )
        .draw(display)?;
    }
    draw_footer(display, state.display, "UP PREV  DOWN NEXT  HOLD BOOT BACK")
}

fn page_label(
    index: usize,
    count: usize,
    overflow: Option<crate::atlas_markdown::AtlasMarkdownOverflow>,
) -> String {
    if count == 0 {
        "PAGE -".into()
    } else {
        let suffix = if matches!(
            overflow,
            Some(crate::atlas_markdown::AtlasMarkdownOverflow::Truncated)
        ) {
            "+"
        } else {
            ""
        };
        format!("PAGE {}/{}{suffix}", index.saturating_add(1), count)
    }
}
