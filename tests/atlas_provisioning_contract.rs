#[test]
fn provisioning_helper_reads_secrets_without_echoing_or_persisting_to_sd() {
    let script = include_str!("../scripts/provision-atlas-lite.sh");

    assert!(script.contains("read -r -s ATLAS_TOKEN"));
    assert!(script.contains("read -r -s WIFI_CREDENTIALS"));
    assert!(script.contains("physical-write=pending"));
    assert!(!script.contains("/sdcard"));
    assert!(!script.contains("echo \"$ATLAS_TOKEN\""));
    assert!(!script.contains("echo \"$WIFI_CREDENTIALS\""));
}

#[test]
fn configuration_source_has_no_secret_logging_or_generic_serialization() {
    let source = include_str!("../src/atlas_config.rs");

    assert!(!source.contains("log::"));
    assert!(!source.contains("#[derive(Serialize"));
    assert!(!source.contains("/sdcard"));
}

#[test]
fn target_store_uses_the_same_bounded_key_and_value_domain() {
    let source = include_str!("../src/atlas_config.rs");

    assert!(source.contains("pub const CONFIG_STORE_KEYS: [&str; 6]"));
    assert!(source.contains("pub const MAX_CONFIG_ENTRIES: usize = CONFIG_STORE_KEYS.len()"));
    assert!(source.contains("super::validate_store_key(key)?"));
    assert!(source.contains("super::store_value_limit(key)?"));
    assert!(source.contains("super::validate_store_key_value(key, value)?"));
}

#[test]
fn firmware_boot_loads_network_configuration_from_nvs_not_plaintext_sd() {
    let main = include_str!("../src/main.rs");

    assert!(main.contains("EspNvsConfigStore"));
    assert!(main.contains("ConfigRepository"));
    assert!(!main.contains("NetworkConfig::load_from_path"));
    assert!(!main.contains("WIFI_CONFIG_PATH"));
}

#[test]
fn sd_setup_does_not_instruct_atlas_lite_to_store_credentials_on_sd() {
    let docs = include_str!("../docs/SD_CARD_SETUP.md");

    assert!(docs.contains("## Atlas Lite Wi-Fi: never use SD credentials"));
    assert!(
        docs.contains("Atlas Lite production firmware does not read or create `/RUSTMIX/WIFI.TXT`")
    );
    assert!(docs.contains("## Legacy Rustmix Wi-Fi bring-up only"));
    assert!(docs.contains("not an Atlas Lite production configuration"));
    assert!(docs.contains("./scripts/provision-atlas-lite.sh"));
    assert!(docs.contains("physical-write=pending"));
    assert!(docs.contains("rm -- /Volumes/YOUR_SD_CARD/RUSTMIX/WIFI.TXT"));
    assert!(!docs.contains("Copy or edit `/RUSTMIX/WIFI.TXT`"));
}
