# AGENTS.md — Atlas Lite

## Mission

Atlas Lite is a native, low-power e-paper client for Atlas running on the Waveshare ESP32-S3-ePaper-3.97.

Atlas Server remains the source of truth. Atlas Lite must not become a local Atlas server, a copy of the Atlas web/PWA client, or a full Vault replica.

The firmware is based on the MIT-licensed upstream project:

- Upstream: `aimindseye/rustmix-wave`
- Intended fork: `rqui/atlas-lite`
- Atlas Server: `rqui/atlas`

## Binding sources of truth

Read these in order before changing code:

1. `docs/implementation/ATLAS-LITE-01.md`
2. `docs/ATLAS_LITE_ARCHITECTURE.md`
3. `docs/UPSTREAM.md`
4. Current code and tests in this repository
5. Current `master` of `rqui/atlas` for server/API behavior
6. Historical plans/specs only as background

If a historical document conflicts with current code or the current Atlas Lite plan, current code plus `ATLAS-LITE-01.md` win.

## Execution model

Use `superpowers:subagent-driven-development` for implementation.

Luna Max acts as lead/controller.

### Parallelism

Read-only discovery may be parallelized into independent scouts:

- Rustmix platform/hardware audit
- Atlas REST/auth/contracts audit
- Waveshare board/revision audit

Implementation on one worktree must be sequential by task. Do not run multiple implementers concurrently against the same branch/worktree.

For each implementation task:

1. fresh implementer subagent;
2. run the specified tests;
3. commit the task;
4. task-scoped review for spec compliance and code quality;
5. fix/re-review until accepted;
6. continue to the next task.

Run a broad final review before declaring a milestone complete.

## Git rules

Never work directly on `main` or `master`.

Expected repository relationship:

```text
origin   -> rqui/atlas-lite
upstream -> aimindseye/rustmix-wave
```

Use isolated worktrees and milestone branches.

Recommended first branch:

```text
codex/atlas-lite-bootstrap-01
```

Create Draft PRs. Do not merge, deploy, publish releases, or rewrite upstream history without explicit approval.

If Atlas Server changes are needed, use a separate worktree, branch, commits, and Draft PR in `rqui/atlas`.

## Upstream preservation

Treat Rustmix Wave as the hardware/platform upstream.

Prefer adding Atlas-specific modules over rewriting working platform code.

Areas to preserve closely unless a measured requirement forces a change:

- e-paper transport and refresh policy
- framebuffer/orientation
- button and power-key decoding
- RTC
- battery/PMIC integration
- audio/I2S ownership
- SDMMC mount and low-level storage handling
- Wi-Fi runtime
- runtime worker and memory diagnostics
- sleep/wake plumbing

Do not remove MIT license/copyright/attribution.

## Product scope

MVP surfaces:

- Home
- Library
- Note
- Search
- Views
- Capture
- Settings

Out of MVP:

- games
- generic weather
- generic calendar
- dictionary
- Lua apps
- browser/PWA/React
- local LLM
- local embeddings/RAG
- permanent wake-word assistant
- full Vault replication

Old Rustmix product features should first be hidden/disabled from the Atlas product shell, not aggressively deleted from platform code. Prune later only after the Atlas shell is stable and upstream merge cost is understood.

## Atlas protocol rules

Prefer existing REST APIs.

Initial MVP uses:

- `GET /api/v1/notes`
- `GET /api/v1/notes/by-id/:id`
- `GET /api/v1/search`
- `GET /api/v1/views`
- `GET /api/v1/views/:id/results`
- `POST /api/v1/capture/text`

Authentication:

```http
Authorization: Bearer at_v1...
```

Minimum intended capabilities:

- `notes:read`
- `search:read`
- `views:read`
- `capture:write`

Do not add `/api/v1/device/*` merely for convenience. Add a device façade only after measured evidence shows a real ESP32 constraint such as unacceptable payload size, request count, memory pressure, latency, or energy cost.

Do not access Atlas SQLite or Vault filesystem directly.

## Embedded data rules

Use bounded payloads, bounded lists, explicit timeouts, limited retries, and backoff.

On ESP32, deserialize only fields Atlas Lite needs. Avoid arbitrary generic JSON trees such as unrestricted `serde_json::Value` in normal device flows.

Do not retain `frontmatter`, View `values`, or other arbitrary maps unless a feature explicitly needs them.

## Secrets

Do not store Atlas API keys or product Wi-Fi credentials on microSD.

Final product secrets belong in NVS or another ESP32-appropriate protected persistent store.

Never log:

- API tokens
- Authorization headers
- Wi-Fi passwords
- provider secrets

Rustmix's `/sdcard/RUSTMIX/WIFI.TXT` is upstream behavior, not the final Atlas Lite security model.

## microSD

Atlas Lite data root:

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

Use bounded storage and recovery-safe `.TMP` / `.BAK` write patterns where applicable.

No secrets on SD.

## Hardware verification

Compilation is not hardware verification.

Never mark these as verified without the physical board:

- e-paper refresh and ghosting
- rotary/buttons
- PMIC/battery behavior
- SDMMC
- RTC
- Wi-Fi stability
- sleep/wake
- microphone/speaker/audio
- actual power consumption

Record hardware evidence separately from host/build evidence.

## Safety and server boundaries

Atlas Lite must never:

- bypass Atlas authorization;
- use an admin API key;
- execute arbitrary MCP tools;
- fetch arbitrary user-provided URLs from firmware;
- expose arbitrary filesystem access;
- allow unbounded uploads/responses;
- embed third-party AI provider secrets in firmware.

Voice STT belongs server-side.

## Required validation

Preserve and run Rustmix validation:

```bash
./scripts/validate.sh
```

This currently covers formatting, source-contract validation, and host tests.

Run an ESP-IDF firmware build whenever the task touches target-side wiring.

Hardware tests remain a separate explicit gate.
