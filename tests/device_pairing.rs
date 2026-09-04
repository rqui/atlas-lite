use sha2::{Digest, Sha256};
use waveshare_epd397_rust_app::{
    atlas_config::{ConfigRepository, ConfigStatus, FakeConfigStore, MINIMUM_CAPABILITIES},
    device_pairing::{parse_poll_response, PairingError, PairingStatus, PendingPairing},
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
