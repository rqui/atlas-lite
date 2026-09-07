//! Essential product preferences persisted in internal NVS.
//!
//! Removable storage remains a compatibility import/export path, but losing
//! the SD card cannot reset the selected typography or regional profile.

use std::{collections::BTreeMap, error::Error, fmt};

use crate::{
    app::display::{DisplayPreferences, UiFontFamily, UiFontSize},
    regional::{RegionalPreferences, TemperatureUnit, TimeZoneProfile},
};

const SCHEMA: &str = "1";
const VERSION: &str = "version";
const FONT_FAMILY: &str = "font_family";
const FONT_SIZE: &str = "font_size";
const TIMEZONE: &str = "timezone";
const TEMPERATURE: &str = "temp_unit";
const KEYS: [&str; 5] = [VERSION, FONT_FAMILY, FONT_SIZE, TIMEZONE, TEMPERATURE];
const MAX_VALUE_BYTES: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductPreferencesError {
    Backend,
    Corrupt(&'static str),
    UnsupportedSchema,
}

impl fmt::Display for ProductPreferencesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backend => f.write_str("NVS backend failure"),
            Self::Corrupt(key) => write!(f, "invalid product preference {key}"),
            Self::UnsupportedSchema => f.write_str("unsupported product preference schema"),
        }
    }
}

impl Error for ProductPreferencesError {}

pub trait ProductPreferencesStore {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, ProductPreferencesError>;
    fn set(&mut self, key: &str, value: &[u8]) -> Result<(), ProductPreferencesError>;
    fn clear(&mut self) -> Result<(), ProductPreferencesError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductPreferences {
    pub display: DisplayPreferences,
    pub regional: RegionalPreferences,
}

impl Default for ProductPreferences {
    fn default() -> Self {
        Self {
            display: DisplayPreferences::default(),
            regional: RegionalPreferences::default(),
        }
    }
}

pub struct ProductPreferencesRepository<S> {
    store: S,
}

impl<S: ProductPreferencesStore> ProductPreferencesRepository<S> {
    #[must_use]
    pub const fn new(store: S) -> Self {
        Self { store }
    }

    pub fn load(&self) -> Result<Option<ProductPreferences>, ProductPreferencesError> {
        let version = self.read(VERSION)?;
        let values = [
            self.read(FONT_FAMILY)?,
            self.read(FONT_SIZE)?,
            self.read(TIMEZONE)?,
            self.read(TEMPERATURE)?,
        ];
        if version.is_none() && values.iter().all(Option::is_none) {
            return Ok(None);
        }
        if version.as_deref() != Some(SCHEMA) {
            return Err(ProductPreferencesError::UnsupportedSchema);
        }
        let family = match values[0].as_deref() {
            Some("inter") => UiFontFamily::Inter,
            Some("atkinson-hyperlegible") => UiFontFamily::AtkinsonHyperlegible,
            _ => return Err(ProductPreferencesError::Corrupt(FONT_FAMILY)),
        };
        let size = match values[1].as_deref() {
            Some("compact") => UiFontSize::Compact,
            Some("standard") => UiFontSize::Standard,
            Some("large") => UiFontSize::Large,
            _ => return Err(ProductPreferencesError::Corrupt(FONT_SIZE)),
        };
        let timezone = TimeZoneProfile::parse(
            values[2]
                .as_deref()
                .ok_or(ProductPreferencesError::Corrupt(TIMEZONE))?,
        )
        .map_err(|_| ProductPreferencesError::Corrupt(TIMEZONE))?;
        let temperature_unit = TemperatureUnit::parse(
            values[3]
                .as_deref()
                .ok_or(ProductPreferencesError::Corrupt(TEMPERATURE))?,
        )
        .map_err(|_| ProductPreferencesError::Corrupt(TEMPERATURE))?;
        Ok(Some(ProductPreferences {
            display: DisplayPreferences {
                font_family: family,
                font_size: size,
            },
            regional: RegionalPreferences {
                timezone,
                temperature_unit,
                ..RegionalPreferences::default()
            },
        }))
    }

    pub fn save(&mut self, preferences: ProductPreferences) -> Result<(), ProductPreferencesError> {
        self.store.set(
            FONT_FAMILY,
            preferences.display.font_family.marker().as_bytes(),
        )?;
        self.store
            .set(FONT_SIZE, preferences.display.font_size.marker().as_bytes())?;
        self.store
            .set(TIMEZONE, preferences.regional.timezone_name().as_bytes())?;
        self.store.set(
            TEMPERATURE,
            preferences.regional.temperature_unit.marker().as_bytes(),
        )?;
        self.store.set(VERSION, SCHEMA.as_bytes())
    }

    pub fn clear(&mut self) -> Result<(), ProductPreferencesError> {
        self.store.clear()
    }

    fn read(&self, key: &'static str) -> Result<Option<String>, ProductPreferencesError> {
        let value = self.store.get(key)?;
        match value {
            Some(bytes) if bytes.len() > MAX_VALUE_BYTES => {
                Err(ProductPreferencesError::Corrupt(key))
            }
            Some(bytes) => String::from_utf8(bytes)
                .map(Some)
                .map_err(|_| ProductPreferencesError::Corrupt(key)),
            None => Ok(None),
        }
    }
}

#[derive(Default)]
pub struct FakeProductPreferencesStore {
    values: BTreeMap<String, Vec<u8>>,
}

impl ProductPreferencesStore for FakeProductPreferencesStore {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, ProductPreferencesError> {
        if !KEYS.contains(&key) {
            return Err(ProductPreferencesError::Backend);
        }
        Ok(self.values.get(key).cloned())
    }
    fn set(&mut self, key: &str, value: &[u8]) -> Result<(), ProductPreferencesError> {
        if !KEYS.contains(&key) || value.len() > MAX_VALUE_BYTES {
            return Err(ProductPreferencesError::Backend);
        }
        self.values.insert(key.into(), value.into());
        Ok(())
    }
    fn clear(&mut self) -> Result<(), ProductPreferencesError> {
        self.values.clear();
        Ok(())
    }
}

#[cfg(target_os = "espidf")]
pub mod espidf {
    use super::{ProductPreferencesError, ProductPreferencesStore, KEYS, MAX_VALUE_BYTES};
    use esp_idf_svc::nvs::{EspDefaultNvs, EspDefaultNvsPartition, EspNvs};

    pub struct EspNvsProductPreferencesStore {
        nvs: EspDefaultNvs,
    }
    impl EspNvsProductPreferencesStore {
        pub fn open(partition: EspDefaultNvsPartition) -> Result<Self, ProductPreferencesError> {
            EspNvs::new(partition, "atlasui", true)
                .map(|nvs| Self { nvs })
                .map_err(|_| ProductPreferencesError::Backend)
        }
    }
    impl ProductPreferencesStore for EspNvsProductPreferencesStore {
        fn get(&self, key: &str) -> Result<Option<Vec<u8>>, ProductPreferencesError> {
            if !KEYS.contains(&key) {
                return Err(ProductPreferencesError::Backend);
            }
            let Some(length) = self
                .nvs
                .blob_len(key)
                .map_err(|_| ProductPreferencesError::Backend)?
            else {
                return Ok(None);
            };
            if length > MAX_VALUE_BYTES {
                return Err(ProductPreferencesError::Backend);
            }
            let mut value = vec![0; length];
            self.nvs
                .get_blob(key, &mut value)
                .map_err(|_| ProductPreferencesError::Backend)?;
            Ok(Some(value))
        }
        fn set(&mut self, key: &str, value: &[u8]) -> Result<(), ProductPreferencesError> {
            if !KEYS.contains(&key) || value.len() > MAX_VALUE_BYTES {
                return Err(ProductPreferencesError::Backend);
            }
            self.nvs
                .set_blob(key, value)
                .map_err(|_| ProductPreferencesError::Backend)
        }
        fn clear(&mut self) -> Result<(), ProductPreferencesError> {
            self.nvs
                .erase_all()
                .map_err(|_| ProductPreferencesError::Backend)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvs_model_round_trips_display_and_timezone_without_sd() {
        let mut repository =
            ProductPreferencesRepository::new(FakeProductPreferencesStore::default());
        let preferences = ProductPreferences {
            display: DisplayPreferences {
                font_family: UiFontFamily::AtkinsonHyperlegible,
                font_size: UiFontSize::Large,
            },
            regional: RegionalPreferences {
                timezone: TimeZoneProfile::EuropeMadrid,
                temperature_unit: TemperatureUnit::Celsius,
                ..RegionalPreferences::default()
            },
        };
        repository.save(preferences).unwrap();
        assert_eq!(repository.load().unwrap(), Some(preferences));
    }

    #[test]
    fn empty_nvs_is_an_explicit_migration_candidate() {
        let repository = ProductPreferencesRepository::new(FakeProductPreferencesStore::default());
        assert_eq!(repository.load().unwrap(), None);
    }
}
