//! Atlas Lite configuration kept separate from removable-storage features.
//!
//! The configuration domain intentionally has no serialization implementation:
//! callers must select individual values instead of dumping an object that
//! could include an Atlas token or Wi-Fi credential.

use std::{collections::BTreeMap, error::Error, fmt};

pub const CONFIG_SCHEMA_VERSION: &str = "1";
pub const MAX_DEVICE_ID_BYTES: usize = 64;
pub const MAX_ATLAS_URL_BYTES: usize = 192;
pub const MAX_API_TOKEN_BYTES: usize = 512;
pub const MAX_WIFI_SSID_BYTES: usize = 32;
pub const MAX_WIFI_CREDENTIALS_BYTES: usize = 63;
pub const MAX_CONFIG_VALUE_BYTES: usize = MAX_API_TOKEN_BYTES;

const VERSION_KEY: &str = "version";
const DEVICE_ID_KEY: &str = "device_id";
const ATLAS_URL_KEY: &str = "atlas_url";
const API_TOKEN_KEY: &str = "api_token";
const WIFI_SSID_KEY: &str = "wifi_ssid";
const WIFI_CREDENTIALS_KEY: &str = "wifi_cred";
pub const CONFIG_STORE_KEYS: [&str; 6] = [
    VERSION_KEY,
    DEVICE_ID_KEY,
    ATLAS_URL_KEY,
    API_TOKEN_KEY,
    WIFI_SSID_KEY,
    WIFI_CREDENTIALS_KEY,
];

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
            Self::UnsupportedSchema { found } => {
                write!(formatter, "unsupported configuration schema {found}")
            }
            Self::Store(error) => write!(formatter, "configuration store error: {error}"),
        }
    }
}

impl Error for ConfigError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigStoreError {
    ValueTooLarge { key: String, length: usize },
    Backend,
}

impl fmt::Display for ConfigStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
    pub fn insert_raw(&mut self, key: &str, value: &[u8]) {
        self.values.insert(key.into(), value.into());
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
        Ok(self.values.get(key).cloned())
    }

    fn set(&mut self, key: &str, value: &[u8]) -> Result<(), ConfigStoreError> {
        if self.fail_operations {
            return Err(ConfigStoreError::Backend);
        }
        if value.len() > MAX_CONFIG_VALUE_BYTES {
            return Err(ConfigStoreError::ValueTooLarge {
                key: key.into(),
                length: value.len(),
            });
        }
        self.values.insert(key.into(), value.into());
        Ok(())
    }

    fn remove(&mut self, key: &str) -> Result<(), ConfigStoreError> {
        if self.fail_operations {
            return Err(ConfigStoreError::Backend);
        }
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

fn normalize_atlas_url(value: String) -> Result<String, ConfigError> {
    if value.is_empty()
        || value.len() > MAX_ATLAS_URL_BYTES
        || value.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return Err(ConfigError::InvalidValue {
            field: ConfigField::AtlasUrl,
            reason: "must be a bounded URL without whitespace",
        });
    }
    let remainder = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
        .ok_or(ConfigError::InvalidValue {
            field: ConfigField::AtlasUrl,
            reason: "must use http or https",
        })?;
    if remainder.is_empty()
        || remainder.contains('@')
        || remainder.contains('?')
        || remainder.contains('#')
    {
        return Err(ConfigError::InvalidValue {
            field: ConfigField::AtlasUrl,
            reason: "must not include credentials, query, or fragment",
        });
    }
    let (authority, path) = remainder.split_once('/').unwrap_or((remainder, ""));
    if authority.is_empty() {
        return Err(ConfigError::InvalidValue {
            field: ConfigField::AtlasUrl,
            reason: "must include an authority",
        });
    }
    if !path.is_empty() {
        return Err(ConfigError::InvalidValue {
            field: ConfigField::AtlasUrl,
            reason: "must be an Atlas base URL without a path",
        });
    }
    Ok(value.trim_end_matches('/').into())
}

fn validate_api_token(value: &str) -> Result<(), ConfigError> {
    if value.is_empty()
        || value.len() > MAX_API_TOKEN_BYTES
        || value.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return Err(ConfigError::InvalidValue {
            field: ConfigField::ApiToken,
            reason: "must be a bounded non-whitespace token",
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

    use super::{ConfigStore, ConfigStoreError, MAX_CONFIG_VALUE_BYTES};

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
            let length = self
                .nvs
                .blob_len(key)
                .map_err(|_| ConfigStoreError::Backend)?;
            let Some(length) = length else {
                return Ok(None);
            };
            if length > MAX_CONFIG_VALUE_BYTES {
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
            if value.len() > MAX_CONFIG_VALUE_BYTES {
                return Err(ConfigStoreError::ValueTooLarge {
                    key: key.into(),
                    length: value.len(),
                });
            }
            self.nvs
                .set_blob(key, value)
                .map_err(|_| ConfigStoreError::Backend)
        }

        fn remove(&mut self, key: &str) -> Result<(), ConfigStoreError> {
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
        assert!(super::CONFIG_STORE_KEYS.iter().all(|key| key.len() <= 15));
    }
}
