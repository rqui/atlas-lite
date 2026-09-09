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
        row.into_styled(if active {
            PrimitiveStyle::with_stroke(BinaryColor::On, 3)
        } else {
            PrimitiveStyle::with_stroke(BinaryColor::On, 1)
        })
        .draw(display)?;
        Text::new(
            &note.title,
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
