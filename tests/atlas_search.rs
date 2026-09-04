use waveshare_epd397_rust_app::{
    app::{
        router::{AtlasNavigationSurface, AtlasNoteOrigin, AtlasRoute},
        screens::atlas_search::{atlas_search_chrome, atlas_search_retry_guidance},
        AppState,
    },
    atlas_client::{AtlasClient, MockAtlasTransport, MockTransportOutcome, TransportRequest},
    atlas_search::{AtlasSearchFocus, SEARCH_RESULT_LIMIT, SEARCH_SNIPPET_MAX_BYTES},
    atlas_state::AtlasConnectionState,
    buttons::ButtonEvent,
    simulator::{SemanticInput, Simulator, SimulatorNoteFixture, SimulatorSearchFixture},
};

const ID: &str = "11111111-1111-4111-8111-111111111111";
const SEARCH: &[u8] = br#"{"query":"plan","total":1,"hits":[{"atlasId":"11111111-1111-4111-8111-111111111111","path":"ignored.md","title":"Morning plan","snippet":"Review Atlas notes.","revision":"r1","state":"managed"}]}"#;
const EMPTY: &[u8] = br#"{"query":"missing","total":0,"hits":[]}"#;
const NOTE: &[u8] = br##"{"id":"11111111-1111-4111-8111-111111111111","title":"Morning plan","revision":"r1","body":"# Plan","parentId":null,"order":null}"##;

fn client_with(outcome: MockTransportOutcome) -> AtlasClient<MockAtlasTransport> {
    let mut transport = MockAtlasTransport::default();
    transport.push_outcome(outcome);
    AtlasClient::new(transport)
}

#[test]
fn empty_query_never_reaches_transport_or_changes_other_surface_state() {
    let mut state = AppState::default();
    let mut client = AtlasClient::new(MockAtlasTransport::default());
    state.refresh_atlas_search(&mut client);

    assert!(client.transport().requests().is_empty());
    assert_eq!(
        state.atlas_search_connection,
        AtlasConnectionState::Unconfigured
    );
    assert_eq!(
        state.atlas_library_connection,
        AtlasConnectionState::Unconfigured
    );
    assert_eq!(state.atlas.connection, AtlasConnectionState::Unconfigured);
    assert_eq!(atlas_search_chrome(&state).status(), "TYPE QUERY");
}

#[test]
fn success_unicode_and_no_results_are_bounded_and_surface_local() {
    let mut state = AppState::default();
    let mut client = client_with(MockTransportOutcome::response(200, SEARCH));
    state.atlas_search.set_query("café");
    state.refresh_atlas_search(&mut client);
    assert_eq!(state.atlas_search.results()[0].id(), ID);
    assert_eq!(
        state.atlas_search_connection,
        AtlasConnectionState::Connected
    );
    assert_eq!(state.atlas_search.focus(), AtlasSearchFocus::Results);
    assert_eq!(
        client.transport().requests(),
        &[TransportRequest::Search {
            query: "café".into(),
            limit: SEARCH_RESULT_LIMIT,
            offset: 0
        }]
    );

    client
        .transport_mut()
        .push_outcome(MockTransportOutcome::response(200, EMPTY));
    state.atlas_search.set_query("missing");
    state.refresh_atlas_search(&mut client);
    assert!(state.atlas_search.results().is_empty());
    assert_eq!(atlas_search_chrome(&state).status(), "NO RESULTS");
}

#[test]
fn unicode_input_and_rendered_response_fields_respect_byte_budgets() {
    let mut state = AppState::default();
    state.atlas_search.set_query(&"é".repeat(100));
    assert!(state.atlas_search.query().len() <= 128);
    assert!(state
        .atlas_search
        .query()
        .is_char_boundary(state.atlas_search.query().len()));

    let long = "界".repeat(100);
    let response = format!(
        r#"{{"query":"x","total":1,"hits":[{{"atlasId":"{ID}","path":"ignored","title":"{long}","snippet":"{long}","revision":"r1","state":"managed"}}]}}"#
    );
    let mut client = client_with(MockTransportOutcome::response(200, response.as_bytes()));
    state.atlas_search.set_query("x");
    state.refresh_atlas_search(&mut client);
    assert!(state.atlas_search.results()[0].snippet().len() <= SEARCH_SNIPPET_MAX_BYTES);
    assert!(state.atlas_search.results()[0].snippet().ends_with('…'));
}

#[test]
fn failures_preserve_last_safe_results_and_are_not_home_or_library_errors() {
    let mut state = AppState::default();
    let mut client = client_with(MockTransportOutcome::response(200, SEARCH));
    state.atlas_search.set_query("plan");
    state.refresh_atlas_search(&mut client);
    let safe_results = state.atlas_search.results().to_vec();

    for outcome in [
        MockTransportOutcome::unavailable(),
        MockTransportOutcome::timeout(),
        MockTransportOutcome::offline(),
        MockTransportOutcome::malformed(),
        MockTransportOutcome::oversized(),
    ] {
        client.transport_mut().push_outcome(outcome);
        state.refresh_atlas_search(&mut client);
        assert_eq!(state.atlas_search.results(), safe_results.as_slice());
        assert_eq!(
            state.atlas_library_connection,
            AtlasConnectionState::Unconfigured
        );
        assert_eq!(state.atlas.connection, AtlasConnectionState::Unconfigured);
    }
    assert_eq!(
        state.atlas_search_connection,
        AtlasConnectionState::ServerError
    );
    assert_eq!(atlas_search_chrome(&state).status(), "ERROR CACHED");
}

#[test]
fn refine_or_failed_refresh_never_labels_prior_query_hits_as_current_results() {
    let mut state = AppState::default();
    let mut client = client_with(MockTransportOutcome::response(200, SEARCH));
    state.atlas_search.set_query("plan");
    state.refresh_atlas_search(&mut client);
    assert_eq!(state.atlas_search.results().len(), 1);

    state.atlas_search.set_query("plans");
    client
        .transport_mut()
        .push_outcome(MockTransportOutcome::offline());
    state.refresh_atlas_search(&mut client);

    assert!(state.atlas_search.results().is_empty());
    assert_eq!(atlas_search_chrome(&state).status(), "OFFLINE");
    assert_eq!(atlas_search_chrome(&state).source(), "EMPTY");
}

#[test]
fn simulator_boot_short_reaches_go_and_refine_through_real_semantic_input() {
    let mut simulator = Simulator::default();
    simulator.handle_input(SemanticInput::Down).unwrap();
    simulator.handle_input(SemanticInput::Select).unwrap();
    simulator.handle_input(SemanticInput::Select).unwrap();
    for _ in 0..5 {
        simulator.handle_input(SemanticInput::Down).unwrap();
    }
    simulator.handle_input(SemanticInput::BootShort).unwrap();
    for _ in 0..4 {
        simulator.handle_input(SemanticInput::Down).unwrap();
    }
    simulator.handle_input(SemanticInput::Select).unwrap();

    assert_eq!(simulator.state().atlas_search.query(), "A");
    assert_eq!(
        simulator.atlas_requests()[0],
        TransportRequest::Search {
            query: "A".into(),
            limit: SEARCH_RESULT_LIMIT,
            offset: 0,
        }
    );
    simulator.handle_input(SemanticInput::Select).unwrap();
    assert_eq!(
        simulator.state().atlas_search.focus(),
        AtlasSearchFocus::Input
    );
}

#[test]
fn index_not_ready_without_retry_after_keeps_its_specific_status_and_guidance() {
    let mut state = AppState::default();
    let mut client = client_with(MockTransportOutcome::unavailable());
    state.atlas_search.set_query("plan");

    state.refresh_atlas_search(&mut client);

    assert!(state.atlas_search.index_not_ready());
    assert_eq!(state.atlas_search.retry_after_seconds(), None);
    assert_eq!(atlas_search_chrome(&state).status(), "INDEX NOT READY");
    assert_eq!(atlas_search_retry_guidance(&state), "RETRY SEARCH");

    let mut invalid_retry_after = client_with(MockTransportOutcome::response_with_retry_after(
        503,
        br#"{"error":{"code":"INDEX_NOT_READY","message":"Index is not ready","requestId":"req-1"}}"#,
        u32::MAX,
    ));
    state.refresh_atlas_search(&mut invalid_retry_after);
    assert!(state.atlas_search.index_not_ready());
    assert_eq!(state.atlas_search.retry_after_seconds(), None);
    assert_eq!(atlas_search_retry_guidance(&state), "RETRY SEARCH");
}

#[test]
fn refine_action_returns_to_input_so_no_results_and_failures_can_retry_or_edit() {
    let mut state = AppState::default();
    let mut client = client_with(MockTransportOutcome::response(200, EMPTY));
    state.atlas_search.set_query("missing");
    state.refresh_atlas_search(&mut client);
    state
        .router
        .navigate_atlas_to(AtlasNavigationSurface::Search);

    assert!(state.atlas_search.refine_selected());
    state.apply(ButtonEvent::Select);
    assert_eq!(state.atlas_search.focus(), AtlasSearchFocus::Input);

    state.atlas_search.set_query("plan");
    client
        .transport_mut()
        .push_outcome(MockTransportOutcome::unavailable_with_retry_after(21));
    state.refresh_atlas_search(&mut client);
    assert_eq!(state.atlas_search.focus(), AtlasSearchFocus::Results);
    assert_eq!(state.atlas_search.retry_after_seconds(), Some(21));
    assert_eq!(atlas_search_chrome(&state).status(), "INDEX NOT READY");
    assert_eq!(atlas_search_retry_guidance(&state), "RETRY 21S");
    state.apply(ButtonEvent::Select);
    assert_eq!(state.atlas_search.focus(), AtlasSearchFocus::Input);
    state.atlas_search.set_query("retry");
    client
        .transport_mut()
        .push_outcome(MockTransportOutcome::response(200, EMPTY));
    state.refresh_atlas_search(&mut client);
    assert_eq!(state.atlas_search.query(), "retry");
    assert_eq!(state.atlas_search.retry_after_seconds(), None);
}

#[test]
fn search_result_opens_note_with_search_origin_and_back_returns_to_search() {
    let mut state = AppState::default();
    let mut client = client_with(MockTransportOutcome::response(200, SEARCH));
    state.atlas_search.set_query("plan");
    state.refresh_atlas_search(&mut client);
    state
        .router
        .navigate_atlas_to(AtlasNavigationSurface::Search);
    client
        .transport_mut()
        .push_outcome(MockTransportOutcome::response(200, NOTE));

    state.apply(ButtonEvent::Select);
    assert_eq!(state.atlas_route(), AtlasRoute::Note);
    assert_eq!(state.atlas_note.selected_id(), Some(ID));
    assert_eq!(state.atlas_note.origin(), Some(AtlasNoteOrigin::Search));
    state.load_atlas_note(&mut client);
    state.back();
    assert_eq!(state.atlas_route(), AtlasRoute::Search);
}

#[test]
fn simulator_fixtures_cover_success_no_results_unicode_and_typed_errors_deterministically() {
    for fixture in [
        SimulatorSearchFixture::Success,
        SimulatorSearchFixture::NoResults,
        SimulatorSearchFixture::Unicode,
        SimulatorSearchFixture::Unavailable,
        SimulatorSearchFixture::Timeout,
        SimulatorSearchFixture::Offline,
        SimulatorSearchFixture::Malformed,
        SimulatorSearchFixture::Oversized,
    ] {
        let mut simulator = Simulator::default();
        simulator.apply_search_fixture(fixture);
        let first = simulator.render().unwrap().to_vec();
        let second = simulator.render().unwrap().to_vec();
        assert_eq!(
            first, second,
            "fixture {fixture:?} repolled while rendering"
        );
    }

    let mut simulator = Simulator::default();
    simulator.apply_search_fixture(SimulatorSearchFixture::Success);
    simulator.queue_note_fixture(SimulatorNoteFixture::Loaded);
    // Search results become interactive only on the Search route.
    simulator
        .handle_key(waveshare_epd397_rust_app::simulator::SimulatorKey::ArrowDown)
        .unwrap();
    simulator
        .handle_key(waveshare_epd397_rust_app::simulator::SimulatorKey::Enter)
        .unwrap();
    simulator
        .handle_key(waveshare_epd397_rust_app::simulator::SimulatorKey::Enter)
        .unwrap();
    assert_eq!(simulator.state().atlas_route(), AtlasRoute::Note);
}
