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
#[derive(Clone, Eq, PartialEq)]
pub struct CaptureTextRequest {
    text: String,
}

impl CaptureTextRequest {
    pub fn new(text: impl Into<String>) -> Result<Self, AtlasClientError> {
        let text = text.into();
        if text.is_empty() {
            return Err(AtlasClientError::InvalidRequest(
                "capture text must not be empty",
            ));
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
#[derive(Clone, Debug, Eq, PartialEq)]
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
    InvalidRequest(&'static str),
    UnexpectedStatus {
        status: u16,
        error: Option<CanonicalApiError>,
    },
}

/// Transport-independent typed Atlas application API.
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

    pub fn list_notes(
        &mut self,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<NoteSummaryPage, AtlasClientError> {
        validate_limit(limit, MAX_NOTE_SUMMARIES)?;
        let body = self.request(TransportRequest::ListNotes {
            cursor: cursor.map(str::to_owned),
            limit,
        })?;
        parse_note_summary_page(&body).map_err(classify_dto_error)
    }

    pub fn get_note(&mut self, id: &str) -> Result<AtlasNoteDocument, AtlasClientError> {
        validate_non_empty(id, "note id must not be empty")?;
        let body = self.request(TransportRequest::GetNote { id: id.into() })?;
        parse_note_document(&body).map_err(classify_dto_error)
    }

    pub fn search(
        &mut self,
        query: &str,
        limit: usize,
        offset: usize,
    ) -> Result<SearchResponse, AtlasClientError> {
        validate_non_empty(query, "search query must not be empty")?;
        validate_limit(limit, MAX_SEARCH_HITS)?;
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
        validate_non_empty(id, "view id must not be empty")?;
        validate_limit(limit, MAX_VIEW_RESULTS)?;
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
        validate_non_empty(idempotency_key, "idempotency key must not be empty")?;
        self.request(TransportRequest::CaptureText {
            request: request.clone(),
            idempotency_key: idempotency_key.into(),
        })?;
        Ok(())
    }

    fn request(&mut self, request: TransportRequest) -> Result<Vec<u8>, AtlasClientError> {
        let response = self
            .transport
            .execute(request)
            .map_err(|error| match error {
                TransportError::Timeout => AtlasClientError::Timeout,
                TransportError::Offline => AtlasClientError::Offline,
            })?;

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

fn validate_non_empty(value: &str, reason: &'static str) -> Result<(), AtlasClientError> {
    if value.is_empty() {
        return Err(AtlasClientError::InvalidRequest(reason));
    }
    Ok(())
}

fn validate_limit(limit: usize, maximum: usize) -> Result<(), AtlasClientError> {
    if limit == 0 || limit > maximum {
        return Err(AtlasClientError::InvalidRequest(
            "limit must be within the operation response bound",
        ));
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
#[derive(Default)]
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
