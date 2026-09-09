//! Complete, bounded normalized Book replicas on SD.
//!
//! This is deliberately separate from the small disposable Atlas cache. Book
//! IDs are content hashes, so a completed directory is immutable and can be
//! promoted atomically after every expected segment validates.

use std::{
    collections::BTreeSet,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use crate::atlas_dto::{
    AtlasBookSummary, BookContentSegment, BookCoverBitmap, BookManifest, BookSummaryPage,
    BOOK_COVER_BITMAP_BYTES, MAX_BOOK_SEGMENT_BLOCKS, MAX_BOOK_SUMMARIES,
};

const CATALOG_FILE: &str = "CATALOG.JSN";
const MANIFEST_FILE: &str = "MANIF.JSN";
const COVER_FILE: &str = "COVER.PBM";
const READY_FILE: &str = "READY";
const MAX_BOOK_TEXT_BYTES: u64 = 48 * 1024 * 1024;
const MAX_LIBRARY_TEXT_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug)]
pub enum AtlasBookStoreError {
    InvalidRoot,
    InvalidBookId,
    InvalidData,
    Limit,
    Io(io::Error),
}

impl fmt::Display for AtlasBookStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRoot => formatter.write_str("invalid Atlas Book store root"),
            Self::InvalidBookId => formatter.write_str("invalid Atlas Book id"),
            Self::InvalidData => formatter.write_str("invalid Atlas Book replica"),
            Self::Limit => formatter.write_str("Atlas Book replica exceeds its limit"),
            Self::Io(error) => write!(formatter, "Atlas Book storage error: {error}"),
        }
    }
}

impl std::error::Error for AtlasBookStoreError {}

impl From<io::Error> for AtlasBookStoreError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Clone, Debug)]
pub struct AtlasBookStore {
    root: PathBuf,
}

impl AtlasBookStore {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, AtlasBookStoreError> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        reject_symlink(&root)?;
        Ok(Self { root })
    }

    pub fn offline_list(&self) -> Result<BookSummaryPage, AtlasBookStoreError> {
        let path = self.root.join(CATALOG_FILE);
        let bytes = read_bounded(&path, 64 * 1024)?;
        let page: BookSummaryPage =
            serde_json::from_slice(&bytes).map_err(|_| AtlasBookStoreError::InvalidData)?;
        if page.next_cursor.is_some() || page.items.len() > MAX_BOOK_SUMMARIES {
            return Err(AtlasBookStoreError::InvalidData);
        }
        Ok(page)
    }

    pub fn begin(&self, manifest: &BookManifest) -> Result<(), AtlasBookStoreError> {
        validate_manifest(manifest)?;
        let stage = self.stage(&manifest.book.id)?;
        if stage.exists() {
            reject_symlink(&stage)?;
            fs::remove_dir_all(&stage)?;
        }
        fs::create_dir(&stage)?;
        for spine in &manifest.spine {
            fs::create_dir(stage.join(spine_dir(spine.index)))?;
        }
        write_atomic(
            &stage.join(MANIFEST_FILE),
            &serde_json::to_vec(manifest).map_err(|_| AtlasBookStoreError::InvalidData)?,
        )
    }

    pub fn store_cover(
        &self,
        book_id: &str,
        cover: &BookCoverBitmap,
    ) -> Result<(), AtlasBookStoreError> {
        if cover.pixels.len() != BOOK_COVER_BITMAP_BYTES {
            return Err(AtlasBookStoreError::InvalidData);
        }
        write_atomic(&self.stage(book_id)?.join(COVER_FILE), &cover.pixels)
    }

    pub fn store_segment(
        &self,
        book_id: &str,
        segment: &BookContentSegment,
    ) -> Result<(), AtlasBookStoreError> {
        if segment.book_id != book_id
            || segment.blocks.is_empty()
            || segment.blocks.len() > MAX_BOOK_SEGMENT_BLOCKS
        {
            return Err(AtlasBookStoreError::InvalidData);
        }
        let first = segment.blocks[0].index;
        if first as usize % MAX_BOOK_SEGMENT_BLOCKS != 0 {
            return Err(AtlasBookStoreError::InvalidData);
        }
        for (offset, block) in segment.blocks.iter().enumerate() {
            if usize::from(block.index) != usize::from(first) + offset {
                return Err(AtlasBookStoreError::InvalidData);
            }
        }
        let path = self
            .stage(book_id)?
            .join(spine_dir(segment.spine_item))
            .join(segment_file(first));
        let bytes = serde_json::to_vec(segment).map_err(|_| AtlasBookStoreError::InvalidData)?;
        if bytes.len() > 32 * 1024 {
            return Err(AtlasBookStoreError::Limit);
        }
        write_atomic(&path, &bytes)
    }

    pub fn finish(&self, manifest: &BookManifest) -> Result<(), AtlasBookStoreError> {
        validate_manifest(manifest)?;
        let stage = self.stage(&manifest.book.id)?;
        reject_symlink(&stage)?;
        if manifest.book.cover_url.is_some()
            && read_bounded(&stage.join(COVER_FILE), BOOK_COVER_BITMAP_BYTES)?.len()
                != BOOK_COVER_BITMAP_BYTES
        {
            return Err(AtlasBookStoreError::InvalidData);
        }
        for spine in &manifest.spine {
            for block in (0..spine.block_count).step_by(MAX_BOOK_SEGMENT_BLOCKS) {
                let path = stage.join(spine_dir(spine.index)).join(segment_file(block));
                let segment: BookContentSegment =
                    serde_json::from_slice(&read_bounded(&path, 32 * 1024)?)
                        .map_err(|_| AtlasBookStoreError::InvalidData)?;
                let expected = usize::from(spine.block_count.saturating_sub(block))
                    .min(MAX_BOOK_SEGMENT_BLOCKS);
                if segment.book_id != manifest.book.id
                    || segment.spine_item != spine.index
                    || segment.blocks.len() != expected
                    || segment.blocks.first().map(|item| item.index) != Some(block)
                    || segment.blocks.iter().enumerate().any(|(offset, item)| {
                        usize::from(item.index) != usize::from(block) + offset
                    })
                {
                    return Err(AtlasBookStoreError::InvalidData);
                }
            }
        }
        write_atomic(&stage.join(READY_FILE), b"1\n")?;
        let completed = self.completed(&manifest.book.id)?;
        if completed.exists() {
            reject_symlink(&completed)?;
            fs::remove_dir_all(stage)?;
        } else {
            fs::rename(stage, completed)?;
        }
        Ok(())
    }

    pub fn replace_catalog(
        &self,
        server: &[AtlasBookSummary],
    ) -> Result<BookSummaryPage, AtlasBookStoreError> {
        if server.len() > MAX_BOOK_SUMMARIES {
            return Err(AtlasBookStoreError::Limit);
        }
        let mut total = 0_u64;
        let mut items = Vec::new();
        for summary in server {
            if !self.is_complete(&summary.id) {
                return Err(AtlasBookStoreError::InvalidData);
            }
            let manifest = self.manifest(&summary.id)?;
            total = total.saturating_add(
                manifest
                    .spine
                    .iter()
                    .map(|item| u64::from(item.text_bytes))
                    .sum::<u64>(),
            );
            if total > MAX_LIBRARY_TEXT_BYTES {
                return Err(AtlasBookStoreError::Limit);
            }
            items.push(summary.clone());
        }
        let page = BookSummaryPage {
            items,
            next_cursor: None,
        };
        write_atomic(
            &self.root.join(CATALOG_FILE),
            &serde_json::to_vec(&page).map_err(|_| AtlasBookStoreError::InvalidData)?,
        )?;
        Ok(page)
    }

    pub fn manifest(&self, book_id: &str) -> Result<BookManifest, AtlasBookStoreError> {
        let completed = self.completed(book_id)?;
        reject_symlink(&completed)?;
        let bytes = read_bounded(&completed.join(MANIFEST_FILE), 64 * 1024)?;
        let manifest =
            serde_json::from_slice(&bytes).map_err(|_| AtlasBookStoreError::InvalidData)?;
        validate_manifest(&manifest)?;
        Ok(manifest)
    }

    pub fn cover(&self, book_id: &str) -> Result<BookCoverBitmap, AtlasBookStoreError> {
        let completed = self.completed(book_id)?;
        reject_symlink(&completed)?;
        let pixels = read_bounded(&completed.join(COVER_FILE), BOOK_COVER_BITMAP_BYTES)?;
        if pixels.len() != BOOK_COVER_BITMAP_BYTES {
            return Err(AtlasBookStoreError::InvalidData);
        }
        Ok(BookCoverBitmap { pixels })
    }

    pub fn segment(
        &self,
        book_id: &str,
        spine_item: u16,
        block: u16,
    ) -> Result<BookContentSegment, AtlasBookStoreError> {
        let first = block - block % MAX_BOOK_SEGMENT_BLOCKS as u16;
        let completed = self.completed(book_id)?;
        reject_symlink(&completed)?;
        let path = completed
            .join(spine_dir(spine_item))
            .join(segment_file(first));
        let segment: BookContentSegment = serde_json::from_slice(&read_bounded(&path, 32 * 1024)?)
            .map_err(|_| AtlasBookStoreError::InvalidData)?;
        if segment.book_id != book_id
            || segment.spine_item != spine_item
            || !segment.blocks.iter().any(|item| item.index == block)
        {
            return Err(AtlasBookStoreError::InvalidData);
        }
        Ok(segment)
    }

    pub fn is_complete(&self, book_id: &str) -> bool {
        let Ok(path) = self.completed(book_id) else {
            return false;
        };
        if reject_symlink(&path).is_err() || !path.join(READY_FILE).is_file() {
            return false;
        }
        let Ok(bytes) = read_bounded(&path.join(MANIFEST_FILE), 64 * 1024) else {
            return false;
        };
        let Ok(manifest) = serde_json::from_slice::<BookManifest>(&bytes) else {
            return false;
        };
        if validate_manifest(&manifest).is_err() || manifest.book.id != book_id {
            return false;
        }
        manifest.book.cover_url.is_none()
            || read_bounded(&path.join(COVER_FILE), BOOK_COVER_BITMAP_BYTES)
                .is_ok_and(|pixels| pixels.len() == BOOK_COVER_BITMAP_BYTES)
    }

    fn completed(&self, book_id: &str) -> Result<PathBuf, AtlasBookStoreError> {
        Ok(self.root.join(book_hash(book_id)?))
    }

    fn stage(&self, book_id: &str) -> Result<PathBuf, AtlasBookStoreError> {
        Ok(self.root.join(format!("{}.TMP", book_hash(book_id)?)))
    }
}

fn validate_manifest(manifest: &BookManifest) -> Result<(), AtlasBookStoreError> {
    book_hash(&manifest.book.id)?;
    let mut indexes = BTreeSet::new();
    let text = manifest.spine.iter().try_fold(0_u64, |total, item| {
        if !indexes.insert(item.index) {
            return Err(AtlasBookStoreError::InvalidData);
        }
        Ok(total.saturating_add(u64::from(item.text_bytes)))
    })?;
    if text > MAX_BOOK_TEXT_BYTES {
        return Err(AtlasBookStoreError::Limit);
    }
    Ok(())
}

fn book_hash(book_id: &str) -> Result<&str, AtlasBookStoreError> {
    let Some(hash) = book_id.strip_prefix("book_") else {
        return Err(AtlasBookStoreError::InvalidBookId);
    };
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(AtlasBookStoreError::InvalidBookId);
    }
    Ok(hash)
}

fn spine_dir(index: u16) -> String {
    format!("S{index:03}")
}

fn segment_file(block: u16) -> String {
    format!("B{block:04}.JSN")
}

fn reject_symlink(path: &Path) -> Result<(), AtlasBookStoreError> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(AtlasBookStoreError::InvalidRoot);
    }
    Ok(())
}

fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, AtlasBookStoreError> {
    recover_atomic(path)?;
    reject_symlink(path)?;
    let file = File::open(path)?;
    if file.metadata()?.len() > limit as u64 {
        return Err(AtlasBookStoreError::Limit);
    }
    let mut bytes = Vec::with_capacity(file.metadata()?.len() as usize);
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(AtlasBookStoreError::Limit);
    }
    Ok(bytes)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), AtlasBookStoreError> {
    let tmp = path.with_extension("TMP");
    let backup = path.with_extension("BAK");
    if tmp.exists() {
        reject_symlink(&tmp)?;
        fs::remove_file(&tmp)?;
    }
    let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    if backup.exists() {
        reject_symlink(&backup)?;
        fs::remove_file(&backup)?;
    }
    if path.exists() {
        reject_symlink(path)?;
        fs::rename(path, &backup)?;
    }
    if let Err(error) = fs::rename(&tmp, path) {
        if backup.exists() && !path.exists() {
            let _ = fs::rename(&backup, path);
        }
        return Err(error.into());
    }
    if backup.exists() {
        fs::remove_file(backup)?;
    }
    Ok(())
}

fn recover_atomic(path: &Path) -> Result<(), AtlasBookStoreError> {
    let backup = path.with_extension("BAK");
    if path.exists() {
        reject_symlink(path)?;
        if backup.exists() {
            reject_symlink(&backup)?;
            fs::remove_file(backup)?;
        }
    } else if backup.exists() {
        reject_symlink(&backup)?;
        fs::rename(backup, path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atlas_dto::{BookBlockKind, BookContentBlock, BookImportStatus, BookSpineItem};

    fn manifest() -> BookManifest {
        BookManifest {
            book: AtlasBookSummary {
                id: format!("book_{}", "a".repeat(64)),
                title: "País català".into(),
                authors: vec!["Autora".into()],
                language: Some("ca".into()),
                byte_size: 123,
                import_status: BookImportStatus::Ready,
                cover_url: Some("/cover".into()),
            },
            spine: vec![BookSpineItem {
                index: 0,
                label: "One".into(),
                block_count: 25,
                text_bytes: 10,
            }],
            toc: vec![],
        }
    }

    fn segment(first: u16, count: u16) -> BookContentSegment {
        BookContentSegment {
            book_id: format!("book_{}", "a".repeat(64)),
            spine_item: 0,
            cursor: None,
            next_cursor: (first == 0).then(|| "next".into()),
            blocks: (first..first + count)
                .map(|index| BookContentBlock {
                    index,
                    kind: BookBlockKind::Paragraph,
                    text: format!("block {index}"),
                })
                .collect(),
        }
    }

    #[test]
    fn promotes_only_complete_books_and_reopens_random_blocks_offline() {
        let root = std::env::temp_dir().join(format!("atlas-book-store-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let store = AtlasBookStore::new(&root).unwrap();
        let manifest = manifest();
        store.begin(&manifest).unwrap();
        store
            .store_segment(&manifest.book.id, &segment(0, 24))
            .unwrap();
        assert!(store.finish(&manifest).is_err());
        store
            .store_segment(&manifest.book.id, &segment(24, 1))
            .unwrap();
        assert!(store.finish(&manifest).is_err());
        store
            .store_cover(
                &manifest.book.id,
                &BookCoverBitmap {
                    pixels: vec![0; BOOK_COVER_BITMAP_BYTES],
                },
            )
            .unwrap();
        store.finish(&manifest).unwrap();
        let list = store
            .replace_catalog(std::slice::from_ref(&manifest.book))
            .unwrap();
        assert_eq!(list.items.len(), 1);
        assert_eq!(
            store.segment(&manifest.book.id, 0, 24).unwrap().blocks[0].index,
            24
        );
        fs::rename(root.join(CATALOG_FILE), root.join("CATALOG.BAK")).unwrap();
        assert_eq!(store.offline_list().unwrap().items.len(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_non_contiguous_segment_before_it_can_be_promoted() {
        let root = std::env::temp_dir().join(format!("atlas-book-gap-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let store = AtlasBookStore::new(&root).unwrap();
        let manifest = manifest();
        store.begin(&manifest).unwrap();
        let mut invalid = segment(0, 2);
        invalid.blocks[1].index = 3;
        assert!(store.store_segment(&manifest.book.id, &invalid).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_failed_new_replica_cannot_empty_the_existing_catalogue() {
        let root =
            std::env::temp_dir().join(format!("atlas-book-store-catalog-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let store = AtlasBookStore::new(&root).unwrap();
        let manifest = manifest();
        store.begin(&manifest).unwrap();
        store
            .store_segment(&manifest.book.id, &segment(0, 24))
            .unwrap();
        store
            .store_segment(&manifest.book.id, &segment(24, 1))
            .unwrap();
        store
            .store_cover(
                &manifest.book.id,
                &BookCoverBitmap {
                    pixels: vec![0; BOOK_COVER_BITMAP_BYTES],
                },
            )
            .unwrap();
        store.finish(&manifest).unwrap();
        store
            .replace_catalog(std::slice::from_ref(&manifest.book))
            .unwrap();
        let missing = AtlasBookSummary {
            id: format!("book_{}", "b".repeat(64)),
            ..manifest.book.clone()
        };
        assert!(store.replace_catalog(&[missing]).is_err());
        assert_eq!(store.offline_list().unwrap().items.len(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}
