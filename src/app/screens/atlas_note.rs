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
    atlas_note::AtlasNoteStatus,
    orientation::OrientedFrameBuffer,
};

/// Shared reader viewport. Header and status end at y=122; the footer starts
/// at y=746, leaving 608 px of useful content height on the 480 x 800 canvas.
pub const NOTE_TEXT_TOP: i32 = 136;
pub const NOTE_TEXT_BOTTOM: i32 = 744;
pub const NOTE_TEXT_LEFT: i32 = 22;
pub const NOTE_TEXT_RIGHT: i32 = 458;

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
    draw_header(
        display,
        state.display,
        crate::app::PRODUCT_VISIBLE_NAME,
        "NOTE",
    )?;
    draw_status_row(
        display,
        state.display,
        StatusRow {
            left: note.status().label(),
            middle: note.origin().map_or("", |origin| origin.route().label()),
            right: &page,
        },
    )?;

    if note.document().is_some() {
        let text_bounds = TextBounds::new(
            NOTE_TEXT_LEFT,
            NOTE_TEXT_TOP,
            NOTE_TEXT_RIGHT,
            NOTE_TEXT_BOTTOM,
        );
        if let Some(page) = note.current_page() {
            let mut baseline = NOTE_TEXT_TOP;
            for line in page.lines() {
                let style = match line.kind() {
                    crate::atlas_markdown::AtlasMarkdownLineKind::Heading1
                    | crate::atlas_markdown::AtlasMarkdownLineKind::Heading2
                    | crate::atlas_markdown::AtlasMarkdownLineKind::Heading3 => heading,
                    crate::atlas_markdown::AtlasMarkdownLineKind::Body
                    | crate::atlas_markdown::AtlasMarkdownLineKind::List
                    | crate::atlas_markdown::AtlasMarkdownLineKind::Separator => body,
                };
                baseline += i32::from(style.line_height());
                if baseline > text_bounds.bottom {
                    break;
                }
                Text::new(line.text(), Point::new(NOTE_TEXT_LEFT, baseline), style)
                    .draw_clipped(display, text_bounds)?;
            }
        }
    } else {
        Text::new(note.status().label(), Point::new(22, 208), heading).draw(display)?;
        let message = match note.status() {
            AtlasNoteStatus::Idle => "Choose a note from Atlas and refresh.",
            AtlasNoteStatus::Loading => "Loading selected note...",
            AtlasNoteStatus::Error(_) => "Selected note could not be loaded.",
            AtlasNoteStatus::Loaded | AtlasNoteStatus::OfflineCached => "Empty note document.",
        };
        Text::new(message, Point::new(22, 252), body).draw(display)?;
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
