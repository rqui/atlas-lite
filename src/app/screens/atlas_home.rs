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
    atlas_state::AtlasConnectionState,
    board_services::BoardSnapshot,
    network::NetworkSnapshot,
    orientation::OrientedFrameBuffer,
    storage::StorageSnapshot,
};

const DIAGNOSTIC_ROW_COUNT: usize = 6;
const HOME_MENU_X: i32 = 22;
const HOME_MENU_FIRST_TOP: i32 = 144;
const HOME_MENU_ROW_STEP: i32 = 104;
const HOME_MENU_WIDTH: u32 = 436;
const HOME_MENU_HEIGHT: u32 = 80;
const ATLAS_HOME_FOOTER_HINT: &str = "UP/DOWN SELECT";

/// Compact Home control legend that remains visible at every supported font
/// family and size profile.
#[must_use]
pub const fn atlas_home_footer_hint() -> &'static str {
    ATLAS_HOME_FOOTER_HINT
}

/// A redacted, hardware-independent Home view model built only from existing
/// read-only snapshots. It deliberately retains status labels, never source
/// snapshots or network identifiers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasHomeDiagnostics {
    title: &'static str,
    rows: [DiagnosticRow; DIAGNOSTIC_ROW_COUNT],
}

/// The compact, secret-free product content shown by the Atlas Home renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasHomeContent {
    status: [String; 3],
}

impl AtlasHomeContent {
    #[must_use]
    pub fn status(&self) -> [&str; 3] {
        self.status.each_ref().map(String::as_str)
    }
}

/// Build Home content from already-owned snapshots only. Rendering this model
/// never calls AtlasClient; data refresh remains an explicit AppState action.
#[must_use]
pub fn atlas_home_content(state: &AppState) -> AtlasHomeContent {
    let battery = state
        .board
        .power
        .and_then(|power| power.battery_percent)
        .map_or_else(|| "--".into(), |percent| format!("{percent}%"));
    AtlasHomeContent {
        status: [
            atlas_connection_label(
                if state.atlas_home_connection == AtlasConnectionState::Unconfigured {
                    state.atlas.connection
                } else {
                    state.atlas_home_connection
                },
            )
            .into(),
            battery,
            wifi_label(state.network.wifi_state).into(),
        ],
    }
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
            title: crate::app::PRODUCT_VISIBLE_NAME,
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
    let content = atlas_home_content(state);
    let body = state.display.body_style();

    draw_header(
        display,
        state.display,
        crate::app::PRODUCT_VISIBLE_NAME,
        "HOME",
    )?;
    draw_status_row(
        display,
        state.display,
        StatusRow {
            left: content.status()[0],
            middle: content.status()[1],
            right: content.status()[2],
        },
    )?;

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

    draw_footer(display, state.display, atlas_home_footer_hint())?;
    Ok(())
}

const fn atlas_connection_label(
    connection: crate::atlas_state::AtlasConnectionState,
) -> &'static str {
    use crate::atlas_state::AtlasConnectionState;

    match connection {
        AtlasConnectionState::Unconfigured => "SETUP",
        AtlasConnectionState::Connecting => "CONNECTING",
        AtlasConnectionState::Connected => "CONNECTED",
        AtlasConnectionState::Unauthorized => "AUTH ERROR",
        AtlasConnectionState::Forbidden => "FORBIDDEN",
        AtlasConnectionState::Timeout => "TIMEOUT",
        AtlasConnectionState::ServerError => "SERVER ERROR",
        AtlasConnectionState::Offline => "OFFLINE",
    }
}

const fn wifi_label(connection: crate::network::WifiConnectionState) -> &'static str {
    use crate::network::WifiConnectionState;

    match connection {
        WifiConnectionState::Disabled => "OFFLINE",
        WifiConnectionState::ConfigurationMissing => "SETUP",
        WifiConnectionState::Connecting => "CONNECTING",
        WifiConnectionState::Connected => "CONNECTED",
        WifiConnectionState::Failed => "ERROR",
    }
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
    use super::{
        atlas_home_content, atlas_home_footer_hint, atlas_home_menu_rect, render_atlas_home,
        AtlasHomeDiagnostics,
    };
    use crate::{
        app::display::{DisplayPreferences, UiFontFamily, UiFontSize},
        app::AppState,
        atlas_state::{AtlasConnectionState, AtlasSnapshot},
        board_services::BoardSnapshot,
        framebuffer::FrameBuffer,
        network::{NetworkSnapshot, WifiConnectionState},
        orientation::{DisplayOrientation, OrientedFrameBuffer},
        power::PowerSnapshot,
        rtc::RtcDateTime,
        storage::StorageSnapshot,
    };

    #[test]
    fn menu_rows_are_non_overlapping_and_fit() {
        let mut bottom = 0;
        for index in 0..5 {
            let row = atlas_home_menu_rect(index).unwrap();
            assert!(row.top_left.y >= bottom);
            assert!(row.bottom_right().unwrap().y < 746);
            bottom = row.bottom_right().unwrap().y + 1;
        }
        assert!(atlas_home_menu_rect(5).is_none());
    }

    #[test]
    fn home_content_combines_connection_hardware_and_bounded_atlas_labels() {
        let mut state = AppState::default();
        state.update_board_snapshot(BoardSnapshot {
            rtc: Some(RtcDateTime {
                year: 2026,
                month: 9,
                day: 4,
                weekday: 5,
                hour: 9,
                minute: 5,
                second: 0,
            }),
            power: Some(PowerSnapshot {
                battery_percent: Some(50),
                ..PowerSnapshot::default()
            }),
            ..BoardSnapshot::default()
        });
        state.update_network_snapshot(NetworkSnapshot {
            wifi_state: WifiConnectionState::Connected,
            ..NetworkSnapshot::default()
        });
        state.update_atlas_snapshot(AtlasSnapshot {
            connection: AtlasConnectionState::Offline,
        });
        let content = atlas_home_content(&state);

        assert_eq!(content.status(), ["OFFLINE", "50%", "CONNECTED"]);
    }

    #[test]
    fn menu_first_home_renders_the_same_five_targets_for_every_font_profile() {
        for font_family in [UiFontFamily::Inter, UiFontFamily::AtkinsonHyperlegible] {
            for font_size in [UiFontSize::Compact, UiFontSize::Standard, UiFontSize::Large] {
                let mut state = AppState::default();
                state.display = DisplayPreferences {
                    font_family,
                    font_size,
                };
                state.home_selected = 4;
                let mut frame = FrameBuffer::new_white();
                let mut display =
                    OrientedFrameBuffer::new(&mut frame, DisplayOrientation::Portrait);
                render_atlas_home(&mut display, &state).unwrap();
                assert!(frame.as_bytes().iter().any(|byte| *byte != 0xFF));
            }
        }
    }

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

        assert_eq!(diagnostics.title(), "ATLAS");
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

    #[test]
    fn footer_hint_fits_every_supported_font_profile() {
        assert_eq!(atlas_home_footer_hint(), "UP/DOWN SELECT");

        for font_family in [UiFontFamily::Inter, UiFontFamily::AtkinsonHyperlegible] {
            for font_size in [UiFontSize::Compact, UiFontSize::Standard, UiFontSize::Large] {
                let preferences = DisplayPreferences {
                    font_family,
                    font_size,
                };
                assert!(
                    preferences
                        .footer_style()
                        .text_width(atlas_home_footer_hint())
                        <= 448,
                    "{font_family:?} {font_size:?} footer overflows"
                );
            }
        }
    }
}
