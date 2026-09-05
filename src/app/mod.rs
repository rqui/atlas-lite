//! Modular portrait product UI shell.
//!
//! Hardware-independent screen state and drawing code live below this module.
//! `main.rs` wires peripherals, forwards debounced events, captures optional
//! board-service snapshots and asks this shell to render the active route.

use core::convert::Infallible;

use crate::{framebuffer::FrameBuffer, orientation::OrientedFrameBuffer};

pub mod display;
pub mod menu;
pub mod reader_atkinson_next_assets;
pub mod reader_literata_assets;
pub mod reader_serif_assets;
pub mod reader_typography;
pub mod router;
pub mod screens;
pub mod state;
pub mod typography;
pub mod widgets;

pub use router::ScreenRoute;
pub use state::AppState;

/// Product name shown on Atlas Lite screens. Internal identifiers deliberately
/// retain their stable `atlas-lite` names for configuration compatibility.
pub const PRODUCT_VISIBLE_NAME: &str = "ATLAS";

/// Idle interval before the panel controller and ALDO3 rail enter sleep.
pub const PANEL_IDLE_SLEEP_SECONDS: u64 = 60;
/// Detail-screen status cadence inherited from the sample-app clock use case.
pub const SAMPLE_LIVE_REFRESH_SECONDS: u64 = 30;
/// Motion diagnostics refresh at a slower e-paper-safe cadence.
pub const MOTION_LIVE_REFRESH_SECONDS: u64 = 10;
/// Motion-event diagnostics refresh slowly unless an event arrives sooner.
pub const IMU_EVENT_SCREEN_REFRESH_SECONDS: u64 =
    crate::imu_events::IMU_EVENT_SCREEN_REFRESH_SECONDS;
/// Network diagnostics refresh while visible.
pub const NETWORK_LIVE_REFRESH_SECONDS: u64 = 10;
/// Concise network serial heartbeat; UI refresh remains independent.
pub const NETWORK_LOG_HEARTBEAT_SECONDS: u64 = 30;
/// Voice-recording e-paper timer updates stay deliberately coarse.
pub const VOICE_RECORD_SCREEN_REFRESH_SECONDS: u64 =
    crate::voice_notes::VOICE_RECORD_SCREEN_REFRESH_SECONDS;
/// Poll the PCF85063 alarm flag and domain schedule once per second.
pub const ALARM_POLL_SECONDS: u64 = 1;

/// Clear the native frame and render the active product screen through the
/// orientation adapter.
pub fn render_current_screen(frame: &mut FrameBuffer, state: &AppState) -> Result<(), Infallible> {
    frame.clear_white();
    let mut display = OrientedFrameBuffer::new(frame, state.orientation);
    screens::render_active_screen(&mut display, state)
}

#[cfg(test)]
mod tests {
    use embedded_graphics::prelude::{Point, Size};

    use super::{render_current_screen, AppState, ScreenRoute};
    use crate::{
        app::{menu::atlas_home_entries, screens::atlas_home::atlas_home_menu_rect},
        buttons::ButtonEvent,
        framebuffer::FrameBuffer,
        orientation::DisplayOrientation,
    };

    #[test]
    fn atlas_home_renderer_places_brand_and_active_row_ink() {
        let mut frame = FrameBuffer::new_white();
        render_current_screen(&mut frame, &AppState::default()).unwrap();
        // The Atlas header remains a solid black product band.
        assert_eq!(frame.is_black(Point::new(10, 479)), Some(true));
        let selected = atlas_home_menu_rect(0).unwrap();
        let rail = DisplayOrientation::Portrait
            .map_logical_to_native(Point::new(
                selected.top_left.x + 14,
                selected.top_left.y + 39,
            ))
            .unwrap();
        assert_eq!(frame.is_black(rail), Some(true));
    }

    #[test]
    fn atlas_home_menu_geometry_and_ink_cover_every_selection() {
        let orientation = DisplayOrientation::Portrait;
        assert_eq!(orientation.logical_size(), Size::new(480, 800));

        for selection in 0..atlas_home_entries().len() {
            let selected_rect = atlas_home_menu_rect(selection).expect("menu row exists");
            assert!(selected_rect.top_left.x >= 0);
            assert!(selected_rect.top_left.y >= 0);
            assert!(selected_rect.top_left.x + selected_rect.size.width as i32 <= 480);
            assert!(selected_rect.top_left.y + selected_rect.size.height as i32 <= 800);

            let selected_logical =
                Point::new(selected_rect.top_left.x + 14, selected_rect.top_left.y + 39);
            let selected_native = orientation
                .map_logical_to_native(selected_logical)
                .expect("selected ink stays on the portrait surface");
            let mut frame = FrameBuffer::new_white();
            let mut state = AppState::default();
            state.home_selected = selection;
            render_current_screen(&mut frame, &state).unwrap();
            assert_eq!(
                frame.is_black(selected_native),
                Some(true),
                "selected row {selection} has no ink at its expected stroke"
            );

            for other_selection in 0..atlas_home_entries().len() {
                if other_selection == selection {
                    continue;
                }
                let other_rect = atlas_home_menu_rect(other_selection).expect("menu row exists");
                let other_native = orientation
                    .map_logical_to_native(Point::new(
                        other_rect.top_left.x + 14,
                        other_rect.top_left.y + 39,
                    ))
                    .expect("unselected ink stays on the portrait surface");
                assert_eq!(
                    frame.is_black(other_native),
                    Some(false),
                    "unselected row {other_selection} is unexpectedly thick at its stroke"
                );
            }
        }
    }

    #[test]
    fn legacy_settings_display_renderer_remains_reachable() {
        let mut frame = FrameBuffer::new_white();
        let mut state = AppState::default();
        state.router.navigate_to(ScreenRoute::Settings);
        for _ in 0..3 {
            state.apply(ButtonEvent::Down);
        }
        state.apply(ButtonEvent::Select);
        assert_eq!(state.active_route(), ScreenRoute::Display);
        render_current_screen(&mut frame, &state).unwrap();
        assert_eq!(frame.is_black(Point::new(0, 479)), Some(true));
    }

    #[test]
    fn tools_file_browser_route_is_reachable() {
        let mut state = AppState::default();
        state.router.navigate_to(ScreenRoute::Tools);
        state.apply(ButtonEvent::Select);
        assert_eq!(state.active_route(), ScreenRoute::Files);
    }

    #[test]
    fn tools_dictionary_route_renders_offline_without_sd_pack() {
        let mut frame = FrameBuffer::new_white();
        let mut state = AppState::default();
        state.router.navigate_to(ScreenRoute::Tools);
        state.apply(ButtonEvent::Down);
        state.apply(ButtonEvent::Select);
        assert_eq!(state.active_route(), ScreenRoute::Dictionary);
        render_current_screen(&mut frame, &state).unwrap();
    }

    #[test]
    fn tools_unit_converter_route_renders_offline() {
        let mut frame = FrameBuffer::new_white();
        let mut state = AppState::default();
        state.router.navigate_to(ScreenRoute::Tools);
        state.apply(ButtonEvent::Down);
        state.apply(ButtonEvent::Down);
        state.apply(ButtonEvent::Select);
        assert_eq!(state.active_route(), ScreenRoute::UnitConverter);
        render_current_screen(&mut frame, &state).unwrap();
    }

    #[test]
    fn power_key_short_menu_route_renders_and_returns_to_previous_screen() {
        let mut frame = FrameBuffer::new_white();
        let mut state = AppState::default();
        state.router.navigate_to(ScreenRoute::Dictionary);
        state.open_power_key_menu();
        assert_eq!(state.active_route(), ScreenRoute::PowerKeyMenu);
        render_current_screen(&mut frame, &state).unwrap();
        state.apply(ButtonEvent::Select);
        assert_eq!(state.active_route(), ScreenRoute::Dictionary);
        assert!(state.take_power_key_manual_refresh_request());
    }
}
