//! Bounded Atlas HTTPS request preparation and ESP-IDF transport boundary.
//!
//! The portable portion composes redacted requests and retry policy for host
//! tests. ESP-IDF handles remain in the target-only adapter below.

use core::fmt;

use crate::{
    atlas_client::{CaptureTextRequest, TransportError, TransportRequest},
    atlas_config::AtlasConfig,
    atlas_dto::MAX_RESPONSE_BODY_BYTES,
};

/// One Atlas HTTPS attempt has an explicit ESP-IDF timeout.
pub const ATLAS_HTTP_TIMEOUT_SECONDS: u64 = 10;
/// Successful and error bodies share the DTO response bound.
pub const ATLAS_HTTP_RESPONSE_BODY_BYTES: usize = MAX_RESPONSE_BODY_BYTES;
/// The largest JSON capture payload accepted at the transport boundary.
pub const ATLAS_HTTP_REQUEST_BODY_BYTES: usize = 4 * 1024;
/// A read has its initial attempt plus two bounded retries.
pub const ATLAS_READ_ATTEMPT_LIMIT: usize = 3;
/// Retry delays are fixed so a read cannot keep the radio active indefinitely.
pub const ATLAS_READ_BACKOFF_MILLIS: [u64; ATLAS_READ_ATTEMPT_LIMIT - 1] = [250, 500];
/// TLS and bounded response reading execute away from the main orchestration task.
pub const ATLAS_HTTPS_WORKER_STACK_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HttpMethod {
    Get,
    Post,
}

/// Secret-free high-level status for diagnostics and simulator fakes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtlasTransportStatus {
    Connected,
    Unauthorized,
    Forbidden,
    Timeout,
    ServerError,
    Offline,
}

impl AtlasTransportStatus {
    #[must_use]
    pub const fn from_transport_error(error: TransportError) -> Self {
        match error {
            TransportError::Timeout => Self::Timeout,
            TransportError::Offline => Self::Offline,
        }
    }
}

/// Classify an HTTP status without retaining a response body.
#[must_use]
pub const fn classify_transport_status(status: u16) -> AtlasTransportStatus {
    match status {
        200..=299 => AtlasTransportStatus::Connected,
        401 => AtlasTransportStatus::Unauthorized,
        403 => AtlasTransportStatus::Forbidden,
        408 => AtlasTransportStatus::Timeout,
        500..=599 => AtlasTransportStatus::ServerError,
        _ => AtlasTransportStatus::ServerError,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AtlasHttpsError {
    InsecureUrl,
    InvalidToken,
    RequestTooLarge,
}

/// Prepared request whose Debug output deliberately omits all sensitive data.
pub struct PreparedRequest {
    #[cfg_attr(not(target_os = "espidf"), allow(dead_code))]
    method: HttpMethod,
    url: String,
    headers: Vec<(String, String)>,
    #[cfg_attr(not(target_os = "espidf"), allow(dead_code))]
    body: Vec<u8>,
}

impl fmt::Debug for PreparedRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PreparedRequest { method: <redacted>, url: <redacted>, headers: <redacted>, body: <redacted> }")
    }
}

impl PreparedRequest {
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// Build a canonical Atlas REST request with component-level percent encoding.
pub fn prepare_request(
    config: &AtlasConfig,
    request: &TransportRequest,
) -> Result<PreparedRequest, AtlasHttpsError> {
    if !config.atlas_url().starts_with("https://") {
        return Err(AtlasHttpsError::InsecureUrl);
    }
    if !config.api_token().starts_with("at_v1") {
        return Err(AtlasHttpsError::InvalidToken);
    }

    let (method, path, body, idempotency_key) = match request {
        TransportRequest::ListNotes { cursor, limit } => (
            HttpMethod::Get,
            query_path(
                "/api/v1/notes",
                &[
                    cursor.as_deref().map(|value| ("cursor", value)),
                    Some(("limit", &limit.to_string())),
                ],
            ),
            Vec::new(),
            None,
        ),
        TransportRequest::GetNote { id } => (
            HttpMethod::Get,
            format!("/api/v1/notes/by-id/{}", percent_encode(id)),
            Vec::new(),
            None,
        ),
        TransportRequest::Search {
            query,
            limit,
            offset,
        } => (
            HttpMethod::Get,
            query_path(
                "/api/v1/search",
                &[
                    Some(("q", query)),
                    Some(("limit", &limit.to_string())),
                    Some(("offset", &offset.to_string())),
                ],
            ),
            Vec::new(),
            None,
        ),
        TransportRequest::ListViews => (HttpMethod::Get, "/api/v1/views".into(), Vec::new(), None),
        TransportRequest::GetViewResults { id, cursor, limit } => (
            HttpMethod::Get,
            query_path(
                &format!("/api/v1/views/{}/results", percent_encode(id)),
                &[
                    cursor.as_deref().map(|value| ("cursor", value)),
                    Some(("limit", &limit.to_string())),
                ],
            ),
            Vec::new(),
            None,
        ),
        TransportRequest::CaptureText {
            request,
            idempotency_key,
        } => (
            HttpMethod::Post,
            "/api/v1/capture/text".into(),
            capture_body(request)?,
            Some(idempotency_key.as_str()),
        ),
    };
    if body.len() > ATLAS_HTTP_REQUEST_BODY_BYTES {
        return Err(AtlasHttpsError::RequestTooLarge);
    }
    let mut headers = vec![
        ("accept".into(), "application/json".into()),
        (
            "authorization".into(),
            format!("Bearer {}", config.api_token()),
        ),
    ];
    if method == HttpMethod::Post {
        headers.push(("content-type".into(), "application/json".into()));
        headers.push(("content-length".into(), body.len().to_string()));
    }
    if let Some(idempotency_key) = idempotency_key {
        headers.push(("idempotency-key".into(), idempotency_key.into()));
    }
    Ok(PreparedRequest {
        method,
        url: format!("{}{}", config.atlas_url(), path),
        headers,
        body,
    })
}

fn capture_body(request: &CaptureTextRequest) -> Result<Vec<u8>, AtlasHttpsError> {
    let body = serde_json::to_vec(&serde_json::json!({ "text": request.text() }))
        .map_err(|_| AtlasHttpsError::RequestTooLarge)?;
    Ok(body)
}

fn query_path(path: &str, fields: &[Option<(&str, &str)>]) -> String {
    let query = fields
        .iter()
        .flatten()
        .map(|(key, value)| format!("{key}={}", percent_encode(value)))
        .collect::<Vec<_>>();
    if query.is_empty() {
        path.into()
    } else {
        format!("{path}?{}", query.join("&"))
    }
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use core::fmt::Write as _;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

#[must_use]
const fn safe_read(request: &TransportRequest) -> bool {
    !matches!(request, TransportRequest::CaptureText { .. })
}

/// Retry only read operations, with a strict attempt limit and no mutation replay.
pub fn retry_safe_read<T, F>(
    request: &TransportRequest,
    mut operation: F,
) -> Result<T, TransportError>
where
    F: FnMut() -> Result<T, TransportError>,
{
    let attempts = if safe_read(request) {
        ATLAS_READ_ATTEMPT_LIMIT
    } else {
        1
    };
    let mut last_error = TransportError::Offline;
    for _ in 0..attempts {
        match operation() {
            Ok(value) => return Ok(value),
            Err(error) => last_error = error,
        }
    }
    Err(last_error)
}

#[cfg(target_os = "espidf")]
mod espidf {
    use std::{thread, time::Duration};

    use embedded_svc::{
        http::{client::Client as HttpClient, Method},
        io::Write as _,
        utils::io,
    };
    use esp_idf_svc::{
        http::client::{Configuration as HttpConfiguration, EspHttpConnection},
        sys,
    };

    use super::*;
    use crate::atlas_client::{AtlasTransport, TransportResponse};
    use crate::runtime_worker::{run_named_worker, NamedWorkerError};

    /// ESP-IDF HTTPS adapter. Each attempt constructs a fresh TLS connection;
    /// failed reads cannot poison a later request.
    pub struct EspIdfAtlasTransport {
        config: AtlasConfig,
    }

    impl EspIdfAtlasTransport {
        #[must_use]
        pub fn new(config: AtlasConfig) -> Self {
            Self { config }
        }
    }

    impl AtlasTransport for EspIdfAtlasTransport {
        fn execute(
            &mut self,
            request: TransportRequest,
        ) -> Result<TransportResponse, TransportError> {
            let config = self.config.clone();
            let request_for_worker = request.clone();
            let result =
                run_named_worker("atlas-https", ATLAS_HTTPS_WORKER_STACK_BYTES, move || {
                    execute_with_read_retries(&config, &request_for_worker)
                });
            match result {
                Ok(response) => Ok(response),
                Err(NamedWorkerError::Operation(error)) => Err(error),
                Err(_) => Err(TransportError::Offline),
            }
        }
    }

    fn execute_with_read_retries(
        config: &AtlasConfig,
        request: &TransportRequest,
    ) -> Result<TransportResponse, TransportError> {
        let attempts = if safe_read(request) {
            ATLAS_READ_ATTEMPT_LIMIT
        } else {
            1
        };
        let mut last_error = TransportError::Offline;
        for attempt in 0..attempts {
            match execute_once(config, request) {
                Ok(response) => return Ok(response),
                Err(error) => {
                    last_error = error;
                    if attempt + 1 < attempts {
                        thread::sleep(Duration::from_millis(ATLAS_READ_BACKOFF_MILLIS[attempt]));
                    }
                }
            }
        }
        Err(last_error)
    }

    fn execute_once(
        config: &AtlasConfig,
        request: &TransportRequest,
    ) -> Result<TransportResponse, TransportError> {
        let prepared = prepare_request(config, request).map_err(|_| TransportError::Offline)?;
        let http_config = HttpConfiguration {
            crt_bundle_attach: Some(sys::esp_crt_bundle_attach),
            timeout: Some(Duration::from_secs(ATLAS_HTTP_TIMEOUT_SECONDS)),
            buffer_size: Some(1024),
            buffer_size_tx: Some(1024),
            keep_alive_enable: false,
            ..Default::default()
        };
        let connection = EspHttpConnection::new(&http_config).map_err(classify_esp_error)?;
        let mut client = HttpClient::wrap(connection);
        let headers = prepared
            .headers
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect::<Vec<_>>();
        let method = match prepared.method {
            HttpMethod::Get => Method::Get,
            HttpMethod::Post => Method::Post,
        };
        let mut outgoing = client
            .request(method, prepared.url(), &headers)
            .map_err(classify_io_error)?;
        if prepared.method == HttpMethod::Post {
            outgoing
                .write_all(&prepared.body)
                .map_err(|error| classify_esp_error(error.0))?;
        }
        let mut response = outgoing
            .submit()
            .map_err(|error| classify_esp_error(error.0))?;
        let status = response.status();
        let mut body = [0_u8; ATLAS_HTTP_RESPONSE_BODY_BYTES];
        let read = io::try_read_full(&mut response, &mut body)
            .map_err(|error| classify_esp_error(error.0 .0))?;
        if read == body.len() {
            return Err(TransportError::Offline);
        }
        Ok(TransportResponse {
            status,
            body: body[..read].into(),
        })
    }

    fn classify_esp_error(error: esp_idf_svc::sys::EspError) -> TransportError {
        if error.code() == sys::ESP_ERR_TIMEOUT || error.code() == sys::ESP_ERR_HTTP_EAGAIN {
            TransportError::Timeout
        } else {
            TransportError::Offline
        }
    }

    fn classify_io_error(error: esp_idf_svc::io::EspIOError) -> TransportError {
        classify_esp_error(error.0)
    }
}

#[cfg(target_os = "espidf")]
pub use espidf::EspIdfAtlasTransport;
