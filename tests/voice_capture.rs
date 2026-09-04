use std::{
    fs,
    io::Read,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use waveshare_epd397_rust_app::{
    voice_capture::{
        AtlasAudioLimits, AtlasVoiceCapture, VoiceCaptureError, VoiceUploadAck, VoiceUploadOutcome,
        VoiceUploadTransport,
    },
    voice_notes::{bytes_per_second, FinalizedVoiceWav},
};

fn root(label: &str) -> PathBuf {
    std::env::temp_dir().canonicalize().unwrap().join(format!(
        "atlas-voice-{label}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
fn limits() -> AtlasAudioLimits {
    AtlasAudioLimits {
        max_wav_bytes: 4096,
        max_files: 2,
        max_total_bytes: 8192,
    }
}
fn finalized(store: &AtlasVoiceCapture) -> FinalizedVoiceWav {
    let mut session = store.start_recording("DATE UNKNOWN".into()).unwrap();
    session.append_pcm16_mono(&[0, 0, 1, 0]).unwrap();
    session.finalize_raw().unwrap()
}
struct Ack {
    keys: Vec<String>,
    fail: bool,
    strict: bool,
}
impl VoiceUploadTransport for Ack {
    fn upload_wav(
        &mut self,
        p: &waveshare_epd397_rust_app::voice_capture::PendingAudioUpload,
        wav: &mut dyn Read,
    ) -> Result<VoiceUploadAck, VoiceCaptureError> {
        self.keys.push(p.idempotency_key.clone());
        let mut bytes = Vec::new();
        wav.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes.len() as u64, p.wav_bytes);
        if self.fail {
            return Err(VoiceCaptureError::Upload);
        }
        Ok(VoiceUploadAck {
            capture_id: "00000000-0000-4000-8000-000000000001".into(),
            attachment_name: if self.strict {
                "bad".into()
            } else {
                "00000000-0000-4000-8000-000000000001-audio.wav".into()
            },
            sha256: p.sha256.clone(),
            size: p.wav_bytes,
        })
    }
}

#[test]
fn finalized_recording_persists_and_reboot_retry_uses_same_key() {
    let r = root("retry");
    let store = AtlasVoiceCapture::with_limits(&r, limits()).unwrap();
    let p = store.persist_finalized(finalized(&store)).unwrap();
    drop(store);
    let store = AtlasVoiceCapture::with_limits(&r, limits()).unwrap();
    let mut offline = Ack {
        keys: vec![],
        fail: true,
        strict: false,
    };
    assert_eq!(
        store.flush_one(&mut offline).unwrap(),
        VoiceUploadOutcome::RetainedForRetry
    );
    let mut online = Ack {
        keys: vec![],
        fail: false,
        strict: false,
    };
    assert_eq!(
        store.flush_one(&mut online).unwrap(),
        VoiceUploadOutcome::Acknowledged
    );
    assert_eq!(offline.keys, online.keys);
    assert_eq!(offline.keys[0], p.idempotency_key);
    assert!(fs::read_dir(&r).unwrap().next().is_none());
    let _ = fs::remove_dir_all(r);
}

#[test]
fn lost_response_and_non_strict_ack_never_delete_audio() {
    let r = root("strict");
    let store = AtlasVoiceCapture::with_limits(&r, limits()).unwrap();
    let p = store.persist_finalized(finalized(&store)).unwrap();
    let mut bad = Ack {
        keys: vec![],
        fail: false,
        strict: true,
    };
    assert_eq!(
        store.flush_one(&mut bad).unwrap(),
        VoiceUploadOutcome::RetainedForRetry
    );
    assert!(r.join(&p.wav_name).exists());
    let mut retry = Ack {
        keys: vec![],
        fail: false,
        strict: false,
    };
    store.flush_one(&mut retry).unwrap();
    assert_eq!(bad.keys, retry.keys);
    let _ = fs::remove_dir_all(r);
}

#[test]
fn interrupted_tmp_is_finalized_and_queued_on_reboot() {
    let r = root("recovery");
    let store = AtlasVoiceCapture::with_limits(&r, limits()).unwrap();
    let mut session = store.start_recording("DATE UNKNOWN".into()).unwrap();
    session.append_pcm16_mono(&[0, 0]).unwrap();
    drop(session);
    drop(store);
    let store = AtlasVoiceCapture::with_limits(&r, limits()).unwrap();
    let mut ack = Ack {
        keys: vec![],
        fail: false,
        strict: false,
    };
    assert_eq!(
        store.flush_one(&mut ack).unwrap(),
        VoiceUploadOutcome::Acknowledged
    );
    let _ = fs::remove_dir_all(r);
}

#[test]
fn corrupt_symlink_and_storage_bounds_fail_closed() {
    let r = root("bounds");
    let store = AtlasVoiceCapture::with_limits(&r, limits()).unwrap();
    fs::write(r.join("A000001.WAV"), b"bad").unwrap();
    assert!(matches!(
        store.start_recording("x".into()),
        Err(VoiceCaptureError::UnsafeInventory)
    ));
    let _ = fs::remove_dir_all(r);
    let r = root("count");
    let store = AtlasVoiceCapture::with_limits(
        &r,
        AtlasAudioLimits {
            max_wav_bytes: 4096,
            max_files: 1,
            max_total_bytes: 4096,
        },
    )
    .unwrap();
    let f = finalized(&store);
    store.persist_finalized(f).unwrap();
    assert!(matches!(
        store.start_recording("x".into()),
        Err(VoiceCaptureError::Limit)
    ));
    assert_eq!(bytes_per_second(), 32_000);
    let _ = fs::remove_dir_all(r);
}

#[test]
fn identical_audio_has_random_canonical_distinct_identity_and_repeated_finalize_is_stable() {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    let r = root("identity");
    let store = AtlasVoiceCapture::with_limits(&r, limits()).unwrap();
    let wav = finalized(&store);
    let a = store.persist_finalized(wav.clone()).unwrap();
    assert_eq!(a, store.persist_finalized(wav).unwrap());
    let b = store.persist_finalized(finalized(&store)).unwrap();
    assert_ne!(a.idempotency_key, b.idempotency_key);
    assert_eq!(a.sha256, b.sha256);
    assert_eq!(
        URL_SAFE_NO_PAD
            .decode(a.idempotency_key.split('.').nth(2).unwrap())
            .unwrap()
            .len(),
        16
    );
    fs::remove_dir_all(r).unwrap();
}

#[test]
fn committed_identity_backup_and_finalization_gap_recover_without_regeneration() {
    let r = root("atomic");
    let store = AtlasVoiceCapture::with_limits(&r, limits()).unwrap();
    let wav = finalized(&store);
    // Reset between rename to WAV and queue creation.
    let store = AtlasVoiceCapture::with_limits(&r, limits()).unwrap();
    let p = store.persist_finalized(wav).unwrap();
    fs::rename(r.join("A000001.AQ"), r.join("A000001.QBK")).unwrap();
    fs::write(r.join("A000001.QTM"), b"interrupted").unwrap();
    let store = AtlasVoiceCapture::with_limits(&r, limits()).unwrap();
    let mut offline = Ack {
        keys: vec![],
        fail: true,
        strict: false,
    };
    store.flush_one(&mut offline).unwrap();
    assert_eq!(offline.keys, vec![p.idempotency_key]);
    fs::write(r.join("A000001.AQ"), b"corrupt").unwrap();
    assert!(AtlasVoiceCapture::with_limits(&r, limits()).is_err());
    assert!(r.join("A000001.WAV").exists());
    fs::remove_dir_all(r).unwrap();
}

#[test]
fn strict_wire_ack_and_mutated_same_size_wav_are_rejected() {
    use waveshare_epd397_rust_app::atlas_https::parse_audio_ack;
    let r = root("hash");
    let store = AtlasVoiceCapture::with_limits(&r, limits()).unwrap();
    let p = store.persist_finalized(finalized(&store)).unwrap();
    let receipt = serde_json::json!({"captureId":"00000000-0000-4000-8000-000000000001", "status":"accepted", "attachment":{"name":"00000000-0000-4000-8000-000000000002-audio.wav","sha256":p.sha256,"size":p.wav_bytes}});
    let bytes = serde_json::to_vec(&receipt).unwrap();
    assert!(parse_audio_ack(202, &bytes, &p).is_ok());
    for status in [200, 201, 204, 400] {
        assert!(parse_audio_ack(status, &bytes, &p).is_err());
    }
    for field in ["status", "captureId"] {
        let mut invalid = receipt.clone();
        invalid[field] = "wrong".into();
        assert!(parse_audio_ack(202, &serde_json::to_vec(&invalid).unwrap(), &p).is_err());
    }
    let mut invalid = receipt;
    invalid["attachment"]["sha256"] = "0".repeat(64).into();
    assert!(parse_audio_ack(202, &serde_json::to_vec(&invalid).unwrap(), &p).is_err());
    let mut wav = fs::read(r.join(&p.wav_name)).unwrap();
    wav[44] ^= 1;
    fs::write(r.join(&p.wav_name), wav).unwrap();
    let mut ack = Ack {
        keys: vec![],
        fail: false,
        strict: false,
    };
    assert!(store.flush_one(&mut ack).is_err());
    assert!(ack.keys.is_empty());
    fs::remove_dir_all(r).unwrap();
}

#[test]
fn byte_duration_inventory_and_symlink_bounds() {
    use waveshare_epd397_rust_app::voice_capture::ATLAS_AUDIO_MAX_WAV_BYTES;
    let r = root("byte-bound");
    let store = AtlasVoiceCapture::with_limits(&r, limits()).unwrap();
    let mut recording = store.start_recording("test".into()).unwrap();
    assert!(recording.append_pcm16_mono(&[0; 4096]).is_err());
    recording.cancel().unwrap();
    assert!(AtlasVoiceCapture::with_limits(
        &r,
        AtlasAudioLimits {
            max_wav_bytes: ATLAS_AUDIO_MAX_WAV_BYTES + 2,
            ..Default::default()
        }
    )
    .is_err());
    fs::write(r.join("A000001.TMP"), vec![0; 8192]).unwrap();
    assert!(store.start_recording("test".into()).is_err());
    fs::remove_file(r.join("A000001.TMP")).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("/private/tmp", r.join("A000001.WAV")).unwrap();
        assert!(store.start_recording("test".into()).is_err());
    }
    fs::remove_dir_all(r).unwrap();
}

#[test]
fn simulator_real_capture_back_reboot_lost_response_and_retry() {
    use waveshare_epd397_rust_app::{
        atlas_client::MockTransportOutcome,
        simulator::{SemanticInput, Simulator},
    };
    let r = root("sim");
    let mut sim = Simulator::default();
    sim.enable_voice(&r).unwrap();
    // Home has Library, Search, Views, Capture, Settings.
    for _ in 0..3 {
        sim.handle_input(SemanticInput::Down).unwrap();
    }
    sim.handle_input(SemanticInput::Select).unwrap();
    sim.handle_input(SemanticInput::Select).unwrap();
    sim.handle_input(SemanticInput::Back).unwrap();
    sim.voice_transport_mut()
        .push_outcome(MockTransportOutcome::lost_response());
    assert_eq!(
        sim.voice_tick().unwrap(),
        VoiceUploadOutcome::RetainedForRetry
    );
    let pending = sim.voice_transport_mut().audio_requests[0].clone();
    drop(sim);
    let mut sim = Simulator::default();
    sim.enable_voice(&r).unwrap();
    let receipt = serde_json::json!({"captureId":"00000000-0000-4000-8000-000000000001","status":"accepted","attachment":{"name":"00000000-0000-4000-8000-000000000002-audio.wav","sha256":pending.sha256,"size":pending.wav_bytes}});
    sim.voice_transport_mut()
        .push_outcome(MockTransportOutcome::response(
            202,
            serde_json::to_vec(&receipt).unwrap(),
        ));
    assert_eq!(sim.voice_tick().unwrap(), VoiceUploadOutcome::Acknowledged);
    assert_eq!(
        sim.voice_transport_mut().audio_requests[0].idempotency_key,
        pending.idempotency_key
    );
    assert_eq!(sim.voice_tick().unwrap(), VoiceUploadOutcome::Empty);
    fs::remove_dir_all(r).unwrap();
}
