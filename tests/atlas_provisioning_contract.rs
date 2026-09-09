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

    assert!(source.contains("pub const CONFIG_STORE_KEYS: [&str; 7]"));
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
fn boot_recovery_and_unpair_are_fail_closed_product_paths() {
    let main = include_str!("../src/main.rs");
    let https = include_str!("../src/atlas_https.rs");

    assert!(main.contains("atlas-lite=boot-recovery action=clear-local-config"));
    assert!(main.contains("Unpair needs Atlas connection"));
    assert!(main.find(".revoke_pairing()").unwrap() < main.find(".unpair()?").unwrap());
    assert!(https.contains("/api/v1/pairing/current"));
    assert!(https.contains("204 | 401 => Ok(())"));
}

#[test]
fn ota_slot_is_confirmed_only_at_the_main_loop_checkpoint() {
    let main = include_str!("../src/main.rs");
    let checkpoint = main.find("checkpoint=main-loop-ready").unwrap();
    let pairing = main.find("if atlas_config.is_none()").unwrap();
    let loop_start = main[checkpoint..].find("loop {").unwrap() + checkpoint;

    assert!(pairing < checkpoint);
    assert!(checkpoint < loop_start);
    assert!(!main.contains("checkpoint=drivers-and-display-ready"));
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
