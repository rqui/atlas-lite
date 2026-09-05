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
        typography::{Text, TextBounds, UiTextStyle},
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
const HOME_CONTENT_LEFT: i32 = 22;
const HOME_CONTENT_RIGHT: i32 = 458;
const CAPTURE_ACTION_BASELINE: i32 = 398;
const HOME_SECTION_BASELINE: i32 = 434;
const HOME_MENU_X: i32 = 22;
const HOME_MENU_FIRST_TOP: i32 = 456;
const HOME_MENU_ROW_STEP: i32 = 48;
const HOME_MENU_WIDTH: u32 = 436;
const HOME_MENU_HEIGHT: u32 = 36;
const ATLAS_HOME_FOOTER_HINT: &str = "MOVE SELECT HOLD BOOT BACK";

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
    time: String,
    recent_notes: Vec<String>,
    view_shortcuts: Vec<String>,
}

impl AtlasHomeContent {
    #[must_use]
    pub fn status(&self) -> [&str; 3] {
        self.status.each_ref().map(String::as_str)
    }

    #[must_use]
    pub fn time(&self) -> &str {
        &self.time
    }

    #[must_use]
    pub fn recent_notes(&self) -> &[String] {
        &self.recent_notes
    }

    #[must_use]
    pub fn view_shortcuts(&self) -> &[String] {
        &self.view_shortcuts
    }

    #[must_use]
    pub const fn capture_label(&self) -> &'static str {
        "CAPTURE >"
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
    let time = state.board.rtc.map_or_else(
        || "--:--".into(),
        |rtc| format!("{:02}:{:02}", rtc.hour, rtc.minute),
    );

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
        time,
        recent_notes: state.atlas_home.recent_notes().to_vec(),
        view_shortcuts: state.atlas_home.view_shortcuts().to_vec(),
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
    let heading = state.display.heading_style();

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

    diagnostic_line(display, 150, "TIME", content.time(), body)?;
    Text::new("RECENT", Point::new(22, 188), heading).draw(display)?;
    summary_lines(
        display,
        222,
        content.recent_notes(),
        "NO RECENT NOTES",
        body,
    )?;
    Text::new("VIEWS", Point::new(22, 316), heading).draw(display)?;
    summary_lines(
        display,
        350,
        content.view_shortcuts(),
        "NO VIEW SHORTCUTS",
        body,
    )?;
    Text::new(
        content.capture_label(),
        Point::new(HOME_CONTENT_LEFT, CAPTURE_ACTION_BASELINE),
        heading,
    )
    .draw(display)?;

    Text::new(
        "HOME",
        Point::new(HOME_CONTENT_LEFT, HOME_SECTION_BASELINE),
        heading,
    )
    .draw(display)?;
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

fn summary_lines(
    display: &mut OrientedFrameBuffer<'_>,
    first_y: i32,
    labels: &[String],
    empty_label: &str,
    style: UiTextStyle,
) -> Result<(), Infallible> {
    if labels.is_empty() {
        draw_home_label(display, empty_label, first_y, style)?;
        return Ok(());
    }
    for (index, label) in labels.iter().enumerate() {
        draw_home_label(display, label, first_y + index as i32 * 30, style)?;
    }
    Ok(())
}

fn draw_home_label(
    display: &mut OrientedFrameBuffer<'_>,
    label: &str,
    baseline: i32,
    style: UiTextStyle,
) -> Result<(), Infallible> {
    let bounds = TextBounds::new(
        HOME_CONTENT_LEFT,
        baseline - i32::from(style.line_height()),
        HOME_CONTENT_RIGHT,
        baseline + 4,
    );
    let fitted = fit_home_label(label, style, bounds.width());
    Text::new(&fitted, Point::new(HOME_CONTENT_LEFT, baseline), style)
        .draw_clipped(display, bounds)?;
    Ok(())
}

/// Fit a Home summary label to the actual bitmap strike, including wide glyphs.
/// The result is bounded by both the available measured width and a short
/// fallback marker when the source text does not fit.
pub(crate) fn fit_home_label(label: &str, style: UiTextStyle, max_width: i32) -> String {
    if max_width <= 0 {
        return String::new();
    }
    if style.text_width(label) <= max_width {
        return label.to_owned();
    }

    let ellipsis = "…";
    let ellipsis_width = style.text_width(ellipsis);
    let mut fitted = String::new();
    let mut width = 0;
    for character in label.chars() {
        let character_width = style.text_width(&character.to_string());
        if width + character_width + ellipsis_width > max_width {
            break;
        }
        fitted.push(character);
        width += character_width;
    }
    if ellipsis_width <= max_width {
        fitted.push_str(ellipsis);
    }
    fitted
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
    use super::{
        atlas_home_content, atlas_home_footer_hint, fit_home_label, AtlasHomeDiagnostics,
        CAPTURE_ACTION_BASELINE, HOME_SECTION_BASELINE,
    };
    use crate::{
        app::display::{DisplayPreferences, UiFontFamily, UiFontSize},
        app::typography::{Text, TextBounds},
        app::AppState,
        atlas_client::{AtlasClient, MockAtlasTransport, MockTransportOutcome},
        atlas_state::{AtlasConnectionState, AtlasSnapshot},
        board_services::BoardSnapshot,
        framebuffer::FrameBuffer,
        network::{NetworkSnapshot, WifiConnectionState},
        orientation::{DisplayOrientation, OrientedFrameBuffer},
        power::PowerSnapshot,
        rtc::RtcDateTime,
        storage::StorageSnapshot,
    };

    fn rendered_ink_bounds(
        text: &str,
        baseline: i32,
        style: super::UiTextStyle,
    ) -> (i32, i32, i32, i32) {
        let orientation = DisplayOrientation::Portrait;
        let mut frame = FrameBuffer::new_white();
        let mut display = OrientedFrameBuffer::new(&mut frame, orientation);
        Text::new(
            text,
            embedded_graphics::prelude::Point::new(22, baseline),
            style,
        )
        .draw(&mut display)
        .unwrap();

        let mut bounds = (480, 800, -1, -1);
        for y in 0..800 {
            for x in 0..480 {
                let native = orientation
                    .map_logical_to_native(embedded_graphics::prelude::Point::new(x, y))
                    .unwrap();
                if frame.is_black(native) == Some(true) {
                    bounds.0 = bounds.0.min(x);
                    bounds.1 = bounds.1.min(y);
                    bounds.2 = bounds.2.max(x);
                    bounds.3 = bounds.3.max(y);
                }
            }
        }
        bounds
    }

    #[test]
    fn capture_action_and_home_heading_ink_regions_do_not_overlap_for_any_font_profile() {
        for font_family in [UiFontFamily::Inter, UiFontFamily::AtkinsonHyperlegible] {
            for font_size in [UiFontSize::Compact, UiFontSize::Standard, UiFontSize::Large] {
                let preferences = DisplayPreferences {
                    font_family,
                    font_size,
                };
                let capture = rendered_ink_bounds(
                    "CAPTURE >",
                    CAPTURE_ACTION_BASELINE,
                    preferences.heading_style(),
                );
                let home =
                    rendered_ink_bounds("HOME", HOME_SECTION_BASELINE, preferences.heading_style());

                assert!(
                    capture.3 < home.1,
                    "{font_family:?} {font_size:?}: capture y={}..{} overlaps home y={}..{}",
                    capture.1,
                    capture.3,
                    home.1,
                    home.3
                );
            }
        }
    }

    #[test]
    fn wide_glyph_home_label_fits_the_rendered_width_for_any_font_profile() {
        let wide_title = "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW";
        for font_family in [UiFontFamily::Inter, UiFontFamily::AtkinsonHyperlegible] {
            for font_size in [UiFontSize::Compact, UiFontSize::Standard, UiFontSize::Large] {
                let preferences = DisplayPreferences {
                    font_family,
                    font_size,
                };
                let style = preferences.body_style();
                let fitted = fit_home_label(wide_title, style, 436);
                assert!(style.text_width(&fitted) <= 436);
                assert!(fitted.ends_with('…'));

                let orientation = DisplayOrientation::Portrait;
                let mut frame = FrameBuffer::new_white();
                let mut display = OrientedFrameBuffer::new(&mut frame, orientation);
                Text::new(
                    &fitted,
                    embedded_graphics::prelude::Point::new(22, 222),
                    style,
                )
                .draw_clipped(&mut display, TextBounds::new(22, 190, 458, 240))
                .unwrap();
                for y in 190..240 {
                    for x in 458..480 {
                        let native = orientation
                            .map_logical_to_native(embedded_graphics::prelude::Point::new(x, y))
                            .unwrap();
                        assert_eq!(
                            frame.is_black(native),
                            Some(false),
                            "{font_family:?} {font_size:?}: wide label escaped right clipping edge"
                        );
                    }
                }
            }
        }
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
        let mut transport = MockAtlasTransport::default();
        transport.push_outcome(MockTransportOutcome::response(200, r#"{"items":[{"id":null,"path":"Inbox.md","title":"Morning plan","state":"managed","revision":"r1","parentId":null,"order":null}],"nextCursor":null}"#));
        transport.push_outcome(MockTransportOutcome::response(200, r#"{"items":[{"id":"33333333-3333-3333-3333-333333333333","name":"Today","revision":"r1","status":"ok","layout":"list"}]}"#));
        state.refresh_atlas_home(&mut AtlasClient::new(transport));
        state.update_atlas_snapshot(AtlasSnapshot {
            connection: AtlasConnectionState::Offline,
        });

        let content = atlas_home_content(&state);

        assert_eq!(content.status(), ["CONNECTED", "50%", "CONNECTED"]);
        assert_eq!(content.time(), "09:05");
        assert_eq!(content.recent_notes(), ["Morning plan"]);
        assert_eq!(content.view_shortcuts(), ["Today"]);
        assert_eq!(content.capture_label(), "CAPTURE >");
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
        assert_eq!(atlas_home_footer_hint(), "MOVE SELECT HOLD BOOT BACK");

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
