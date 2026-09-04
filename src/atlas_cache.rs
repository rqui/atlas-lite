//! Typed, bounded offline read cache built on [`crate::atlas_storage`].
//!
//! Atlas remains authoritative: this module only persists deliberately small
//! read snapshots and reports cache data as local/stale. It owns no transport,
//! renderer, mutation queue, credentials, or filesystem paths from Atlas.

use std::{collections::BTreeSet, fmt};

use serde::{Deserialize, Serialize};

use crate::{
    atlas_client::MAX_SEARCH_QUERY_BYTES,
    atlas_dto::{
        AtlasNoteDocument, NoteSummaryPage, SearchResponse, ViewResultPage, MAX_NOTE_SUMMARIES,
        MAX_SEARCH_HITS, MAX_VIEW_RESULTS,
    },
    atlas_note::{
        MAX_ATLAS_NOTE_BODY_BYTES, MAX_ATLAS_NOTE_REVISION_BYTES, MAX_ATLAS_NOTE_TITLE_BYTES,
    },
    atlas_storage::{AtlasDirectory, AtlasEntryDisposition, AtlasStorage, AtlasStorageError},
    atlas_views::{VIEW_PATH_MAX_BYTES, VIEW_TITLE_MAX_BYTES},
};

/// Schema accepted by this firmware. Unknown versions are cache misses, never
/// interpreted as current data.
pub const ATLAS_CACHE_SCHEMA_VERSION: u8 = 1;
/// A fixed operational bound independent from the SD byte budget.
pub const MAX_CACHE_RECORDS: usize = 32;
const MAX_CACHE_KEY_BYTES: usize = 256;
const MAX_SOURCE_TIMESTAMP: u64 = 4_102_444_800; // 2100-01-01 UTC

/// Metadata retained with every cache record. The caller supplies a bounded
/// monotonic `last_used` value, making LRU ordering deterministic in host tests
/// and independent of an RTC being available.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AtlasCacheMetadata {
    pub source_revision: Option<String>,
    pub source_timestamp: Option<u64>,
    pub last_used: u64,
}

/// Explicit cache state for one surface. These values are intentionally not a
/// global connectivity state: Home, Note, Search, and Views remain independent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtlasOfflineStatus {
    Online,
    Syncing,
    OfflineCached,
    OfflineNoData,
    Error,
}

/// A typed offline read result. `OfflineCached` is always stale/local and must
/// never be presented as server authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasOfflineRead<T> {
    pub status: AtlasOfflineStatus,
    pub value: Option<T>,
    pub metadata: Option<AtlasCacheMetadata>,
}

impl<T> AtlasOfflineRead<T> {
    fn cached(value: T, metadata: AtlasCacheMetadata) -> Self {
        Self {
            status: AtlasOfflineStatus::OfflineCached,
            value: Some(value),
            metadata: Some(metadata),
        }
    }

    fn no_data() -> Self {
        Self {
            status: AtlasOfflineStatus::OfflineNoData,
            value: None,
            metadata: None,
        }
    }

    fn error() -> Self {
        Self {
            status: AtlasOfflineStatus::Error,
            value: None,
            metadata: None,
        }
    }
}

/// Cache errors intentionally contain neither serialized payload bytes nor
/// user content. A malformed record is isolated to that lookup.
#[derive(Debug)]
pub enum AtlasCacheError {
    Storage(AtlasStorageError),
    Serialize,
    InvalidRecord,
    InvalidMetadata,
    InvalidKey,
    RecordLimit,
    UntrustedInventory,
}

impl fmt::Display for AtlasCacheError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => write!(formatter, "Atlas cache storage error: {error}"),
            Self::Serialize => formatter.write_str("Atlas cache record serialization failed"),
            Self::InvalidRecord => formatter.write_str("Atlas cache record is invalid"),
            Self::InvalidMetadata => formatter.write_str("Atlas cache metadata is invalid"),
            Self::InvalidKey => formatter.write_str("Atlas cache key is invalid"),
            Self::RecordLimit => formatter.write_str("Atlas cache record limit reached"),
            Self::UntrustedInventory => {
                formatter.write_str("Atlas cache inventory is incomplete or untrusted")
            }
        }
    }
}

impl std::error::Error for AtlasCacheError {}

impl From<AtlasStorageError> for AtlasCacheError {
    fn from(error: AtlasStorageError) -> Self {
        Self::Storage(error)
    }
}

/// Typed repository; all writes use the M5.1 atomic/integrity storage seam.
#[derive(Clone, Debug)]
pub struct AtlasCacheRepository {
    storage: AtlasStorage,
    max_records: usize,
}

impl AtlasCacheRepository {
    pub fn new(storage: AtlasStorage) -> Self {
        Self::with_record_limit(storage, MAX_CACHE_RECORDS)
    }

    pub fn with_record_limit(storage: AtlasStorage, max_records: usize) -> Self {
        Self {
            storage,
            max_records: max_records.max(1).min(MAX_CACHE_RECORDS),
        }
    }

    pub fn store_home(
        &self,
        page: NoteSummaryPage,
        metadata: AtlasCacheMetadata,
    ) -> Result<(), AtlasCacheError> {
        self.store(CachePayload::Home(page), "HOME", metadata)
    }

    pub fn store_note(
        &self,
        document: AtlasNoteDocument,
        metadata: AtlasCacheMetadata,
    ) -> Result<(), AtlasCacheError> {
        let id = document.id.clone().ok_or(AtlasCacheError::InvalidKey)?;
        self.store(CachePayload::Note(document), &id, metadata)
    }

    pub fn store_views(
        &self,
        page: ViewResultPage,
        requested_cursor: Option<&str>,
        metadata: AtlasCacheMetadata,
    ) -> Result<(), AtlasCacheError> {
        let key = view_key(&page.view.id, requested_cursor);
        self.store(CachePayload::Views(page), &key, metadata)
    }

    pub fn store_search(
        &self,
        response: SearchResponse,
        metadata: AtlasCacheMetadata,
    ) -> Result<(), AtlasCacheError> {
        let key = response.query.clone();
        self.store(CachePayload::Search(response), &key, metadata)
    }

    pub fn offline_home(&self) -> AtlasOfflineRead<NoteSummaryPage> {
        self.offline_typed(AtlasDirectory::CacheHome, "HOME", |payload| match payload {
            CachePayload::Home(page) => Some(page),
            _ => None,
        })
    }

    pub fn offline_note(&self, id: &str) -> AtlasOfflineRead<AtlasNoteDocument> {
        self.offline_typed(AtlasDirectory::CacheNotes, id, |payload| match payload {
            CachePayload::Note(document) => Some(document),
            _ => None,
        })
    }

    pub fn offline_views(
        &self,
        view_id: &str,
        cursor: Option<&str>,
    ) -> AtlasOfflineRead<ViewResultPage> {
        let key = format!("{view_id}\u{1f}{}", cursor.unwrap_or_default());
        self.offline_typed(AtlasDirectory::CacheViews, &key, |payload| match payload {
            CachePayload::Views(page) => Some(page),
            _ => None,
        })
    }

    pub fn offline_search(&self, query: &str) -> AtlasOfflineRead<SearchResponse> {
        self.offline_typed(
            AtlasDirectory::CacheSearch,
            query,
            |payload| match payload {
                CachePayload::Search(response) => Some(response),
                _ => None,
            },
        )
    }

    fn offline_typed<T>(
        &self,
        directory: AtlasDirectory,
        key: &str,
        extract: impl FnOnce(CachePayload) -> Option<T>,
    ) -> AtlasOfflineRead<T> {
        match self.lookup(directory, key) {
            Ok(Some(record)) => {
                let record = self.touch(record).unwrap_or_else(|record| record);
                extract(record.record.payload)
                    .map(|value| AtlasOfflineRead::cached(value, record.record.metadata))
                    .unwrap_or_else(AtlasOfflineRead::error)
            }
            Ok(None) => AtlasOfflineRead::no_data(),
            Err(_) => AtlasOfflineRead::error(),
        }
    }

    fn store(
        &self,
        payload: CachePayload,
        key: &str,
        metadata: AtlasCacheMetadata,
    ) -> Result<(), AtlasCacheError> {
        validate_key(key)?;
        validate_metadata(&metadata)?;
        validate_payload(&payload)?;
        let directory = payload.directory();
        let inventory = self.inventory()?;
        if inventory.untrusted {
            return Err(AtlasCacheError::UntrustedInventory);
        }
        let entries = inventory.entries;
        let existing = entries
            .iter()
            .find(|entry| entry.record.key == key && entry.directory == directory)
            .map(|entry| entry.name.clone());
        let record = CacheRecord {
            schema_version: ATLAS_CACHE_SCHEMA_VERSION,
            key: key.into(),
            metadata,
            payload,
        };
        let bytes = serde_json::to_vec(&record).map_err(|_| AtlasCacheError::Serialize)?;
        // DTO bounds do not guarantee their JSON representation fits the
        // storage limit. Reject it before selecting a record-limit victim.
        self.storage.check_file_bytes(&bytes)?;

        match existing {
            Some(name) => self.storage.replace_bytes(directory, &name, &bytes)?,
            None if entries.len() < self.max_records => {
                let name = self.allocate_name(directory, key, &entries)?;
                match self.storage.replace_bytes(directory, &name, &bytes) {
                    Ok(()) => {}
                    Err(AtlasStorageError::CacheBudgetExceeded { .. }) => {
                        let victim =
                            eviction_victim(&entries).ok_or(AtlasCacheError::RecordLimit)?;
                        self.storage.replace_cache_eviction_bytes(
                            victim.directory,
                            &victim.name,
                            directory,
                            &name,
                            &bytes,
                        )?;
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            None => {
                let victim = eviction_victim(&entries).ok_or(AtlasCacheError::RecordLimit)?;
                let name = self.allocate_name(directory, key, &entries)?;
                self.storage.replace_cache_eviction_bytes(
                    victim.directory,
                    &victim.name,
                    directory,
                    &name,
                    &bytes,
                )?;
            }
        }
        Ok(())
    }

    fn lookup(
        &self,
        directory: AtlasDirectory,
        key: &str,
    ) -> Result<Option<CacheEntry>, AtlasCacheError> {
        validate_key(key)?;
        let inventory = self.inventory()?;
        if inventory.untrusted {
            return Err(AtlasCacheError::UntrustedInventory);
        }
        for entry in inventory.entries {
            if entry.directory == directory && entry.record.key == key {
                return Ok(Some(entry));
            }
        }
        Ok(None)
    }

    fn touch(&self, mut entry: CacheEntry) -> Result<CacheEntry, CacheEntry> {
        let Ok(inventory) = self.inventory() else {
            return Err(entry);
        };
        if inventory.untrusted {
            return Err(entry);
        }
        entry.record.metadata.last_used = inventory
            .entries
            .iter()
            .map(|candidate| candidate.record.metadata.last_used)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let Ok(bytes) = serde_json::to_vec(&entry.record) else {
            return Err(entry);
        };
        if self
            .storage
            .replace_bytes(entry.directory, &entry.name, &bytes)
            .is_err()
        {
            return Err(entry);
        }
        Ok(entry)
    }

    fn inventory(&self) -> Result<CacheInventory, AtlasCacheError> {
        self.storage.recover_cache_eviction()?;
        let mut records = Vec::new();
        let mut untrusted = false;
        for directory in cache_directories() {
            let mut listing = self.storage.list(directory)?;
            let recovered = listing
                .recovery_artifacts
                .iter()
                .filter_map(|entry| recovery_primary_name(&entry.name))
                .try_fold(false, |recovered, name| {
                    self.storage
                        .recover_replacement_group(directory, &name)
                        .map(|did_recover| recovered || did_recover)
                })?;
            if recovered {
                listing = self.storage.list(directory)?;
            }
            untrusted |= listing.inspection_incomplete
                || listing.omitted_entries != 0
                || listing.recovery_artifacts_omitted != 0
                || listing.corrupt_entries != 0
                || listing.unknown_entries != 0;
            let mut primary_names = BTreeSet::new();
            for entry in listing.entries {
                match entry.disposition {
                    AtlasEntryDisposition::Ready => {
                        primary_names.insert(entry.name);
                    }
                    AtlasEntryDisposition::RecoveryArtifact => {
                        untrusted = true;
                        if let Some(primary) = recovery_primary_name(&entry.name) {
                            primary_names.insert(primary);
                        }
                    }
                    AtlasEntryDisposition::Corrupt | AtlasEntryDisposition::Unknown => {
                        untrusted = true;
                    }
                }
            }
            for name in primary_names {
                let Ok(bytes) = self.storage.read_bytes(directory, &name) else {
                    untrusted = true;
                    continue;
                };
                let Ok(record) = serde_json::from_slice::<CacheRecord>(&bytes) else {
                    untrusted = true;
                    continue;
                };
                if !record.is_valid_for(directory) {
                    untrusted = true;
                    continue;
                }
                records.push(CacheEntry {
                    directory,
                    name,
                    record,
                });
            }
        }
        Ok(CacheInventory {
            entries: records,
            untrusted,
        })
    }

    fn allocate_name(
        &self,
        directory: AtlasDirectory,
        key: &str,
        entries: &[CacheEntry],
    ) -> Result<String, AtlasCacheError> {
        let occupied: BTreeSet<String> = self
            .storage
            .list(directory)?
            .entries
            .into_iter()
            .map(|entry| entry.name)
            .collect();
        for probe in 0..MAX_CACHE_RECORDS {
            let name = format!("{:08X}.DAT", hash_name(key, probe));
            if !occupied.contains(&name) && !entries.iter().any(|entry| entry.name == name) {
                return Ok(name);
            }
        }
        Err(AtlasCacheError::RecordLimit)
    }
}

#[derive(Serialize, Deserialize)]
struct CacheRecord {
    schema_version: u8,
    key: String,
    metadata: AtlasCacheMetadata,
    payload: CachePayload,
}

impl CacheRecord {
    fn is_valid_for(&self, directory: AtlasDirectory) -> bool {
        self.schema_version == ATLAS_CACHE_SCHEMA_VERSION
            && validate_key(&self.key).is_ok()
            && validate_metadata(&self.metadata).is_ok()
            && self.payload.directory() == directory
            && validate_payload(&self.payload).is_ok()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
enum CachePayload {
    Home(NoteSummaryPage),
    Note(AtlasNoteDocument),
    Views(ViewResultPage),
    Search(SearchResponse),
}

impl CachePayload {
    const fn directory(&self) -> AtlasDirectory {
        match self {
            Self::Home(_) => AtlasDirectory::CacheHome,
            Self::Note(_) => AtlasDirectory::CacheNotes,
            Self::Views(_) => AtlasDirectory::CacheViews,
            Self::Search(_) => AtlasDirectory::CacheSearch,
        }
    }
}

struct CacheEntry {
    directory: AtlasDirectory,
    name: String,
    record: CacheRecord,
}

struct CacheInventory {
    entries: Vec<CacheEntry>,
    untrusted: bool,
}

fn cache_directories() -> [AtlasDirectory; 4] {
    [
        AtlasDirectory::CacheHome,
        AtlasDirectory::CacheNotes,
        AtlasDirectory::CacheViews,
        AtlasDirectory::CacheSearch,
    ]
}

fn view_key(view_id: &str, requested_cursor: Option<&str>) -> String {
    format!("{view_id}\u{1f}{}", requested_cursor.unwrap_or_default())
}

fn hash_name(key: &str, probe: usize) -> u32 {
    let mut hash = 0x811c_9dc5_u32;
    for byte in key.bytes().chain(probe.to_le_bytes()) {
        hash = (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193);
    }
    hash
}

/// Cache records only own the fixed eight-hex-digit names generated by
/// `allocate_name`; recovery artifacts for any other FAT name stay preserved.
fn recovery_primary_name(name: &str) -> Option<String> {
    let stem = name.strip_suffix(".BAK")?;
    if stem.len() == 8 && stem.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Some(format!("{stem}.DAT"))
    } else {
        None
    }
}

fn eviction_victim(entries: &[CacheEntry]) -> Option<&CacheEntry> {
    entries.iter().min_by(|left, right| {
        (left.record.metadata.last_used, left.name.as_str())
            .cmp(&(right.record.metadata.last_used, right.name.as_str()))
    })
}

fn validate_key(key: &str) -> Result<(), AtlasCacheError> {
    if key.is_empty() || key.len() > MAX_CACHE_KEY_BYTES || key.contains('\0') {
        return Err(AtlasCacheError::InvalidKey);
    }
    Ok(())
}

fn validate_metadata(metadata: &AtlasCacheMetadata) -> Result<(), AtlasCacheError> {
    if metadata.source_revision.as_deref().is_some_and(|revision| {
        revision.is_empty() || revision.len() > MAX_ATLAS_NOTE_REVISION_BYTES
    }) || metadata
        .source_timestamp
        .is_some_and(|timestamp| timestamp > MAX_SOURCE_TIMESTAMP)
    {
        return Err(AtlasCacheError::InvalidMetadata);
    }
    Ok(())
}

fn validate_payload(payload: &CachePayload) -> Result<(), AtlasCacheError> {
    match payload {
        CachePayload::Home(page) => {
            if page.items.len() > MAX_NOTE_SUMMARIES
                || page.next_cursor.as_deref().is_some_and(too_long)
            {
                return Err(AtlasCacheError::InvalidRecord);
            }
            for item in &page.items {
                if too_long(&item.path)
                    || too_long(&item.title)
                    || too_long(&item.revision)
                    || item.id.as_deref().is_some_and(too_long)
                    || item.parent_id.as_deref().is_some_and(too_long)
                    || item.order.as_deref().is_some_and(too_long)
                {
                    return Err(AtlasCacheError::InvalidRecord);
                }
            }
        }
        CachePayload::Note(document) => {
            if document
                .id
                .as_deref()
                .is_none_or(|id| id.is_empty() || too_long(id))
                || document.title.is_empty()
                || document.title.len() > MAX_ATLAS_NOTE_TITLE_BYTES
                || document.revision.is_empty()
                || document.revision.len() > MAX_ATLAS_NOTE_REVISION_BYTES
                || document.body.len() > MAX_ATLAS_NOTE_BODY_BYTES
                || document.parent_id.as_deref().is_some_and(too_long)
                || document.order.as_deref().is_some_and(too_long)
            {
                return Err(AtlasCacheError::InvalidRecord);
            }
        }
        CachePayload::Views(page) => {
            if page.items.len() > MAX_VIEW_RESULTS
                || too_long(&page.view.id)
                || too_long(&page.view.name)
                || too_long(&page.view.revision)
                || page.next_cursor.as_deref().is_some_and(too_long)
            {
                return Err(AtlasCacheError::InvalidRecord);
            }
            for item in &page.items {
                if item
                    .id
                    .as_deref()
                    .is_none_or(|id| id.is_empty() || too_long(id))
                    || item.title.len() > VIEW_TITLE_MAX_BYTES
                    || item.path.len() > VIEW_PATH_MAX_BYTES
                    || too_long(&item.revision)
                {
                    return Err(AtlasCacheError::InvalidRecord);
                }
            }
        }
        CachePayload::Search(response) => {
            if response.query.is_empty()
                || response.query.len() > MAX_SEARCH_QUERY_BYTES
                || response.hits.len() > MAX_SEARCH_HITS
            {
                return Err(AtlasCacheError::InvalidRecord);
            }
            for hit in &response.hits {
                if hit
                    .id
                    .as_deref()
                    .is_none_or(|id| id.is_empty() || too_long(id))
                    || hit.title.len() > VIEW_TITLE_MAX_BYTES
                    || hit.snippet.len() > MAX_ATLAS_NOTE_BODY_BYTES
                    || too_long(&hit.path)
                    || too_long(&hit.revision)
                {
                    return Err(AtlasCacheError::InvalidRecord);
                }
            }
        }
    }
    Ok(())
}

fn too_long(value: &str) -> bool {
    value.is_empty() || value.len() > MAX_CACHE_KEY_BYTES
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::{
        atlas_dto::{NoteState, SearchHit, ViewLayout, ViewResult, ViewStatus, ViewSummary},
        atlas_storage::AtlasStorageLimits,
    };

    fn root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "atlas-cache-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn repository(label: &str, records: usize) -> (AtlasCacheRepository, PathBuf) {
        let root = root(label);
        let storage = AtlasStorage::with_limits(
            root.clone(),
            AtlasStorageLimits {
                max_file_bytes: 32 * 1024,
                max_cache_bytes: 256 * 1024,
                max_directory_entries: 64,
            },
        )
        .unwrap();
        (
            AtlasCacheRepository::with_record_limit(storage, records),
            root,
        )
    }

    fn metadata(last_used: u64) -> AtlasCacheMetadata {
        AtlasCacheMetadata {
            source_revision: Some("r1".into()),
            source_timestamp: Some(1),
            last_used,
        }
    }

    fn note(id: &str) -> AtlasNoteDocument {
        AtlasNoteDocument {
            id: Some(id.into()),
            title: "Cached note".into(),
            revision: "r1".into(),
            body: "# cached".into(),
            parent_id: None,
            order: None,
        }
    }

    fn search(query: &str) -> SearchResponse {
        SearchResponse {
            query: query.into(),
            total: 1,
            hits: vec![SearchHit {
                id: Some("note-1".into()),
                path: "note.md".into(),
                title: "Cached".into(),
                snippet: "cached snippet".into(),
                revision: "r1".into(),
                state: Some(NoteState::Managed),
            }],
        }
    }

    #[test]
    fn note_hit_miss_and_metadata_round_trip_are_typed_and_stale() {
        let (repository, root) = repository("hit", 4);
        repository.store_note(note("note-1"), metadata(7)).unwrap();
        let hit = repository.offline_note("note-1");
        assert_eq!(hit.status, AtlasOfflineStatus::OfflineCached);
        assert_eq!(hit.value.unwrap().body, "# cached");
        assert_eq!(hit.metadata.unwrap(), metadata(8));
        assert_eq!(
            repository.offline_note("missing").status,
            AtlasOfflineStatus::OfflineNoData
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn corrupt_or_truncated_record_fails_closed_for_offline_reads() {
        let (repository, root) = repository("corrupt", 4);
        repository.store_note(note("note-1"), metadata(1)).unwrap();
        let directory = root.join("CACHE/NOTES");
        let file = fs::read_dir(&directory)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        fs::write(file, b"ATLS\x01\x02").unwrap();
        assert_eq!(
            repository.offline_note("note-1").status,
            AtlasOfflineStatus::Error
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_unbounded_payload_fields_and_items() {
        let (repository, root) = repository("bounds", 4);
        let mut oversized = note("note-1");
        oversized.body = "x".repeat(MAX_ATLAS_NOTE_BODY_BYTES + 1);
        assert!(matches!(
            repository.store_note(oversized, metadata(1)),
            Err(AtlasCacheError::InvalidRecord)
        ));
        let mut response = search("query");
        response.hits = (0..=MAX_SEARCH_HITS)
            .map(|index| SearchHit {
                id: Some(format!("id-{index}")),
                path: "n.md".into(),
                title: "n".into(),
                snippet: "s".into(),
                revision: "r".into(),
                state: None,
            })
            .collect();
        assert!(matches!(
            repository.store_search(response, metadata(2)),
            Err(AtlasCacheError::InvalidRecord)
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn deterministic_lru_eviction_breaks_ties_by_fat_name() {
        let (repository, root) = repository("evict", 2);
        repository.store_note(note("first"), metadata(1)).unwrap();
        repository.store_note(note("second"), metadata(1)).unwrap();
        let expected = ["first", "second"]
            .into_iter()
            .min_by_key(|key| hash_name(key, 0))
            .unwrap();
        repository.store_note(note("third"), metadata(2)).unwrap();
        assert_eq!(
            repository.offline_note(expected).status,
            AtlasOfflineStatus::OfflineNoData
        );
        assert_eq!(
            repository.offline_note("third").status,
            AtlasOfflineStatus::OfflineCached
        );
        assert_eq!(fs::read_dir(root.join("CACHE/NOTES")).unwrap().count(), 2);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn record_limit_evicts_global_lru_across_cache_surfaces() {
        let (repository, root) = repository("global-evict", 2);
        repository
            .store_note(note("old-note"), metadata(1))
            .unwrap();
        repository
            .store_search(search("newer-search"), metadata(2))
            .unwrap();

        repository
            .store_search(search("newest-search"), metadata(3))
            .unwrap();

        assert_eq!(
            repository.offline_note("old-note").status,
            AtlasOfflineStatus::OfflineNoData
        );
        assert_eq!(
            repository.offline_search("newer-search").status,
            AtlasOfflineStatus::OfflineCached
        );
        assert_eq!(
            repository.offline_search("newest-search").status,
            AtlasOfflineStatus::OfflineCached
        );
        assert_eq!(fs::read_dir(root.join("CACHE/NOTES")).unwrap().count(), 0);
        assert_eq!(fs::read_dir(root.join("CACHE/SEARCH")).unwrap().count(), 2);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn byte_budget_evicts_global_lru_before_record_limit() {
        let root = root("byte-evict");
        let initial_storage = AtlasStorage::with_limits(
            root.clone(),
            AtlasStorageLimits {
                max_file_bytes: 32 * 1024,
                max_cache_bytes: 256 * 1024,
                max_directory_entries: 16,
            },
        )
        .unwrap();
        let initial = AtlasCacheRepository::with_record_limit(initial_storage, 4);
        initial.store_note(note("old-note"), metadata(1)).unwrap();
        let old_size = fs::read_dir(root.join("CACHE/NOTES"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .metadata()
            .unwrap()
            .len();

        // Both IDs have the same serialized width. The normal temporary write
        // cannot fit alongside the old record, but it can after staging it.
        let storage = AtlasStorage::with_limits(
            root.clone(),
            AtlasStorageLimits {
                max_file_bytes: 32 * 1024,
                max_cache_bytes: old_size + 1,
                max_directory_entries: 16,
            },
        )
        .unwrap();
        let repository = AtlasCacheRepository::with_record_limit(storage, 4);
        repository
            .store_note(note("new-note"), metadata(2))
            .unwrap();
        assert_eq!(
            repository.offline_note("old-note").status,
            AtlasOfflineStatus::OfflineNoData
        );
        assert_eq!(
            repository.offline_note("new-note").status,
            AtlasOfflineStatus::OfflineCached
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn same_surface_record_limit_eviction_uses_distinct_staged_candidate() {
        let root = root("same-surface-eviction");
        let initial_storage = AtlasStorage::with_limits(
            root.clone(),
            AtlasStorageLimits {
                max_file_bytes: 32 * 1024,
                max_cache_bytes: 256 * 1024,
                max_directory_entries: 16,
            },
        )
        .unwrap();
        let initial = AtlasCacheRepository::with_record_limit(initial_storage, 1);
        initial.store_note(note("old-note"), metadata(1)).unwrap();
        let old_size = fs::read_dir(root.join("CACHE/NOTES"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .metadata()
            .unwrap()
            .len();

        // Both records fit individually, but the cache budget admits only a
        // staged replacement. The candidate must use a distinct primary name
        // so the storage eviction transaction can preserve the old record
        // until the new one commits.
        let storage = AtlasStorage::with_limits(
            root.clone(),
            AtlasStorageLimits {
                max_file_bytes: 32 * 1024,
                max_cache_bytes: old_size + 1,
                max_directory_entries: 16,
            },
        )
        .unwrap();
        let repository = AtlasCacheRepository::with_record_limit(storage, 1);
        repository
            .store_note(note("new-note"), metadata(2))
            .unwrap();

        assert_eq!(
            repository.offline_note("old-note").status,
            AtlasOfflineStatus::OfflineNoData
        );
        assert_eq!(
            repository.offline_note("new-note").status,
            AtlasOfflineStatus::OfflineCached
        );
        let names: Vec<_> = fs::read_dir(root.join("CACHE/NOTES"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names, vec![format!("{:08X}.DAT", hash_name("new-note", 0))]);
        assert!(!root.join("LOGS/EVICT.TXN").exists());
        assert!(!root.join("LOGS/EVICT.STG").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn oversized_serialized_candidate_preserves_record_limit_victim() {
        let root = root("oversized-candidate");
        let storage = AtlasStorage::with_limits(
            root.clone(),
            AtlasStorageLimits {
                max_file_bytes: 1024,
                max_cache_bytes: 8 * 1024,
                max_directory_entries: 16,
            },
        )
        .unwrap();
        let repository = AtlasCacheRepository::with_record_limit(storage, 1);
        repository.store_note(note("first"), metadata(1)).unwrap();

        let mut oversized = note("second");
        oversized.body = "x".repeat(2 * 1024);
        assert!(matches!(
            repository.store_note(oversized, metadata(2)),
            Err(AtlasCacheError::Storage(
                AtlasStorageError::FileTooLarge { .. }
            ))
        ));
        assert_eq!(
            repository.offline_note("first").status,
            AtlasOfflineStatus::OfflineCached
        );
        assert_eq!(
            repository.offline_note("second").status,
            AtlasOfflineStatus::OfflineNoData
        );
        assert_eq!(fs::read_dir(root.join("CACHE/NOTES")).unwrap().count(), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn oversized_cross_surface_candidate_preserves_global_lru_victim() {
        let root = root("oversized-cross-surface-candidate");
        let storage = AtlasStorage::with_limits(
            root.clone(),
            AtlasStorageLimits {
                max_file_bytes: 1024,
                max_cache_bytes: 8 * 1024,
                max_directory_entries: 16,
            },
        )
        .unwrap();
        let repository = AtlasCacheRepository::with_record_limit(storage, 1);
        repository.store_note(note("first"), metadata(1)).unwrap();

        let mut oversized = search("second");
        oversized.hits[0].snippet = "x".repeat(2 * 1024);
        assert!(matches!(
            repository.store_search(oversized, metadata(2)),
            Err(AtlasCacheError::Storage(
                AtlasStorageError::FileTooLarge { .. }
            ))
        ));
        assert_eq!(
            repository.offline_note("first").status,
            AtlasOfflineStatus::OfflineCached
        );
        assert_eq!(
            repository.offline_search("second").status,
            AtlasOfflineStatus::OfflineNoData
        );
        assert_eq!(fs::read_dir(root.join("CACHE/NOTES")).unwrap().count(), 1);
        assert_eq!(fs::read_dir(root.join("CACHE/SEARCH")).unwrap().count(), 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn offline_read_touches_last_used_and_changes_eviction_order() {
        let (repository, root) = repository("touch-eviction", 2);
        repository.store_note(note("first"), metadata(1)).unwrap();
        repository.store_note(note("second"), metadata(2)).unwrap();

        assert_eq!(
            repository.offline_note("first").metadata.unwrap().last_used,
            3
        );
        repository.store_note(note("third"), metadata(4)).unwrap();

        assert_eq!(
            repository.offline_note("first").status,
            AtlasOfflineStatus::OfflineCached
        );
        assert_eq!(
            repository.offline_note("second").status,
            AtlasOfflineStatus::OfflineNoData
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn lookup_recovers_missing_primary_but_fails_closed_for_corrupt_group() {
        let (repository, root) = repository("backup-recovery", 4);
        repository.store_note(note("missing"), metadata(1)).unwrap();
        let notes = root.join("CACHE/NOTES");
        let primary = fs::read_dir(&notes)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let backup = primary.with_extension("BAK");
        fs::rename(&primary, &backup).unwrap();

        assert_eq!(
            repository.offline_note("missing").status,
            AtlasOfflineStatus::OfflineCached
        );
        assert!(primary.exists());
        assert!(!backup.exists());

        repository.store_note(note("corrupt"), metadata(2)).unwrap();
        let corrupt_primary = fs::read_dir(&notes)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| path != &primary)
            .unwrap();
        let corrupt_backup = corrupt_primary.with_extension("BAK");
        fs::copy(&corrupt_primary, &corrupt_backup).unwrap();
        fs::write(&corrupt_primary, b"corrupt").unwrap();

        assert_eq!(
            repository.offline_note("corrupt").status,
            AtlasOfflineStatus::Error
        );
        assert!(matches!(
            repository.store_note(note("blocked"), metadata(3)),
            Err(AtlasCacheError::UntrustedInventory)
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unknown_schema_blocks_writes_without_evicting_known_records() {
        let (repository, root) = repository("unknown-schema", 2);
        repository.store_note(note("first"), metadata(1)).unwrap();
        repository.store_note(note("second"), metadata(2)).unwrap();
        let unsupported = CacheRecord {
            schema_version: ATLAS_CACHE_SCHEMA_VERSION + 1,
            key: "unsupported".into(),
            metadata: metadata(3),
            payload: CachePayload::Note(note("unsupported")),
        };
        let bytes = serde_json::to_vec(&unsupported).unwrap();
        repository
            .storage
            .replace_bytes(AtlasDirectory::CacheNotes, "BAD.DAT", &bytes)
            .unwrap();

        assert!(matches!(
            repository.store_note(note("third"), metadata(4)),
            Err(AtlasCacheError::UntrustedInventory)
        ));
        assert_eq!(
            repository.offline_note("first").status,
            AtlasOfflineStatus::Error
        );
        assert_eq!(
            repository.offline_note("second").status,
            AtlasOfflineStatus::Error
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn omitted_listing_entries_fail_closed_before_record_limit_eviction() {
        let root = root("omitted-listing");
        let storage = AtlasStorage::with_limits(
            root.clone(),
            AtlasStorageLimits {
                max_file_bytes: 32 * 1024,
                max_cache_bytes: 256 * 1024,
                max_directory_entries: 1,
            },
        )
        .unwrap();
        let repository = AtlasCacheRepository::with_record_limit(storage, 2);
        repository.store_note(note("first"), metadata(1)).unwrap();
        repository.store_note(note("second"), metadata(2)).unwrap();

        assert_ne!(
            repository
                .storage
                .list(AtlasDirectory::CacheNotes)
                .unwrap()
                .omitted_entries,
            0
        );
        assert!(matches!(
            repository.store_note(note("third"), metadata(3)),
            Err(AtlasCacheError::UntrustedInventory)
        ));
        assert_eq!(
            repository.offline_note("first").status,
            AtlasOfflineStatus::Error
        );
        assert_eq!(fs::read_dir(root.join("CACHE/NOTES")).unwrap().count(), 2);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn offline_surface_states_remain_independent() {
        let (repository, root) = repository("surface", 4);
        repository
            .store_search(search("plan"), metadata(1))
            .unwrap();
        assert_eq!(
            repository.offline_search("plan").status,
            AtlasOfflineStatus::OfflineCached
        );
        assert_eq!(
            repository.offline_note("note-1").status,
            AtlasOfflineStatus::OfflineNoData
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn budget_failure_preserves_existing_cache() {
        let root = root("budget");
        let storage = AtlasStorage::with_limits(
            root.clone(),
            AtlasStorageLimits {
                max_file_bytes: 1024,
                max_cache_bytes: 1024,
                max_directory_entries: 16,
            },
        )
        .unwrap();
        let repository = AtlasCacheRepository::new(storage);
        repository.store_note(note("note-1"), metadata(1)).unwrap();
        fs::write(root.join("CACHE/HOME/BAD.DAT"), vec![b'x'; 700]).unwrap();
        assert!(matches!(
            repository.store_note(note("note-2"), metadata(2)),
            Err(AtlasCacheError::UntrustedInventory)
        ));
        assert_eq!(
            repository.offline_note("note-1").status,
            AtlasOfflineStatus::Error
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn serialized_cache_and_errors_never_include_transport_secrets() {
        let (repository, root) = repository("secrets", 4);
        repository
            .store_search(search("plan"), metadata(1))
            .unwrap();
        let name = fs::read_dir(root.join("CACHE/SEARCH"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .file_name()
            .into_string()
            .unwrap();
        let text = repository
            .storage
            .read_text(AtlasDirectory::CacheSearch, &name)
            .unwrap();
        assert!(!text.contains("Authorization"));
        assert!(!text.contains("at_v1"));
        assert!(!format!("{:?}", AtlasCacheError::InvalidRecord).contains("at_v1"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn views_payload_remains_a_typed_page() {
        let (repository, root) = repository("views", 4);
        let page = ViewResultPage {
            view: ViewSummary {
                id: "today".into(),
                name: "Today".into(),
                revision: "r1".into(),
                status: ViewStatus::Ok,
                layout: ViewLayout::List,
            },
            items: vec![ViewResult {
                id: Some("note-1".into()),
                path: "note.md".into(),
                title: "Cached".into(),
                state: NoteState::Managed,
                revision: "r1".into(),
            }],
            next_cursor: None,
        };
        repository.store_views(page, None, metadata(1)).unwrap();
        assert_eq!(
            repository.offline_views("today", None).status,
            AtlasOfflineStatus::OfflineCached
        );
        let _ = fs::remove_dir_all(root);
    }
}
