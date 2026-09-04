use waveshare_epd397_rust_app::{
    app::{
        router::{AtlasNavigationSurface, AtlasNoteOrigin, AtlasRoute},
        screens::atlas_views::{views_page_label, views_source, views_status},
        AppState,
    },
    atlas_client::{AtlasClient, MockAtlasTransport, MockTransportOutcome, TransportRequest},
    atlas_state::AtlasConnectionState,
    atlas_views::{AtlasViewsRequest, VIEW_RESULT_LIMIT},
    buttons::ButtonEvent,
    simulator::{Simulator, SimulatorNoteFixture, SimulatorViewsFixture},
};

const VIEW_ID: &str = "22222222-2222-4222-8222-222222222222";
const NOTE_ID: &str = "11111111-1111-4111-8111-111111111111";
const VIEWS: &[u8] = br#"{"items":[{"id":"22222222-2222-4222-8222-222222222222","name":"Today","revision":"r1","status":"ok","layout":"board"}]}"#;
const EMPTY: &[u8] = br#"{"items":[]}"#;
const PAGE_ONE: &[u8] = br#"{"view":{"id":"22222222-2222-4222-8222-222222222222","name":"Today","revision":"r1","status":"ok","layout":"table"},"items":[{"id":"11111111-1111-4111-8111-111111111111","path":"Inbox/Plan.md","title":"Morning plan","state":"managed","revision":"r1"}],"nextCursor":"opaque-next"}"#;
const PAGE_TWO: &[u8] = br#"{"view":{"id":"22222222-2222-4222-8222-222222222222","name":"Today","revision":"r1","status":"ok","layout":"calendar"},"items":[{"id":"33333333-3333-4333-8333-333333333333","path":"Inbox/Next.md","title":"Next plan","state":"managed","revision":"r2"}],"nextCursor":null}"#;
const NOTE: &[u8] = br##"{"id":"11111111-1111-4111-8111-111111111111","title":"Morning plan","revision":"r1","body":"# Plan","parentId":null,"order":null}"##;

fn page_with_cursor(cursor: &str) -> Vec<u8> {
    format!(
        r#"{{"view":{{"id":"{VIEW_ID}","name":"Today","revision":"r1","status":"ok","layout":"table"}},"items":[{{"id":"{NOTE_ID}","path":"Inbox/Plan.md","title":"Morning plan","state":"managed","revision":"r1"}}],"nextCursor":"{cursor}"}}"#
    )
    .into_bytes()
}

fn execute_pending(state: &mut AppState, client: &mut AtlasClient<MockAtlasTransport>) {
    let request = state
        .take_atlas_views_request()
        .expect("explicit Views request");
    state.refresh_atlas_views(client, request);
}

#[test]
fn list_selection_pages_and_note_back_use_only_typed_explicit_requests() {
    let mut state = AppState::default();
    let mut transport = MockAtlasTransport::default();
    transport.push_outcome(MockTransportOutcome::response(200, VIEWS));
    transport.push_outcome(MockTransportOutcome::response(200, PAGE_ONE));
    transport.push_outcome(MockTransportOutcome::response(200, PAGE_TWO));
    transport.push_outcome(MockTransportOutcome::response(200, NOTE));
    let mut client = AtlasClient::new(transport);

    state.request_atlas_views_list();
    execute_pending(&mut state, &mut client);
    assert_eq!(state.atlas_views.views().len(), 1);
    state
        .router
        .navigate_atlas_to(AtlasNavigationSurface::Views);
    state.apply(ButtonEvent::Select);
    execute_pending(&mut state, &mut client);
    assert_eq!(state.atlas_views.results()[0].id(), NOTE_ID);
    assert!(state.atlas_views.pagination_incomplete());
    state.apply(ButtonEvent::Down);
    state.apply(ButtonEvent::Select);
    execute_pending(&mut state, &mut client);
    assert_eq!(state.atlas_views.page_number(), 2);
    assert_eq!(state.atlas_views.results()[0].title(), "Next plan");

    state.apply(ButtonEvent::Select);
    assert_eq!(state.atlas_route(), AtlasRoute::Note);
    assert_eq!(state.atlas_note.origin(), Some(AtlasNoteOrigin::Views));
    state.load_atlas_note(&mut client);
    state.back();
    assert_eq!(state.atlas_route(), AtlasRoute::Views);
    assert_eq!(
        client.transport().requests(),
        &[
            TransportRequest::ListViews,
            TransportRequest::GetViewResults {
                id: VIEW_ID.into(),
                cursor: None,
                limit: VIEW_RESULT_LIMIT
            },
            TransportRequest::GetViewResults {
                id: VIEW_ID.into(),
                cursor: Some("opaque-next".into()),
                limit: VIEW_RESULT_LIMIT
            },
            TransportRequest::GetNote {
                id: "33333333-3333-4333-8333-333333333333".into()
            },
        ]
    );
}

#[test]
fn errors_and_malformed_or_oversized_responses_preserve_views_only_safe_data() {
    let mut state = AppState::default();
    let mut transport = MockAtlasTransport::default();
    transport.push_outcome(MockTransportOutcome::response(200, VIEWS));
    transport.push_outcome(MockTransportOutcome::response(200, PAGE_ONE));
    let mut client = AtlasClient::new(transport);
    state.request_atlas_views_list();
    execute_pending(&mut state, &mut client);
    state
        .router
        .navigate_atlas_to(AtlasNavigationSurface::Views);
    state.apply(ButtonEvent::Select);
    execute_pending(&mut state, &mut client);
    let safe = state.atlas_views.results().to_vec();
    for (index, outcome) in [
        MockTransportOutcome::not_found(),
        MockTransportOutcome::unavailable(),
        MockTransportOutcome::timeout(),
        MockTransportOutcome::offline(),
        MockTransportOutcome::malformed(),
        MockTransportOutcome::oversized(),
    ]
    .into_iter()
    .enumerate()
    {
        client.transport_mut().push_outcome(outcome);
        if index == 0 {
            state.apply(ButtonEvent::Down);
        }
        state.apply(ButtonEvent::Select);
        execute_pending(&mut state, &mut client);
        assert_eq!(state.atlas_views.results(), safe.as_slice());
        assert_eq!(
            state.atlas_search_connection,
            AtlasConnectionState::Unconfigured
        );
        assert_eq!(
            state.atlas_library_connection,
            AtlasConnectionState::Unconfigured
        );
    }
    assert_eq!(views_status(&state), "ERROR CACHED");
}

#[test]
fn invalid_next_cursor_metadata_is_rejected_before_page_state_mutation() {
    let mut state = AppState::default();
    let mut transport = MockAtlasTransport::default();
    transport.push_outcome(MockTransportOutcome::response(200, VIEWS));
    transport.push_outcome(MockTransportOutcome::response(200, PAGE_ONE));
    let mut client = AtlasClient::new(transport);
    state.request_atlas_views_list();
    execute_pending(&mut state, &mut client);
    state
        .router
        .navigate_atlas_to(AtlasNavigationSurface::Views);
    state.apply(ButtonEvent::Select);
    execute_pending(&mut state, &mut client);
    let safe_results = state.atlas_views.results().to_vec();
    let safe_page = state.atlas_views.page_number();
    let safe_requests = state.atlas_views.page_requests();
    let safe_incomplete = state.atlas_views.pagination_incomplete();

    for cursor in [String::new(), "x".repeat(129)] {
        client
            .transport_mut()
            .push_outcome(MockTransportOutcome::response(
                200,
                &page_with_cursor(&cursor),
            ));
        let request = state
            .atlas_views
            .next_page_request()
            .expect("valid prior cursor");
        state.refresh_atlas_views(&mut client, request);
        assert_eq!(state.atlas_views.results(), safe_results.as_slice());
        assert_eq!(state.atlas_views.page_number(), safe_page);
        assert_eq!(state.atlas_views.page_requests(), safe_requests);
        assert_eq!(state.atlas_views.pagination_incomplete(), safe_incomplete);
        assert_eq!(
            state.atlas_views_connection,
            AtlasConnectionState::ServerError
        );
        assert_eq!(views_source(&state), "CACHED");
    }
}

#[test]
fn reopening_same_view_starts_a_fresh_cursor_session_after_the_cap() {
    let mut state = AppState::default();
    let mut transport = MockAtlasTransport::default();
    transport.push_outcome(MockTransportOutcome::response(200, VIEWS));
    transport.push_outcome(MockTransportOutcome::response(200, PAGE_ONE));
    transport.push_outcome(MockTransportOutcome::response(200, PAGE_ONE));
    transport.push_outcome(MockTransportOutcome::response(200, PAGE_ONE));
    let mut client = AtlasClient::new(transport);
    state.request_atlas_views_list();
    execute_pending(&mut state, &mut client);
    state
        .router
        .navigate_atlas_to(AtlasNavigationSurface::Views);
    state.apply(ButtonEvent::Select);
    execute_pending(&mut state, &mut client);
    for _ in 0..2 {
        let request = state
            .atlas_views
            .next_page_request()
            .expect("page budget remains");
        state.refresh_atlas_views(&mut client, request);
    }
    assert_eq!(state.atlas_views.page_requests(), 3);
    assert!(!state.atlas_views.next_page_available());
    assert!(!state.atlas_views.next_page_selected());
    assert_eq!(views_page_label(&state.atlas_views), "MORE LIMIT");

    assert_eq!(
        state.atlas_views.select_view_request(),
        Some(AtlasViewsRequest::Results {
            id: VIEW_ID.into(),
            cursor: None
        })
    );
    assert_eq!(state.atlas_views.page_requests(), 0);
    assert!(!state.atlas_views.pagination_incomplete());
    assert!(!state.atlas_views.next_page_available());
    assert_eq!(state.atlas_views.results()[0].id(), NOTE_ID);
}

#[test]
fn views_source_comes_only_from_views_freshness_not_retained_data() {
    let mut state = AppState::default();
    let mut client = client_from(MockTransportOutcome::response(200, VIEWS));
    state.request_atlas_views_list();
    execute_pending(&mut state, &mut client);
    assert_eq!(views_source(&state), "LIVE");
    for connection in [
        AtlasConnectionState::Offline,
        AtlasConnectionState::Timeout,
        AtlasConnectionState::ServerError,
    ] {
        state.atlas_views_connection = connection;
        assert_eq!(views_source(&state), "CACHED");
    }
}

fn client_from(outcome: MockTransportOutcome) -> AtlasClient<MockAtlasTransport> {
    let mut transport = MockAtlasTransport::default();
    transport.push_outcome(outcome);
    AtlasClient::new(transport)
}

#[test]
fn empty_invalid_and_bounded_pages_are_explicit_without_desktop_layout_data() {
    let mut state = AppState::default();
    let mut transport = MockAtlasTransport::default();
    transport.push_outcome(MockTransportOutcome::response(200, EMPTY));
    let mut client = AtlasClient::new(transport);
    state.request_atlas_views_list();
    execute_pending(&mut state, &mut client);
    assert_eq!(views_status(&state), "NO VIEWS");

    let invalid = br#"{"items":[{"id":"not-an-id","name":"Broken","revision":"r1","status":"invalid","layout":"cards"}]}"#;
    client
        .transport_mut()
        .push_outcome(MockTransportOutcome::response(200, invalid));
    state.request_atlas_views_list();
    execute_pending(&mut state, &mut client);
    state
        .router
        .navigate_atlas_to(AtlasNavigationSurface::Views);
    state.apply(ButtonEvent::Select);
    assert!(state.take_atlas_views_request().is_none());
    assert!(!state.atlas_views.views()[0].valid());
}

#[test]
fn simulator_fixtures_are_deterministic_and_rendering_never_polls() {
    for fixture in [
        SimulatorViewsFixture::Success,
        SimulatorViewsFixture::Empty,
        SimulatorViewsFixture::Pagination,
        SimulatorViewsFixture::Unavailable,
        SimulatorViewsFixture::Timeout,
        SimulatorViewsFixture::Offline,
        SimulatorViewsFixture::Malformed,
        SimulatorViewsFixture::Oversized,
    ] {
        let mut simulator = Simulator::default();
        simulator.apply_views_fixture(fixture);
        let first = simulator.render().unwrap().to_vec();
        let second = simulator.render().unwrap().to_vec();
        assert_eq!(
            first, second,
            "fixture {fixture:?} repolled while rendering"
        );
    }

    let mut simulator = Simulator::default();
    simulator.apply_views_fixture(SimulatorViewsFixture::Success);
    simulator.queue_note_fixture(SimulatorNoteFixture::Loaded);
    let _ = simulator;
}
