//! Bounded remote Books state. EPUB parsing remains server-side; the device
//! retains only safe reflowable blocks and applies its existing Reader layout.

use crate::{
    atlas_client::{AtlasClient, AtlasClientError, AtlasTransport},
    atlas_dto::{
        AtlasBookSummary, BookBlockKind, BookBookmarks, BookContentSegment, BookManifest,
        BookReadingAnchor,
    },
    reader::{paginate_reflowable_text, ReaderLayout},
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

/// Device-only Book state. No field is written to microSD; server anchors are
/// durable only after an explicit, bounded synchronization request succeeds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasBooksState {
    pub view: BooksView,
    pub connection: BooksConnection,
    pub books: Vec<AtlasBookSummary>,
    /// The server returned another cursor; the local 32-item snapshot is not
    /// an authoritative total.
    pub list_has_more: bool,
    pub selected: usize,
    pub manifest: Option<BookManifest>,
    pub bookmarks: BookBookmarks,
    pub current_segment: Option<BookContentSegment>,
    pub adjacent_segment: Option<BookContentSegment>,
    pub current_page: Option<RemoteReaderPage>,
    pub resume_anchor: Option<BookReadingAnchor>,
    /// Progress is deliberately retained only for the one book whose existing
    /// progress endpoint was already requested while opening its manifest.
    pub resume_percentage: Option<u8>,
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
            list_has_more: false,
            selected: 0,
            manifest: None,
            bookmarks: BookBookmarks { items: Vec::new() },
            current_segment: None,
            adjacent_segment: None,
            current_page: None,
            resume_anchor: None,
            resume_percentage: None,
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
                    self.list_has_more = page.next_cursor.is_some();
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
                    let progress = client.get_book_progress(&id).ok().flatten();
                    self.resume_anchor = progress.as_ref().map(|value| value.anchor);
                    self.resume_percentage = progress.map(|value| value.percentage);
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
                        self.cache_segment(segment);
                        self.show_page(anchor, layout, record_history);
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
        self.request_segment(anchor, false);
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
        self.feedback = Some(if self.connection == BooksConnection::RePairRequired {
            "Re-pair device to enable Books"
        } else {
            "BOOKS REQUEST FAILED"
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
        assert_eq!(state.feedback, Some("Re-pair device to enable Books"));
    }
}
