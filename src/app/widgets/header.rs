//! Reusable black product header.

use core::convert::Infallible;

use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{Drawable, Point, Primitive, Size},
    primitives::{Line, PrimitiveStyle, Rectangle},
};

use crate::{
    app::{
        display::DisplayPreferences, state::AppState, typography::Text,
        widgets::atlas_brand::draw_atlas_mark,
    },
    network::WifiConnectionState,
    orientation::OrientedFrameBuffer,
};

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
    // Product status is deliberately a thin, quiet strip. Screen identity is
    // rendered in the surface itself, leaving the reader and Home room for a
    // larger, more legible hierarchy.
    Rectangle::new(Point::new(0, 0), Size::new(480, 48))
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
        .draw(display)?;
    Rectangle::new(Point::new(0, 47), Size::new(480, 1))
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
        .draw(display)?;
    draw_atlas_mark(display, Point::new(14, 12), BinaryColor::On)?;
    Text::new("ATLAS", Point::new(48, 29), state.display.detail_style()).draw(display)?;
    let date = state.board.compact_date_label(state.regional);
    Text::new(context, Point::new(108, 29), state.display.detail_style()).draw(display)?;
    if context.is_empty() {
        Text::new(&date, Point::new(108, 29), state.display.detail_style()).draw(display)?;
    }

    let time = state.board.compact_time_12h_label(state.regional);
    let battery = state
        .board
        .power
        .and_then(|power| power.battery_percent)
        .map_or_else(|| "--%".into(), |percent| format!("{percent}%"));
    Text::new(&time, Point::new(238, 30), state.display.detail_style()).draw(display)?;
    draw_wifi_icon(display, Point::new(326, 14), state.network.wifi_state)?;
    draw_battery_icon(
        display,
        Point::new(358, 13),
        state.board.power.and_then(|p| p.battery_percent),
    )?;
    Text::new(&battery, Point::new(394, 30), state.display.detail_style()).draw(display)?;
    Ok(())
}

fn draw_wifi_icon(
    display: &mut OrientedFrameBuffer<'_>,
    origin: Point,
    state: WifiConnectionState,
) -> Result<(), Infallible> {
    let ink = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
    if state == WifiConnectionState::Connected {
        Line::new(origin + Point::new(0, 1), origin + Point::new(18, 1))
            .into_styled(ink)
            .draw(display)?;
        Line::new(origin + Point::new(3, 6), origin + Point::new(15, 6))
            .into_styled(ink)
            .draw(display)?;
        Line::new(origin + Point::new(6, 11), origin + Point::new(12, 11))
            .into_styled(ink)
            .draw(display)?;
        Rectangle::new(origin + Point::new(8, 15), Size::new(3, 3))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;
    } else {
        Rectangle::new(origin + Point::new(2, 2), Size::new(16, 14))
            .into_styled(ink)
            .draw(display)?;
        Line::new(origin + Point::new(2, 2), origin + Point::new(18, 16))
            .into_styled(ink)
            .draw(display)?;
    }
    Ok(())
}

fn draw_battery_icon(
    display: &mut OrientedFrameBuffer<'_>,
    origin: Point,
    percent: Option<u8>,
) -> Result<(), Infallible> {
    let ink = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
    Rectangle::new(origin, Size::new(25, 14))
        .into_styled(ink)
        .draw(display)?;
    Rectangle::new(origin + Point::new(25, 4), Size::new(3, 6))
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
        .draw(display)?;
    if let Some(percent) = percent {
        let width = (u32::from(percent.min(100)) * 21 / 100).max(1);
        Rectangle::new(origin + Point::new(2, 2), Size::new(width, 10))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)?;
    }
    Ok(())
}
