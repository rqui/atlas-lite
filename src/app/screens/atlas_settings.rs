//! Product-focused Atlas Lite Settings and maintenance actions.

use core::convert::Infallible;

use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{Drawable, Point, Primitive, Size},
    primitives::{PrimitiveStyle, Rectangle},
};

use crate::{
    app::{
        state::AppState,
        typography::{Text, UiTextStyle},
        widgets::{
            footer::draw_footer,
            header::draw_header,
            status_row::{draw_status_row, StatusRow},
        },
    },
    build_info::FIRMWARE_VERSION,
    orientation::OrientedFrameBuffer,
};

const ACTIONS: [&str; 5] = [
    "Check for update",
    "Restart",
    "Reset Wi-Fi",
    "Unpair Atlas",
    "Factory reset",
];

pub fn render_atlas_settings(
    display: &mut OrientedFrameBuffer<'_>,
    state: &AppState,
) -> Result<(), Infallible> {
    let body = state.display.body_style();
    let detail = state.display.detail_style();
    let rssi = state.network.rssi_label();
    let battery = state
        .board
        .power
        .and_then(|p| p.battery_percent)
        .map_or_else(|| "--".into(), |p| format!("{p}%"));
    let device = state.product_device_id.as_deref().unwrap_or("--");

    draw_header(display, state.display, "SETTINGS", "ATLAS LITE DEVICE")?;
    draw_status_row(
        display,
        state.display,
        StatusRow {
            left: state.atlas.connection.label(),
            middle: state.network.wifi_state.label(),
            right: &battery,
        },
    )?;
    line(display, 150, "Device", device, detail)?;
    line(display, 184, "Firmware", FIRMWARE_VERSION, detail)?;
    line(display, 218, "Wi-Fi / RSSI", &rssi, detail)?;
    line(
        display,
        252,
        "Sync",
        state.network.ntp_state.label(),
        detail,
    )?;
    line(
        display,
        286,
        "Atlas",
        atlas_transport_label(state.product_private_lan_http),
        detail,
    )?;
    line(
        display,
        320,
        "Status",
        state
            .product_settings_feedback
            .as_deref()
            .unwrap_or(state.storage.status_label()),
        detail,
    )?;

    for (index, label) in ACTIONS.iter().enumerate() {
        action(
            display,
            364 + index as i32 * 64,
            label,
            index == state.product_settings_selected,
            body,
        )?;
    }
    draw_footer(
        display,
        state.display,
        "SELECT ACTION  HOLD POWER SLEEP  HOLD BOOT BACK",
    )
}

const fn atlas_transport_label(private_lan_http: bool) -> &'static str {
    if private_lan_http {
        "LAN HTTP / DEVELOPMENT"
    } else {
        "HTTPS"
    }
}

fn line(
    display: &mut OrientedFrameBuffer<'_>,
    y: i32,
    label: &str,
    value: &str,
    style: UiTextStyle,
) -> Result<(), Infallible> {
    Text::new(label, Point::new(22, y), style).draw(display)?;
    Text::new(value, Point::new(176, y), style).draw(display)?;
    Ok(())
}

fn action(
    display: &mut OrientedFrameBuffer<'_>,
    top: i32,
    label: &str,
    selected: bool,
    style: UiTextStyle,
) -> Result<(), Infallible> {
    Rectangle::new(Point::new(22, top), Size::new(436, 52))
        .into_styled(PrimitiveStyle::with_stroke(
            BinaryColor::On,
            if selected { 5 } else { 1 },
        ))
        .draw(display)?;
    Text::new(
        if selected { ">" } else { " " },
        Point::new(38, top + 34),
        style,
    )
    .draw(display)?;
    Text::new(label, Point::new(68, top + 34), style).draw(display)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{atlas_transport_label, render_atlas_settings};
    use crate::{
        app::{router::AtlasNavigationSurface, AppState},
        framebuffer::FrameBuffer,
        orientation::OrientedFrameBuffer,
    };

    #[test]
    fn settings_renders_without_secrets_or_hardware() {
        let mut state = AppState::default();
        state
            .router
            .navigate_atlas_to(AtlasNavigationSurface::Settings);
        state.product_device_id = Some("atlas-lite-01234567".into());
        let mut frame = FrameBuffer::new_white();
        let mut display = OrientedFrameBuffer::new(&mut frame, Default::default());
        render_atlas_settings(&mut display, &state).unwrap();
    }

    #[test]
    fn settings_marks_private_http_as_development_mode() {
        assert_eq!(atlas_transport_label(true), "LAN HTTP / DEVELOPMENT");
        assert_eq!(atlas_transport_label(false), "HTTPS");
    }
}
