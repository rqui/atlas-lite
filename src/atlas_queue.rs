//! Durable, bounded Atlas capture queue.
//!
//! Queue data is deliberately limited to capture text, a persistent
//! idempotency key, and delivery state. It owns neither credentials nor retry
//! scheduling: one explicit flush attempts at most one queued capture.

use std::{collections::BTreeSet, fmt};

use serde::{Deserialize, Serialize};

use crate::{
    atlas_client::{
        AtlasClient, AtlasTransport, CaptureTextRequest, RequestValidationError,
        MAX_CAPTURE_TEXT_BYTES, MAX_IDEMPOTENCY_KEY_BYTES,
    },
    atlas_storage::{AtlasDirectory, AtlasEntryDisposition, AtlasStorage, AtlasStorageError},
};

/// Schema accepted by this firmware. Unknown records are retained, not sent.
pub const ATLAS_QUEUE_SCHEMA_VERSION: u8 = 1;
/// Operational bound independent from the SD-card file-size limit.
pub const MAX_QUEUE_RECORDS: usize = 16;
/// Total on-card byte budget for recognized queue records, including envelopes.
pub const MAX_QUEUE_BYTES: u64 = 128 * 1024;
const INTEGRITY_ENVELOPE_BYTES: u64 = 13;
const MAX_QUEUE_NAME_PROBES: u32 = 32;

/// Persistent lifecycle state. `Sending` is intentionally retried after a
/// restart because its server-side outcome is ambiguous.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum QueueState {
    Pending,
    Sending,
}

/// A typed queue record; Debug must not disclose the capture or key.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
struct CaptureQueueRecord {
    schema_version: u8,
    state: QueueState,
    idempotency_key: String,
    text: String,
}

impl fmt::Debug for CaptureQueueRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CaptureQueueRecord { <redacted> }")
    }
}

impl CaptureQueueRecord {
    fn new(request: &CaptureTextRequest, idempotency_key: &str) -> Self {
        Self {
            schema_version: ATLAS_QUEUE_SCHEMA_VERSION,
            state: QueueState::Pending,
            idempotency_key: idempotency_key.into(),
            text: request.text().into(),
        }
    }

    fn is_valid(&self) -> bool {
        self.schema_version == ATLAS_QUEUE_SCHEMA_VERSION
            && !self.text.is_empty()
            && self.text.len() <= MAX_CAPTURE_TEXT_BYTES
            && valid_idempotency_key(&self.idempotency_key)
    }

    fn request(&self) -> Result<CaptureTextRequest, RequestValidationError> {
        CaptureTextRequest::new(self.text.clone())
    }
}

/// Explicit queue outcomes are safe for UI/logging: no user text or key leaks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtlasQueueFlushOutcome {
    Empty,
    Acknowledged,
    RetainedForRetry,
    /// A bad record was preserved and skipped; a later explicit flush may send
    /// another valid item.
    CorruptRecordRetained,
}

/// Queue errors intentionally omit queue contents and identifiers.
#[derive(Debug)]
pub enum AtlasQueueError {
    Storage(AtlasStorageError),
    InvalidRequest(RequestValidationError),
    InvalidIdempotencyKey,
    IdempotencyConflict,
    Serialize,
    QueueFull,
    QueueBytesExceeded { attempted: u64, limit: u64 },
    UntrustedInventory,
}

impl fmt::Display for AtlasQueueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => write!(formatter, "Atlas queue storage error: {error}"),
            Self::InvalidRequest(_) => {
                formatter.write_str("Atlas queue capture request is invalid")
            }
            Self::InvalidIdempotencyKey => {
                formatter.write_str("Atlas queue idempotency key is invalid")
            }
            Self::IdempotencyConflict => {
                formatter.write_str("Atlas queue idempotency key conflicts")
            }
            Self::Serialize => formatter.write_str("Atlas queue record serialization failed"),
            Self::QueueFull => formatter.write_str("Atlas queue record limit reached"),
            Self::QueueBytesExceeded { attempted, limit } => {
                write!(
                    formatter,
                    "Atlas queue would exceed {limit} bytes ({attempted} bytes)"
                )
            }
            Self::UntrustedInventory => {
                formatter.write_str("Atlas queue inventory is incomplete or untrusted")
            }
        }
    }
}

impl std::error::Error for AtlasQueueError {}

impl From<AtlasStorageError> for AtlasQueueError {
    fn from(error: AtlasStorageError) -> Self {
        Self::Storage(error)
    }
}

#[derive(Clone, Debug)]
struct QueueEntry {
    name: String,
    record: CaptureQueueRecord,
}

#[derive(Debug)]
struct QueueInventory {
    entries: Vec<QueueEntry>,
    occupied_names: BTreeSet<String>,
    accounted_bytes: u64,
    untrusted: bool,
}

/// Durable capture repository. Callers supply a canonical Atlas idempotency
/// key, allowing the M6 UI to choose its key source without coupling this
/// persistence/retry layer to clocks, RNGs, or credentials.
#[derive(Clone, Debug)]
pub struct AtlasCaptureQueue {
    storage: AtlasStorage,
    max_records: usize,
    max_bytes: u64,
}

impl AtlasCaptureQueue {
    pub fn new(storage: AtlasStorage) -> Self {
        Self::with_limits(storage, MAX_QUEUE_RECORDS, MAX_QUEUE_BYTES)
    }

    pub fn with_limits(storage: AtlasStorage, max_records: usize, max_bytes: u64) -> Self {
        Self {
            storage,
            max_records: max_records.clamp(1, MAX_QUEUE_RECORDS),
            max_bytes: max_bytes.max(1).min(MAX_QUEUE_BYTES),
        }
    }

    /// Persist the canonical key and text before any network attempt. Repeating
    /// an enqueue with the same key/text is a no-op; a key for different text is
    /// rejected rather than risking an Atlas-side idempotency collision.
    pub fn enqueue_capture(
        &self,
        request: &CaptureTextRequest,
        idempotency_key: &str,
    ) -> Result<(), AtlasQueueError> {
        if !valid_idempotency_key(idempotency_key) {
            return Err(AtlasQueueError::InvalidIdempotencyKey);
        }
        let record = CaptureQueueRecord::new(request, idempotency_key);
        let bytes = encode(&record)?;
        self.storage.check_file_bytes(&bytes)?;

        let inventory = self.inventory()?;
        for entry in &inventory.entries {
            if entry.record.idempotency_key == idempotency_key {
                return if entry.record.text == request.text() {
                    Ok(())
                } else {
                    Err(AtlasQueueError::IdempotencyConflict)
                };
            }
        }
        if inventory.untrusted {
            return Err(AtlasQueueError::UntrustedInventory);
        }
        if inventory.entries.len() >= self.max_records {
            return Err(AtlasQueueError::QueueFull);
        }
        let attempted = inventory
            .accounted_bytes
            .saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX))
            .saturating_add(INTEGRITY_ENVELOPE_BYTES);
        if attempted > self.max_bytes {
            return Err(AtlasQueueError::QueueBytesExceeded {
                attempted,
                limit: self.max_bytes,
            });
        }
        let name = self.allocate_name(idempotency_key, &inventory.occupied_names)?;
        self.storage
            .replace_bytes(AtlasDirectory::Queue, &name, &bytes)?;
        Ok(())
    }

    /// Attempt exactly one durable record. A non-ACK transport/client result is
    /// retained (and normally restored to `Pending`) for a later explicit call.
    pub fn flush_one<T: AtlasTransport>(
        &self,
        client: &mut AtlasClient<T>,
    ) -> Result<AtlasQueueFlushOutcome, AtlasQueueError> {
        let inventory = self.inventory()?;
        let Some(mut entry) = inventory.entries.into_iter().next() else {
            return Ok(if inventory.untrusted {
                AtlasQueueFlushOutcome::CorruptRecordRetained
            } else {
                AtlasQueueFlushOutcome::Empty
            });
        };

        entry.record.state = QueueState::Sending;
        self.write_entry(&entry)?;
        let request = entry
            .record
            .request()
            .map_err(AtlasQueueError::InvalidRequest)?;
        match client.capture_text(&request, &entry.record.idempotency_key) {
            Ok(()) => {
                self.storage.remove_queue_file(&entry.name)?;
                Ok(AtlasQueueFlushOutcome::Acknowledged)
            }
            Err(_error) => {
                entry.record.state = QueueState::Pending;
                // If this persistence fails, the durable `Sending` record is
                // retained and a reboot will safely retry its same key.
                self.write_entry(&entry)?;
                Ok(AtlasQueueFlushOutcome::RetainedForRetry)
            }
        }
    }

    fn write_entry(&self, entry: &QueueEntry) -> Result<(), AtlasQueueError> {
        let bytes = encode(&entry.record)?;
        self.storage
            .replace_bytes(AtlasDirectory::Queue, &entry.name, &bytes)?;
        Ok(())
    }

    fn inventory(&self) -> Result<QueueInventory, AtlasQueueError> {
        let listing = self.storage.list(AtlasDirectory::Queue)?;
        let mut entries = Vec::new();
        let mut occupied_names = BTreeSet::new();
        let mut untrusted = listing.omitted_entries != 0
            || listing.corrupt_entries != 0
            || listing.unknown_entries != 0;
        for item in listing.entries {
            match item.disposition {
                AtlasEntryDisposition::Ready => {
                    occupied_names.insert(item.name.clone());
                    let Ok(bytes) = self.storage.read_bytes(AtlasDirectory::Queue, &item.name)
                    else {
                        untrusted = true;
                        continue;
                    };
                    let Ok(record) = serde_json::from_slice::<CaptureQueueRecord>(&bytes) else {
                        untrusted = true;
                        continue;
                    };
                    if !record.is_valid() {
                        untrusted = true;
                        continue;
                    }
                    entries.push(QueueEntry {
                        name: item.name,
                        record,
                    });
                }
                AtlasEntryDisposition::RecoveryArtifact
                | AtlasEntryDisposition::Corrupt
                | AtlasEntryDisposition::Unknown => untrusted = true,
            }
        }
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(QueueInventory {
            entries,
            occupied_names,
            accounted_bytes: listing.accounted_bytes,
            untrusted,
        })
    }

    fn allocate_name(
        &self,
        idempotency_key: &str,
        occupied_names: &BTreeSet<String>,
    ) -> Result<String, AtlasQueueError> {
        for probe in 0..MAX_QUEUE_NAME_PROBES {
            let name = format!(
                "Q{:07X}.Q",
                fnv1a(idempotency_key.as_bytes(), probe) & 0x0fff_ffff
            );
            if !occupied_names.contains(&name) {
                return Ok(name);
            }
        }
        Err(AtlasQueueError::QueueFull)
    }
}

fn encode(record: &CaptureQueueRecord) -> Result<Vec<u8>, AtlasQueueError> {
    serde_json::to_vec(record).map_err(|_| AtlasQueueError::Serialize)
}

fn valid_idempotency_key(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == MAX_IDEMPOTENCY_KEY_BYTES
        && &bytes[..3] == b"v1."
        && bytes[3..13].iter().all(u8::is_ascii_digit)
        && bytes[13] == b'.'
        && bytes[14..]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn fnv1a(bytes: &[u8], probe: u32) -> u32 {
    bytes
        .iter()
        .chain(probe.to_le_bytes().iter())
        .fold(0x811c_9dc5_u32, |hash, byte| {
            (hash ^ u32::from(*byte)).wrapping_mul(0x0100_0193)
        })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::atlas_client::{MockAtlasTransport, MockTransportOutcome, TransportRequest};
    use crate::atlas_storage::AtlasStorageLimits;

    const KEY_A: &str = "v1.1735689600.AAAAAAAAAAAAAAAAAAAAAA";
    const KEY_B: &str = "v1.1735689601.BBBBBBBBBBBBBBBBBBBBBB";

    fn root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "atlas-queue-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn queue(label: &str) -> (AtlasCaptureQueue, PathBuf) {
        let root = root(label);
        let storage = AtlasStorage::new(&root).unwrap();
        (AtlasCaptureQueue::new(storage), root)
    }

    fn request(text: &str) -> CaptureTextRequest {
        CaptureTextRequest::new(text).unwrap()
    }

    fn capture_request(client: &AtlasClient<MockAtlasTransport>) -> &TransportRequest {
        client.transport().requests().first().unwrap()
    }

    struct SendingInspectTransport {
        storage: AtlasStorage,
        name: String,
        saw_sending: bool,
    }

    impl AtlasTransport for SendingInspectTransport {
        fn execute(
            &mut self,
            _request: TransportRequest,
        ) -> Result<crate::atlas_client::TransportResponse, crate::atlas_client::TransportError>
        {
            let bytes = self
                .storage
                .read_bytes(AtlasDirectory::Queue, &self.name)
                .unwrap();
            let record: CaptureQueueRecord = serde_json::from_slice(&bytes).unwrap();
            self.saw_sending = record.state == QueueState::Sending;
            Ok(crate::atlas_client::TransportResponse {
                status: 204,
                body: Vec::new(),
                retry_after_seconds: None,
            })
        }
    }

    #[test]
    fn persists_key_and_text_before_any_network_attempt() {
        let (queue, root) = queue("persist-first");
        queue
            .enqueue_capture(&request("private thought"), KEY_A)
            .unwrap();
        let listing = fs::read_dir(root.join("QUEUE"))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(listing.len(), 1);
        let storage = AtlasStorage::new(&root).unwrap();
        let name = listing[0].file_name().into_string().unwrap();
        let bytes = storage.read_bytes(AtlasDirectory::Queue, &name).unwrap();
        let record: CaptureQueueRecord = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(record.state, QueueState::Pending);
        assert_eq!(record.idempotency_key, KEY_A);
        assert_eq!(record.text, "private thought");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn full_and_byte_limits_reject_without_mutation() {
        let root = root("limits");
        let storage = AtlasStorage::with_limits(
            root.clone(),
            AtlasStorageLimits {
                max_file_bytes: 1024,
                max_cache_bytes: 1024,
                max_directory_entries: 8,
            },
        )
        .unwrap();
        let queue = AtlasCaptureQueue::with_limits(storage, 1, 128);
        queue.enqueue_capture(&request("one"), KEY_A).unwrap();
        let before = fs::read_dir(root.join("QUEUE")).unwrap().count();
        assert!(matches!(
            queue.enqueue_capture(&request("two"), KEY_B),
            Err(AtlasQueueError::QueueFull)
        ));
        assert_eq!(fs::read_dir(root.join("QUEUE")).unwrap().count(), before);
        let byte_queue =
            AtlasCaptureQueue::with_limits(AtlasStorage::new(root.clone()).unwrap(), 2, 1);
        assert!(matches!(
            byte_queue.enqueue_capture(&request("three"), KEY_B),
            Err(AtlasQueueError::QueueBytesExceeded { .. })
        ));
        assert_eq!(fs::read_dir(root.join("QUEUE")).unwrap().count(), before);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn pending_and_sending_survive_reboot_and_retry_same_key() {
        let (queue, root) = queue("reboot");
        queue.enqueue_capture(&request("reboot me"), KEY_A).unwrap();
        let mut first = AtlasClient::new(MockAtlasTransport::default());
        first
            .transport_mut()
            .push_outcome(MockTransportOutcome::offline());
        assert_eq!(
            queue.flush_one(&mut first).unwrap(),
            AtlasQueueFlushOutcome::RetainedForRetry
        );
        let rebooted = AtlasCaptureQueue::new(AtlasStorage::new(&root).unwrap());
        let mut retry = AtlasClient::new(MockAtlasTransport::default());
        retry
            .transport_mut()
            .push_outcome(MockTransportOutcome::response(204, b""));
        assert_eq!(
            rebooted.flush_one(&mut retry).unwrap(),
            AtlasQueueFlushOutcome::Acknowledged
        );
        assert!(
            matches!(capture_request(&retry), TransportRequest::CaptureText { idempotency_key, .. } if idempotency_key == KEY_A)
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sending_is_persisted_before_transport_and_ambiguous_results_stay_queued() {
        let (queue, root) = queue("sending");
        queue
            .enqueue_capture(&request("lost reply"), KEY_A)
            .unwrap();
        let name = fs::read_dir(root.join("QUEUE"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .file_name()
            .into_string()
            .unwrap();
        let inspect = SendingInspectTransport {
            storage: AtlasStorage::new(&root).unwrap(),
            name,
            saw_sending: false,
        };
        let mut inspect_client = AtlasClient::new(inspect);
        assert_eq!(
            queue.flush_one(&mut inspect_client).unwrap(),
            AtlasQueueFlushOutcome::Acknowledged
        );
        assert!(inspect_client.transport().saw_sending);
        queue
            .enqueue_capture(&request("lost reply"), KEY_A)
            .unwrap();
        let mut client = AtlasClient::new(MockAtlasTransport::default());
        client
            .transport_mut()
            .push_outcome(MockTransportOutcome::lost_response());
        assert_eq!(
            queue.flush_one(&mut client).unwrap(),
            AtlasQueueFlushOutcome::RetainedForRetry
        );
        assert_eq!(client.transport().requests().len(), 1);
        // The retry has one record and the canonical key remains unchanged.
        let mut retry = AtlasClient::new(MockAtlasTransport::default());
        retry
            .transport_mut()
            .push_outcome(MockTransportOutcome::response(201, b""));
        assert_eq!(
            queue.flush_one(&mut retry).unwrap(),
            AtlasQueueFlushOutcome::Acknowledged
        );
        assert!(
            matches!(capture_request(&retry), TransportRequest::CaptureText { idempotency_key, .. } if idempotency_key == KEY_A)
        );
        assert_eq!(fs::read_dir(root.join("QUEUE")).unwrap().count(), 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn repeated_enqueue_is_idempotent_and_one_flush_attempts_one_item() {
        let (queue, root) = queue("duplicate");
        queue.enqueue_capture(&request("once"), KEY_A).unwrap();
        queue.enqueue_capture(&request("once"), KEY_A).unwrap();
        queue.enqueue_capture(&request("later"), KEY_B).unwrap();
        assert_eq!(fs::read_dir(root.join("QUEUE")).unwrap().count(), 2);
        let mut client = AtlasClient::new(MockAtlasTransport::default());
        client
            .transport_mut()
            .push_outcome(MockTransportOutcome::offline());
        assert_eq!(
            queue.flush_one(&mut client).unwrap(),
            AtlasQueueFlushOutcome::RetainedForRetry
        );
        assert_eq!(client.transport().requests().len(), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn acknowledgement_removes_exactly_the_sent_item() {
        let (queue, root) = queue("ack-one");
        queue.enqueue_capture(&request("first"), KEY_A).unwrap();
        queue.enqueue_capture(&request("second"), KEY_B).unwrap();
        let sent_key = queue.inventory().unwrap().entries[0]
            .record
            .idempotency_key
            .clone();
        let mut client = AtlasClient::new(MockAtlasTransport::default());
        client
            .transport_mut()
            .push_outcome(MockTransportOutcome::response(204, b""));
        assert_eq!(
            queue.flush_one(&mut client).unwrap(),
            AtlasQueueFlushOutcome::Acknowledged
        );
        let remaining = queue.inventory().unwrap().entries;
        assert_eq!(remaining.len(), 1);
        assert_ne!(remaining[0].record.idempotency_key, sent_key);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn non_ack_malformed_and_auth_errors_retain_item() {
        for outcome in [
            MockTransportOutcome::response(500, b"no"),
            MockTransportOutcome::response(401, b"malformed"),
            MockTransportOutcome::unauthorized(),
        ] {
            let (queue, root) = queue("retain");
            queue.enqueue_capture(&request("keep"), KEY_A).unwrap();
            let mut client = AtlasClient::new(MockAtlasTransport::default());
            client.transport_mut().push_outcome(outcome);
            assert_eq!(
                queue.flush_one(&mut client).unwrap(),
                AtlasQueueFlushOutcome::RetainedForRetry
            );
            assert_eq!(fs::read_dir(root.join("QUEUE")).unwrap().count(), 1);
            let _ = fs::remove_dir_all(root);
        }
    }

    #[test]
    fn corrupt_records_are_preserved_and_storage_failure_sends_nothing() {
        let (queue, root) = queue("corrupt");
        fs::write(root.join("QUEUE").join("BAD.Q"), b"corrupt").unwrap();
        let mut client = AtlasClient::new(MockAtlasTransport::default());
        assert_eq!(
            queue.flush_one(&mut client).unwrap(),
            AtlasQueueFlushOutcome::CorruptRecordRetained
        );
        assert!(root.join("QUEUE").join("BAD.Q").exists());
        assert!(client.transport().requests().is_empty());
        let storage = AtlasStorage::new(root.join("unavailable")).unwrap();
        let unavailable = AtlasCaptureQueue::new(storage.clone());
        unavailable
            .enqueue_capture(&request("no disk"), KEY_A)
            .unwrap();
        fs::remove_dir_all(root.join("unavailable").join("QUEUE")).unwrap();
        fs::write(root.join("unavailable").join("QUEUE"), b"not a directory").unwrap();
        let mut offline_client = AtlasClient::new(MockAtlasTransport::default());
        assert!(matches!(
            unavailable.flush_one(&mut offline_client),
            Err(AtlasQueueError::Storage(_))
        ));
        assert!(offline_client.transport().requests().is_empty());
        let _ = fs::remove_dir_all(root);
    }
}
