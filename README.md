# Atlas Lite

Atlas Lite is a native, low-power e-paper client for [Atlas](https://github.com/rqui/atlas), targeting the Waveshare ESP32-S3-ePaper-3.97.

The firmware is based on [Rustmix Wave](https://github.com/aimindseye/rustmix-wave) and is designed to preserve its proven board, display, input, audio, SD, networking and power-management work while replacing the multipurpose product shell with an Atlas-focused knowledge client.

## Status

Current status: pre-MVP/bring-up. Implementation has not started in this bundle.

Target product surfaces:

```text
Home
Library
Note
Search
Views
Capture
Settings
```

Atlas Server remains the source of truth. Atlas Lite is not a port of the Atlas PWA and does not run Atlas Server locally.

## Authoritative plan

The authoritative roadmap is [`docs/implementation/ATLAS-LITE-01.md`](docs/implementation/ATLAS-LITE-01.md).

Read:

- `AGENTS.md`
- `docs/ATLAS_LITE_ARCHITECTURE.md`
- `docs/UPSTREAM.md`
- `docs/implementation/ATLAS-LITE-01.md`
- `docs/superpowers/specs/2026-09-03-atlas-lite-design.md`
- `docs/superpowers/plans/2026-09-03-atlas-lite-m0-m1.md`

## Intended repository relationship

```text
aimindseye/rustmix-wave
          │
          │ fork
          ▼
rqui/atlas-lite
```

Expected remotes:

```text
origin   -> rqui/atlas-lite
upstream -> aimindseye/rustmix-wave
```

## MVP protocol

Atlas Lite starts with Atlas's existing HTTPS REST API. A special `/api/v1/device/*` API is not part of the initial implementation and must be justified by real ESP32 profiling before it is added.

## Security baseline

The device uses a dedicated Atlas `at_v1` API key with minimum capabilities:

```text
notes:read
search:read
views:read
capture:write
```

Atlas API keys and product Wi-Fi credentials are not stored on microSD.

## License and upstream

Atlas Lite must retain the MIT license and required attribution from Rustmix Wave.

Atlas Lite is an independent project based on Rustmix Wave; it is not an official Rustmix Wave release.
