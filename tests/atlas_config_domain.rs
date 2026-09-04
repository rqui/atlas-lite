use waveshare_epd397_rust_app::atlas_config::{
    AtlasConfig, ConfigField, ConfigRepository, ConfigStatus, FakeConfigStore,
};

fn configured() -> AtlasConfig {
    AtlasConfig::new(
        "atlas-lite-01",
        "https://atlas.example.test/",
        "at_v1.example-token",
        "Atlas WiFi",
        "correct-horse-battery-staple",
    )
    .expect("valid fixture configuration")
}

#[test]
fn missing_values_are_explicitly_unconfigured() {
    let store = FakeConfigStore::default();
    let repository = ConfigRepository::new(store);

    assert_eq!(
        repository.load().unwrap().status(),
        ConfigStatus::Unconfigured
    );
}

#[test]
fn a_missing_secret_is_partial_without_exposing_it() {
    let mut store = FakeConfigStore::default();
    store.insert_raw("version", b"1");
    store.insert_raw("device_id", b"atlas-lite-01");
    store.insert_raw("atlas_url", b"https://atlas.example.test");
    store.insert_raw("wifi_ssid", b"Atlas WiFi");
    let repository = ConfigRepository::new(store);

    assert_eq!(
        repository.load().unwrap().status(),
        ConfigStatus::Partial(vec![ConfigField::ApiToken, ConfigField::WifiCredentials])
    );
}

#[test]
fn ready_config_redacts_debug_and_normalizes_the_base_url() {
    let config = configured();

    assert_eq!(config.atlas_url(), "https://atlas.example.test");
    let debug = format!("{config:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("example-token"));
    assert!(!debug.contains("correct-horse"));
}

#[test]
fn invalid_atlas_urls_are_rejected() {
    for value in [
        "atlas.example.test",
        "ftp://atlas.example.test",
        "https://user@atlas.example.test",
        "https://atlas.example.test/path?query=value",
        "https://atlas.example.test/#fragment",
    ] {
        assert!(AtlasConfig::new("device", value, "token", "wifi", "password").is_err());
    }
}

#[test]
fn fake_store_supports_save_update_clear_and_corrupt_data() {
    let mut repository = ConfigRepository::new(FakeConfigStore::default());
    repository.save(&configured()).unwrap();
    assert!(repository.load().unwrap().is_ready());

    repository
        .update_wifi("Other WiFi", "another-password")
        .unwrap();
    assert_eq!(
        repository.load().unwrap().config().unwrap().wifi_ssid(),
        "Other WiFi"
    );

    repository.clear().unwrap();
    assert_eq!(
        repository.load().unwrap().status(),
        ConfigStatus::Unconfigured
    );

    repository.store_mut().insert_raw("version", b"1");
    repository.store_mut().insert_raw("atlas_url", &[0xff]);
    assert!(repository.load().is_err());
}
