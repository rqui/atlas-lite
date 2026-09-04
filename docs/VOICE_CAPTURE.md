# M7 voice capture

Atlas Lite records PCM16, mono, 16 kHz WAV through the inherited
`VoiceRecordingSession` and native `AudioRuntime`; this milestone does not add
an I2S or ES8311 driver. Audio is streamed in 4 KiB chunks under
`/sdcard/ATLAS/AUDIO` and is never read into memory as a whole.

Each recording is bounded to five minutes: 16,000 samples/s × two bytes × 300
seconds plus the standard 44-byte WAV header = 9,600,044 bytes. The store
admits at most 16 finalized files and 64 MiB total. The large total leaves room
for recovery siblings without turning a removable SD card into an unbounded
queue.

The recorder writes `Axxxxxx.TMP`, synchronizes the finalized WAV header, then
renames to `Axxxxxx.WAV`. Startup validates and finalizes a valid interrupted
temporary file; malformed, unknown, symlinked, over-limit, or incomplete
inventory entries fail closed and are retained for diagnosis.

Immediately after WAV finalization, `Axxxxxx.AQ` records a stable canonical
idempotency key before any send. The sidecar has `Pending`, `Sending`, and
`Acknowledged` states. Ambiguous/lost responses and reboots retry the same key.
Only a strict `202` receipt matching the frozen audio contract (UUID capture
id, UUID-derived attachment name, 64 hex SHA-256, exact size) permits local
WAV deletion. It never waits for transcription completion. Cancellation before
finalization deletes only the temporary recording; explicit delete removes a
verified WAV/sidecar pair.

`POST /api/v1/capture/audio` sends raw `audio/wav`, bearer authorization, and
that persisted `Idempotency-Key`. The target HTTPS adapter must stream the WAV
and enforce the 202 receipt parser; the host transport seam is intentionally
contract-specific and does not introduce a generic file-upload protocol.
