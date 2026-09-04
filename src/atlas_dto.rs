//! Narrow, transport-independent Atlas REST response models.
//!
//! AtlasClient and HTTP transport belong to a later milestone. This module
//! accepts already-collected response bytes, rejects oversized bodies before
//! JSON deserialization, and retains only fields Atlas Lite can render.

use core::fmt;
use core::marker::PhantomData;

use serde::{
    de::{self, DeserializeOwned, IgnoredAny, SeqAccess, Visitor},
    Deserialize, Deserializer,
};

/// Maximum response payload accepted before normal JSON deserialization.
pub const MAX_RESPONSE_BODY_BYTES: usize = 64 * 1024;

/// Maximum number of note summaries retained from one response page.
pub const MAX_NOTE_SUMMARIES: usize = 64;

/// Maximum number of search hits retained from one response page.
pub const MAX_SEARCH_HITS: usize = 64;

/// Maximum number of View summaries retained from one response page.
pub const MAX_VIEW_SUMMARIES: usize = 32;

/// Maximum number of View results retained from one response page.
pub const MAX_VIEW_RESULTS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtlasDtoError {
    BodyTooLarge { limit: usize, actual: usize },
    InvalidJson { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NoteSummaryPage {
    #[serde(deserialize_with = "deserialize_note_summaries")]
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
    #[serde(rename = "parentId", deserialize_with = "required_nullable_string")]
    pub parent_id: Option<String>,
    #[serde(deserialize_with = "required_nullable_string")]
    pub order: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AtlasNoteDocument {
    #[serde(deserialize_with = "required_nullable_string")]
    pub id: Option<String>,
    pub title: String,
    pub revision: String,
    pub body: String,
    #[serde(rename = "parentId", deserialize_with = "required_nullable_string")]
    pub parent_id: Option<String>,
    #[serde(deserialize_with = "required_nullable_string")]
    pub order: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SearchResponse {
    pub query: String,
    pub total: u32,
    #[serde(deserialize_with = "deserialize_search_hits")]
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
    #[serde(deserialize_with = "deserialize_view_summaries")]
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
    #[serde(deserialize_with = "deserialize_view_results")]
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

fn deserialize_note_summaries<'de, D>(deserializer: D) -> Result<Vec<AtlasNoteSummary>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec::<D, AtlasNoteSummary, MAX_NOTE_SUMMARIES>(deserializer)
}

fn deserialize_search_hits<'de, D>(deserializer: D) -> Result<Vec<SearchHit>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec::<D, SearchHit, MAX_SEARCH_HITS>(deserializer)
}

fn deserialize_view_summaries<'de, D>(deserializer: D) -> Result<Vec<ViewSummary>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec::<D, ViewSummary, MAX_VIEW_SUMMARIES>(deserializer)
}

fn deserialize_view_results<'de, D>(deserializer: D) -> Result<Vec<ViewResult>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec::<D, ViewResult, MAX_VIEW_RESULTS>(deserializer)
}

fn deserialize_bounded_vec<'de, D, T, const MAX: usize>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    deserializer.deserialize_seq(BoundedVecVisitor::<T, MAX>(PhantomData))
}

struct BoundedVecVisitor<T, const MAX: usize>(PhantomData<T>);

impl<'de, T, const MAX: usize> Visitor<'de> for BoundedVecVisitor<T, MAX>
where
    T: Deserialize<'de>,
{
    type Value = Vec<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "a JSON array containing at most {MAX} items")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut items = Vec::with_capacity(MAX);
        while items.len() < MAX {
            match sequence.next_element()? {
                Some(item) => items.push(item),
                None => return Ok(items),
            }
        }

        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(de::Error::custom(format_args!(
                "expected at most {MAX} items"
            )));
        }

        Ok(items)
    }
}
