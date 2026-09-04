use waveshare_epd397_rust_app::{
    atlas_config::{ConfigField, ConfigRepository, ConfigStatus, FakeConfigStore},
    product_provisioning::{
        extract_setup_session_cookie, render_setup_page, PortalError, PortalSession,
        ProvisioningScreenData, ProvisioningSubmission, SetupCredentials, SETUP_AP_PASSWORD_BYTES,
        SETUP_PAGE_MAX_BYTES, SETUP_SESSION_LIFETIME_MS,
    },
};

#[test]
fn portal_accepts_only_bounded_wifi_and_atlas_fields_with_session_proof() {
    let mut session = PortalSession::new("session-proof", "csrf-proof", 1_000).unwrap();
    let submission = session
        .parse_submission(
            b"ssid=Atlas+WiFi&password=correct-horse&atlas_url=https%3A%2F%2Fatlas.local&csrf=csrf-proof",
            Some("session-proof"),
            1_001,
        )
        .unwrap();

    assert_eq!(submission.ssid(), "Atlas WiFi");
    assert_eq!(submission.password(), "correct-horse");
    assert_eq!(submission.atlas_url(), "https://atlas.local");
    assert!(!format!("{submission:?}").contains("correct-horse"));
}

#[test]
fn portal_rejects_csrf_replay_expiry_duplicates_unknown_and_oversized_bodies() {
    let valid = b"ssid=Atlas&password=&atlas_url=https%3A%2F%2Fatlas.local&csrf=csrfproof";

    let mut wrong_cookie = PortalSession::new("sessionproof", "csrfproof", 10).unwrap();
    assert_eq!(
        wrong_cookie.parse_submission(valid, Some("otherproof"), 11),
        Err(PortalError::PossessionRequired)
    );

    let mut expired = PortalSession::new("sessionproof", "csrfproof", 10).unwrap();
    assert_eq!(
        expired.parse_submission(
            valid,
            Some("sessionproof"),
            10 + SETUP_SESSION_LIFETIME_MS + 1,
        ),
        Err(PortalError::Expired)
    );

    let mut duplicate = PortalSession::new("sessionproof", "csrfproof", 10).unwrap();
    assert_eq!(
        duplicate.parse_submission(
            b"ssid=A&ssid=B&password=&atlas_url=https%3A%2F%2Fatlas.local&csrf=csrfproof",
            Some("sessionproof"),
            11,
        ),
        Err(PortalError::Malformed)
    );

    let mut unknown = PortalSession::new("sessionproof", "csrfproof", 10).unwrap();
    assert_eq!(
        unknown.parse_submission(
            b"ssid=A&password=&atlas_url=https%3A%2F%2Fatlas.local&csrf=csrfproof&token=nope",
            Some("sessionproof"),
            11,
        ),
        Err(PortalError::Malformed)
    );

    let mut oversized = PortalSession::new("sessionproof", "csrfproof", 10).unwrap();
    assert_eq!(
        oversized.parse_submission(&vec![b'x'; 513], Some("sessionproof"), 11),
        Err(PortalError::BodyTooLarge)
    );
}

#[test]
fn provisioning_persists_only_wifi_and_atlas_url_before_pairing() {
    let mut repository = ConfigRepository::new(FakeConfigStore::default());
    let submission =
        ProvisioningSubmission::new("Atlas", "password", "https://atlas.local").unwrap();

    repository
        .save_provisioning("device-01", &submission)
        .unwrap();
    let loaded = repository.load().unwrap();
    assert_eq!(
        loaded.status(),
        ConfigStatus::Partial(vec![ConfigField::ApiToken])
    );
    let provisioning = repository.load_provisioning().unwrap().unwrap();
    assert_eq!(provisioning.device_id(), "device-01");
    assert_eq!(provisioning.atlas_url(), "https://atlas.local");
    assert_eq!(provisioning.wifi_ssid(), "Atlas");

    repository.unpair().unwrap();
    assert!(repository.load_provisioning().unwrap().is_some());
    repository.reset_wifi().unwrap();
    assert!(repository.load_provisioning().unwrap().is_none());
}

#[test]
fn portal_consumes_the_session_only_after_nvs_persistence_is_confirmed() {
    let body = b"ssid=Atlas&password=&atlas_url=https%3A%2F%2Fatlas.local&csrf=csrfproof";
    let mut session = PortalSession::new("sessionproof", "csrfproof", 10).unwrap();

    assert!(session
        .parse_submission(body, Some("sessionproof"), 11)
        .is_ok());
    assert!(session
        .parse_submission(body, Some("sessionproof"), 12)
        .is_ok());

    session.confirm_persisted();
    assert_eq!(
        session.parse_submission(body, Some("sessionproof"), 13),
        Err(PortalError::AlreadyCompleted)
    );
}

#[test]
fn setup_credentials_are_bounded_deterministic_and_secret_redacted() {
    let entropy = [0x5a; 64];
    let credentials = SetupCredentials::from_entropy(&entropy).unwrap();

    assert!(credentials.ssid().starts_with("Atlas-Lite-"));
    assert_eq!(credentials.ap_password().len(), SETUP_AP_PASSWORD_BYTES);
    assert!(credentials.session_proof().len() >= 32);
    assert!(credentials.csrf_proof().len() >= 32);
    let debug = format!("{credentials:?}");
    assert!(!debug.contains(credentials.ap_password()));
    assert!(!debug.contains(credentials.session_proof()));
    assert!(!debug.contains(credentials.csrf_proof()));
}

#[test]
fn provisioning_rejects_control_characters_after_form_decoding() {
    for body in [
        b"ssid=Atlas%00WiFi&password=valid&atlas_url=https%3A%2F%2Fatlas.local&csrf=csrfproof"
            .as_slice(),
        b"ssid=Atlas&password=secret%0Avalue&atlas_url=https%3A%2F%2Fatlas.local&csrf=csrfproof"
            .as_slice(),
    ] {
        let mut session = PortalSession::new("sessionproof", "csrfproof", 10).unwrap();
        assert_eq!(
            session.parse_submission(body, Some("sessionproof"), 11),
            Err(PortalError::InvalidValue)
        );
    }
}

#[test]
fn setup_page_exposes_only_the_three_product_fields_and_bounded_csrf() {
    let page = render_setup_page("csrf-proof").unwrap();
    assert!(page.len() <= SETUP_PAGE_MAX_BYTES);
    for field in ["ssid", "password", "atlas_url", "csrf"] {
        assert!(page.contains(&format!("name=\"{field}\"")));
    }
    assert!(!page.to_ascii_lowercase().contains("api token"));
    assert!(!page.contains("api_token"));
}

#[test]
fn setup_cookie_parser_requires_one_exact_ram_session_cookie() {
    assert_eq!(
        extract_setup_session_cookie("theme=dark; atlas_setup=sessionproof; x=1"),
        Some("sessionproof")
    );
    assert_eq!(extract_setup_session_cookie("atlas_setup="), None);
    assert_eq!(
        extract_setup_session_cookie("atlas_setup=one; atlas_setup=two"),
        None
    );
    assert_eq!(extract_setup_session_cookie("not_atlas_setup=value"), None);
}

#[test]
fn device_identity_is_persisted_once_and_survives_reprovisioning() {
    let mut repository = ConfigRepository::new(FakeConfigStore::default());
    assert_eq!(
        repository.ensure_device_id("device-first").unwrap(),
        "device-first"
    );
    assert_eq!(
        repository.ensure_device_id("device-second").unwrap(),
        "device-first"
    );
    repository.reset_wifi().unwrap();
    assert_eq!(
        repository.load_device_id().unwrap().as_deref(),
        Some("device-first")
    );
    repository.clear().unwrap();
    assert_eq!(repository.load_device_id().unwrap(), None);
}

#[test]
fn provisioning_screen_data_displays_local_possession_without_debug_leakage() {
    let data = ProvisioningScreenData::new(
        "Atlas-Lite-ABC123",
        "secretpass12",
        "http://192.168.71.1/",
        "atlas-lite-device",
    )
    .unwrap();
    assert_eq!(data.ap_password(), "secretpass12");
    assert!(data.url().starts_with("http://192.168."));
    let debug = format!("{data:?}");
    assert!(!debug.contains("secretpass12"));
    assert!(debug.contains("Atlas-Lite-ABC123"));
}
