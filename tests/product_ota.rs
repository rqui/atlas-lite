use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use waveshare_epd397_rust_app::product_ota::{artifact_integrity, verify_manifest, OtaError};

fn manifest(artifact: &[u8], version: &str, origin: &str) -> (Vec<u8>, String) {
    let signing = SigningKey::from_bytes(&[7_u8; 32]);
    let hash = format!("{:x}", Sha256::digest(artifact));
    let url = format!("{origin}/atlas-lite/{version}/atlas-lite.bin");
    let payload = format!(
        "atlas-lite-ota-v1\n{version}\nbuild-1\n{url}\n{}\n{hash}\n",
        artifact.len()
    );
    let signature = URL_SAFE_NO_PAD.encode(signing.sign(payload.as_bytes()).to_bytes());
    let json = format!(
        r#"{{"version":"{version}","build":"build-1","artifactUrl":"{url}","size":{},"sha256":"{hash}","signature":"{signature}"}}"#,
        artifact.len()
    );
    let public_key = signing
        .verifying_key()
        .to_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    (json.into_bytes(), public_key)
}

#[test]
fn verifies_fixed_origin_signature_version_size_and_artifact_hash() {
    let artifact = b"esp32 application image";
    let (bytes, public_key) = manifest(artifact, "1.1.0", "https://updates.example");
    let update = verify_manifest(&bytes, "1.0.0", "https://updates.example", &public_key).unwrap();
    assert_eq!(update.version(), "1.1.0");
    assert_eq!(artifact_integrity(&update, artifact), Ok(()));
    assert_eq!(
        artifact_integrity(&update, b"modified"),
        Err(OtaError::Size)
    );
}

#[test]
fn rejects_tampering_downgrade_arbitrary_origin_and_unconfigured_key() {
    let (bytes, public_key) = manifest(b"firmware", "1.1.0", "https://updates.example");
    let bytes = String::from_utf8(bytes)
        .unwrap()
        .replace("build-1", "build-2")
        .into_bytes();
    assert_eq!(
        verify_manifest(&bytes, "1.0.0", "https://updates.example", &public_key),
        Err(OtaError::InvalidSignature)
    );
    let (bytes, public_key) = manifest(b"firmware", "1.0.0", "https://updates.example");
    assert!(verify_manifest(&bytes, "1.0.0", "https://updates.example", &public_key).is_err());
    assert!(verify_manifest(&bytes, "0.9.0", "https://other.example", &public_key).is_err());
    assert!(verify_manifest(&bytes, "0.9.0", "https://updates.example", "").is_err());
}
