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
};

use serde::{Deserialize, Serialize};

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
enum UploadState { Pending, Sending, Acknowledged }

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct PendingAudio {
    schema_version: u8,
    state: UploadState,
    idempotency_key: String,
    wav_name: String,
    wav_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingAudioUpload { pub wav_name: String, pub idempotency_key: String, pub wav_bytes: u64 }

#[derive(Debug)]
pub enum VoiceCaptureError {
    Io(std::io::Error), Corrupt, UnsafeInventory, Limit, Name, Upload,
}
impl fmt::Display for VoiceCaptureError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(match self { Self::Io(_) => "audio storage I/O failed", Self::Corrupt => "audio data is corrupt", Self::UnsafeInventory => "audio inventory is unsafe", Self::Limit => "audio storage limit reached", Self::Name => "audio filename is unsafe", Self::Upload => "audio upload failed" }) } }
impl std::error::Error for VoiceCaptureError {}
impl From<std::io::Error> for VoiceCaptureError { fn from(value: std::io::Error) -> Self { Self::Io(value) } }

/// Deliberately transport-neutral until the HTTP adapter owns the frozen wire
/// contract. `Stored` is the only acknowledgement that permits local deletion.
pub trait VoiceUploadTransport {
    fn upload_wav(&mut self, pending: &PendingAudioUpload, wav: &mut dyn Read) -> Result<VoiceUploadAck, VoiceCaptureError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoiceUploadAck { pub capture_id: String, pub attachment_name: String, pub sha256: String, pub size: u64 }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoiceUploadOutcome { Empty, RetainedForRetry, Acknowledged, UnsafeRetained }

#[derive(Clone, Debug)]
pub struct AtlasVoiceCapture { root: PathBuf, limits: AtlasAudioLimits }

impl AtlasVoiceCapture {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, VoiceCaptureError> { Self::with_limits(root, AtlasAudioLimits::default()) }
    pub fn with_limits(root: impl Into<PathBuf>, limits: AtlasAudioLimits) -> Result<Self, VoiceCaptureError> {
        if limits.max_wav_bytes < WAV_HEADER_BYTES as u64 || limits.max_files == 0 || limits.max_total_bytes < limits.max_wav_bytes { return Err(VoiceCaptureError::Limit); }
        let value = Self { root: root.into(), limits };
        value.recover()?;
        Ok(value)
    }
    pub fn start_recording(&self, recorded_at: String) -> Result<VoiceRecordingSession, VoiceCaptureError> {
        let inventory = self.inventory()?;
        if inventory.unsafe_inventory { return Err(VoiceCaptureError::UnsafeInventory); }
        for name in &inventory.wavs {
            let bytes = fs::metadata(self.root.join(name))?.len();
            if self.validate_wav(name, bytes).is_err() {
                return Err(VoiceCaptureError::UnsafeInventory);
            }
        }
        if inventory.wavs.len() >= self.limits.max_files || inventory.total_bytes.saturating_add(self.limits.max_wav_bytes) > self.limits.max_total_bytes { return Err(VoiceCaptureError::Limit); }
        let name = (1..=999_999).map(|n| format!("A{n:06}.WAV")).find(|n| !inventory.used.contains(n)).ok_or(VoiceCaptureError::Limit)?;
        VoiceRecordingSession::start_named(&self.root, &name, recorded_at, bytes_per_second().saturating_mul(ATLAS_AUDIO_MAX_SECONDS)).map_err(|_| VoiceCaptureError::Io(std::io::Error::other("start WAV recording")))
    }
    /// Commit a durable upload sidecar immediately after WAV finalization, before
    /// any transport call. The deterministic key is content-derived and survives reboot.
    pub fn persist_finalized(&self, wav: FinalizedVoiceWav) -> Result<PendingAudioUpload, VoiceCaptureError> {
        self.validate_wav(&wav.file_name, wav.wav_bytes)?;
        let inventory = self.inventory()?;
        if inventory.unsafe_inventory || inventory.total_bytes > self.limits.max_total_bytes || inventory.wavs.len() > self.limits.max_files { return Err(VoiceCaptureError::UnsafeInventory); }
        let key = idempotency_key(&self.root.join(&wav.file_name))?;
        let record = PendingAudio { schema_version: SCHEMA_VERSION, state: UploadState::Pending, idempotency_key: key.clone(), wav_name: wav.file_name.clone(), wav_bytes: wav.wav_bytes };
        self.write_pending(&record)?;
        Ok(PendingAudioUpload { wav_name: wav.file_name, idempotency_key: key, wav_bytes: wav.wav_bytes })
    }
    pub fn flush_one<T: VoiceUploadTransport>(&self, transport: &mut T) -> Result<VoiceUploadOutcome, VoiceCaptureError> {
        let inventory = self.inventory()?;
        if inventory.unsafe_inventory { return Ok(VoiceUploadOutcome::UnsafeRetained); }
        let Some(mut record) = inventory.pending.into_iter().next() else { return Ok(VoiceUploadOutcome::Empty); };
        if record.state == UploadState::Acknowledged { self.delete_pair(&record)?; return Ok(VoiceUploadOutcome::Acknowledged); }
        self.validate_wav(&record.wav_name, record.wav_bytes)?;
        record.state = UploadState::Sending; self.write_pending(&record)?;
        let request = PendingAudioUpload { wav_name: record.wav_name.clone(), idempotency_key: record.idempotency_key.clone(), wav_bytes: record.wav_bytes };
        let mut file = File::open(self.root.join(&record.wav_name))?;
        match transport.upload_wav(&request, &mut BoundedReader::new(&mut file, record.wav_bytes)) {
            Ok(ack) if valid_ack(&ack, record.wav_bytes) => { record.state = UploadState::Acknowledged; self.write_pending(&record)?; self.delete_pair(&record)?; Ok(VoiceUploadOutcome::Acknowledged) }
            Ok(_) => { record.state = UploadState::Pending; self.write_pending(&record)?; Ok(VoiceUploadOutcome::RetainedForRetry) }
            Err(_) => { record.state = UploadState::Pending; self.write_pending(&record)?; Ok(VoiceUploadOutcome::RetainedForRetry) }
        }
    }
    pub fn cancel_and_delete(&self, wav_name: &str) -> Result<(), VoiceCaptureError> { let record = PendingAudio { schema_version: SCHEMA_VERSION, state: UploadState::Pending, idempotency_key: String::new(), wav_name: wav_name.into(), wav_bytes: 0 }; self.delete_pair(&record) }
    pub fn recover(&self) -> Result<(), VoiceCaptureError> {
        fs::create_dir_all(&self.root)?;
        let inventory = self.inventory()?;
        for tmp in inventory.tmp { self.recover_tmp(&tmp)?; }
        Ok(())
    }
    fn recover_tmp(&self, name: &str) -> Result<(), VoiceCaptureError> {
        let tmp = self.root.join(name); let final_name = format!("{}.WAV", &name[..7]); let final_path = self.root.join(&final_name);
        if fs::symlink_metadata(&tmp)?.file_type().is_symlink() || final_path.exists() { return Err(VoiceCaptureError::UnsafeInventory); }
        let mut file = File::options().read(true).write(true).open(&tmp)?; let size = file.metadata()?.len();
        if size < WAV_HEADER_BYTES as u64 || size > self.limits.max_wav_bytes || (size - WAV_HEADER_BYTES as u64) % 2 != 0 { return Err(VoiceCaptureError::Corrupt); }
        let pcm = u32::try_from(size - WAV_HEADER_BYTES as u64).map_err(|_| VoiceCaptureError::Limit)?;
        file.seek(SeekFrom::Start(0))?; file.write_all(&build_pcm_wav_header(pcm))?; file.sync_all()?; drop(file); fs::rename(tmp, final_path)?;
        let raw = FinalizedVoiceWav { file_name: final_name, pcm_bytes: pcm, wav_bytes: size }; self.persist_finalized(raw)?; Ok(())
    }
    fn validate_wav(&self, name: &str, expected: u64) -> Result<(), VoiceCaptureError> {
        if !is_wav(name) { return Err(VoiceCaptureError::Name); }
        let path = self.root.join(name); let meta = fs::symlink_metadata(&path)?; if !meta.is_file() || meta.file_type().is_symlink() || meta.len() != expected || expected > self.limits.max_wav_bytes { return Err(VoiceCaptureError::Corrupt); }
        let mut file = File::open(path)?; let mut header = [0; WAV_HEADER_BYTES]; file.read_exact(&mut header)?; let pcm = parse_pcm_wav_header(&header).map_err(|_| VoiceCaptureError::Corrupt)?;
        if u64::from(pcm).saturating_add(WAV_HEADER_BYTES as u64) != expected || pcm > bytes_per_second().saturating_mul(ATLAS_AUDIO_MAX_SECONDS) { return Err(VoiceCaptureError::Corrupt); } Ok(())
    }
    fn write_pending(&self, record: &PendingAudio) -> Result<(), VoiceCaptureError> { let bytes = serde_json::to_vec(record).map_err(|_| VoiceCaptureError::Corrupt)?; let path = self.root.join(sidecar(&record.wav_name)?); let tmp = path.with_extension("TMP"); if path.exists() { fs::remove_file(&path)?; } fs::write(&tmp, bytes)?; File::open(&tmp)?.sync_all()?; fs::rename(tmp, path)?; Ok(()) }
    fn delete_pair(&self, record: &PendingAudio) -> Result<(), VoiceCaptureError> { for name in [record.wav_name.clone(), sidecar(&record.wav_name)?] { let p = self.root.join(name); match fs::symlink_metadata(&p) { Ok(m) if m.is_file() && !m.file_type().is_symlink() => fs::remove_file(p)?, Ok(_) => return Err(VoiceCaptureError::UnsafeInventory), Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}, Err(e) => return Err(e.into()), } } Ok(()) }
    fn inventory(&self) -> Result<Inventory, VoiceCaptureError> {
        fs::create_dir_all(&self.root)?; let mut out = Inventory::default(); let mut count = 0;
        for entry in fs::read_dir(&self.root)? { count += 1; if count > MAX_SCAN { out.unsafe_inventory = true; break; } let entry = entry?; let name = entry.file_name().to_string_lossy().to_ascii_uppercase(); let ty = entry.file_type()?; if ty.is_symlink() { out.unsafe_inventory = true; continue; }
            if is_wav(&name) && ty.is_file() { out.total_bytes = out.total_bytes.saturating_add(entry.metadata()?.len()); out.used.insert(name.clone()); out.wavs.push(name); }
            else if is_tmp(&name) && ty.is_file() { out.tmp.push(name); }
            else if is_sidecar(&name) && ty.is_file() { match fs::read(entry.path()).ok().and_then(|b| serde_json::from_slice::<PendingAudio>(&b).ok()) { Some(p) if p.schema_version == SCHEMA_VERSION && p.wav_name == name[..7].to_string() + ".WAV" => out.pending.push(p), _ => out.unsafe_inventory = true } }
            else { out.unsafe_inventory = true; }
        }
        out.wavs.sort(); out.pending.sort_by(|a,b| a.wav_name.cmp(&b.wav_name)); Ok(out)
    }
}

#[derive(Default)] struct Inventory { used: BTreeSet<String>, wavs: Vec<String>, tmp: Vec<String>, pending: Vec<PendingAudio>, total_bytes: u64, unsafe_inventory: bool }
fn is_wav(name: &str) -> bool { name.len() == 11 && name.starts_with('A') && name.ends_with(".WAV") && name.as_bytes()[1..7].iter().all(u8::is_ascii_digit) }
fn is_tmp(name: &str) -> bool { name.len() == 11 && name.starts_with('A') && name.ends_with(".TMP") && name.as_bytes()[1..7].iter().all(u8::is_ascii_digit) }
fn is_sidecar(name: &str) -> bool { name.len() == 10 && name.starts_with('A') && name.ends_with(".AQ") && name.as_bytes()[1..7].iter().all(u8::is_ascii_digit) }
fn sidecar(wav: &str) -> Result<String, VoiceCaptureError> { if !is_wav(wav) { Err(VoiceCaptureError::Name) } else { Ok(format!("{}.AQ", &wav[..7])) } }
fn valid_ack(ack: &VoiceUploadAck, size: u64) -> bool { uuid_like(&ack.capture_id) && uuid_like(ack.attachment_name.strip_suffix("-audio.wav").unwrap_or("")) && ack.sha256.len() == 64 && ack.sha256.bytes().all(|c| c.is_ascii_hexdigit()) && ack.size == size }
fn uuid_like(s: &str) -> bool { s.len() == 36 && [8,13,18,23].into_iter().all(|i| s.as_bytes().get(i) == Some(&b'-')) && s.bytes().enumerate().all(|(i,c)| [8,13,18,23].contains(&i) || c.is_ascii_hexdigit()) }
fn idempotency_key(path: &Path) -> Result<String, VoiceCaptureError> { let mut file = File::open(path)?; let mut hash = 0xcbf29ce484222325u64; let mut buf = [0; ATLAS_AUDIO_STREAM_CHUNK_BYTES]; loop { let n = file.read(&mut buf)?; if n == 0 { break; } for b in &buf[..n] { hash = (hash ^ u64::from(*b)).wrapping_mul(0x100000001b3); } } Ok(format!("v1.0000000000.{hash:016X}AAAAAA")) }
struct BoundedReader<'a> { inner: &'a mut File, remaining: u64 }
impl<'a> BoundedReader<'a> { fn new(inner: &'a mut File, remaining: u64) -> Self { Self { inner, remaining } } }
impl Read for BoundedReader<'_> { fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> { let limit = usize::try_from(self.remaining.min(ATLAS_AUDIO_STREAM_CHUNK_BYTES as u64)).unwrap_or(0).min(buf.len()); let n = self.inner.read(&mut buf[..limit])?; self.remaining = self.remaining.saturating_sub(n as u64); Ok(n) } }
