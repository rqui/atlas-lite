#[cfg(test)]
mod tests {
    use embedded_graphics::prelude::Point;

    use super::{
        AtlasConnectionState, BatteryState, SdState, SemanticInput, SimulatedHardware,
        SimulatedInput, Simulator, SimulatorKey, SimulatorLibraryFixture, SimulatorNoteFixture,
        WifiState, LOGICAL_HEIGHT, LOGICAL_WIDTH, NATIVE_FRAMEBUFFER_SIZE,
    };
    use crate::{
        app::{
            menu::atlas_home_entries, render_current_screen, router::AtlasRoute,
            screens::atlas_home::atlas_home_menu_rect, state::AppState, ScreenRoute,
        },
        atlas_client::TransportRequest,
        buttons::ButtonEvent,
        framebuffer::FrameBuffer,
        orientation::DisplayOrientation,
    };

    #[test]
    fn semantic_input_translation_is_independent_of_physical_keys() {
        assert_eq!(
            SimulatorKey::ArrowUp.semantic_input(),
            Some(SemanticInput::Up)
        );
        assert_eq!(
            SimulatorKey::ArrowDown.semantic_input(),
            Some(SemanticInput::Down)
        );
        assert_eq!(
            SimulatorKey::Enter.semantic_input(),
            Some(SemanticInput::Select)
        );
        assert_eq!(
            SimulatorKey::Escape.semantic_input(),
            Some(SemanticInput::Back)
        );
        assert_eq!(SimulatorKey::H.semantic_input(), Some(SemanticInput::Home));
        assert_eq!(
            SimulatorKey::Home.semantic_input(),
            Some(SemanticInput::Home)
        );
        assert_eq!(SimulatorKey::P.semantic_input(), Some(SemanticInput::Power));
        assert_eq!(SimulatorKey::Other.semantic_input(), None);
    }

    #[test]
    fn semantic_navigation_inputs_translate_to_product_button_events() {
        assert_eq!(SemanticInput::Up.button_event(), Some(ButtonEvent::Up));
        assert_eq!(SemanticInput::Down.button_event(), Some(ButtonEvent::Down));
        assert_eq!(
            SemanticInput::Select.button_event(),
            Some(ButtonEvent::Select)
        );
        assert_eq!(SemanticInput::Back.button_event(), None);
        assert_eq!(SemanticInput::Home.button_event(), None);
        assert_eq!(SemanticInput::Power.button_event(), None);
    }

    #[test]
    fn simulator_only_marks_render_dirty_after_input_or_snapshot_changes() {
        let mut simulator = Simulator::default();
        assert!(simulator.needs_redraw());
        simulator.render().unwrap();
        assert!(!simulator.needs_redraw());
        simulator.render().unwrap();
        assert!(!simulator.needs_redraw());

        simulator.set_hardware(SimulatedHardware::default());
        assert!(!simulator.needs_redraw());

        simulator.set_hardware(SimulatedHardware {
            wifi: WifiState::Offline,
            ..SimulatedHardware::default()
        });
        assert!(simulator.needs_redraw());
    }

    #[test]
    fn home_fixtures_drive_the_real_state_and_renderer_without_polling() {
        for fixture in [
            super::SimulatorHomeFixture::Empty,
            super::SimulatorHomeFixture::Normal,
            super::SimulatorHomeFixture::LongTitles,
            super::SimulatorHomeFixture::OfflineCache,
            super::SimulatorHomeFixture::Error,
        ] {
            let mut simulator = Simulator::default();
            simulator.apply_home_fixture(fixture);
            let first = simulator.render().unwrap().to_vec();
            let second = simulator.render().unwrap().to_vec();

            assert_eq!(
                first, second,
                "fixture {fixture:?} repolled while rendering"
            );
            assert_eq!(first.len(), NATIVE_FRAMEBUFFER_SIZE);
            match fixture {
                super::SimulatorHomeFixture::Empty => {
                    assert!(simulator.state().atlas_home.recent_notes().is_empty());
                    assert!(simulator.state().atlas_home.view_shortcuts().is_empty());
                    assert_eq!(
                        simulator.state().atlas.connection,
                        AtlasConnectionState::Connected
                    );
                }
                super::SimulatorHomeFixture::Normal => {
                    assert_eq!(
                        simulator.state().atlas_home.recent_notes(),
                        ["Morning plan"]
                    );
                    assert_eq!(simulator.state().atlas_home.view_shortcuts(), ["Today"]);
                }
                super::SimulatorHomeFixture::LongTitles => {
                    assert!(
                        simulator
                            .state()
                            .display
                            .body_style()
                            .text_width(simulator.state().atlas_home.recent_notes()[0].as_str())
                            > 436,
                        "wide-glyph fixture must exercise width fitting"
                    );
                    assert!(
                        simulator
                            .state()
                            .display
                            .body_style()
                            .text_width(simulator.state().atlas_home.view_shortcuts()[0].as_str())
                            > 436,
                        "wide-glyph View fixture must exercise width fitting"
                    );
                    let mut ink = false;
                    for (top, bottom) in [(190, 240), (320, 370)] {
                        for y in top..bottom {
                            for x in 22..458 {
                                let native = simulator
                                    .state()
                                    .orientation
                                    .map_logical_to_native(embedded_graphics::prelude::Point::new(
                                        x, y,
                                    ))
                                    .unwrap();
                                ink |= simulator.frame.is_black(native) == Some(true);
                            }
                            for x in 458..480 {
                                let native = simulator
                                    .state()
                                    .orientation
                                    .map_logical_to_native(embedded_graphics::prelude::Point::new(
                                        x, y,
                                    ))
                                    .unwrap();
                                assert_eq!(
                                    simulator.frame.is_black(native),
                                    Some(false),
                                    "wide-glyph Home label escaped its clipping rectangle"
                                );
                            }
                        }
                    }
                    assert!(ink, "wide-glyph fixture rendered no label ink");
                }
                super::SimulatorHomeFixture::OfflineCache => {
                    assert_eq!(
                        simulator.state().atlas.connection,
                        AtlasConnectionState::Offline
                    );
                    assert_eq!(
                        simulator.state().atlas_home.recent_notes(),
                        ["Morning plan"]
                    );
                }
                super::SimulatorHomeFixture::Error => {
                    assert_eq!(
                        simulator.state().atlas.connection,
                        AtlasConnectionState::Timeout
                    );
                    assert!(simulator.state().atlas_home.recent_notes().is_empty());
                }
            }
        }
    }

    #[test]
    fn library_fixtures_drive_the_real_refresh_seam_and_renderer_without_polling() {
        for fixture in [
            SimulatorLibraryFixture::Normal,
            SimulatorLibraryFixture::Partial,
        ] {
            let mut simulator = Simulator::default();
            simulator.apply_library_fixture(fixture);
            simulator.handle_key(SimulatorKey::Enter).unwrap();
            assert_eq!(simulator.state().atlas_route(), AtlasRoute::Library);
            let first = simulator.render().unwrap().to_vec();
            let second = simulator.render().unwrap().to_vec();
            assert_eq!(
                first, second,
                "fixture {fixture:?} repolled while rendering"
            );
            assert_eq!(
                simulator.state().atlas_library.hierarchy().root_ids(),
                ["11111111-1111-4111-8111-111111111111"]
            );
        }
    }

    #[test]
    fn simulator_opens_a_library_note_by_fixture_id_with_one_typed_get_note() {
        let mut simulator = Simulator::default();
        simulator.apply_library_fixture(SimulatorLibraryFixture::Normal);
        simulator.queue_note_fixture(SimulatorNoteFixture::Loaded);

        simulator.handle_key(SimulatorKey::Enter).unwrap();
        assert_eq!(simulator.state().atlas_route(), AtlasRoute::Library);
        simulator.handle_key(SimulatorKey::Enter).unwrap();

        assert_eq!(simulator.state().atlas_route(), AtlasRoute::Note);
        assert_eq!(
            simulator.state().atlas_note.selected_id(),
            Some("11111111-1111-4111-8111-111111111111")
        );
        let get_note_requests: Vec<_> = simulator
            .atlas_requests()
            .iter()
            .filter(|request| matches!(request, TransportRequest::GetNote { .. }))
            .cloned()
            .collect();
        assert_eq!(
            get_note_requests,
            [TransportRequest::GetNote {
                id: "11111111-1111-4111-8111-111111111111".into(),
            }]
        );

        let first = simulator.render().unwrap().to_vec();
        let second = simulator.render().unwrap().to_vec();
        assert_eq!(first, second);
        assert_eq!(first.len(), NATIVE_FRAMEBUFFER_SIZE);
        assert_eq!(
            simulator
                .atlas_requests()
                .iter()
                .filter(|request| matches!(request, TransportRequest::GetNote { .. }))
                .count(),
            1,
            "render must not poll"
        );
    }

    #[test]
    fn note_cache_and_error_state_survive_library_route_transitions_without_render_work() {
        let mut simulator = Simulator::default();
        simulator.apply_library_fixture(SimulatorLibraryFixture::Normal);
        simulator.queue_note_fixture(SimulatorNoteFixture::Loaded);
        simulator.handle_key(SimulatorKey::Enter).unwrap();
        simulator.handle_key(SimulatorKey::Enter).unwrap();
        let cached_body = simulator
            .state()
            .atlas_note
            .document()
            .expect("fixture document loaded")
            .body()
            .to_owned();

        simulator.handle_key(SimulatorKey::Escape).unwrap();
        assert_eq!(simulator.state().atlas_route(), AtlasRoute::Library);
        assert_eq!(
            simulator.state().atlas_note.document().unwrap().body(),
            cached_body
        );
        let requests_before_render = simulator.atlas_requests().len();
        simulator.render().unwrap();
        assert_eq!(simulator.atlas_requests().len(), requests_before_render);

        simulator.queue_note_fixture(SimulatorNoteFixture::OfflineCached);
        simulator.handle_key(SimulatorKey::Enter).unwrap();
        assert_eq!(simulator.state().atlas_route(), AtlasRoute::Note);
        assert_eq!(
            simulator.state().atlas_note.status(),
            crate::atlas_note::AtlasNoteStatus::OfflineCached
        );
        assert_eq!(
            simulator.state().atlas_note.document().unwrap().body(),
            cached_body
        );

        simulator.handle_key(SimulatorKey::Escape).unwrap();
        simulator.queue_note_fixture(SimulatorNoteFixture::Error);
        simulator.handle_key(SimulatorKey::Enter).unwrap();
        assert!(matches!(
            simulator.state().atlas_note.status(),
            crate::atlas_note::AtlasNoteStatus::Error(_)
        ));
        assert_eq!(
            simulator.state().atlas_note.document().unwrap().body(),
            cached_body
        );
    }

    #[test]
    fn note_fixtures_use_the_real_reader_state_and_renderer_without_polling() {
        for fixture in [
            SimulatorNoteFixture::Loaded,
            SimulatorNoteFixture::OfflineCached,
            SimulatorNoteFixture::Error,
        ] {
            let mut simulator = Simulator::default();
            simulator.apply_note_fixture(fixture);
            let first = simulator.render().unwrap().to_vec();
            let second = simulator.render().unwrap().to_vec();
            assert_eq!(
                first, second,
                "fixture {fixture:?} repolled while rendering"
            );
            assert_eq!(simulator.state().atlas_route(), AtlasRoute::Note);
            match fixture {
                SimulatorNoteFixture::Loaded => assert_eq!(
                    simulator.state().atlas_note.status(),
                    crate::atlas_note::AtlasNoteStatus::Loaded
                ),
                SimulatorNoteFixture::OfflineCached => assert_eq!(
                    simulator.state().atlas_note.status(),
                    crate::atlas_note::AtlasNoteStatus::OfflineCached
                ),
                SimulatorNoteFixture::Error => assert!(matches!(
                    simulator.state().atlas_note.status(),
                    crate::atlas_note::AtlasNoteStatus::Error(_)
                )),
            }
        }
    }

    #[test]
    fn hardware_snapshot_contract_is_reusable_without_secret_fields() {
        fn consume_snapshot(snapshot: &impl super::HardwareSnapshot) -> String {
            snapshot.redacted_summary()
        }

        let summary = consume_snapshot(&SimulatedHardware::default());
        assert!(summary.contains("atlas=unconfigured"));
        assert!(!summary.contains("token"));
        assert!(!summary.contains("password"));
    }

    #[test]
    fn simulator_reuses_portrait_product_renderer_and_native_framebuffer() {
        let mut simulator = Simulator::default();
        let first = simulator.render().unwrap().to_vec();
        let second = simulator.render().unwrap().to_vec();
        assert_eq!(first, second);
        assert_eq!(first.len(), NATIVE_FRAMEBUFFER_SIZE);
        assert_eq!(simulator.logical_size(), (LOGICAL_WIDTH, LOGICAL_HEIGHT));
        assert_eq!(super::SimulatedDisplay::logical_size(), (480, 800));
        assert_eq!(super::SimulatedDisplay::native_size(), (800, 480));
    }

    #[test]
    fn semantic_navigation_reaches_shell_routes_and_back() {
        let mut simulator = Simulator::default();
        for _ in 0..5 {
            simulator.handle_key(SimulatorKey::ArrowDown).unwrap();
            simulator.handle_key(SimulatorKey::Enter).unwrap();
            assert_eq!(simulator.state().active_route(), ScreenRoute::Home);
            assert_ne!(simulator.state().atlas_route(), AtlasRoute::Home);
            simulator.handle_key(SimulatorKey::Escape).unwrap();
            assert_eq!(simulator.state().atlas_route(), AtlasRoute::Home);
        }
    }

    #[test]
    fn selected_rows_change_the_real_rendered_frame() {
        let mut simulator = Simulator::default();
        let home = simulator.render().unwrap().to_vec();
        simulator.handle_key(SimulatorKey::ArrowDown).unwrap();
        let next = simulator.render().unwrap().to_vec();
        assert_ne!(home, next);
    }

    #[test]
    fn every_m1_5_atlas_surface_renders_deterministically_within_the_canvas() {
        let expected = [
            AtlasRoute::Home,
            AtlasRoute::Library,
            AtlasRoute::Search,
            AtlasRoute::Views,
            AtlasRoute::Capture,
            AtlasRoute::Settings,
        ];

        for (selection, route) in expected.into_iter().enumerate().skip(1) {
            let mut simulator = Simulator::default();
            for _ in 0..selection - 1 {
                simulator.handle_key(SimulatorKey::ArrowDown).unwrap();
            }
            simulator.handle_key(SimulatorKey::Enter).unwrap();
            assert_eq!(simulator.state().atlas_route(), route);

            let first = simulator.render().unwrap().to_vec();
            let second = simulator.render().unwrap().to_vec();
            assert_eq!(first, second, "repeated render changed {route:?}");
            assert_eq!(first.len(), NATIVE_FRAMEBUFFER_SIZE);
        }
    }

    #[test]
    fn back_from_each_m1_5_atlas_surface_returns_to_home() {
        for selection in 1..=5 {
            let mut simulator = Simulator::default();
            for _ in 0..selection {
                simulator.handle_key(SimulatorKey::ArrowDown).unwrap();
            }
            simulator.handle_key(SimulatorKey::Enter).unwrap();
            simulator.handle_key(SimulatorKey::Escape).unwrap();
            assert_eq!(simulator.state().atlas_route(), AtlasRoute::Home);
        }
    }

    #[test]
    fn diagnostics_are_stable_and_secret_free_for_all_fake_states() {
        let hardware = SimulatedHardware {
            wifi: WifiState::Failed,
            battery: BatteryState::Percent10,
            sd: SdState::Error,
            atlas: AtlasConnectionState::Unauthorized,
            ..SimulatedHardware::default()
        };
        let first = hardware.diagnostic_labels();
        let second = hardware.diagnostic_labels();
        assert_eq!(first, second);
        let rendered = first.join(" ");
        assert!(!rendered.contains("secret"));
        assert!(!rendered.contains("token"));
        assert!(!rendered.contains("password"));
    }

    #[test]
    fn atlas_home_geometry_and_anchors_stay_inside_portrait_logical_bounds() {
        let orientation = DisplayOrientation::Portrait;
        let logical = orientation.logical_size();

        for selection in 0..atlas_home_entries().len() {
            let mut frame = FrameBuffer::new_white();
            let row = atlas_home_menu_rect(selection).expect("planned Atlas Home row");
            let right = row.top_left.x + row.size.width as i32 - 1;
            let bottom = row.top_left.y + row.size.height as i32 - 1;
            assert!(row.top_left.x >= 0);
            assert!(row.top_left.y >= 0);
            assert!(row.top_left.x + row.size.width as i32 <= logical.width as i32);
            assert!(row.top_left.y + row.size.height as i32 <= logical.height as i32);
            for anchor in [
                row.top_left,
                Point::new(right, row.top_left.y),
                Point::new(row.top_left.x, bottom),
                Point::new(right, bottom),
                Point::new(row.top_left.x + 1, row.top_left.y),
                Point::new(row.top_left.x, row.top_left.y + 1),
            ] {
                assert!(
                    anchor.x >= 0
                        && anchor.y >= 0
                        && anchor.x < logical.width as i32
                        && anchor.y < logical.height as i32,
                    "logical anchor {anchor:?} escaped {logical:?}"
                );
                let native = orientation
                    .map_logical_to_native(anchor)
                    .expect("in-bounds logical anchor maps to native frame");
                assert_eq!(frame.is_black(native), Some(false));
            }

            let mut state = AppState::default();
            state.home_selected = selection;
            render_current_screen(&mut frame, &state).unwrap();
            for anchor in [
                row.top_left,
                Point::new(right, row.top_left.y),
                Point::new(row.top_left.x, bottom),
                Point::new(right, bottom),
                Point::new(row.top_left.x + 1, row.top_left.y),
                Point::new(row.top_left.x, row.top_left.y + 1),
            ] {
                let native = orientation
                    .map_logical_to_native(anchor)
                    .expect("rendered logical anchor maps to native frame");
                assert_eq!(frame.is_black(native), Some(true));
            }
        }
    }

    #[test]
    fn selected_atlas_home_row_has_black_ink_where_unselected_stroke_is_white() {
        let orientation = DisplayOrientation::Portrait;

        for selected in 0..atlas_home_entries().len() {
            let row = atlas_home_menu_rect(selected).expect("planned Atlas Home row");
            let probe = orientation
                .map_logical_to_native(Point::new(row.top_left.x + 1, row.top_left.y + 1))
                .expect("selected-row probe is in bounds");

            let mut selected_frame = FrameBuffer::new_white();
            let mut selected_state = AppState::default();
            selected_state.home_selected = selected;
            render_current_screen(&mut selected_frame, &selected_state).unwrap();
            assert_eq!(selected_frame.is_black(probe), Some(true));

            let mut unselected_frame = FrameBuffer::new_white();
            let mut unselected_state = AppState::default();
            unselected_state.home_selected = (selected + 1) % atlas_home_entries().len();
            render_current_screen(&mut unselected_frame, &unselected_state).unwrap();
            assert_eq!(unselected_frame.is_black(probe), Some(false));
        }
    }

    #[test]
    fn every_fake_state_combination_keeps_realistic_secrets_out_of_diagnostics() {
        let candidates = [
            "atlas_pat_live_01HXYZ987654321",
            "Bearer eyJhbGciOiJIUzI1NiJ9.test.signature",
            "wifi-password=correct-horse-battery-staple",
            "-----BEGIN CERTIFICATE-----\\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8A\\n-----END CERTIFICATE-----",
            "api_token: sk_live_1234567890abcdef",
        ];

        let inputs = [
            None,
            Some(SemanticInput::Up),
            Some(SemanticInput::Down),
            Some(SemanticInput::Select),
            Some(SemanticInput::Back),
            Some(SemanticInput::Home),
            Some(SemanticInput::Power),
        ];
        for wifi in [
            WifiState::Connected,
            WifiState::Connecting,
            WifiState::Offline,
            WifiState::Failed,
        ] {
            for battery in [
                BatteryState::Percent100,
                BatteryState::Percent50,
                BatteryState::Percent10,
            ] {
                for sd in [SdState::Mounted, SdState::Missing, SdState::Error] {
                    for rtc in [
                        super::RtcState::Ready,
                        super::RtcState::Unavailable,
                        super::RtcState::IntegrityLost,
                    ] {
                        for atlas in [
                            AtlasConnectionState::Unconfigured,
                            AtlasConnectionState::Connecting,
                            AtlasConnectionState::Connected,
                            AtlasConnectionState::Unauthorized,
                            AtlasConnectionState::Forbidden,
                            AtlasConnectionState::Timeout,
                            AtlasConnectionState::ServerError,
                            AtlasConnectionState::Offline,
                        ] {
                            for last in inputs {
                                let hardware = SimulatedHardware {
                                    input: SimulatedInput { last },
                                    wifi,
                                    battery,
                                    sd,
                                    rtc,
                                    atlas,
                                    ..SimulatedHardware::default()
                                };
                                let diagnostics = hardware.redacted_summary();
                                for candidate in candidates {
                                    assert!(
                                        !diagnostics.contains(candidate),
                                        "diagnostics leaked candidate {candidate:?}: {diagnostics}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn hardware_model_has_bounded_deterministic_states_without_secrets() {
        let hardware = SimulatedHardware::default();
        assert_eq!(hardware.wifi.label(), "connected");
        assert_eq!(hardware.battery.label(), "100%");
        assert_eq!(hardware.sd.label(), "mounted");
        assert_eq!(hardware.atlas.label(), "unconfigured");
        assert_eq!(
            hardware.diagnostic_labels(),
            [
                "display=ready",
                "input=ready",
                "sd=mounted",
                "wifi=connected",
                "battery=100%",
                "rtc=ready",
                "atlas=unconfigured",
            ]
        );
        assert!(!hardware.redacted_summary().contains("password"));
        let _ = [
            WifiState::Connected,
            WifiState::Connecting,
            WifiState::Offline,
            WifiState::Failed,
        ];
        let _ = [
            BatteryState::Percent100,
            BatteryState::Percent50,
            BatteryState::Percent10,
        ];
        let _ = [SdState::Mounted, SdState::Missing, SdState::Error];
        let _ = [
            AtlasConnectionState::Unconfigured,
            AtlasConnectionState::Connecting,
            AtlasConnectionState::Connected,
            AtlasConnectionState::Unauthorized,
            AtlasConnectionState::Forbidden,
            AtlasConnectionState::Timeout,
            AtlasConnectionState::ServerError,
            AtlasConnectionState::Offline,
        ];
    }

    #[test]
    fn hardware_diagnostics_follow_selected_fake_states() {
        let hardware = SimulatedHardware {
            sd: SdState::Error,
            wifi: WifiState::Offline,
            battery: BatteryState::Percent10,
            rtc: super::RtcState::IntegrityLost,
            atlas: AtlasConnectionState::ServerError,
            ..SimulatedHardware::default()
        };
        assert_eq!(hardware.diagnostic_labels()[2], "sd=error");
        assert_eq!(hardware.diagnostic_labels()[3], "wifi=offline");
        assert_eq!(hardware.diagnostic_labels()[4], "battery=10%");
        assert_eq!(hardware.diagnostic_labels()[5], "rtc=integrity_lost");
        assert_eq!(hardware.diagnostic_labels()[6], "atlas=server_error");
    }

    #[test]
    fn selected_hardware_snapshot_is_consumed_by_real_app_state() {
        let hardware = SimulatedHardware {
            sd: SdState::Error,
            wifi: WifiState::Failed,
            battery: BatteryState::Percent10,
            rtc: super::RtcState::Unavailable,
            atlas: AtlasConnectionState::Forbidden,
            ..SimulatedHardware::default()
        };
        let mut state = crate::app::AppState::default();
        hardware.apply_to_app_state(&mut state);
        assert_eq!(state.storage.mounted, true);
        assert!(state.storage.error.is_some());
        assert_eq!(
            state.network.wifi_state,
            crate::network::WifiConnectionState::Failed
        );
        assert_eq!(state.board.power.unwrap().battery_percent, Some(10));
        assert_eq!(state.board.rtc, None);
        assert_eq!(state.atlas.connection, AtlasConnectionState::Forbidden);
    }

    #[test]
    fn simulator_can_transition_atlas_diagnostics_without_device_handles() {
        let mut simulator = Simulator::default();
        simulator.render().unwrap();

        for state in [
            AtlasConnectionState::Connecting,
            AtlasConnectionState::Connected,
            AtlasConnectionState::Unauthorized,
            AtlasConnectionState::Forbidden,
            AtlasConnectionState::Timeout,
            AtlasConnectionState::ServerError,
            AtlasConnectionState::Offline,
            AtlasConnectionState::Unconfigured,
        ] {
            simulator.set_atlas_connection_state(state);
            assert_eq!(simulator.state().atlas.connection, state);
            assert_eq!(simulator.hardware().atlas, state);
            assert!(simulator.needs_redraw());
            simulator.render().unwrap();
        }
    }
}
/// Host-only simulator core for Atlas Lite application and hardware seams.
use core::convert::Infallible;

use crate::{
    app::{render_current_screen, AppState},
    atlas_client::{AtlasClient, MockAtlasTransport, MockTransportOutcome},
    atlas_state::AtlasSnapshot,
    buttons::ButtonEvent,
    framebuffer::{FrameBuffer, FRAMEBUFFER_SIZE},
};

pub use crate::atlas_state::AtlasConnectionState;

pub const LOGICAL_WIDTH: u32 = 480;
pub const LOGICAL_HEIGHT: u32 = 800;
pub const NATIVE_FRAMEBUFFER_SIZE: usize = FRAMEBUFFER_SIZE;

/// Deterministic, secret-free Atlas Home fixtures used by the host simulator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulatorHomeFixture {
    Empty,
    Normal,
    LongTitles,
    OfflineCache,
    Error,
}

/// Deterministic, secret-free Library fixtures using the real client seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulatorLibraryFixture {
    Normal,
    Partial,
}

/// Deterministic Search fixtures through the same typed client used on target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulatorSearchFixture {
    Success,
    NoResults,
    Unicode,
    Unavailable,
    Timeout,
    Offline,
    Malformed,
    Oversized,
}

/// Deterministic Views fixtures through the typed list and cursor seams.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulatorViewsFixture {
    Success,
    Empty,
    Pagination,
    Unavailable,
    Timeout,
    Offline,
    Malformed,
    Oversized,
}

/// Deterministic Note-reader fixtures through the real bounded client seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulatorNoteFixture {
    Loaded,
    OfflineCached,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulatorKey {
    ArrowUp,
    ArrowDown,
    Enter,
    Escape,
    B,
    H,
    Home,
    P,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticInput {
    Up,
    Down,
    Select,
    BootShort,
    Back,
    Home,
    Power,
}

impl SemanticInput {
    #[must_use]
    pub const fn button_event(self) -> Option<ButtonEvent> {
        match self {
            Self::Up => Some(ButtonEvent::Up),
            Self::Down => Some(ButtonEvent::Down),
            Self::Select => Some(ButtonEvent::Select),
            Self::Back | Self::BootShort | Self::Home | Self::Power => None,
        }
    }
}

impl SimulatorKey {
    #[must_use]
    pub const fn semantic_input(self) -> Option<SemanticInput> {
        match self {
            Self::ArrowUp => Some(SemanticInput::Up),
            Self::ArrowDown => Some(SemanticInput::Down),
            Self::Enter => Some(SemanticInput::Select),
            Self::Escape => Some(SemanticInput::Back),
            Self::B => Some(SemanticInput::BootShort),
            Self::H | Self::Home => Some(SemanticInput::Home),
            Self::P => Some(SemanticInput::Power),
            Self::Other => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WifiState {
    Connected,
    Connecting,
    Offline,
    Failed,
}

impl WifiState {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Connecting => "connecting",
            Self::Offline => "offline",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BatteryState {
    Percent100,
    Percent50,
    Percent10,
}

impl BatteryState {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Percent100 => "100%",
            Self::Percent50 => "50%",
            Self::Percent10 => "10%",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SdState {
    Mounted,
    Missing,
    Error,
}

impl SdState {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Mounted => "mounted",
            Self::Missing => "missing",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RtcState {
    Ready,
    Unavailable,
    IntegrityLost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulatedDisplay;

impl Default for SimulatedDisplay {
    fn default() -> Self {
        Self
    }
}

impl SimulatedDisplay {
    #[must_use]
    pub const fn logical_size() -> (u32, u32) {
        (LOGICAL_WIDTH, LOGICAL_HEIGHT)
    }

    #[must_use]
    pub const fn native_size() -> (u32, u32) {
        (800, 480)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SimulatedInput {
    pub last: Option<SemanticInput>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulatedHardware {
    pub display: SimulatedDisplay,
    pub input: SimulatedInput,
    pub sd: SdState,
    pub wifi: WifiState,
    pub battery: BatteryState,
    pub rtc: RtcState,
    pub atlas: AtlasConnectionState,
}

/// Host-side hardware snapshot seam shared by the simulator and future mocks.
pub trait HardwareSnapshot {
    fn apply_to_app_state(&self, state: &mut AppState);

    fn diagnostic_labels(&self) -> [String; 7];

    #[must_use]
    fn redacted_summary(&self) -> String {
        self.diagnostic_labels().join(" ")
    }
}

impl Default for SimulatedHardware {
    fn default() -> Self {
        Self {
            display: SimulatedDisplay::default(),
            input: SimulatedInput::default(),
            sd: SdState::Mounted,
            wifi: WifiState::Connected,
            battery: BatteryState::Percent100,
            rtc: RtcState::Ready,
            atlas: AtlasConnectionState::Unconfigured,
        }
    }
}

impl SimulatedHardware {
    pub fn apply_to_app_state(&self, state: &mut AppState) {
        <Self as HardwareSnapshot>::apply_to_app_state(self, state);
    }

    #[must_use]
    pub fn diagnostic_labels(&self) -> [String; 7] {
        <Self as HardwareSnapshot>::diagnostic_labels(self)
    }

    #[must_use]
    pub fn redacted_summary(&self) -> String {
        <Self as HardwareSnapshot>::redacted_summary(self)
    }

    fn apply_snapshot_to_app_state(&self, state: &mut AppState) {
        use crate::{
            board_services::BoardSnapshot,
            network::{NetworkSnapshot, NtpSyncState, WifiConnectionState},
            power::PowerSnapshot,
            rtc::RtcDateTime,
            storage::StorageSnapshot,
        };

        let wifi_state = match self.wifi {
            WifiState::Connected => WifiConnectionState::Connected,
            WifiState::Connecting => WifiConnectionState::Connecting,
            WifiState::Offline => WifiConnectionState::Disabled,
            WifiState::Failed => WifiConnectionState::Failed,
        };
        state.update_network_snapshot(NetworkSnapshot {
            wifi_state,
            ntp_state: if wifi_state == WifiConnectionState::Connected {
                NtpSyncState::Synchronized
            } else {
                NtpSyncState::WaitingForWifi
            },
            ..NetworkSnapshot::default()
        });

        let mut storage = StorageSnapshot::default();
        storage.mounted = !matches!(self.sd, SdState::Missing);
        if self.sd == SdState::Error {
            storage.error = Some("simulated SD error".into());
        }
        state.update_storage_snapshot(storage);

        let battery_percent = match self.battery {
            BatteryState::Percent100 => 100,
            BatteryState::Percent50 => 50,
            BatteryState::Percent10 => 10,
        };
        let rtc = match self.rtc {
            RtcState::Ready | RtcState::IntegrityLost => Some(RtcDateTime {
                year: 2026,
                month: 1,
                day: 1,
                weekday: 4,
                hour: 0,
                minute: 0,
                second: 0,
            }),
            RtcState::Unavailable => None,
        };
        state.update_board_snapshot(BoardSnapshot {
            rtc,
            power: Some(PowerSnapshot {
                battery_percent: Some(battery_percent),
                ..PowerSnapshot::default()
            }),
            rtc_clock_integrity_was_lost: self.rtc == RtcState::IntegrityLost,
            ..BoardSnapshot::default()
        });
        state.update_atlas_snapshot(AtlasSnapshot {
            connection: self.atlas,
        });
    }

    fn snapshot_diagnostic_labels(&self) -> [String; 7] {
        [
            "display=ready".into(),
            "input=ready".into(),
            format!("sd={}", self.sd.label()),
            format!("wifi={}", self.wifi.label()),
            format!("battery={}", self.battery.label()),
            format!(
                "rtc={}",
                match self.rtc {
                    RtcState::Ready => "ready",
                    RtcState::Unavailable => "unavailable",
                    RtcState::IntegrityLost => "integrity_lost",
                }
            ),
            format!("atlas={}", self.atlas.label()),
        ]
    }
}

impl HardwareSnapshot for SimulatedHardware {
    fn apply_to_app_state(&self, state: &mut AppState) {
        self.apply_snapshot_to_app_state(state);
    }

    fn diagnostic_labels(&self) -> [String; 7] {
        self.snapshot_diagnostic_labels()
    }
}

#[derive(Debug)]
pub struct Simulator {
    state: AppState,
    hardware: SimulatedHardware,
    frame: FrameBuffer,
    needs_redraw: bool,
    atlas_client: AtlasClient<MockAtlasTransport>,
}

impl Default for Simulator {
    fn default() -> Self {
        let hardware = SimulatedHardware::default();
        let mut simulator = Self {
            state: AppState::default(),
            hardware,
            frame: FrameBuffer::new_white(),
            needs_redraw: true,
            atlas_client: AtlasClient::new(MockAtlasTransport::default()),
        };
        simulator.hardware.apply_to_app_state(&mut simulator.state);
        simulator
    }
}

impl Simulator {
    #[must_use]
    pub const fn state(&self) -> &AppState {
        &self.state
    }

    #[must_use]
    pub const fn hardware(&self) -> &SimulatedHardware {
        &self.hardware
    }

    pub fn set_hardware(&mut self, hardware: SimulatedHardware) {
        if self.hardware == hardware {
            return;
        }
        self.hardware = hardware;
        self.hardware.apply_to_app_state(&mut self.state);
        self.needs_redraw = true;
    }

    /// Switch only the host mock's Atlas diagnostic state; no device API or
    /// ESP-IDF handle is involved in simulator control.
    pub fn set_atlas_connection_state(&mut self, atlas: AtlasConnectionState) {
        let mut hardware = self.hardware;
        hardware.atlas = atlas;
        self.set_hardware(hardware);
    }

    /// Apply a scripted AtlasClient response sequence through the real AppState
    /// Home-refresh seam. Rendering remains separate, so it cannot poll.
    pub fn apply_home_fixture(&mut self, fixture: SimulatorHomeFixture) {
        let mut transport = MockAtlasTransport::default();
        match fixture {
            SimulatorHomeFixture::Empty => {
                push_home_responses(&mut transport, EMPTY_NOTES, EMPTY_VIEWS)
            }
            SimulatorHomeFixture::Normal => {
                push_home_responses(&mut transport, NORMAL_NOTES, NORMAL_VIEWS)
            }
            SimulatorHomeFixture::LongTitles => {
                push_home_responses(&mut transport, LONG_TITLE_NOTES, LONG_TITLE_VIEWS)
            }
            SimulatorHomeFixture::OfflineCache => {
                push_home_responses(&mut transport, NORMAL_NOTES, NORMAL_VIEWS);
                transport.push_outcome(MockTransportOutcome::offline());
                transport.push_outcome(MockTransportOutcome::offline());
            }
            SimulatorHomeFixture::Error => {
                transport.push_outcome(MockTransportOutcome::timeout());
                transport.push_outcome(MockTransportOutcome::unavailable());
            }
        }
        let mut client = AtlasClient::new(transport);
        self.state.refresh_atlas_home(&mut client);
        if fixture == SimulatorHomeFixture::OfflineCache {
            self.state.refresh_atlas_home(&mut client);
        }
        self.needs_redraw = true;
    }

    /// Applies one finite scripted Library refresh; rendering never polls it.
    pub fn apply_library_fixture(&mut self, fixture: SimulatorLibraryFixture) {
        match fixture {
            SimulatorLibraryFixture::Normal => self
                .atlas_client
                .transport_mut()
                .push_outcome(MockTransportOutcome::response(200, NORMAL_LIBRARY_PAGE)),
            SimulatorLibraryFixture::Partial => {
                for page in PARTIAL_LIBRARY_PAGES {
                    self.atlas_client
                        .transport_mut()
                        .push_outcome(MockTransportOutcome::response(200, page));
                }
            }
        }
        self.state.refresh_atlas_library(&mut self.atlas_client);
        self.needs_redraw = true;
    }

    /// Applies one explicit bounded Search request; rendering remains inert.
    pub fn apply_search_fixture(&mut self, fixture: SimulatorSearchFixture) {
        let (query, outcome) = match fixture {
            SimulatorSearchFixture::Success => {
                ("plan", MockTransportOutcome::response(200, NORMAL_SEARCH))
            }
            SimulatorSearchFixture::NoResults => {
                ("missing", MockTransportOutcome::response(200, EMPTY_SEARCH))
            }
            SimulatorSearchFixture::Unicode => {
                ("café", MockTransportOutcome::response(200, UNICODE_SEARCH))
            }
            SimulatorSearchFixture::Unavailable => ("plan", MockTransportOutcome::unavailable()),
            SimulatorSearchFixture::Timeout => ("plan", MockTransportOutcome::timeout()),
            SimulatorSearchFixture::Offline => ("plan", MockTransportOutcome::offline()),
            SimulatorSearchFixture::Malformed => ("plan", MockTransportOutcome::malformed()),
            SimulatorSearchFixture::Oversized => ("plan", MockTransportOutcome::oversized()),
        };
        self.state.atlas_search.set_query(query);
        self.atlas_client.transport_mut().push_outcome(outcome);
        self.state.refresh_atlas_search(&mut self.atlas_client);
        self.needs_redraw = true;
    }

    /// Queue deterministic bounded View responses; result pages remain
    /// user-triggered through the real input path.
    pub fn apply_views_fixture(&mut self, fixture: SimulatorViewsFixture) {
        let transport = self.atlas_client.transport_mut();
        match fixture {
            SimulatorViewsFixture::Success => {
                transport.push_outcome(MockTransportOutcome::response(200, NORMAL_VIEWS_LIST));
                transport.push_outcome(MockTransportOutcome::response(200, NORMAL_VIEW_PAGE_ONE));
            }
            SimulatorViewsFixture::Empty => {
                transport.push_outcome(MockTransportOutcome::response(200, EMPTY_VIEWS));
            }
            SimulatorViewsFixture::Pagination => {
                transport.push_outcome(MockTransportOutcome::response(200, NORMAL_VIEWS_LIST));
                transport.push_outcome(MockTransportOutcome::response(200, NORMAL_VIEW_PAGE_ONE));
                transport.push_outcome(MockTransportOutcome::response(200, NORMAL_VIEW_PAGE_TWO));
                transport.push_outcome(MockTransportOutcome::response(
                    200,
                    NORMAL_VIEW_PAGE_TWO_NOTE,
                ));
            }
            SimulatorViewsFixture::Unavailable => {
                transport.push_outcome(MockTransportOutcome::unavailable())
            }
            SimulatorViewsFixture::Timeout => {
                transport.push_outcome(MockTransportOutcome::timeout())
            }
            SimulatorViewsFixture::Offline => {
                transport.push_outcome(MockTransportOutcome::offline())
            }
            SimulatorViewsFixture::Malformed => {
                transport.push_outcome(MockTransportOutcome::malformed())
            }
            SimulatorViewsFixture::Oversized => {
                transport.push_outcome(MockTransportOutcome::oversized())
            }
        }
        self.state.request_atlas_views_list();
        let request = self
            .state
            .take_atlas_views_request()
            .expect("explicit views list request");
        self.state
            .refresh_atlas_views(&mut self.atlas_client, request);
        self.needs_redraw = true;
    }

    /// Queue one deterministic reader outcome for the next user-selected
    /// Library Note. It is consumed only by the typed `GetNote` seam.
    pub fn queue_note_fixture(&mut self, fixture: SimulatorNoteFixture) {
        match fixture {
            SimulatorNoteFixture::Loaded => self
                .atlas_client
                .transport_mut()
                .push_outcome(MockTransportOutcome::response(200, NORMAL_NOTE)),
            SimulatorNoteFixture::OfflineCached => self
                .atlas_client
                .transport_mut()
                .push_outcome(MockTransportOutcome::offline()),
            SimulatorNoteFixture::Error => self
                .atlas_client
                .transport_mut()
                .push_outcome(MockTransportOutcome::not_found()),
        }
    }

    #[must_use]
    pub fn atlas_requests(&self) -> &[crate::atlas_client::TransportRequest] {
        self.atlas_client.transport().requests()
    }

    /// Applies one finite Note request sequence through the real reader seam.
    pub fn apply_note_fixture(&mut self, fixture: SimulatorNoteFixture) {
        const NOTE_ID: &str = "11111111-1111-4111-8111-111111111111";
        let mut transport = MockAtlasTransport::default();
        match fixture {
            SimulatorNoteFixture::Loaded => {
                transport.push_outcome(MockTransportOutcome::response(200, NORMAL_NOTE));
            }
            SimulatorNoteFixture::OfflineCached => {
                transport.push_outcome(MockTransportOutcome::response(200, NORMAL_NOTE));
                transport.push_outcome(MockTransportOutcome::offline());
            }
            SimulatorNoteFixture::Error => {
                transport.push_outcome(MockTransportOutcome::not_found());
            }
        }
        let mut client = AtlasClient::new(transport);
        let _ = self
            .state
            .begin_atlas_note(NOTE_ID, crate::app::router::AtlasNoteOrigin::Home);
        self.state.load_atlas_note(&mut client);
        if fixture == SimulatorNoteFixture::OfflineCached {
            let _ = self
                .state
                .begin_atlas_note(NOTE_ID, crate::app::router::AtlasNoteOrigin::Home);
            self.state.load_atlas_note(&mut client);
        }
        self.needs_redraw = true;
    }

    #[must_use]
    pub const fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }

    #[must_use]
    pub const fn logical_size(&self) -> (u32, u32) {
        (LOGICAL_WIDTH, LOGICAL_HEIGHT)
    }

    pub fn render(&mut self) -> Result<&[u8], Infallible> {
        if self.needs_redraw {
            render_current_screen(&mut self.frame, &self.state)?;
            self.needs_redraw = false;
        }
        Ok(self.frame.as_bytes())
    }

    pub fn handle_key(&mut self, key: SimulatorKey) -> Result<(), Infallible> {
        let Some(input) = key.semantic_input() else {
            return Ok(());
        };
        self.handle_input(input)
    }

    pub fn handle_input(&mut self, input: SemanticInput) -> Result<(), Infallible> {
        self.hardware.input.last = Some(input);
        if let Some(event) = input.button_event() {
            self.state.apply(event);
            if self.state.take_atlas_search_request() {
                self.state.refresh_atlas_search(&mut self.atlas_client);
            }
            if let Some(request) = self.state.take_atlas_views_request() {
                self.state
                    .refresh_atlas_views(&mut self.atlas_client, request);
            }
            if self.state.atlas_note.status() == crate::atlas_note::AtlasNoteStatus::Loading {
                self.state.load_atlas_note(&mut self.atlas_client);
            }
        } else {
            match input {
                SemanticInput::Back => self.state.back(),
                SemanticInput::BootShort => {
                    let _ = self.state.apply_keyboard_boot_short_press();
                }
                SemanticInput::Home => {
                    while self.state.active_route() != crate::app::ScreenRoute::Home
                        || self.state.atlas_route() != crate::app::router::AtlasRoute::Home
                    {
                        self.state.back();
                    }
                }
                SemanticInput::Power => self.state.open_power_key_menu(),
                SemanticInput::Up | SemanticInput::Down | SemanticInput::Select => unreachable!(),
            }
        }
        self.needs_redraw = true;
        Ok(())
    }
}

fn push_home_responses(transport: &mut MockAtlasTransport, notes: &str, views: &str) {
    transport.push_outcome(MockTransportOutcome::response(200, notes));
    transport.push_outcome(MockTransportOutcome::response(200, views));
}

const EMPTY_NOTES: &str = r#"{"items":[],"nextCursor":null}"#;
const EMPTY_VIEWS: &str = r#"{"items":[]}"#;
const NORMAL_VIEWS_LIST: &str = r#"{"items":[{"id":"22222222-2222-4222-8222-222222222222","name":"Today","revision":"r1","status":"ok","layout":"table"}]}"#;
const NORMAL_VIEW_PAGE_ONE: &str = r#"{"view":{"id":"22222222-2222-4222-8222-222222222222","name":"Today","revision":"r1","status":"ok","layout":"table"},"items":[{"id":"11111111-1111-4111-8111-111111111111","path":"Inbox/Plan.md","title":"Morning plan","state":"managed","revision":"r1"}],"nextCursor":"sim-next"}"#;
const NORMAL_VIEW_PAGE_TWO: &str = r#"{"view":{"id":"22222222-2222-4222-8222-222222222222","name":"Today","revision":"r1","status":"ok","layout":"table"},"items":[{"id":"33333333-3333-4333-8333-333333333333","path":"Inbox/Next.md","title":"Next plan","state":"managed","revision":"r1"}],"nextCursor":null}"#;
const NORMAL_VIEW_PAGE_TWO_NOTE: &str = r##"{"id":"33333333-3333-4333-8333-333333333333","title":"Next plan","revision":"r1","body":"# Next\n\nContinue the Atlas plan.","parentId":null,"order":null}"##;
const NORMAL_NOTE: &str = r##"{"id":"11111111-1111-4111-8111-111111111111","title":"Morning plan","revision":"r1","body":"# Morning\n\nReview Atlas notes.","parentId":null,"order":null}"##;
const NORMAL_NOTES: &str = r#"{"items":[{"id":"11111111-1111-1111-1111-111111111111","path":"Inbox.md","title":"Morning plan","state":"managed","revision":"r1","parentId":null,"order":null}],"nextCursor":null}"#;
const NORMAL_VIEWS: &str = r#"{"items":[{"id":"33333333-3333-3333-3333-333333333333","name":"Today","revision":"r1","status":"ok","layout":"list"}]}"#;
const LONG_TITLE_NOTES: &str = r#"{"items":[{"id":"11111111-1111-1111-1111-111111111111","path":"Inbox.md","title":"WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW","state":"managed","revision":"r1","parentId":null,"order":null}],"nextCursor":null}"#;
const LONG_TITLE_VIEWS: &str = r#"{"items":[{"id":"33333333-3333-3333-3333-333333333333","name":"WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW","revision":"r1","status":"ok","layout":"list"}]}"#;
const NORMAL_LIBRARY_PAGE: &str = r#"{"items":[{"id":"11111111-1111-4111-8111-111111111111","path":"Inbox.md","title":"Parent","state":"managed","revision":"r1","parentId":null,"order":"a"},{"id":"22222222-2222-4222-8222-222222222222","path":"Inbox/Child.md","title":"Child","state":"managed","revision":"r1","parentId":"11111111-1111-4111-8111-111111111111","order":"a"}],"nextCursor":null}"#;
const NORMAL_SEARCH: &str = r#"{"query":"plan","total":1,"hits":[{"atlasId":"11111111-1111-4111-8111-111111111111","path":"ignored.md","title":"Morning plan","snippet":"Review Atlas notes.","revision":"r1","state":"managed"}]}"#;
const EMPTY_SEARCH: &str = r#"{"query":"missing","total":0,"hits":[]}"#;
const UNICODE_SEARCH: &str = r#"{"query":"café","total":1,"hits":[{"atlasId":"11111111-1111-4111-8111-111111111111","path":"ignored.md","title":"Café plan","snippet":"Résumé café.","revision":"r1","state":"managed"}]}"#;
const PARTIAL_LIBRARY_PAGES: [&str; 4] = [
    r#"{"items":[{"id":"11111111-1111-4111-8111-111111111111","path":"Inbox.md","title":"Parent","state":"managed","revision":"r1","parentId":null,"order":"a"}],"nextCursor":"page-2"}"#,
    r#"{"items":[],"nextCursor":"page-3"}"#,
    r#"{"items":[],"nextCursor":"page-4"}"#,
    r#"{"items":[],"nextCursor":"more"}"#,
];
