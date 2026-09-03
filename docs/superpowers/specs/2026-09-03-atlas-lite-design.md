# Atlas Lite Design Specification

**Status:** Approved design baseline for implementation  
**Date:** 2026-09-03  
**Target:** Waveshare ESP32-S3-ePaper-3.97  
**Firmware upstream:** `aimindseye/rustmix-wave`  
**Server:** `rqui/atlas`  
**Intended firmware repo:** `rqui/atlas-lite`

## 1. Goal

Build Atlas Lite as a native e-paper client for Atlas by preserving Rustmix Wave's proven ESP32/Waveshare platform and replacing its multipurpose product shell with a focused Atlas knowledge client.

The MVP succeeds when a physical device can securely connect to Atlas, browse/read/search knowledge, open Views, capture text, operate from bounded offline cache, recover queued captures idempotently, and sleep/wake correctly.

## 2. Non-goals

The MVP does not:

- port Atlas React/PWA;
- run a browser;
- run Atlas Server on ESP32;
- clone the whole Vault;
- expose arbitrary filesystem access;
- use MCP as the normal device protocol;
- run embeddings/RAG/LLM locally;
- implement a permanent always-listening assistant;
- preserve Rustmix games/weather/calendar/dictionary/Lua as product features.

## 3. System boundary

Atlas Server owns:

- Vault and Markdown source of truth;
- hierarchy truth;
- FTS/search;
- Views;
- Capture;
- auth/capabilities;
- idempotency;
- future STT/provider integration.

Atlas Lite owns:

- e-paper UI;
- navigation/input;
- narrow REST client;
- bounded cache;
- pending offline queue;
- NVS provisioning;
- SD data/cache;
- hardware/audio/power lifecycle.

## 4. Upstream strategy

`rqui/atlas-lite` must be a real fork of `aimindseye/rustmix-wave` if GitHub permits.

Expected remotes:

```text
origin   -> rqui/atlas-lite
upstream -> aimindseye/rustmix-wave
```

The implementation must retain upstream license/attribution and minimize edits to hardware/platform code.

## 5. Reuse decisions

### Reuse essentially as-is

- e-paper transport/framebuffer/orientation;
- panel refresh coordinator;
- rotary/button decoding;
- RTC;
- board services;
- power/sleep primitives;
- audio codec/I2S ownership;
- runtime worker/memory instrumentation;
- SDMMC mount/low-level access;
- Wi-Fi runtime, except credentials/provisioning model.

### Reuse with adaptation

- router/state/product shell;
- Home screen widgets;
- Reader typography/pagination concepts;
- keyboard-grid navigation;
- Voice Notes WAV recorder;
- recovery-safe FAT write patterns;
- HTTPS worker pattern.

### Hide/disable first, prune later

- games;
- Lua apps;
- dictionary;
- generic calendar;
- weather;
- unit converter;
- generic file-browser product surface;
- unrelated productivity screens.

## 6. REST decision

MVP starts with existing Atlas REST, not a new device API.

Required routes:

```text
GET  /api/v1/notes
GET  /api/v1/notes/by-id/:id
GET  /api/v1/search
GET  /api/v1/views
GET  /api/v1/views/:id/results
POST /api/v1/capture/text
```

Atlas's current note summaries already include stable ID, title, revision, parentId and order, which are enough for an initial bounded hierarchy model.

A `/api/v1/device/*` façade is prohibited until profiling supplies evidence that current REST causes a material memory, latency, round-trip, or energy problem.

## 7. Authentication

Use a dedicated `at_v1` API key with exactly the minimum capabilities required by the MVP:

```text
notes:read
search:read
views:read
capture:write
```

Do not grant write/move/trash/admin permissions for a read/capture-only MVP.

Store the token in NVS, never SD.

## 8. Networking

Use explicit HTTPS validation, bounded response buffers, request timeouts, limited retries, and classified errors.

Atlas Lite must not maintain a permanent WebSocket.

Network activity should be demand-based and power-aware.

## 9. DTO/memory policy

Device DTOs contain only screen-relevant fields.

Do not retain arbitrary Atlas frontmatter or View values in the baseline reader/list implementations.

Avoid generic JSON value trees in target-side normal flows. Parsing must have a clear response-size bound.

## 10. Storage

Use `/ATLAS` on FAT/FAT32 SD.

```text
/ATLAS/
  CACHE/
  QUEUE/
  AUDIO/
  ASSETS/
  LOGS/
```

Cache is bounded and disposable.

Queue entries are durable until acknowledged.

Secrets are forbidden on SD.

## 11. Offline semantics

Cached content may be read while offline.

Offline captures are queued with a stable idempotency key before the first attempt. The same key survives reset/retry and is removed only after successful Atlas acknowledgement.

The device must visibly distinguish stale/cached/offline state.

## 12. Text capture

Use the existing canonical Atlas endpoint:

```text
POST /api/v1/capture/text
```

with `Idempotency-Key`.

## 13. Voice

Reuse Rustmix's PCM16 mono 16 kHz recording implementation.

Voice is staged after text capture. STT runs server-side through an abstraction owned by Atlas Server. No AI provider secret is stored in firmware.

An existing Atlas file-capture route may be used as a safe first voice-note preservation path before STT if that reduces server scope.

## 14. UI

Product routes:

```text
Home
Library
Note
Search
Views
Capture
Settings
```

UI is monochrome, rotary-first, pagination-friendly, and e-paper-native.

No frame-rate-driven animations.

## 15. Power

Normal behavior aims for:

```text
wake -> cached frame -> conditional Wi-Fi sync -> updated frame -> Wi-Fi suspend -> idle/sleep
```

The e-paper image remains useful while the MCU sleeps.

## 16. Verification

Host tests cover:

- routing/state;
- DTO parsing;
- bounded response rejection;
- cache/queue serialization;
- idempotency persistence;
- offline/online transitions;
- Markdown subset;
- pagination;
- malformed payload handling.

Physical board validation covers:

- display;
- ghosting;
- input;
- SD;
- RTC;
- PMIC/battery;
- Wi-Fi;
- sleep/wake;
- audio;
- microphone;
- power use.

Hardware is never marked verified solely from build success.

## 17. Milestones

- M0 — Fork/bootstrap and planning baseline
- M1 — Atlas product shell + hardware-preserving bring-up
- M2 — Secure provisioning + AtlasClient
- M3 — Home + Library + Note
- M4 — Search + Views
- M5 — Offline cache + idempotent queue
- M6 — Text Capture
- M7 — Voice Capture / server STT bridge
- M8 — Power, pairing, OTA and product polish

Each milestone must end in a reviewable Draft PR or a clearly documented continuation branch. No merge/deploy/release without explicit approval.
