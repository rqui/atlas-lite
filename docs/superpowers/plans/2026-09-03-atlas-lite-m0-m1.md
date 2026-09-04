# Atlas Lite M0-M1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Establish the real Atlas Lite fork and replace the visible Rustmix product entry shell with a hardware-preserving Atlas Lite bring-up surface.

**Architecture:** Keep Rustmix's main-task hardware ownership, existing snapshots, and shared refresh coordinator. M0 adds only repository/planning metadata. M1 changes routing/product-shell presentation and adds an Atlas diagnostics Home without introducing Atlas REST yet.

**Tech Stack:** Rust 1.82+, ESP-IDF, embedded-graphics, existing Rustmix modules.

**Spec:** `docs/superpowers/specs/2026-09-03-atlas-lite-design.md`

## Global Constraints

- `origin` must be `rqui/atlas-lite`.
- `upstream` must be `aimindseye/rustmix-wave`.
- Keep MIT license and upstream attribution.
- Work on `codex/atlas-lite-bootstrap-01` or a clearly equivalent isolated milestone branch.
- Do not merge, release or deploy.
- Do not delete working platform modules in M1.
- All panel refreshes remain routed through `src/panel_refresh.rs`.
- Screens do not own hardware handles.
- `./scripts/validate.sh` must remain green before the Draft PR is considered ready for user review.
- Embedded build success is not hardware verification.

---

### Task 1: Fork and repository relationship

**Files:** no source changes.

**Produces:** a real `rqui/atlas-lite` remote plus an isolated worktree on the milestone branch.

- [ ] Create a GitHub fork of `aimindseye/rustmix-wave` under `rqui` named `atlas-lite`.
- [ ] Clone/use the fork and set:
  ```bash
  git remote rename origin upstream
  git remote add origin <authenticated rqui/atlas-lite remote>
  ```
  If the fork clone already has `origin=rqui/atlas-lite`, instead add:
  ```bash
  git remote add upstream https://github.com/aimindseye/rustmix-wave.git
  ```
- [ ] Verify:
  ```bash
  git remote -v
  ```
  Expected ownership:
  ```text
  origin   rqui/atlas-lite
  upstream aimindseye/rustmix-wave
  ```
- [ ] Fetch:
  ```bash
  git fetch origin
  git fetch upstream
  ```
- [ ] Record:
  ```bash
  git rev-parse upstream/main
  git rev-parse HEAD
  ```
- [ ] Create isolated worktree/branch using the repository's worktree convention:
  ```text
  codex/atlas-lite-bootstrap-01
  ```
- [ ] Confirm:
  ```bash
  git status --short
  git branch --show-current
  ```
  Expected: clean worktree, non-main branch.

**Review gate:** repository relationship and clean BASE are correct before any file write.

---

### Task 2: Commit the approved Atlas Lite planning baseline

**Files:**
- Create: `AGENTS.md`
- Create/replace with Atlas-focused top section: `README.md`
- Create: `docs/ARCHITECTURE.md` only if the upstream file is intentionally replaced; otherwise preserve upstream architecture and add `docs/ATLAS_LITE_ARCHITECTURE.md`
- Create: `docs/UPSTREAM.md`
- Create: `docs/implementation/ATLAS-LITE-01.md`
- Create: `docs/superpowers/specs/2026-09-03-atlas-lite-design.md`
- Create: `docs/superpowers/plans/2026-09-03-atlas-lite-m0-m1.md`

**Important collision rule:** Rustmix already has `docs/ARCHITECTURE.md`. Do not destroy its useful upstream platform documentation. Preferred implementation is:
- preserve upstream `docs/ARCHITECTURE.md`;
- save Atlas-specific architecture as `docs/ATLAS_LITE_ARCHITECTURE.md`;
- update links in the imported plan/spec accordingly.

- [ ] Copy/import the approved planning bundle.
- [ ] Adjust only links/file names required by the collision rule; do not silently change product requirements.
- [ ] Add an Atlas Lite README introduction that states:
  - Atlas Lite is a native Atlas e-paper client;
  - it is based on Rustmix Wave;
  - current status is pre-MVP/bring-up;
  - authoritative roadmap path;
  - hardware target;
  - no implication of official Rustmix affiliation.
- [ ] Preserve upstream license and attribution.
- [ ] Run:
  ```bash
  ./scripts/validate.sh
  ```
  Expected: PASS. If existing upstream validation fails on the exact BASE, prove that by running the same command on an untouched BASE worktree before changing behavior.
- [ ] Commit:
  ```bash
  git add AGENTS.md README.md docs/
  git commit -m "docs: bootstrap Atlas Lite planning"
  ```

**Review gate:** docs accurately reflect current code; no source behavior change.

---

### Task 3: Define the Atlas product route contract with tests first

**Files:**
- Modify: `src/app/router.rs`
- Test: existing `#[cfg(test)]` module in `src/app/router.rs`, unless repository conventions justify a focused new test module.

**Produces:** route/back contract for Home, Library, Note, Search, Views, Capture, Settings without exposing unrelated Rustmix categories from the Atlas product root.

- [ ] Write failing tests for the intended route hierarchy.
  Required assertions:
  - Home has no parent.
  - Library/Search/Views/Capture/Settings back to Home.
  - the chosen Note return-context design does not hard-code every Note open to Library if Note can be opened from Search/Views.
  - unrelated legacy categories are not part of the Atlas product-root menu contract.
- [ ] Run the focused host test and verify failure for the new contract.
- [ ] Implement the minimum route/state changes.
  If static `ScreenRoute::parent()` cannot represent Note's originating surface, add a small Atlas navigation context in application state rather than encoding a false static parent.
- [ ] Re-run focused tests.
- [ ] Run:
  ```bash
  ./scripts/validate.sh
  ```
- [ ] Commit:
  ```bash
  git add src/app/router.rs src/app/state.rs
  git commit -m "feat: define Atlas Lite navigation shell"
  ```
  Omit `src/app/state.rs` from the commit if it was not needed.

**Review gate:** no hardware handles moved; no unrelated platform refactor.

---

### Task 4: Add Atlas Lite diagnostics Home with tests first

**Files:**
- Create: `src/app/screens/atlas_home.rs`
- Modify: `src/app/screens/mod.rs`
- Modify: render dispatch location following existing Rustmix pattern
- Modify: Home/menu data source needed to expose Atlas routes
- Test: host-testable rendering/state helpers following existing screen conventions

**Produces:** first visible Atlas Lite screen using only existing snapshots.

- [ ] Write failing host tests for a pure diagnostics view model that maps existing snapshots to redacted display labels.
  Required labels:
  ```text
  ATLAS LITE
  Display
  Input
  SD
  Wi-Fi
  Battery
  RTC
  ```
- [ ] Test that no Wi-Fi password/token field can enter the diagnostics model.
- [ ] Implement the pure diagnostics model and renderer.
- [ ] Wire the route into screen dispatch without acquiring hardware handles in the screen.
- [ ] Ensure the existing refresh request path is used; do not call panel transport directly.
- [ ] Update product-root menu/navigation to:
  ```text
  Library
  Search
  Views
  Capture
  Settings
  ```
- [ ] Run focused tests.
- [ ] Run:
  ```bash
  ./scripts/validate.sh
  ```
- [ ] Commit:
  ```bash
  git add src/app/
  git commit -m "feat: add Atlas Lite diagnostics home"
  ```

**Review gate:** renderer is e-paper-native, static/low-churn, and platform ownership remains unchanged.

---

### Task 5: Atlas Lite branding without upstream destruction

**Files:**
- Modify: `Cargo.toml` only if package/bin naming can be changed without breaking ESP-IDF tooling
- Modify: build-info/product-visible strings as needed
- Modify: README/docs when names changed
- Do not mass-rename upstream log markers or low-level module identifiers.

- [ ] Write/adjust any string/build-info tests before changing values.
- [ ] Change user-facing product naming to `Atlas Lite`.
- [ ] Keep upstream authorship/license.
- [ ] Do not claim hardware has been validated unless it has.
- [ ] Run:
  ```bash
  ./scripts/validate.sh
  ```
- [ ] Run the repository's documented embedded firmware build.
- [ ] Commit:
  ```bash
  git add Cargo.toml src README.md docs
  git commit -m "chore: brand firmware as Atlas Lite"
  ```
  Stage only files actually changed.

**Review gate:** diff is targeted; package/toolchain behavior is intact.

---

### Task 6: Physical bring-up evidence

**Files:**
- Create: `docs/implementation/ATLAS-LITE-M1-HARDWARE.md`

- [ ] Flash the M1 build to the Waveshare ESP32-S3-ePaper-3.97.
- [ ] Record board/firmware SHA.
- [ ] Verify each item independently:
  ```text
  boot
  e-paper first frame
  partial refresh behavior
  rotary movement
  rotary select
  BOOT short/long behavior
  Power behavior
  SD mount/detection
  RTC read
  battery/charge snapshot
  Wi-Fi connect behavior
  sleep
  wake/restore
  ```
- [ ] Record actual result for each as `PASS`, `FAIL`, or `NOT TESTED`.
- [ ] Do not modify driver code merely to make documentation match a different PMIC label; investigate actual failure/evidence first.
- [ ] Run final:
  ```bash
  ./scripts/validate.sh
  ```
- [ ] Commit evidence:
  ```bash
  git add docs/implementation/ATLAS-LITE-M1-HARDWARE.md
  git commit -m "docs: record Atlas Lite M1 hardware validation"
  ```

**Review gate:** any failed load-bearing hardware behavior blocks M1 completion and becomes a focused debugging task.

---

### Task 7: Open Draft PR and final review

- [ ] Run final whole-branch code/spec review using the most capable reviewer available.
- [ ] Fix accepted findings and re-run affected tests.
- [ ] Verify clean:
  ```bash
  git status --short
  ```
- [ ] Push the milestone branch to `origin`.
- [ ] Open Draft PR:
  ```text
  Atlas Lite M0-M1: bootstrap and hardware-preserving shell
  ```
- [ ] PR body must report:
  ```text
  Upstream BASE
  HEAD
  branch
  commits
  ./scripts/validate.sh result
  embedded build result
  hardware PASS/FAIL/NOT TESTED matrix
  known issues
  next milestone: M2 secure provisioning + AtlasClient
  ```
- [ ] Do not merge.

**M0-M1 completion condition:** Draft PR exists, validation evidence is explicit, and no claim exceeds the tests actually performed.
