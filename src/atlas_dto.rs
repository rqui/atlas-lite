//! Narrow, transport-independent Atlas REST response models.
//!
//! AtlasClient and HTTP transport belong to a later milestone. This module
//! accepts already-collected response bytes, rejects oversized bodies before
//! JSON deserialization, and retains only fields Atlas Lite can render.

use serde::{de::DeserializeOwned, Deserialize, Deserializer};

/// Maximum response payload accepted before normal JSON deserialization.
pub const MAX_RESPONSE_BODY_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtlasDtoError {
    BodyTooLarge { limit: usize, actual: usize },
    InvalidJson { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NoteSummaryPage {
    pub items: Vec<AtlasNoteSummary>,
    #[serde(rename = "nextCursor", deserialize_with = "required_nullable_string")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AtlasNoteSummary {
    #[serde(deserialize_with = "required_nullable_string")]
    pub id: Option<String>,
    pub path: String,
    pub title: String,
    pub state: NoteState,
    pub revision: String,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
    pub order: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AtlasNoteDocument {
    #[serde(deserialize_with = "required_nullable_string")]
    pub id: Option<String>,
    pub title: String,
    pub revision: String,
    pub body: String,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SearchResponse {
    pub query: String,
    pub total: u32,
    pub hits: Vec<SearchHit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SearchHit {
    #[serde(rename = "atlasId", deserialize_with = "required_nullable_string")]
    pub id: Option<String>,
    pub path: String,
    pub title: String,
    pub snippet: String,
    pub revision: String,
    pub state: Option<NoteState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ViewSummaryPage {
    pub items: Vec<ViewSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ViewSummary {
    pub id: String,
    pub name: String,
    pub revision: String,
    pub status: ViewStatus,
    pub layout: ViewLayout,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ViewResultPage {
    pub view: ViewSummary,
    pub items: Vec<ViewResult>,
    #[serde(rename = "nextCursor", deserialize_with = "required_nullable_string")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ViewResult {
    #[serde(deserialize_with = "required_nullable_string")]
    pub id: Option<String>,
    pub path: String,
    pub title: String,
    pub state: NoteState,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ApiError {
    pub error: CanonicalApiError,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CanonicalApiError {
    pub code: String,
    pub message: String,
    #[serde(rename = "requestId")]
    pub request_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteState {
    Managed,
    Unmanaged,
    Invalid,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewStatus {
    Ok,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewLayout {
    Table,
    List,
    Cards,
    Board,
    Calendar,
}

pub fn parse_note_summary_page(body: &[u8]) -> Result<NoteSummaryPage, AtlasDtoError> {
    parse_bounded(body)
}

pub fn parse_note_document(body: &[u8]) -> Result<AtlasNoteDocument, AtlasDtoError> {
    parse_bounded(body)
}

pub fn parse_search_response(body: &[u8]) -> Result<SearchResponse, AtlasDtoError> {
    parse_bounded(body)
}

pub fn parse_view_summaries(body: &[u8]) -> Result<ViewSummaryPage, AtlasDtoError> {
    parse_bounded(body)
}

pub fn parse_view_result_page(body: &[u8]) -> Result<ViewResultPage, AtlasDtoError> {
    parse_bounded(body)
}

pub fn parse_api_error(body: &[u8]) -> Result<CanonicalApiError, AtlasDtoError> {
    parse_bounded::<ApiError>(body).map(|body| body.error)
}

fn parse_bounded<T: DeserializeOwned>(body: &[u8]) -> Result<T, AtlasDtoError> {
    if body.len() > MAX_RESPONSE_BODY_BYTES {
        return Err(AtlasDtoError::BodyTooLarge {
            limit: MAX_RESPONSE_BODY_BYTES,
            actual: body.len(),
        });
    }

    serde_json::from_slice(body).map_err(|error| AtlasDtoError::InvalidJson {
        message: error.to_string(),
    })
}

fn required_nullable_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)
}
