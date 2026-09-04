use waveshare_epd397_rust_app::atlas_client::{
    AtlasClient, AtlasClientError, CaptureTextRequest, MockAtlasTransport, MockTransportOutcome,
    TransportRequest,
};

const NOTES: &[u8] = br#"{"items":[],"nextCursor":null}"#;
const NOTE: &[u8] =
    br#"{"id":"note-1","title":"One","revision":"r1","body":"body","parentId":null}"#;
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
        client.capture_text(&request, "capture-001"),
        Err(AtlasClientError::Offline)
    );
    client.capture_text(&request, "capture-001").unwrap();

    assert_eq!(
        client.transport().requests(),
        &[
            TransportRequest::CaptureText {
                request: request.clone(),
                idempotency_key: "capture-001".into(),
            },
            TransportRequest::CaptureText {
                request,
                idempotency_key: "capture-001".into(),
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
        client.capture_text(&request, "capture-oversized"),
        Err(AtlasClientError::ResponseTooLarge)
    );
    assert_eq!(
        client.transport().requests(),
        &[TransportRequest::CaptureText {
            request,
            idempotency_key: "capture-oversized".into(),
        }]
    );
}

#[test]
fn client_routes_every_read_operation_through_the_transport() {
    let mut transport = MockAtlasTransport::default();
    for body in [NOTE, SEARCH, VIEWS, VIEW_RESULTS] {
        transport.push_outcome(MockTransportOutcome::response(200, body));
    }
    let mut client = AtlasClient::new(transport);

    assert_eq!(client.get_note("note-1").unwrap().title, "One");
    assert!(client.search("term", 1, 0).unwrap().hits.is_empty());
    assert!(client.list_views().unwrap().items.is_empty());
    assert!(client
        .get_view_results("view-1", Some("cursor-2"), 1)
        .unwrap()
        .items
        .is_empty());

    assert_eq!(
        client.transport().requests(),
        &[
            TransportRequest::GetNote {
                id: "note-1".into(),
            },
            TransportRequest::Search {
                query: "term".into(),
                limit: 1,
                offset: 0,
            },
            TransportRequest::ListViews,
            TransportRequest::GetViewResults {
                id: "view-1".into(),
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
            AtlasClientError::Unavailable(error) => {
                assert_eq!(error.code, "ATLAS_INDEX_NOT_READY")
            }
            AtlasClientError::Timeout
            | AtlasClientError::Offline
            | AtlasClientError::MalformedPayload
            | AtlasClientError::ResponseTooLarge => {}
            other => panic!("unexpected mock failure: {other:?}"),
        }
    }
}
