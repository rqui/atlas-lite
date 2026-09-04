use waveshare_epd397_rust_app::{
    app::{
        router::{AtlasNavigationSurface, AtlasNoteOrigin, AtlasRoute},
        screens::atlas_search::atlas_search_chrome,
        AppState,
    },
    atlas_client::{AtlasClient, MockAtlasTransport, MockTransportOutcome, TransportRequest},
    atlas_search::{AtlasSearchFocus, SEARCH_RESULT_LIMIT, SEARCH_SNIPPET_MAX_BYTES},
    atlas_state::AtlasConnectionState,
    buttons::ButtonEvent,
    simulator::{Simulator, SimulatorNoteFixture, SimulatorSearchFixture},
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
