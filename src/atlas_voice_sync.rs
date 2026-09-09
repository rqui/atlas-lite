//! Serialized Atlas Voice Recordings synchronization state.

use std::collections::{BTreeMap, VecDeque};

use crate::{
    atlas_client::{AtlasClient, AtlasTransport},
    atlas_dto::{VoiceRecordingSummary, MAX_VOICE_RECORDINGS},
    atlas_voice_store::AtlasVoiceStore,
};

#[derive(Clone, Debug, Eq, PartialEq)]
enum Request {
    List(Option<String>),
    Download(VoiceRecordingSummary),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AtlasVoiceSyncState {
    pending: Option<Request>,
    downloads: VecDeque<VoiceRecordingSummary>,
    next_cursor: Option<String>,
    retained: usize,
    known_hashes: Option<BTreeMap<String, String>>,
    pub feedback: Option<&'static str>,
}

impl AtlasVoiceSyncState {
    pub fn request_sync(&mut self) {
        if self.pending.is_none() && self.downloads.is_empty() {
            self.retained = 0;
            self.next_cursor = None;
            self.known_hashes = None;
            self.pending = Some(Request::List(None));
            self.feedback = Some("Syncing recordings…");
        }
    }

    #[must_use]
    pub fn has_pending(&self) -> bool {
        self.pending.is_some() || !self.downloads.is_empty()
    }

    pub fn consume<T: AtlasTransport>(
        &mut self,
        client: &mut AtlasClient<T>,
        store: Option<&AtlasVoiceStore>,
    ) -> bool {
        let Some(request) = self.pending.take() else {
            return false;
        };
        let Some(store) = store else {
            self.feedback = Some("MicroSD unavailable");
            self.downloads.clear();
            return true;
        };
        match request {
            Request::List(cursor) => match client.list_voice_recordings(cursor.as_deref(), 32) {
                Ok(page) => {
                    if self.known_hashes.is_none() {
                        self.known_hashes = match store.inventory_hashes() {
                            Ok(hashes) => Some(hashes),
                            Err(_) => {
                                self.feedback = Some("Unable to scan saved recordings");
                                return true;
                            }
                        };
                    }
                    for item in page.items {
                        if self.retained >= MAX_VOICE_RECORDINGS {
                            break;
                        }
                        self.retained += 1;
                        if let Some(name) = self
                            .known_hashes
                            .as_ref()
                            .and_then(|hashes| hashes.get(&item.sha256))
                        {
                            let _ = store.update_metadata(name, &item);
                        } else {
                            self.downloads.push_back(item);
                        }
                    }
                    self.next_cursor = page.next_cursor;
                    self.advance();
                }
                Err(_) => {
                    self.downloads.clear();
                    self.feedback = Some("Offline · saved recordings available");
                }
            },
            Request::Download(summary) => {
                let mut promoted = None;
                let result = store.begin_download().and_then(|(name, staging)| {
                    let download = client
                        .download_voice_recording(
                            &summary.id,
                            &staging,
                            summary.byte_size,
                            &summary.sha256,
                        )
                        .map_err(|_| std::io::Error::other("voice recording download failed"))
                        .and_then(|()| store.finish_download(&name, &staging, &summary));
                    if download.is_err() {
                        store.discard_staging(&staging);
                    } else {
                        promoted = Some(name);
                    }
                    download
                });
                if result.is_err() {
                    self.downloads.clear();
                    self.next_cursor = None;
                    self.feedback = Some("Sync paused · saved recordings available");
                } else {
                    if let (Some(hashes), Some(name)) = (&mut self.known_hashes, promoted) {
                        hashes.insert(summary.sha256, name);
                    }
                    self.advance();
                }
            }
        }
        true
    }

    fn advance(&mut self) {
        if let Some(item) = self.downloads.pop_front() {
            self.pending = Some(Request::Download(item));
        } else if let Some(cursor) = self.next_cursor.take() {
            if self.retained < MAX_VOICE_RECORDINGS {
                self.pending = Some(Request::List(Some(cursor)));
            } else {
                self.feedback = Some("Recording limit reached");
            }
        } else {
            self.feedback = Some("Available offline");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        atlas_client::{TransportError, TransportRequest, TransportResponse},
        voice_notes::{build_pcm_wav_header, scan_voice_notes},
    };
    use sha2::{Digest, Sha256};
    use std::{fs, io::Write, path::Path};

    struct Transport {
        list: Vec<u8>,
        wav: Vec<u8>,
        executions: usize,
        downloads: usize,
    }

    impl AtlasTransport for Transport {
        fn execute(
            &mut self,
            request: TransportRequest,
        ) -> Result<TransportResponse, TransportError> {
            assert!(matches!(
                request,
                TransportRequest::ListVoiceRecordings { .. }
            ));
            self.executions += 1;
            Ok(TransportResponse {
                status: 200,
                body: self.list.clone(),
                retry_after_seconds: None,
            })
        }

        fn download_voice_recording(
            &mut self,
            _id: &str,
            destination: &Path,
            expected_bytes: u64,
            expected_sha256: &str,
        ) -> Result<(), TransportError> {
            assert_eq!(expected_bytes, self.wav.len() as u64);
            assert_eq!(expected_sha256, format!("{:x}", Sha256::digest(&self.wav)));
            std::fs::File::create(destination)
                .unwrap()
                .write_all(&self.wav)
                .unwrap();
            self.downloads += 1;
            Ok(())
        }
    }

    #[test]
    fn list_then_stream_download_becomes_offline_playable_without_duplicate_fetch() {
        let root = std::env::temp_dir().join(format!("atlas-voice-sync-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let store = AtlasVoiceStore::new(&root).unwrap();
        let mut wav = build_pcm_wav_header(4).to_vec();
        wav.extend_from_slice(&[0, 0, 1, 0]);
        let sha = format!("{:x}", Sha256::digest(&wav));
        let list = serde_json::to_vec(&serde_json::json!({
            "items": [{
                "id": "00000000-0000-4000-8000-000000000001",
                "title": "Offline test",
                "capturedAt": "2026-09-08T18:00:00.000Z",
                "durationMs": 1,
                "byteSize": wav.len(),
                "sha256": sha,
                "transcriptionStatus": "pending",
                "audioUrl": "/api/v1/voice-recordings/00000000-0000-4000-8000-000000000001/audio"
            }],
            "nextCursor": null
        }))
        .unwrap();
        let mut client = AtlasClient::new(Transport {
            list,
            wav,
            executions: 0,
            downloads: 0,
        });
        let mut sync = AtlasVoiceSyncState::default();
        sync.request_sync();
        assert!(sync.consume(&mut client, Some(&store)));
        assert!(sync.has_pending());
        assert!(sync.consume(&mut client, Some(&store)));
        assert!(!sync.has_pending());
        assert_eq!(client.transport().executions, 1);
        assert_eq!(client.transport().downloads, 1);
        assert_eq!(scan_voice_notes(&root).unwrap()[0].title, "OFFLINE TEST");
        fs::remove_dir_all(root).unwrap();
    }
}
