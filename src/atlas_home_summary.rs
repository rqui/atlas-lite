//! Tiny last-known Home counters persisted in internal NVS.
//!
//! This record intentionally contains no Atlas IDs, titles, note bodies,
//! manifests or other user content.

use std::{error::Error, fmt};

const SCHEMA: u8 = 1;
const KEY: &str = "summary";
const RECORD_BYTES: usize = 6;
const FLAG_LIBRARY: u8 = 1 << 0;
const FLAG_BOOKS: u8 = 1 << 1;
const FLAG_LIBRARY_PARTIAL: u8 = 1 << 2;
const FLAG_BOOKS_PARTIAL: u8 = 1 << 3;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AtlasHomeSummary {
    pub library_roots: Option<u8>,
    pub library_notes: Option<u8>,
    pub library_partial: bool,
    pub books_count: Option<u8>,
    pub books_partial: bool,
    pub resume_percentage: Option<u8>,
}

impl AtlasHomeSummary {
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.library_roots.is_none() && self.books_count.is_none()
    }

    fn encode(self) -> Result<[u8; RECORD_BYTES], AtlasHomeSummaryError> {
        if self.library_roots.is_some() != self.library_notes.is_some()
            || self.resume_percentage.is_some_and(|value| value > 100)
        {
            return Err(AtlasHomeSummaryError::Corrupt);
        }
        let mut flags = 0;
        if self.library_roots.is_some() {
            flags |= FLAG_LIBRARY;
        }
        if self.books_count.is_some() {
            flags |= FLAG_BOOKS;
        }
        if self.library_partial {
            flags |= FLAG_LIBRARY_PARTIAL;
        }
        if self.books_partial {
            flags |= FLAG_BOOKS_PARTIAL;
        }
        Ok([
            SCHEMA,
            flags,
            self.library_roots.unwrap_or(0),
            self.library_notes.unwrap_or(0),
            self.books_count.unwrap_or(0),
            self.resume_percentage.unwrap_or(u8::MAX),
        ])
    }

    fn decode(record: &[u8]) -> Result<Self, AtlasHomeSummaryError> {
        if record.len() != RECORD_BYTES || record[0] != SCHEMA {
            return Err(AtlasHomeSummaryError::UnsupportedSchema);
        }
        let flags = record[1];
        let summary = Self {
            library_roots: (flags & FLAG_LIBRARY != 0).then_some(record[2]),
            library_notes: (flags & FLAG_LIBRARY != 0).then_some(record[3]),
            library_partial: flags & FLAG_LIBRARY_PARTIAL != 0,
            books_count: (flags & FLAG_BOOKS != 0).then_some(record[4]),
            books_partial: flags & FLAG_BOOKS_PARTIAL != 0,
            resume_percentage: (record[5] != u8::MAX).then_some(record[5]),
        };
        summary.encode()?;
        Ok(summary)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtlasHomeSummaryError {
    Backend,
    Corrupt,
    UnsupportedSchema,
}

impl fmt::Display for AtlasHomeSummaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backend => formatter.write_str("Home summary NVS backend failure"),
            Self::Corrupt => formatter.write_str("invalid Home summary"),
            Self::UnsupportedSchema => formatter.write_str("unsupported Home summary schema"),
        }
    }
}

impl Error for AtlasHomeSummaryError {}

pub trait AtlasHomeSummaryStore {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, AtlasHomeSummaryError>;
    fn set(&mut self, key: &str, value: &[u8]) -> Result<(), AtlasHomeSummaryError>;
    fn clear(&mut self) -> Result<(), AtlasHomeSummaryError>;
}

pub struct AtlasHomeSummaryRepository<S> {
    store: S,
}

impl<S: AtlasHomeSummaryStore> AtlasHomeSummaryRepository<S> {
    #[must_use]
    pub const fn new(store: S) -> Self {
        Self { store }
    }

    pub fn load(&self) -> Result<Option<AtlasHomeSummary>, AtlasHomeSummaryError> {
        self.store
            .get(KEY)?
            .map(|record| AtlasHomeSummary::decode(&record))
            .transpose()
    }

    /// Persist only when the bounded record changed. Returns true on a write.
    pub fn save_if_changed(
        &mut self,
        summary: AtlasHomeSummary,
    ) -> Result<bool, AtlasHomeSummaryError> {
        let record = summary.encode()?;
        if self.store.get(KEY)?.as_deref() == Some(record.as_slice()) {
            return Ok(false);
        }
        self.store.set(KEY, &record)?;
        Ok(true)
    }

    pub fn clear(&mut self) -> Result<(), AtlasHomeSummaryError> {
        self.store.clear()
    }

    #[cfg(test)]
    fn into_store(self) -> S {
        self.store
    }
}

#[cfg(target_os = "espidf")]
pub mod espidf {
    use super::{AtlasHomeSummaryError, AtlasHomeSummaryStore, KEY, RECORD_BYTES};
    use esp_idf_svc::nvs::{EspDefaultNvs, EspDefaultNvsPartition, EspNvs};

    pub struct EspNvsAtlasHomeSummaryStore {
        nvs: EspDefaultNvs,
    }

    impl EspNvsAtlasHomeSummaryStore {
        pub fn open(partition: EspDefaultNvsPartition) -> Result<Self, AtlasHomeSummaryError> {
            EspNvs::new(partition, "atlashome", true)
                .map(|nvs| Self { nvs })
                .map_err(|_| AtlasHomeSummaryError::Backend)
        }
    }

    impl AtlasHomeSummaryStore for EspNvsAtlasHomeSummaryStore {
        fn get(&self, key: &str) -> Result<Option<Vec<u8>>, AtlasHomeSummaryError> {
            if key != KEY {
                return Err(AtlasHomeSummaryError::Backend);
            }
            let Some(length) = self
                .nvs
                .blob_len(key)
                .map_err(|_| AtlasHomeSummaryError::Backend)?
            else {
                return Ok(None);
            };
            if length != RECORD_BYTES {
                return Err(AtlasHomeSummaryError::Corrupt);
            }
            let mut record = vec![0; length];
            self.nvs
                .get_blob(key, &mut record)
                .map_err(|_| AtlasHomeSummaryError::Backend)?;
            Ok(Some(record))
        }

        fn set(&mut self, key: &str, value: &[u8]) -> Result<(), AtlasHomeSummaryError> {
            if key != KEY || value.len() != RECORD_BYTES {
                return Err(AtlasHomeSummaryError::Backend);
            }
            self.nvs
                .set_blob(key, value)
                .map_err(|_| AtlasHomeSummaryError::Backend)
        }

        fn clear(&mut self) -> Result<(), AtlasHomeSummaryError> {
            self.nvs
                .erase_all()
                .map_err(|_| AtlasHomeSummaryError::Backend)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[derive(Default)]
    struct FakeStore {
        values: BTreeMap<String, Vec<u8>>,
        writes: usize,
    }

    impl AtlasHomeSummaryStore for FakeStore {
        fn get(&self, key: &str) -> Result<Option<Vec<u8>>, AtlasHomeSummaryError> {
            Ok(self.values.get(key).cloned())
        }

        fn set(&mut self, key: &str, value: &[u8]) -> Result<(), AtlasHomeSummaryError> {
            self.values.insert(key.into(), value.into());
            self.writes += 1;
            Ok(())
        }

        fn clear(&mut self) -> Result<(), AtlasHomeSummaryError> {
            self.values.clear();
            Ok(())
        }
    }

    fn fixture() -> AtlasHomeSummary {
        AtlasHomeSummary {
            library_roots: Some(4),
            library_notes: Some(19),
            library_partial: false,
            books_count: Some(12),
            books_partial: true,
            resume_percentage: Some(68),
        }
    }

    #[test]
    fn nvs_summary_round_trips_without_user_content() {
        let mut repository = AtlasHomeSummaryRepository::new(FakeStore::default());
        assert!(repository.save_if_changed(fixture()).unwrap());
        assert_eq!(repository.load().unwrap(), Some(fixture()));
    }

    #[test]
    fn unchanged_network_summary_does_not_write_nvs_again() {
        let mut repository = AtlasHomeSummaryRepository::new(FakeStore::default());
        assert!(repository.save_if_changed(fixture()).unwrap());
        assert!(!repository.save_if_changed(fixture()).unwrap());
        assert_eq!(repository.into_store().writes, 1);
    }
}
