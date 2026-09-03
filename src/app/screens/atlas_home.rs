//! Static, e-paper-native Atlas Lite diagnostics Home.

use core::convert::Infallible;

use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{Drawable, Point, Primitive, Size},
    primitives::{PrimitiveStyle, Rectangle},
};

use crate::{
    app::{
        menu::atlas_home_entries,
        state::AppState,
        typography::{Text, UiTextStyle},
        widgets::{
            footer::draw_footer,
            header::draw_header,
            status_row::{draw_status_row, StatusRow},
        },
    },
    board_services::BoardSnapshot,
    network::NetworkSnapshot,
    orientation::OrientedFrameBuffer,
    storage::StorageSnapshot,
};

const DIAGNOSTIC_ROW_COUNT: usize = 6;
const HOME_MENU_X: i32 = 22;
const HOME_MENU_FIRST_TOP: i32 = 456;
const HOME_MENU_ROW_STEP: i32 = 48;
const HOME_MENU_WIDTH: u32 = 436;
const HOME_MENU_HEIGHT: u32 = 36;

/// A redacted, hardware-independent Home view model built only from existing
/// read-only snapshots. It deliberately retains status labels, never source
/// snapshots or network identifiers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasHomeDiagnostics {
    title: &'static str,
    rows: [DiagnosticRow; DIAGNOSTIC_ROW_COUNT],
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DiagnosticRow {
    label: &'static str,
    value: String,
}

impl AtlasHomeDiagnostics {
    #[must_use]
    pub fn from_snapshots(
        board: &BoardSnapshot,
        storage: &StorageSnapshot,
        network: &NetworkSnapshot,
    ) -> Self {
        let battery = board
            .power
            .and_then(|power| power.battery_percent)
            .map_or_else(|| "--".into(), |percent| format!("{percent}%"));
        let rtc = if board.rtc_clock_integrity_was_lost {
            "CHECK"
        } else if board.rtc.is_some() {
            "READY"
        } else {
            "UNAVAILABLE"
        };

        Self {
            title: "ATLAS LITE",
            rows: [
                DiagnosticRow {
                    label: "Display",
                    value: "READY".into(),
                },
                DiagnosticRow {
                    label: "Input",
                    value: "READY".into(),
                },
                DiagnosticRow {
                    label: "SD",
                    value: storage.status_label().into(),
                },
                DiagnosticRow {
                    label: "Wi-Fi",
                    value: network.wifi_state.label().into(),
                },
                DiagnosticRow {
                    label: "Battery",
                    value: battery,
                },
                DiagnosticRow {
                    label: "RTC",
                    value: rtc.into(),
                },
            ],
        }
    }

    #[must_use]
    pub const fn title(&self) -> &'static str {
        self.title
    }

    #[must_use]
    pub fn rows(&self) -> [(&'static str, &str); DIAGNOSTIC_ROW_COUNT] {
        self.rows
            .each_ref()
            .map(|row| (row.label, row.value.as_str()))
    }
}

/// Return the logical portrait rectangle for one visible Atlas Home entry.
/// Keeping this geometry pure makes the screen bounds host-testable without
/// acquiring a panel or other hardware handle.
#[must_use]
pub(crate) fn atlas_home_menu_rect(index: usize) -> Option<Rectangle> {
    if index >= atlas_home_entries().len() {
        return None;
    }

    Some(Rectangle::new(
        Point::new(
            HOME_MENU_X,
            HOME_MENU_FIRST_TOP + index as i32 * HOME_MENU_ROW_STEP,
        ),
        Size::new(HOME_MENU_WIDTH, HOME_MENU_HEIGHT),
    ))
}

/// Render the initial static Home screen. It receives snapshots through
/// `AppState`; hardware and panel transport remain owned by the main loop.
pub fn render_atlas_home(
    display: &mut OrientedFrameBuffer<'_>,
    state: &AppState,
) -> Result<(), Infallible> {
    let diagnostics =
        AtlasHomeDiagnostics::from_snapshots(&state.board, &state.storage, &state.network);
    let body = state.display.body_style();
    let heading = state.display.heading_style();

    draw_header(
        display,
        state.display,
        diagnostics.title(),
        "DEVICE DIAGNOSTICS",
    )?;
    draw_status_row(
        display,
        state.display,
        StatusRow {
            left: "STATIC",
            middle: "SNAPSHOT",
            right: "E-PAPER",
        },
    )?;

    for (index, (label, value)) in diagnostics.rows().iter().enumerate() {
        diagnostic_line(display, 162 + index as i32 * 40, label, value, body)?;
    }

    Text::new("HOME", Point::new(22, 434), heading).draw(display)?;
    for (index, entry) in atlas_home_entries().iter().enumerate() {
        let row = atlas_home_menu_rect(index).expect("Atlas Home entries have visible rows");
        menu_line(
            display,
            row,
            entry.label,
            state.home_selected == index,
            body,
        )?;
    }

    draw_footer(
        display,
        state.display,
        "UP DOWN MOVE  SELECT PENDING  HOLD BOOT BACK",
    )?;
    Ok(())
}

fn diagnostic_line(
    display: &mut OrientedFrameBuffer<'_>,
    y: i32,
    label: &str,
    value: &str,
    style: UiTextStyle,
) -> Result<(), Infallible> {
    Text::new(label, Point::new(22, y), style).draw(display)?;
    Text::new(value, Point::new(210, y), style).draw(display)?;
    Ok(())
}

fn menu_line(
    display: &mut OrientedFrameBuffer<'_>,
    row: Rectangle,
    label: &str,
    selected: bool,
    style: UiTextStyle,
) -> Result<(), Infallible> {
    let top = row.top_left.y;
    row.into_styled(if selected {
        PrimitiveStyle::with_stroke(BinaryColor::On, 3)
    } else {
        PrimitiveStyle::with_stroke(BinaryColor::On, 1)
    })
    .draw(display)?;
    Text::new(
        if selected { ">" } else { " " },
        Point::new(34, top + 24),
        style,
    )
    .draw(display)?;
    Text::new(label, Point::new(62, top + 24), style).draw(display)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::AtlasHomeDiagnostics;
    use crate::{
        board_services::BoardSnapshot,
        network::{NetworkSnapshot, WifiConnectionState},
        power::PowerSnapshot,
        rtc::RtcDateTime,
        storage::StorageSnapshot,
    };

    #[test]
    fn diagnostics_model_maps_existing_snapshots_to_static_labels() {
        let board = BoardSnapshot {
            rtc: Some(RtcDateTime {
                year: 2026,
                month: 9,
                day: 3,
                weekday: 4,
                hour: 12,
                minute: 0,
                second: 0,
            }),
            power: Some(PowerSnapshot {
                battery_percent: Some(87),
                battery_voltage_mv: Some(4_012),
                vbus_present: false,
                charging: false,
            }),
            ..BoardSnapshot::default()
        };
        let storage = StorageSnapshot {
            mounted: true,
            ..StorageSnapshot::default()
        };
        let network = NetworkSnapshot {
            wifi_state: WifiConnectionState::Connected,
            ..NetworkSnapshot::default()
        };

        let diagnostics = AtlasHomeDiagnostics::from_snapshots(&board, &storage, &network);

        assert_eq!(diagnostics.title(), "ATLAS LITE");
        assert_eq!(
            diagnostics.rows(),
            [
                ("Display", "READY"),
                ("Input", "READY"),
                ("SD", "SD EMPTY"),
                ("Wi-Fi", "CONNECTED"),
                ("Battery", "87%"),
                ("RTC", "READY"),
            ]
        );
    }

    #[test]
    fn diagnostics_model_redacts_network_identifiers_and_secret_like_values() {
        let network = NetworkSnapshot {
            wifi_state: WifiConnectionState::Connected,
            ssid: Some("private-wifi-name".into()),
            ntp_server: "at_v1_token_must_not_render".into(),
            error: Some("wifi-password-must-not-render".into()),
            ..NetworkSnapshot::default()
        };

        let diagnostics = AtlasHomeDiagnostics::from_snapshots(
            &BoardSnapshot::default(),
            &StorageSnapshot::default(),
            &network,
        );
        let rendered_model = format!("{diagnostics:?}");

        assert_eq!(diagnostics.rows()[3], ("Wi-Fi", "CONNECTED"));
        assert!(!rendered_model.contains("private-wifi-name"));
        assert!(!rendered_model.contains("at_v1_token_must_not_render"));
        assert!(!rendered_model.contains("wifi-password-must-not-render"));
    }
}
