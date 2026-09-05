//! Atlas Lite configuration kept separate from removable-storage features.
//!
//! The configuration domain intentionally has no serialization implementation:
//! callers must select individual values instead of dumping an object that
//! could include an Atlas token or Wi-Fi credential.

use std::{collections::BTreeMap, error::Error, fmt};

pub const CONFIG_SCHEMA_VERSION: &str = "1";
pub const MAX_DEVICE_ID_BYTES: usize = 64;
pub const MAX_ATLAS_URL_BYTES: usize = 192;
/// Canonical `at_v1` bearer length: version, 16-byte token ID, and 32-byte secret.
pub const MAX_API_TOKEN_BYTES: usize = 72;
pub const MAX_WIFI_SSID_BYTES: usize = 32;
pub const MAX_WIFI_CREDENTIALS_BYTES: usize = 63;
pub const MAX_CONFIG_VALUE_BYTES: usize = MAX_API_TOKEN_BYTES;
pub const MAX_PAIRING_STATE_BYTES: usize = 768;

/// The transport security selected by a validated Atlas base URL.
///
/// HTTPS is the normal and default mode. Plain HTTP is intentionally limited
/// to a literal RFC1918 IPv4 address so hostnames can never become an implicit
/// DNS-based exception to the TLS policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtlasUrlSecurity {
    Https,
    PrivateLanHttp,
}

impl AtlasUrlSecurity {
    #[must_use]
    pub const fn is_private_lan_http(self) -> bool {
        matches!(self, Self::PrivateLanHttp)
    }
}

const VERSION_KEY: &str = "version";
const DEVICE_ID_KEY: &str = "device_id";
const ATLAS_URL_KEY: &str = "atlas_url";
const API_TOKEN_KEY: &str = "api_token";
const WIFI_SSID_KEY: &str = "wifi_ssid";
const WIFI_CREDENTIALS_KEY: &str = "wifi_cred";
const PAIRING_STATE_KEY: &str = "pair_pending";
pub const CONFIG_STORE_KEYS: [&str; 7] = [
    VERSION_KEY,
    DEVICE_ID_KEY,
    ATLAS_URL_KEY,
    API_TOKEN_KEY,
    WIFI_SSID_KEY,
    WIFI_CREDENTIALS_KEY,
    PAIRING_STATE_KEY,
];
/// A conforming config store can contain no more entries than the fixed key
/// domain above. Unknown keys are rejected before backend access.
pub const MAX_CONFIG_ENTRIES: usize = CONFIG_STORE_KEYS.len();

/// The only capabilities Atlas Lite intends to request for its dedicated key.
/// This is metadata, not a local authorization or server-scope implementation.
pub const MINIMUM_CAPABILITIES: [&str; 4] =
    ["notes:read", "search:read", "views:read", "capture:write"];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ConfigField {
    DeviceId,
    AtlasUrl,
    ApiToken,
    WifiSsid,
    WifiCredentials,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigStatus {
    Unconfigured,
    Partial(Vec<ConfigField>),
    Ready,
}

/// Validated values. Secrets remain private and this type is deliberately not
/// serializable through a generic derive.
#[derive(Clone, Eq, PartialEq)]
pub struct AtlasConfig {
    device_id: String,
    atlas_url: String,
    api_token: String,
    wifi_ssid: String,
    wifi_credentials: String,
}

impl fmt::Debug for AtlasConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AtlasConfig")
            .field("device_id", &self.device_id)
            .field("atlas_url", &self.atlas_url)
            .field("api_token", &"<redacted>")
            .field("wifi_ssid", &self.wifi_ssid)
            .field("wifi_credentials", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for AtlasConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "AtlasConfig(device_id={}, atlas_url={}, api_token=<redacted>, wifi_ssid={}, wifi_credentials=<redacted>)",
            self.device_id, self.atlas_url, self.wifi_ssid
        )
    }
}

impl AtlasConfig {
    pub fn new(
        device_id: impl Into<String>,
        atlas_url: impl Into<String>,
        api_token: impl Into<String>,
        wifi_ssid: impl Into<String>,
        wifi_credentials: impl Into<String>,
    ) -> Result<Self, ConfigError> {
        let device_id = device_id.into();
        let atlas_url = normalize_atlas_url(atlas_url.into())?;
        let api_token = api_token.into();
        let wifi_ssid = wifi_ssid.into();
        let wifi_credentials = wifi_credentials.into();
        validate_device_id(&device_id)?;
        validate_api_token(&api_token)?;
        validate_wifi_ssid(&wifi_ssid)?;
        validate_wifi_credentials(&wifi_credentials)?;
        Ok(Self {
            device_id,
            atlas_url,
            api_token,
            wifi_ssid,
            wifi_credentials,
        })
    }

    #[must_use]
    pub fn device_id(&self) -> &str {
        &self.device_id
    }
    #[must_use]
    pub fn atlas_url(&self) -> &str {
        &self.atlas_url
    }
    #[must_use]
    pub fn atlas_url_security(&self) -> AtlasUrlSecurity {
        // `AtlasConfig::new` and NVS loading both normalize this field before
        // constructing the value.
        atlas_url_security(&self.atlas_url).expect("validated Atlas URL")
    }
    #[must_use]
    pub fn api_token(&self) -> &str {
        &self.api_token
    }
    #[must_use]
    pub fn wifi_ssid(&self) -> &str {
        &self.wifi_ssid
    }
    #[must_use]
    pub fn wifi_credentials(&self) -> &str {
        &self.wifi_credentials
    }
}

/// The non-token configuration needed to connect and begin pairing.
#[derive(Clone, Eq, PartialEq)]
pub struct ProvisionedConfig {
    device_id: String,
    atlas_url: String,
    wifi_ssid: String,
    wifi_credentials: String,
}

impl fmt::Debug for ProvisionedConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProvisionedConfig")
            .field("device_id", &self.device_id)
            .field("atlas_url", &self.atlas_url)
            .field("wifi_ssid", &self.wifi_ssid)
            .field("wifi_credentials", &"<redacted>")
            .finish()
    }
}

impl ProvisionedConfig {
    #[must_use]
    pub fn device_id(&self) -> &str {
        &self.device_id
    }
    #[must_use]
    pub fn atlas_url(&self) -> &str {
        &self.atlas_url
    }
    #[must_use]
    pub fn atlas_url_security(&self) -> AtlasUrlSecurity {
        // Provisioning uses the same validated configuration boundary as a
        // fully paired config.
        atlas_url_security(&self.atlas_url).expect("validated Atlas URL")
    }
    #[must_use]
    pub fn wifi_ssid(&self) -> &str {
        &self.wifi_ssid
    }
    #[must_use]
    pub fn wifi_credentials(&self) -> &str {
        &self.wifi_credentials
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedConfig {
    status: ConfigStatus,
    config: Option<AtlasConfig>,
}

impl LoadedConfig {
    #[must_use]
    pub fn status(&self) -> ConfigStatus {
        self.status.clone()
    }
    #[must_use]
    pub fn config(&self) -> Option<&AtlasConfig> {
        self.config.as_ref()
    }
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.status == ConfigStatus::Ready
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigError {
    InvalidValue {
        field: ConfigField,
        reason: &'static str,
    },
    Corrupt {
        key: &'static str,
    },
    UnsupportedSchema {
        found: String,
    },
    Store(ConfigStoreError),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidValue { field, reason } => {
                write!(formatter, "invalid {field:?}: {reason}")
            }
            Self::Corrupt { key } => write!(formatter, "corrupt configuration entry {key}"),
            Self::UnsupportedSchema { .. } => {
                formatter.write_str("unsupported configuration schema")
            }
            Self::Store(error) => write!(formatter, "configuration store error: {error}"),
        }
    }
}

impl Error for ConfigError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigStoreError {
    UnknownKey { key: String },
    ValueTooLarge { key: String, length: usize },
    Backend,
}

impl fmt::Display for ConfigStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownKey { key } => write!(formatter, "unsupported configuration key {key}"),
            Self::ValueTooLarge { key, length } => {
                write!(formatter, "value too large for {key}: {length}")
            }
            Self::Backend => formatter.write_str("backend failure"),
        }
    }
}

impl Error for ConfigStoreError {}

/// Minimal byte store boundary for host fakes and target-internal NVS.
pub trait ConfigStore {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, ConfigStoreError>;
    fn set(&mut self, key: &str, value: &[u8]) -> Result<(), ConfigStoreError>;
    fn remove(&mut self, key: &str) -> Result<(), ConfigStoreError>;
    fn clear(&mut self) -> Result<(), ConfigStoreError>;
}

#[derive(Default)]
pub struct FakeConfigStore {
    values: BTreeMap<String, Vec<u8>>,
    fail_operations: bool,
}

impl FakeConfigStore {
    pub fn insert_raw(&mut self, key: &str, value: &[u8]) -> Result<(), ConfigStoreError> {
        validate_store_key_value(key, value)?;
        self.values.insert(key.into(), value.into());
        Ok(())
    }

    pub fn set_fail_operations(&mut self, enabled: bool) {
        self.fail_operations = enabled;
    }
}

impl ConfigStore for FakeConfigStore {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, ConfigStoreError> {
        if self.fail_operations {
            return Err(ConfigStoreError::Backend);
        }
        validate_store_key(key)?;
        Ok(self.values.get(key).cloned())
    }

    fn set(&mut self, key: &str, value: &[u8]) -> Result<(), ConfigStoreError> {
        if self.fail_operations {
            return Err(ConfigStoreError::Backend);
        }
        validate_store_key_value(key, value)?;
        self.values.insert(key.into(), value.into());
        Ok(())
    }

    fn remove(&mut self, key: &str) -> Result<(), ConfigStoreError> {
        if self.fail_operations {
            return Err(ConfigStoreError::Backend);
        }
        validate_store_key(key)?;
        self.values.remove(key);
        Ok(())
    }

    fn clear(&mut self) -> Result<(), ConfigStoreError> {
        if self.fail_operations {
            return Err(ConfigStoreError::Backend);
        }
        self.values.clear();
        Ok(())
    }
}

pub struct ConfigRepository<S> {
    store: S,
}

impl<S: ConfigStore> ConfigRepository<S> {
    #[must_use]
    pub const fn new(store: S) -> Self {
        Self { store }
    }
    #[must_use]
    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }

    pub fn load(&self) -> Result<LoadedConfig, ConfigError> {
        let version = self.read(VERSION_KEY)?;
        let fields = [
            (ConfigField::DeviceId, self.read(DEVICE_ID_KEY)?),
            (ConfigField::AtlasUrl, self.read(ATLAS_URL_KEY)?),
            (ConfigField::ApiToken, self.read(API_TOKEN_KEY)?),
            (ConfigField::WifiSsid, self.read(WIFI_SSID_KEY)?),
            (
                ConfigField::WifiCredentials,
                self.read(WIFI_CREDENTIALS_KEY)?,
            ),
        ];
        if version.is_none() && fields.iter().all(|(_, value)| value.is_none()) {
            return Ok(LoadedConfig {
                status: ConfigStatus::Unconfigured,
                config: None,
            });
        }
        let version = version.ok_or(ConfigError::Corrupt { key: VERSION_KEY })?;
        if version != CONFIG_SCHEMA_VERSION {
            return Err(ConfigError::UnsupportedSchema { found: version });
        }
        let missing = fields
            .iter()
            .filter_map(|(field, value)| value.is_none().then_some(*field))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            for (field, value) in &fields {
                if let Some(value) = value {
                    validate_field(*field, value)?;
                }
            }
            return Ok(LoadedConfig {
                status: ConfigStatus::Partial(missing),
                config: None,
            });
        }
        let config = AtlasConfig::new(
            fields[0].1.as_deref().unwrap_or_default(),
            fields[1].1.as_deref().unwrap_or_default(),
            fields[2].1.as_deref().unwrap_or_default(),
            fields[3].1.as_deref().unwrap_or_default(),
            fields[4].1.as_deref().unwrap_or_default(),
        )?;
        Ok(LoadedConfig {
            status: ConfigStatus::Ready,
            config: Some(config),
        })
    }

    pub fn save(&mut self, config: &AtlasConfig) -> Result<(), ConfigError> {
        self.write(VERSION_KEY, CONFIG_SCHEMA_VERSION)?;
        self.write(DEVICE_ID_KEY, config.device_id())?;
        self.write(ATLAS_URL_KEY, config.atlas_url())?;
        self.write(API_TOKEN_KEY, config.api_token())?;
        self.write(WIFI_SSID_KEY, config.wifi_ssid())?;
        self.write(WIFI_CREDENTIALS_KEY, config.wifi_credentials())
    }

    pub fn save_provisioning(
        &mut self,
        device_id: &str,
        submission: &crate::product_provisioning::ProvisioningSubmission,
    ) -> Result<(), ConfigError> {
        validate_device_id(device_id)?;
        validate_wifi_ssid(submission.ssid())?;
        validate_wifi_credentials(submission.password())?;
        let atlas_url = normalize_atlas_url(submission.atlas_url().to_owned())?;
        self.write(VERSION_KEY, CONFIG_SCHEMA_VERSION)?;
        self.write(DEVICE_ID_KEY, device_id)?;
        self.write(ATLAS_URL_KEY, &atlas_url)?;
        self.write(WIFI_SSID_KEY, submission.ssid())?;
        self.write(WIFI_CREDENTIALS_KEY, submission.password())
    }

    /// Return the persisted stable identity, creating it once from caller
    /// supplied CSPRNG material when this namespace has no identity yet.
    pub fn ensure_device_id(&mut self, candidate: &str) -> Result<String, ConfigError> {
        validate_device_id(candidate)?;
        if let Some(existing) = self.load_device_id()? {
            return Ok(existing);
        }
        match self.read(VERSION_KEY)? {
            None => self.write(VERSION_KEY, CONFIG_SCHEMA_VERSION)?,
            Some(version) if version == CONFIG_SCHEMA_VERSION => {}
            Some(found) => return Err(ConfigError::UnsupportedSchema { found }),
        }
        self.write(DEVICE_ID_KEY, candidate)?;
        Ok(candidate.to_owned())
    }

    pub fn load_device_id(&self) -> Result<Option<String>, ConfigError> {
        let Some(device_id) = self.read(DEVICE_ID_KEY)? else {
            return Ok(None);
        };
        validate_device_id(&device_id)?;
        Ok(Some(device_id))
    }

    pub fn load_provisioning(&self) -> Result<Option<ProvisionedConfig>, ConfigError> {
        let version = self.read(VERSION_KEY)?;
        let device_id = self.read(DEVICE_ID_KEY)?;
        let atlas_url = self.read(ATLAS_URL_KEY)?;
        let wifi_ssid = self.read(WIFI_SSID_KEY)?;
        let wifi_credentials = self.read(WIFI_CREDENTIALS_KEY)?;
        if [
            device_id.as_ref(),
            atlas_url.as_ref(),
            wifi_ssid.as_ref(),
            wifi_credentials.as_ref(),
        ]
        .iter()
        .all(|value| value.is_none())
        {
            return Ok(None);
        }
        if version.as_deref() != Some(CONFIG_SCHEMA_VERSION) {
            return Err(
                version.map_or(ConfigError::Corrupt { key: VERSION_KEY }, |found| {
                    ConfigError::UnsupportedSchema { found }
                }),
            );
        }
        let (Some(device_id), Some(atlas_url), Some(wifi_ssid), Some(wifi_credentials)) =
            (device_id, atlas_url, wifi_ssid, wifi_credentials)
        else {
            return Ok(None);
        };
        validate_device_id(&device_id)?;
        let atlas_url = normalize_atlas_url(atlas_url)?;
        validate_wifi_ssid(&wifi_ssid)?;
        validate_wifi_credentials(&wifi_credentials)?;
        Ok(Some(ProvisionedConfig {
            device_id,
            atlas_url,
            wifi_ssid,
            wifi_credentials,
        }))
    }

    pub fn save_api_token(&mut self, token: &str) -> Result<(), ConfigError> {
        validate_api_token(token)?;
        self.write(API_TOKEN_KEY, token)
    }

    pub fn save_pending_pairing(
        &mut self,
        pending: &crate::device_pairing::PendingPairing,
    ) -> Result<(), ConfigError> {
        let bytes = pending
            .to_persisted_bytes()
            .map_err(|_| ConfigError::Corrupt {
                key: PAIRING_STATE_KEY,
            })?;
        self.store
            .set(PAIRING_STATE_KEY, &bytes)
            .map_err(ConfigError::Store)
    }

    pub fn load_pending_pairing(
        &self,
    ) -> Result<Option<crate::device_pairing::PendingPairing>, ConfigError> {
        let Some(bytes) = self
            .store
            .get(PAIRING_STATE_KEY)
            .map_err(ConfigError::Store)?
        else {
            return Ok(None);
        };
        crate::device_pairing::PendingPairing::from_persisted_bytes(&bytes)
            .map(Some)
            .map_err(|_| ConfigError::Corrupt {
                key: PAIRING_STATE_KEY,
            })
    }

    /// Persist the already-generated bearer before removing retry material.
    /// A reset between these operations is safe: boot sees a usable token and
    /// may discard the redundant pending record later.
    pub fn complete_pairing(
        &mut self,
        pending: &crate::device_pairing::PendingPairing,
    ) -> Result<(), ConfigError> {
        self.save_api_token(&pending.bearer())?;
        self.store
            .remove(PAIRING_STATE_KEY)
            .map_err(ConfigError::Store)
    }

    pub fn discard_pending_pairing(&mut self) -> Result<(), ConfigError> {
        self.store
            .remove(PAIRING_STATE_KEY)
            .map_err(ConfigError::Store)
    }

    pub fn unpair(&mut self) -> Result<(), ConfigError> {
        self.store
            .remove(API_TOKEN_KEY)
            .map_err(ConfigError::Store)?;
        self.store
            .remove(PAIRING_STATE_KEY)
            .map_err(ConfigError::Store)
    }

    pub fn reset_wifi(&mut self) -> Result<(), ConfigError> {
        self.store
            .remove(WIFI_SSID_KEY)
            .map_err(ConfigError::Store)?;
        self.store
            .remove(WIFI_CREDENTIALS_KEY)
            .map_err(ConfigError::Store)
    }

    pub fn update_wifi(&mut self, ssid: &str, credentials: &str) -> Result<(), ConfigError> {
        validate_wifi_ssid(ssid)?;
        validate_wifi_credentials(credentials)?;
        self.write(WIFI_SSID_KEY, ssid)?;
        self.write(WIFI_CREDENTIALS_KEY, credentials)
    }

    /// Reserved for the future Settings factory-reset action; it clears only
    /// this configuration namespace and never touches removable storage.
    pub fn clear(&mut self) -> Result<(), ConfigError> {
        self.store.clear().map_err(ConfigError::Store)
    }

    fn read(&self, key: &'static str) -> Result<Option<String>, ConfigError> {
        let value = self.store.get(key).map_err(ConfigError::Store)?;
        match value {
            None => Ok(None),
            Some(value) if value.len() > MAX_CONFIG_VALUE_BYTES => {
                Err(ConfigError::Corrupt { key })
            }
            Some(value) => String::from_utf8(value)
                .map(Some)
                .map_err(|_| ConfigError::Corrupt { key }),
        }
    }

    fn write(&mut self, key: &str, value: &str) -> Result<(), ConfigError> {
        self.store
            .set(key, value.as_bytes())
            .map_err(ConfigError::Store)
    }
}

fn validate_field(field: ConfigField, value: &str) -> Result<(), ConfigError> {
    match field {
        ConfigField::DeviceId => validate_device_id(value),
        ConfigField::AtlasUrl => normalize_atlas_url(value.into()).map(|_| ()),
        ConfigField::ApiToken => validate_api_token(value),
        ConfigField::WifiSsid => validate_wifi_ssid(value),
        ConfigField::WifiCredentials => validate_wifi_credentials(value),
    }
}

fn validate_store_key(key: &str) -> Result<(), ConfigStoreError> {
    if CONFIG_STORE_KEYS.contains(&key) {
        Ok(())
    } else {
        Err(ConfigStoreError::UnknownKey { key: key.into() })
    }
}

fn validate_store_key_value(key: &str, value: &[u8]) -> Result<(), ConfigStoreError> {
    let max_length = store_value_limit(key)?;
    if value.len() > max_length {
        return Err(ConfigStoreError::ValueTooLarge {
            key: key.into(),
            length: value.len(),
        });
    }
    Ok(())
}

fn store_value_limit(key: &str) -> Result<usize, ConfigStoreError> {
    validate_store_key(key)?;
    Ok(match key {
        VERSION_KEY => CONFIG_SCHEMA_VERSION.len(),
        DEVICE_ID_KEY => MAX_DEVICE_ID_BYTES,
        ATLAS_URL_KEY => MAX_ATLAS_URL_BYTES,
        API_TOKEN_KEY => MAX_API_TOKEN_BYTES,
        WIFI_SSID_KEY => MAX_WIFI_SSID_BYTES,
        WIFI_CREDENTIALS_KEY => MAX_WIFI_CREDENTIALS_BYTES,
        PAIRING_STATE_KEY => MAX_PAIRING_STATE_BYTES,
        _ => unreachable!("validate_store_key already checked this key"),
    })
}

fn validate_device_id(value: &str) -> Result<(), ConfigError> {
    if value.is_empty()
        || value.len() > MAX_DEVICE_ID_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(ConfigError::InvalidValue {
            field: ConfigField::DeviceId,
            reason: "must be 1-64 ASCII letters, digits, hyphen, or underscore",
        });
    }
    Ok(())
}

pub(crate) fn normalize_atlas_url(value: String) -> Result<String, ConfigError> {
    atlas_url_security(&value).ok_or(ConfigError::InvalidValue {
        field: ConfigField::AtlasUrl,
        reason: "must be an HTTPS base URL or HTTP to a literal RFC1918 IPv4 address",
    })?;
    Ok(value.trim_end_matches('/').into())
}

/// Return the only transport modes accepted for an Atlas base URL.
///
/// This is the single URL policy used by NVS configuration, provisioning,
/// pairing and the normal/audio transports. It deliberately never resolves a
/// hostname: HTTP development mode is for an IPv4 literal in RFC1918 only.
#[must_use]
pub fn atlas_url_security(value: &str) -> Option<AtlasUrlSecurity> {
    if value.is_empty()
        || value.len() > MAX_ATLAS_URL_BYTES
        || value.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return None;
    }
    let (security, remainder) = if let Some(remainder) = value.strip_prefix("https://") {
        (AtlasUrlSecurity::Https, remainder)
    } else if let Some(remainder) = value.strip_prefix("http://") {
        (AtlasUrlSecurity::PrivateLanHttp, remainder)
    } else {
        return None;
    };
    if remainder.is_empty()
        || remainder.contains('@')
        || remainder.contains('?')
        || remainder.contains('#')
    {
        return None;
    }
    let (authority, path) = remainder.split_once('/').unwrap_or((remainder, ""));
    if authority.is_empty() {
        return None;
    }
    if !path.is_empty() {
        return None;
    }
    if security == AtlasUrlSecurity::PrivateLanHttp && !is_rfc1918_ipv4_authority(authority) {
        return None;
    }
    Some(security)
}

fn is_rfc1918_ipv4_authority(authority: &str) -> bool {
    let (host, port) = match authority.split_once(':') {
        Some((host, port)) if !port.contains(':') => (host, Some(port)),
        Some(_) => return false,
        None => (authority, None),
    };
    if host.is_empty()
        || port.is_some_and(|value| {
            value.is_empty()
                || !value.bytes().all(|byte| byte.is_ascii_digit())
                || !matches!(value.parse::<u16>(), Ok(1..=u16::MAX))
        })
    {
        return false;
    }

    let mut octets = host.split('.');
    let (Some(first), Some(second), Some(_third), Some(_fourth), None) = (
        octets.next().and_then(parse_ipv4_octet),
        octets.next().and_then(parse_ipv4_octet),
        octets.next().and_then(parse_ipv4_octet),
        octets.next().and_then(parse_ipv4_octet),
        octets.next(),
    ) else {
        return false;
    };
    first == 10 || (first == 172 && (16..=31).contains(&second)) || (first == 192 && second == 168)
}

fn parse_ipv4_octet(value: &str) -> Option<u8> {
    if value.is_empty()
        || value.len() > 3
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return None;
    }
    value.parse().ok()
}

pub fn is_canonical_at_v1_token(value: &str) -> bool {
    let mut parts = value.split('.');
    let (Some(version), Some(token_id), Some(secret), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    version == "at_v1"
        && canonical_base64url_segment(token_id, 22, b"AQgw")
        && canonical_base64url_segment(secret, 43, b"AEIMQUYcgkosw048")
}

fn canonical_base64url_segment(value: &str, length: usize, valid_final: &[u8]) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        && value
            .as_bytes()
            .last()
            .is_some_and(|byte| valid_final.contains(byte))
}

fn validate_api_token(value: &str) -> Result<(), ConfigError> {
    if value.len() != MAX_API_TOKEN_BYTES || !is_canonical_at_v1_token(value) {
        return Err(ConfigError::InvalidValue {
            field: ConfigField::ApiToken,
            reason: "must be a canonical at_v1 integration token",
        });
    }
    Ok(())
}

fn validate_wifi_ssid(value: &str) -> Result<(), ConfigError> {
    if value.is_empty() || value.len() > MAX_WIFI_SSID_BYTES {
        return Err(ConfigError::InvalidValue {
            field: ConfigField::WifiSsid,
            reason: "must contain 1-32 bytes",
        });
    }
    Ok(())
}

fn validate_wifi_credentials(value: &str) -> Result<(), ConfigError> {
    if value.len() > MAX_WIFI_CREDENTIALS_BYTES {
        return Err(ConfigError::InvalidValue {
            field: ConfigField::WifiCredentials,
            reason: "must contain at most 63 bytes",
        });
    }
    Ok(())
}

#[cfg(target_os = "espidf")]
pub mod espidf {
    use esp_idf_svc::nvs::{EspDefaultNvs, EspDefaultNvsPartition, EspNvs};

    use super::{ConfigStore, ConfigStoreError};

    /// Internal NVS adapter. It neither mounts nor references SD storage and
    /// has no logging path, so credential values cannot reach diagnostics.
    pub struct EspNvsConfigStore {
        nvs: EspDefaultNvs,
    }

    impl EspNvsConfigStore {
        pub fn open(partition: EspDefaultNvsPartition) -> Result<Self, ConfigStoreError> {
            EspNvs::new(partition, "atlaslite", true)
                .map(|nvs| Self { nvs })
                .map_err(|_| ConfigStoreError::Backend)
        }
    }

    impl ConfigStore for EspNvsConfigStore {
        fn get(&self, key: &str) -> Result<Option<Vec<u8>>, ConfigStoreError> {
            super::validate_store_key(key)?;
            let length = self
                .nvs
                .blob_len(key)
                .map_err(|_| ConfigStoreError::Backend)?;
            let Some(length) = length else {
                return Ok(None);
            };
            let max_length = super::store_value_limit(key)?;
            if length > max_length {
                return Err(ConfigStoreError::ValueTooLarge {
                    key: key.into(),
                    length,
                });
            }
            let mut value = vec![0; length];
            self.nvs
                .get_blob(key, &mut value)
                .map_err(|_| ConfigStoreError::Backend)?;
            Ok(Some(value))
        }

        fn set(&mut self, key: &str, value: &[u8]) -> Result<(), ConfigStoreError> {
            super::validate_store_key_value(key, value)?;
            self.nvs
                .set_blob(key, value)
                .map_err(|_| ConfigStoreError::Backend)
        }

        fn remove(&mut self, key: &str) -> Result<(), ConfigStoreError> {
            super::validate_store_key(key)?;
            self.nvs
                .remove(key)
                .map(|_| ())
                .map_err(|_| ConfigStoreError::Backend)
        }

        fn clear(&mut self) -> Result<(), ConfigStoreError> {
            self.nvs.erase_all().map_err(|_| ConfigStoreError::Backend)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ConfigRepository, ConfigStore, ConfigStoreError, FakeConfigStore};

    #[test]
    fn fake_store_can_surface_a_backend_failure() {
        let mut store = FakeConfigStore::default();
        store.set_fail_operations(true);
        assert_eq!(store.get("version"), Err(ConfigStoreError::Backend));
        assert!(ConfigRepository::new(store).load().is_err());
    }

    #[test]
    fn config_key_names_fit_esp_nvs_limit() {
        assert_eq!(super::MAX_CONFIG_ENTRIES, 7);
        assert_eq!(super::CONFIG_STORE_KEYS.len(), super::MAX_CONFIG_ENTRIES);
        assert!(super::CONFIG_STORE_KEYS.iter().all(|key| key.len() <= 15));
    }
}
