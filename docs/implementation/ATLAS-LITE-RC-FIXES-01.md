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

## Reference sources for tool verification

- ESP Rust flashing tool documentation: https://github.com/esp-rs/espflash
- Build-aware flashing documentation: https://github.com/esp-rs/espflash/tree/main/cargo-espflash
- ESP-IDF partition and OTA documentation: https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/api-guides/partition-tables.html and https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/api-reference/system/ota.html

Consult the version corresponding to the actual installed toolchain; these references do not replace generated-artifact verification.
