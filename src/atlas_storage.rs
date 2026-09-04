//! Bounded, recovery-safe storage for Atlas Lite data on the SD card.
//!
//! This module owns `/sdcard/ATLAS` when constructed with that root.  It is
//! intentionally independent from the read-only Rustmix storage browser and
//! does not define cache records or queue behaviour.  Callers can only address
//! the fixed Atlas directories with FAT 8.3-safe file names.

use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

/// Default maximum bytes accepted for one Atlas SD file.
pub const MAX_ATLAS_FILE_BYTES: u64 = 64 * 1024;
/// Default logical cache budget across all `CACHE/*` directories.
pub const MAX_ATLAS_CACHE_BYTES: u64 = 512 * 1024;
/// Maximum entries retained by one deterministic directory listing.
pub const MAX_ATLAS_DIRECTORY_ENTRIES: usize = 128;
/// Maximum filesystem entries inspected while accounting for the cache tree.
/// Unknown directories cannot make a budget check consume unbounded memory or
/// time, and entries beyond this bound fail closed.
pub const MAX_ATLAS_CACHE_SCAN_ENTRIES: usize = 1024;

const INTEGRITY_MAGIC: [u8; 4] = *b"ATLS";
const INTEGRITY_VERSION: u8 = 1;
const INTEGRITY_HEADER_BYTES: usize = 13;

const ATLAS_LAYOUT: [AtlasDirectory; 9] = [
    AtlasDirectory::Cache,
    AtlasDirectory::CacheHome,
    AtlasDirectory::CacheNotes,
    AtlasDirectory::CacheViews,
    AtlasDirectory::CacheSearch,
    AtlasDirectory::Queue,
    AtlasDirectory::Audio,
    AtlasDirectory::Assets,
    AtlasDirectory::Logs,
];

/// One fixed Atlas-owned SD directory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtlasDirectory {
    Cache,
    CacheHome,
    CacheNotes,
    CacheViews,
    CacheSearch,
    Queue,
    Audio,
    Assets,
    Logs,
}

impl AtlasDirectory {
    const fn relative_path(self) -> &'static str {
        match self {
            Self::Cache => "CACHE",
            Self::CacheHome => "CACHE/HOME",
            Self::CacheNotes => "CACHE/NOTES",
            Self::CacheViews => "CACHE/VIEWS",
            Self::CacheSearch => "CACHE/SEARCH",
            Self::Queue => "QUEUE",
            Self::Audio => "AUDIO",
            Self::Assets => "ASSETS",
            Self::Logs => "LOGS",
        }
    }

    const fn is_cache(self) -> bool {
        matches!(
            self,
            Self::Cache | Self::CacheHome | Self::CacheNotes | Self::CacheViews | Self::CacheSearch
        )
    }
}

/// Explicit, injectable storage bounds.  Smaller values make host tests exact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtlasStorageLimits {
    pub max_file_bytes: u64,
    pub max_cache_bytes: u64,
    pub max_directory_entries: usize,
}

impl Default for AtlasStorageLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: MAX_ATLAS_FILE_BYTES,
            max_cache_bytes: MAX_ATLAS_CACHE_BYTES,
            max_directory_entries: MAX_ATLAS_DIRECTORY_ENTRIES,
        }
    }
}

/// A single listed entry.  Unknown and corrupt entries are reported, never removed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasStorageEntry {
    pub name: String,
    pub size_bytes: Option<u64>,
    pub disposition: AtlasEntryDisposition,
}

/// Classification that lets later cache/queue code isolate one bad entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtlasEntryDisposition {
    /// A regular, bounded, Atlas-addressable primary file.
    Ready,
    /// A `.TMP` or `.BAK` sibling left by an interrupted replacement.
    RecoveryArtifact,
    /// A nominal Atlas file that cannot safely be read (for example, a symlink).
    Corrupt,
    /// A preserved file or directory outside the Atlas file-name contract.
    Unknown,
}

/// Deterministic, bounded result of listing one fixed directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasDirectoryListing {
    pub entries: Vec<AtlasStorageEntry>,
    pub accounted_bytes: u64,
    pub corrupt_entries: usize,
    pub unknown_entries: usize,
    pub omitted_entries: usize,
}

/// Errors never include file contents, which could contain user data.
#[derive(Debug)]
pub enum AtlasStorageError {
    InvalidRoot,
    InvalidName,
    Symlink(PathBuf),
    NotRegularFile(PathBuf),
    NotFound(PathBuf),
    FileTooLarge {
        size: u64,
        limit: u64,
    },
    CacheBudgetExceeded {
        attempted: u64,
        limit: u64,
    },
    CacheScanLimitExceeded {
        limit: usize,
    },
    CacheRootWrite,
    InvalidLimits,
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    InvalidText,
    InvalidIntegrity,
}

impl fmt::Display for AtlasStorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRoot => write!(formatter, "Atlas storage root is not a directory"),
            Self::InvalidName => write!(formatter, "Atlas storage name is not FAT 8.3 safe"),
            Self::Symlink(path) => write!(
                formatter,
                "Atlas storage symlink rejected: {}",
                path.display()
            ),
            Self::NotRegularFile(path) => write!(
                formatter,
                "Atlas storage entry is not a regular file: {}",
                path.display()
            ),
            Self::NotFound(path) => write!(
                formatter,
                "Atlas storage entry is missing: {}",
                path.display()
            ),
            Self::FileTooLarge { size, limit } => write!(
                formatter,
                "Atlas storage file exceeds {limit} byte limit ({size} bytes)"
            ),
            Self::CacheBudgetExceeded { attempted, limit } => write!(
                formatter,
                "Atlas cache would exceed {limit} bytes ({attempted} bytes)"
            ),
            Self::CacheScanLimitExceeded { limit } => write!(
                formatter,
                "Atlas cache scan exceeded the {limit} entry safety bound"
            ),
            Self::CacheRootWrite => write!(formatter, "Atlas cache root is layout-only"),
            Self::InvalidLimits => write!(formatter, "Atlas storage limits must be non-zero"),
            Self::Io {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "Atlas storage {operation} {}: {source}",
                path.display()
            ),
            Self::InvalidText => write!(formatter, "Atlas storage file is not valid UTF-8"),
            Self::InvalidIntegrity => write!(formatter, "Atlas storage integrity check failed"),
        }
    }
}

impl std::error::Error for AtlasStorageError {}

/// Host-testable Atlas SD root.  The injected root represents `/sdcard/ATLAS`.
#[derive(Clone, Debug)]
pub struct AtlasStorage {
    root: PathBuf,
    limits: AtlasStorageLimits,
}

impl AtlasStorage {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, AtlasStorageError> {
        Self::with_limits(root, AtlasStorageLimits::default())
    }

    pub fn with_limits(
        root: impl Into<PathBuf>,
        limits: AtlasStorageLimits,
    ) -> Result<Self, AtlasStorageError> {
        if limits.max_file_bytes == 0
            || limits.max_cache_bytes == 0
            || limits.max_directory_entries == 0
        {
            return Err(AtlasStorageError::InvalidLimits);
        }
        let storage = Self {
            root: root.into(),
            limits,
        };
        storage.ensure_layout()?;
        Ok(storage)
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Create exactly the fixed Atlas layout, rejecting existing symlinks/files.
    pub fn ensure_layout(&self) -> Result<(), AtlasStorageError> {
        self.ensure_directory(&self.root)?;
        for directory in ATLAS_LAYOUT {
            self.ensure_directory(&self.directory_path(directory))?;
        }
        Ok(())
    }

    /// Atomically replace a bounded file.  The old primary remains as `.BAK`
    /// until a fully synchronized `.TMP` has replaced it.
    pub fn replace_bytes(
        &self,
        directory: AtlasDirectory,
        name: &str,
        bytes: &[u8],
    ) -> Result<(), AtlasStorageError> {
        self.validate_name(name)?;
        if directory == AtlasDirectory::Cache {
            return Err(AtlasStorageError::CacheRootWrite);
        }
        self.ensure_layout()?;
        self.check_file_bytes(bytes)?;
        let primary = self.file_path(directory, name);
        let stored_bytes = integrity_envelope(bytes);
        self.check_cache_budget(directory, &stored_bytes)?;

        let temp = sibling(&primary, "TMP");
        let backup = sibling(&primary, "BAK");
        self.reject_symlink_if_exists(&primary)?;
        self.reject_symlink_if_exists(&temp)?;
        self.reject_symlink_if_exists(&backup)?;
        self.remove_regular_if_exists(&temp)?;
        self.remove_regular_if_exists(&backup)?;

        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|source| io_error("create temporary", &temp, source))?;
        file.write_all(&stored_bytes)
            .map_err(|source| io_error("write temporary", &temp, source))?;
        file.sync_all()
            .map_err(|source| io_error("sync temporary", &temp, source))?;
        drop(file);

        if primary.exists() {
            fs::rename(&primary, &backup)
                .map_err(|source| io_error("backup primary", &primary, source))?;
        }
        if let Err(source) = fs::rename(&temp, &primary) {
            if backup.exists() {
                let _ = fs::rename(&backup, &primary);
            }
            return Err(io_error("replace primary", &primary, source));
        }
        // FAT on ESP-IDF has no directory handle we can sync.  On host filesystems
        // this is best-effort only: the primary has already been committed and a
        // post-commit durability hint must not turn a successful replacement into
        // a reported failure.
        let _ = self.sync_directory(primary.parent().expect("file path has parent"));
        let _ = self.remove_regular_if_exists(&backup);
        Ok(())
    }

    /// Reject bytes that cannot fit in one Atlas file without creating or
    /// replacing any filesystem entry.  Cache callers use this before choosing
    /// a record-limit eviction victim.
    pub fn check_file_bytes(&self, bytes: &[u8]) -> Result<(), AtlasStorageError> {
        let size = u64::try_from(bytes.len()).map_err(|_| AtlasStorageError::FileTooLarge {
            size: u64::MAX,
            limit: self.limits.max_file_bytes,
        })?;
        self.check_file_budget(size)
    }

    pub fn replace_text(
        &self,
        directory: AtlasDirectory,
        name: &str,
        text: &str,
    ) -> Result<(), AtlasStorageError> {
        self.replace_bytes(directory, name, text.as_bytes())
    }

    /// Read a bounded primary.  If startup finds a missing primary and a valid
    /// backup, the backup is restored before it is read.  A stale `.TMP` is
    /// deliberately retained for listing/diagnostics.
    pub fn read_bytes(
        &self,
        directory: AtlasDirectory,
        name: &str,
    ) -> Result<Vec<u8>, AtlasStorageError> {
        self.validate_name(name)?;
        self.ensure_layout()?;
        let primary = self.file_path(directory, name);
        let backup = sibling(&primary, "BAK");
        self.restore_missing_primary(&primary, &backup)?;
        match self.read_candidate(&primary) {
            Ok(bytes) => Ok(bytes),
            Err(primary_error) if backup.exists() => {
                self.read_candidate(&backup).or(Err(primary_error))
            }
            Err(error) => Err(error),
        }
    }

    pub fn read_text(
        &self,
        directory: AtlasDirectory,
        name: &str,
    ) -> Result<String, AtlasStorageError> {
        self.validate_name(name)?;
        self.ensure_layout()?;
        let primary = self.file_path(directory, name);
        let backup = sibling(&primary, "BAK");
        self.restore_missing_primary(&primary, &backup)?;
        match self.read_candidate(&primary).and_then(bytes_to_text) {
            Ok(text) => Ok(text),
            Err(primary_error) if backup.exists() => self
                .read_candidate(&backup)
                .and_then(bytes_to_text)
                .or(Err(primary_error)),
            Err(error) => Err(error),
        }
    }

    /// Remove one verified cache primary during deterministic cache eviction.
    /// Queue, audio, assets, and logs are deliberately not addressable here.
    pub fn remove_cache_file(
        &self,
        directory: AtlasDirectory,
        name: &str,
    ) -> Result<(), AtlasStorageError> {
        self.validate_name(name)?;
        if !directory.is_cache() || directory == AtlasDirectory::Cache {
            return Err(AtlasStorageError::CacheRootWrite);
        }
        self.ensure_layout()?;
        let primary = self.file_path(directory, name);
        self.reject_symlink_if_exists(&primary)?;
        match fs::symlink_metadata(&primary) {
            Ok(metadata) if metadata.is_file() => fs::remove_file(&primary)
                .map_err(|source| io_error("remove cache entry", &primary, source)),
            Ok(_) => Err(AtlasStorageError::NotRegularFile(primary)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(io_error("inspect cache entry", &primary, source)),
        }
    }

    /// Commit a new cache entry before removing an evicted entry in another
    /// cache surface. The candidate uses the normal synchronized replacement
    /// protocol, so a failed write leaves the victim untouched. If removing
    /// the victim fails after that commit, remove the candidate again to keep
    /// the previous cache entry available whenever rollback succeeds.
    pub fn replace_cache_eviction_bytes(
        &self,
        victim_directory: AtlasDirectory,
        victim_name: &str,
        candidate_directory: AtlasDirectory,
        candidate_name: &str,
        bytes: &[u8],
    ) -> Result<(), AtlasStorageError> {
        self.validate_name(victim_name)?;
        self.validate_name(candidate_name)?;
        if !victim_directory.is_cache()
            || victim_directory == AtlasDirectory::Cache
            || !candidate_directory.is_cache()
            || candidate_directory == AtlasDirectory::Cache
        {
            return Err(AtlasStorageError::CacheRootWrite);
        }
        self.replace_bytes(candidate_directory, candidate_name, bytes)?;
        if let Err(error) = self.remove_cache_file(victim_directory, victim_name) {
            let _ = self.remove_cache_file(candidate_directory, candidate_name);
            return Err(error);
        }
        Ok(())
    }

    /// List one fixed directory without trusting individual entries.
    pub fn list(
        &self,
        directory: AtlasDirectory,
    ) -> Result<AtlasDirectoryListing, AtlasStorageError> {
        self.ensure_layout()?;
        let path = self.directory_path(directory);
        let read_dir =
            fs::read_dir(&path).map_err(|source| io_error("list directory", &path, source))?;
        let mut entries = Vec::new();
        let mut corrupt_entries = 0_usize;
        let mut unknown_entries = 0_usize;
        let mut accounted_bytes = 0_u64;
        let mut total_entries = 0_usize;

        for item in read_dir {
            total_entries = total_entries.saturating_add(1);
            let entry = match item {
                Ok(entry) => entry,
                Err(_) => {
                    corrupt_entries = corrupt_entries.saturating_add(1);
                    continue;
                }
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            let (disposition, size_bytes) = self.classify_entry(&entry.path(), &name);
            match disposition {
                AtlasEntryDisposition::Ready => {
                    accounted_bytes = accounted_bytes.saturating_add(size_bytes.unwrap_or(0));
                }
                AtlasEntryDisposition::Corrupt => {
                    corrupt_entries = corrupt_entries.saturating_add(1)
                }
                AtlasEntryDisposition::Unknown => {
                    unknown_entries = unknown_entries.saturating_add(1)
                }
                AtlasEntryDisposition::RecoveryArtifact if directory.is_cache() => {
                    accounted_bytes = accounted_bytes.saturating_add(size_bytes.unwrap_or(0));
                }
                AtlasEntryDisposition::RecoveryArtifact => {}
            }
            insert_bounded_sorted(
                &mut entries,
                AtlasStorageEntry {
                    name,
                    size_bytes,
                    disposition,
                },
                self.limits.max_directory_entries,
            );
        }

        Ok(AtlasDirectoryListing {
            omitted_entries: total_entries.saturating_sub(entries.len()),
            entries,
            accounted_bytes,
            corrupt_entries,
            unknown_entries,
        })
    }

    /// On-disk bytes occupied by every regular file beneath the cache roots.
    /// This deliberately includes corrupt, recovery, and unknown entries so a
    /// failed replacement cannot turn existing physical bytes into unbounded
    /// space. Non-regular entries are preserved but do not consume file bytes.
    pub fn cache_bytes(&self) -> Result<u64, AtlasStorageError> {
        let path = self.directory_path(AtlasDirectory::Cache);
        let mut pending = vec![path.clone()];
        let mut bytes = 0_u64;
        let mut scanned_entries = 0_usize;
        while let Some(directory) = pending.pop() {
            let read_dir = fs::read_dir(&directory)
                .map_err(|source| io_error("list cache directory", &directory, source))?;
            for item in read_dir {
                scanned_entries = scanned_entries.saturating_add(1);
                if scanned_entries > MAX_ATLAS_CACHE_SCAN_ENTRIES {
                    return Err(AtlasStorageError::CacheScanLimitExceeded {
                        limit: MAX_ATLAS_CACHE_SCAN_ENTRIES,
                    });
                }
                let entry =
                    item.map_err(|source| io_error("inspect cache entry", &directory, source))?;
                let entry_path = entry.path();
                let metadata = fs::symlink_metadata(&entry_path)
                    .map_err(|source| io_error("inspect cache entry", &entry_path, source))?;
                if metadata.is_dir() {
                    pending.push(entry_path);
                } else if metadata.is_file() {
                    bytes = bytes.checked_add(metadata.len()).ok_or(
                        AtlasStorageError::CacheBudgetExceeded {
                            attempted: u64::MAX,
                            limit: self.limits.max_cache_bytes,
                        },
                    )?;
                    if bytes > self.limits.max_cache_bytes {
                        return Err(AtlasStorageError::CacheBudgetExceeded {
                            attempted: bytes,
                            limit: self.limits.max_cache_bytes,
                        });
                    }
                }
            }
        }
        Ok(bytes)
    }

    fn check_file_budget(&self, size: u64) -> Result<(), AtlasStorageError> {
        if size > self.limits.max_file_bytes {
            return Err(AtlasStorageError::FileTooLarge {
                size,
                limit: self.limits.max_file_bytes,
            });
        }
        Ok(())
    }

    fn check_cache_budget(
        &self,
        directory: AtlasDirectory,
        replacement_bytes: &[u8],
    ) -> Result<(), AtlasStorageError> {
        if !directory.is_cache() {
            return Ok(());
        }
        let replacement_size = u64::try_from(replacement_bytes.len()).unwrap_or(u64::MAX);
        // Reserve the synchronized temporary alongside every existing primary,
        // backup, and temporary.  This keeps an interrupted replacement from
        // exceeding the cache budget even before its old primary is removed.
        let attempted = self.cache_bytes()?.saturating_add(replacement_size);
        if attempted > self.limits.max_cache_bytes {
            return Err(AtlasStorageError::CacheBudgetExceeded {
                attempted,
                limit: self.limits.max_cache_bytes,
            });
        }
        Ok(())
    }

    fn restore_missing_primary(
        &self,
        primary: &Path,
        backup: &Path,
    ) -> Result<(), AtlasStorageError> {
        self.reject_symlink_if_exists(primary)?;
        self.reject_symlink_if_exists(backup)?;
        if !primary.exists() && backup.exists() {
            self.read_candidate(backup)?;
            fs::rename(backup, primary)
                .map_err(|source| io_error("restore backup", primary, source))?;
            let _ = self.sync_directory(primary.parent().expect("file path has parent"));
        }
        Ok(())
    }

    fn read_candidate(&self, path: &Path) -> Result<Vec<u8>, AtlasStorageError> {
        let metadata = fs::symlink_metadata(path).map_err(|source| match source.kind() {
            io::ErrorKind::NotFound => AtlasStorageError::NotFound(path.into()),
            _ => io_error("inspect file", path, source),
        })?;
        if metadata.file_type().is_symlink() {
            return Err(AtlasStorageError::Symlink(path.into()));
        }
        if !metadata.is_file() {
            return Err(AtlasStorageError::NotRegularFile(path.into()));
        }
        if metadata.len() > self.max_stored_file_bytes() {
            return Err(AtlasStorageError::FileTooLarge {
                size: metadata.len(),
                limit: self.max_stored_file_bytes(),
            });
        }
        let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
        File::open(path)
            .map_err(|source| io_error("open file", path, source))?
            .take(self.max_stored_file_bytes().saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|source| io_error("read file", path, source))?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > self.max_stored_file_bytes() {
            return Err(AtlasStorageError::FileTooLarge {
                size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                limit: self.max_stored_file_bytes(),
            });
        }
        let payload = validate_integrity_envelope(&bytes)?;
        self.check_file_budget(u64::try_from(payload.len()).unwrap_or(u64::MAX))?;
        Ok(payload)
    }

    fn classify_entry(&self, path: &Path, name: &str) -> (AtlasEntryDisposition, Option<u64>) {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(_) => return (AtlasEntryDisposition::Corrupt, None),
        };
        if metadata.file_type().is_symlink() {
            return (AtlasEntryDisposition::Corrupt, None);
        }
        if !metadata.is_file() {
            return (AtlasEntryDisposition::Unknown, None);
        }
        let size = metadata.len();
        if is_recovery_artifact(name) && is_fat83_file_name(name) {
            return (AtlasEntryDisposition::RecoveryArtifact, Some(size));
        }
        if !is_fat83_file_name(name) {
            return (AtlasEntryDisposition::Unknown, Some(size));
        }
        if size > self.max_stored_file_bytes() {
            return (AtlasEntryDisposition::Corrupt, Some(size));
        }
        if self.read_candidate(path).is_err() {
            return (AtlasEntryDisposition::Corrupt, Some(size));
        }
        (AtlasEntryDisposition::Ready, Some(size))
    }

    fn ensure_directory(&self, path: &Path) -> Result<(), AtlasStorageError> {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                Err(AtlasStorageError::Symlink(path.into()))
            }
            Ok(metadata) if metadata.is_dir() => Ok(()),
            Ok(_) => Err(AtlasStorageError::InvalidRoot),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::create_dir(path).map_err(|source| io_error("create directory", path, source))
            }
            Err(source) => Err(io_error("inspect directory", path, source)),
        }
    }

    fn reject_symlink_if_exists(&self, path: &Path) -> Result<(), AtlasStorageError> {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                Err(AtlasStorageError::Symlink(path.into()))
            }
            Ok(_) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(io_error("inspect path", path, source)),
        }
    }

    fn remove_regular_if_exists(&self, path: &Path) -> Result<(), AtlasStorageError> {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                Err(AtlasStorageError::Symlink(path.into()))
            }
            Ok(metadata) if metadata.is_file() => fs::remove_file(path)
                .map_err(|source| io_error("remove recovery artifact", path, source)),
            Ok(_) => Err(AtlasStorageError::NotRegularFile(path.into())),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(io_error("inspect recovery artifact", path, source)),
        }
    }

    #[cfg(not(target_os = "espidf"))]
    fn sync_directory(&self, path: &Path) -> Result<(), AtlasStorageError> {
        File::open(path)
            .and_then(|directory| directory.sync_all())
            .map_err(|source| io_error("sync directory", path, source))
    }

    #[cfg(target_os = "espidf")]
    fn sync_directory(&self, _path: &Path) -> Result<(), AtlasStorageError> {
        // ESP-IDF FAT exposes file synchronization but not a portable directory
        // handle.  The temporary file is synced before rename; no post-rename
        // directory operation is attempted on this target.
        Ok(())
    }

    fn max_stored_file_bytes(&self) -> u64 {
        self.limits
            .max_file_bytes
            .saturating_add(INTEGRITY_HEADER_BYTES as u64)
    }

    fn validate_name(&self, name: &str) -> Result<(), AtlasStorageError> {
        if is_fat83_file_name(name) && !is_recovery_artifact(name) {
            Ok(())
        } else {
            Err(AtlasStorageError::InvalidName)
        }
    }

    fn directory_path(&self, directory: AtlasDirectory) -> PathBuf {
        self.root.join(directory.relative_path())
    }

    fn file_path(&self, directory: AtlasDirectory, name: &str) -> PathBuf {
        self.directory_path(directory).join(name)
    }
}

fn bytes_to_text(bytes: Vec<u8>) -> Result<String, AtlasStorageError> {
    String::from_utf8(bytes).map_err(|_| AtlasStorageError::InvalidText)
}

fn integrity_envelope(payload: &[u8]) -> Vec<u8> {
    let length = u32::try_from(payload.len()).expect("Atlas file limits fit in u32");
    let mut encoded = Vec::with_capacity(INTEGRITY_HEADER_BYTES + payload.len());
    encoded.extend_from_slice(&INTEGRITY_MAGIC);
    encoded.push(INTEGRITY_VERSION);
    encoded.extend_from_slice(&length.to_le_bytes());
    encoded.extend_from_slice(&checksum(payload).to_le_bytes());
    encoded.extend_from_slice(payload);
    encoded
}

fn validate_integrity_envelope(bytes: &[u8]) -> Result<Vec<u8>, AtlasStorageError> {
    if bytes.len() < INTEGRITY_HEADER_BYTES
        || bytes[..4] != INTEGRITY_MAGIC
        || bytes[4] != INTEGRITY_VERSION
    {
        return Err(AtlasStorageError::InvalidIntegrity);
    }
    let length = u32::from_le_bytes(bytes[5..9].try_into().expect("fixed envelope slice"));
    let expected_checksum =
        u32::from_le_bytes(bytes[9..13].try_into().expect("fixed envelope slice"));
    let payload = &bytes[INTEGRITY_HEADER_BYTES..];
    if usize::try_from(length).ok() != Some(payload.len()) || checksum(payload) != expected_checksum
    {
        return Err(AtlasStorageError::InvalidIntegrity);
    }
    Ok(payload.to_vec())
}

fn checksum(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5_u32, |hash, byte| {
        (hash ^ u32::from(*byte)).wrapping_mul(0x0100_0193)
    })
}

fn sibling(path: &Path, extension: &str) -> PathBuf {
    path.with_extension(extension)
}

fn io_error(operation: &'static str, path: &Path, source: io::Error) -> AtlasStorageError {
    AtlasStorageError::Io {
        operation,
        path: path.into(),
        source,
    }
}

fn is_recovery_artifact(name: &str) -> bool {
    matches!(
        Path::new(name).extension().and_then(|value| value.to_str()),
        Some("TMP" | "BAK")
    )
}

fn is_fat83_file_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    let Some((stem, extension)) = name.split_once('.') else {
        return (1..=8).contains(&bytes.len())
            && bytes.iter().all(|value| {
                value.is_ascii_uppercase() || value.is_ascii_digit() || *value == b'_'
            });
    };
    !stem.is_empty()
        && stem.len() <= 8
        && !extension.is_empty()
        && extension.len() <= 3
        && !extension.contains('.')
        && stem
            .bytes()
            .chain(extension.bytes())
            .all(|value| value.is_ascii_uppercase() || value.is_ascii_digit() || value == b'_')
}

fn insert_bounded_sorted(
    entries: &mut Vec<AtlasStorageEntry>,
    entry: AtlasStorageEntry,
    limit: usize,
) {
    let index = entries
        .binary_search_by(|current| current.name.cmp(&entry.name))
        .unwrap_or_else(|index| index);
    if index < limit {
        entries.insert(index, entry);
        if entries.len() > limit {
            entries.pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "atlas-storage-{label}-{}-{unique}",
            std::process::id()
        ))
    }

    fn storage(label: &str) -> AtlasStorage {
        AtlasStorage::new(temp_root(label)).unwrap()
    }

    #[test]
    fn creates_only_fixed_layout() {
        let storage = storage("layout");
        for directory in ATLAS_LAYOUT {
            assert!(storage.directory_path(directory).is_dir());
        }
        assert_eq!(fs::read_dir(storage.root()).unwrap().count(), 5);
        let _ = fs::remove_dir_all(storage.root());
    }

    #[test]
    fn rejects_traversal_absolute_and_recovery_names() {
        let storage = storage("names");
        for name in [
            "../BAD.TXT",
            "/BAD.TXT",
            "sub/BAD.TXT",
            "lower.txt",
            "NINECHARS.TXT",
            "STATE.TMP",
        ] {
            assert!(matches!(
                storage.replace_text(AtlasDirectory::Queue, name, "x"),
                Err(AtlasStorageError::InvalidName)
            ));
        }
        assert!(!storage.root().parent().unwrap().join("BAD.TXT").exists());
        let _ = fs::remove_dir_all(storage.root());
    }

    #[test]
    fn exact_size_is_accepted_and_oversize_does_not_mutate() {
        let root = temp_root("limits");
        let storage = AtlasStorage::with_limits(
            root,
            AtlasStorageLimits {
                max_file_bytes: 4,
                max_cache_bytes: 8,
                max_directory_entries: 8,
            },
        )
        .unwrap();
        storage
            .replace_bytes(AtlasDirectory::Queue, "ITEM.DAT", b"four")
            .unwrap();
        assert_eq!(
            storage
                .read_bytes(AtlasDirectory::Queue, "ITEM.DAT")
                .unwrap(),
            b"four"
        );
        assert!(matches!(
            storage.replace_bytes(AtlasDirectory::Queue, "ITEM.DAT", b"large"),
            Err(AtlasStorageError::FileTooLarge { .. })
        ));
        assert_eq!(
            storage
                .read_bytes(AtlasDirectory::Queue, "ITEM.DAT")
                .unwrap(),
            b"four"
        );
        let _ = fs::remove_dir_all(storage.root());
    }

    #[test]
    fn cache_budget_is_checked_before_mutation() {
        let storage = AtlasStorage::with_limits(
            temp_root("cache-budget"),
            AtlasStorageLimits {
                max_file_bytes: 8,
                max_cache_bytes: 30,
                max_directory_entries: 8,
            },
        )
        .unwrap();
        storage
            .replace_bytes(AtlasDirectory::CacheHome, "HOME.DAT", b"four")
            .unwrap();
        assert!(matches!(
            storage.replace_bytes(AtlasDirectory::CacheNotes, "NOTE.DAT", b"four"),
            Err(AtlasStorageError::CacheBudgetExceeded { .. })
        ));
        assert!(!storage
            .file_path(AtlasDirectory::CacheNotes, "NOTE.DAT")
            .exists());
        let _ = fs::remove_dir_all(storage.root());
    }

    #[test]
    fn cache_root_is_layout_only() {
        let storage = storage("cache-root");
        assert!(matches!(
            storage.replace_bytes(AtlasDirectory::Cache, "ROOT.DAT", b"no"),
            Err(AtlasStorageError::CacheRootWrite)
        ));
        assert!(!storage
            .file_path(AtlasDirectory::Cache, "ROOT.DAT")
            .exists());
        let _ = fs::remove_dir_all(storage.root());
    }

    #[cfg(target_os = "espidf")]
    #[test]
    fn espidf_directory_sync_is_a_noop_after_commit() {
        let storage = storage("espidf-directory-sync");
        assert!(storage.sync_directory(storage.root()).is_ok());
        let _ = fs::remove_dir_all(storage.root());
    }

    #[test]
    fn cache_budget_reserves_recovery_artifacts() {
        let storage = AtlasStorage::with_limits(
            temp_root("cache-recovery-budget"),
            AtlasStorageLimits {
                max_file_bytes: 8,
                max_cache_bytes: 40,
                max_directory_entries: 8,
            },
        )
        .unwrap();
        storage
            .replace_bytes(AtlasDirectory::CacheHome, "HOME.DAT", b"four")
            .unwrap();
        let backup = storage.file_path(AtlasDirectory::CacheNotes, "NOTE.BAK");
        fs::write(&backup, integrity_envelope(b"old")).unwrap();

        assert_eq!(storage.cache_bytes().unwrap(), 33);
        assert!(matches!(
            storage.replace_bytes(AtlasDirectory::CacheNotes, "NOTE.DAT", b"four"),
            Err(AtlasStorageError::CacheBudgetExceeded { .. })
        ));
        assert!(backup.exists());
        let _ = fs::remove_dir_all(storage.root());
    }

    #[test]
    fn cache_budget_counts_corrupt_regular_files() {
        let storage = AtlasStorage::with_limits(
            temp_root("cache-corrupt-budget"),
            AtlasStorageLimits {
                max_file_bytes: 8,
                max_cache_bytes: 23,
                max_directory_entries: 8,
            },
        )
        .unwrap();
        let corrupt = storage.file_path(AtlasDirectory::CacheHome, "BAD.DAT");
        fs::write(&corrupt, b"corrupt").unwrap();

        assert_eq!(storage.cache_bytes().unwrap(), 7);
        assert!(matches!(
            storage.replace_bytes(AtlasDirectory::CacheNotes, "NOTE.DAT", b"four"),
            Err(AtlasStorageError::CacheBudgetExceeded { .. })
        ));
        assert!(corrupt.exists());
        let _ = fs::remove_dir_all(storage.root());
    }

    #[test]
    fn cache_budget_counts_root_and_unknown_nested_regular_files() {
        let storage = AtlasStorage::with_limits(
            temp_root("cache-tree-budget"),
            AtlasStorageLimits {
                max_file_bytes: 8,
                max_cache_bytes: 10,
                max_directory_entries: 8,
            },
        )
        .unwrap();
        fs::write(
            storage
                .directory_path(AtlasDirectory::Cache)
                .join("ROOT.DAT"),
            b"root",
        )
        .unwrap();
        let unknown = storage
            .directory_path(AtlasDirectory::Cache)
            .join("UNKNOWN");
        fs::create_dir(&unknown).unwrap();
        fs::write(unknown.join("NESTED.BIN"), b"nested").unwrap();

        assert_eq!(storage.cache_bytes().unwrap(), 10);
        assert!(matches!(
            storage.replace_bytes(AtlasDirectory::CacheHome, "HOME.DAT", b"x"),
            Err(AtlasStorageError::CacheBudgetExceeded { .. })
        ));
        assert!(!storage
            .file_path(AtlasDirectory::CacheHome, "HOME.DAT")
            .exists());
        let _ = fs::remove_dir_all(storage.root());
    }

    #[test]
    fn cache_budget_fails_closed_for_an_unbounded_unknown_tree() {
        let storage = storage("cache-scan-limit");
        let cache = storage.directory_path(AtlasDirectory::Cache);
        for index in 0..=MAX_ATLAS_CACHE_SCAN_ENTRIES {
            fs::create_dir(cache.join(format!("D{index:04}"))).unwrap();
        }

        assert!(matches!(
            storage.cache_bytes(),
            Err(AtlasStorageError::CacheScanLimitExceeded { .. })
        ));
        let _ = fs::remove_dir_all(storage.root());
    }

    #[test]
    fn primary_corruption_falls_back_to_backup_and_missing_primary_is_restored() {
        let storage = storage("backup");
        let primary = storage.file_path(AtlasDirectory::Queue, "ITEM.DAT");
        let backup = sibling(&primary, "BAK");
        fs::write(
            &primary,
            vec![b'x'; (MAX_ATLAS_FILE_BYTES + INTEGRITY_HEADER_BYTES as u64 + 1) as usize],
        )
        .unwrap();
        fs::write(&backup, integrity_envelope(b"good")).unwrap();
        assert_eq!(
            storage
                .read_bytes(AtlasDirectory::Queue, "ITEM.DAT")
                .unwrap(),
            b"good"
        );
        fs::remove_file(&primary).unwrap();
        assert_eq!(
            storage
                .read_bytes(AtlasDirectory::Queue, "ITEM.DAT")
                .unwrap(),
            b"good"
        );
        assert!(primary.exists());
        assert!(!backup.exists());
        let _ = fs::remove_dir_all(storage.root());
    }

    #[test]
    fn interrupted_temp_and_backup_state_restores_backup_and_lists_temp() {
        let storage = storage("interrupted");
        let primary = storage.file_path(AtlasDirectory::Queue, "ITEM.DAT");
        fs::write(sibling(&primary, "TMP"), integrity_envelope(b"new")).unwrap();
        fs::write(sibling(&primary, "BAK"), integrity_envelope(b"old")).unwrap();
        assert_eq!(
            storage
                .read_bytes(AtlasDirectory::Queue, "ITEM.DAT")
                .unwrap(),
            b"old"
        );
        let listing = storage.list(AtlasDirectory::Queue).unwrap();
        assert!(listing.entries.iter().any(|entry| entry.name == "ITEM.TMP"
            && entry.disposition == AtlasEntryDisposition::RecoveryArtifact));
        let _ = fs::remove_dir_all(storage.root());
    }

    #[test]
    fn listing_is_deterministic_and_isolates_unknown_and_corrupt_entries() {
        let storage = AtlasStorage::with_limits(
            temp_root("listing"),
            AtlasStorageLimits {
                max_file_bytes: 4,
                max_cache_bytes: 8,
                max_directory_entries: 2,
            },
        )
        .unwrap();
        let queue = storage.directory_path(AtlasDirectory::Queue);
        fs::write(queue.join("ZED.DAT"), integrity_envelope(b"ok")).unwrap();
        fs::write(queue.join("ALPHA.DAT"), integrity_envelope(b"ok")).unwrap();
        fs::write(queue.join("ODD-name"), b"preserve").unwrap();
        fs::write(queue.join("LARGE.DAT"), vec![b'x'; 18]).unwrap();
        let listing = storage.list(AtlasDirectory::Queue).unwrap();
        assert_eq!(
            listing
                .entries
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            vec!["ALPHA.DAT", "LARGE.DAT"]
        );
        assert_eq!(listing.omitted_entries, 2);
        assert_eq!(listing.corrupt_entries, 1);
        assert_eq!(listing.unknown_entries, 1);
        assert!(queue.join("ODD-name").exists());
        let _ = fs::remove_dir_all(storage.root());
    }

    #[test]
    fn under_limit_corruption_falls_back_to_valid_backup() {
        let storage = AtlasStorage::with_limits(
            temp_root("under-limit-corruption"),
            AtlasStorageLimits {
                max_file_bytes: 32,
                max_cache_bytes: 128,
                max_directory_entries: 8,
            },
        )
        .unwrap();
        let primary = storage.file_path(AtlasDirectory::Queue, "ITEM.DAT");
        let backup = sibling(&primary, "BAK");
        fs::write(&primary, b"ATLS\x01\x04\0\0\0\0\0\0\0cut").unwrap();
        fs::write(&backup, integrity_envelope(b"good")).unwrap();

        assert_eq!(
            storage
                .read_bytes(AtlasDirectory::Queue, "ITEM.DAT")
                .unwrap(),
            b"good"
        );
        let _ = fs::remove_dir_all(storage.root());
    }

    #[test]
    fn listing_marks_under_limit_invalid_primary_as_corrupt() {
        let storage = AtlasStorage::with_limits(
            temp_root("listing-under-limit-corruption"),
            AtlasStorageLimits {
                max_file_bytes: 32,
                max_cache_bytes: 128,
                max_directory_entries: 8,
            },
        )
        .unwrap();
        let primary = storage.file_path(AtlasDirectory::Queue, "ITEM.DAT");
        fs::write(&primary, b"ATLS\x01\x04\0\0\0\0\0\0\0cut").unwrap();

        let listing = storage.list(AtlasDirectory::Queue).unwrap();
        assert_eq!(listing.corrupt_entries, 1);
        assert!(listing.entries.iter().any(|entry| {
            entry.name == "ITEM.DAT" && entry.disposition == AtlasEntryDisposition::Corrupt
        }));
        let _ = fs::remove_dir_all(storage.root());
    }

    #[test]
    fn both_invalid_candidates_fail_without_recovery() {
        let storage = storage("both-invalid");
        let primary = storage.file_path(AtlasDirectory::Queue, "ITEM.DAT");
        let backup = sibling(&primary, "BAK");
        fs::write(&primary, b"bad").unwrap();
        fs::write(&backup, b"also bad").unwrap();

        assert!(matches!(
            storage.read_bytes(AtlasDirectory::Queue, "ITEM.DAT"),
            Err(AtlasStorageError::InvalidIntegrity)
        ));
        assert!(primary.exists());
        assert!(backup.exists());
        let _ = fs::remove_dir_all(storage.root());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_without_following_them() {
        use std::os::unix::fs::symlink;
        let storage = storage("symlink");
        let target = storage.root().parent().unwrap().join("outside.txt");
        fs::write(&target, b"outside").unwrap();
        let link = storage.file_path(AtlasDirectory::Queue, "ITEM.DAT");
        symlink(&target, &link).unwrap();
        assert!(matches!(
            storage.read_bytes(AtlasDirectory::Queue, "ITEM.DAT"),
            Err(AtlasStorageError::Symlink(_))
        ));
        assert_eq!(fs::read(&target).unwrap(), b"outside");
        let _ = fs::remove_file(target);
        let _ = fs::remove_dir_all(storage.root());
    }
}
