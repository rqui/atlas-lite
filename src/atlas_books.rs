//! Bounded remote Books state. EPUB parsing remains server-side; the device
//! retains only safe reflowable blocks and applies its existing Reader layout.

use crate::{
    atlas_book_store::AtlasBookStore,
    atlas_cache::{AtlasCacheMetadata, AtlasCacheRepository},
    atlas_client::{AtlasClient, AtlasClientError, AtlasTransport},
    atlas_dto::{
        AtlasBookSummary, BookBlockKind, BookBookmarks, BookContentSegment, BookCoverBitmap,
        BookManifest, BookReadingAnchor, BookSummaryPage,
    },
    reader::{paginate_reflowable_text, ReaderLayout},
};
use std::collections::VecDeque;

pub const BOOK_LIST_LIMIT: usize = 32;
pub const REMOTE_PAGE_CACHE_LIMIT: usize = 8;
pub const REMOTE_SEGMENT_CACHE_LIMIT: usize = 2;
pub const LIST_COVER_CACHE_LIMIT: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BooksConnection {
    Unconfigured,
    Connecting,
    Connected,
    Offline,
    Error,
    /// The pairing is retained; a scope-upgrade pairing is required.
    RePairRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BooksView {
    List,
    Detail,
    Toc,
    Bookmarks,
    Reader,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteReaderPage {
    pub anchor: BookReadingAnchor,
    pub next_anchor: BookReadingAnchor,
    pub lines: Vec<RemoteReaderLine>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteReaderLine {
    pub text: String,
    pub paragraph_end: bool,
    pub kind: BookBlockKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PendingBookRequest {
    List,
    Open {
        id: String,
    },
    Segment {
        spine_item: u16,
        block: u16,
        anchor: BookReadingAnchor,
        record_history: bool,
    },
    SyncProgress {
        anchor: BookReadingAnchor,
    },
    Bookmark {
        anchor: BookReadingAnchor,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum BookSyncStep {
    Manifest {
        id: String,
    },
    Cover {
        manifest: BookManifest,
    },
    Segment {
        manifest: BookManifest,
        spine_position: usize,
        block: u16,
    },
    Finish {
        manifest: BookManifest,
    },
}

/// Device Book state. The live state remains bounded in RAM; when an optional
/// Atlas cache is available, list/detail and recently read segments are also
/// persisted as stale offline copies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasBooksState {
    pub view: BooksView,
    pub connection: BooksConnection,
    pub books: Vec<AtlasBookSummary>,
    /// The server returned another cursor; the local 32-item snapshot is not
    /// an authoritative total.
    pub list_has_more: bool,
    /// A successful empty list is distinct from an unrequested or failed list.
    pub list_loaded: bool,
    pub selected: usize,
    pub manifest: Option<BookManifest>,
    pub bookmarks: BookBookmarks,
    pub current_segment: Option<BookContentSegment>,
    pub current_cover: Option<BookCoverBitmap>,
    pub adjacent_segment: Option<BookContentSegment>,
    pub current_page: Option<RemoteReaderPage>,
    pub resume_anchor: Option<BookReadingAnchor>,
    /// Progress is deliberately retained only for the one book whose existing
    /// progress endpoint was already requested while opening its manifest.
    pub resume_percentage: Option<u8>,
    page_history: Vec<RemoteReaderPage>,
    current_book_id: Option<String>,
    current_cover_book_id: Option<String>,
    list_covers: Vec<(String, BookCoverBitmap)>,
    pending: Option<PendingBookRequest>,
    pub feedback: Option<&'static str>,
    page_turns_since_sync: u8,
    progress_dirty: bool,
    sync_queue: VecDeque<BookSyncStep>,
    sync_catalog: Option<BookSummaryPage>,
    sync_failed: bool,
}

impl Default for AtlasBooksState {
    fn default() -> Self {
        Self {
            view: BooksView::List,
            connection: BooksConnection::Unconfigured,
            books: Vec::new(),
            list_has_more: false,
            list_loaded: false,
            selected: 0,
            manifest: None,
            bookmarks: BookBookmarks { items: Vec::new() },
            current_segment: None,
            current_cover: None,
            adjacent_segment: None,
            current_page: None,
            resume_anchor: None,
            resume_percentage: None,
            page_history: Vec::new(),
            current_book_id: None,
            current_cover_book_id: None,
            list_covers: Vec::new(),
            pending: None,
            feedback: None,
            page_turns_since_sync: 0,
            progress_dirty: false,
            sync_queue: VecDeque::new(),
            sync_catalog: None,
            sync_failed: false,
        }
    }
}

impl AtlasBooksState {
    pub fn hydrate_cached_list(&mut self, page: crate::atlas_dto::BookSummaryPage) {
        self.list_has_more = page.next_cursor.is_some();
        self.books = page.items;
        self.list_loaded = true;
        self.selected = self.selected.min(self.books.len().saturating_sub(1));
        self.connection = BooksConnection::Offline;
        self.feedback = Some("Offline — showing saved books");
    }

    pub fn hydrate_offline_store(&mut self, store: &AtlasBookStore) {
        if let Ok(page) = store.offline_list() {
            self.hydrate_cached_list(page);
            self.list_covers.clear();
            for book in self.books.iter().take(LIST_COVER_CACHE_LIMIT) {
                if let Ok(cover) = store.cover(&book.id) {
                    self.list_covers.push((book.id.clone(), cover));
                }
            }
        }
    }

    pub fn request_list(&mut self) {
        if self.pending.is_none() {
            self.pending = Some(PendingBookRequest::List);
            self.connection = BooksConnection::Connecting;
        }
    }

    #[must_use]
    pub const fn has_pending_request(&self) -> bool {
        self.pending.is_some()
    }

    #[must_use]
    pub fn has_background_sync(&self) -> bool {
        !self.sync_queue.is_empty()
    }

    #[must_use]
    pub fn selected_book(&self) -> Option<&AtlasBookSummary> {
        self.books.get(self.selected)
    }

    pub fn apply(&mut self, up: bool, select: bool, layout: ReaderLayout) {
        match self.view {
            BooksView::List => {
                if self.books.is_empty() {
                    if select {
                        self.request_list();
                    }
                    return;
                }
                if up {
                    self.selected = self.selected.checked_sub(1).unwrap_or(self.books.len() - 1);
                } else if !select {
                    self.selected = (self.selected + 1) % self.books.len();
                }
                if select {
                    let id = self.books[self.selected].id.clone();
                    self.pending = Some(PendingBookRequest::Open { id });
                    self.connection = BooksConnection::Connecting;
                }
            }
            BooksView::Detail => {
                // The detail action cycle is Reader, TOC, Bookmarks. This keeps
                // the narrow three-button surface deterministic.
                if up {
                    self.selected = self.selected.checked_sub(1).unwrap_or(2);
                } else if !select {
                    self.selected = (self.selected + 1) % 3;
                }
                if select {
                    match self.selected {
                        0 => self.start_reader(
                            self.resume_anchor.unwrap_or(BookReadingAnchor {
                                spine_item: 0,
                                block: 0,
                                character_offset: 0,
                            }),
                            layout,
                        ),
                        1 => {
                            self.view = BooksView::Toc;
                            self.selected = 0;
                        }
                        _ => {
                            self.view = BooksView::Bookmarks;
                            self.selected = 0;
                        }
                    }
                }
            }
            BooksView::Toc => {
                let Some(manifest) = self.manifest.as_ref() else {
                    return;
                };
                if manifest.toc.is_empty() {
                    return;
                }
                if up {
                    self.selected = self
                        .selected
                        .checked_sub(1)
                        .unwrap_or(manifest.toc.len() - 1);
                } else if !select {
                    self.selected = (self.selected + 1) % manifest.toc.len();
                }
                if select {
                    let entry = &manifest.toc[self.selected];
                    self.start_reader(
                        BookReadingAnchor {
                            spine_item: entry.spine_item,
                            block: entry.block,
                            character_offset: 0,
                        },
                        layout,
                    );
                }
            }
            BooksView::Bookmarks => {
                if self.bookmarks.items.is_empty() {
                    return;
                }
                if up {
                    self.selected = self
                        .selected
                        .checked_sub(1)
                        .unwrap_or(self.bookmarks.items.len() - 1);
                } else if !select {
                    self.selected = (self.selected + 1) % self.bookmarks.items.len();
                }
                if select {
                    self.start_reader(self.bookmarks.items[self.selected].anchor, layout);
                }
            }
            BooksView::Reader => {
                if select {
                    if let Some(page) = self.current_page.as_ref() {
                        self.pending = Some(PendingBookRequest::Bookmark {
                            anchor: page.anchor,
                        });
                    }
                } else if up {
                    self.previous_page();
                } else {
                    self.next_page(layout);
                }
            }
        }
    }

    pub fn leave_reader(&mut self) {
        if self.view == BooksView::Reader && self.progress_dirty {
            if let Some(page) = self.current_page.as_ref() {
                self.pending = Some(PendingBookRequest::SyncProgress {
                    anchor: page.anchor,
                });
            }
        }
        self.view = BooksView::Detail;
        self.selected = 0;
    }

    /// Consume one level of the Books-local hierarchy. A reader exit queues a
    /// best-effort progress sync, but never requires the radio to stay awake.
    /// Returns false only when the Books list itself is already the root.
    pub fn back(&mut self) -> bool {
        match self.view {
            BooksView::Reader => {
                self.leave_reader();
                true
            }
            BooksView::Toc | BooksView::Bookmarks => {
                self.view = BooksView::Detail;
                self.selected = 0;
                true
            }
            BooksView::Detail => {
                self.view = BooksView::List;
                self.selected = self
                    .current_book_id
                    .as_ref()
                    .and_then(|id| self.books.iter().position(|book| &book.id == id))
                    .unwrap_or(0);
                true
            }
            BooksView::List => false,
        }
    }

    pub fn consume<T: AtlasTransport>(
        &mut self,
        client: &mut AtlasClient<T>,
        layout: ReaderLayout,
    ) -> bool {
        self.consume_with_cache(client, layout, None)
    }

    pub fn consume_with_cache<T: AtlasTransport>(
        &mut self,
        client: &mut AtlasClient<T>,
        layout: ReaderLayout,
        cache: Option<&AtlasCacheRepository>,
    ) -> bool {
        self.consume_with_stores(client, layout, cache, None)
    }

    pub fn consume_with_stores<T: AtlasTransport>(
        &mut self,
        client: &mut AtlasClient<T>,
        layout: ReaderLayout,
        cache: Option<&AtlasCacheRepository>,
        store: Option<&AtlasBookStore>,
    ) -> bool {
        let Some(request) = self.pending.take() else {
            if let Some(store) = store {
                let covers_changed = self.refresh_list_cover_window(store);
                return self.consume_sync(client, store) || covers_changed;
            }
            return false;
        };
        match request {
            PendingBookRequest::List => match client.list_books(None, BOOK_LIST_LIMIT) {
                Ok(page) => {
                    self.list_has_more = page.next_cursor.is_some();
                    self.books = page.items.clone();
                    self.list_loaded = true;
                    self.selected = 0;
                    self.connection = BooksConnection::Connected;
                    self.feedback = None;
                    if let Some(store) = store {
                        self.schedule_sync(store, &page);
                    }
                    if let Some(cache) = cache {
                        let _ = cache.store_book_list(page, AtlasCacheMetadata::default());
                    }
                }
                Err(error) => {
                    self.record_error(&error);
                    if let Some(cached) = cache.and_then(|cache| cache.offline_book_list().value) {
                        self.list_has_more = cached.next_cursor.is_some();
                        self.books = cached.items;
                        self.list_loaded = true;
                        self.selected = self.selected.min(self.books.len().saturating_sub(1));
                        self.feedback = Some("Offline — showing saved books");
                    }
                }
            },
            PendingBookRequest::Open { id } => match client.get_book_manifest(&id) {
                Ok(manifest) => {
                    self.current_book_id = Some(id.clone());
                    self.manifest = Some(manifest.clone());
                    let live_cover = manifest
                        .book
                        .cover_url
                        .as_ref()
                        .and_then(|_| client.get_book_cover(&id).ok());
                    let cover = live_cover
                        .clone()
                        .or_else(|| cache.and_then(|cache| cache.offline_book_cover(&id).value));
                    self.current_cover = cover;
                    self.current_cover_book_id = self.current_cover.as_ref().map(|_| id.clone());
                    let progress = client.get_book_progress(&id).ok().flatten();
                    self.resume_anchor = progress.as_ref().map(|value| value.anchor);
                    self.resume_percentage = progress.as_ref().map(|value| value.percentage);
                    self.bookmarks = client
                        .list_book_bookmarks(&id)
                        .unwrap_or(BookBookmarks { items: Vec::new() });
                    self.view = BooksView::Detail;
                    self.selected = 0;
                    self.connection = BooksConnection::Connected;
                    self.feedback = None;
                    if let Some(cache) = cache {
                        let _ = cache.store_book_manifest(manifest, AtlasCacheMetadata::default());
                        if let Some(cover) = live_cover {
                            let _ =
                                cache.store_book_cover(&id, cover, AtlasCacheMetadata::default());
                        }
                        if let Some(progress) = progress {
                            let _ =
                                cache.store_book_progress(progress, AtlasCacheMetadata::default());
                        }
                        let _ = cache.store_book_bookmarks(
                            &id,
                            self.bookmarks.clone(),
                            AtlasCacheMetadata::default(),
                        );
                    }
                }
                Err(error) => {
                    self.record_error(&error);
                    if let Some(cache) = cache {
                        if let Some(manifest) = cache.offline_book_manifest(&id).value {
                            self.current_book_id = Some(id.clone());
                            self.manifest = Some(manifest);
                            self.current_cover = cache.offline_book_cover(&id).value;
                            self.current_cover_book_id =
                                self.current_cover.as_ref().map(|_| id.clone());
                            self.bookmarks = cache
                                .offline_book_bookmarks(&id)
                                .value
                                .unwrap_or(BookBookmarks { items: Vec::new() });
                            let progress = cache.offline_book_progress(&id).value;
                            self.resume_anchor = progress.as_ref().map(|value| value.anchor);
                            self.resume_percentage = progress.map(|value| value.percentage);
                            self.view = BooksView::Detail;
                            self.selected = 0;
                            self.feedback = Some("Offline — saved copy");
                        }
                    }
                    if self.manifest.is_none() {
                        if let Some(store) = store {
                            if let Ok(manifest) = store.manifest(&id) {
                                self.current_book_id = Some(id.clone());
                                self.manifest = Some(manifest);
                                self.current_cover = store.cover(&id).ok();
                                self.current_cover_book_id =
                                    self.current_cover.as_ref().map(|_| id.clone());
                                self.view = BooksView::Detail;
                                self.selected = 0;
                                self.connection = BooksConnection::Offline;
                                self.feedback = Some("Offline — downloaded book");
                            }
                        }
                    }
                }
            },
            PendingBookRequest::Segment {
                spine_item,
                block,
                anchor,
                record_history,
            } => {
                let Some(id) = self.current_book_id.clone() else {
                    self.feedback = Some("BOOK NOT SELECTED");
                    return true;
                };
                match client.get_book_content_at(&id, spine_item, block) {
                    Ok(segment) => {
                        if let Some(cache) = cache {
                            let _ = cache
                                .store_book_segment(segment.clone(), AtlasCacheMetadata::default());
                        }
                        self.cache_segment(segment);
                        self.show_page(anchor, layout, record_history);
                        self.connection = BooksConnection::Connected;
                    }
                    Err(error) => {
                        self.record_error(&error);
                        if let Some(segment) = cache.and_then(|cache| {
                            cache.offline_book_segment(&id, spine_item, block).value
                        }) {
                            self.cache_segment(segment);
                            self.show_page(anchor, layout, record_history);
                            self.connection = BooksConnection::Offline;
                            self.feedback = Some("Offline — saved copy");
                        }
                        if self.segment_for_anchor(anchor).is_none() {
                            if let Some(segment) =
                                store.and_then(|store| store.segment(&id, spine_item, block).ok())
                            {
                                self.cache_segment(segment);
                                self.show_page(anchor, layout, record_history);
                                self.connection = BooksConnection::Offline;
                                self.feedback = Some("Offline — downloaded book");
                            }
                        }
                    }
                }
            }
            PendingBookRequest::SyncProgress { anchor } => {
                let Some(id) = self.current_book_id.clone() else {
                    return true;
                };
                match client.put_book_progress(&id, anchor) {
                    Ok(_) => {
                        self.progress_dirty = false;
                        self.page_turns_since_sync = 0;
                    }
                    Err(error) => {
                        self.progress_dirty = true;
                        self.record_error(&error);
                    }
                }
            }
            PendingBookRequest::Bookmark { anchor } => {
                let Some(id) = self.current_book_id.clone() else {
                    return true;
                };
                match client.create_book_bookmark(&id, anchor, None) {
                    Ok(()) => {
                        self.feedback = Some("BOOKMARK SAVED");
                        let _ = client
                            .list_book_bookmarks(&id)
                            .map(|value| self.bookmarks = value);
                    }
                    Err(error) => self.record_error(&error),
                }
            }
        }
        true
    }

    fn schedule_sync(&mut self, store: &AtlasBookStore, page: &BookSummaryPage) {
        if page.next_cursor.is_some() {
            return;
        }
        self.sync_queue.clear();
        self.sync_failed = false;
        self.sync_catalog = Some(page.clone());
        for book in &page.items {
            if !store.is_complete(&book.id) {
                self.sync_queue.push_back(BookSyncStep::Manifest {
                    id: book.id.clone(),
                });
            }
        }
        if self.sync_queue.is_empty() {
            if store.replace_catalog(&page.items).is_err() {
                self.sync_failed = true;
            }
        } else {
            self.feedback = Some("Syncing books for offline reading…");
        }
    }

    fn consume_sync<T: AtlasTransport>(
        &mut self,
        client: &mut AtlasClient<T>,
        store: &AtlasBookStore,
    ) -> bool {
        let Some(step) = self.sync_queue.pop_front() else {
            return false;
        };
        let result: Result<(), ()> = match step {
            BookSyncStep::Manifest { id } => client
                .get_book_manifest(&id)
                .map_err(|_| ())
                .and_then(|manifest| {
                    store.begin(&manifest).map_err(|_| ())?;
                    if manifest.book.cover_url.is_some() {
                        self.sync_queue.push_front(BookSyncStep::Cover { manifest });
                    } else {
                        self.queue_first_segment(manifest);
                    }
                    Ok(())
                }),
            BookSyncStep::Cover { manifest } => client
                .get_book_cover(&manifest.book.id)
                .map_err(|_| ())
                .and_then(|cover| {
                    store
                        .store_cover(&manifest.book.id, &cover)
                        .map_err(|_| ())?;
                    self.remember_list_cover(&manifest.book.id, cover);
                    self.queue_first_segment(manifest);
                    Ok(())
                }),
            BookSyncStep::Segment {
                manifest,
                spine_position,
                block,
            } => {
                let spine_index = manifest.spine[spine_position].index;
                let spine_blocks = manifest.spine[spine_position].block_count;
                client
                    .get_book_content_at(&manifest.book.id, spine_index, block)
                    .map_err(|_| ())
                    .and_then(|segment| {
                        store
                            .store_segment(&manifest.book.id, &segment)
                            .map_err(|_| ())?;
                        let next = block.saturating_add(segment.blocks.len() as u16);
                        if next < spine_blocks {
                            self.sync_queue.push_front(BookSyncStep::Segment {
                                manifest,
                                spine_position,
                                block: next,
                            });
                        } else if let Some(next_spine) = manifest
                            .spine
                            .iter()
                            .enumerate()
                            .skip(spine_position + 1)
                            .find_map(|(position, spine)| {
                                (spine.block_count > 0).then_some(position)
                            })
                        {
                            self.sync_queue.push_front(BookSyncStep::Segment {
                                manifest,
                                spine_position: next_spine,
                                block: 0,
                            });
                        } else {
                            self.sync_queue
                                .push_front(BookSyncStep::Finish { manifest });
                        }
                        Ok(())
                    })
            }
            BookSyncStep::Finish { manifest } => store.finish(&manifest).map_err(|_| ()),
        };
        if result.is_err() {
            self.sync_failed = true;
        }
        if self.sync_queue.is_empty() {
            if !self.sync_failed {
                if let Some(page) = self.sync_catalog.take() {
                    self.sync_failed = store.replace_catalog(&page.items).is_err();
                }
            }
            self.feedback = Some(if self.sync_failed {
                "Unable to finish offline sync"
            } else {
                "Available offline"
            });
        }
        true
    }

    fn queue_first_segment(&mut self, manifest: BookManifest) {
        let first = manifest
            .spine
            .iter()
            .position(|spine| spine.block_count > 0);
        if let Some(spine_position) = first {
            self.sync_queue.push_front(BookSyncStep::Segment {
                manifest,
                spine_position,
                block: 0,
            });
        } else {
            self.sync_queue
                .push_front(BookSyncStep::Finish { manifest });
        }
    }

    #[must_use]
    pub fn cover_for(&self, book_id: &str) -> Option<&BookCoverBitmap> {
        (self.current_cover_book_id.as_deref() == Some(book_id))
            .then_some(self.current_cover.as_ref())
            .flatten()
            .or_else(|| {
                self.list_covers
                    .iter()
                    .find_map(|(id, cover)| (id == book_id).then_some(cover))
            })
    }

    fn remember_list_cover(&mut self, book_id: &str, cover: BookCoverBitmap) {
        if !self
            .books
            .iter()
            .take(LIST_COVER_CACHE_LIMIT)
            .any(|book| book.id == book_id)
        {
            return;
        }
        if let Some((_, existing)) = self.list_covers.iter_mut().find(|(id, _)| id == book_id) {
            *existing = cover;
            return;
        }
        if self.list_covers.len() < LIST_COVER_CACHE_LIMIT {
            self.list_covers.push((book_id.to_owned(), cover));
        }
    }

    fn refresh_list_cover_window(&mut self, store: &AtlasBookStore) -> bool {
        if self.view != BooksView::List || self.books.is_empty() {
            return false;
        }
        let start = self
            .selected
            .saturating_sub(LIST_COVER_CACHE_LIMIT - 1)
            .min(self.books.len().saturating_sub(LIST_COVER_CACHE_LIMIT));
        let wanted = self
            .books
            .iter()
            .skip(start)
            .take(LIST_COVER_CACHE_LIMIT)
            .map(|book| book.id.clone())
            .collect::<Vec<_>>();
        let before = self.list_covers.len();
        self.list_covers.retain(|(id, _)| wanted.contains(id));
        let mut changed = self.list_covers.len() != before;
        for id in wanted {
            if self.list_covers.iter().any(|(cached, _)| cached == &id) {
                continue;
            }
            if let Ok(cover) = store.cover(&id) {
                self.list_covers.push((id, cover));
                changed = true;
            }
        }
        changed
    }

    fn start_reader(&mut self, anchor: BookReadingAnchor, layout: ReaderLayout) {
        self.view = BooksView::Reader;
        self.current_page = None;
        self.page_history.clear();
        self.page_turns_since_sync = 0;
        if self.segment_for_anchor(anchor).is_some() {
            self.show_page(anchor, layout, false);
        } else {
            self.request_segment(anchor, false);
        }
    }

    fn cache_segment(&mut self, segment: BookContentSegment) {
        self.adjacent_segment = self.current_segment.replace(segment);
        if REMOTE_SEGMENT_CACHE_LIMIT < 2 {
            self.adjacent_segment = None;
        }
    }

    fn show_page(
        &mut self,
        anchor: BookReadingAnchor,
        layout: ReaderLayout,
        record_history: bool,
    ) -> bool {
        let page = self
            .segment_for_anchor(anchor)
            .and_then(|segment| page_from_segment(segment, anchor, layout));
        let Some(page) = page else {
            self.feedback = Some("BOOK POSITION UNAVAILABLE");
            return false;
        };
        let previous = self.current_page.replace(page.clone());
        if record_history {
            if let Some(previous) = previous.as_ref() {
                self.remember_page(previous.clone());
            }
            self.progress_dirty = true;
            self.page_turns_since_sync = self.page_turns_since_sync.saturating_add(1);
            if self.page_turns_since_sync >= 8
                || previous
                    .as_ref()
                    .is_some_and(|value| value.anchor.spine_item != page.anchor.spine_item)
            {
                self.pending = Some(PendingBookRequest::SyncProgress {
                    anchor: page.anchor,
                });
            }
        }
        self.view = BooksView::Reader;
        self.feedback = None;
        true
    }

    fn next_page(&mut self, layout: ReaderLayout) {
        if self.pending.is_some() {
            return;
        }
        let Some(current) = self.current_page.clone() else {
            return;
        };
        let Some(next) = self.normalize_next_anchor(current.next_anchor) else {
            self.feedback = Some("END OF BOOK");
            return;
        };
        if self.segment_for_anchor(next).is_some() {
            self.show_page(next, layout, true);
        } else {
            // Keep the displayed page and history unchanged until the one
            // useful boundary request succeeds. A failed fetch is retryable.
            self.request_segment(next, true);
        }
    }

    fn previous_page(&mut self) {
        if let Some(page) = self.page_history.pop() {
            self.current_page = Some(page);
            self.progress_dirty = true;
        }
    }

    fn segment_for_anchor(&self, anchor: BookReadingAnchor) -> Option<&BookContentSegment> {
        [
            self.current_segment.as_ref(),
            self.adjacent_segment.as_ref(),
        ]
        .into_iter()
        .flatten()
        .find(|segment| {
            segment.spine_item == anchor.spine_item
                && segment
                    .blocks
                    .iter()
                    .any(|block| block.index == anchor.block)
        })
    }

    fn normalize_next_anchor(&self, anchor: BookReadingAnchor) -> Option<BookReadingAnchor> {
        let manifest = self.manifest.as_ref()?;
        let spine = manifest.spine.get(usize::from(anchor.spine_item))?;
        if anchor.block < spine.block_count {
            return Some(anchor);
        }
        let next_spine = anchor.spine_item.checked_add(1)?;
        manifest.spine.get(usize::from(next_spine))?;
        Some(BookReadingAnchor {
            spine_item: next_spine,
            block: 0,
            character_offset: 0,
        })
    }

    fn request_segment(&mut self, anchor: BookReadingAnchor, record_history: bool) {
        if self.pending.is_some() {
            return;
        }
        self.pending = Some(PendingBookRequest::Segment {
            spine_item: anchor.spine_item,
            block: anchor.block,
            anchor,
            record_history,
        });
        self.connection = BooksConnection::Connecting;
    }

    fn remember_page(&mut self, page: RemoteReaderPage) {
        if self.page_history.len() == REMOTE_PAGE_CACHE_LIMIT {
            self.page_history.remove(0);
        }
        self.page_history.push(page);
    }

    fn record_error(&mut self, error: &AtlasClientError) {
        self.connection = if matches!(error, AtlasClientError::Forbidden(_)) {
            BooksConnection::RePairRequired
        } else if matches!(error, AtlasClientError::Offline | AtlasClientError::Timeout) {
            BooksConnection::Offline
        } else {
            BooksConnection::Error
        };
        self.feedback = Some(match error {
            AtlasClientError::Unauthorized(_) | AtlasClientError::Forbidden(_) => {
                "Authorization required"
            }
            AtlasClientError::Offline | AtlasClientError::Timeout => "Offline",
            AtlasClientError::MalformedPayload | AtlasClientError::ResponseTooLarge => {
                "Unable to read response"
            }
            _ => "Unable to load",
        });
    }
}

fn page_from_segment(
    segment: &BookContentSegment,
    anchor: BookReadingAnchor,
    layout: ReaderLayout,
) -> Option<RemoteReaderPage> {
    if layout.lines_per_page == 0 {
        return None;
    }
    let mut lines = Vec::new();
    let mut next = anchor;
    loop {
        let block = segment
            .blocks
            .iter()
            .find(|block| block.index == next.block)?;
        let start = usize::from(next.character_offset);
        if start > block.text.len() || !block.text.is_char_boundary(start) {
            return None;
        }
        if start == block.text.len() {
            next.block = next.block.checked_add(1)?;
            next.character_offset = 0;
            if !segment
                .blocks
                .iter()
                .any(|candidate| candidate.index == next.block)
            {
                break;
            }
            continue;
        }
        let mut remaining = layout;
        remaining.lines_per_page = layout.lines_per_page.saturating_sub(lines.len());
        if remaining.lines_per_page == 0 {
            break;
        }
        let (page_lines, end) = paginate_reflowable_text(&block.text, remaining, start);
        if end <= start {
            return None;
        }
        lines.extend(page_lines.into_iter().map(|line| RemoteReaderLine {
            text: line.text,
            paragraph_end: line.paragraph_end,
            kind: block.kind.clone(),
        }));
        next = if end < block.text.len() {
            BookReadingAnchor {
                character_offset: u16::try_from(end).ok()?,
                ..next
            }
        } else {
            BookReadingAnchor {
                block: next.block.checked_add(1)?,
                character_offset: 0,
                ..next
            }
        };
        if lines.len() >= layout.lines_per_page || end < block.text.len() {
            break;
        }
        if !segment
            .blocks
            .iter()
            .any(|candidate| candidate.index == next.block)
        {
            break;
        }
    }
    (!lines.is_empty()).then_some(RemoteReaderPage {
        anchor,
        next_anchor: next,
        lines,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::ReaderPreferences;

    fn segment(blocks: Vec<(u16, BookBlockKind, &str)>) -> BookContentSegment {
        BookContentSegment {
            book_id: "book_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            spine_item: 0,
            cursor: None,
            next_cursor: None,
            blocks: blocks
                .into_iter()
                .map(|(index, kind, text)| crate::atlas_dto::BookContentBlock {
                    index,
                    kind,
                    text: text.into(),
                })
                .collect(),
        }
    }

    #[test]
    fn reader_pages_cross_blocks_and_preserve_heading_semantics() {
        let page = page_from_segment(
            &segment(vec![
                (0, BookBlockKind::Heading, "Capítol u"),
                (1, BookBlockKind::Paragraph, "país català ñ ç"),
                (
                    2,
                    BookBlockKind::Paragraph,
                    "un paràgraf més llarg per acabar la pàgina",
                ),
            ]),
            BookReadingAnchor {
                spine_item: 0,
                block: 0,
                character_offset: 0,
            },
            ReaderLayout {
                chars_per_line: 12,
                lines_per_page: 4,
                ..ReaderPreferences::default().layout()
            },
        )
        .expect("contiguous blocks produce a page");
        assert!(page
            .lines
            .iter()
            .any(|line| line.kind == BookBlockKind::Heading));
        assert!(page
            .lines
            .iter()
            .any(|line| line.text.contains("país català")));
        assert!(page.next_anchor.block >= 1);
    }

    #[test]
    fn reader_rejects_non_utf8_anchor_and_accepts_the_same_byte_anchor_as_server() {
        let text = "país català ñ ç → reanudar";
        let segment = segment(vec![(0, BookBlockKind::Paragraph, text)]);
        assert!(page_from_segment(
            &segment,
            BookReadingAnchor {
                spine_item: 0,
                block: 0,
                character_offset: 3
            },
            ReaderPreferences::default().layout(),
        )
        .is_none());
        let valid_start = u16::try_from("país català ñ ç".len()).unwrap();
        let resumed = page_from_segment(
            &segment,
            BookReadingAnchor {
                spine_item: 0,
                block: 0,
                character_offset: valid_start,
            },
            ReaderPreferences::default().layout(),
        )
        .expect("the UTF-8 byte anchor shared with Atlas resumes after ç");
        assert!(resumed
            .lines
            .iter()
            .any(|line| line.text.contains("reanudar")));
    }

    #[test]
    fn forbidden_books_scope_keeps_pairing_and_requests_repair() {
        let mut state = AtlasBooksState::default();
        state.record_error(&AtlasClientError::Forbidden(
            crate::atlas_dto::CanonicalApiError {
                code: "AUTH_FORBIDDEN".into(),
                message: "no".into(),
                request_id: "r".into(),
            },
        ));
        assert_eq!(state.connection, BooksConnection::RePairRequired);
        assert_eq!(state.feedback, Some("Authorization required"));
    }
}
