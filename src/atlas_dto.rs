//! Narrow, transport-independent Atlas REST response models.
//!
//! AtlasClient and HTTP transport belong to a later milestone. This module
//! accepts already-collected response bytes, rejects oversized bodies before
//! JSON deserialization, and retains only fields Atlas Lite can render.

use core::fmt;
use core::marker::PhantomData;

use serde::{
    de::{self, DeserializeOwned, IgnoredAny, SeqAccess, Visitor},
    Deserialize, Deserializer, Serialize,
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
pub const MAX_BOOK_SUMMARIES: usize = 32;
pub const MAX_BOOK_SPINE_ITEMS: usize = 128;
pub const MAX_BOOK_TOC_ITEMS: usize = 128;
pub const MAX_BOOK_SEGMENT_BLOCKS: usize = 24;
pub const MAX_BOOK_BOOKMARKS: usize = 128;
pub const MAX_BOOK_TEXT_BYTES: usize = 2_048;
pub const MAX_BOOK_LABEL_BYTES: usize = 256;
pub const MAX_BOOK_ID_BYTES: usize = 69;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtlasDtoError {
    BodyTooLarge { limit: usize, actual: usize },
    InvalidJson { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct NoteSummaryPage {
    #[serde(deserialize_with = "deserialize_note_summaries")]
    pub items: Vec<AtlasNoteSummary>,
    #[serde(rename = "nextCursor", deserialize_with = "required_nullable_string")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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

/// The bounded acknowledgement returned by `POST /api/v1/capture/text`.
///
/// This deliberately validates the NoteDocumentResponse fields that establish
/// an authoritative capture result, while discarding frontmatter rather than
/// retaining an arbitrary map on the device.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CaptureTextAcknowledgement {
    #[serde(deserialize_with = "required_nullable_string")]
    pub id: Option<String>,
    pub path: String,
    pub state: NoteState,
    pub title: String,
    #[serde(deserialize_with = "required_nullable_string")]
    pub created: Option<String>,
    #[serde(deserialize_with = "required_nullable_string")]
    pub updated: Option<String>,
    pub revision: String,
    #[serde(rename = "frontmatter", deserialize_with = "required_ignored")]
    _frontmatter: (),
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SearchResponse {
    pub query: String,
    pub total: u32,
    #[serde(deserialize_with = "deserialize_search_hits")]
    pub hits: Vec<SearchHit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SearchHit {
    #[serde(rename = "atlasId", deserialize_with = "required_nullable_string")]
    pub id: Option<String>,
    pub path: String,
    pub title: String,
    pub snippet: String,
    pub revision: String,
    pub state: Option<NoteState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ViewSummaryPage {
    #[serde(deserialize_with = "deserialize_view_summaries")]
    pub items: Vec<ViewSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ViewSummary {
    pub id: String,
    pub name: String,
    pub revision: String,
    pub status: ViewStatus,
    pub layout: ViewLayout,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ViewResultPage {
    pub view: ViewSummary,
    #[serde(deserialize_with = "deserialize_view_results")]
    pub items: Vec<ViewResult>,
    #[serde(rename = "nextCursor", deserialize_with = "required_nullable_string")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ViewResult {
    #[serde(deserialize_with = "required_nullable_string")]
    pub id: Option<String>,
    pub path: String,
    pub title: String,
    pub state: NoteState,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct BookSummaryPage {
    #[serde(deserialize_with = "deserialize_book_summaries")]
    pub items: Vec<AtlasBookSummary>,
    #[serde(
        rename = "nextCursor",
        deserialize_with = "required_nullable_bounded_cursor"
    )]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AtlasBookSummary {
    #[serde(deserialize_with = "bounded_book_id")]
    pub id: String,
    #[serde(deserialize_with = "bounded_book_label")]
    pub title: String,
    #[serde(deserialize_with = "deserialize_book_authors")]
    pub authors: Vec<String>,
    #[serde(deserialize_with = "required_nullable_bounded_label")]
    pub language: Option<String>,
    #[serde(rename = "byteSize")]
    pub byte_size: u32,
    #[serde(rename = "importStatus")]
    pub import_status: BookImportStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BookImportStatus {
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct BookManifest {
    pub book: AtlasBookSummary,
    #[serde(deserialize_with = "deserialize_book_spine")]
    pub spine: Vec<BookSpineItem>,
    #[serde(deserialize_with = "deserialize_book_toc")]
    pub toc: Vec<BookTocEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct BookSpineItem {
    pub index: u16,
    #[serde(deserialize_with = "bounded_book_label")]
    pub label: String,
    #[serde(rename = "blockCount")]
    pub block_count: u16,
    #[serde(rename = "textBytes")]
    pub text_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct BookTocEntry {
    #[serde(deserialize_with = "bounded_book_label")]
    pub label: String,
    #[serde(rename = "spineItem")]
    pub spine_item: u16,
    pub block: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct BookContentSegment {
    #[serde(rename = "bookId", deserialize_with = "bounded_book_id")]
    pub book_id: String,
    #[serde(rename = "spineItem")]
    pub spine_item: u16,
    #[serde(deserialize_with = "required_nullable_bounded_cursor")]
    pub cursor: Option<String>,
    #[serde(
        rename = "nextCursor",
        deserialize_with = "required_nullable_bounded_cursor"
    )]
    pub next_cursor: Option<String>,
    #[serde(deserialize_with = "deserialize_book_blocks")]
    pub blocks: Vec<BookContentBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct BookContentBlock {
    pub index: u16,
    pub kind: BookBlockKind,
    #[serde(deserialize_with = "bounded_book_text")]
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BookBlockKind {
    Heading,
    Paragraph,
    Break,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct BookReadingAnchor {
    #[serde(rename = "spineItem")]
    pub spine_item: u16,
    pub block: u16,
    #[serde(rename = "characterOffset")]
    pub character_offset: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct BookReadingState {
    #[serde(rename = "bookId", deserialize_with = "bounded_book_id")]
    pub book_id: String,
    pub anchor: BookReadingAnchor,
    pub percentage: u8,
    pub revision: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct BookBookmarks {
    #[serde(deserialize_with = "deserialize_book_bookmarks")]
    pub items: Vec<BookBookmark>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct BookBookmark {
    #[serde(deserialize_with = "bounded_book_label")]
    pub id: String,
    #[serde(rename = "bookId", deserialize_with = "bounded_book_id")]
    pub book_id: String,
    pub anchor: BookReadingAnchor,
    #[serde(deserialize_with = "required_nullable_bounded_label")]
    pub label: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteState {
    Managed,
    Unmanaged,
    Invalid,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewStatus {
    Ok,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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

pub fn parse_capture_text_acknowledgement(
    body: &[u8],
) -> Result<CaptureTextAcknowledgement, AtlasDtoError> {
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

pub fn parse_book_summary_page(body: &[u8]) -> Result<BookSummaryPage, AtlasDtoError> {
    parse_bounded(body)
}
pub fn parse_book_manifest(body: &[u8]) -> Result<BookManifest, AtlasDtoError> {
    parse_bounded(body)
}
pub fn parse_book_content_segment(body: &[u8]) -> Result<BookContentSegment, AtlasDtoError> {
    parse_bounded(body)
}
pub fn parse_book_reading_state(body: &[u8]) -> Result<BookReadingState, AtlasDtoError> {
    parse_bounded(body)
}
pub fn parse_book_progress(body: &[u8]) -> Result<Option<BookReadingState>, AtlasDtoError> {
    parse_bounded(body)
}
pub fn parse_book_bookmarks(body: &[u8]) -> Result<BookBookmarks, AtlasDtoError> {
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

fn required_ignored<'de, D>(deserializer: D) -> Result<(), D::Error>
where
    D: Deserializer<'de>,
{
    IgnoredAny::deserialize(deserializer).map(|_| ())
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

fn deserialize_book_summaries<'de, D>(deserializer: D) -> Result<Vec<AtlasBookSummary>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec::<D, AtlasBookSummary, MAX_BOOK_SUMMARIES>(deserializer)
}
fn deserialize_book_spine<'de, D>(deserializer: D) -> Result<Vec<BookSpineItem>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec::<D, BookSpineItem, MAX_BOOK_SPINE_ITEMS>(deserializer)
}
fn deserialize_book_toc<'de, D>(deserializer: D) -> Result<Vec<BookTocEntry>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec::<D, BookTocEntry, MAX_BOOK_TOC_ITEMS>(deserializer)
}
fn deserialize_book_blocks<'de, D>(deserializer: D) -> Result<Vec<BookContentBlock>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec::<D, BookContentBlock, MAX_BOOK_SEGMENT_BLOCKS>(deserializer)
}
fn deserialize_book_bookmarks<'de, D>(deserializer: D) -> Result<Vec<BookBookmark>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec::<D, BookBookmark, MAX_BOOK_BOOKMARKS>(deserializer)
}
fn deserialize_book_authors<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec::<D, BoundedBookLabel, 16>(deserializer)
        .map(|items| items.into_iter().map(|item| item.0).collect())
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct BoundedBookLabel(#[serde(deserialize_with = "bounded_book_label")] String);

fn bounded_book_string<'de, D, const MAX: usize>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    if value.is_empty() || value.len() > MAX {
        return Err(de::Error::custom(format_args!(
            "expected a non-empty string up to {MAX} bytes"
        )));
    }
    Ok(value)
}
fn bounded_book_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    bounded_book_string::<D, MAX_BOOK_ID_BYTES>(deserializer)
}
fn bounded_book_label<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    bounded_book_string::<D, MAX_BOOK_LABEL_BYTES>(deserializer)
}
fn bounded_book_text<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    bounded_book_string::<D, MAX_BOOK_TEXT_BYTES>(deserializer)
}
fn required_nullable_bounded_label<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<BoundedBookLabel>::deserialize(deserializer).map(|value| value.map(|item| item.0))
}
fn required_nullable_bounded_cursor<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).and_then(|value| match value {
        Some(value) if value.is_empty() || value.len() > 128 => {
            Err(de::Error::custom("invalid bounded cursor"))
        }
        value => Ok(value),
    })
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
