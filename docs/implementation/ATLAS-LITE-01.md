# ATLAS-LITE-01 — Implementation Roadmap

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development` to execute task-by-task. Read `AGENTS.md` and `docs/superpowers/specs/2026-09-03-atlas-lite-design.md` first.

**Goal:** Deliver a maintainable Atlas Lite firmware fork for Waveshare ESP32-S3-ePaper-3.97, using Rustmix Wave as hardware/platform upstream and Atlas REST as the authoritative knowledge backend.

**Architecture:** Keep Rustmix's proven hardware ownership and e-paper lifecycle. Add a narrow Atlas product layer with typed bounded REST DTOs, NVS-held secrets, SD cache/queue, and e-paper-native screens. Atlas Server remains authoritative; server additions occur only behind evidence-backed gaps.

**Tech stack:** Rust 1.82+ / ESP-IDF / esp-idf-svc / embedded-graphics / existing Rustmix platform / Atlas REST `/api/v1`.

**Spec:** `docs/superpowers/specs/2026-09-03-atlas-lite-design.md`

## Global constraints

- Intended repo is a real GitHub fork: `rqui/atlas-lite` ← `aimindseye/rustmix-wave`.
- Preserve MIT license, notices, and upstream attribution.
- Never work directly on `main`/`master`.
- No merge, deploy, publish, release, or destructive history rewrite without explicit approval.
- Atlas Server is the source of truth.
- Do not port React/PWA/browser code to ESP32.
- Existing Atlas REST is the MVP protocol.
- Do not create `/api/v1/device/*` without profiling evidence.
- Dedicated Atlas key uses minimum scopes: `notes:read`, `search:read`, `views:read`, `capture:write`.
- Secrets are forbidden on SD and in logs.
- Target-side network bodies and parsed structures must be bounded.
- Do not use unrestricted generic JSON trees in ordinary ESP32 flows.
- Preserve the shared e-paper refresh coordinator.
- Compilation does not count as hardware verification.
- Server changes, when justified, use a separate branch/worktree/PR in `rqui/atlas`.

---

## M0 — Fork and bootstrap baseline

### Task M0.1: Create and verify the fork

**Deliverable:** remote fork exists and repository relationship is explicit.

- [ ] Create GitHub fork `rqui/atlas-lite` from `aimindseye/rustmix-wave`.
- [ ] Confirm default branch tracks the upstream history.
- [ ] Configure:
  ```text
  origin   -> rqui/atlas-lite
  upstream -> aimindseye/rustmix-wave
  ```
- [ ] Run `git remote -v` and record the result in the SDD ledger.
- [ ] Run `git fetch upstream`.
- [ ] Record exact upstream `main` SHA used as BASE.
- [ ] Confirm `LICENSE`/MIT attribution remains intact.
- [ ] Create isolated worktree and branch:
  ```text
  codex/atlas-lite-bootstrap-01
  ```

**Acceptance:**
- `origin` and `upstream` are correct.
- Branch is not `main`.
- No Atlas code changes yet.

### Task M0.2: Install the Atlas Lite planning baseline

**Files to add from this planning bundle:**

```text
AGENTS.md
docs/ATLAS_LITE_ARCHITECTURE.md
docs/UPSTREAM.md
docs/implementation/ATLAS-LITE-01.md
docs/superpowers/specs/2026-09-03-atlas-lite-design.md
docs/superpowers/plans/2026-09-03-atlas-lite-m0-m1.md
```

- [ ] Add the planning files.
- [ ] Update the upstream README minimally so the repository identifies itself as Atlas Lite without deleting Rustmix attribution.
- [ ] Link the authoritative plan from README.
- [ ] Run `./scripts/validate.sh`.
- [ ] Commit planning/bootstrap documentation.
- [ ] Push branch.
- [ ] Open a Draft PR titled `docs: bootstrap Atlas Lite fork`.

**Acceptance:**
- Plan is versioned in `rqui/atlas-lite`.
- Upstream attribution is intact.
- Validation outcome is documented.
- Draft PR is open, not merged.

---

## M1 — Atlas shell and hardware-preserving bring-up

### Product decision

M1 must not delete working Rustmix platform modules. Hide/disable unrelated product routes first. The objective is to prove that Atlas branding and shell can sit on top of the known-good platform without destabilizing display/input/power/storage/network/audio.

### Task M1.1: Introduce Atlas product route model

**Primary files:**
- modify `src/app/router.rs`
- modify `src/app/state.rs` only where the route integration requires it
- add Atlas-specific host tests alongside routing/state

**Target routes:**

```text
Home
Library
Note
Search
Views
Capture
Settings
```

Implementation may add Atlas-prefixed route variants during transition if reusing the existing `Home`/`Settings` names would create ambiguous parent relationships.

**Required behavior:**
- Home is root.
- Library/Search/Views/Capture/Settings return to Home on hierarchical Back.
- Note returns to the surface that opened it; if the existing router only supports static parentage, model an explicit return context in Atlas state rather than adding ad-hoc hardware behavior.
- Rustmix unrelated categories are no longer reachable from normal Atlas Home navigation.

**Tests:**
- route parent/back behavior;
- long-BOOT home/back behavior remains consistent with shell conventions;
- old Rustmix product routes are not included in Atlas Home menu data.

### Task M1.2: Add Atlas Home diagnostics screen

**Primary files:**
- add `src/app/screens/atlas_home.rs`
- modify `src/app/screens/mod.rs`
- modify the existing render dispatch only where required

Initial screen is diagnostics-first:

```text
ATLAS LITE

Display       OK
Input         OK
SD            <state>
Wi-Fi         <state>
Battery       <state>
RTC           <state>
```

**Rules:**
- use existing snapshots;
- do not move hardware handles into screen code;
- use existing typography/widgets where useful;
- all display updates go through the shared refresh coordinator;
- no network call is required yet.

**Tests:**
- host render/state tests where existing screen patterns permit;
- snapshot labels do not reveal secrets.

### Task M1.3: Branding and build metadata

**Primary files:**
- update package/bin/product naming only where safe;
- update visible strings and README;
- preserve upstream authorship/license.

**Rules:**
- do not rename low-level source markers merely for aesthetics if that creates large upstream diffs;
- user-facing product says `Atlas Lite`;
- diagnostic logs may retain Rustmix upstream markers until a deliberate logging migration task.

**Validation:**
```bash
./scripts/validate.sh
```

Then build the ESP-IDF target with the repository's documented embedded build command.

**Hardware gate:**
On the physical board verify:
- boot;
- e-paper first frame;
- rotary;
- BOOT;
- power key;
- SD detection;
- RTC;
- battery snapshot;
- Wi-Fi behavior;
- sleep/wake.

Record hardware checks individually. Failed/unperformed checks remain `pending`, never inferred from build success.

**M1 exit criteria:**
- Atlas Lite screen boots on the physical board;
- platform still behaves safely;
- host validation passes;
- embedded build passes;
- hardware evidence is recorded;
- Draft PR remains unmerged.

---

## M2 — Secure provisioning and AtlasClient

### Task M2.1: Add Atlas configuration model

Create a host-testable Atlas config domain containing:

```text
atlas_url
device_id
api_token metadata/reference
```

Target persistence must use NVS on ESP-IDF.

Move final Wi-Fi provisioning away from plaintext SD as part of this milestone. If Rustmix SD Wi-Fi is temporarily kept for bring-up, document it as development-only and do not store the Atlas token there.

**Tests:**
- URL validation;
- redacted Debug/display;
- missing/invalid config states;
- no serialization path writes API token to SD.

### Task M2.2: Add narrow Atlas DTOs

Define only fields required by Atlas Lite.

Minimum response models:

- note summary page;
- note document;
- search response;
- View summaries;
- View result page;
- canonical API error.

Do not retain:
- note `frontmatter`;
- search `score` unless UI needs it;
- View arbitrary `values` in the first list UI.

Use a bounded JSON parser/deserializer strategy compatible with ESP32.

**Tests:**
- parse representative current Atlas payloads;
- ignore unneeded fields;
- reject malformed required fields;
- reject over-budget bodies before parse.

### Task M2.3: Add transport-independent AtlasClient interface

Expose operations equivalent to:

```text
list_notes(cursor, limit)
get_note(id)
search(query, limit, offset)
list_views()
get_view_results(id, cursor, limit)
capture_text(request, idempotency_key)
```

Keep UI independent from ESP-IDF HTTP handles.

### Task M2.4: Add ESP-IDF HTTPS transport

Follow Rustmix's bounded HTTPS worker pattern.

Required:
- TLS certificate bundle validation;
- explicit timeout;
- response-size cap;
- `Authorization: Bearer <token>`;
- optional `Idempotency-Key`;
- classified retryable/non-retryable errors;
- no token/header logging;
- bounded retries, not infinite loops.

**Acceptance:**
A real board can authenticate to current Atlas REST and perform a safe read operation using a minimum-scope key.

---

## M3 — Home, Library and Note

### Task M3.1: Atlas Home data model

Initial Home derives from existing REST without a device façade.

Use a bounded recent/list page and bounded View summaries.

Show:
- connection/offline status;
- battery/Wi-Fi/time;
- recent notes;
- Views;
- Capture action.

Do not add a server aggregation endpoint unless profiling demonstrates need.

### Task M3.2: Library hierarchy

Build bounded hierarchy from current note summaries:

```text
id
parentId
order
title
```

Rules:
- IDs, not paths, are structural identity;
- sort siblings by Atlas order semantics;
- handle missing parent/cycle/invalid state without panic;
- pagination must not imply that an incomplete page is the whole Vault.

If a usable bounded hierarchy cannot be represented correctly without walking too many pages, measure it. That measurement is one possible justification for a server-side hierarchy endpoint/device façade later.

### Task M3.3: Note reader

Fetch `/api/v1/notes/by-id/:id`.

Render Markdown subset:

- headings;
- paragraphs;
- unordered/ordered lists;
- checkboxes;
- bold/italic if practical with current typography;
- links/wikilinks as readable text;
- separators.

Reuse Rustmix reader typography/pagination concepts where useful.

No arbitrary HTML/JS/embeds in M3.

Persist a bounded opened-note cache.

---

## M4 — Search and Views

### Task M4.1: Search input

Reuse `KeyboardGridNavigation`.

Search flow:

```text
input -> GET /api/v1/search -> bounded hits -> open Note
```

Handle:
- empty query;
- 503 index not ready + Retry-After;
- offline cached last result when available;
- malformed/oversized response.

### Task M4.2: Views

Use:
- `GET /api/v1/views`
- `GET /api/v1/views/:id/results`

Initial result UI uses:
- id;
- title;
- path/state as needed;
- pagination cursor.

Ignore arbitrary `values` until a later feature needs view columns.

---

## M5 — Bounded SD cache and idempotent queue

### Task M5.1: Atlas storage root and atomic write helper

Add:

```text
/ATLAS/CACHE
/ATLAS/QUEUE
/ATLAS/AUDIO
/ATLAS/ASSETS
/ATLAS/LOGS
```

Reuse Rustmix FAT-safe `.TMP` / `.BAK` recovery concepts.

Implement:
- root confinement;
- per-file and total cache budget;
- safe replacement;
- corruption fallback;
- eviction policy.

### Task M5.2: Cache repository

Cache:
- Home/list snapshot;
- opened notes;
- recent View pages;
- optional recent search pages.

Each cache record includes:
- schema version;
- source revision/timestamp where available;
- payload;
- last-used metadata sufficient for eviction.

### Task M5.3: Durable pending queue

Each mutation record is created with a persistent idempotency key before first send.

State machine:

```text
pending -> sending -> acknowledged
             |
             +-> pending retry
```

After reset, `sending` safely returns to retry using the same key.

Do not delete a queue item until Atlas acknowledges success or an explicitly terminal policy is applied.

**Tests:**
- reboot simulation;
- lost response after successful server commit;
- retry with same key;
- corrupt queue item isolation;
- queue budget.

---

## M6 — Text Capture

### Task M6.1: Capture UI

Provide a fast route from Home.

Reuse the keyboard-grid input model.

### Task M6.2: Online canonical capture

Send:

```text
POST /api/v1/capture/text
Authorization: Bearer ...
Idempotency-Key: ...
```

Use current `capture:write` capability.

### Task M6.3: Offline capture

Queue first, then attempt network send. This ordering ensures the idempotency key survives a reset during transport.

**Acceptance:**
- successful capture appears once in Atlas;
- offline capture survives reboot;
- reconnect retry does not duplicate it.

---

## M7 — Voice Capture

### Task M7.1: Reuse Rustmix recording boundary

Reuse:
- PCM16;
- mono;
- 16 kHz;
- streamed WAV finalization;
- safe temporary file behavior;
- mic gain handling where useful.

Atlas audio root is `/ATLAS/AUDIO`.

### Task M7.2: Preserve audio before STT

Before building STT, verify that recording/recovery works under Atlas Lite.

Optionally use Atlas's existing file-capture path to preserve a WAV in Atlas if product behavior is acceptable.

### Task M7.3: Add server-side STT only as a separate Atlas PR

First inspect current Atlas architecture again.

If no canonical audio-to-capture operation exists, add the smallest server feature that:

1. authenticates `capture:write`;
2. enforces audio size/time/MIME limits;
3. invokes an injected STT provider abstraction;
4. takes transcript text into canonical Capture;
5. supports idempotency;
6. never exposes provider secrets to firmware.

Do not hard-code OpenAI or another provider into ESP32.

The exact route is chosen in the Atlas Server design task; `/api/v1/capture/audio` is preferred over a generic `/device/*` namespace if the operation is broadly meaningful.

---

## M8 — Power, pairing, OTA and polish

### Task M8.1: Measure power lifecycle

Profile:
- boot;
- Wi-Fi connect;
- one Home sync;
- active reading;
- idle;
- deep sleep.

Use measurements to tune connection lifetime and sync cadence.

### Task M8.2: Pairing

Design Atlas Web device pairing so the user does not type an `at_v1` token with the rotary encoder.

Target:

```text
device_id + short code
 -> Atlas Web Settings > Devices
 -> user approval
 -> minimum-scope device credential
 -> NVS
```

This is an Atlas Server/Web feature with its own PR.

### Task M8.3: OTA

Adopt an ESP-IDF-compatible OTA partition/update design with:
- authenticated/verified source;
- version metadata;
- rollback;
- no arbitrary firmware URL.

Do not block core MVP on OTA unless the required partitioning change must be made before field use.

### Task M8.4: Prune dead Rustmix product code

Only now measure binary/source impact and prune unrelated product modules in a dedicated reviewable change.

Preserve platform modules still shared by Atlas.

---

# Milestone Definition of Done

The Atlas Lite MVP is complete only when the physical device can:

1. boot Atlas Lite;
2. connect to Wi-Fi without relying on plaintext product secrets on SD;
3. authenticate to Atlas with a limited API key;
4. render Home;
5. navigate Library;
6. open/read a note;
7. search notes;
8. open Views;
9. create text Capture;
10. read cached content offline;
11. queue a capture offline;
12. retry without duplication;
13. recover if Atlas is unavailable;
14. sleep/wake with usable retained display behavior;
15. show battery/Wi-Fi/sync status.

Voice/STT is an additional milestone and may ship after this baseline if its server dependency is not ready.

# Reporting contract per milestone

Return:

```text
Repository:
Upstream:
Origin:

BASE:
HEAD:
Branch:

Commits:
Draft PR:

Host validation:
Embedded build:

Hardware verified:
Hardware pending:

Files/modules added:
Files/modules modified:

Known issues:
Rulings/deviations from plan:
Next milestone:
```

Do not mark pending hardware checks as passed.
