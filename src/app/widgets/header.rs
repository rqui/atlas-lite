//! Reusable black product header.

use core::convert::Infallible;

use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{Drawable, Point, Primitive, Size},
    primitives::{Line, PrimitiveStyle, Rectangle},
};

use crate::{
    app::{
        display::DisplayPreferences,
        state::AppState,
        typography::{Text, UiTextRole},
        widgets::atlas_brand::draw_atlas_mark,
    },
    network::WifiConnectionState,
    orientation::OrientedFrameBuffer,
};

pub const ATLAS_TOPBAR_HEIGHT: i32 = 48;
pub const ATLAS_HOME_TOPBAR_HEIGHT: i32 = 56;
// Text glyphs may extend a few pixels beyond their horizontal advance. Keep
// the measured advance at 462 so every rasterized percentage pixel remains at
// or before the required logical x=470 product margin.
const TOPBAR_RIGHT: i32 = 462;
const WIFI_WIDTH: i32 = 20;
const BATTERY_WIDTH: i32 = 28;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AtlasTopbarLayout {
    pub time_left: i32,
    pub time_right: i32,
    pub wifi_left: i32,
    pub battery_left: i32,
    pub percent_left: i32,
    pub percent_right: i32,
}

#[must_use]
pub(crate) fn atlas_topbar_layout(
    preferences: DisplayPreferences,
    time: &str,
    battery: &str,
) -> AtlasTopbarLayout {
    let style = preferences.text_style(UiTextRole::Detail, BinaryColor::Off);
    let percent_right = TOPBAR_RIGHT;
    let percent_left = percent_right - style.text_width(battery);
    let battery_left = percent_left - 6 - BATTERY_WIDTH;
    let wifi_left = battery_left - 10 - WIFI_WIDTH;
    let time_right = wifi_left - 12;
    let time_left = time_right - style.text_width(time);
    AtlasTopbarLayout {
        time_left,
        time_right,
        wifi_left,
        battery_left,
        percent_left,
        percent_right,
    }
}

/// Draw the product header shared by every portrait screen.
pub fn draw_header(
    display: &mut OrientedFrameBuffer<'_>,
    preferences: DisplayPreferences,
    title: &str,
    subtitle: &str,
) -> Result<(), Infallible> {
    let black = PrimitiveStyle::with_fill(BinaryColor::On);
    let title_style = preferences.header_title_style();
    let subtitle_style = preferences.header_subtitle_style();

    Rectangle::new(Point::new(0, 0), Size::new(480, 70))
        .into_styled(black)
        .draw(display)?;
    Text::new(title, Point::new(18, 32), title_style).draw(display)?;
    Text::new(subtitle, Point::new(18, 60), subtitle_style).draw(display)?;
    Ok(())
}

/// Draw the branded Atlas header used by every Atlas Lite product surface.
///
/// This deliberately keeps the existing 70-pixel shell height so the Note
/// reader, compact status strip and their bounded content viewports do not
/// lose usable area.
pub fn draw_atlas_header(
    display: &mut OrientedFrameBuffer<'_>,
    preferences: DisplayPreferences,
    subtitle: &str,
) -> Result<(), Infallible> {
    let black = PrimitiveStyle::with_fill(BinaryColor::On);
    Rectangle::new(Point::new(0, 0), Size::new(480, 70))
        .into_styled(black)
        .draw(display)?;
    draw_atlas_mark(display, Point::new(18, 15), BinaryColor::Off)?;
    Text::new(
        "ATLAS",
        Point::new(68, 34),
        preferences.header_title_style(),
    )
    .draw(display)?;
    Text::new(
        subtitle,
        Point::new(68, 60),
        preferences.header_subtitle_style(),
    )
    .draw(display)?;
    Ok(())
}

/// Draw the compact, non-live Atlas product top bar. It reads the already
/// captured board/network snapshots and therefore never causes a refresh or a
/// network request while rendering.
pub fn draw_atlas_topbar(
    display: &mut OrientedFrameBuffer<'_>,
    state: &AppState,
    context: &str,
) -> Result<(), Infallible> {
    draw_atlas_topbar_with_height(display, state, context, ATLAS_TOPBAR_HEIGHT)
}

/// Home owns a slightly taller product masthead without changing the fixed
/// 48-pixel Reader viewport header.
pub fn draw_atlas_home_topbar(
    display: &mut OrientedFrameBuffer<'_>,
    state: &AppState,
) -> Result<(), Infallible> {
    draw_atlas_topbar_with_height(display, state, "", ATLAS_HOME_TOPBAR_HEIGHT)
}

fn draw_atlas_topbar_with_height(
    display: &mut OrientedFrameBuffer<'_>,
    state: &AppState,
    context: &str,
    height: i32,
) -> Result<(), Infallible> {
    Rectangle::new(Point::zero(), Size::new(480, height as u32))
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
        .draw(display)?;
    let mark_y = (height - 34) / 2;
    draw_atlas_mark(display, Point::new(8, mark_y), BinaryColor::Off)?;
    let brand = state.display.text_style(UiTextRole::Body, BinaryColor::Off);
    let status = state
        .display
        .text_style(UiTextRole::Detail, BinaryColor::Off);
    let brand_baseline = (height + i32::from(brand.line_height())) / 2 - 2;
    let status_baseline = (height + i32::from(status.line_height())) / 2 - 2;
    Text::new("ATLAS", Point::new(50, brand_baseline), brand).draw(display)?;
    if !context.is_empty() {
        Text::new(context, Point::new(124, status_baseline), status).draw(display)?;
    }

    let time = state.board.time_label(state.regional);
    let battery = state
        .board
        .power
        .and_then(|power| power.battery_percent)
        .map_or_else(|| "--%".into(), |percent| format!("{percent}%"));
    let layout = atlas_topbar_layout(state.display, &time, &battery);
    Text::new(&time, Point::new(layout.time_left, status_baseline), status).draw(display)?;
    let icon_y = (height - 18) / 2;
    draw_wifi_icon(
        display,
        Point::new(layout.wifi_left, icon_y),
        state.network.wifi_state,
        BinaryColor::Off,
    )?;
    draw_battery_icon(
        display,
        Point::new(layout.battery_left, (height - 14) / 2),
        state.board.power.and_then(|p| p.battery_percent),
        BinaryColor::Off,
    )?;
    Text::new(
        &battery,
        Point::new(layout.percent_left, status_baseline),
        status,
    )
    .draw(display)?;
    Ok(())
}

fn draw_wifi_icon(
    display: &mut OrientedFrameBuffer<'_>,
    origin: Point,
    state: WifiConnectionState,
    ink: BinaryColor,
) -> Result<(), Infallible> {
    let stroke = PrimitiveStyle::with_stroke(ink, 2);
    if state == WifiConnectionState::Connected {
        Line::new(origin + Point::new(0, 1), origin + Point::new(18, 1))
            .into_styled(stroke)
            .draw(display)?;
        Line::new(origin + Point::new(3, 6), origin + Point::new(15, 6))
            .into_styled(stroke)
            .draw(display)?;
        Line::new(origin + Point::new(6, 11), origin + Point::new(12, 11))
            .into_styled(stroke)
            .draw(display)?;
        Rectangle::new(origin + Point::new(8, 15), Size::new(3, 3))
            .into_styled(PrimitiveStyle::with_fill(ink))
            .draw(display)?;
    } else {
        Rectangle::new(origin + Point::new(2, 2), Size::new(16, 14))
            .into_styled(stroke)
            .draw(display)?;
        Line::new(origin + Point::new(2, 2), origin + Point::new(18, 16))
            .into_styled(stroke)
            .draw(display)?;
    }
    Ok(())
}

fn draw_battery_icon(
    display: &mut OrientedFrameBuffer<'_>,
    origin: Point,
    percent: Option<u8>,
    ink: BinaryColor,
) -> Result<(), Infallible> {
    let stroke = PrimitiveStyle::with_stroke(ink, 2);
    Rectangle::new(origin, Size::new(25, 14))
        .into_styled(stroke)
        .draw(display)?;
    Rectangle::new(origin + Point::new(25, 4), Size::new(3, 6))
        .into_styled(PrimitiveStyle::with_fill(ink))
        .draw(display)?;
    if let Some(percent) = percent {
        let width = (u32::from(percent.min(100)) * 21 / 100).max(1);
        Rectangle::new(origin + Point::new(2, 2), Size::new(width, 10))
            .into_styled(PrimitiveStyle::with_fill(ink))
            .draw(display)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use embedded_graphics::prelude::Point;

    use super::{atlas_topbar_layout, draw_atlas_home_topbar};
    use crate::{
        app::{display::DisplayPreferences, AppState},
        board_services::BoardSnapshot,
        framebuffer::FrameBuffer,
        network::{NetworkSnapshot, WifiConnectionState},
        orientation::{DisplayOrientation, OrientedFrameBuffer},
        power::PowerSnapshot,
        rtc::RtcDateTime,
    };

    #[test]
    fn status_group_is_built_from_the_right_edge_for_every_battery_width() {
        for battery in ["9%", "48%", "100%", "--%"] {
            let layout = atlas_topbar_layout(DisplayPreferences::default(), "20:57", battery);
            assert_eq!(layout.percent_right, 462);
            assert!(layout.percent_left < layout.percent_right);
            assert!(layout.battery_left + 28 < layout.percent_left);
            assert!(layout.wifi_left + 20 < layout.battery_left);
            assert!(layout.time_left < layout.time_right);
            assert!(
                layout.time_left > 190,
                "status must not collide with the brand"
            );
        }
    }

    #[test]
    fn home_topbar_is_black_with_white_mark_and_text() {
        let mut state = AppState::default();
        state.update_board_snapshot(BoardSnapshot {
            rtc: Some(RtcDateTime {
                year: 2026,
                month: 1,
                day: 1,
                weekday: 4,
                hour: 20,
                minute: 57,
                second: 0,
            }),
            power: Some(PowerSnapshot {
                battery_percent: Some(100),
                ..PowerSnapshot::default()
            }),
            ..BoardSnapshot::default()
        });
        state.update_network_snapshot(NetworkSnapshot {
            wifi_state: WifiConnectionState::Connected,
            ..NetworkSnapshot::default()
        });
        let orientation = DisplayOrientation::Portrait;
        let mut frame = FrameBuffer::new_white();
        let mut display = OrientedFrameBuffer::new(&mut frame, orientation);
        draw_atlas_home_topbar(&mut display, &state).unwrap();
        drop(display);

        let background = orientation
            .map_logical_to_native(Point::new(479, 55))
            .unwrap();
        let mark = orientation
            .map_logical_to_native(Point::new(25, 18))
            .unwrap();
        assert_eq!(frame.is_black(background), Some(true));
        assert_eq!(frame.is_black(mark), Some(false));
        for y in 0..56 {
            for x in 471..480 {
                let native = orientation.map_logical_to_native(Point::new(x, y)).unwrap();
                assert_ne!(
                    frame.is_black(native),
                    Some(false),
                    "white status pixel beyond x=470"
                );
            }
        }
    }
}
