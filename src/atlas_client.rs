//! Transport-independent Atlas application client.
//!
//! The client owns request routing and response classification while transports
//! only exchange bounded byte responses. This keeps UI and host tests free of
//! ESP-IDF HTTP types.

use core::fmt;

use crate::atlas_dto::{
    parse_api_error, parse_note_document, parse_note_summary_page, parse_search_response,
    parse_view_result_page, parse_view_summaries, AtlasDtoError, AtlasNoteDocument,
    CanonicalApiError, NoteSummaryPage, SearchResponse, ViewResultPage, ViewSummaryPage,
    MAX_NOTE_SUMMARIES, MAX_RESPONSE_BODY_BYTES, MAX_SEARCH_HITS, MAX_VIEW_RESULTS,
};

/// A bounded capture request. Its content is intentionally redacted from Debug.
pub const MAX_CAPTURE_TEXT_BYTES: usize = 4 * 1024;
/// Cursor bytes retained before percent encoding into a bounded request URL.
pub const MAX_CURSOR_BYTES: usize = 128;
/// Atlas note and View route parameters are canonical UUID text.
pub const ATLAS_UUID_BYTES: usize = 36;
/// Search text is deliberately smaller than Atlas server's unbounded query contract.
pub const MAX_SEARCH_QUERY_BYTES: usize = 128;
/// Device-side offset cap prevents a request from asking the server to skip an unbounded index range.
pub const MAX_SEARCH_OFFSET: usize = 10_000;
/// Atlas's current v1 idempotency key is `v1.<10 digits>.<22 base64url bytes>`.
pub const MAX_IDEMPOTENCY_KEY_BYTES: usize = 36;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestValidationError {
    CursorTooLong,
    InvalidNoteId,
    InvalidViewId,
    QueryEmpty,
    QueryTooLong,
    InvalidLimit,
    OffsetTooLarge,
    InvalidIdempotencyKey,
    CaptureTextEmpty,
    CaptureTextTooLong,
}

/// A bounded capture request. Its content is intentionally redacted from Debug.
#[derive(Clone, Eq, PartialEq)]
pub struct CaptureTextRequest {
    text: String,
}

impl CaptureTextRequest {
    pub fn new(text: impl Into<String>) -> Result<Self, RequestValidationError> {
        let text = text.into();
        if text.is_empty() {
            return Err(RequestValidationError::CaptureTextEmpty);
        }
        if text.len() > MAX_CAPTURE_TEXT_BYTES {
            return Err(RequestValidationError::CaptureTextTooLong);
        }
        Ok(Self { text })
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl fmt::Debug for CaptureTextRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CaptureTextRequest { text: <redacted> }")
    }
}

/// Application requests accepted by an Atlas transport.
#[derive(Clone, Eq, PartialEq)]
pub enum TransportRequest {
    ListNotes {
        cursor: Option<String>,
        limit: usize,
    },
    GetNote {
        id: String,
    },
    Search {
        query: String,
        limit: usize,
        offset: usize,
    },
    ListViews,
    GetViewResults {
        id: String,
        cursor: Option<String>,
        limit: usize,
    },
    CaptureText {
        request: CaptureTextRequest,
        idempotency_key: String,
    },
}

impl fmt::Debug for TransportRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ListNotes { .. } => {
                formatter.write_str("TransportRequest::ListNotes { <redacted> }")
            }
            Self::GetNote { .. } => formatter.write_str("TransportRequest::GetNote { <redacted> }"),
            Self::Search { .. } => formatter.write_str("TransportRequest::Search { <redacted> }"),
            Self::ListViews => formatter.write_str("TransportRequest::ListViews"),
            Self::GetViewResults { .. } => {
                formatter.write_str("TransportRequest::GetViewResults { <redacted> }")
            }
            Self::CaptureText { .. } => {
                formatter.write_str("TransportRequest::CaptureText { <redacted> }")
            }
        }
    }
}

/// Raw, bounded body response supplied by a transport implementation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Failures that occur before a server response is available.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportError {
    Timeout,
    Offline,
    ResponseTooLarge,
}

impl fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Timeout => "timeout",
            Self::Offline => "offline",
            Self::ResponseTooLarge => "response too large",
        })
    }
}

/// Narrow boundary implemented by target HTTPS and host/simulator transports.
pub trait AtlasTransport {
    fn execute(&mut self, request: TransportRequest) -> Result<TransportResponse, TransportError>;
}

/// Typed outcomes exposed to application state and screens.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AtlasClientError {
    Unauthorized(CanonicalApiError),
    Forbidden(CanonicalApiError),
    NotFound(CanonicalApiError),
    RateLimited(CanonicalApiError),
    Unavailable(CanonicalApiError),
    Timeout,
    Offline,
    MalformedPayload,
    ResponseTooLarge,
    InvalidRequest(RequestValidationError),
    UnexpectedStatus {
        status: u16,
        error: Option<CanonicalApiError>,
    },
}

/// Transport-independent typed Atlas application API.
#[derive(Debug)]
pub struct AtlasClient<T> {
    transport: T,
}

impl<T> AtlasClient<T>
where
    T: AtlasTransport,
{
    #[must_use]
    pub const fn new(transport: T) -> Self {
        Self { transport }
    }

    #[must_use]
    pub const fn transport(&self) -> &T {
        &self.transport
    }

    /// Mutable transport access is reserved for deterministic host fixtures;
    /// production code still uses only the typed client methods.
    #[cfg(not(target_os = "espidf"))]
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    pub fn list_notes(
        &mut self,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<NoteSummaryPage, AtlasClientError> {
        let body = self.request(TransportRequest::ListNotes {
            cursor: cursor.map(str::to_owned),
            limit,
        })?;
        parse_note_summary_page(&body).map_err(classify_dto_error)
    }

    pub fn get_note(&mut self, id: &str) -> Result<AtlasNoteDocument, AtlasClientError> {
        let body = self.request(TransportRequest::GetNote { id: id.into() })?;
        parse_note_document(&body).map_err(classify_dto_error)
    }

    pub fn search(
        &mut self,
        query: &str,
        limit: usize,
        offset: usize,
    ) -> Result<SearchResponse, AtlasClientError> {
        let body = self.request(TransportRequest::Search {
            query: query.into(),
            limit,
            offset,
        })?;
        parse_search_response(&body).map_err(classify_dto_error)
    }

    pub fn list_views(&mut self) -> Result<ViewSummaryPage, AtlasClientError> {
        let body = self.request(TransportRequest::ListViews)?;
        parse_view_summaries(&body).map_err(classify_dto_error)
    }

    pub fn get_view_results(
        &mut self,
        id: &str,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<ViewResultPage, AtlasClientError> {
        let body = self.request(TransportRequest::GetViewResults {
            id: id.into(),
            cursor: cursor.map(str::to_owned),
            limit,
        })?;
        parse_view_result_page(&body).map_err(classify_dto_error)
    }

    pub fn capture_text(
        &mut self,
        request: &CaptureTextRequest,
        idempotency_key: &str,
    ) -> Result<(), AtlasClientError> {
        self.request(TransportRequest::CaptureText {
            request: request.clone(),
            idempotency_key: idempotency_key.into(),
        })?;
        Ok(())
    }

    fn request(&mut self, request: TransportRequest) -> Result<Vec<u8>, AtlasClientError> {
        validate_transport_request(&request).map_err(AtlasClientError::InvalidRequest)?;
        let response = self
            .transport
            .execute(request)
            .map_err(|error| match error {
                TransportError::Timeout => AtlasClientError::Timeout,
                TransportError::Offline => AtlasClientError::Offline,
                TransportError::ResponseTooLarge => AtlasClientError::ResponseTooLarge,
            })?;

        if response.body.len() > MAX_RESPONSE_BODY_BYTES {
            return Err(AtlasClientError::ResponseTooLarge);
        }

        if (200..300).contains(&response.status) {
            return Ok(response.body);
        }

        let error = parse_canonical_error(&response.body);
        Err(match response.status {
            401 => AtlasClientError::Unauthorized(error.ok_or(AtlasClientError::MalformedPayload)?),
            403 => AtlasClientError::Forbidden(error.ok_or(AtlasClientError::MalformedPayload)?),
            404 => AtlasClientError::NotFound(error.ok_or(AtlasClientError::MalformedPayload)?),
            429 => AtlasClientError::RateLimited(error.ok_or(AtlasClientError::MalformedPayload)?),
            503 => AtlasClientError::Unavailable(error.ok_or(AtlasClientError::MalformedPayload)?),
            status => AtlasClientError::UnexpectedStatus { status, error },
        })
    }
}

pub fn validate_transport_request(
    request: &TransportRequest,
) -> Result<(), RequestValidationError> {
    match request {
        TransportRequest::ListNotes { cursor, limit } => {
            validate_cursor(cursor.as_deref())?;
            validate_limit(*limit, MAX_NOTE_SUMMARIES)
        }
        TransportRequest::GetNote { id } => {
            validate_uuid(id, RequestValidationError::InvalidNoteId)
        }
        TransportRequest::Search {
            query,
            limit,
            offset,
        } => {
            if query.is_empty() {
                return Err(RequestValidationError::QueryEmpty);
            }
            if query.len() > MAX_SEARCH_QUERY_BYTES {
                return Err(RequestValidationError::QueryTooLong);
            }
            validate_limit(*limit, MAX_SEARCH_HITS)?;
            if *offset > MAX_SEARCH_OFFSET {
                return Err(RequestValidationError::OffsetTooLarge);
            }
            Ok(())
        }
        TransportRequest::ListViews => Ok(()),
        TransportRequest::GetViewResults { id, cursor, limit } => {
            validate_uuid(id, RequestValidationError::InvalidViewId)?;
            validate_cursor(cursor.as_deref())?;
            validate_limit(*limit, MAX_VIEW_RESULTS)
        }
        TransportRequest::CaptureText {
            request,
            idempotency_key,
        } => {
            validate_capture_text(request.text())?;
            validate_idempotency_key(idempotency_key)
        }
    }
}

fn validate_cursor(cursor: Option<&str>) -> Result<(), RequestValidationError> {
    if cursor.is_some_and(|value| value.is_empty() || value.len() > MAX_CURSOR_BYTES) {
        return Err(RequestValidationError::CursorTooLong);
    }
    Ok(())
}

fn validate_uuid(value: &str, error: RequestValidationError) -> Result<(), RequestValidationError> {
    let bytes = value.as_bytes();
    if bytes.len() != ATLAS_UUID_BYTES
        || !bytes.iter().enumerate().all(|(index, byte)| {
            matches!(index, 8 | 13 | 18 | 23) && *byte == b'-'
                || !matches!(index, 8 | 13 | 18 | 23) && byte.is_ascii_hexdigit()
        })
    {
        return Err(error);
    }
    Ok(())
}

fn validate_limit(limit: usize, maximum: usize) -> Result<(), RequestValidationError> {
    if limit == 0 || limit > maximum {
        return Err(RequestValidationError::InvalidLimit);
    }
    Ok(())
}

fn validate_capture_text(value: &str) -> Result<(), RequestValidationError> {
    if value.is_empty() {
        return Err(RequestValidationError::CaptureTextEmpty);
    }
    if value.len() > MAX_CAPTURE_TEXT_BYTES {
        return Err(RequestValidationError::CaptureTextTooLong);
    }
    Ok(())
}

fn validate_idempotency_key(value: &str) -> Result<(), RequestValidationError> {
    let bytes = value.as_bytes();
    if bytes.len() != MAX_IDEMPOTENCY_KEY_BYTES
        || &bytes[..3] != b"v1."
        || !bytes[3..13].iter().all(u8::is_ascii_digit)
        || bytes[13] != b'.'
        || !bytes[14..]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(RequestValidationError::InvalidIdempotencyKey);
    }
    Ok(())
}

fn parse_canonical_error(body: &[u8]) -> Option<CanonicalApiError> {
    parse_api_error(body).ok()
}

fn classify_dto_error(error: AtlasDtoError) -> AtlasClientError {
    match error {
        AtlasDtoError::BodyTooLarge { .. } => AtlasClientError::ResponseTooLarge,
        AtlasDtoError::InvalidJson { .. } => AtlasClientError::MalformedPayload,
    }
}

/// Deterministic outcomes available to host tests and the native simulator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MockTransportOutcome {
    Response(TransportResponse),
    Failure(TransportError),
}

impl MockTransportOutcome {
    #[must_use]
    pub fn response(status: u16, body: impl AsRef<[u8]>) -> Self {
        Self::Response(TransportResponse {
            status,
            body: body.as_ref().into(),
        })
    }

    #[must_use]
    pub fn unauthorized() -> Self {
        Self::canonical_error(401, "ATLAS_UNAUTHORIZED")
    }
    #[must_use]
    pub fn forbidden() -> Self {
        Self::canonical_error(403, "ATLAS_FORBIDDEN")
    }
    #[must_use]
    pub fn not_found() -> Self {
        Self::canonical_error(404, "NOTE_NOT_FOUND")
    }
    #[must_use]
    pub fn rate_limited() -> Self {
        Self::canonical_error(429, "RATE_LIMITED")
    }
    #[must_use]
    pub fn unavailable() -> Self {
        Self::canonical_error(503, "ATLAS_INDEX_NOT_READY")
    }
    #[must_use]
    pub const fn timeout() -> Self {
        Self::Failure(TransportError::Timeout)
    }
    #[must_use]
    pub const fn offline() -> Self {
        Self::Failure(TransportError::Offline)
    }
    #[must_use]
    pub fn malformed() -> Self {
        Self::response(200, br#"{"items":[}"#)
    }
    #[must_use]
    pub fn oversized() -> Self {
        Self::response(200, vec![b' '; MAX_RESPONSE_BODY_BYTES + 1])
    }

    fn canonical_error(status: u16, code: &str) -> Self {
        Self::response(
            status,
            format!(
                r#"{{"error":{{"code":"{code}","message":"mock failure","requestId":"mock-request"}}}}"#
            ),
        )
    }
}

/// FIFO scripted transport that records only typed, secret-free request data.
#[derive(Debug, Default)]
pub struct MockAtlasTransport {
    outcomes: Vec<MockTransportOutcome>,
    requests: Vec<TransportRequest>,
}

impl MockAtlasTransport {
    pub fn push_outcome(&mut self, outcome: MockTransportOutcome) {
        self.outcomes.push(outcome);
    }

    #[must_use]
    pub fn requests(&self) -> &[TransportRequest] {
        &self.requests
    }
}

impl AtlasTransport for MockAtlasTransport {
    fn execute(&mut self, request: TransportRequest) -> Result<TransportResponse, TransportError> {
        self.requests.push(request);
        match self.outcomes.is_empty() {
            false => match self.outcomes.remove(0) {
                MockTransportOutcome::Response(response) => Ok(response),
                MockTransportOutcome::Failure(error) => Err(error),
            },
            true => Err(TransportError::Offline),
        }
    }
}
