//! Explicit M1 placeholders for Atlas routes whose data flows arrive later.

use core::convert::Infallible;

use embedded_graphics::prelude::Point;

use crate::{
    app::{
        router::AtlasRoute,
        state::AppState,
        typography::Text,
        widgets::{
            footer::draw_footer,
            header::draw_atlas_header,
            status_row::{draw_status_row, StatusRow},
        },
    },
    orientation::OrientedFrameBuffer,
};

/// Render an intentionally non-networked M1 Atlas shell surface.
pub fn render_atlas_shell(
    display: &mut OrientedFrameBuffer<'_>,
    state: &AppState,
    route: AtlasRoute,
) -> Result<(), Infallible> {
    let body = state.display.body_style();
    let heading = state.display.heading_style();
    let (title, hint) = atlas_shell_content(route);

    draw_atlas_header(display, state.display, title)?;
    draw_status_row(
        display,
        state.display,
        StatusRow {
            left: if route == AtlasRoute::Capture {
                "VOICE"
            } else {
                "M1"
            },
            middle: if route == AtlasRoute::Capture {
                state.voice_notes.mode.label()
            } else {
                "SHELL"
            },
            right: state.network.wifi_state.label(),
        },
    )?;
    Text::new(title, Point::new(22, 244), heading).draw(display)?;
    let capture = route == AtlasRoute::Capture;
    Text::new(
        if capture {
            state.voice_notes.mode.label()
        } else {
            "M1 shell placeholder"
        },
        Point::new(22, 326),
        body,
    )
    .draw(display)?;
    Text::new(
        if capture {
            "PCM16 MONO 16 KHZ"
        } else {
            "No Atlas data is loaded yet."
        },
        Point::new(22, 370),
        body,
    )
    .draw(display)?;
    Text::new(hint, Point::new(22, 458), body).draw(display)?;
    if capture {
        let status = state
            .voice_notes
            .error
            .as_deref()
            .or(state.voice_notes.export_status.as_deref())
            .unwrap_or("Ready to record when MicroSD and microphone are available");
        for (line, baseline) in capture_status_lines(status).iter().zip([510, 544, 578]) {
            Text::new(line, Point::new(22, baseline), body).draw(display)?;
        }
    }
    draw_footer(display, state.display, atlas_shell_footer(route))
}

/// Bounded, word-aware capture feedback. Unlike byte/character truncation it
/// keeps the actionable cause visible on the e-paper surface.
#[must_use]
pub fn capture_status_lines(status: &str) -> Vec<String> {
    const MAX_CHARS: usize = 31;
    const MAX_LINES: usize = 3;
    let mut lines = Vec::with_capacity(MAX_LINES);
    let mut line = String::new();
    for word in status.split_whitespace() {
        let projected = line.chars().count() + usize::from(!line.is_empty()) + word.chars().count();
        if projected > MAX_CHARS && !line.is_empty() {
            lines.push(core::mem::take(&mut line));
            if lines.len() == MAX_LINES {
                return lines;
            }
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.extend(
            word.chars()
                .take(MAX_CHARS.saturating_sub(line.chars().count())),
        );
    }
    if !line.is_empty() && lines.len() < MAX_LINES {
        lines.push(line);
    }
    lines
}

#[must_use]
pub const fn atlas_shell_content(route: AtlasRoute) -> (&'static str, &'static str) {
    match route {
        AtlasRoute::Library => ("LIBRARY", "SELECT OPEN NOTE"),
        AtlasRoute::Search => ("SEARCH", "SELECT OPEN NOTE"),
        AtlasRoute::Views => ("VIEWS", "SELECT OPEN NOTE"),
        AtlasRoute::Note => ("NOTE", "HOLD BOOT TO RETURN"),
        AtlasRoute::Capture => ("VOICE CAPTURE", "SELECT START OR STOP"),
        AtlasRoute::Settings => ("SETTINGS", "DEVICE MAINTENANCE"),
        AtlasRoute::Home => ("HOME", "HOME IS RENDERED SEPARATELY"),
    }
}

#[must_use]
const fn atlas_shell_footer(route: AtlasRoute) -> &'static str {
    match route {
        AtlasRoute::Library | AtlasRoute::Search | AtlasRoute::Views => {
            "SELECT OPEN  HOLD BOOT BACK"
        }
        AtlasRoute::Capture => "SELECT START/STOP  HOLD BOOT BACK",
        AtlasRoute::Note | AtlasRoute::Settings | AtlasRoute::Home => "HOLD BOOT BACK",
    }
}

#[cfg(test)]
mod tests {
    use super::{atlas_shell_content, capture_status_lines};
    use crate::app::router::AtlasRoute;

    #[test]
    fn only_permitted_m1_origins_offer_note_opening() {
        for route in [AtlasRoute::Library, AtlasRoute::Search, AtlasRoute::Views] {
            assert_eq!(atlas_shell_content(route).1, "SELECT OPEN NOTE");
        }
        for route in [AtlasRoute::Capture, AtlasRoute::Settings, AtlasRoute::Note] {
            assert_ne!(atlas_shell_content(route).1, "SELECT OPEN NOTE");
        }
    }

    #[test]
    fn capture_feedback_wraps_the_actionable_storage_cause() {
        assert_eq!(
            capture_status_lines("MicroSD unavailable; insert accessible card"),
            ["MicroSD unavailable; insert", "accessible card"]
        );
    }
}
