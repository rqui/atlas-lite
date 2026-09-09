use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use waveshare_epd397_rust_app::{
    atlas_cache::{AtlasCacheMetadata, AtlasCacheRepository, AtlasOfflineStatus},
    atlas_client::{
        AtlasClient, CaptureTextRequest, MockAtlasTransport, MockTransportOutcome, TransportRequest,
    },
    atlas_dto::AtlasNoteDocument,
    atlas_queue::{AtlasCaptureQueue, AtlasQueueFlushOutcome},
    atlas_storage::{AtlasDirectory, AtlasStorage, AtlasStorageLimits},
};

const KEY: &str = "v1.1735689600.AAAAAAAAAAAAAAAAAAAAAA";
const ACK: &[u8] = br#"{"id":"00000000-0000-4000-8000-000000000001","path":"captures/one.md","state":"managed","title":"One","created":null,"updated":null,"revision":"r1","frontmatter":{},"body":"remember this"}"#;

fn root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "atlas-m54-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn document() -> AtlasNoteDocument {
    AtlasNoteDocument {
        id: Some("00000000-0000-4000-8000-000000000001".into()),
        title: "Cached".into(),
        revision: "r1".into(),
        body: "cached body".into(),
        parent_id: None,
        order: None,
    }
}

#[test]
fn cache_acceptance_covers_hit_miss_and_corrupt_isolation() {
    let root = root("cache");
    let storage = AtlasStorage::new(&root).unwrap();
    let cache = AtlasCacheRepository::new(storage.clone());
    assert_eq!(
        cache
            .offline_note("00000000-0000-4000-8000-000000000001")
            .status,
        AtlasOfflineStatus::OfflineNoData
    );
    cache
        .store_note(
            document(),
            AtlasCacheMetadata {
                source_revision: Some("r1".into()),
                source_timestamp: Some(1),
                last_used: 0,
            },
        )
        .unwrap();
    cache
        .store_note(
            AtlasNoteDocument {
                id: Some("00000000-0000-4000-8000-000000000002".into()),
                ..document()
            },
            AtlasCacheMetadata::default(),
        )
        .unwrap();
    let hit = cache.offline_note("00000000-0000-4000-8000-000000000001");
    assert_eq!(hit.status, AtlasOfflineStatus::OfflineCached);
    assert_eq!(hit.value.unwrap().body, "cached body");
    let entries = storage.list(AtlasDirectory::CacheNotes).unwrap().entries;
    let first_name = entries
        .iter()
        .find(|entry| {
            storage
                .read_bytes(AtlasDirectory::CacheNotes, &entry.name)
                .map(|bytes| {
                    String::from_utf8_lossy(&bytes).contains("00000000-0000-4000-8000-000000000001")
                })
                .unwrap_or(false)
        })
        .expect("cache entry for the targeted note")
        .name
        .clone();
    storage
        .replace_bytes(AtlasDirectory::CacheNotes, &first_name, b"truncated")
        .unwrap();
    let statuses = [
        cache
            .offline_note("00000000-0000-4000-8000-000000000001")
            .status,
        cache
            .offline_note("00000000-0000-4000-8000-000000000002")
            .status,
    ];
    assert_eq!(
        statuses,
        [AtlasOfflineStatus::Error, AtlasOfflineStatus::Error]
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn queue_acceptance_persists_reboots_retries_same_key_and_acks() {
    let root = root("queue");
    let storage = AtlasStorage::new(&root).unwrap();
    let queue = AtlasCaptureQueue::new(storage.clone());
    let request = CaptureTextRequest::new("remember this").unwrap();
    queue.enqueue_capture(&request, KEY).unwrap();
    let same_key_before = storage.list(AtlasDirectory::Queue).unwrap().entries;
    let same_key_before_bytes = storage
        .read_bytes(AtlasDirectory::Queue, &same_key_before[0].name)
        .unwrap();
    queue.enqueue_capture(&request, KEY).unwrap();
    let same_key_after = storage.list(AtlasDirectory::Queue).unwrap().entries;
    assert_eq!(same_key_after, same_key_before);
    assert_eq!(
        storage
            .read_bytes(AtlasDirectory::Queue, &same_key_before[0].name)
            .unwrap(),
        same_key_before_bytes
    );
    assert!(matches!(
        queue.enqueue_capture(&CaptureTextRequest::new("changed").unwrap(), KEY),
        Err(waveshare_epd397_rust_app::atlas_queue::AtlasQueueError::IdempotencyConflict)
    ));
    let rebooted = AtlasCaptureQueue::new(AtlasStorage::new(&root).unwrap());
    let mut transport = MockAtlasTransport::default();
    transport.push_outcome(MockTransportOutcome::offline());
    transport.push_outcome(MockTransportOutcome::lost_response());
    transport.push_outcome(MockTransportOutcome::response(201, ACK));
    let mut client = AtlasClient::new(transport);
    assert_eq!(
        rebooted.flush_one(&mut client).unwrap(),
        AtlasQueueFlushOutcome::RetainedForRetry
    );
    assert_eq!(
        rebooted.flush_one(&mut client).unwrap(),
        AtlasQueueFlushOutcome::RetainedForRetry
    );
    assert_eq!(
        rebooted.flush_one(&mut client).unwrap(),
        AtlasQueueFlushOutcome::Acknowledged
    );
    assert_eq!(
        client.transport().requests(),
        &[
            TransportRequest::CaptureText {
                request: request.clone(),
                idempotency_key: KEY.into()
            },
            TransportRequest::CaptureText {
                request,
                idempotency_key: KEY.into()
            },
            TransportRequest::CaptureText {
                request: CaptureTextRequest::new("remember this").unwrap(),
                idempotency_key: KEY.into()
            }
        ]
    );
    assert_eq!(
        storage.list(AtlasDirectory::Queue).unwrap().entries.len(),
        0
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn acceptance_limits_are_bounded_and_fail_without_mutation() {
    let root = root("limits");
    let storage = AtlasStorage::with_limits(
        root.clone(),
        AtlasStorageLimits {
            max_file_bytes: 512,
            max_cache_bytes: 512,
            max_directory_entries: 8,
        },
    )
    .unwrap();
    let queue = AtlasCaptureQueue::with_limits(storage.clone(), 2, 250);
    let request = CaptureTextRequest::new("bounded").unwrap();
    queue.enqueue_capture(&request, KEY).unwrap();
    let before = storage.list(AtlasDirectory::Queue).unwrap().entries;
    let before_bytes = storage
        .read_bytes(AtlasDirectory::Queue, &before[0].name)
        .unwrap();
    assert!(matches!(
        queue.enqueue_capture(
            &CaptureTextRequest::new("second").unwrap(),
            "v1.1735689601.BBBBBBBBBBBBBBBBBBBBBB"
        ),
        Err(waveshare_epd397_rust_app::atlas_queue::AtlasQueueError::QueueBytesExceeded { .. })
    ));
    let after = storage.list(AtlasDirectory::Queue).unwrap().entries;
    assert_eq!(after, before);
    assert_eq!(
        storage
            .read_bytes(AtlasDirectory::Queue, &before[0].name)
            .unwrap(),
        before_bytes
    );
    let _ = fs::remove_dir_all(root);
}
