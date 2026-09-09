//! Read-only, SD-backed Atlas Voice Recordings library.

use core::convert::Infallible;

use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{Drawable, Point, Primitive, Size},
    primitives::{PrimitiveStyle, Rectangle},
};

use crate::{
    app::{
        state::AppState,
        typography::{Text, TextBounds, UiTextRole},
        widgets::{footer::draw_footer, header::draw_atlas_topbar},
    },
    orientation::OrientedFrameBuffer,
    voice_notes::format_duration,
};

const FIRST_ROW_Y: i32 = 112;
const ROW_HEIGHT: i32 = 88;
const VISIBLE_ROWS: usize = 6;

#[must_use]
fn recording_date_title(recorded_at: &str) -> String {
    let bytes = recorded_at.as_bytes();
    let valid_date = bytes.len() >= 10
        && bytes[..10].iter().all(u8::is_ascii)
        && bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit);
    if !valid_date {
        return "DATE UNKNOWN".into();
    }

    let date = &recorded_at[..10];
    let time = bytes[10..].windows(5).position(|candidate| {
        candidate[0].is_ascii_digit()
            && candidate[1].is_ascii_digit()
            && candidate[2] == b':'
            && candidate[3].is_ascii_digit()
            && candidate[4].is_ascii_digit()
    });
    time.map_or_else(
        || date.into(),
        |offset| format!("{date} {}", &recorded_at[10 + offset..15 + offset]),
    )
}

pub fn render_atlas_voice_recordings(
    display: &mut OrientedFrameBuffer<'_>,
    state: &AppState,
) -> Result<(), Infallible> {
    draw_atlas_topbar(display, state, "VOICE RECORDINGS")?;
    draw_footer(display, state.display, "SELECT PLAY/STOP · BOOT BACK")?;
    if state.voice_notes.notes.is_empty() {
        Text::new(
            if !state.storage.mounted {
                "MicroSD unavailable"
            } else if state.atlas_voice_sync.has_pending() {
                "Syncing recordings…"
            } else {
                state
                    .atlas_voice_sync
                    .feedback
                    .unwrap_or("No voice recordings")
            },
            Point::new(24, 230),
            state
                .display
                .text_style(UiTextRole::Heading, BinaryColor::On),
        )
        .draw(display)?;
        return Ok(());
    }

    let selected = state.voice_notes.selected.saturating_sub(2);
    let start = selected.saturating_sub(VISIBLE_ROWS - 1);
    for (visible, note) in state
        .voice_notes
        .notes
        .iter()
        .skip(start)
        .take(VISIBLE_ROWS)
        .enumerate()
    {
        let index = start + visible;
        let top = FIRST_ROW_Y + visible as i32 * ROW_HEIGHT;
        let row = Rectangle::new(Point::new(14, top), Size::new(452, 76));
        let active = index == selected;
        let title = recording_date_title(&note.recorded_at);
        row.into_styled(if active {
            PrimitiveStyle::with_stroke(BinaryColor::On, 3)
        } else {
            PrimitiveStyle::with_stroke(BinaryColor::On, 1)
        })
        .draw(display)?;
        Text::new(
            &title,
            Point::new(30, top + 31),
            state
                .display
                .text_style(UiTextRole::Heading, BinaryColor::On),
        )
        .draw_clipped(display, TextBounds::new(30, top + 6, 352, top + 39))?;
        Text::new(
            &format!(
                "{} · {} KB",
                format_duration(note.duration_seconds),
                note.wav_bytes / 1024
            ),
            Point::new(30, top + 61),
            state
                .display
                .text_style(UiTextRole::Detail, BinaryColor::On),
        )
        .draw_clipped(display, TextBounds::new(30, top + 38, 420, top + 70))?;
        if state.voice_notes.playing_file.as_deref() == Some(&note.file_name) {
            let playback_status = format!("VOL {}%", state.audio.volume_percent);
            Text::new(
                &playback_status,
                Point::new(358, top + 31),
                state
                    .display
                    .text_style(UiTextRole::Detail, BinaryColor::On),
            )
            .draw(display)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::recording_date_title;

    #[test]
    fn recording_title_is_date_first_without_voice_note_prefix() {
        assert_eq!(
            recording_date_title("2026-09-09T08:20:31.000Z"),
            "2026-09-09 08:20"
        );
        assert_eq!(
            recording_date_title("2026-09-09  08:20:31"),
            "2026-09-09 08:20"
        );
        assert_eq!(recording_date_title("2026-09-09"), "2026-09-09");
        assert_eq!(recording_date_title("DATE UNKNOWN"), "DATE UNKNOWN");
    }
}
