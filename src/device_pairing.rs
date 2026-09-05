//! Retry-safe Atlas Lite pairing material and bounded wire contract.

use core::fmt;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::atlas_config::{
    atlas_url_security, is_canonical_at_v1_token, MAX_PAIRING_STATE_BYTES, MINIMUM_CAPABILITIES,
};

pub const PAIRING_CODE_LENGTH: usize = 8;
pub const PAIRING_REQUEST_BODY_MAX_BYTES: usize = 1024;
pub const PAIRING_RESPONSE_BODY_MAX_BYTES: usize = 512;
pub const PAIRING_POLL_INTERVAL_SECONDS: u64 = 5;
/// The first retry after an unavailable pairing server. This stays below the
/// server's five-starts-per-minute limit even before exponential backoff.
pub const PAIRING_START_RETRY_INITIAL_SECONDS: u64 = 15;
pub const PAIRING_START_RETRY_MAX_SECONDS: u64 = 60;
pub const PAIRING_START_RATE_LIMIT_RETRY_SECONDS: u64 = 60;
const CODE_ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const VERIFIER_PREFIX: &[u8] = b"atlas-integration-token-v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairingError {
    Entropy,
    InvalidValue,
    TooLarge,
    Malformed,
}

/// A bounded, monotonic retry scheduler for the idempotent pairing start.
///
/// Polling is deliberately not governed by this scheduler: it remains on its
/// fixed five-second cadence, including while a start retry is deferred.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PairingStartRetry {
    start_confirmed: bool,
    consecutive_failures: u8,
    next_start_at_seconds: u64,
}

impl PairingStartRetry {
    #[must_use]
    pub const fn new(start_confirmed: bool) -> Self {
        Self {
            start_confirmed,
            consecutive_failures: 0,
            next_start_at_seconds: 0,
        }
    }

    #[must_use]
    pub const fn should_start(&self, elapsed_seconds: u64) -> bool {
        !self.start_confirmed && elapsed_seconds >= self.next_start_at_seconds
    }

    pub fn accepted(&mut self) {
        self.start_confirmed = true;
        self.consecutive_failures = 0;
    }

    pub fn unavailable(&mut self, elapsed_seconds: u64) {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        let exponent = u32::from(self.consecutive_failures.saturating_sub(1)).min(2);
        let delay = PAIRING_START_RETRY_INITIAL_SECONDS
            .saturating_mul(1_u64 << exponent)
            .min(PAIRING_START_RETRY_MAX_SECONDS);
        self.next_start_at_seconds = elapsed_seconds.saturating_add(delay);
    }

    pub fn rate_limited(&mut self, elapsed_seconds: u64) {
        self.next_start_at_seconds =
            elapsed_seconds.saturating_add(PAIRING_START_RATE_LIMIT_RETRY_SECONDS);
    }

    #[must_use]
    pub const fn next_start_at_seconds(&self) -> u64 {
        self.next_start_at_seconds
    }
}

/// Build the exact bounded headers for a pairing POST.
///
/// Empty body requests intentionally have `Content-Length: 0` but no content
/// type. ESP-IDF otherwise treats a POST without a length as chunked; this
/// expresses a real zero-byte request while preventing Fastify from invoking
/// its JSON parser for `poll`.
pub fn pairing_post_headers(
    body: &[u8],
    authorization: Option<&str>,
) -> Result<Vec<(&'static str, String)>, PairingError> {
    if body.len() > PAIRING_REQUEST_BODY_MAX_BYTES {
        return Err(PairingError::TooLarge);
    }
    let mut headers = vec![("Accept", "application/json".to_string())];
    if !body.is_empty() {
        headers.push(("Content-Type", "application/json".to_string()));
    }
    headers.push(("Content-Length", body.len().to_string()));
    if let Some(value) = authorization {
        headers.push(("Authorization", value.to_string()));
    }
    Ok(headers)
}

/// Construct a fixed Atlas pairing endpoint from a validated base URL.
///
/// Pairing is limited to its own versioned API subtree; the configured base is
/// always rechecked through the common Atlas URL policy before concatenation.
pub fn pairing_endpoint(atlas_url: &str, path: &str) -> Result<String, PairingError> {
    if atlas_url_security(atlas_url).is_none()
        || !path.starts_with("/api/v1/pairing/")
        || path.len() > 128
        || path
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || matches!(byte, b'?' | b'#'))
    {
        return Err(PairingError::InvalidValue);
    }
    Ok(format!("{atlas_url}{path}"))
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingPairing {
    request_id: String,
    device_id: String,
    device_name: String,
    code: String,
    poll_secret: String,
    token_id: String,
    api_secret: String,
    secret_salt: String,
    secret_verifier: String,
    #[serde(default)]
    start_confirmed: bool,
}

impl fmt::Debug for PendingPairing {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PendingPairing")
            .field("request_id", &self.request_id)
            .field("device_id", &self.device_id)
            .field("device_name", &self.device_name)
            .field("code", &self.code)
            .field("poll_secret", &"<redacted>")
            .field("api_secret", &"<redacted>")
            .field("secret_salt", &"<redacted>")
            .field("secret_verifier", &"<redacted>")
            .finish()
    }
}

impl PendingPairing {
    pub fn from_entropy(
        device_id: &str,
        device_name: &str,
        entropy: &[u8],
    ) -> Result<Self, PairingError> {
        if entropy.len() < 104 || !valid_device_id(device_id) {
            return Err(PairingError::Entropy);
        }
        let device_name = device_name.trim();
        if device_name.is_empty()
            || device_name.len() > 80
            || device_name.chars().any(char::is_control)
        {
            return Err(PairingError::InvalidValue);
        }
        let request_id = hex(&entropy[0..16]);
        let code = entropy[16..24]
            .iter()
            .map(|byte| CODE_ALPHABET[usize::from(*byte & 31)] as char)
            .collect();
        let poll_secret = URL_SAFE_NO_PAD.encode(&entropy[24..56]);
        let token_id = URL_SAFE_NO_PAD.encode(&entropy[56..72]);
        let api_secret = URL_SAFE_NO_PAD.encode(&entropy[72..104]);
        let mut salt_hash = Sha256::new();
        salt_hash.update(b"atlas-lite-pairing-salt-v1\0");
        salt_hash.update(entropy);
        let secret_salt = salt_hash.finalize();
        let mut verifier_hash = Sha256::new();
        verifier_hash.update(VERIFIER_PREFIX);
        verifier_hash.update(secret_salt);
        verifier_hash.update(&entropy[72..104]);
        let secret_verifier = verifier_hash.finalize();
        let pending = Self {
            request_id,
            device_id: device_id.into(),
            device_name: device_name.into(),
            code,
            poll_secret,
            token_id,
            api_secret,
            secret_salt: URL_SAFE_NO_PAD.encode(secret_salt),
            secret_verifier: URL_SAFE_NO_PAD.encode(secret_verifier),
            start_confirmed: false,
        };
        pending.validate()?;
        Ok(pending)
    }

    #[must_use]
    pub fn request_id(&self) -> &str {
        &self.request_id
    }
    #[must_use]
    pub fn device_id(&self) -> &str {
        &self.device_id
    }
    #[must_use]
    pub fn device_name(&self) -> &str {
        &self.device_name
    }
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }
    #[must_use]
    pub fn poll_secret(&self) -> &str {
        &self.poll_secret
    }
    #[must_use]
    pub fn bearer(&self) -> String {
        format!("at_v1.{}.{}", self.token_id, self.api_secret)
    }

    #[must_use]
    pub const fn start_confirmed(&self) -> bool {
        self.start_confirmed
    }

    /// Record a successful 201 or compatible 409 before the next reboot.
    pub fn mark_start_confirmed(&mut self) {
        self.start_confirmed = true;
    }

    pub fn start_body(&self) -> Result<Vec<u8>, PairingError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            request_id: &'a str,
            device_id: &'a str,
            device_name: &'a str,
            code: &'a str,
            poll_secret: &'a str,
            token_id: &'a str,
            secret_salt: &'a str,
            secret_verifier: &'a str,
            scopes: [&'static str; 4],
        }
        let body = serde_json::to_vec(&Body {
            request_id: &self.request_id,
            device_id: &self.device_id,
            device_name: &self.device_name,
            code: &self.code,
            poll_secret: &self.poll_secret,
            token_id: &self.token_id,
            secret_salt: &self.secret_salt,
            secret_verifier: &self.secret_verifier,
            scopes: MINIMUM_CAPABILITIES,
        })
        .map_err(|_| PairingError::Malformed)?;
        if body.len() > PAIRING_REQUEST_BODY_MAX_BYTES {
            return Err(PairingError::TooLarge);
        }
        Ok(body)
    }

    pub fn to_persisted_bytes(&self) -> Result<Vec<u8>, PairingError> {
        let bytes = serde_json::to_vec(self).map_err(|_| PairingError::Malformed)?;
        if bytes.len() > MAX_PAIRING_STATE_BYTES {
            return Err(PairingError::TooLarge);
        }
        Ok(bytes)
    }

    pub fn from_persisted_bytes(bytes: &[u8]) -> Result<Self, PairingError> {
        if bytes.len() > MAX_PAIRING_STATE_BYTES {
            return Err(PairingError::TooLarge);
        }
        let pending: Self = serde_json::from_slice(bytes).map_err(|_| PairingError::Malformed)?;
        pending.validate()?;
        Ok(pending)
    }

    fn validate(&self) -> Result<(), PairingError> {
        if self.request_id.len() != 32
            || !self.request_id.bytes().all(|b| b.is_ascii_hexdigit())
            || !valid_device_id(&self.device_id)
            || self.device_name.is_empty()
            || self.device_name.len() > 80
            || self.code.len() != PAIRING_CODE_LENGTH
            || !self.code.bytes().all(|b| CODE_ALPHABET.contains(&b))
            || decode_len(&self.poll_secret) != Some(32)
            || decode_len(&self.token_id) != Some(16)
            || decode_len(&self.api_secret) != Some(32)
            || decode_len(&self.secret_salt) != Some(32)
            || decode_len(&self.secret_verifier) != Some(32)
            || !is_canonical_at_v1_token(&self.bearer())
        {
            return Err(PairingError::InvalidValue);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairingStatus {
    Pending,
    Approved,
    Denied,
    Expired,
}

pub fn parse_poll_response(
    status: u16,
    body: &[u8],
    request_id: &str,
) -> Result<PairingStatus, PairingError> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    struct Response {
        request_id: String,
        status: String,
        expires_at: u64,
    }
    if status != 200 || body.len() > PAIRING_RESPONSE_BODY_MAX_BYTES {
        return Err(PairingError::Malformed);
    }
    let response: Response = serde_json::from_slice(body).map_err(|_| PairingError::Malformed)?;
    if response.request_id != request_id || response.expires_at == 0 {
        return Err(PairingError::Malformed);
    }
    match response.status.as_str() {
        "pending" => Ok(PairingStatus::Pending),
        "approved" => Ok(PairingStatus::Approved),
        "denied" => Ok(PairingStatus::Denied),
        "expired" => Ok(PairingStatus::Expired),
        _ => Err(PairingError::Malformed),
    }
}

fn decode_len(value: &str) -> Option<usize> {
    let bytes = URL_SAFE_NO_PAD.decode(value).ok()?;
    (URL_SAFE_NO_PAD.encode(&bytes) == value).then_some(bytes.len())
}

fn valid_device_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[usize::from(byte >> 4)] as char);
        output.push(HEX[usize::from(byte & 15)] as char);
    }
    output
}

#[cfg(target_os = "espidf")]
pub mod espidf {
    use std::time::Duration;

    use anyhow::{anyhow, Result};
    use embedded_svc::{
        http::{client::Client as HttpClient, Method},
        io::Write as _,
    };
    use esp_idf_svc::{
        http::client::{Configuration, EspHttpConnection, FollowRedirectsPolicy},
        sys,
    };

    use crate::atlas_config::ProvisionedConfig;

    use super::{
        pairing_endpoint, pairing_post_headers, parse_poll_response, PairingStatus, PendingPairing,
        PAIRING_RESPONSE_BODY_MAX_BYTES,
    };

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum PairingStartOutcome {
        Accepted,
        RateLimited,
    }

    pub struct EspIdfPairingTransport<'a> {
        provisioning: &'a ProvisionedConfig,
    }

    impl<'a> EspIdfPairingTransport<'a> {
        #[must_use]
        pub const fn new(provisioning: &'a ProvisionedConfig) -> Self {
            Self { provisioning }
        }

        pub fn start(&mut self, pending: &PendingPairing) -> Result<PairingStartOutcome> {
            let body = pending
                .start_body()
                .map_err(|_| anyhow!("invalid pairing material"))?;
            let response = self.post("/api/v1/pairing/requests", &body, None)?;
            match response.0 {
                201 | 409 => Ok(PairingStartOutcome::Accepted),
                429 => Ok(PairingStartOutcome::RateLimited),
                status => Err(anyhow!("pairing start rejected status={status}")),
            }
        }

        pub fn poll(&mut self, pending: &PendingPairing) -> Result<PairingStatus> {
            let path = format!("/api/v1/pairing/requests/{}/poll", pending.request_id());
            let authorization = format!("Pairing {}", pending.poll_secret());
            let (status, body) = self.post(&path, b"", Some(&authorization))?;
            parse_poll_response(status, &body, pending.request_id())
                .map_err(|_| anyhow!("invalid pairing response"))
        }

        fn post(
            &mut self,
            path: &str,
            body: &[u8],
            authorization: Option<&str>,
        ) -> Result<(u16, Vec<u8>)> {
            let config = Configuration {
                crt_bundle_attach: Some(sys::esp_crt_bundle_attach),
                timeout: Some(Duration::from_secs(10)),
                buffer_size: Some(1024),
                buffer_size_tx: Some(1024),
                keep_alive_enable: false,
                follow_redirects_policy: FollowRedirectsPolicy::FollowNone,
                ..Default::default()
            };
            let mut client = HttpClient::wrap(EspHttpConnection::new(&config)?);
            let url = pairing_endpoint(self.provisioning.atlas_url(), path)
                .map_err(|_| anyhow!("pairing URL rejected by Atlas URL policy"))?;
            let header_values = pairing_post_headers(body, authorization)
                .map_err(|_| anyhow!("pairing request headers rejected"))?;
            let headers: Vec<(&str, &str)> = header_values
                .iter()
                .map(|(name, value)| (*name, value.as_str()))
                .collect();
            let mut request = client.request(Method::Post, &url, &headers)?;
            if !body.is_empty() {
                request.write_all(body)?;
            }
            let mut response = request.submit()?;
            let status = response.status();
            let mut bytes = Vec::with_capacity(PAIRING_RESPONSE_BODY_MAX_BYTES);
            let mut chunk = [0_u8; 256];
            loop {
                let read = response.read(&mut chunk)?;
                if read == 0 {
                    break;
                }
                if bytes.len() + read > PAIRING_RESPONSE_BODY_MAX_BYTES {
                    return Err(anyhow!("pairing response too large"));
                }
                bytes.extend_from_slice(&chunk[..read]);
            }
            Ok((status, bytes))
        }
    }
}
