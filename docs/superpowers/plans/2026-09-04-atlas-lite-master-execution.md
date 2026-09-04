# Atlas Lite — Detailed Execution Plan M1.5–M8

> **Status:** authoritative execution plan for all work after M1.
>
> **Controller:** Luna Max.
>
> **Required workflow:** `superpowers:subagent-driven-development`.
>
> **Repository:** `rqui/atlas-lite`.
>
> **Server repository:** `rqui/atlas`.
>
> **Hardware upstream:** `aimindseye/rustmix-wave`.

This document contains the implementation detail that should **not** need to be repeated in chat prompts. Future prompts should only identify the milestone to execute, the branch/worktree boundary, and any user-specific stop condition.

Read together with:

1. `AGENTS.md`
2. `docs/implementation/ATLAS-LITE-01.md`
3. `docs/superpowers/specs/2026-09-03-atlas-lite-design.md`
4. `docs/ATLAS_LITE_ARCHITECTURE.md`
5. `docs/UPSTREAM.md`
6. current source/tests in this repository
7. current `master` of `rqui/atlas` when server/API behavior matters

If this plan conflicts with the product spec or current code, the spec and current behavior win. Record the ruling in the local SDD ledger and continue unless a genuine Superpowers stop condition applies.

---

# 0. Permanent execution policy

## 0.1 Git/worktree policy

Never implement on `main`/`master`.

Every milestone uses an isolated worktree and milestone branch.

Expected repository remotes:

```text
origin   -> rqui/atlas-lite
upstream -> aimindseye/rustmix-wave
```

Before each milestone:

```bash
git fetch origin
git fetch upstream
git status --short
git branch --show-current
```

Record:

```bash
git rev-parse HEAD
git rev-parse upstream/main
```

Use Draft PRs. Do not merge, deploy, publish releases, rewrite shared history, or modify `rqui/atlas` outside a separate worktree/branch/PR unless explicitly authorized.

Stacked PRs are allowed when the preceding milestone remains Draft. The PR base must be the branch it truly depends on so the diff contains only the new milestone.

## 0.2 Cost/model policy

Subagents are required where they add value, but model use must be deliberate.

### LOW / cheap model, low reasoning

Use for:

- documentation updates;
- fixtures and representative payloads;
- straightforward tests;
- scripts;
- mocks/fakes;
- mechanical refactors;
- symbol renames;
- formatting/build cleanup;
- one- or two-file changes with explicit acceptance criteria.

### MEDIUM / standard model, medium reasoning

Use for:

- Rust integration across several files;
- state/router changes;
- simulator architecture implementation after the architecture has been decided;
- DTO/parsing work;
- NVS abstraction;
- AtlasClient;
- network transport;
- cache/queue implementation;
- ordinary debugging;
- meaningful task-scoped reviews.

### HIGH / Max model, high reasoning

Reserve for:

1. one milestone preflight when architectural judgment is genuinely required;
2. one blocker that cannot be resolved from the spec/code after normal analysis;
3. final whole-branch review.

Normal budget per milestone:

```text
HIGH/MAX calls: target <= 2
```

A third HIGH/MAX call requires a recorded reason in the SDD ledger.

Do not use Max for mechanical edits, obvious tests, Markdown, fixtures, formatting, or routine review fixes.

## 0.3 Subagent concurrency

Maximum normal concurrency:

```text
2 read-only scouts
1 implementation agent
1 reviewer for the completed task
```

Read-only scouts may run in parallel when independent.

Never run multiple implementers against the same worktree concurrently.

Implementers must not create their own subagents.

Do not duplicate task reviews.

## 0.4 Review/fix loops

For material implementation tasks:

```text
implement -> tests -> commit -> task review -> fix -> scoped re-review
```

Normal maximum for the same finding:

```text
2 fix/re-review rounds
```

If the same issue remains after two rounds, the controller must diagnose the underlying blocker before spending more model calls. Escalate model capability only when the problem actually requires more judgment.

For trivial/mechanical work, a full implementer/reviewer pair is not mandatory. The controller may perform the change directly if the task is low-risk and tests clearly prove it.

Always run one broad final review for a milestone.

## 0.5 Context discipline

Do not paste complete specs into every subagent prompt.

Create/use focused task briefs containing only:

- task requirements;
- files/interfaces involved;
- decisions already made;
- acceptance tests;
- relevant global constraints.

Agents should not reread the entire project history.

Local execution reports/ledgers belong under `.superpowers/sdd/` and remain gitignored. Durable product decisions and evidence belong under `docs/`.

## 0.6 Verification language

Keep these evidence classes separate:

```text
HOST TESTED
TARGET BUILD TESTED
SIMULATOR TESTED
QEMU TESTED
HARDWARE TESTED
```

Never translate one category into another.

A successful Xtensa build does not verify e-paper, PMIC, SD, RTC, Wi-Fi radio behavior, sleep current, microphone, speaker, or physical buttons.

---

# M1.5 — Native Simulation Harness

## Goal

Create a native host simulator that reuses the real Atlas Lite application state, router, renderers, framebuffer, typography/layout, and navigation logic. The simulator exists to make M2–M6 development fast and testable without pretending to emulate the Waveshare hardware electrically.

The simulator must **not** become a second independently implemented UI.

Target architecture:

```text
                  Atlas Lite application/core
                           |
                +----------+----------+
                |                     |
             ESP32-S3                 host
                |                     |
        Waveshare hardware      Atlas Lite Simulator
                                      |
                               framebuffer/window
                               mock hardware
                               mock Atlas transport
```

## M1.5.1 Preflight

Use at most two read-only scouts:

### Scout A — current Atlas Lite/Rustmix hostability

Inspect:

- framebuffer and orientation;
- `AppState`;
- router/navigation;
- current Atlas shell/Home renderers;
- input abstractions;
- host-test patterns;
- `cfg(target_os = "espidf")` boundaries;
- dependencies that prevent host linking;
- existing ways to render/capture framebuffer content.

Return the smallest architectural seam that permits host simulation while keeping firmware dependencies clean.

### Scout B — simulator technology choice

Compare only lightweight host options compatible with macOS and Rust. Prefer a simple framebuffer window/event loop over a large UI framework.

Criteria:

- host-only dependency;
- small maintenance surface;
- deterministic framebuffer capture;
- keyboard input;
- no dependency leakage into ESP32 firmware;
- CI/headless tests possible even when interactive window tests are not.

The controller makes the final choice. Do not over-research.

## M1.5.2 Host-only structure

Choose the structure that best matches the current Cargo layout. Preferred outcomes include a host-only binary or tool package such as:

```text
tools/atlas-lite-sim/
```

or an equivalent target-gated binary.

Requirements:

- simulator GUI/window dependencies are host-only;
- ESP32 build graph remains clean;
- shared application modules are reused, not copied;
- renderer code remains product code, not simulator code;
- simulator mocks implement interfaces/snapshots consumed by the same application layer.

Add a simple launch path, preferably:

```bash
./scripts/sim.sh
```

The exact Cargo command may be documented as well.

## M1.5.3 Display simulation

The simulator presents the logical Atlas Lite canvas currently used by the product, expected to remain `480 x 800` unless current code proves otherwise.

Reuse the actual framebuffer and renderer path.

Interactive display requirements:

- nearest-neighbor or otherwise crisp monochrome scaling;
- optional larger window size without changing logical geometry;
- no animations required;
- redraw only after input/state changes;
- deterministic render path for tests.

Do not pretend to simulate physical e-paper ghosting or refresh timing unless a later test model explicitly requires it.

## M1.5.4 Input simulation

Map keyboard events to semantic input, not directly to arbitrary screen behavior.

Minimum mapping:

```text
Up / Down          rotary previous/next
Enter              select
Esc                hierarchical back
H or Home          home, when product semantics support it
P                  simulated power event
```

If current Rustmix semantics use horizontal/vertical navigation modes for keyboard grids, expose equivalent keys in the simulator when those screens arrive.

Tests must prove semantic navigation independent of the physical key codes.

## M1.5.5 Simulated hardware snapshots

Provide explicit fake/simulated state for:

```text
Display
Input
SD
Wi-Fi
Battery
RTC
Atlas connection
```

Minimum controllable states:

```text
Wi-Fi:
  connected
  connecting
  offline
  failed

Battery:
  100%
  50%
  10%

SD:
  mounted
  missing
  error

Atlas:
  unconfigured
  connecting
  connected
  unauthorized
  forbidden
  timeout
  server_error
  offline
```

The simulator must not instantiate ESP-IDF hardware drivers.

## M1.5.6 Deterministic render/snapshot tests

Create a headless render path capable of producing deterministic framebuffer outputs for at least:

- Home;
- Library placeholder;
- Search placeholder;
- Views placeholder;
- Capture placeholder;
- Settings placeholder.

Tests should cover, where practical:

- geometry stays inside logical bounds;
- selected rows have distinguishable ink from unselected rows;
- navigation reaches intended routes;
- Back returns correctly;
- state labels are deterministic;
- secret values cannot render into diagnostics;
- repeated rendering of the same state is byte-identical.

PNG export is useful but not required if a deterministic framebuffer/hash fixture is simpler and more stable. Avoid brittle pixel snapshots for content that does not need pixel-perfect contracts.

## M1.5.7 QEMU spike

Perform a strictly bounded spike for ESP32-S3 QEMU.

Value targets:

```text
boot/startup
panic detection
basic flash/partition behavior
NVS feasibility
memory/startup diagnostics
```

Do not block M1.5 on QEMU.

If running the current Rust/ESP-IDF ELF requires invasive board-peripheral emulation, record `DEFERRED` in `docs/SIMULATION.md` and stop the spike.

Do not build custom SSD1677/PMIC/audio models as part of this milestone.

## M1.5.8 Wokwi policy

Document Wokwi/Custom Chips as a future possibility only.

Do not implement a custom SSD1677 chip in M1.5.

The native host simulator is the primary development environment because it can reuse application code with much less peripheral-emulation effort.

## M1.5.9 Documentation

Create/update:

```text
docs/SIMULATION.md
```

Document:

- what the simulator proves;
- what it does not prove;
- launch command;
- keyboard controls;
- fake hardware states;
- headless tests;
- QEMU status;
- Wokwi status;
- distinction from hardware verification.

## M1.5.10 Validation

Required:

```bash
./scripts/validate.sh
./scripts/build.sh
git diff --check
```

Also run simulator-focused tests and launch smoke where possible.

Exit criteria:

- native simulator exists;
- real renderers/framebuffer are reused;
- input semantics are simulated;
- hardware snapshots are controllable;
- deterministic host tests exist;
- firmware build still passes;
- physical drivers were not altered merely to support simulation.

---

# M2 — Secure Provisioning + AtlasClient

## Goal

Add the secure configuration, bounded Atlas protocol layer, host/simulator mocks, and ESP-IDF HTTPS transport required for later product screens.

M2 establishes infrastructure; it does not need to finish Library/Search/Views UI.

## M2.1 Re-audit current Atlas contracts

Before coding, inspect current `master` of `rqui/atlas`.

Confirm current routes, methods, DTOs, error codes, idempotency requirements and capability names for:

```text
GET  /api/v1/notes
GET  /api/v1/notes/by-id/:id
GET  /api/v1/search
GET  /api/v1/views
GET  /api/v1/views/:id/results
POST /api/v1/capture/text
```

Confirm device minimum scopes. Expected:

```text
notes:read
search:read
views:read
capture:write
```

Do not assume names if Atlas changed.

Record only relevant contract drift; do not perform broad Atlas refactoring.

## M2.2 Configuration domain

Create a host-testable configuration domain separating values from persistence.

Logical configuration includes:

```text
device_id
atlas_url
api_token
wifi_ssid
wifi_credentials
```

Requirements:

- URL validation;
- explicit unconfigured/partial/ready states;
- redacted `Debug`/display behavior;
- no accidental secret serialization into generic state dumps;
- no secrets written to SD;
- no secrets logged;
- easy fake persistence for simulator/tests.

Prefer interfaces such as:

```text
ConfigRepository / SecretStore
```

or equivalent names fitting current architecture.

## M2.3 NVS persistence

Target persistence on ESP-IDF uses NVS or an equivalent ESP32 internal persistent store.

Suggested namespace content:

```text
atlas.device_id
atlas.url
atlas.token
wifi.ssid
wifi.credentials
```

Exact naming may follow ESP-IDF constraints.

Requirements:

- bounded key/value sizes;
- explicit missing/corrupt errors;
- update/clear operations;
- secret-aware logs;
- factory-reset support path prepared for later Settings;
- migration/version hook if format may evolve.

Host tests use fake/in-memory storage.

## M2.4 Development provisioning

Provide a development-friendly way to populate configuration without recompiling firmware.

Preferred first milestone flow is USB/serial or a host provisioning helper.

Conceptual UX:

```text
Atlas Lite setup

Wi-Fi SSID:
Wi-Fi Password:
Atlas URL:
Atlas API Token:

write -> device NVS
```

Security requirements:

- do not commit secrets;
- avoid placing token/password in shell history where practical;
- do not echo secrets;
- do not write secrets to SD;
- do not log secret values;
- document how to clear credentials.

If physical NVS writes cannot be verified without the board, split evidence into host-tested parsing/protocol, target-build-tested adapter, and hardware pending.

Captive portal and Atlas pairing are **not** part of M2. They belong to M8.

## M2.5 Narrow device DTOs

Create Rust models containing only fields required by Atlas Lite.

### Note summary

Baseline fields:

```text
id
path only if needed
title
state
revision
parent_id
order
icon only if actually rendered
updated only if actually rendered
```

### Note document

```text
id
title
revision
body
parent_id
```

Do not retain arbitrary `frontmatter` in M2.

### Search hit

```text
atlas_id/id
title
path if needed
snippet
revision
state if needed
```

Do not retain `score` unless product behavior uses it.

### View summary

```text
id
name
revision
status
layout
```

### View result

```text
id
title
path/state if needed
revision
```

Do not retain arbitrary View `values`, relation maps or rollup maps in the initial result list.

### Error DTO

Support Atlas canonical error body fields needed for diagnostics/routing without arbitrary maps unless explicitly bounded.

## M2.6 Bounded JSON policy

Response byte limit must be checked **before** normal parsing.

Avoid unrestricted generic JSON trees in ESP32 normal flows.

Unknown fields should generally be ignored by narrow DTO parsing while required fields are validated.

Tests:

```text
valid representative payload
extra ignored fields
missing required field
malformed JSON
oversized body
unexpected type
canonical Atlas error body
UTF-8 edge cases relevant to titles/snippets
```

Fixtures should be representative of current Atlas contracts, not invented fantasy payloads.

## M2.7 AtlasClient abstraction

Define a transport-independent interface supporting:

```text
list_notes(cursor, limit)
get_note(id)
search(query, limit, offset)
list_views()
get_view_results(id, cursor, limit)
capture_text(request, idempotency_key)
```

Exact signatures may use typed request/response structs.

UI/state code should depend on AtlasClient/application operations, not ESP-IDF HTTP handles.

Create `MockAtlasTransport` or equivalent for simulator/tests.

Mocks must support success and meaningful failure modes:

```text
401 unauthorized
403 forbidden
404 not found
429 rate limited
503 unavailable/index not ready
timeout
transport offline
malformed payload
oversized payload
```

## M2.8 ESP-IDF HTTPS transport

Follow Rustmix's established bounded HTTPS worker pattern where appropriate.

Requirements:

- TLS certificate validation;
- ESP certificate bundle or approved trust mechanism;
- explicit request timeout;
- bounded response body;
- bounded retries/backoff;
- new/clean connection after poisoned transport failure when appropriate;
- Authorization header:

```http
Authorization: Bearer at_v1...
```

- optional mutation header:

```http
Idempotency-Key: ...
```

- URL/query encoding;
- HTTP status classification;
- no Authorization/token/password logs;
- reasonable worker stack/memory budget documented if a dedicated worker is used.

Retry automatically only where semantics are safe. Mutation retries must preserve the same idempotency key.

## M2.9 Simulator integration

The host simulator must be able to switch between mock Atlas states without hardware:

```text
unconfigured
connecting
connected
unauthorized
forbidden
timeout
server error
offline
```

M2 may expose these as diagnostics/status only; full Library/Search/Views screens arrive later.

## M2.10 Device API prohibition

Do not add `/api/v1/device/*` in M2.

A device façade requires measured evidence such as:

```text
unsafe memory peak
unacceptable response bytes
excessive request count
material latency
material battery/energy cost
```

Convenience is not sufficient justification.

## M2.11 CI

If inexpensive and maintainable, ensure GitHub Actions validates at least:

```text
format
source contracts
host tests
simulator/headless tests
```

ESP-IDF CI build is optional if it significantly increases setup/runtime cost; local Xtensa build remains mandatory.

## M2.12 Validation/exit

Required:

```bash
./scripts/validate.sh
./scripts/build.sh
git diff --check
```

Exit criteria:

- host config model passes;
- NVS adapter compiles for target;
- dev provisioning flow exists or its hardware-dependent final write is explicitly pending;
- minimum scopes confirmed against current Atlas;
- narrow DTOs tested;
- AtlasClient tested with mock transport;
- HTTPS target adapter builds;
- simulator can exercise Atlas connection/error states;
- no secret leaks found.

---

# M3 — Home + Library + Note Reader

## Goal

Turn the M1 shell into a useful read client using the M2 AtlasClient.

## M3.1 Home data

Populate Home from current REST without a new device API.

Show bounded data such as:

```text
status/time/battery/Wi-Fi
recent notes
Views shortcuts
Capture action
```

Keep request count and payload size measurable.

Do not add continuous polling. Refresh on explicit entry/wake/user action or another justified low-power trigger.

Simulator fixtures must cover empty, normal, long-title, offline-cache and error states.

## M3.2 Library hierarchy

Use note summaries and Atlas structural identity:

```text
id
parentId
order
```

Rules:

- IDs, not paths, determine structure;
- sibling order follows Atlas order semantics;
- invalid/missing parent/cycle states must not panic;
- incomplete pagination must not be treated as a complete Vault tree;
- memory use is bounded.

If correct hierarchy requires too many pages/requests, profile and document. Only measured evidence can justify a server hierarchy/device endpoint.

## M3.3 Note loading

Use `GET /api/v1/notes/by-id/:id`.

Show loading/error/offline-cached states without blanking the whole screen unnecessarily.

Persist bounded recently opened notes for M5 cache integration; before M5 this may be an in-memory seam with storage interface prepared.

## M3.4 Markdown subset

Render at minimum:

```text
H1-H3
paragraphs
unordered lists
ordered lists
checkboxes
basic emphasis if current fonts permit
wikilinks/links as readable text
horizontal separators/simple blocks
```

No arbitrary HTML/JS.

Complex embeds are not rendered in M3.

Reuse Rustmix reader pagination/typography where it reduces risk.

Tests cover:

- long paragraphs;
- Unicode;
- empty body;
- headings/lists/checks;
- page boundaries;
- malformed/unclosed lightweight Markdown constructs;
- content too large for one in-memory parse strategy.

## M3.5 Navigation

A Note returns to the surface that opened it (Library/Home/Search/View).

Preserve origin context explicitly; do not encode a false static parent.

## M3 exit

Simulator can navigate Home -> Library -> Note using mock Atlas fixtures, and target build passes.

---

# M4 — Search + Views

## Goal

Expose Atlas's authoritative search and View projections without building local equivalents.

## M4.1 Search input

Reuse the shared keyboard-grid navigation where practical.

Flow:

```text
query -> Atlas search -> bounded hits -> Note
```

Handle:

- empty query;
- long query limit;
- Unicode;
- no results;
- index not ready / 503;
- retryable server failure;
- offline last-result cache when M5 is present.

Do not build a full local Vault index.

## M4.2 Search results

Render title/snippet/state only as needed.

Avoid excessive snippet allocation. Bound result count and snippet length.

## M4.3 Views list

Use existing View summaries.

Show stable, useful metadata only.

## M4.4 View results

Use paginated View results.

Initial UI is list-first and opens notes.

Do not implement arbitrary desktop View layouts on e-paper in M4. Board/calendar/cards may eventually get device-specific representations, but only after base usefulness is proven.

Ignore arbitrary View values unless a later requirement explicitly needs columns.

## M4 exit

Simulator and host tests cover Search/Views success/errors/pagination. Xtensa build passes.

---

# M5 — Offline Cache + Durable Queue

## Goal

Make Atlas Lite useful during network/NAS outages without turning it into a Vault replica.

## M5.1 SD layout

Use:

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

No secrets on SD.

## M5.2 Safe FAT writes

Reuse Rustmix `.TMP` / `.BAK` recovery patterns where appropriate.

Requirements:

- root confinement;
- no path traversal;
- atomic/recovery-safe replacement;
- corruption isolation;
- interrupted write recovery;
- bounded file sizes;
- bounded total cache size.

## M5.3 Cache schema

Cache only useful bounded snapshots:

```text
Home/recent summaries
opened note bodies
recent View pages
optional recent search pages
```

Each record should carry:

```text
schema version
source revision/timestamp where available
last used / cache metadata
payload
```

Implement deterministic eviction, preferably LRU-like or another simple bounded policy.

## M5.4 Offline state

Product states include:

```text
ONLINE
SYNCING
OFFLINE_CACHED
OFFLINE_NO_DATA
ERROR
```

Show stale/offline indicators without intrusive redraw loops.

## M5.5 Durable mutation queue

Every queued mutation receives a persistent idempotency key **before first network attempt**.

State model:

```text
pending -> sending -> acknowledged
             |
             +-> pending retry
```

After reboot, an uncertain `sending` item retries using the same idempotency key.

Delete only after authoritative success/explicit terminal policy.

Tests:

- reboot during send;
- request reached server but response was lost;
- retry uses same key;
- duplicate prevention;
- corrupt queue record;
- queue full/budget reached;
- SD unavailable.

## M5 exit

Offline cached reads and durable retry infrastructure are proven on host/simulator. Physical SD behavior remains separate hardware evidence.

---

# M6 — Text Capture

## Goal

Provide a fast, resilient Atlas Capture flow online and offline.

## M6.1 UI

Capture should be reachable quickly from Home.

Reuse keyboard grid/text-entry infrastructure.

Keep editor scope intentionally small: text capture, not a full note editor.

## M6.2 Queue-first mutation

Generate and persist idempotency key/queue record before attempting network delivery.

Online path:

```text
queue record
 -> POST /api/v1/capture/text
 -> Atlas success
 -> remove queue record
```

Offline path:

```text
queue record
 -> offline
 -> retain
 -> reconnect
 -> same idempotency key
 -> Atlas success
 -> remove
```

## M6.3 Feedback

Show concise states:

```text
Saved
Queued offline
Sending
Failed — will retry
Authentication required
```

Avoid continuous polling/redraw.

## M6 exit

Host/simulator proves no duplicate capture under lost-response/reboot scenarios. Real server integration is exercised where credentials/environment permit; hardware remains separately labeled.

---

# M7 — Voice Capture

## Goal

Reuse Rustmix audio recording to capture voice safely, then connect it to server-side transcription without putting provider secrets on the device.

## M7.1 Recorder reuse

Reuse existing Rustmix Voice Notes boundaries:

```text
PCM16
mono
16 kHz
streamed WAV
safe temporary/finalization behavior
mic gain handling where useful
```

Store Atlas voice data under:

```text
/ATLAS/AUDIO/
```

Do not rewrite codec/I2S drivers unless a verified Atlas-specific requirement forces it.

## M7.2 Local preservation first

Before STT integration, prove safe record/finalize/recovery semantics.

Handle:

- recording interrupted by reset;
- SD full/missing;
- cancel/delete;
- duration/size bounds.

## M7.3 Server STT boundary

STT belongs in Atlas Server through an injected provider abstraction.

No OpenAI/other provider API key in firmware.

First re-audit current Atlas before adding anything.

If a new operation is required, prefer a generally meaningful capture route such as:

```text
POST /api/v1/capture/audio
```

rather than a generic `/device/*` namespace, provided current Atlas architecture supports that cleanly.

Server requirements:

- `capture:write` or a specifically justified capability;
- authenticated request;
- MIME/duration/size limits;
- idempotency;
- provider abstraction;
- transcript -> canonical Capture service;
- timeout/error handling;
- no provider secret exposure.

Any Atlas Server change occurs in a separate worktree/branch/Draft PR.

## M7.4 Device upload

Use bounded streaming/chunking appropriate for ESP32 memory.

Do not load unbounded WAV into RAM.

Classify upload/transcription failures separately from local recording failures.

## M7 exit

Local audio safety and server contract are tested; physical mic/speaker/codec behavior must be verified on hardware before being marked PASS.

---

# M8 — Productization: Power, Pairing, OTA, Release, Cleanup

## Goal

Turn the working client into a device that can be provisioned and maintained without developer tools.

## M8.1 Power profiling

Measure on physical hardware:

```text
boot
Wi-Fi connect
one Home sync
active reading
idle
sleep
deep sleep
wake
```

Record actual current/energy measurements where possible.

Tune:

- Wi-Fi connection lifetime;
- reconnect/backoff;
- sync triggers;
- sleep timeout;
- partial/full refresh cadence;
- worker lifetime;
- cache use after wake.

Do not optimize based only on assumptions.

## M8.2 Product provisioning portal

Replace developer provisioning for normal users with a temporary setup AP/captive portal or similarly simple local flow.

Target first-boot flow:

```text
flash/start
 -> temporary Atlas Lite AP
 -> phone connects
 -> local setup page
 -> Wi-Fi SSID/password + Atlas URL
 -> save to NVS
 -> reboot/connect
```

Do not ask normal users to paste API tokens.

The setup portal must be temporary, local, bounded and shut down after provisioning.

## M8.3 Atlas device pairing

Target product flow:

```text
Atlas Lite generates device_id + short pairing code
 -> user opens Atlas Web Settings > Devices
 -> enters/confirms code
 -> Atlas shows requested capabilities
 -> user authorizes
 -> Atlas issues minimum-scope device credential
 -> device receives it
 -> token stored in NVS
```

Requested capabilities should remain the minimum needed for shipped features.

Design requirements:

- short-lived pairing code;
- single-use authorization;
- rate limiting;
- explicit user approval;
- revocable device credential;
- device listing/name/last-seen metadata where appropriate;
- no token shown/logged unnecessarily;
- unpair/revoke semantics.

This requires a separate Atlas Server/Web plan and PR.

## M8.4 Device Settings

Product Settings should eventually expose:

```text
Atlas server / connection state
Wi-Fi state / RSSI
Sync / last successful sync
Device ID/name
Firmware version/build
Battery
Storage
Restart
Sleep
Reset Wi-Fi
Unpair Atlas
Factory reset
```

Factory reset clears Atlas Lite NVS namespace and other documented local product state without touching Atlas Server data.

## M8.5 OTA

Use an ESP-IDF-compatible OTA design.

Requirements:

- fixed trusted update source or authenticated manifest;
- integrity/signature verification as appropriate;
- version metadata;
- rollback partition strategy;
- interrupted-update recovery;
- no arbitrary firmware URL entry;
- no update secret embedded unnecessarily.

Partitioning decisions that affect future OTA must be made before public field releases.

## M8.6 Release artifacts

Continue supporting the validated ELF flow.

After physical validation of bootloader/partition/application offsets, add a merged/factory image workflow if useful:

```text
atlas-lite-vX.Y.Z.elf
atlas-lite-vX.Y.Z.bin
SHA256SUMS
```

Never create a one-file `.bin` by guessing offsets.

Document exact flash commands and recovery path.

## M8.7 Prune old Rustmix product code

Only after Atlas Lite is stable, measure and remove unrelated product modules in a dedicated cleanup diff.

Candidates include old games/weather/calendar/dictionary/Lua/product screens, but preserve any shared infrastructure still used by Atlas.

Compare:

```text
binary size
build time
source complexity
upstream merge cost
```

Do not prune merely for aesthetics.

## M8 exit

A normal user can flash/provision/pair/use/update/reset the device without developer secrets or recompilation, and power/release behavior has physical evidence.

---

# Cross-cutting UI rules

All milestones follow these e-paper rules:

- monochrome/high contrast;
- readable type;
- obvious current selection;
- no FPS-driven animation;
- avoid flashing;
- page-based navigation where preferable to continuous scrolling;
- redraw only on meaningful state changes;
- use partial refresh when safe;
- periodic/global refresh only for ghost cleanup/known panel needs;
- preserve useful display image during sleep;
- never bypass the shared refresh coordinator.

The product should visually read as Atlas Lite, while upstream legal attribution remains in documentation/about metadata.

---

# Cross-cutting reliability rules

Atlas Lite must recover without boot loops from:

```text
Atlas Server down
NAS down
Wi-Fi unavailable
DNS failure
TLS/transport timeout
401/403 revoked credential
429/503 temporary server issue
malformed response
oversized response
SD missing
SD corruption
cache corruption
queue corruption
low battery
sleep/reset during mutation
```

Use last-known-good state when safe.

No infinite retries.

No unbounded allocations/responses/audio/results.

---

# Cross-cutting security rules

Never:

- use an admin token on device;
- write API token/Wi-Fi password to SD;
- log Authorization headers or secrets;
- bypass Atlas canonical services into SQLite/Vault filesystem;
- expose arbitrary filesystem access;
- expose arbitrary URL fetching from firmware;
- execute arbitrary MCP tools from the device;
- embed AI provider secrets in firmware;
- accept arbitrary firmware update URLs;
- allow unbounded body/audio/result sizes.

REST is the device protocol. MCP remains an agent integration surface unless a future product requirement explicitly proves otherwise.

---

# Cross-cutting test strategy

## Host tests

Prefer pure/host-testable code for:

- routing/state;
- DTO parsing;
- URL/config validation;
- secret redaction;
- response bounds;
- Markdown parsing/pagination;
- hierarchy construction;
- cache schema/eviction;
- queue/idempotency;
- retry classification;
- offline transitions;
- simulator state/rendering.

## Target build

Whenever ESP-IDF/target-side wiring changes:

```bash
./scripts/build.sh
```

The artifact must be confirmed as Xtensa ESP32-S3 output, not merely a host build.

## Simulator

Use simulator for UI/application integration, not electrical claims.

## Hardware

Physical test matrix remains required for:

- panel refresh/ghosting;
- rotary/buttons/power key;
- SDMMC;
- RTC;
- PMIC/battery/charge;
- Wi-Fi radio stability;
- sleep/wake/current;
- microphone/speaker/audio;
- real flash/recovery behavior.

---

# CI policy

Prefer CI that is fast enough to run on every PR:

```text
format
source contracts
host tests
simulator/headless tests
```

Add full ESP-IDF CI only when setup/runtime is reliable and proportionate. A local target build remains mandatory for target-side changes even if hosted CI is absent.

---

# Server-change gate

Do not modify `rqui/atlas` for convenience.

When an Atlas Lite milestone identifies a server gap:

1. prove current API cannot meet the requirement reasonably;
2. document memory/request/latency/security reason;
3. design the smallest canonical Atlas change;
4. use separate Atlas worktree/branch/Draft PR;
5. add server auth/capability/idempotency tests;
6. keep firmware and server commits separated.

---

# Milestone reporting contract

At the end of every milestone return:

```text
Repository:
BASE:
HEAD:
Branch:
Stacked on PR/branch:

Commits:
Draft PR:

Model/token strategy actually used:
- HIGH/MAX calls:
- MEDIUM calls:
- LOW/cheap calls:
- subagents dispatched:
- escalations avoided/performed:

Features completed:
Tests added:
Host validation:
Simulator validation:
Embedded build:
CI:

Hardware verified:
Hardware pending:

Atlas Server changes:
Security findings:
Known issues:
Rulings/deviations:
Deferred to save cost/tokens:
Next milestone:
```

Never mark hardware checks PASS from host/simulator/build evidence.

---

# Recommended milestone branch/PR sequence

Names may change slightly if needed, but keep scope isolated:

```text
M1.5  codex/atlas-lite-sim-01
M2    codex/atlas-lite-connectivity-01
M3    codex/atlas-lite-read-01
M4    codex/atlas-lite-search-views-01
M5    codex/atlas-lite-offline-01
M6    codex/atlas-lite-capture-01
M7    codex/atlas-lite-voice-01
M8    codex/atlas-lite-productization-01
```

Combining adjacent milestones into one stacked PR is allowed only when the diff remains reviewable and the controller explicitly records why. M1.5 + M2 is acceptable because the simulator is infrastructure for M2, but later milestones should normally stay separate.

---

# Final MVP Definition of Done

The physical Atlas Lite device must eventually:

1. boot reliably;
2. provision Wi-Fi without recompilation;
3. pair/authenticate with minimum privileges;
4. render Home;
5. navigate Library;
6. open/read Markdown notes;
7. search;
8. open Views;
9. capture text;
10. read useful cached content offline;
11. queue capture offline;
12. retry without duplication;
13. recover when Atlas/NAS/network is unavailable;
14. sleep and wake correctly;
15. retain a useful e-paper image while asleep;
16. expose battery/Wi-Fi/sync/device status;
17. support a safe firmware update/recovery process for field use.

Voice/STT is an additional milestone and may follow the read/capture MVP if needed, but its architecture must continue to keep provider secrets server-side.
