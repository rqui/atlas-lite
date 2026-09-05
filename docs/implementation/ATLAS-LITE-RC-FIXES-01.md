# ATLAS-LITE-RC-FIXES-01 — Installation and integration close-out

**Date:** 2026-09-04
**Status:** approved corrective scope; implementation and verification pending.
**Executor:** Terra Codex, working directly under `AGENTS.md`.

## 1. Objective and limits

Close the concrete installation/integration uncertainties identified after M8. Keep the initial roadmap and voice-first product decision. Do not restart phases or add features.

Deliver a reproducible first-install candidate, a tested combination of existing Atlas pairing/audio changes, and an accurate real-transcription setup/verification procedure. Do not launch another architecture project.

Out of scope: a new assistant, manual Text Capture UI, new generic device APIs, speculative driver/power tuning, broad renames, mandatory removal of Rustmix modules, new OTA distribution infrastructure and a second pairing protocol.

No production merge, deploy, release publication, physical flash/erase, eFuse write, purchase or paid-provider activation is authorized. Test scripts must use fakes/dry runs unless the user separately authorizes a real device operation.

## 2. Starting points and safe Git workflow

Observed references, to revalidate before execution:

| Repository | Work | Reference |
| --- | --- | --- |
| `rqui/atlas-lite` | M8 candidate, PR #7 | `codex/atlas-lite-productization-01`, source HEAD `9183b51c827f347f37c9cd6e9a7bde0788c64fed` |
| `rqui/atlas` | audio/STT, PR #158 | `codex/atlas-lite-audio-capture-01`, observed HEAD `e300f9f41df69f5a96e615b15f3cf54bf2d48a40` |
| `rqui/atlas` | pairing, PR #160 | `codex/atlas-lite-pairing-01`, observed HEAD `3870b4136567d7532c0fba757523ae5ec1849106` |

The firmware branch now also carries this documentation. Fetch its latest remote tip rather than resetting to the historical source HEAD. Preserve all existing work.

Create an isolated firmware correction branch/worktree from the actual PR #7 head. Suggested name: `codex/atlas-lite-rc-fixes-01`. Open its Draft PR against the branch it actually depends on, normally `codex/atlas-lite-productization-01` while #7 remains open.

Inspect current server `master`, PR ancestry and checks. Use a separate server worktree if integration or fixes require it. Respect that repository's instructions. Keep the existing PR branches and production refs intact. A local temporary integration branch combining reviewed changes is permitted for validation only; it is not permission to merge GitHub PRs or deploy.

## 3. Work A — Coherent initial flashing and packaging

### Evidence to establish first

M8 adds a custom A/B table and rollback-related bootloader configuration. At the reviewed source HEAD, `scripts/flash.sh` and `scripts/flash-release.sh` pass only an ELF to standalone `espflash`; the release bundle is described as ELF-only.

This is an installation guarantee gap, not a demonstrated hardware failure. Determine what the installed tool/version and project configuration actually select. Do not assume either that defaults are wrong or that an ELF contains every required bootloader/table artifact.

Inspect only relevant files and generated outputs:

```text
.cargo/config.toml
Cargo.toml / build.rs / sdkconfig.defaults / partitions.csv
scripts/build.sh
scripts/flash.sh
scripts/build-release-firmware.sh
scripts/flash-release.sh
scripts/test-release-flash-workflow.sh
scripts/rust-toolchain.sh
docs/RELEASE.md / docs/PHYSICAL_SMOKE_TEST.md
ESP-IDF-generated bootloader, partition-table and application outputs
```

Use the actual installed `espflash`/`cargo-espflash` help and official documentation. Establish exact supported flags/configuration. Do not invent a command line from memory.

### Required correction

1. Select the application ELF, generated bootloader and generated partition table from the same current build. Resolve files deterministically through build metadata or verified paths; never take the first stale glob match from another worktree/profile.
2. Ensure both development and packaged installation select that coherent set. Use documented ELF-aware flashing with explicit artifacts, or a documented build-aware tool path that demonstrably selects them. Do not introduce guessed raw-address writes.
3. Check target chip/flash size and table capacity/alignment/overlap using the project's tools. The intended table has `otadata` and two 6 MiB application slots on the 16 MiB target. Verify the generated table and rollback-enabled bootloader rather than just the source CSV/config.
4. Reject missing, ambiguous, stale or incompatible artifacts before opening a serial port or writing anything. Do not silently fall back to a default bootloader/table.
5. Package everything needed to reproduce installation outside the source checkout: application, matching bootloader/table, installer, checksum/provenance manifest and instructions. Record source SHA, tool versions, build configuration and SHA-256 of every flashed artifact. Do not rely on private local build paths remaining present.
6. Do not package the ELF as an OTA application binary. Label initial-install artifacts and OTA application images unambiguously. A merged factory `.bin` is not required for this correction and stays deferred unless its layout is separately proven; no extra image workflow merely for convenience.
7. Keep serial-port selection explicit and explain when a user-visible partition migration or erase might be necessary. Never perform whole-flash erase automatically. Document backup and ROM/USB recovery before any recommended destructive operation.
8. Update the existing release/script tests instead of adding a second packaging framework.

### Acceptance

Host/fake-tool tests must assert the exact artifact arguments/config passed, paths containing spaces, explicit port handling, missing/mismatched artifacts, and checksum rejection. A package unpacked into a clean temporary directory must resolve all its own files without the original build tree. Test execution must not flash a device.

Run the relevant script tests and firmware validation/build. Report generated partition entries and actual image sizes/hash provenance. Physical successful boot and rollback remain NOT TESTED until separately observed.

### Follow-up: application-image provenance

The generated `flasher_args.json` belongs to the ESP-IDF auxiliary project
built by `esp-idf-sys`. Its bootloader and partition-table outputs remain
useful build metadata, but its `.app.file` must not be presented as the Atlas
Lite application image without proving that it was derived from the packaged
Atlas Lite ELF.

For the initial-install candidate, generate `application.bin` by converting
the exact `atlas-lite.elf` placed in the same bundle with an official,
version-recorded tool. Preserve the explicit generated bootloader,
partition-table and `ota_0` selection; do not add raw-address installation or
a merged factory image.

Acceptance for this follow-up:

- the bundle manifest records the conversion tool, version and command;
- `application.bin` is reproduced by an independent conversion of that
  bundle's ELF with those recorded options;
- the image is an ESP32-S3 application image, fits the `ota_0` slot, and does
  not concatenate the bootloader or partition table;
- a regression fails if packaging copies the auxiliary project's `.app.file`;
- the extracted ZIP checksum verification and all existing installation tests
  still pass, without contacting hardware.

## 4. Work B — Combined Atlas pairing and audio integration

### Establish a real combined tree

Individually passing PRs do not prove that the deployed server contains both contracts. Determine which changes are already ancestors of current `master`; combine only missing changes in an isolated integration checkout. Do not apply a PR twice or rebase unrelated user work.

Use a disposable test vault and test-only credentials. Do not migrate or operate on the user's live vault. Record exact master, source PR SHAs and combined test HEAD/tree.

Inspect the previously reported audio timeout in PR #158's synthetic-merge check: exact test, logs, environment, ordering and reproduction. Distinguish a repeatable defect from a one-off runner event using evidence. Fix only a demonstrated cause. Do not turn it green by skipping the test, adding arbitrary sleeps, disabling checks or inflating timeouts without a causal explanation.

### Required integrated checks

- A user-approved device credential receives exactly the existing four intended scopes and can read through the normal REST API.
- The same paired credential can POST a valid bounded WAV to `/api/v1/capture/audio` with a persisted canonical `Idempotency-Key`.
- The `202 accepted` response follows durable ownership of the WAV and pending note, not merely queued in-memory work. The receipt identity/attachment/hash/size are validated against the actual contract.
- Lost response and same-key retry do not create a second note/attachment. Restart recovery preserves pending work and original audio.
- Automatic transcription updates the same note and preserves its original WAV link. STT failure must not lose audio or create duplicate notes.
- Revocation denies subsequent protected reads/uploads; expiry, insufficient scope and invalid payloads produce canonical errors.
- Preserve and document the existing 30-day automatic-recovery limitation and manual preservation behavior; do not silently extend retention or discard expired audio.

Run focused regressions during changes, then the server's required lint/typecheck/tests/build on the exact combined candidate. Run Docker smoke when a functioning runtime is available; otherwise report the unavailable runtime separately and still finish non-Docker validation. Do not claim combined hosted CI from separate PR checks.

If a code correction is needed in Atlas, publish it in a separate reviewable Draft PR with its dependency/base clearly stated. Prefer minimal dependency-aware fixes over an integration PR that accidentally duplicates every earlier feature. If only integration evidence is needed, document it without manufacturing server changes.

## 5. Work C — Actual transcription configuration

The implemented worker is automatic only when a compatible provider is configured. The current server adapter sends raw `audio/wav` by HTTP POST to `ATLAS_AUDIO_TRANSCRIPTION_URL`, optionally with server-only `ATLAS_AUDIO_TRANSCRIPTION_TOKEN`, and expects JSON containing exactly one `transcript` string. Confirm this against the current server code before documenting it.

Do not claim that pointing this URL at any vendor's transcription endpoint automatically works: request authentication, multipart/raw body and response format must match. Reuse a compatible existing service if available. Do not create an unrelated AI app or silently choose a paid provider.

Required deliverables:

1. Document the exact environment/config fields, network reachability, input/output contract, timeout/size limits and unconfigured/error behavior.
2. Use a deterministic local compatible provider for reproducible integration/recovery tests. Label it MOCK/TEST PROVIDER, never live STT.
3. When a compatible real provider and authorized credentials are already available, run one short non-sensitive audio sample end to end and verify actual meaningful transcript text in the same note with the original attachment. Do not expose secrets or upload real user recordings without authorization.
4. When credentials/service are absent, finish all safe implementation and fixture tests, explicitly report `LIVE STT: BLOCKED — provider not configured`, and provide the precise setup/verification command or procedure. A missing provider does not justify inventing success, starting another milestone or abandoning the installation corrections.

Do not weaken TLS or remove provider limits to make a test pass. Any necessary provider adaptation requires a concrete selected compatible contract and a small server-only change.

## 6. Work D — Documentation and first-board handoff

Keep `README.md`, `docs/RELEASE.md`, `docs/PHYSICAL_SMOKE_TEST.md`, `docs/KNOWN_ISSUES.md` and relevant server docs consistent with the actual result. No new task-management scaffolding is needed.

The original roadmap, design baseline, voice-first decision and implementation evidence must remain accessible. Remove only obsolete execution-framework instructions or orphaned links found in affected documentation; do not delete useful product requirements, tests, drivers, licenses or unrelated server guidance.

Prepare one concrete first-board procedure:

```text
verify candidate hashes and matching flash artifacts
-> document backup/ROM recovery
-> user-authorized USB installation
-> boot and screen/input
-> setup AP -> Wi-Fi/Atlas URL -> approval in Atlas Web
-> limited-credential read of an existing test note
-> record a short voice clip
-> verify durable upload, original WAV and automatic same-note transcript
```

After that basic path works, the physical checklist may proceed to offline/reboot, loss-of-power behavior, power measurement and signed OTA/rollback. Do not begin destructive power-cut testing before confirming basic boot and recovery. No unattended hardware operation is part of this coding task.

OTA source/signing keys/distribution not configured is an explicit operational prerequisite, not a reason to invent new infrastructure. Preserve existing fail-closed behavior.

## 7. Completion and reporting

Work directly and avoid repeated scouts/audits. Use existing tests, focused fixes and one final correctness/security/scope review. Do not turn an arbitrary dispatch cap into a new blocker.

Finish the independent code/doc corrections even when hardware, Docker or a provider is unavailable; mark each missing verification separately.

Return:

```text
Firmware repo / BASE / HEAD / branch / Draft PR:
Server source refs / combined integration tree / any corrective Draft PR:
Actual flashing method and selected bootloader/table/application:
Candidate package path, contents, versions and checksums:
Host/script tests:
Firmware build and simulator:
Server combined tests/build:
Hosted CI (exact SHA, not inferred):
Audio-timeout investigation and resolution:
Mock-provider integration:
Live STT provider integration or precise blocker:
Physical tests: NOT TESTED unless actually performed with authorization
Remaining operational prerequisites:
Single next action for the user:
```

Do not label the device release-ready merely because code compiles. No merge, production deployment, published release or physical write.

## 8. Physical bring-up follow-up: Library, Home and Voice Capture

### Confirmed causes

- The Atlas Library route changed navigation state but the firmware dispatcher
  only consumed Search, Views and Note work. A normal physical `SELECT` into
  Library therefore issued no `ListNotes` request. The simulator fixture had
  hidden that gap by directly invoking the refresh seam before navigation.
- The observed Voice Capture boot evidence confirms that SDMMC mounting is
  unavailable (`send_op_cond` returned `ESP_ERR_TIMEOUT`). It does **not**
  demonstrate a microphone, ES8311, I2S, PCM-format or WAV-header failure.
  The existing recorder correctly refuses to begin without mounted storage;
  the presentation did not identify that condition clearly enough.

### Correction and acceptance criteria

- Home and Library use explicit, one-shot pending requests consumed by the
  shared AppState dispatcher in both firmware and simulator. Rendering never
  calls transport. A valid connected configuration queues Home once at boot;
  entering an uninitialized Library queues its bounded page load once; an
  empty/error Library supports an explicit `SELECT` retry. Home, Library,
  Search and Views retain independent connection outcomes.
- The required regression starts with an empty Library, injects only physical
  simulator input, observes `ListNotes`, selects the returned row, observes
  `GetNote`, and opens the existing Note Reader. It does not preload the
  Library or call refresh directly.
- Capture reports the actionable class for unavailable SD, SD write/space,
  microphone, I2S, WAV finalization and upload delivery. Text wraps on screen
  rather than silently truncating the cause. A successful state means the WAV
  was finalized and durably queued; `Delivered to Atlas` means upload ACK, not
  transcription completion. PCM16 mono 16 kHz and the inherited ES8311/I2S
  conversion remain unchanged.
- Visible product headers use `ATLAS`; stable internal names, NVS keys,
  package identifiers, paths and networking policy remain unchanged.

### Physical status

`NOT TESTED` for this corrected HEAD. The next physical check is limited to
booting the matching candidate, confirming an accessible microSD card, then
checking Home/Library traffic and one short Capture recording. Do not erase
NVS, re-pair, flash, change SDMMC pins/frequency/power, or alter audio drivers
as part of this correction.

## 9. Physical follow-up: post-response refresh, reader geometry and Library order

### Confirmed causes and correction

- Atlas requests completed after the input-triggered panel refresh. Their
  visible state changes had no one-shot invalidation, so Library and Note
  remained painted as Loading until a later physical input caused a redraw.
  `AppState` now raises a one-shot Atlas render invalidation after a completed
  Home, Library, Search, Views or Note request. The existing main-loop panel
  owner consumes it once through the existing refresh coordinator; renderers
  and transport workers remain separate.
- The Note renderer now distinguishes Idle, Loading and Error when no document
  exists. The old selection instruction is shown only for Idle; a cached
  document is retained only when it belongs to the selected stable ID.
- Note now retains `document.title()` in a single clipped, preference-aware
  title band (126–152 px). It is width-truncated with an ASCII ellipsis, so it
  never overlaps the status strip or Markdown body, including when Markdown
  starts without a heading. The body remains a 436 px by 588 px viewport;
  together with the title band this preserves approximately 614 px of useful
  reader area without returning to the former small viewport.
- Server ordering was inspected read-only at current `rqui/atlas` master
  revision `62040555cd5c33fbbef27cfd9de7bad2ef477e0d`, specifically
  `apps/web/src/notes/hierarchy.ts` and `packages/shared/src/hierarchy.ts`.
  The latter supplies `compareHierarchySiblings`: canonical non-negative
  decimal order by arbitrary precision, invalid/null last, then
  `path.localeCompare('en', { numeric: true, sensitivity: 'base' })`, then ID.
  The firmware parity fixture is generated from that exact comparator and
  checks its ID sequence across numeric, case/accent, nested-path and invalid
  order ties.
- ESP32 firmware does not claim ICU-wide collation parity. It preserves a
  bounded exact sort key of at most 1,024 UTF-8 bytes for ASCII plus explicit
  NFC Latin-1 folds (including Á/É/Í/Ó/Ú/Ñ and their lower-case forms). Atlas
  logical paths permit wider Unicode, so unsupported scalars or paths over the
  bound are withheld with `UnsupportedPathOrder` rather than silently
  truncating or ordering them differently from Web. Partial cursor results
  retain children whose parents were not fetched as provisional roots and keep
  the partial indicator.

### Acceptance

- One navigation into Library and one note selection each receive a subsequent
  panel refresh after their typed request completes, without polling renderers
  or a second physical key press. Stable idle ticks consume neither requests
  nor panel refreshes.
- A 480 x 800 portrait Note keeps a compact visible title and 588 px of
  Markdown body (about 614 px including the title band); long paragraphs,
  headings, lists, long words and UTF-8 are wrapped into bounded pages without
  using clipping to discard body overflow.
- Library order is independent of page arrival and title, supports decimal
  values beyond `u64`, and preserves selected IDs across visual reordering.
  Unsupported Unicode/path lengths fail visibly and deterministically instead
  of claiming complete Web collation parity.
- Physical validation remains `NOT TESTED` for the resulting HEAD.

## Reference sources for tool verification

- ESP Rust flashing tool documentation: https://github.com/esp-rs/espflash
- Build-aware flashing documentation: https://github.com/esp-rs/espflash/tree/main/cargo-espflash
- ESP-IDF partition and OTA documentation: https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/api-guides/partition-tables.html and https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/api-reference/system/ota.html

Consult the version corresponding to the actual installed toolchain; these references do not replace generated-artifact verification.
