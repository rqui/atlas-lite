# Atlas Lite Architecture

## Purpose

Atlas Lite is a native client for Atlas on the Waveshare ESP32-S3-ePaper-3.97. It reuses Rustmix Wave's board integration, e-paper lifecycle, input, audio, SD, networking, and power-management work while replacing the multipurpose Rustmix product shell with an Atlas-focused shell.

```text
                    NAS / Server
              ┌────────────────────┐
              │    Atlas Server    │
              │ Vault / FTS / Views│
              │ Capture / Auth     │
              └─────────┬──────────┘
                        │ HTTPS / REST
                        │
              ┌─────────▼──────────┐
              │    Atlas Lite      │
              │     ESP32-S3       │
              │                    │
              │ Atlas UI           │
              │ AtlasClient        │
              │ cache / queue      │
              └──┬────┬────┬──────┘
                 │    │    │
              e-paper SD  audio
                 │
           rotary/buttons
```

Atlas Server is authoritative. Atlas Lite holds only bounded cache/state required for a low-power client.

## Upstream platform boundary

Rustmix Wave already separates hardware ownership from host-testable application modules:

```text
src/main.rs
  ESP-IDF integration
  peripheral ownership
  event loop
  cross-domain coordination
        │
        ├── src/app/state.rs
        ├── src/app/router.rs
        ├── src/app/screens/*
        ├── src/panel_refresh.rs
        ├── src/network.rs
        ├── src/storage.rs
        ├── src/voice_notes.rs
        └── other domain modules
```

Atlas Lite should preserve that shape.

### Keep close to upstream

- `src/epaper.rs`
- `src/framebuffer.rs`
- `src/orientation.rs`
- `src/panel_refresh.rs`
- `src/buttons.rs`
- `src/board_services.rs`
- `src/power*.rs`
- `src/rtc*.rs`
- `src/audio/*`
- ESP-IDF ownership in `src/main.rs`
- `src/runtime_worker.rs`
- `src/runtime_memory.rs`
- low-level SDMMC mounting/storage plumbing
- Wi-Fi runtime where compatible

`src/panel_refresh.rs` remains the single refresh-policy boundary. Atlas screens request refreshes; they do not select SSD1677 commands directly.

### Adapt at the product layer

The Atlas product layer should be explicit and host-testable:

```text
src/
  atlas/
    mod.rs
    client.rs
    dto.rs
    state.rs
    cache.rs
    queue.rs
    markdown.rs
    config.rs
  app/
    router.rs
    state.rs
    screens/
      atlas_home.rs
      atlas_library.rs
      atlas_note.rs
      atlas_search.rs
      atlas_views.rs
      atlas_capture.rs
      atlas_settings.rs
```

Exact file names may follow established Rustmix conventions, but Atlas-specific network, DTO, cache, queue, and state logic must remain grouped rather than leaking through hardware modules.

## Product routes

Target product routes:

```text
Home
Library
Note
Search
Views
Capture
Settings
```

Existing Rustmix routes may remain compiled during early milestones but must not remain exposed in the Atlas Lite navigation shell unless needed for diagnostics.

## Atlas protocol

### Decision: existing REST first

The MVP must begin with Atlas's existing REST surface.

Current Atlas provides:

```text
GET  /api/v1/notes
GET  /api/v1/notes/by-id/:id
GET  /api/v1/search
GET  /api/v1/views
GET  /api/v1/views/:id/results
POST /api/v1/capture/text
```

These already cover the Atlas Lite MVP.

The current Notes summary contract includes:

- stable `id`
- `title`
- `path`
- `revision`
- `parentId`
- `order`
- state/icon/update metadata

That is sufficient to derive an initial hierarchy on-device from a bounded page/set of summaries.

### Device façade gate

Do not create `/api/v1/device/*` in the initial implementation.

A server-side device façade becomes justified only when profiling demonstrates one or more of:

- response bodies cannot be processed within a safe bounded memory budget;
- the number of REST round trips creates unacceptable latency or battery use;
- Home requires an aggregation whose absence causes material power/latency cost;
- Atlas's current response shape forces unsafe generic JSON parsing;
- pagination/current hierarchy access produces an unacceptable request budget.

If that gate is met, the façade must be a thin adapter over canonical Atlas services, not duplicated business logic.

## Embedded DTO strategy

Atlas Lite must define narrow Rust DTOs containing only fields needed by the screen/operation.

Examples:

```rust
pub struct AtlasNoteSummary {
    pub id: Option<String>,
    pub title: String,
    pub revision: String,
    pub parent_id: Option<String>,
    pub order: Option<String>,
}
```

A full note initially needs:

```rust
pub struct AtlasNoteDocument {
    pub id: Option<String>,
    pub title: String,
    pub revision: String,
    pub body: String,
    pub parent_id: Option<String>,
}
```

Do not deserialize or retain arbitrary `frontmatter` in the reader path.

Search should ignore ranking fields not needed for rendering.

Views should initially render the stable result identity/title/path and ignore arbitrary `values` unless a later screen explicitly requires a column.

This keeps memory bounded and avoids generic JSON trees.

## HTTP client

The target-side client should follow Rustmix's existing bounded HTTPS pattern:

- ESP-IDF HTTP connection
- certificate bundle verification
- explicit request timeout
- bounded response body
- short-lived worker for TLS/JSON work if needed
- classified transport / HTTP / parse errors
- no secret-bearing logs

The client API exposed to host-testable code should be transport-independent where practical.

## Authentication

Atlas Lite uses a dedicated `at_v1` API key.

Minimum intended capability set:

```text
notes:read
search:read
views:read
capture:write
```

No write/move/trash/admin capabilities unless a future feature requires them.

Authorization is sent as:

```http
Authorization: Bearer at_v1...
```

The token never goes to SD or logs.

## Provisioning and NVS

Final Atlas Lite provisioning state:

```text
NVS
  device_id
  atlas_url
  api_token
  wifi_ssid
  wifi_credentials
  timezone / network preferences as needed
```

Rustmix currently supports removable-SD Wi-Fi provisioning. Atlas Lite may use it only for a temporary bring-up experiment before secure NVS provisioning exists. The production MVP must not depend on a plaintext SD credential file.

Pairing through Atlas Web is a later milestone; manual serial/NVS provisioning is acceptable for early development.

## microSD

The card is cache/data storage, not firmware storage.

```text
/ATLAS/
  CACHE/
    HOME/
    NOTES/
    VIEWS/
    SEARCH/
  QUEUE/
  AUDIO/
  ASSETS/
  LOGS/
```

Cache entries must be bounded and recoverable. Mutable files should use atomic/recovery-safe patterns compatible with FAT.

## Offline model

Atlas Lite is not a replicated Vault.

Cache only:

- recent/home summaries;
- explicitly opened note bodies;
- recently opened View result pages;
- optional recent search result pages.

Offline states:

```text
ONLINE
OFFLINE_CACHED
OFFLINE_NO_DATA
SYNCING
ERROR
```

Text captures created offline enter `/ATLAS/QUEUE/`.

Each queued mutation gets a persistent idempotency key before the first network attempt. The same key is reused across reboot/retry until Atlas acknowledges success.

## Capture

### Text

Use the existing canonical endpoint:

```text
POST /api/v1/capture/text
Idempotency-Key: <persistent request id>
```

### Voice

Rustmix already has host-testable PCM16 mono 16 kHz WAV framing and streamed SD recording.

Atlas Lite should reuse that machinery rather than reimplementing recording.

Voice delivery is staged:

1. preserve a local WAV safely;
2. optionally upload/store the audio through an existing Atlas file-capture path if useful;
3. add server-side STT only in a dedicated Atlas Server milestone;
4. after STT, use canonical Capture to create the text note.

No OpenAI or other provider secret belongs in the ESP32 firmware.

## Input model

Preserve physical conventions unless hardware tests show a problem.

Target semantics:

```text
rotary clockwise      next / down / page forward
rotary counterclock   previous / up / page back
rotary select         open / activate
BOOT short            contextual secondary action
BOOT long             Back
Power short           existing safe power/display action
Power long            sleep
```

Rustmix's keyboard-grid navigator should be reused for text entry.

## E-paper model

Native panel: 800×480. Rustmix currently uses a logical portrait 480×800 framebuffer.

Do not change orientation in the bootstrap milestone.

Use:

- shared refresh coordinator;
- partial refresh for ordinary navigation;
- periodic global/base refresh for ghost cleanup;
- global refresh after wake;
- no animation loops;
- no unnecessary live redraw.

## Power lifecycle

Target lifecycle:

```text
wake
 -> render retained/cached state quickly
 -> Wi-Fi on when sync is needed
 -> bounded sync
 -> render updated state
 -> Wi-Fi suspend/off
 -> idle
 -> deep sleep
```

Do not keep a permanent WebSocket.

Realtime may be considered only during an explicitly active session and only after measuring value against power cost.

## Error handling

Atlas unavailability must never cause a boot loop.

Classify at least:

- network unavailable;
- DNS/TLS/transport failure;
- Atlas 401/403;
- Atlas 404;
- Atlas 429/503 retryable;
- malformed/oversized response;
- SD missing/corrupt;
- cache corrupt;
- queue retry pending.

Always keep navigation responsive using the last valid local snapshot when possible.

## Hardware revision caution

Rustmix's current architecture documents an AXP2101 PMIC. Waveshare documentation for current board material may describe a different power-management component/revision.

Do not rewrite working Rustmix power code based solely on a documentation label. Validate the actual physical board and upstream code behavior first. Treat PMIC changes as a hardware-specific task with explicit bench verification.

## Validation

Host:

```bash
./scripts/validate.sh
```

Target:

- ESP-IDF build after target wiring changes
- serial boot log
- hardware smoke matrix

The final source of truth for hardware support is observed behavior on the user's Waveshare board, not compile success.
