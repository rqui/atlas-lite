//! Append-only, bounded SD replica for Atlas Voice Recordings.

use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

use crate::{
    atlas_dto::VoiceRecordingSummary,
    voice_note_metadata::{upsert_voice_note_metadata, VoiceNoteMetadata},
    voice_notes::{
        build_pcm_wav_header, bytes_per_second, is_voice_wav_name, parse_pcm_wav_header,
        WAV_HEADER_BYTES,
    },
};

#[derive(Clone, Debug)]
pub struct AtlasVoiceStore {
    root: PathBuf,
}

impl AtlasVoiceStore {
    pub fn new(root: impl Into<PathBuf>) -> std::io::Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        if fs::symlink_metadata(&root)?.file_type().is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "voice root is a symlink",
            ));
        }
        for entry in fs::read_dir(&root)?.take(crate::voice_notes::VOICE_NOTE_LIMIT + 1) {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_ascii_uppercase();
            let bytes = name.as_bytes();
            let owned_staging = bytes.len() == 8
                && bytes[0] == b'V'
                && &bytes[4..] == b".TMP"
                && bytes[1..4].iter().all(u8::is_ascii_digit);
            if owned_staging {
                let metadata = entry.metadata()?;
                if metadata.is_file() && !metadata.file_type().is_symlink() {
                    fs::remove_file(entry.path())?;
                }
            }
        }
        Ok(Self { root })
    }

    pub fn contains(&self, summary: &VoiceRecordingSummary) -> std::io::Result<bool> {
        let inventory = self.inventory_hashes()?;
        if let Some(name) = inventory.get(&summary.sha256) {
            self.update_metadata(name, summary)?;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn inventory_hashes(&self) -> std::io::Result<BTreeMap<String, String>> {
        let mut hashes = BTreeMap::new();
        for entry in fs::read_dir(&self.root)?.take(crate::voice_notes::VOICE_NOTE_LIMIT + 1) {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_ascii_uppercase();
            let metadata = fs::symlink_metadata(entry.path())?;
            if !is_voice_wav_name(&name) || !metadata.is_file() || metadata.file_type().is_symlink()
            {
                continue;
            }
            hashes.insert(hash_file(&entry.path())?, name);
        }
        Ok(hashes)
    }

    pub fn update_metadata(
        &self,
        file_name: &str,
        summary: &VoiceRecordingSummary,
    ) -> std::io::Result<()> {
        upsert_voice_note_metadata(
            &self.root,
            VoiceNoteMetadata {
                file_name: file_name.into(),
                recorded_at: summary.captured_at.clone(),
                title: summary.title.clone(),
            },
        )
        .map_err(std::io::Error::other)
    }

    pub fn begin_download(&self) -> std::io::Result<(String, PathBuf)> {
        for number in 1..=999 {
            let name = format!("VOICE{number:03}.WAV");
            let target = self.root.join(&name);
            let staging = self.root.join(format!("V{number:03}.TMP"));
            if !target.exists() && !staging.exists() {
                return Ok((name, staging));
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::StorageFull,
            "voice recording library is full",
        ))
    }

    pub fn finish_download(
        &self,
        file_name: &str,
        staging: &Path,
        summary: &VoiceRecordingSummary,
    ) -> std::io::Result<()> {
        if !is_voice_wav_name(file_name)
            || staging.parent() != Some(self.root.as_path())
            || staging.extension().and_then(|value| value.to_str()) != Some("TMP")
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid voice staging path",
            ));
        }
        let metadata = fs::symlink_metadata(staging)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() != summary.byte_size
            || hash_file(staging)? != summary.sha256
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid voice recording download",
            ));
        }
        let mut header = [0_u8; WAV_HEADER_BYTES];
        File::open(staging)?.read_exact(&mut header)?;
        let pcm_bytes = parse_pcm_wav_header(&header)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "unsupported WAV"))?;
        if header != build_pcm_wav_header(pcm_bytes)
            || pcm_bytes == 0
            || pcm_bytes % 2 != 0
            || u64::from(pcm_bytes).saturating_add(WAV_HEADER_BYTES as u64) != summary.byte_size
            || pcm_bytes > bytes_per_second().saturating_mul(5 * 60)
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid voice WAV length",
            ));
        }
        let target = self.root.join(file_name);
        if target.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "voice target already exists",
            ));
        }
        fs::rename(staging, &target)?;
        self.update_metadata(file_name, summary)
    }

    pub fn discard_staging(&self, staging: &Path) {
        if staging.parent() == Some(self.root.as_path()) {
            let _ = fs::remove_file(staging);
        }
    }
}

fn hash_file(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{atlas_dto::VoiceRecordingSummary, voice_notes::build_pcm_wav_header};
    use std::io::Write;

    #[test]
    fn promotes_only_a_hash_verified_supported_wav() {
        let root = std::env::temp_dir().join(format!("atlas-voice-store-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let store = AtlasVoiceStore::new(&root).unwrap();
        let (name, stage) = store.begin_download().unwrap();
        let mut bytes = build_pcm_wav_header(4).to_vec();
        bytes.extend_from_slice(&[0, 0, 1, 0]);
        File::create(&stage).unwrap().write_all(&bytes).unwrap();
        let summary = VoiceRecordingSummary {
            id: "00000000-0000-4000-8000-000000000001".into(),
            title: "Meeting".into(),
            captured_at: "2026-09-08T18:00:00.000Z".into(),
            duration_ms: 1,
            byte_size: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            transcription_status: "pending".into(),
            audio_url: "/api/v1/voice-recordings/id/audio".into(),
        };
        store.finish_download(&name, &stage, &summary).unwrap();
        assert!(store.contains(&summary).unwrap());
        assert!(root.join(name).is_file());
        fs::remove_dir_all(root).unwrap();
    }
}
