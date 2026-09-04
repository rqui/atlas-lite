//! Minimal e-paper-safe Note reader surface for M3.3 loading state.

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
    draw_header(display, state.display, "ATLAS LITE", "NOTE")?;
    draw_status_row(
        display,
        state.display,
        StatusRow {
            left: note.status().label(),
            middle: note.origin().map_or("", |origin| origin.route().label()),
            right: "ID",
        },
    )?;

    if let Some(document) = note.document() {
        let title_bounds = TextBounds::new(22, 150, 458, 190);
        Text::new(document.title(), Point::new(22, 184), heading)
            .draw_clipped(display, title_bounds)?;
        for (index, line) in document.body().lines().take(12).enumerate() {
            let baseline = 226 + index as i32 * i32::from(body.line_height());
            let bounds = TextBounds::new(22, baseline - 28, 458, baseline + 4);
            Text::new(line, Point::new(22, baseline), body).draw_clipped(display, bounds)?;
        }
        if document.body().is_empty() {
            Text::new("EMPTY NOTE", Point::new(22, 232), body).draw(display)?;
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
    draw_footer(display, state.display, "HOLD BOOT BACK")
}
