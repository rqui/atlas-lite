use waveshare_epd397_rust_app::atlas_config::{
    AtlasConfig, AtlasUrlSecurity, ConfigField, ConfigRepository, ConfigStatus, ConfigStore,
    ConfigStoreError, FakeConfigStore,
};

const TOKEN: &str = "at_v1.AAAAAAAAAAAAAAAAAAAAAA.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

fn configured() -> AtlasConfig {
    AtlasConfig::new(
        "atlas-lite-01",
        "https://atlas.example.test/",
        "at_v1.AAAAAAAAAAAAAAAAAAAAAA.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
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
    store.insert_raw("version", b"1").unwrap();
    store.insert_raw("device_id", b"atlas-lite-01").unwrap();
    store
        .insert_raw("atlas_url", b"https://atlas.example.test")
        .unwrap();
    store.insert_raw("wifi_ssid", b"Atlas WiFi").unwrap();
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
    let display = format!("{config}");
    assert!(debug.contains("<redacted>"));
    assert!(display.contains("<redacted>"));
    assert!(!debug.contains("example-token"));
    assert!(!debug.contains("correct-horse"));
    assert!(!display.contains("example-token"));
    assert!(!display.contains("correct-horse"));
}

#[test]
fn invalid_atlas_urls_are_rejected() {
    for value in [
        "atlas.example.test",
        "http://atlas.example.test",
        "ftp://atlas.example.test",
        "https://user@atlas.example.test",
        "https://atlas.example.test/path?query=value",
        "https://atlas.example.test/#fragment",
    ] {
        assert!(AtlasConfig::new("device", value, "token", "wifi", "password").is_err());
    }
}

#[test]
fn private_rfc1918_ipv4_http_is_the_only_http_exception() {
    for value in [
        "http://192.168.10.10:3333",
        "http://192.168.1.20",
        "http://10.0.0.5:3333",
        "http://172.16.0.10",
        "http://172.31.255.254:8080",
    ] {
        let config = AtlasConfig::new("device", value, TOKEN, "wifi", "password").unwrap();
        assert_eq!(config.atlas_url(), value);
        assert_eq!(
            config.atlas_url_security(),
            AtlasUrlSecurity::PrivateLanHttp
        );
    }
}

#[test]
fn http_rejects_public_and_nonliteral_destinations() {
    for value in [
        "http://8.8.8.8",
        "http://1.1.1.1",
        "http://172.15.0.1",
        "http://172.32.0.1",
        "http://169.254.1.1",
        "http://127.0.0.1",
        "http://localhost",
        "http://example.com",
        "http://atlas.local",
        "http://192.168.1.20:0",
        "http://192.168.1.20:99999",
        "http://192.168.01.20",
    ] {
        assert!(AtlasConfig::new("device", value, "token", "wifi", "password").is_err());
    }
}

#[test]
fn https_remains_allowed_and_base_url_restrictions_apply_to_both_schemes() {
    let https = AtlasConfig::new(
        "device",
        "https://atlas.example.test:4443/",
        TOKEN,
        "wifi",
        "password",
    )
    .unwrap();
    assert_eq!(https.atlas_url(), "https://atlas.example.test:4443");
    assert_eq!(https.atlas_url_security(), AtlasUrlSecurity::Https);

    for value in [
        "http://192.168.10.10:3333/path",
        "http://192.168.10.10:3333?query=value",
        "http://192.168.10.10:3333#fragment",
        "http://user@192.168.10.10:3333",
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

    repository.store_mut().insert_raw("version", b"1").unwrap();
    repository
        .store_mut()
        .insert_raw("atlas_url", &[0xff])
        .unwrap();
    assert!(repository.load().is_err());
}

#[test]
fn fake_store_rejects_unknown_keys_and_oversized_values() {
    let mut store = FakeConfigStore::default();
    assert_eq!(
        store.set("unexpected", b"value"),
        Err(ConfigStoreError::UnknownKey {
            key: "unexpected".into()
        })
    );
    assert_eq!(
        store.set("wifi_ssid", &[b'x'; 33]),
        Err(ConfigStoreError::ValueTooLarge {
            key: "wifi_ssid".into(),
            length: 33
        })
    );
    assert_eq!(
        store.set("api_token", &[b'x'; 513]),
        Err(ConfigStoreError::ValueTooLarge {
            key: "api_token".into(),
            length: 513
        })
    );
}
