use waveshare_epd397_rust_app::atlas_client::{
    AtlasClient, AtlasClientError, CaptureTextRequest, MockAtlasTransport, MockTransportOutcome,
    TransportRequest, MAX_CURSOR_BYTES, MAX_SEARCH_OFFSET,
};
use waveshare_epd397_rust_app::atlas_dto::MAX_RESPONSE_BODY_BYTES;

const TEST_IDEMPOTENCY_KEY: &str = "v1.1735689600.AAAAAAAAAAAAAAAAAAAAAA";
const NOTE_ID: &str = "00000000-0000-4000-8000-000000000001";
const VIEW_ID: &str = "00000000-0000-4000-8000-000000000002";

const NOTES: &[u8] = br#"{"items":[],"nextCursor":null}"#;
const NOTE: &[u8] = br#"{"id":null,"path":"notes/one.md","state":"managed","title":"One","revision":"r1","body":"body","parentId":null,"order":null}"#;
const SEARCH: &[u8] = br#"{"query":"term","total":0,"hits":[]}"#;
const VIEWS: &[u8] = br#"{"items":[]}"#;
const VIEW_RESULTS: &[u8] = br#"{"view":{"id":"view-1","name":"View","revision":"r1","status":"ok","layout":"list"},"items":[],"nextCursor":null}"#;

#[test]
fn client_routes_note_reads_and_parses_the_bounded_dto() {
    let mut transport = MockAtlasTransport::default();
    transport.push_outcome(MockTransportOutcome::response(200, NOTES));
    let mut client = AtlasClient::new(transport);

    let page = client.list_notes(Some("next-page"), 1).unwrap();

    assert!(page.items.is_empty());
    assert_eq!(
        client.transport().requests(),
        &[TransportRequest::ListNotes {
            cursor: Some("next-page".into()),
            limit: 1,
        }]
    );
}

#[test]
fn capture_forwards_the_same_idempotency_key_on_a_retry() {
    let request = CaptureTextRequest::new("remember this").unwrap();
    let mut transport = MockAtlasTransport::default();
    transport.push_outcome(MockTransportOutcome::offline());
    transport.push_outcome(MockTransportOutcome::response(204, b""));
    let mut client = AtlasClient::new(transport);

    assert_eq!(
        client.capture_text(&request, TEST_IDEMPOTENCY_KEY),
        Err(AtlasClientError::Offline)
    );
    client.capture_text(&request, TEST_IDEMPOTENCY_KEY).unwrap();

    assert_eq!(
        client.transport().requests(),
        &[
            TransportRequest::CaptureText {
                request: request.clone(),
                idempotency_key: TEST_IDEMPOTENCY_KEY.into(),
            },
            TransportRequest::CaptureText {
                request,
                idempotency_key: TEST_IDEMPOTENCY_KEY.into(),
            },
        ]
    );
}

#[test]
fn capture_rejects_oversized_success_and_preserves_idempotency_routing() {
    let request = CaptureTextRequest::new("remember this").unwrap();
    let mut transport = MockAtlasTransport::default();
    transport.push_outcome(MockTransportOutcome::oversized());
    let mut client = AtlasClient::new(transport);

    assert_eq!(
        client.capture_text(&request, TEST_IDEMPOTENCY_KEY),
        Err(AtlasClientError::ResponseTooLarge)
    );
    assert_eq!(
        client.transport().requests(),
        &[TransportRequest::CaptureText {
            request,
            idempotency_key: TEST_IDEMPOTENCY_KEY.into(),
        }]
    );
}

#[test]
fn client_rejects_oversized_or_malformed_variable_inputs_before_transport() {
    let mut client = AtlasClient::new(MockAtlasTransport::default());

    assert!(matches!(
        client.list_notes(Some(&"x".repeat(MAX_CURSOR_BYTES + 1)), 1),
        Err(AtlasClientError::InvalidRequest(_))
    ));
    assert!(matches!(
        client.get_note("not-a-uuid"),
        Err(AtlasClientError::InvalidRequest(_))
    ));
    assert!(matches!(
        client.search(&"q".repeat(1025), 1, 0),
        Err(AtlasClientError::InvalidRequest(_))
    ));
    assert!(matches!(
        client.search("q", 1, MAX_SEARCH_OFFSET + 1),
        Err(AtlasClientError::InvalidRequest(_))
    ));
    assert!(matches!(
        client.capture_text(&CaptureTextRequest::new("safe").unwrap(), "capture-001"),
        Err(AtlasClientError::InvalidRequest(_))
    ));
    assert!(client.transport().requests().is_empty());
}

#[test]
fn client_accepts_an_exactly_bounded_read_and_capture_response() {
    let mut exact_notes = NOTES.to_vec();
    exact_notes.resize(MAX_RESPONSE_BODY_BYTES, b' ');
    let exact_capture = vec![b' '; MAX_RESPONSE_BODY_BYTES];
    let mut transport = MockAtlasTransport::default();
    transport.push_outcome(MockTransportOutcome::response(200, exact_notes));
    transport.push_outcome(MockTransportOutcome::response(204, exact_capture));
    let mut client = AtlasClient::new(transport);

    assert!(client.list_notes(None, 1).unwrap().items.is_empty());
    client
        .capture_text(
            &CaptureTextRequest::new("safe").unwrap(),
            TEST_IDEMPOTENCY_KEY,
        )
        .unwrap();
}

#[test]
fn client_routes_every_read_operation_through_the_transport() {
    let mut transport = MockAtlasTransport::default();
    for body in [NOTE, SEARCH, VIEWS, VIEW_RESULTS] {
        transport.push_outcome(MockTransportOutcome::response(200, body));
    }
    let mut client = AtlasClient::new(transport);

    assert_eq!(client.get_note(NOTE_ID).unwrap().title, "One");
    assert!(client.search("term", 1, 0).unwrap().hits.is_empty());
    assert!(client.list_views().unwrap().items.is_empty());
    assert!(client
        .get_view_results(VIEW_ID, Some("cursor-2"), 1)
        .unwrap()
        .items
        .is_empty());

    assert_eq!(
        client.transport().requests(),
        &[
            TransportRequest::GetNote { id: NOTE_ID.into() },
            TransportRequest::Search {
                query: "term".into(),
                limit: 1,
                offset: 0,
            },
            TransportRequest::ListViews,
            TransportRequest::GetViewResults {
                id: VIEW_ID.into(),
                cursor: Some("cursor-2".into()),
                limit: 1,
            },
        ]
    );
}

#[test]
fn mock_exposes_every_required_typed_failure_without_secrets() {
    let expected = [
        MockTransportOutcome::unauthorized(),
        MockTransportOutcome::forbidden(),
        MockTransportOutcome::not_found(),
        MockTransportOutcome::rate_limited(),
        MockTransportOutcome::unavailable(),
        MockTransportOutcome::timeout(),
        MockTransportOutcome::offline(),
        MockTransportOutcome::malformed(),
        MockTransportOutcome::oversized(),
    ];

    for outcome in expected {
        let mut transport = MockAtlasTransport::default();
        transport.push_outcome(outcome);
        let mut client = AtlasClient::new(transport);
        let error = client.list_notes(None, 1).unwrap_err();
        match error {
            AtlasClientError::Unauthorized(error) => assert_eq!(error.code, "ATLAS_UNAUTHORIZED"),
            AtlasClientError::Forbidden(error) => assert_eq!(error.code, "ATLAS_FORBIDDEN"),
            AtlasClientError::NotFound(error) => assert_eq!(error.code, "NOTE_NOT_FOUND"),
            AtlasClientError::RateLimited(error) => assert_eq!(error.code, "RATE_LIMITED"),
            AtlasClientError::IndexNotReady {
                error,
                retry_after_seconds,
            } => {
                assert_eq!(error.code, "INDEX_NOT_READY");
                assert_eq!(retry_after_seconds, None);
            }
            AtlasClientError::Timeout
            | AtlasClientError::Offline
            | AtlasClientError::MalformedPayload
            | AtlasClientError::ResponseTooLarge => {}
            other => panic!("unexpected mock failure: {other:?}"),
        }
    }
}

#[test]
fn client_preserves_bounded_index_not_ready_retry_after_metadata() {
    let mut transport = MockAtlasTransport::default();
    transport.push_outcome(MockTransportOutcome::unavailable_with_retry_after(37));
    let mut client = AtlasClient::new(transport);

    assert!(matches!(
        client.search("term", 1, 0),
        Err(AtlasClientError::IndexNotReady {
            retry_after_seconds: Some(37),
            ..
        })
    ));

    let mut transport = MockAtlasTransport::default();
    transport.push_outcome(MockTransportOutcome::response_with_retry_after(
        503,
        br#"{"error":{"code":"INDEX_NOT_READY","message":"mock failure","requestId":"mock-request"}}"#,
        u32::MAX,
    ));
    let mut client = AtlasClient::new(transport);
    assert!(matches!(
        client.search("term", 1, 0),
        Err(AtlasClientError::IndexNotReady {
            retry_after_seconds: None,
            ..
        })
    ));
}
