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
