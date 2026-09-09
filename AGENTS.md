# AGENTS.md — Atlas Lite

## Mission and scope

Atlas Lite is a native e-paper client for Atlas on Waveshare ESP32-S3-ePaper-3.97. It is a fork of `aimindseye/rustmix-wave`; `rqui/atlas` remains the authoritative server. Do not port React/PWA, run a browser or server on the ESP32, or replicate the full Vault.

## Sources of truth

Read `docs/implementation/README.md` first. The initial product roadmap is `docs/implementation/ATLAS-LITE-01.md`. Apply the dated voice-first decision and the narrowly scoped `docs/implementation/ATLAS-LITE-RC-FIXES-01.md` correction plan. The original design baseline is preserved at `docs/implementation/ATLAS-LITE-DESIGN.md`; later explicit product decisions override its historical text-capture requirements.

Current source/tests describe what is implemented, not proof that every requirement is complete. Resolve discrepancies with a small documented correction, not by silently changing requirements. Read the relevant repository's own instructions before server work.

## Working method

Work directly as the principal coding agent. There is no mandatory orchestration framework, scout phase, per-task fresh agent, per-task reviewer, fixed model tier, or dispatch quota.

Reuse prior analysis and existing implementation. Do not restart completed milestones. Keep an actionable short checklist, implement coherent changes, run focused regressions while editing, and run the required final validation once the candidate is stable. Review the final diff for correctness, security and scope. Re-review only material fixes, not cosmetic edits.

Delegation is optional and justified only by a concrete independent need. Do not create agents to format files, run commands, repeat audits or write routine documentation. Do not stop working merely because an artificial assignment count has been reached. Report real environmental blockers precisely and finish other safe, independent work.

## Git boundaries

- `origin` is `rqui/atlas-lite`; `upstream` is `aimindseye/rustmix-wave`.
- Fetch and record actual refs before working. Historical SHAs are provenance, not an instruction to reset current work.
- Use an isolated branch/worktree. Preserve local changes and existing commits; never reset or force-push shared history.
- Use Draft PRs. Do not merge PRs, deploy, publish releases, flash hardware, erase storage, or modify eFuses without explicit authorization.
- Server changes belong in a separate `rqui/atlas` worktree/branch/PR. A local disposable combination for integration testing is not authorization to merge production branches.
- Keep build output, credentials and transient agent reports out of Git. Preserve license/copyright and useful upstream documentation.

## Product boundaries

Keep Home, Library, Note, Search, Views, voice Capture and Settings. M6 manual Text Capture UI is cancelled. Keep existing typed `capture_text` infrastructure where useful; do not build a manual note editor or remove the Search keyboard.

Reuse working Rustmix display, refresh coordinator, input, SD, RTC, PMIC, audio/I2S and sleep code. Avoid speculative driver rewrites, broad renames and pruning without measured benefit. Atlas-specific functionality should remain separate from the platform.

Use existing typed REST contracts. Device pairing and audio capture have specific contracts; do not add a generic `/api/v1/device/*` facade without measured need. Never bypass canonical Atlas authorization, Capture, Vault or attachment services.

The device capability set is `notes:read`, `search:read`, `views:read`, `capture:write`. Do not introduce admin credentials or embed provider secrets in firmware.

## Resource and data safety

Use explicit bounds for bytes, entries, duration, concurrency, retries and timeouts. Avoid unrestricted JSON trees in device flows. Preserve stable IDs and per-surface freshness/error states.

Atlas/Wi-Fi credentials belong in internal NVS, never microSD, logs, shell history or public artifacts. NVS placement alone is not a claim of encryption at rest.

The SD data root remains `/ATLAS/` with bounded CACHE, QUEUE, AUDIO, ASSETS and LOGS. Cache may be disposable; pending captures are not. Preserve finalized WAVs and stable persisted idempotency keys through uncertain delivery and reboot. Device upload responsibility ends only at the validated durable server receipt, not merely an HTTP success or STT completion.

The server retains the original audio and updates the same note with automatic transcription. A configured compatible provider is required; mock-provider success is not live transcription evidence.

## Verification and reporting

Run the existing `./scripts/validate.sh`, targeted simulator tests, `./scripts/build.sh` for target changes, and `git diff --check` as appropriate. Run the server's required checks for server changes. Documentation-only edits need link/scope/diff checks, not an invented firmware test run.

Keep HOST TESTED, TARGET BUILD TESTED, SIMULATOR TESTED, LIVE INTEGRATION TESTED and HARDWARE TESTED distinct. No build or mock proves physical display, audio, radio, NVS, power loss, OTA rollback or current consumption.

Report actual repo/base/head/branch/PR, tests and commands run, artifact provenance when relevant, findings fixed, remaining blockers and precise untested areas. Do not equate a clean merge or absent CI checks with passing CI.
