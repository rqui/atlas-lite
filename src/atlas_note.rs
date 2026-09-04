//! Bounded Atlas Note reader state.
//!
//! This is deliberately an in-memory seam only. M5 may replace the recent
//! store with durable cache plumbing, but the reader never needs paths,
//! frontmatter, or generic JSON to stay useful offline.

use crate::{
    app::router::AtlasNoteOrigin,
    atlas_client::{
        validate_transport_request, AtlasClient, AtlasClientError, AtlasTransport, TransportRequest,
    },
    atlas_dto::AtlasNoteDocument,
};

/// Maximum reader title size retained after the bounded DTO parser.
pub const MAX_ATLAS_NOTE_TITLE_BYTES: usize = 256;
/// Maximum revision size retained by the reader/cache seam.
pub const MAX_ATLAS_NOTE_REVISION_BYTES: usize = 128;
/// Maximum Markdown body retained by the reader/cache seam.
pub const MAX_ATLAS_NOTE_BODY_BYTES: usize = 16 * 1024;
/// Maximum number of recently opened documents kept only in memory for M3.
pub const MAX_RECENT_ATLAS_NOTES: usize = 3;

/// Reader-safe subset of an Atlas document. Paths and frontmatter are never retained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasReaderDocument {
    id: String,
    title: String,
    revision: String,
    body: String,
}

impl AtlasReaderDocument {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn revision(&self) -> &str {
        &self.revision
    }

    #[must_use]
    pub fn body(&self) -> &str {
        &self.body
    }
}

/// Explicit failure classes exposed to the e-paper reader.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtlasNoteError {
    InvalidId,
    NotFound,
    Unauthorized,
    Forbidden,
    Unavailable,
    RateLimited,
    Timeout,
    Offline,
    MalformedPayload,
    Oversized,
    ServerError,
}

/// Explicit reader presentation state. A document, when present, remains
/// available during a same-ID refresh failure rather than being blanked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtlasNoteStatus {
    Idle,
    Loading,
    Loaded,
    OfflineCached,
    Error(AtlasNoteError),
}

impl AtlasNoteStatus {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "READY",
            Self::Loading => "LOADING",
            Self::Loaded => "ONLINE",
            Self::OfflineCached => "OFFLINE CACHED",
            Self::Error(AtlasNoteError::InvalidId) => "INVALID ID",
            Self::Error(AtlasNoteError::NotFound) => "NOT FOUND",
            Self::Error(AtlasNoteError::Unauthorized) => "UNAUTHORIZED",
            Self::Error(AtlasNoteError::Forbidden) => "FORBIDDEN",
            Self::Error(AtlasNoteError::Unavailable) => "UNAVAILABLE",
            Self::Error(AtlasNoteError::RateLimited) => "RATE LIMITED",
            Self::Error(AtlasNoteError::Timeout) => "TIMEOUT",
            Self::Error(AtlasNoteError::Offline) => "OFFLINE",
            Self::Error(AtlasNoteError::MalformedPayload) => "BAD DATA",
            Self::Error(AtlasNoteError::Oversized) => "TOO LARGE",
            Self::Error(AtlasNoteError::ServerError) => "SERVER ERROR",
        }
    }
}

/// Current selection plus the deliberately bounded recent-note seam.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasNoteState {
    selected_id: Option<String>,
    origin: Option<AtlasNoteOrigin>,
    status: AtlasNoteStatus,
    document: Option<AtlasReaderDocument>,
    recent: Vec<AtlasReaderDocument>,
}

impl Default for AtlasNoteState {
    fn default() -> Self {
        Self {
            selected_id: None,
            origin: None,
            status: AtlasNoteStatus::Idle,
            document: None,
            recent: Vec::with_capacity(MAX_RECENT_ATLAS_NOTES),
        }
    }
}

impl AtlasNoteState {
    #[must_use]
    pub fn selected_id(&self) -> Option<&str> {
        self.selected_id.as_deref()
    }

    #[must_use]
    pub const fn origin(&self) -> Option<AtlasNoteOrigin> {
        self.origin
    }

    #[must_use]
    pub const fn status(&self) -> AtlasNoteStatus {
        self.status
    }

    #[must_use]
    pub fn document(&self) -> Option<&AtlasReaderDocument> {
        self.document.as_ref()
    }

    #[must_use]
    pub fn recent(&self) -> &[AtlasReaderDocument] {
        &self.recent
    }

    /// Selects an Atlas ID and retains only a matching recent document while
    /// fetching. Returning false guarantees no transport request is possible.
    pub fn begin(&mut self, id: &str, origin: AtlasNoteOrigin) -> bool {
        if validate_note_id(id).is_err() {
            self.selected_id = None;
            self.origin = None;
            self.document = None;
            self.status = AtlasNoteStatus::Error(AtlasNoteError::InvalidId);
            return false;
        }

        self.selected_id = Some(id.into());
        self.origin = Some(origin);
        self.document = self
            .recent
            .iter()
            .find(|document| document.id == id)
            .cloned();
        self.status = AtlasNoteStatus::Loading;
        true
    }

    /// Performs exactly one typed request for the currently selected ID.
    pub fn load<T>(&mut self, client: &mut AtlasClient<T>)
    where
        T: AtlasTransport,
    {
        let Some(id) = self.selected_id.clone() else {
            return;
        };

        match client.get_note(&id) {
            Ok(document) => match reader_document(document, &id) {
                Ok(document) => {
                    self.remember(document.clone());
                    self.document = Some(document);
                    self.status = AtlasNoteStatus::Loaded;
                }
                Err(error) => self.fail(error),
            },
            Err(error) => self.fail(classify_client_error(&error)),
        }
    }

    fn fail(&mut self, error: AtlasNoteError) {
        self.status = if error == AtlasNoteError::Offline && self.document.is_some() {
            AtlasNoteStatus::OfflineCached
        } else {
            AtlasNoteStatus::Error(error)
        };
    }

    fn remember(&mut self, document: AtlasReaderDocument) {
        self.recent.retain(|recent| recent.id != document.id);
        self.recent.insert(0, document);
        self.recent.truncate(MAX_RECENT_ATLAS_NOTES);
    }
}

fn validate_note_id(id: &str) -> Result<(), ()> {
    validate_transport_request(&TransportRequest::GetNote { id: id.into() }).map_err(|_| ())
}

fn reader_document(
    document: AtlasNoteDocument,
    selected_id: &str,
) -> Result<AtlasReaderDocument, AtlasNoteError> {
    let Some(id) = document.id else {
        return Err(AtlasNoteError::MalformedPayload);
    };
    if id != selected_id
        || validate_note_id(&id).is_err()
        || document.title.is_empty()
        || document.revision.is_empty()
    {
        return Err(AtlasNoteError::MalformedPayload);
    }
    if document.title.len() > MAX_ATLAS_NOTE_TITLE_BYTES
        || document.revision.len() > MAX_ATLAS_NOTE_REVISION_BYTES
        || document.body.len() > MAX_ATLAS_NOTE_BODY_BYTES
    {
        return Err(AtlasNoteError::Oversized);
    }
    Ok(AtlasReaderDocument {
        id,
        title: document.title,
        revision: document.revision,
        body: document.body,
    })
}

fn classify_client_error(error: &AtlasClientError) -> AtlasNoteError {
    match error {
        AtlasClientError::Unauthorized(_) => AtlasNoteError::Unauthorized,
        AtlasClientError::Forbidden(_) => AtlasNoteError::Forbidden,
        AtlasClientError::NotFound(_) => AtlasNoteError::NotFound,
        AtlasClientError::RateLimited(_) => AtlasNoteError::RateLimited,
        AtlasClientError::Unavailable(_) => AtlasNoteError::Unavailable,
        AtlasClientError::Timeout => AtlasNoteError::Timeout,
        AtlasClientError::Offline => AtlasNoteError::Offline,
        AtlasClientError::MalformedPayload => AtlasNoteError::MalformedPayload,
        AtlasClientError::ResponseTooLarge => AtlasNoteError::Oversized,
        AtlasClientError::InvalidRequest(_) => AtlasNoteError::InvalidId,
        AtlasClientError::UnexpectedStatus { .. } => AtlasNoteError::ServerError,
    }
}
