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
2. `docs/superpowers/plans/2026-09-04-atlas-lite-master-execution.md`
3. the detailed milestone plan named by the current task, when one exists
4. `docs/superpowers/specs/2026-09-03-atlas-lite-design.md`
5. `docs/ATLAS_LITE_ARCHITECTURE.md`
6. `docs/UPSTREAM.md`
7. current code and tests in this repository
8. current `master` of `rqui/atlas` for server/API behavior
9. historical plans/specs only as background

If a historical document conflicts with current code or the current Atlas Lite plan/spec, current code plus the current authoritative plan/spec win.

The detailed implementation requirements belong in repository documentation. Chat prompts should normally only identify the milestone to execute, the required branch/worktree boundary, and any temporary user-specific stop condition. Do not rely on a long chat prompt as the sole source of product requirements.

## Execution model

Use `superpowers:subagent-driven-development` for implementation.

Luna Max acts as lead/controller.

### Parallelism

Read-only discovery may be parallelized into independent scouts:

- Rustmix platform/hardware audit
- Atlas REST/auth/contracts audit
- Waveshare board/revision audit
- another narrowly scoped read-only investigation explicitly required by the milestone plan

Implementation on one worktree must be sequential by task. Do not run multiple implementers concurrently against the same branch/worktree.

Normal maximum concurrency:

```text
2 read-only scouts
1 implementation agent
1 reviewer for the completed task
```

Implementers must not create their own subagents.

For each material implementation task:

1. fresh implementer subagent;
2. run the specified tests;
3. commit the task;
4. task-scoped review for spec compliance and code quality;
5. fix/re-review until accepted, within the cost policy below;
6. continue to the next task.

For trivial/mechanical changes with deterministic tests, the controller may implement directly instead of creating an unnecessary implementer/reviewer pair.

Run a broad final review before declaring a milestone complete.

## Model and token-cost policy

Use subagents deliberately. Do not default every task to the most capable or highest-reasoning model.

### LOW / cheap model, low reasoning

Use for:

- documentation;
- fixtures;
- straightforward tests;
- scripts;
- mocks/fakes;
- formatting;
- mechanical refactors;
- symbol renames;
- one- or two-file changes with explicit acceptance criteria.

### MEDIUM / standard model, medium reasoning

Use for:

- Rust integration across several files;
- state/router changes;
- simulator implementation after architecture is decided;
- DTO/parsing work;
- NVS/config persistence;
- AtlasClient;
- networking;
- cache/queue logic;
- normal debugging;
- meaningful task reviews.

### HIGH / Max model, high reasoning

Reserve for:

1. one milestone preflight when real architectural judgment is required;
2. one blocker that cannot be resolved from the spec/current code using normal analysis;
3. the final whole-branch review.

Normal milestone target:

```text
HIGH/MAX calls <= 2
```

A third HIGH/MAX call requires a written reason in the local SDD ledger.

Do not spend HIGH/MAX calls on Markdown, fixtures, formatting, obvious tests, mechanical edits, or routine review fixes.

### Fix-loop budget

Normal maximum for the same finding:

```text
2 implement/fix + re-review rounds
```

If the same problem survives two rounds, the controller must diagnose the blocker before dispatching more agents. Escalate model capability only when the underlying problem actually needs additional judgment.

Do not duplicate reviews.

### Context discipline

Do not paste complete specs or accumulated conversation history into every subagent prompt.

Give each agent only:

- its focused task/brief;
- relevant files/interfaces;
- decisions it must preserve;
- acceptance tests;
- essential global constraints.

Detailed milestone requirements live under `docs/`; local execution artifacts live under `.superpowers/sdd/` and remain gitignored.

## Git rules

Never work directly on `main` or `master`.

Expected repository relationship:

```text
origin   -> rqui/atlas-lite
upstream -> aimindseye/rustmix-wave
```

Use isolated worktrees and milestone branches.

Create Draft PRs. Do not merge, deploy, publish releases, or rewrite upstream history without explicit approval.

Stacked PRs are allowed when a milestone depends on an unmerged predecessor. The PR base must be the branch it truly depends on so the diff contains only the new milestone.

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

Re-check current capability names in `rqui/atlas` before implementing the protocol milestone.

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

Keep evidence categories explicit:

```text
HOST TESTED
TARGET BUILD TESTED
SIMULATOR TESTED
QEMU TESTED
HARDWARE TESTED
```

Never mark these as hardware verified without the physical board:

- e-paper refresh and ghosting
- rotary/buttons
- PMIC/battery behavior
- SDMMC
- RTC
- Wi-Fi stability
- sleep/wake
- microphone/speaker/audio
- actual power consumption

Record hardware evidence separately from host/build/simulator evidence.

## Safety and server boundaries

Atlas Lite must never:

- bypass Atlas authorization;
- use an admin API key;
- execute arbitrary MCP tools;
- fetch arbitrary user-provided URLs from firmware;
- expose arbitrary filesystem access;
- allow unbounded uploads/responses;
- embed third-party AI provider secrets in firmware;
- accept arbitrary firmware update URLs.

Voice STT belongs server-side.

## Required validation

Preserve and run Rustmix validation:

```bash
./scripts/validate.sh
```

This currently covers formatting, source-contract validation, and host tests.

Run an ESP-IDF firmware build whenever the task touches target-side wiring:

```bash
./scripts/build.sh
```

Run simulator/headless validation whenever simulator/application rendering changes.

Hardware tests remain a separate explicit gate.

## Milestone reporting

Use the reporting contract in:

```text
docs/superpowers/plans/2026-09-04-atlas-lite-master-execution.md
```

Every milestone report must state the actual model strategy used, validation category, hardware pending state, deviations, and anything deliberately deferred to control cost/tokens.
