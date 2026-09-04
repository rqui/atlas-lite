//! Atlas voice capture persistence and upload boundary.
//!
//! Audio bytes are never loaded as a whole: the inherited `VoiceRecordingSession`
//! streams PCM into a temporary WAV and this module streams the final WAV to a
//! transport. Queue sidecars are deliberately in `AUDIO`, not M5's text queue.

use std::{
    collections::BTreeSet,
    fmt,
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::voice_notes::{
    build_pcm_wav_header, bytes_per_second, parse_pcm_wav_header, FinalizedVoiceWav,
    VoiceRecordingSession, WAV_HEADER_BYTES,
};

pub const ATLAS_AUDIO_ROOT: &str = "/sdcard/ATLAS/AUDIO";
pub const ATLAS_AUDIO_MAX_SECONDS: u32 = 5 * 60;
pub const ATLAS_AUDIO_MAX_WAV_BYTES: u64 = 9_600_044;
pub const ATLAS_AUDIO_MAX_FILES: usize = 16;
pub const ATLAS_AUDIO_MAX_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
pub const ATLAS_AUDIO_STREAM_CHUNK_BYTES: usize = 4_096;
const SCHEMA_VERSION: u8 = 1;
const MAX_SCAN: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtlasAudioLimits {
    pub max_wav_bytes: u64,
    pub max_files: usize,
    pub max_total_bytes: u64,
}

impl Default for AtlasAudioLimits {
    fn default() -> Self {
        Self {
            max_wav_bytes: ATLAS_AUDIO_MAX_WAV_BYTES,
            max_files: ATLAS_AUDIO_MAX_FILES,
            max_total_bytes: ATLAS_AUDIO_MAX_TOTAL_BYTES,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum UploadState {
    Pending,
    Sending,
    Acknowledged,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct PendingAudio {
    schema_version: u8,
    state: UploadState,
    idempotency_key: String,
    wav_name: String,
    wav_bytes: u64,
    sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingAudioUpload {
    pub wav_name: String,
    pub idempotency_key: String,
    pub wav_bytes: u64,
    pub sha256: String,
}

#[derive(Debug)]
pub enum VoiceCaptureError {
    Io(std::io::Error),
    Corrupt,
    UnsafeInventory,
    Limit,
    Name,
    Upload,
    Clock,
}
impl fmt::Display for VoiceCaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Io(_) => "audio storage I/O failed",
            Self::Corrupt => "audio data is corrupt",
            Self::UnsafeInventory => "audio inventory is unsafe",
            Self::Limit => "audio storage limit reached",
            Self::Name => "audio filename is unsafe",
            Self::Upload => "audio upload failed",
            Self::Clock => "waiting for valid network time",
        })
    }
}
impl std::error::Error for VoiceCaptureError {}
impl From<std::io::Error> for VoiceCaptureError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// Deliberately transport-neutral until the HTTP adapter owns the frozen wire
/// contract. `Stored` is the only acknowledgement that permits local deletion.
pub trait VoiceUploadTransport {
    fn upload_wav(
        &mut self,
        pending: &PendingAudioUpload,
        wav: &mut dyn Read,
    ) -> Result<VoiceUploadAck, VoiceCaptureError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoiceUploadAck {
    pub capture_id: String,
    pub attachment_name: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoiceUploadOutcome {
    Empty,
    RetainedForRetry,
    Acknowledged,
    UnsafeRetained,
}

#[derive(Clone, Debug)]
pub struct AtlasVoiceCapture {
    root: PathBuf,
    limits: AtlasAudioLimits,
}

impl AtlasVoiceCapture {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, VoiceCaptureError> {
        Self::with_limits(root, AtlasAudioLimits::default())
    }
    pub fn with_limits(
        root: impl Into<PathBuf>,
        limits: AtlasAudioLimits,
    ) -> Result<Self, VoiceCaptureError> {
        if limits.max_wav_bytes < WAV_HEADER_BYTES as u64
            || limits.max_files == 0
            || limits.max_files > ATLAS_AUDIO_MAX_FILES
            || limits.max_wav_bytes > ATLAS_AUDIO_MAX_WAV_BYTES
            || limits.max_total_bytes > ATLAS_AUDIO_MAX_TOTAL_BYTES
            || limits.max_total_bytes < limits.max_wav_bytes
        {
            return Err(VoiceCaptureError::Limit);
        }
        let value = Self {
            root: root.into(),
            limits,
        };
        value.recover()?;
        Ok(value)
    }
    pub fn start_recording(
        &self,
        recorded_at: String,
    ) -> Result<VoiceRecordingSession, VoiceCaptureError> {
        let inventory = self.inventory()?;
        if inventory.unsafe_inventory {
            return Err(VoiceCaptureError::UnsafeInventory);
        }
        for name in &inventory.wavs {
            let bytes = fs::metadata(self.root.join(name))?.len();
            if self.validate_wav(name, bytes).is_err() {
                return Err(VoiceCaptureError::UnsafeInventory);
            }
        }
        if inventory.wavs.len() >= self.limits.max_files
            || inventory
                .total_bytes
                .saturating_add(self.limits.max_wav_bytes)
                > self.limits.max_total_bytes
        {
            return Err(VoiceCaptureError::Limit);
        }
        let name = (1..=999_999)
            .map(|n| format!("A{n:06}.WAV"))
            .find(|n| !inventory.used.contains(n))
            .ok_or(VoiceCaptureError::Limit)?;
        VoiceRecordingSession::start_named(
            &self.root,
            &name,
            recorded_at,
            (self.limits.max_wav_bytes - WAV_HEADER_BYTES as u64) as u32,
        )
        .map_err(|_| VoiceCaptureError::Io(std::io::Error::other("start WAV recording")))
    }
    /// Commit a durable upload sidecar immediately after WAV finalization, before
    /// any transport call. Existing identity is immutable, including uncertain sends.
    pub fn persist_finalized(
        &self,
        wav: FinalizedVoiceWav,
    ) -> Result<PendingAudioUpload, VoiceCaptureError> {
        self.validate_wav(&wav.file_name, wav.wav_bytes)?;
        let inventory = self.inventory()?;
        if inventory.unsafe_inventory
            || inventory.total_bytes > self.limits.max_total_bytes
            || inventory.wavs.len() > self.limits.max_files
        {
            return Err(VoiceCaptureError::UnsafeInventory);
        }
        if let Some(existing) = inventory
            .pending
            .iter()
            .find(|p| p.wav_name == wav.file_name)
        {
            return Ok(upload_request(existing));
        }
        let key = idempotency_key()?;
        let sha256 = hash_wav(&self.root.join(&wav.file_name))?;
        let record = PendingAudio {
            schema_version: SCHEMA_VERSION,
            state: UploadState::Pending,
            idempotency_key: key.clone(),
            wav_name: wav.file_name.clone(),
            wav_bytes: wav.wav_bytes,
            sha256: sha256.clone(),
        };
        self.write_pending(&record)?;
        Ok(PendingAudioUpload {
            wav_name: wav.file_name,
            idempotency_key: key,
            wav_bytes: wav.wav_bytes,
            sha256,
        })
    }
    pub fn flush_one<T: VoiceUploadTransport>(
        &self,
        transport: &mut T,
    ) -> Result<VoiceUploadOutcome, VoiceCaptureError> {
        let inventory = self.inventory()?;
        if inventory.unsafe_inventory {
            return Ok(VoiceUploadOutcome::UnsafeRetained);
        }
        let Some(mut record) = inventory.pending.into_iter().next() else {
            return Ok(VoiceUploadOutcome::Empty);
        };
        if record.state == UploadState::Acknowledged {
            self.delete_pair(&record)?;
            return Ok(VoiceUploadOutcome::Acknowledged);
        }
        self.validate_wav(&record.wav_name, record.wav_bytes)?;
        if hash_wav(&self.root.join(&record.wav_name))? != record.sha256 {
            return Err(VoiceCaptureError::Corrupt);
        }
        let request = PendingAudioUpload {
            wav_name: record.wav_name.clone(),
            idempotency_key: record.idempotency_key.clone(),
            wav_bytes: record.wav_bytes,
            sha256: record.sha256.clone(),
        };
        let mut file = File::open(self.root.join(&record.wav_name))?;
        match transport.upload_wav(
            &request,
            &mut BoundedReader::new(&mut file, record.wav_bytes),
        ) {
            Ok(ack) if valid_ack(&ack, &request) => {
                record.state = UploadState::Acknowledged;
                self.write_pending(&record)?;
                self.delete_pair(&record)?;
                Ok(VoiceUploadOutcome::Acknowledged)
            }
            Ok(_) => Ok(VoiceUploadOutcome::RetainedForRetry),
            Err(_) => Ok(VoiceUploadOutcome::RetainedForRetry),
        }
    }
    pub fn cancel_and_delete(&self, wav_name: &str) -> Result<(), VoiceCaptureError> {
        let record = PendingAudio {
            schema_version: SCHEMA_VERSION,
            state: UploadState::Pending,
            idempotency_key: String::new(),
            wav_name: wav_name.into(),
            wav_bytes: 0,
            sha256: String::new(),
        };
        self.delete_pair(&record)
    }
    pub fn recover(&self) -> Result<(), VoiceCaptureError> {
        fs::create_dir_all(&self.root)?;
        self.check_root()?;
        // A backup is the last committed identity. Never mint a replacement for
        // a damaged identity; leave ambiguous/corrupt records for recovery.
        for entry in fs::read_dir(&self.root)?.take(MAX_SCAN + 1) {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.ends_with(".QBK") || name.ends_with(".QTM") {
                if name.len() != 11
                    || !is_wav(&format!("{}.WAV", &name[..7]))
                    || !entry.file_type()?.is_file()
                {
                    return Err(VoiceCaptureError::UnsafeInventory);
                }
                let path = entry.path();
                let committed = path.with_extension("AQ");
                if !committed.exists() {
                    let backup = path.with_extension("QBK");
                    let source = if backup.exists() {
                        backup
                    } else {
                        path.clone()
                    };
                    read_pending(&source)?;
                    fs::rename(source, &committed)?;
                }
                read_pending(&committed)?;
                if path.exists() {
                    fs::remove_file(path)?;
                }
            }
        }
        let inventory = self.inventory()?;
        if inventory.unsafe_inventory {
            return Err(VoiceCaptureError::UnsafeInventory);
        }
        for tmp in inventory.tmp {
            self.recover_tmp(&tmp)?;
        }
        let inventory = self.inventory()?;
        for name in inventory.wavs {
            if !inventory.pending.iter().any(|p| p.wav_name == name) {
                let size = fs::metadata(self.root.join(&name))?.len();
                let result = self.persist_finalized(FinalizedVoiceWav {
                    file_name: name,
                    wav_bytes: size,
                    pcm_bytes: size.saturating_sub(44) as u32,
                });
                if !matches!(result, Ok(_) | Err(VoiceCaptureError::Clock)) {
                    result?;
                }
            }
        }
        Ok(())
    }
    fn recover_tmp(&self, name: &str) -> Result<(), VoiceCaptureError> {
        let tmp = self.root.join(name);
        let final_name = format!("{}.WAV", &name[..7]);
        let final_path = self.root.join(&final_name);
        if fs::symlink_metadata(&tmp)?.file_type().is_symlink() || final_path.exists() {
            return Err(VoiceCaptureError::UnsafeInventory);
        }
        let mut file = File::options().read(true).write(true).open(&tmp)?;
        let size = file.metadata()?.len();
        if size <= WAV_HEADER_BYTES as u64
            || size > self.limits.max_wav_bytes
            || (size - WAV_HEADER_BYTES as u64) % 2 != 0
        {
            return Err(VoiceCaptureError::Corrupt);
        }
        let pcm =
            u32::try_from(size - WAV_HEADER_BYTES as u64).map_err(|_| VoiceCaptureError::Limit)?;
        let mut header = [0; WAV_HEADER_BYTES];
        file.read_exact(&mut header)?;
        parse_pcm_wav_header(&header).map_err(|_| VoiceCaptureError::Corrupt)?;
        file.seek(SeekFrom::Start(0))?;
        file.write_all(&build_pcm_wav_header(pcm))?;
        file.sync_all()?;
        drop(file);
        fs::rename(tmp, final_path)?;
        let raw = FinalizedVoiceWav {
            file_name: final_name,
            pcm_bytes: pcm,
            wav_bytes: size,
        };
        match self.persist_finalized(raw) {
            Ok(_) | Err(VoiceCaptureError::Clock) => {}
            Err(e) => return Err(e),
        }
        Ok(())
    }
    fn validate_wav(&self, name: &str, expected: u64) -> Result<(), VoiceCaptureError> {
        if !is_wav(name) {
            return Err(VoiceCaptureError::Name);
        }
        let path = self.root.join(name);
        let meta = fs::symlink_metadata(&path)?;
        if !meta.is_file()
            || meta.file_type().is_symlink()
            || meta.len() != expected
            || expected > self.limits.max_wav_bytes
        {
            return Err(VoiceCaptureError::Corrupt);
        }
        let mut file = File::open(path)?;
        let mut header = [0; WAV_HEADER_BYTES];
        file.read_exact(&mut header)?;
        let pcm = parse_pcm_wav_header(&header).map_err(|_| VoiceCaptureError::Corrupt)?;
        if header != build_pcm_wav_header(pcm) {
            return Err(VoiceCaptureError::Corrupt);
        }
        if pcm == 0
            || pcm % 2 != 0
            || u64::from(pcm).saturating_add(WAV_HEADER_BYTES as u64) != expected
            || pcm > bytes_per_second().saturating_mul(ATLAS_AUDIO_MAX_SECONDS)
        {
            return Err(VoiceCaptureError::Corrupt);
        }
        Ok(())
    }
    fn write_pending(&self, record: &PendingAudio) -> Result<(), VoiceCaptureError> {
        let bytes = serde_json::to_vec(record).map_err(|_| VoiceCaptureError::Corrupt)?;
        let path = self.root.join(sidecar(&record.wav_name)?);
        let tmp = path.with_extension("QTM");
        let backup = path.with_extension("QBK");
        let mut file = File::options().write(true).create_new(true).open(&tmp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        if path.exists() {
            fs::rename(&path, &backup)?;
        }
        fs::rename(tmp, &path)?;
        if backup.exists() {
            fs::remove_file(backup)?;
        }
        Ok(())
    }
    fn delete_pair(&self, record: &PendingAudio) -> Result<(), VoiceCaptureError> {
        for name in [record.wav_name.clone(), sidecar(&record.wav_name)?] {
            let p = self.root.join(name);
            match fs::symlink_metadata(&p) {
                Ok(m) if m.is_file() && !m.file_type().is_symlink() => fs::remove_file(p)?,
                Ok(_) => return Err(VoiceCaptureError::UnsafeInventory),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }
    fn inventory(&self) -> Result<Inventory, VoiceCaptureError> {
        fs::create_dir_all(&self.root)?;
        self.check_root()?;
        let mut out = Inventory::default();
        let mut count = 0;
        for entry in fs::read_dir(&self.root)? {
            count += 1;
            if count > MAX_SCAN {
                out.unsafe_inventory = true;
                break;
            }
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let ty = entry.file_type()?;
            if ty.is_symlink() {
                out.unsafe_inventory = true;
                continue;
            }
            out.total_bytes = out.total_bytes.saturating_add(entry.metadata()?.len());
            if is_wav(&name) && ty.is_file() {
                out.used.insert(name.clone());
                out.wavs.push(name);
            } else if is_tmp(&name) && ty.is_file() {
                out.used.insert(format!("{}.WAV", &name[..7]));
                out.tmp.push(name);
            } else if is_sidecar(&name) && ty.is_file() {
                out.used.insert(format!("{}.WAV", &name[..7]));
                match read_pending(&entry.path()) {
                    Ok(p)
                        if p.schema_version == SCHEMA_VERSION
                            && p.wav_name == name[..7].to_string() + ".WAV" =>
                    {
                        out.pending.push(p)
                    }
                    _ => out.unsafe_inventory = true,
                }
            } else {
                out.unsafe_inventory = true;
            }
        }
        out.wavs.sort();
        if out.total_bytes > self.limits.max_total_bytes
            || out.wavs.len() + out.tmp.len() > self.limits.max_files
        {
            out.unsafe_inventory = true;
        }
        out.pending.sort_by(|a, b| a.wav_name.cmp(&b.wav_name));
        Ok(out)
    }
    fn check_root(&self) -> Result<(), VoiceCaptureError> {
        for path in self.root.ancestors() {
            if fs::symlink_metadata(path)?.file_type().is_symlink() {
                return Err(VoiceCaptureError::UnsafeInventory);
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct Inventory {
    used: BTreeSet<String>,
    wavs: Vec<String>,
    tmp: Vec<String>,
    pending: Vec<PendingAudio>,
    total_bytes: u64,
    unsafe_inventory: bool,
}
fn is_wav(name: &str) -> bool {
    name.len() == 11
        && name.starts_with('A')
        && name.ends_with(".WAV")
        && name.as_bytes()[1..7].iter().all(u8::is_ascii_digit)
}
fn is_tmp(name: &str) -> bool {
    name.len() == 11
        && name.starts_with('A')
        && name.ends_with(".TMP")
        && name.as_bytes()[1..7].iter().all(u8::is_ascii_digit)
}
fn is_sidecar(name: &str) -> bool {
    name.len() == 10
        && name.starts_with('A')
        && name.ends_with(".AQ")
        && name.as_bytes()[1..7].iter().all(u8::is_ascii_digit)
}
fn sidecar(wav: &str) -> Result<String, VoiceCaptureError> {
    if !is_wav(wav) {
        Err(VoiceCaptureError::Name)
    } else {
        Ok(format!("{}.AQ", &wav[..7]))
    }
}
pub fn valid_ack(ack: &VoiceUploadAck, pending: &PendingAudioUpload) -> bool {
    uuid_like(&ack.capture_id)
        && uuid_like(ack.attachment_name.strip_suffix("-audio.wav").unwrap_or(""))
        && ack.sha256.len() == 64
        && ack.sha256.bytes().all(|c| c.is_ascii_hexdigit())
        && ack.sha256 == pending.sha256
        && ack.size == pending.wav_bytes
}
fn uuid_like(s: &str) -> bool {
    s.len() == 36
        && [8, 13, 18, 23]
            .into_iter()
            .all(|i| s.as_bytes().get(i) == Some(&b'-'))
        && s.bytes()
            .enumerate()
            .all(|(i, c)| [8, 13, 18, 23].contains(&i) || c.is_ascii_hexdigit())
}
pub fn hash_wav(path: &Path) -> Result<String, VoiceCaptureError> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buf = [0; ATLAS_AUDIO_STREAM_CHUNK_BYTES];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn idempotency_key() -> Result<String, VoiceCaptureError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if !(1_577_836_800..=9_999_999_999).contains(&now) {
        return Err(VoiceCaptureError::Clock);
    }
    let mut random = [0u8; 16];
    getrandom::getrandom(&mut random).map_err(|_| VoiceCaptureError::Upload)?;
    Ok(format!("v1.{now:010}.{}", URL_SAFE_NO_PAD.encode(random)))
}
fn upload_request(p: &PendingAudio) -> PendingAudioUpload {
    PendingAudioUpload {
        wav_name: p.wav_name.clone(),
        idempotency_key: p.idempotency_key.clone(),
        wav_bytes: p.wav_bytes,
        sha256: p.sha256.clone(),
    }
}
fn read_pending(path: &Path) -> Result<PendingAudio, VoiceCaptureError> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.len() > 1024 {
        return Err(VoiceCaptureError::Corrupt);
    }
    let p: PendingAudio =
        serde_json::from_slice(&fs::read(path)?).map_err(|_| VoiceCaptureError::Corrupt)?;
    let parts: Vec<_> = p.idempotency_key.split('.').collect();
    if p.schema_version != SCHEMA_VERSION
        || !is_wav(&p.wav_name)
        || p.wav_bytes <= 44
        || p.wav_bytes > ATLAS_AUDIO_MAX_WAV_BYTES
        || p.sha256.len() != 64
        || !p.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        || parts.len() != 3
        || parts[0] != "v1"
        || parts[1].len() != 10
        || !parts[1].bytes().all(|b| b.is_ascii_digit())
        || URL_SAFE_NO_PAD
            .decode(parts[2])
            .map_or(true, |b| b.len() != 16)
    {
        return Err(VoiceCaptureError::Corrupt);
    }
    Ok(p)
}
struct BoundedReader<'a> {
    inner: &'a mut File,
    remaining: u64,
}
impl<'a> BoundedReader<'a> {
    fn new(inner: &'a mut File, remaining: u64) -> Self {
        Self { inner, remaining }
    }
}
impl Read for BoundedReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let limit = usize::try_from(self.remaining.min(ATLAS_AUDIO_STREAM_CHUNK_BYTES as u64))
            .unwrap_or(0)
            .min(buf.len());
        let n = self.inner.read(&mut buf[..limit])?;
        self.remaining = self.remaining.saturating_sub(n as u64);
        Ok(n)
    }
}
