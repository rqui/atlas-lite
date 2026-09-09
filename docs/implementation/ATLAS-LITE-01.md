# ATLAS-LITE-01 — Initial product roadmap

**Product:** Atlas Lite, a native client for Waveshare ESP32-S3-ePaper-3.97.
**Firmware:** `rqui/atlas-lite`, based on `aimindseye/rustmix-wave`.
**Server:** `rqui/atlas`, the source of truth.

The original product scope and milestone numbering remain in force. This roadmap does not prescribe an agent framework or repeated execution of completed phases. Work directly under `AGENTS.md`.

The dated [voice-first decision](ATLAS-LITE-VOICE-FIRST-DECISION.md) cancels M6's manual Text Capture UI. The original [design baseline](ATLAS-LITE-DESIGN.md) is retained for provenance. The current task is the bounded [release-candidate corrections](ATLAS-LITE-RC-FIXES-01.md), not another product phase.

## Architecture and constraints

Atlas Server owns the Vault, hierarchy, search, Views, Capture, authorization, idempotency and server-side transcription. Atlas Lite owns native UI/input, bounded REST requests, local cache and pending operations, hardware/audio and power lifecycle.

Do not port the Web/PWA/React frontend, embed a browser, run Atlas Server on ESP32, clone the whole Vault, or add local LLM/embeddings. Keep unrelated games, generic weather/calendar/dictionary and Lua applications out of the Atlas shell. Hide before pruning working shared platform code.

Preserve upstream MIT attribution, hardware ownership, existing e-paper refresh coordination, SD, RTC, audio/I2S, PMIC/input and power primitives. Prefer grouped Atlas product modules and small adapters over broad driver changes. Retain portrait logical 480 x 800 unless a justified product/hardware change is validated.

Prefer existing REST and narrow typed DTOs. A new `/api/v1/device/*` facade needs measured memory, payload, latency or request-count evidence; it is not the default solution. Use canonical services rather than bypassing Atlas with direct filesystem/SQLite writes.

Use dedicated credentials with the minimum `notes:read`, `search:read`, `views:read`, `capture:write` capabilities. Store credentials internally, not on SD or in logs. Enforce limits on requests, responses, stored data, retries and recording duration. Preserve per-surface freshness, stable note identity and explicit offline/error labels.

Use isolated worktrees/branches and Draft PRs. Server changes stay in their own repository and PR. No production merge, deploy, publication or hardware write without explicit authorization.

## M0 — Fork and planning baseline

Create and verify the real fork relationship:

```text
origin   -> rqui/atlas-lite
upstream -> aimindseye/rustmix-wave
```

Record the actual upstream BASE and preserve history/license. Maintain README, this roadmap, Atlas-specific architecture, upstream guide, dated decisions and hardware evidence. Keep the useful original Rustmix platform documentation.

Acceptance: remote provenance and remotes are correct; implementation uses an isolated branch; planning is versioned; no unreported source changes.

## M1 — Atlas shell and hardware-preserving bring-up

Introduce Home, Library, Note, Search, Views, Capture and Settings routes. Home is the root. Note returns to its actual opening surface rather than a false static parent. Hide unrelated Rustmix applications from normal Atlas navigation without deleting shared infrastructure.

Use existing hardware snapshots to render a diagnostics-first Atlas Home: display, input, SD, Wi-Fi, battery and RTC. Screen modules must not acquire peripheral handles. Route refresh requests through the shared coordinator. Change user-facing branding without cosmetic mass-renaming of upstream internals.

Acceptance: route/state tests, host validation and target build pass. Boot, display/input, SD, RTC, battery, Wi-Fi and sleep/wake remain individually NOT TESTED until observed on the physical board.

## M1.5 — Native simulation harness

Reuse the real product state/router/framebuffer/fonts/renderers on a host-only simulator. Do not create a second UI. Keep GUI dependencies out of the ESP32 dependency graph and support deterministic headless frames and semantic input.

Simulate connected/connecting/offline/failed networking, battery levels, mounted/missing/error SD, clock and Atlas unconfigured/connected/auth/error states. Cover navigation, bounds and byte-identical repeated frames. Document controls and scope in `docs/SIMULATION.md`.

QEMU/peripheral emulation is optional exploration, not a dependency or a reason to rewrite working drivers. No custom display emulator is required. Simulation does not prove physical e-paper timing, ghosting or power behavior.

## M2 — Configuration, provisioning and AtlasClient

### Configuration and storage

Separate validated config, persistence interface, target NVS implementation and host fake. Values include device ID, Atlas URL/token and Wi-Fi SSID/credentials. Support missing/partial/ready/corrupt states, bounded fields, clear/update and a version hook. Debug/display must redact secrets. Product setup must not rely on plaintext `WIFI.TXT` or recompilation.

An early serial/host setup tool is a means, not a separate product requirement after the functioning M8 setup portal exists. Distinguish an implemented writer/receiver from an input-only stub and distinguish both from physical validation.

### Protocol

Audit current Atlas source and retain a transport-independent client for current contracts:

```text
GET  /api/v1/notes
GET  /api/v1/notes/by-id/:id
GET  /api/v1/search
GET  /api/v1/views
GET  /api/v1/views/:id/results
POST /api/v1/capture/text
```

Keep only required identity/title/revision/body/hierarchy/pagination and error fields. Ignore unneeded arbitrary frontmatter/View values. Enforce the response-byte cap before parsing; cover extra fields, malformed/missing fields, UTF-8 and oversized data.

Target HTTPS uses certificate verification, explicit timeouts, bounded workers/buffers, secret-free logs and limited retries. Preserve mutation idempotency. Test auth headers, URI construction, errors, retries and mock transport separately from actual TLS/radio behavior.

Acceptance: host-tested domain/client, target build and an explicit real connection test when environment permits. Do not claim live connection from fixtures alone.

## M3 — Home, Library and Note

Home shows bounded recent notes, View shortcuts, Capture access, time/battery/network/sync status. Use current REST; aggregate server endpoints only for a demonstrated constraint.

Library uses stable note IDs, parent IDs and canonical sibling order, not paths as identity. Bound retained pages/nodes. Handle cycles, duplicates and missing parents without panic or an invented complete hierarchy. Show incomplete data honestly; preserve selection and Back behavior.

Note uses the same reader regardless of origin. Render bounded headings, paragraphs, ordered/unordered lists, checkboxes, readable links/wikilinks and separators; bold/italic where practical. No arbitrary HTML/JavaScript or complex embeds. Use pagination/position state and keep opened-note retention bounded. Durable cache belongs to M5.

Acceptance: Home -> Library -> Note and Back, empty/error/offline/oversized input, Unicode/geometry and pagination tests in the shared simulator and a passing embedded build.

## M4 — Search and Views

Reuse keyboard-grid navigation for short Search queries; cancelling manual note entry does not cancel Search input. Search is server-authoritative, not a local Vault index. Handle empty query, index-not-ready/Retry-After, empty results, malformed/oversized responses, offline data and changed-query invalidation.

Views use existing list/result routes, validated View identity and opaque pagination cursors. Retain bounded titles/IDs and useful metadata, not desktop layouts or arbitrary column maps. Show remaining pages/budget limits honestly.

Search/Views use the existing Note reader and restore the actual origin on Back. Their freshness/error state must not relabel cached Library/Home data as live.

## M5 — Bounded cache and durable pending operations

Use the fixed `/ATLAS/` tree:

```text
CACHE/HOME
CACHE/NOTES
CACHE/VIEWS
CACHE/SEARCH
QUEUE
AUDIO
ASSETS
LOGS
```

Use bounded files, totals and inventory scans, root-confined paths, integrity checking and recovery-safe replacement. Reuse existing FAT-safe patterns; preserve unknown/corrupt data safely. Cache eviction must never delete pending captures or audio.

Cache recent Home/list data, opened notes and recent Views/Search pages with schema version, source revision/time and bounded eviction metadata. Offline reads use the last valid snapshot and state that it is cached.

Persist mutation identity before the first network attempt. Uncertain Sending state must be retryable with the same key after reboot or a lost response. Remove pending responsibility only on a valid canonical acknowledgement. Validate full storage, interrupted writes, corruption, reboot, lost-response retry and isolation from secrets.

The existing M5 queue is text-specific infrastructure. Voice may use a minimal separate durable queue while sharing persistence principles; do not force WAV bytes into a JSON text record or rewrite M5 unnecessarily. Do not equate repository primitives with full UI/runtime integration without exercising the call path.

## M6 — CANCELLED: standalone manual Text Capture UI

Do not build a rotary note-entry keyboard, long-form text editor or standalone manual capture milestone. Keep the existing typed `capture_text` method and useful queue tests. Preserve short input required for Search and setup. Keep Capture as the product route for M7 voice.

## M7 — Voice Capture

Reuse Rustmix `VoiceRecordingSession` and existing ES8311/I2S ownership. The intended format is PCM16 mono 16 kHz WAV. Bound duration, file count/size, total storage and transfer buffers. Finalize recordings safely under `/ATLAS/AUDIO/`; retain a stable random persisted idempotency key and audio identity through reset/offline/retry.

The product flow is:

```text
record -> finalized local WAV -> durable pending upload
-> Atlas durably accepts WAV and pending note -> device delivery acknowledged
-> automatic server STT -> transcript in the same note + preserved original WAV
```

The device does not wait for STT completion to transfer durable responsibility. Validate the actual receipt against the sent audio and canonical response. Preserve unsafe/expired records for recovery instead of silently duplicating or discarding them.

Server work stays separate. Use a canonical audio-capture contract, capture authorization, validated MIME/WAV/duration/size, idempotency and bounded automatic recovery. STT providers run behind a server abstraction; no provider key belongs on the ESP32. Original audio must remain associated with the transcription. A live compatible provider must be configured and tested before automatic transcription is advertised as operational.

Acceptance: recorder/queue/upload/reboot/offline/lost-ACK/duplicate/failure tests, server recovery and same-note transcript tests, streaming bounds and a real provider integration check when available. Microphone/audio quality and power-loss behavior still require hardware.

## M8 — Product setup, Settings, power and update readiness

### Setup and pairing

Provide first-boot setup without recompilation or typing long tokens with the rotary control. The existing M8 design uses a temporary protected AP/local portal for Wi-Fi and HTTPS Atlas URL, followed by explicit user approval in Atlas Web Devices settings. Reuse the implemented bounded pairing contract, pending identity/proof persistence, minimum scopes, expiry and revocation; do not replace its protocol with a second design.

Settings exposes status and bounded reset/unpair/update/restart actions. Describe exactly what local credentials/data and server authorization each action affects. Physical recovery must be documented and tested; don't call an input-only script a working provisioning path.

### Power

Preserve the central display coordinator and existing sleep/wake behavior. Use measured boot/connect/sync/reading/idle/sleep data to tune networking, not speculative autonomy claims. Avoid permanent WebSockets and unnecessary refreshes. Block unsafe sleep during recording or critical writes as the implemented design requires.

### Updates and releases

OTA must verify trusted source/signature, version, bounded size and digest; write the inactive slot and retain rollback until a justified health checkpoint. Treat application image, bootloader and partition table as one coherent build contract for initial installation. The release-candidate correction plan closes verification of that installation path.

Do not publish or sign releases without authorization. Never guess raw-address offsets or silently erase user data. Keep ROM/USB recovery available and document the first physical update/rollback validation.

### Cleanup

Do not delete platform code merely because Atlas hides its old UI. Prune only proven unused product code with measurable benefit and regression evidence. No cleanup is required as a pretext for another phase.

## Product acceptance and evidence

The device must ultimately boot, configure Wi-Fi, pair with limited credentials, browse/read/search/open Views, read cached content offline, record/preserve/upload voice, recover pending work without duplicates, retain the original audio with automatic same-note transcription, show status and recover/sleep/wake appropriately.

Manual Text Capture is not an acceptance requirement. Real provider, real server combination and hardware outcomes must be reported separately from mocks/builds. Implemented OTA code is not a published update service or tested rollback.

For each delivery record repository, base/head/branch, PR dependencies, commands/results, simulator/build/live/hardware evidence, artifact hashes where relevant, remaining issues and next concrete action. No test-count claim exceeds the exact tree actually tested.
