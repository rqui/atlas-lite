# ATLAS-OFFLINE-MEDIA-01 — Offline Books and Voice Recordings

Status: Draft implementation. No merge, release, deployment or flash is authorized.

Atlas Lite renders the last complete SD catalogue first and then reconciles with
Atlas when online. `/sdcard/ATLAS/BOOKS` owns complete normalized Book replicas;
`/sdcard/ATLAS/VOICE` owns confirmed voice recordings; `/sdcard/ATLAS/AUDIO` remains
the upload outbox. These stores are separate from the disposable 512 KiB Atlas cache.

A Book is promoted only after its manifest, every server-declared cover and every
bounded content segment validate. A failed or partial list never deletes local books. A complete
authoritative catalogue may retire missing books after the active reader releases
them. Cover synchronization precedes content so the Books list improves immediately.

A finalized local recording is playable from the outbox. After a strict durable
server `202`, it is atomically promoted into the voice library rather than deleted.
The bounded server voice catalogue can add recordings made elsewhere. Playback
reuses the existing PCM16 mono 16 kHz audio runtime and never loads a whole WAV into
RAM.

Entering Voice Recordings renders `/sdcard/ATLAS/VOICE` first, then reconciles at
most 32 canonical server recordings. Missing WAV files stream through the existing
serialized Atlas HTTPS worker into a staging file, validate exact length, SHA-256
and PCM16 mono 16 kHz headers, and are renamed only after validation. Atlas Capture
uploads use their existing streaming worker, but the main loop prevents uploads and
other Atlas HTTP operations from being active concurrently.

Voice Recordings is no longer a separate Atlas Home row. Atlas Server exposes one
managed `Voice recordings` root in the normal Library hierarchy; a short SELECT keeps
the standard disclose/collapse behavior and a held SELECT enters the existing offline
voice player. Back returns to Library and held BOOT still returns to Home. Capture
remains the Home fast path for making a new recording, so removing the duplicate Home
row does not remove recording or offline playback.

All HTTP operations remain serialized. UI navigation and selector movement perform
no network I/O. Host tests prove recovery and state transitions; physical SD removal,
power loss, Wi-Fi/TLS, speaker playback and long-run heap behavior require hardware.

The final Xtensa release ELF is 2,726,800 bytes and its loadable flash text plus
read-only data is 2,490,004 bytes, inside each 6 MiB OTA slot. Against the branch
BASE artifact, flash text grew by 503,320 bytes and read-only data by 90,800 bytes;
this includes the merged streaming audio-capture runtime, complete offline Books,
SHA-256-verified Voice downloads and their UI. Static internal DRAM changed only
from 23,348 to 23,404 bytes of data and from 17,552 to 18,808 bytes of BSS. Runtime
heap and stack high-water still require hardware measurement.
