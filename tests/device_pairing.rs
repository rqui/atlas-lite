use sha2::{Digest, Sha256};
use waveshare_epd397_rust_app::{
    atlas_config::{ConfigRepository, ConfigStatus, FakeConfigStore, MINIMUM_CAPABILITIES},
    device_pairing::{
        pairing_endpoint, pairing_post_headers, parse_poll_response, PairingError,
        PairingStartRetry, PairingStatus, PendingPairing, PAIRING_POLL_INTERVAL_SECONDS,
        PAIRING_START_RATE_LIMIT_RETRY_SECONDS, PAIRING_START_RETRY_INITIAL_SECONDS,
    },
};

fn pending() -> PendingPairing {
    PendingPairing::from_entropy("atlas-lite-device", "Roger's Atlas Lite", &[0x5a; 104]).unwrap()
}

#[test]
fn device_generates_and_persists_stable_key_before_first_request() {
    let pairing = pending();
    let bearer = pairing.bearer();
    assert!(bearer.starts_with("at_v1."));
    let body = String::from_utf8(pairing.start_body().unwrap()).unwrap();
    assert!(body.contains(pairing.request_id()));
    assert!(body.contains(pairing.code()));
    assert!(!body.contains(&bearer));
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(json.get("apiSecret").is_none());
    assert!(json.get("bearer").is_none());
    for scope in MINIMUM_CAPABILITIES {
        assert!(body.contains(scope));
    }

    let mut repository = ConfigRepository::new(FakeConfigStore::default());
    repository.save_pending_pairing(&pairing).unwrap();
    let restored = repository.load_pending_pairing().unwrap().unwrap();
    assert_eq!(restored, pairing);
    assert_eq!(restored.bearer(), bearer);
}

#[test]
fn verifier_matches_the_canonical_at_v1_server_algorithm() {
    let pairing = pending();
    let body: serde_json::Value = serde_json::from_slice(&pairing.start_body().unwrap()).unwrap();
    let bearer = pairing.bearer();
    let secret = base64::Engine::decode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        bearer.rsplit('.').next().unwrap(),
    )
    .unwrap();
    let salt = base64::Engine::decode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        body["secretSalt"].as_str().unwrap(),
    )
    .unwrap();
    let mut hash = Sha256::new();
    hash.update(b"atlas-integration-token-v1\0");
    hash.update(salt);
    hash.update(secret);
    assert_eq!(
        base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            hash.finalize()
        ),
        body["secretVerifier"].as_str().unwrap(),
    );
}

#[test]
fn approval_promotes_existing_bearer_then_removes_pending_state() {
    let pairing = pending();
    let mut repository = ConfigRepository::new(FakeConfigStore::default());
    let provisioning =
        waveshare_epd397_rust_app::product_provisioning::ProvisioningSubmission::new(
            "WiFi",
            "password",
            "https://atlas.test",
        )
        .unwrap();
    repository
        .save_provisioning(pairing.device_id(), &provisioning)
        .unwrap();
    repository.save_pending_pairing(&pairing).unwrap();
    repository.complete_pairing(&pairing).unwrap();
    assert!(repository.load_pending_pairing().unwrap().is_none());
    assert_eq!(repository.load().unwrap().status(), ConfigStatus::Ready);
    assert_eq!(
        repository.load().unwrap().config().unwrap().api_token(),
        pairing.bearer()
    );
}

#[test]
fn pairing_endpoint_uses_the_shared_private_http_url_policy() {
    assert_eq!(
        pairing_endpoint("http://192.168.10.10:3333", "/api/v1/pairing/requests").unwrap(),
        "http://192.168.10.10:3333/api/v1/pairing/requests"
    );
    assert_eq!(
        pairing_endpoint("http://atlas.local", "/api/v1/pairing/requests"),
        Err(PairingError::InvalidValue)
    );
}

#[test]
fn physical_style_poll_is_a_zero_byte_post_without_json_content_type() {
    let pairing = pending();
    let authorization = format!("Pairing {}", pairing.poll_secret());
    let headers = pairing_post_headers(b"", Some(&authorization)).unwrap();

    assert_eq!(
        headers,
        vec![
            ("Accept", "application/json".into()),
            ("Content-Length", "0".into()),
            ("Authorization", authorization),
        ]
    );
    assert!(!headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("content-type")));

    let start_headers = pairing_post_headers(&pairing.start_body().unwrap(), None).unwrap();
    assert!(start_headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("content-type") && value == "application/json"
    }));
}

#[test]
fn start_retry_is_persistently_suppressed_after_accepted_or_compatible_start() {
    let mut pairing = pending();
    let material_before = (
        pairing.request_id().to_string(),
        pairing.poll_secret().to_string(),
        pairing.bearer(),
    );
    assert!(!pairing.start_confirmed());

    let mut retry = PairingStartRetry::new(pairing.start_confirmed());
    assert!(retry.should_start(0));
    pairing.mark_start_confirmed();
    retry.accepted();
    assert!(!retry.should_start(0));

    let mut repository = ConfigRepository::new(FakeConfigStore::default());
    repository.save_pending_pairing(&pairing).unwrap();
    let after_reboot = repository.load_pending_pairing().unwrap().unwrap();
    assert!(after_reboot.start_confirmed());
    assert_eq!(
        material_before,
        (
            after_reboot.request_id().to_string(),
            after_reboot.poll_secret().to_string(),
            after_reboot.bearer(),
        )
    );
    assert!(!PairingStartRetry::new(after_reboot.start_confirmed()).should_start(0));
}

#[test]
fn pre_retry_scheduler_pending_state_remains_usable_after_firmware_upgrade() {
    let pairing = pending();
    let mut legacy: serde_json::Value =
        serde_json::from_slice(&pairing.to_persisted_bytes().unwrap()).unwrap();
    legacy.as_object_mut().unwrap().remove("start_confirmed");
    let legacy = serde_json::to_vec(&legacy).unwrap();

    let restored = PendingPairing::from_persisted_bytes(&legacy).unwrap();
    assert!(!restored.start_confirmed());
    assert_eq!(restored.request_id(), pairing.request_id());
    assert_eq!(restored.poll_secret(), pairing.poll_secret());
    assert_eq!(restored.bearer(), pairing.bearer());
}

#[test]
fn unavailable_and_rate_limited_starts_back_off_while_polling_stays_fixed() {
    let mut retry = PairingStartRetry::new(false);
    assert!(retry.should_start(0));

    retry.unavailable(0);
    assert_eq!(
        retry.next_start_at_seconds(),
        PAIRING_START_RETRY_INITIAL_SECONDS
    );
    assert!(!retry.should_start(PAIRING_START_RETRY_INITIAL_SECONDS - 1));
    assert!(retry.should_start(PAIRING_START_RETRY_INITIAL_SECONDS));

    retry.unavailable(PAIRING_START_RETRY_INITIAL_SECONDS);
    assert_eq!(retry.next_start_at_seconds(), 45);
    retry.rate_limited(45);
    assert_eq!(
        retry.next_start_at_seconds(),
        45 + PAIRING_START_RATE_LIMIT_RETRY_SECONDS
    );
    assert!(!retry.should_start(45 + PAIRING_START_RATE_LIMIT_RETRY_SECONDS - 1));
    assert!(retry.should_start(45 + PAIRING_START_RATE_LIMIT_RETRY_SECONDS));
    assert_eq!(PAIRING_POLL_INTERVAL_SECONDS, 5);
}

#[test]
fn poll_response_is_bounded_bound_to_request_and_strict() {
    let request_id = "0123456789abcdef0123456789abcdef";
    let body = format!(r#"{{"requestId":"{request_id}","status":"approved","expiresAt":123}}"#);
    assert_eq!(
        parse_poll_response(200, body.as_bytes(), request_id),
        Ok(PairingStatus::Approved)
    );
    assert_eq!(
        parse_poll_response(200, body.as_bytes(), "fedcba9876543210fedcba9876543210"),
        Err(PairingError::Malformed)
    );
    assert_eq!(
        parse_poll_response(200, &vec![b'x'; 513], request_id),
        Err(PairingError::Malformed)
    );
    let extra = format!(
        r#"{{"requestId":"{request_id}","status":"approved","expiresAt":123,"bearer":"secret"}}"#
    );
    assert_eq!(
        parse_poll_response(200, extra.as_bytes(), request_id),
        Err(PairingError::Malformed)
    );
    for (status, expected) in [
        ("pending", PairingStatus::Pending),
        ("denied", PairingStatus::Denied),
        ("expired", PairingStatus::Expired),
    ] {
        let response =
            format!(r#"{{"requestId":"{request_id}","status":"{status}","expiresAt":123}}"#);
        assert_eq!(
            parse_poll_response(200, response.as_bytes(), request_id),
            Ok(expected)
        );
    }
}

#[test]
fn pending_debug_and_corrupt_persistence_never_disclose_or_activate_secret() {
    let pairing = pending();
    let debug = format!("{pairing:?}");
    assert!(!debug.contains(pairing.poll_secret()));
    assert!(!debug.contains(pairing.bearer().rsplit('.').next().unwrap()));
    let mut bytes = pairing.to_persisted_bytes().unwrap();
    bytes.truncate(bytes.len() / 2);
    assert_eq!(
        PendingPairing::from_persisted_bytes(&bytes),
        Err(PairingError::Malformed)
    );
}
