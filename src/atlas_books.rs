//! Bounded remote Books state. EPUB parsing remains server-side; the device
//! retains only safe reflowable blocks and applies its existing Reader layout.

use crate::{
    atlas_client::{AtlasClient, AtlasClientError, AtlasTransport},
    atlas_dto::{
        AtlasBookSummary, BookBookmarks, BookContentBlock, BookContentSegment, BookManifest,
        BookReadingAnchor,
    },
    reader::{paginate_reflowable_text, ReaderLayout, ReaderPageLine},
};

pub const BOOK_LIST_LIMIT: usize = 32;
pub const REMOTE_PAGE_CACHE_LIMIT: usize = 8;
pub const REMOTE_SEGMENT_CACHE_LIMIT: usize = 2;

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
    pub next_character_offset: u16,
    pub lines: Vec<ReaderPageLine>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PendingBookRequest {
    List,
    Open {
        id: String,
    },
    Segment {
        spine_item: u16,
        cursor: Option<String>,
        anchor: BookReadingAnchor,
    },
    SyncProgress {
        anchor: BookReadingAnchor,
    },
    Bookmark {
        anchor: BookReadingAnchor,
    },
}

/// Device-only Book state. No field is written to microSD; server anchors are
/// durable only after an explicit, bounded synchronization request succeeds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasBooksState {
    pub view: BooksView,
    pub connection: BooksConnection,
    pub books: Vec<AtlasBookSummary>,
    pub selected: usize,
    pub manifest: Option<BookManifest>,
    pub bookmarks: BookBookmarks,
    pub current_segment: Option<BookContentSegment>,
    pub adjacent_segment: Option<BookContentSegment>,
    pub current_page: Option<RemoteReaderPage>,
    pub resume_anchor: Option<BookReadingAnchor>,
    page_history: Vec<RemoteReaderPage>,
    current_book_id: Option<String>,
    pending: Option<PendingBookRequest>,
    pub feedback: Option<&'static str>,
    page_turns_since_sync: u8,
    progress_dirty: bool,
}

impl Default for AtlasBooksState {
    fn default() -> Self {
        Self {
            view: BooksView::List,
            connection: BooksConnection::Unconfigured,
            books: Vec::new(),
            selected: 0,
            manifest: None,
            bookmarks: BookBookmarks { items: Vec::new() },
            current_segment: None,
            adjacent_segment: None,
            current_page: None,
            resume_anchor: None,
            page_history: Vec::new(),
            current_book_id: None,
            pending: None,
            feedback: None,
            page_turns_since_sync: 0,
            progress_dirty: false,
        }
    }
}

impl AtlasBooksState {
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
                        0 => self.start_reader(self.resume_anchor.unwrap_or(BookReadingAnchor {
                            spine_item: 0,
                            block: 0,
                            character_offset: 0,
                        })),
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
                    self.start_reader(BookReadingAnchor {
                        spine_item: entry.spine_item,
                        block: entry.block,
                        character_offset: 0,
                    });
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
                    self.start_reader(self.bookmarks.items[self.selected].anchor);
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
        let Some(request) = self.pending.take() else {
            return false;
        };
        match request {
            PendingBookRequest::List => match client.list_books(None, BOOK_LIST_LIMIT) {
                Ok(page) => {
                    self.books = page.items;
                    self.selected = 0;
                    self.connection = BooksConnection::Connected;
                    self.feedback = None;
                }
                Err(error) => self.record_error(&error),
            },
            PendingBookRequest::Open { id } => match client.get_book_manifest(&id) {
                Ok(manifest) => {
                    self.current_book_id = Some(id.clone());
                    self.manifest = Some(manifest);
                    self.resume_anchor = client
                        .get_book_progress(&id)
                        .ok()
                        .flatten()
                        .map(|value| value.anchor);
                    self.bookmarks = client
                        .list_book_bookmarks(&id)
                        .unwrap_or(BookBookmarks { items: Vec::new() });
                    self.view = BooksView::Detail;
                    self.selected = 0;
                    self.connection = BooksConnection::Connected;
                    self.feedback = None;
                }
                Err(error) => self.record_error(&error),
            },
            PendingBookRequest::Segment {
                spine_item,
                cursor,
                anchor,
            } => {
                let Some(id) = self.current_book_id.clone() else {
                    self.feedback = Some("BOOK NOT SELECTED");
                    return true;
                };
                match client.get_book_content(&id, spine_item, cursor.as_deref()) {
                    Ok(segment) => {
                        self.adjacent_segment = self.current_segment.replace(segment);
                        if self.adjacent_segment.is_some() && REMOTE_SEGMENT_CACHE_LIMIT < 2 {
                            self.adjacent_segment = None;
                        }
                        self.page_history.clear();
                        self.show_page(anchor, layout);
                        self.connection = BooksConnection::Connected;
                    }
                    Err(error) => self.record_error(&error),
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

    fn start_reader(&mut self, anchor: BookReadingAnchor) {
        self.view = BooksView::Reader;
        self.current_page = None;
        self.page_history.clear();
        self.page_turns_since_sync = 0;
        self.pending = Some(PendingBookRequest::Segment {
            spine_item: anchor.spine_item,
            cursor: None,
            anchor,
        });
        self.connection = BooksConnection::Connecting;
    }

    fn show_page(&mut self, anchor: BookReadingAnchor, layout: ReaderLayout) {
        let Some(segment) = self.current_segment.as_ref() else {
            return;
        };
        let Some(block) = segment
            .blocks
            .iter()
            .find(|block| block.index == anchor.block)
        else {
            self.feedback = Some("BOOK POSITION UNAVAILABLE");
            return;
        };
        let page = page_from_block(block, anchor, layout);
        self.current_page = Some(page);
        self.view = BooksView::Reader;
    }

    fn next_page(&mut self, layout: ReaderLayout) {
        let Some(current) = self.current_page.clone() else {
            return;
        };
        if self.page_history.len() == REMOTE_PAGE_CACHE_LIMIT {
            self.page_history.remove(0);
        }
        self.page_history.push(current.clone());
        self.progress_dirty = true;
        self.page_turns_since_sync = self.page_turns_since_sync.saturating_add(1);
        if self.page_turns_since_sync >= 8 {
            self.pending = Some(PendingBookRequest::SyncProgress {
                anchor: current.anchor,
            });
        }
        let Some(segment) = self.current_segment.as_ref() else {
            return;
        };
        if let Some(block) = segment
            .blocks
            .iter()
            .find(|block| block.index == current.anchor.block)
        {
            if usize::from(current.next_character_offset) < block.text.len() {
                self.show_page(
                    BookReadingAnchor {
                        character_offset: current.next_character_offset,
                        ..current.anchor
                    },
                    layout,
                );
                return;
            }
        }
        if let Some(next) = segment
            .blocks
            .iter()
            .find(|block| block.index > current.anchor.block)
        {
            self.show_page(
                BookReadingAnchor {
                    spine_item: segment.spine_item,
                    block: next.index,
                    character_offset: 0,
                },
                layout,
            );
        } else if let Some(cursor) = segment.next_cursor.clone() {
            self.pending = Some(PendingBookRequest::Segment {
                spine_item: segment.spine_item,
                cursor: Some(cursor),
                anchor: BookReadingAnchor {
                    spine_item: segment.spine_item,
                    block: current.anchor.block.saturating_add(1),
                    character_offset: 0,
                },
            });
        } else {
            self.feedback = Some("END OF CHAPTER");
        }
    }

    fn previous_page(&mut self) {
        if let Some(page) = self.page_history.pop() {
            self.current_page = Some(page);
        }
    }

    fn record_error(&mut self, error: &AtlasClientError) {
        self.connection = if matches!(error, AtlasClientError::Forbidden(_)) {
            BooksConnection::RePairRequired
        } else if matches!(error, AtlasClientError::Offline | AtlasClientError::Timeout) {
            BooksConnection::Offline
        } else {
            BooksConnection::Error
        };
        self.feedback = Some(if self.connection == BooksConnection::RePairRequired {
            "Re-pair device to enable Books"
        } else {
            "BOOKS REQUEST FAILED"
        });
    }
}

fn page_from_block(
    block: &BookContentBlock,
    anchor: BookReadingAnchor,
    layout: ReaderLayout,
) -> RemoteReaderPage {
    let start = usize::from(anchor.character_offset).min(block.text.len());
    let (lines, end) = paginate_reflowable_text(&block.text, layout, start);
    RemoteReaderPage {
        anchor,
        next_character_offset: u16::try_from(end).unwrap_or(u16::MAX),
        lines,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::ReaderPreferences;

    #[test]
    fn reader_pages_are_local_and_preserve_latin_accents() {
        let block = BookContentBlock {
            index: 0,
            kind: crate::atlas_dto::BookBlockKind::Paragraph,
            text: "El país català: à é í ó ú ü ñ ç ¿ ¡".into(),
        };
        let page = page_from_block(
            &block,
            BookReadingAnchor {
                spine_item: 0,
                block: 0,
                character_offset: 0,
            },
            ReaderPreferences::default().layout(),
        );
        assert!(page
            .lines
            .iter()
            .any(|line| line.text.contains("país català")));
        assert!(page.lines.iter().any(|line| line.text.contains("ñ ç ¿ ¡")));
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
        assert_eq!(state.feedback, Some("Re-pair device to enable Books"));
    }
}
