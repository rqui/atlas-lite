# Atlas Lite — Voice-first capture decision

**Status:** Authoritative product decision
**Date:** 2026-09-04
**Applies after:** M5 — Bounded SD cache and idempotent queue

This decision supersedes the standalone **M6 — Text Capture** product milestone described in older roadmap/spec/master-plan sections.

## Decision

Atlas Lite will **not** implement a dedicated text-entry Capture UI as an MVP milestone.

The physical product is rotary/button driven and already includes the audio hardware needed for a much more natural capture path. Building and polishing a full text keyboard on e-paper would add substantial implementation and UX cost for comparatively low product value.

Therefore:

```text
M5  Offline Cache + Durable Queue
M6  CANCELLED / SKIPPED — standalone Text Capture UI
M7  Voice Capture — next functional milestone after M5
M8  Power, pairing, provisioning, OTA, release and polish
```

Keep the historical milestone numbers to avoid renumbering existing PRs, plans and reports. Future execution should explicitly skip M6.

## What remains from text capture

Do **not** remove the existing typed `capture_text` AtlasClient/REST capability merely because the UI milestone is cancelled.

It remains useful as infrastructure because:

- Atlas already exposes canonical `POST /api/v1/capture/text`;
- M5 queue/idempotency tests can use a small bounded mutation payload;
- future server-side STT can ultimately feed transcript text into canonical Atlas Capture;
- keeping the seam costs little and avoids unnecessary churn.

Do not build:

- a full rotary text keyboard for Capture;
- a dedicated long-form text editor;
- M6 Capture screen UX beyond any minimal placeholder/removal needed for coherent navigation.

If a tiny text-input/debug seam is needed for tests, keep it host/simulator-oriented and do not treat it as product UX.

## Voice-first product path

Atlas Lite already inherits Rustmix audio architecture for the Waveshare ESP32-S3-ePaper-3.97, including the ES8311 audio codec path and Voice Notes implementation.

The intended capture experience becomes:

```text
Home / Capture
    -> start voice recording
    -> PCM16 mono 16 kHz WAV
    -> persist safely under /ATLAS/AUDIO/
    -> queue/preserve while offline
    -> upload when network is available
    -> server-side STT
    -> canonical Atlas Capture
```

### M7 requirements remain strict

M7 must:

- reuse the working Rustmix microphone/audio ownership rather than reimplement I2S/audio drivers;
- use PCM16 mono 16 kHz WAV unless physical evidence requires changing it;
- finalize recordings recovery-safely;
- store audio under `/ATLAS/AUDIO/`;
- bound duration/file size/count/storage usage;
- survive reboot/interrupted upload without losing a finalized recording;
- never put AI-provider credentials on the ESP32;
- perform STT server-side;
- preserve idempotency across retries;
- use M5 durable persistence/queue primitives where appropriate;
- keep playback/speaker support optional for the first Voice milestone unless it is already trivial through inherited Rustmix code.

### Server integration

Before changing `rqui/atlas`, re-audit the current server.

Prefer the smallest broadly useful server contract. If the existing file-capture path can safely preserve WAV files first, that is acceptable as an intermediate step. If a dedicated audio-to-capture operation is needed, design it in a separate Atlas Server branch/worktree/Draft PR.

A meaningful route such as `/api/v1/capture/audio` is preferable to inventing `/api/v1/device/*`.

Server STT must:

1. require appropriate capture authorization;
2. enforce MIME/size/duration limits;
3. preserve idempotency;
4. inject the STT provider behind a server abstraction;
5. pass the resulting transcript into canonical Capture;
6. keep provider secrets exclusively server-side.

## Navigation/UI consequence

The existing `Capture` route may be retained to avoid unnecessary router churn, but its product meaning changes from **text entry** to **voice capture**.

Home should eventually expose Capture as a fast path to recording.

No standalone text Capture milestone should be scheduled after M5.

## Definition of done impact

The Atlas Lite MVP no longer requires manual text entry on the device.

Capture success for MVP means the physical device can record a bounded voice note, preserve it safely, deliver it to Atlas when connectivity permits, and obtain/submit server-side transcription without duplication or secret leakage.
