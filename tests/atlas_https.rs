use std::fs;
use waveshare_epd397_rust_app::{
    atlas_client::{CaptureTextRequest, TransportError, TransportRequest, MAX_CAPTURE_TEXT_BYTES},
    atlas_config::AtlasConfig,
    atlas_https::{
        classify_transport_status, prepare_request, retry_safe_read, AtlasTransportStatus,
        ATLAS_READ_ATTEMPT_LIMIT,
    },
};

fn config() -> AtlasConfig {
    AtlasConfig::new(
        "atlas-lite-01",
        "https://atlas.example.test",
        "at_v1_secret_token_value",
        "Atlas Lite",
        "not-a-real-password",
    )
    .unwrap()
}

#[test]
fn prepared_headers_include_auth_and_mutation_key_without_debug_leakage() {
    let request = TransportRequest::CaptureText {
        request: CaptureTextRequest::new("remember this").unwrap(),
        idempotency_key: "capture-001".into(),
    };

    let prepared = prepare_request(&config(), &request).unwrap();

    assert_eq!(
        prepared.header("authorization"),
        Some("Bearer at_v1_secret_token_value")
    );
    assert_eq!(prepared.header("idempotency-key"), Some("capture-001"));
    assert_eq!(prepared.body_len(), br#"{"text":"remember this"}"#.len());
    assert_eq!(
        prepared.url(),
        "https://atlas.example.test/api/v1/capture/text"
    );
    let debug = format!("{prepared:?}");
    assert!(!debug.contains("at_v1_secret_token_value"));
    assert!(!debug.contains("capture-001"));
    assert!(!debug.contains("https://atlas.example.test"));
}

#[test]
fn normal_capture_keeps_json_body_bounded_and_has_mutation_headers() {
    let request = TransportRequest::CaptureText {
        request: CaptureTextRequest::new("remember this").unwrap(),
        idempotency_key: "capture-001".into(),
    };

    let prepared = prepare_request(&config(), &request).unwrap();

    assert_eq!(prepared.header("content-type"), Some("application/json"));
    assert_eq!(prepared.header("idempotency-key"), Some("capture-001"));
    assert_eq!(prepared.body_len(), 24);
}

#[test]
fn large_capture_returns_request_too_large_before_transport_can_send_it() {
    let request = TransportRequest::CaptureText {
        request: CaptureTextRequest::new("\\".repeat(MAX_CAPTURE_TEXT_BYTES)).unwrap(),
        idempotency_key: "capture-001".into(),
    };

    assert!(matches!(
        prepare_request(&config(), &request),
        Err(waveshare_epd397_rust_app::atlas_https::AtlasHttpsError::RequestTooLarge)
    ));
}

#[test]
fn capture_text_request_is_bounded_before_transport_serialization() {
    assert!(CaptureTextRequest::new("x".repeat(MAX_CAPTURE_TEXT_BYTES + 1)).is_err());
}

#[test]
fn source_contract_requires_heap_response_and_bounded_streaming_capture_serialization() {
    let source = fs::read_to_string("src/atlas_https.rs").unwrap();
    assert!(!source.contains("[0_u8;"));
    assert!(!source.contains("[0_u8; ATLAS_HTTP_RESPONSE_BODY_BYTES]"));
    assert!(source.contains("Vec::with_capacity(ATLAS_HTTP_RESPONSE_BODY_BYTES)"));
    assert!(source.contains("Vec::with_capacity(limit)"));
    assert!(source.contains("BoundedJsonWriter"));
    assert!(!source.contains("serde_json::json!"));
}

#[test]
fn typed_requests_redact_query_capture_and_idempotency_values_in_debug() {
    let request = TransportRequest::CaptureText {
        request: CaptureTextRequest::new("do not log this text").unwrap(),
        idempotency_key: "capture-secret-key".into(),
    };
    let debug = format!("{request:?}");
    assert!(!debug.contains("do not log this text"));
    assert!(!debug.contains("capture-secret-key"));
}

#[test]
fn prepared_read_urls_percent_encode_path_and_query_values() {
    let note =
        prepare_request(&config(), &TransportRequest::GetNote { id: "a/b ?".into() }).unwrap();
    assert_eq!(
        note.url(),
        "https://atlas.example.test/api/v1/notes/by-id/a%2Fb%20%3F"
    );

    let search = prepare_request(
        &config(),
        &TransportRequest::Search {
            query: "tea & cake?".into(),
            limit: 7,
            offset: 2,
        },
    )
    .unwrap();
    assert_eq!(
        search.url(),
        "https://atlas.example.test/api/v1/search?q=tea%20%26%20cake%3F&limit=7&offset=2"
    );
}

#[test]
fn adapter_rejects_non_https_base_urls_before_request_construction() {
    let insecure = AtlasConfig::new(
        "atlas-lite-01",
        "http://atlas.example.test",
        "at_v1_secret_token_value",
        "Atlas Lite",
        "not-a-real-password",
    )
    .unwrap();

    assert!(prepare_request(&insecure, &TransportRequest::ListViews).is_err());
}

#[test]
fn status_and_transport_errors_have_deterministic_diagnostics_classification() {
    assert_eq!(
        classify_transport_status(401),
        AtlasTransportStatus::Unauthorized
    );
    assert_eq!(
        classify_transport_status(403),
        AtlasTransportStatus::Forbidden
    );
    assert_eq!(
        classify_transport_status(408),
        AtlasTransportStatus::Timeout
    );
    assert_eq!(
        classify_transport_status(500),
        AtlasTransportStatus::ServerError
    );
    assert_eq!(
        classify_transport_status(503),
        AtlasTransportStatus::ServerError
    );
    assert_eq!(
        AtlasTransportStatus::from_transport_error(TransportError::Offline),
        AtlasTransportStatus::Offline
    );
}

#[test]
fn safe_reads_retry_at_the_fixed_bound_but_mutations_do_not() {
    let mut read_attempts = 0;
    let read: Result<(), TransportError> = retry_safe_read(&TransportRequest::ListViews, || {
        read_attempts += 1;
        Err(TransportError::Timeout)
    });
    assert_eq!(read, Err(TransportError::Timeout));
    assert_eq!(read_attempts, ATLAS_READ_ATTEMPT_LIMIT);

    let capture = TransportRequest::CaptureText {
        request: CaptureTextRequest::new("remember this").unwrap(),
        idempotency_key: "capture-001".into(),
    };
    let mut mutation_attempts = 0;
    let mutation: Result<(), TransportError> = retry_safe_read(&capture, || {
        mutation_attempts += 1;
        Err(TransportError::Offline)
    });
    assert_eq!(mutation, Err(TransportError::Offline));
    assert_eq!(mutation_attempts, 1);
}
