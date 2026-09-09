//! Signed, fixed-origin OTA manifest and A/B update boundary.

use core::fmt;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub const OTA_MANIFEST_MAX_BYTES: usize = 2 * 1024;
pub const OTA_ARTIFACT_MAX_BYTES: u64 = 6 * 1024 * 1024;
pub const OTA_MANIFEST_PATH: &str = "/atlas-lite/stable/manifest.json";
pub const OTA_TRUSTED_ORIGIN: Option<&str> = option_env!("ATLAS_LITE_OTA_ORIGIN");
pub const OTA_PUBLIC_KEY_HEX: Option<&str> = option_env!("ATLAS_LITE_OTA_PUBLIC_KEY_HEX");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OtaError {
    Unconfigured,
    InvalidManifest,
    UntrustedSource,
    InvalidSignature,
    NotNewer,
    Size,
    Integrity,
    Io,
}

#[derive(Clone, Eq, PartialEq)]
pub struct VerifiedUpdate {
    version: String,
    build: String,
    artifact_url: String,
    size: u64,
    sha256: [u8; 32],
}

impl fmt::Debug for VerifiedUpdate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedUpdate")
            .field("version", &self.version)
            .field("build", &self.build)
            .field("size", &self.size)
            .finish()
    }
}

impl VerifiedUpdate {
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }
    #[must_use]
    pub fn artifact_url(&self) -> &str {
        &self.artifact_url
    }
    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }
    #[must_use]
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Manifest {
    version: String,
    build: String,
    artifact_url: String,
    size: u64,
    sha256: String,
    signature: String,
}

pub fn verify_manifest(
    bytes: &[u8],
    current_version: &str,
    trusted_origin: &str,
    public_key_hex: &str,
) -> Result<VerifiedUpdate, OtaError> {
    if bytes.len() > OTA_MANIFEST_MAX_BYTES {
        return Err(OtaError::Size);
    }
    if !trusted_origin.starts_with("https://") || trusted_origin.ends_with('/') {
        return Err(OtaError::Unconfigured);
    }
    let manifest: Manifest =
        serde_json::from_slice(bytes).map_err(|_| OtaError::InvalidManifest)?;
    if manifest.version.len() > 32
        || manifest.build.len() > 64
        || manifest.artifact_url.len() > 256
        || !manifest
            .artifact_url
            .starts_with(&format!("{trusted_origin}/atlas-lite/"))
        || manifest.size == 0
        || manifest.size > OTA_ARTIFACT_MAX_BYTES
        || !newer(&manifest.version, current_version)
    {
        return Err(if !manifest.artifact_url.starts_with(trusted_origin) {
            OtaError::UntrustedSource
        } else {
            OtaError::InvalidManifest
        });
    }
    let sha256 = decode_hex_32(&manifest.sha256).ok_or(OtaError::InvalidManifest)?;
    let public_key = decode_hex_32(public_key_hex).ok_or(OtaError::Unconfigured)?;
    let signature_bytes = URL_SAFE_NO_PAD
        .decode(&manifest.signature)
        .map_err(|_| OtaError::InvalidSignature)?;
    let signature =
        Signature::from_slice(&signature_bytes).map_err(|_| OtaError::InvalidSignature)?;
    let key = VerifyingKey::from_bytes(&public_key).map_err(|_| OtaError::Unconfigured)?;
    let signed = canonical_payload(&manifest);
    key.verify(signed.as_bytes(), &signature)
        .map_err(|_| OtaError::InvalidSignature)?;
    Ok(VerifiedUpdate {
        version: manifest.version,
        build: manifest.build,
        artifact_url: manifest.artifact_url,
        size: manifest.size,
        sha256,
    })
}

pub fn artifact_integrity(update: &VerifiedUpdate, bytes: &[u8]) -> Result<(), OtaError> {
    if bytes.len() as u64 != update.size {
        return Err(OtaError::Size);
    }
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    if digest != update.sha256 {
        return Err(OtaError::Integrity);
    }
    Ok(())
}

fn canonical_payload(manifest: &Manifest) -> String {
    format!(
        "atlas-lite-ota-v1\n{}\n{}\n{}\n{}\n{}\n",
        manifest.version, manifest.build, manifest.artifact_url, manifest.size, manifest.sha256
    )
}

fn newer(candidate: &str, current: &str) -> bool {
    fn parts(value: &str) -> Option<[u32; 3]> {
        let mut items = value.split('.');
        let result = [
            items.next()?.parse().ok()?,
            items.next()?.parse().ok()?,
            items.next()?.parse().ok()?,
        ];
        (items.next().is_none()).then_some(result)
    }
    matches!((parts(candidate), parts(current)), (Some(candidate), Some(current)) if candidate > current)
}

fn decode_hex_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut output = [0_u8; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let text = core::str::from_utf8(chunk).ok()?;
        output[index] = u8::from_str_radix(text, 16).ok()?;
    }
    Some(output)
}

#[cfg(target_os = "espidf")]
pub mod espidf {
    use super::{
        verify_manifest, OtaError, VerifiedUpdate, OTA_MANIFEST_MAX_BYTES, OTA_MANIFEST_PATH,
        OTA_PUBLIC_KEY_HEX, OTA_TRUSTED_ORIGIN,
    };
    use embedded_svc::{
        http::{client::Client as HttpClient, Method},
        io::Write as _,
    };
    use esp_idf_svc::{
        http::client::{Configuration, EspHttpConnection},
        ota::EspOta,
        sys,
    };
    use sha2::{Digest, Sha256};
    use std::time::Duration;

    pub fn install_verified<R: embedded_svc::io::Read>(
        update: &VerifiedUpdate,
        reader: &mut R,
    ) -> Result<(), OtaError> {
        let mut ota = EspOta::new().map_err(|_| OtaError::Io)?;
        let mut slot = ota
            .initiate_update_with_known_size(update.size() as usize)
            .map_err(|_| OtaError::Io)?;
        let mut remaining = update.size();
        let mut hash = Sha256::new();
        let mut chunk = [0_u8; 4096];
        while remaining > 0 {
            let limit = remaining.min(chunk.len() as u64) as usize;
            let read = reader.read(&mut chunk[..limit]).map_err(|_| OtaError::Io)?;
            if read == 0 {
                return Err(OtaError::Size);
            }
            slot.write_all(&chunk[..read]).map_err(|_| OtaError::Io)?;
            hash.update(&chunk[..read]);
            remaining -= read as u64;
        }
        let digest: [u8; 32] = hash.finalize().into();
        if &digest != update.sha256() {
            return Err(OtaError::Integrity);
        }
        slot.complete().map_err(|_| OtaError::Io)
    }

    pub fn mark_running_image_valid() -> Result<(), OtaError> {
        EspOta::new()
            .and_then(|mut ota| ota.mark_running_slot_valid())
            .map_err(|_| OtaError::Io)
    }

    /// Fetch a signed manifest from the compile-time fixed origin and stream
    /// its verified artifact directly into the inactive OTA slot.
    pub fn fetch_and_install(current_version: &str) -> Result<String, OtaError> {
        let origin = OTA_TRUSTED_ORIGIN.ok_or(OtaError::Unconfigured)?;
        let public_key = OTA_PUBLIC_KEY_HEX.ok_or(OtaError::Unconfigured)?;
        let config = Configuration {
            crt_bundle_attach: Some(sys::esp_crt_bundle_attach),
            timeout: Some(Duration::from_secs(20)),
            buffer_size: Some(4096),
            keep_alive_enable: false,
            ..Default::default()
        };
        let mut client =
            HttpClient::wrap(EspHttpConnection::new(&config).map_err(|_| OtaError::Io)?);
        let manifest_url = format!("{origin}{OTA_MANIFEST_PATH}");
        let request = client
            .request(
                Method::Get,
                &manifest_url,
                &[("Accept", "application/json")],
            )
            .map_err(|_| OtaError::Io)?;
        let mut response = request.submit().map_err(|_| OtaError::Io)?;
        if response.status() != 200 {
            return Err(OtaError::Io);
        }
        let mut manifest = Vec::with_capacity(OTA_MANIFEST_MAX_BYTES);
        let mut chunk = [0_u8; 256];
        loop {
            let read = response.read(&mut chunk).map_err(|_| OtaError::Io)?;
            if read == 0 {
                break;
            }
            if manifest.len() + read > OTA_MANIFEST_MAX_BYTES {
                return Err(OtaError::Size);
            }
            manifest.extend_from_slice(&chunk[..read]);
        }
        drop(response);
        let update = verify_manifest(&manifest, current_version, origin, public_key)?;
        let version = update.version().to_owned();
        let request = client
            .request(
                Method::Get,
                update.artifact_url(),
                &[("Accept", "application/octet-stream")],
            )
            .map_err(|_| OtaError::Io)?;
        let mut response = request.submit().map_err(|_| OtaError::Io)?;
        if response.status() != 200 {
            return Err(OtaError::Io);
        }
        install_verified(&update, &mut response)?;
        Ok(version)
    }
}
