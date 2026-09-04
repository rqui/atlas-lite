//! Bounded Atlas HTTPS request preparation and ESP-IDF transport boundary.
//!
//! The portable portion composes redacted requests and retry policy for host
//! tests. ESP-IDF handles remain in the target-only adapter below.

use core::fmt;
use std::io;

use crate::{
    atlas_client::{
        validate_transport_request, CaptureTextRequest, RequestValidationError, TransportError,
        TransportRequest,
    },
    atlas_config::{is_canonical_at_v1_token, AtlasConfig},
    atlas_dto::MAX_RESPONSE_BODY_BYTES,
};

/// One Atlas HTTPS attempt has an explicit ESP-IDF timeout.
pub const ATLAS_HTTP_TIMEOUT_SECONDS: u64 = 10;
/// Successful and error bodies share the DTO response bound.
pub const ATLAS_HTTP_RESPONSE_BODY_BYTES: usize = MAX_RESPONSE_BODY_BYTES;
/// The largest JSON capture payload accepted at the transport boundary.
pub const ATLAS_HTTP_REQUEST_BODY_BYTES: usize = 4 * 1024;
/// URL including the validated Atlas base, encoded path, and encoded query.
pub const ATLAS_HTTP_URL_BYTES: usize = 640;
/// Sum of HTTP header names, values, framing, and final terminator.
pub const ATLAS_HTTP_HEADER_BYTES: usize = 256;
/// The maximum request material retained by the HTTPS worker at one time.
pub const ATLAS_HTTP_TOTAL_REQUEST_BYTES: usize = 5 * 1024;
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
            TransportError::ResponseTooLarge => Self::ServerError,
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
    InvalidRequest(RequestValidationError),
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

    #[must_use]
    pub const fn body_len(&self) -> usize {
        self.body.len()
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
    if !is_canonical_at_v1_token(config.api_token()) {
        return Err(AtlasHttpsError::InvalidToken);
    }
    validate_transport_request(request).map_err(AtlasHttpsError::InvalidRequest)?;
    let url_len = estimated_url_len(config.atlas_url().len(), request);
    if url_len > ATLAS_HTTP_URL_BYTES {
        return Err(AtlasHttpsError::RequestTooLarge);
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
    let header_bytes =
        estimated_header_bytes(config.api_token().len(), body.len(), idempotency_key);
    if header_bytes > ATLAS_HTTP_HEADER_BYTES
        || url_len + header_bytes + body.len() > ATLAS_HTTP_TOTAL_REQUEST_BYTES
    {
        return Err(AtlasHttpsError::RequestTooLarge);
    }
    let mut headers = Vec::with_capacity(5);
    headers.extend([
        ("accept".into(), "application/json".into()),
        (
            "authorization".into(),
            format!("Bearer {}", config.api_token()),
        ),
    ]);
    if method == HttpMethod::Post {
        headers.push(("content-type".into(), "application/json".into()));
        headers.push(("content-length".into(), body.len().to_string()));
    }
    if let Some(idempotency_key) = idempotency_key {
        headers.push(("idempotency-key".into(), idempotency_key.into()));
    }
    let mut url = String::with_capacity(url_len);
    url.push_str(config.atlas_url());
    url.push_str(&path);
    Ok(PreparedRequest {
        method,
        url,
        headers,
        body,
    })
}

fn estimated_url_len(base_len: usize, request: &TransportRequest) -> usize {
    base_len
        + match request {
            TransportRequest::ListNotes { cursor, limit } => {
                "/api/v1/notes".len()
                    + query_len(&[
                        cursor.as_deref().map(|value| ("cursor", value)),
                        Some(("limit", &limit.to_string())),
                    ])
            }
            TransportRequest::GetNote { id } => {
                "/api/v1/notes/by-id/".len() + percent_encoded_len(id)
            }
            TransportRequest::Search {
                query,
                limit,
                offset,
            } => {
                "/api/v1/search".len()
                    + query_len(&[
                        Some(("q", query)),
                        Some(("limit", &limit.to_string())),
                        Some(("offset", &offset.to_string())),
                    ])
            }
            TransportRequest::ListViews => "/api/v1/views".len(),
            TransportRequest::GetViewResults { id, cursor, limit } => {
                "/api/v1/views/".len()
                    + percent_encoded_len(id)
                    + "/results".len()
                    + query_len(&[
                        cursor.as_deref().map(|value| ("cursor", value)),
                        Some(("limit", &limit.to_string())),
                    ])
            }
            TransportRequest::CaptureText { .. } => "/api/v1/capture/text".len(),
        }
}

fn query_len(fields: &[Option<(&str, &str)>]) -> usize {
    let mut total = 0;
    let mut count: usize = 0;
    for (key, value) in fields.iter().flatten() {
        total += key.len() + 1 + percent_encoded_len(value);
        count += 1;
    }
    total + usize::from(count > 0) + count.saturating_sub(1)
}

fn percent_encoded_len(value: &str) -> usize {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
                1
            } else {
                3
            }
        })
        .sum()
}

fn estimated_header_bytes(
    token_len: usize,
    body_len: usize,
    idempotency_key: Option<&str>,
) -> usize {
    const HEADER_FRAMING_BYTES: usize = 4;
    let mut total = ("accept".len() + "application/json".len() + HEADER_FRAMING_BYTES)
        + ("authorization".len() + "Bearer ".len() + token_len + HEADER_FRAMING_BYTES);
    if body_len > 0 {
        total += "content-type".len() + "application/json".len() + HEADER_FRAMING_BYTES;
        total += "content-length".len() + body_len.to_string().len() + HEADER_FRAMING_BYTES;
    }
    if let Some(key) = idempotency_key {
        total += "idempotency-key".len() + key.len() + HEADER_FRAMING_BYTES;
    }
    total + 2
}

fn capture_body(request: &CaptureTextRequest) -> Result<Vec<u8>, AtlasHttpsError> {
    #[derive(serde::Serialize)]
    struct CapturePayload<'a> {
        text: &'a str,
    }

    let mut writer = BoundedJsonWriter::new(ATLAS_HTTP_REQUEST_BODY_BYTES);
    serde_json::to_writer(
        &mut writer,
        &CapturePayload {
            text: request.text(),
        },
    )
    .map_err(|_| AtlasHttpsError::RequestTooLarge)?;
    Ok(writer.into_inner())
}

/// A streaming JSON sink that never grows beyond the transport request cap.
struct BoundedJsonWriter {
    bytes: Vec<u8>,
    limit: usize,
}

impl BoundedJsonWriter {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(limit),
            limit,
        }
    }

    fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}

impl io::Write for BoundedJsonWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "bounded JSON request exceeded limit",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
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
    let mut encoded = String::with_capacity(percent_encoded_len(value));
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

#[cfg_attr(not(any(target_os = "espidf", test)), allow(dead_code))]
fn read_bounded_response<F>(mut read: F) -> Result<Vec<u8>, TransportError>
where
    F: FnMut(&mut [u8]) -> Result<usize, TransportError>,
{
    let mut body = Vec::with_capacity(ATLAS_HTTP_RESPONSE_BODY_BYTES);
    let mut chunk = [0_u8; 1024];
    loop {
        let read = read(&mut chunk)?;
        if read == 0 {
            return Ok(body);
        }
        if read > ATLAS_HTTP_RESPONSE_BODY_BYTES.saturating_sub(body.len()) {
            return Err(TransportError::ResponseTooLarge);
        }
        body.extend_from_slice(&chunk[..read]);
    }
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
        let body = read_bounded_response(|chunk| {
            io::try_read_full(&mut response, chunk).map_err(|error| classify_esp_error(error.0 .0))
        })?;
        Ok(TransportResponse { status, body })
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

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read as _};

    use super::{
        capture_body, read_bounded_response, CaptureTextRequest, TransportError,
        ATLAS_HTTP_REQUEST_BODY_BYTES, ATLAS_HTTP_RESPONSE_BODY_BYTES,
    };

    #[test]
    fn capture_body_is_exact_json_for_normal_text() {
        let request = CaptureTextRequest::new("remember this").unwrap();
        assert_eq!(
            capture_body(&request).unwrap(),
            br#"{"text":"remember this"}"#
        );
    }

    #[test]
    fn capture_body_writer_accounts_for_json_escaping_at_the_limit() {
        let request = CaptureTextRequest::new("\\".repeat(ATLAS_HTTP_REQUEST_BODY_BYTES)).unwrap();
        assert!(capture_body(&request).is_err());
    }

    #[test]
    fn bounded_reader_accepts_an_exactly_limited_body() {
        let mut reader = Cursor::new(vec![b'x'; ATLAS_HTTP_RESPONSE_BODY_BYTES]);
        assert_eq!(
            read_bounded_response(|chunk| reader.read(chunk).map_err(|_| TransportError::Offline))
                .unwrap()
                .len(),
            ATLAS_HTTP_RESPONSE_BODY_BYTES
        );
    }

    #[test]
    fn bounded_reader_rejects_the_first_byte_over_the_limit() {
        let mut reader = Cursor::new(vec![b'x'; ATLAS_HTTP_RESPONSE_BODY_BYTES + 1]);
        assert_eq!(
            read_bounded_response(|chunk| reader.read(chunk).map_err(|_| TransportError::Offline)),
            Err(TransportError::ResponseTooLarge)
        );
    }
}
