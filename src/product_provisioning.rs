//! Bounded first-boot provisioning policy.
//!
//! The target adapter serves this contract only on the temporary setup AP.
//! Secrets are accepted as form bytes and handed directly to the NVS-backed
//! configuration repository; this module has no filesystem or logging path.

use std::{collections::BTreeMap, fmt};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use sha2::{Digest, Sha256};

use crate::atlas_config::{
    normalize_atlas_url, MAX_ATLAS_URL_BYTES, MAX_WIFI_CREDENTIALS_BYTES, MAX_WIFI_SSID_BYTES,
};

pub const SETUP_BODY_MAX_BYTES: usize = 512;
pub const SETUP_PAGE_MAX_BYTES: usize = 4 * 1024;
pub const SETUP_SESSION_LIFETIME_MS: u64 = 10 * 60 * 1_000;
pub const SETUP_MAX_SUBMISSIONS: u8 = 8;
pub const SETUP_HTTP_PORT: u16 = 80;
pub const SETUP_AP_PASSWORD_BYTES: usize = 12;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PortalError {
    InvalidSession,
    PossessionRequired,
    Expired,
    AlreadyCompleted,
    TooManyAttempts,
    BodyTooLarge,
    Malformed,
    InvalidValue,
}

/// Ephemeral setup-AP and browser possession material derived from fresh
/// device entropy. Only the SSID is safe for ordinary diagnostics.
#[derive(Clone, Eq, PartialEq)]
pub struct SetupCredentials {
    ssid: String,
    ap_password: String,
    session_proof: String,
    csrf_proof: String,
}

/// Values intentionally rendered on the physical e-paper during setup. The AP
/// password is local possession material and remains redacted from Debug/logs.
#[derive(Clone, Eq, PartialEq)]
pub struct ProvisioningScreenData {
    ssid: String,
    ap_password: String,
    url: String,
    device_id: String,
}

impl fmt::Debug for ProvisioningScreenData {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProvisioningScreenData")
            .field("ssid", &self.ssid)
            .field("ap_password", &"<redacted>")
            .field("url", &self.url)
            .field("device_id", &self.device_id)
            .finish()
    }
}

impl ProvisioningScreenData {
    pub fn new(
        ssid: &str,
        ap_password: &str,
        url: &str,
        device_id: &str,
    ) -> Result<Self, PortalError> {
        if ssid.is_empty()
            || ssid.len() > MAX_WIFI_SSID_BYTES
            || ap_password.len() != SETUP_AP_PASSWORD_BYTES
            || url.is_empty()
            || url.len() > 64
            || device_id.is_empty()
            || device_id.len() > 64
            || [ssid, ap_password, url, device_id]
                .iter()
                .any(|value| value.chars().any(char::is_control))
        {
            return Err(PortalError::InvalidValue);
        }
        Ok(Self {
            ssid: ssid.into(),
            ap_password: ap_password.into(),
            url: url.into(),
            device_id: device_id.into(),
        })
    }

    #[must_use]
    pub fn ssid(&self) -> &str {
        &self.ssid
    }
    #[must_use]
    pub fn ap_password(&self) -> &str {
        &self.ap_password
    }
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }
    #[must_use]
    pub fn device_id(&self) -> &str {
        &self.device_id
    }
}

impl fmt::Debug for SetupCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SetupCredentials")
            .field("ssid", &self.ssid)
            .field("ap_password", &"<redacted>")
            .field("session_proof", &"<redacted>")
            .field("csrf_proof", &"<redacted>")
            .finish()
    }
}

impl SetupCredentials {
    pub fn from_entropy(entropy: &[u8]) -> Result<Self, PortalError> {
        if entropy.len() < 32 {
            return Err(PortalError::InvalidSession);
        }
        let session_proof = derive_proof(b"atlas-lite-setup-session\0", entropy);
        let csrf_proof = derive_proof(b"atlas-lite-setup-csrf\0", entropy);
        let access_material = derive_proof(b"atlas-lite-setup-ap\0", entropy);
        Ok(Self {
            ssid: format!("Atlas-Lite-{}", &access_material[..6]),
            ap_password: access_material[..SETUP_AP_PASSWORD_BYTES].to_owned(),
            session_proof,
            csrf_proof,
        })
    }

    #[must_use]
    pub fn ssid(&self) -> &str {
        &self.ssid
    }
    #[must_use]
    pub fn ap_password(&self) -> &str {
        &self.ap_password
    }
    #[must_use]
    pub fn session_proof(&self) -> &str {
        &self.session_proof
    }
    #[must_use]
    pub fn csrf_proof(&self) -> &str {
        &self.csrf_proof
    }
}

fn derive_proof(domain: &[u8], entropy: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(entropy);
    URL_SAFE_NO_PAD.encode(hash.finalize())
}

/// Minimal local-only setup page. The browser receives no Atlas credential;
/// pairing starts only after Wi-Fi and the Atlas origin are persisted.
pub fn render_setup_page(csrf_proof: &str) -> Result<String, PortalError> {
    if !valid_proof(csrf_proof) {
        return Err(PortalError::InvalidSession);
    }
    let page = format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Atlas Lite setup</title><style>body{{font-family:sans-serif;max-width:32rem;margin:2rem auto;padding:0 1rem}}label,input,button{{display:block;width:100%;box-sizing:border-box;margin:.7rem 0;padding:.7rem}}</style></head><body><h1>Atlas Lite setup</h1><p>Connect this device to Wi-Fi and Atlas. Pairing follows on the device.</p><form method="post" action="/setup"><label>Wi-Fi name<input name="ssid" maxlength="32" required></label><label>Wi-Fi password<input name="password" type="password" maxlength="63"></label><label>Atlas URL<input name="atlas_url" type="url" maxlength="192" placeholder="https://atlas.example" required></label><input type="hidden" name="csrf" value="{csrf_proof}"><button type="submit">Save and restart</button></form></body></html>"#
    );
    if page.len() > SETUP_PAGE_MAX_BYTES {
        return Err(PortalError::BodyTooLarge);
    }
    Ok(page)
}

/// Parse one exact HttpOnly setup-session cookie. Duplicate values fail closed.
#[must_use]
pub fn extract_setup_session_cookie(header: &str) -> Option<&str> {
    let mut found = None;
    for cookie in header.split(';') {
        let (name, value) = cookie.trim().split_once('=')?;
        if name == "atlas_setup" {
            if value.is_empty() || found.is_some() || !valid_proof(value) {
                return None;
            }
            found = Some(value);
        }
    }
    found
}

/// Validated provisioning data. Debug deliberately redacts the password.
#[derive(Clone, Eq, PartialEq)]
pub struct ProvisioningSubmission {
    ssid: String,
    password: String,
    atlas_url: String,
}

impl fmt::Debug for ProvisioningSubmission {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProvisioningSubmission")
            .field("ssid", &self.ssid)
            .field("password", &"<redacted>")
            .field("atlas_url", &self.atlas_url)
            .finish()
    }
}

impl ProvisioningSubmission {
    pub fn new(ssid: &str, password: &str, atlas_url: &str) -> Result<Self, PortalError> {
        if ssid.is_empty()
            || ssid.len() > MAX_WIFI_SSID_BYTES
            || password.len() > MAX_WIFI_CREDENTIALS_BYTES
            || atlas_url.len() > MAX_ATLAS_URL_BYTES
            || ssid.chars().any(char::is_control)
            || password.chars().any(char::is_control)
        {
            return Err(PortalError::InvalidValue);
        }
        let atlas_url =
            normalize_atlas_url(atlas_url.to_owned()).map_err(|_| PortalError::InvalidValue)?;
        Ok(Self {
            ssid: ssid.to_owned(),
            password: password.to_owned(),
            atlas_url,
        })
    }

    #[must_use]
    pub fn ssid(&self) -> &str {
        &self.ssid
    }

    #[must_use]
    pub fn password(&self) -> &str {
        &self.password
    }

    #[must_use]
    pub fn atlas_url(&self) -> &str {
        &self.atlas_url
    }
}

/// One browser session bound to the locally displayed setup-AP possession
/// secret. It is intentionally RAM-only and becomes unusable after success.
pub struct PortalSession {
    session_proof: String,
    csrf_proof: String,
    created_at_ms: u64,
    attempts: u8,
    completed: bool,
}

impl fmt::Debug for PortalSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PortalSession")
            .field("session_proof", &"<redacted>")
            .field("csrf_proof", &"<redacted>")
            .field("created_at_ms", &self.created_at_ms)
            .field("attempts", &self.attempts)
            .field("completed", &self.completed)
            .finish()
    }
}

impl PortalSession {
    pub fn new(
        session_proof: impl Into<String>,
        csrf_proof: impl Into<String>,
        created_at_ms: u64,
    ) -> Result<Self, PortalError> {
        let session_proof = session_proof.into();
        let csrf_proof = csrf_proof.into();
        if !valid_proof(&session_proof) || !valid_proof(&csrf_proof) {
            return Err(PortalError::InvalidSession);
        }
        Ok(Self {
            session_proof,
            csrf_proof,
            created_at_ms,
            attempts: 0,
            completed: false,
        })
    }

    pub fn parse_submission(
        &mut self,
        body: &[u8],
        session_cookie: Option<&str>,
        now_ms: u64,
    ) -> Result<ProvisioningSubmission, PortalError> {
        if self.completed {
            return Err(PortalError::AlreadyCompleted);
        }
        if now_ms.saturating_sub(self.created_at_ms) > SETUP_SESSION_LIFETIME_MS {
            return Err(PortalError::Expired);
        }
        if session_cookie != Some(self.session_proof.as_str()) {
            return Err(PortalError::PossessionRequired);
        }
        if body.len() > SETUP_BODY_MAX_BYTES {
            return Err(PortalError::BodyTooLarge);
        }
        if self.attempts >= SETUP_MAX_SUBMISSIONS {
            return Err(PortalError::TooManyAttempts);
        }
        self.attempts = self.attempts.saturating_add(1);
        let fields = parse_form(body)?;
        if fields.get("csrf").map(String::as_str) != Some(self.csrf_proof.as_str()) {
            return Err(PortalError::PossessionRequired);
        }
        let submission = ProvisioningSubmission::new(
            fields.get("ssid").ok_or(PortalError::Malformed)?,
            fields.get("password").ok_or(PortalError::Malformed)?,
            fields.get("atlas_url").ok_or(PortalError::Malformed)?,
        )?;
        Ok(submission)
    }

    /// Consume the session only after its validated values reached NVS.
    pub fn confirm_persisted(&mut self) {
        self.completed = true;
    }
}

fn valid_proof(value: &str) -> bool {
    (8..=96).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn parse_form(body: &[u8]) -> Result<BTreeMap<String, String>, PortalError> {
    let source = std::str::from_utf8(body).map_err(|_| PortalError::Malformed)?;
    let mut fields = BTreeMap::new();
    for pair in source.split('&') {
        let (key, value) = pair.split_once('=').ok_or(PortalError::Malformed)?;
        if !matches!(key, "ssid" | "password" | "atlas_url" | "csrf") || fields.contains_key(key) {
            return Err(PortalError::Malformed);
        }
        fields.insert(key.to_owned(), percent_decode(value)?);
    }
    if fields.len() != 4 {
        return Err(PortalError::Malformed);
    }
    Ok(fields)
}

fn percent_decode(value: &str) -> Result<String, PortalError> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let high = hex(bytes[index + 1]).ok_or(PortalError::Malformed)?;
                let low = hex(bytes[index + 2]).ok_or(PortalError::Malformed)?;
                output.push((high << 4) | low);
                index += 3;
            }
            b'%' => return Err(PortalError::Malformed),
            b'+' => {
                output.push(b' ');
                index += 1;
            }
            byte => {
                output.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(output).map_err(|_| PortalError::Malformed)
}

const fn hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(target_os = "espidf")]
pub mod espidf {
    use std::{
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };

    use anyhow::{anyhow, Context, Result};
    use embedded_svc::{
        http::{Headers, Method},
        io::{Read as _, Write as _},
        wifi::{AccessPointConfiguration, AuthMethod, Configuration as WifiConfiguration},
    };
    use esp_idf_svc::{
        eventloop::EspSystemEventLoop,
        hal::modem::WifiModemPeripheral,
        http::server::{Configuration as HttpConfiguration, EspHttpServer},
        nvs::EspDefaultNvsPartition,
        wifi::{BlockingWifi, EspWifi},
    };
    use log::{info, warn};

    use crate::atlas_config::{espidf::EspNvsConfigStore, ConfigRepository};

    use super::{
        extract_setup_session_cookie, render_setup_page, PortalError, PortalSession,
        SetupCredentials, SETUP_BODY_MAX_BYTES, SETUP_HTTP_PORT, SETUP_SESSION_LIFETIME_MS,
    };

    const SETUP_SERVER_STACK_BYTES: usize = 12 * 1024;
    const SETUP_AP_CHANNEL: u8 = 6;

    struct SharedSetup {
        session: PortalSession,
        completed: bool,
        started_at: Instant,
    }

    /// Temporary AP + local HTTP setup service. Dropping it stops both.
    pub struct ProductProvisioningServer {
        _server: EspHttpServer<'static>,
        _wifi: BlockingWifi<EspWifi<'static>>,
        shared: Arc<Mutex<SharedSetup>>,
        credentials: SetupCredentials,
        device_id: String,
        url: String,
    }

    impl ProductProvisioningServer {
        pub fn start<M>(modem: M, nvs: EspDefaultNvsPartition) -> Result<Self>
        where
            M: WifiModemPeripheral + 'static,
        {
            let mut entropy = [0_u8; 32];
            getrandom::getrandom(&mut entropy).map_err(|_| anyhow!("setup entropy unavailable"))?;
            let credentials = SetupCredentials::from_entropy(&entropy)
                .map_err(|_| anyhow!("invalid setup entropy"))?;
            let candidate_device_id = format!("atlas-lite-{}", hex_id(&entropy[..16]));
            let mut repository = ConfigRepository::new(
                EspNvsConfigStore::open(nvs.clone())
                    .map_err(|_| anyhow!("Atlas Lite NVS unavailable"))?,
            );
            let device_id = repository
                .ensure_device_id(&candidate_device_id)
                .map_err(|_| anyhow!("Atlas Lite identity unavailable"))?;

            let sys_loop = EspSystemEventLoop::take()?;
            let mut wifi = BlockingWifi::wrap(
                EspWifi::new(modem, sys_loop.clone(), Some(nvs.clone()))?,
                sys_loop,
            )?;
            wifi.set_configuration(&WifiConfiguration::AccessPoint(AccessPointConfiguration {
                ssid: credentials
                    .ssid()
                    .try_into()
                    .context("setup SSID exceeds ESP-IDF capacity")?,
                ssid_hidden: false,
                channel: SETUP_AP_CHANNEL,
                auth_method: AuthMethod::WPA2Personal,
                password: credentials
                    .ap_password()
                    .try_into()
                    .context("setup password exceeds ESP-IDF capacity")?,
                max_connections: 2,
                ..Default::default()
            }))?;
            wifi.start()?;
            wifi.wait_netif_up()?;
            let ip = wifi.wifi().ap_netif().get_ip_info()?.ip;
            let url = format!("http://{ip}/");

            let page = render_setup_page(credentials.csrf_proof())
                .map_err(|_| anyhow!("setup page unavailable"))?;
            let cookie = format!(
                "atlas_setup={}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}",
                credentials.session_proof(),
                SETUP_SESSION_LIFETIME_MS / 1_000
            );
            let shared = Arc::new(Mutex::new(SharedSetup {
                session: PortalSession::new(
                    credentials.session_proof(),
                    credentials.csrf_proof(),
                    0,
                )
                .map_err(|_| anyhow!("setup session unavailable"))?,
                completed: false,
                started_at: Instant::now(),
            }));

            let mut server = EspHttpServer::new(&HttpConfiguration {
                http_port: SETUP_HTTP_PORT,
                stack_size: SETUP_SERVER_STACK_BYTES,
                max_open_sockets: 2,
                max_sessions: 2,
                max_uri_handlers: 2,
                session_timeout: Duration::from_secs(30),
                ..Default::default()
            })?;
            server.fn_handler("/", Method::Get, move |request| {
                request
                    .into_response(
                        200,
                        Some("OK"),
                        &[
                            ("Content-Type", "text/html; charset=utf-8"),
                            ("Cache-Control", "no-store"),
                            ("Set-Cookie", cookie.as_str()),
                        ],
                    )?
                    .write_all(page.as_bytes())?;
                Ok::<(), anyhow::Error>(())
            })?;

            let submit_shared = Arc::clone(&shared);
            server.fn_handler("/setup", Method::Post, move |mut request| {
                let length = request
                    .content_len()
                    .map(|value| value as usize)
                    .unwrap_or(0);
                if length == 0 || length > SETUP_BODY_MAX_BYTES {
                    request
                        .into_status_response(if length > SETUP_BODY_MAX_BYTES {
                            413
                        } else {
                            400
                        })?
                        .write_all(b"Invalid bounded setup request")?;
                    return Ok::<(), anyhow::Error>(());
                }
                let session_cookie = request
                    .header("Cookie")
                    .and_then(extract_setup_session_cookie)
                    .map(str::to_owned);
                let mut body = vec![0_u8; length];
                request.read_exact(&mut body)?;
                let mut state = submit_shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                let elapsed = state
                    .started_at
                    .elapsed()
                    .as_millis()
                    .min(u128::from(u64::MAX)) as u64;
                let submission =
                    match state
                        .session
                        .parse_submission(&body, session_cookie.as_deref(), elapsed)
                    {
                        Ok(submission) => submission,
                        Err(error) => {
                            let status = portal_status(&error);
                            request
                                .into_status_response(status)?
                                .write_all(b"Setup request rejected")?;
                            return Ok(());
                        }
                    };
                let mut repository = match EspNvsConfigStore::open(nvs.clone()) {
                    Ok(store) => ConfigRepository::new(store),
                    Err(_) => {
                        request
                            .into_status_response(503)?
                            .write_all(b"Setup storage unavailable; retry")?;
                        return Ok(());
                    }
                };
                let persisted_device_id = repository
                    .load_device_id()
                    .map_err(|_| anyhow!("device identity unavailable"))?
                    .ok_or_else(|| anyhow!("device identity missing"))?;
                if repository
                    .save_provisioning(&persisted_device_id, &submission)
                    .is_err()
                {
                    warn!("atlas-lite=product-provisioning status=persist-failed");
                    request
                        .into_status_response(503)?
                        .write_all(b"Setup storage unavailable; retry")?;
                    return Ok(());
                }
                state.session.confirm_persisted();
                state.completed = true;
                info!("atlas-lite=product-provisioning status=persisted reboot=pending");
                request
                    .into_response(200, Some("OK"), &[("Cache-Control", "no-store")])?
                    .write_all(b"Saved. Atlas Lite is restarting.")?;
                Ok(())
            })?;

            info!(
                "atlas-lite=product-provisioning status=ready ssid={} url={} expiry-seconds={}",
                credentials.ssid(),
                url,
                SETUP_SESSION_LIFETIME_MS / 1_000
            );
            Ok(Self {
                _server: server,
                _wifi: wifi,
                shared,
                credentials,
                device_id,
                url,
            })
        }

        #[must_use]
        pub fn is_complete(&self) -> bool {
            self.shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .completed
        }

        #[must_use]
        pub fn is_expired(&self) -> bool {
            self.shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .started_at
                .elapsed()
                >= Duration::from_millis(SETUP_SESSION_LIFETIME_MS)
        }

        #[must_use]
        pub fn ssid(&self) -> &str {
            self.credentials.ssid()
        }
        #[must_use]
        pub fn ap_password(&self) -> &str {
            self.credentials.ap_password()
        }
        #[must_use]
        pub fn url(&self) -> &str {
            &self.url
        }
        #[must_use]
        pub fn device_id(&self) -> &str {
            &self.device_id
        }
    }

    impl Drop for ProductProvisioningServer {
        fn drop(&mut self) {
            info!("atlas-lite=product-provisioning status=stopped");
        }
    }

    fn portal_status(error: &PortalError) -> u16 {
        match error {
            PortalError::BodyTooLarge => 413,
            PortalError::Expired | PortalError::TooManyAttempts => 429,
            PortalError::AlreadyCompleted => 409,
            PortalError::InvalidSession | PortalError::PossessionRequired => 403,
            PortalError::Malformed | PortalError::InvalidValue => 400,
        }
    }

    fn hex_id(bytes: &[u8]) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            output.push(char::from(HEX[usize::from(byte >> 4)]));
            output.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        output
    }
}
