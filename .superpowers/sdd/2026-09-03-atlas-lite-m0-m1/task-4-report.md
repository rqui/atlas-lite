# Task 4 report — Atlas Lite diagnostics Home review fixes

## Status

The original Task 4 implementation was produced in commit `3bbc5ea feat: add
Atlas Lite diagnostics home`. The original implementer reached its usage limit
before emitting the required report, so this audit report and the geometry
coverage fix were completed in a separate implementation round.

The first review-fix round found no blocking functional defect in the original
implementation. It addressed the missing report and the lack of robust
five-row geometry/render coverage.

This second review-fix round addresses the reported Important refresh-policy
defect: a successful periodic or manual Weather fetch also repainted the static
Atlas Home route. The refresh remains owned by the existing coordinator and is
now limited to the two visible Weather surfaces.

## Verified findings before editing

- `task-4-report.md` was absent before the first review-fix round.
- The original render assertion in `src/app/mod.rs` checked only two native
  framebuffer pixels for the default selection.
- The original Home renderer drew five menu rows, but no test iterated all five
  selections, checked the logical `480x800` portrait bounds, or compared
  selected and unselected stroke ink for every row.
- In `src/main.rs`, successful Weather fetches matched `ScreenRoute::Home`,
  `ScreenRoute::Weather`, and `ScreenRoute::WeatherDetails` before calling the
  shared `refresh_screen` path. Home is static Atlas diagnostics content, so
  that match caused avoidable e-paper churn and possible ghosting.

## Scope

- Added a pure `atlas_home_menu_rect` geometry helper and reused it from the
  existing renderer so test geometry cannot drift from product geometry.
- Added one host render test that covers all five Atlas Home selections,
  logical portrait bounds, native coordinate mapping, selected-row ink, and
  unselected-row ink.
- Added the host-testable `ScreenRoute::is_weather_refresh_visible` policy:
  only `Weather` and `WeatherDetails` return true. `Home` and `Settings` are
  explicitly covered as false.
- Reused that policy after a successful Weather fetch. The existing
  `refresh_screen` call, panel refresh coordinator, and Weather-visible
  behavior are retained.
- No REST, hardware, driver, document-authority, future-surface, or hardware
  behavior was changed. `SELECT PENDING` remains unchanged.

## TDD evidence

### Original Task 4

The original RED/GREEN evidence was not captured. It is intentionally not
reconstructed or inferred from the existing commit.

### First review-fix round — RED

The geometry test was added before the production helper or renderer was
changed.

Command:

```bash
export PATH=/opt/homebrew/opt/rustup/bin:/Users/roger/.cargo/bin:$PATH
cargo test atlas_home_menu_geometry_and_ink_cover_every_selection
```

Result: exit 101. Compilation failed because the new test imported the not-yet
implemented `atlas_home_menu_rect` helper:

```text
error[E0432]: unresolved import
`crate::app::screens::atlas_home::atlas_home_menu_rect`
no `atlas_home_menu_rect` in `app::screens::atlas_home`
```

### First review-fix round — GREEN

After adding the minimal pure helper and routing the renderer through it, the
same focused command passed:

```text
running 1 test
test app::tests::atlas_home_menu_geometry_and_ink_cover_every_selection ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 315 filtered out
```

The command also compiled the `main` test target, which has no test cases.
After formatting, the focused command was run once more with the same result.

### Second review-fix round — RED

The Weather refresh-policy test was added before the predicate existed.

Command:

```bash
export PATH=/opt/homebrew/opt/rustup/bin:/Users/roger/.cargo/bin:$PATH
HOST_TRIPLE="$(rustc +stable -vV | sed -n 's/^host: //p')"
cargo +stable test --target "$HOST_TRIPLE" --lib weather_fetch_refreshes_only_weather_surfaces
```

Result: exit 101. The test failed to compile because
`ScreenRoute::is_weather_refresh_visible` did not yet exist.

### Second review-fix round — GREEN

After adding the pure route predicate and using it in the successful Weather
fetch branch, the same focused command exited 0:

```text
running 1 test
test app::router::tests::weather_fetch_refreshes_only_weather_surfaces ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 316 filtered out
```

## Validation this round

### Full host validation

The required shell command was run with the requested setup:

```bash
export PATH=/opt/homebrew/opt/rustup/bin:/Users/roger/.cargo/bin:$PATH
source /Users/roger/export-esp.sh
./scripts/validate.sh
```

Result: PASS, exit 0. All source-contract checks passed; the native host target
was `aarch64-apple-darwin`; the host suite reported `317 passed; 0 failed; 0
ignored; 0 measured`; and `host-test-native-target-isolation=ok`.

### ESP-IDF build

The documented embedded build was run with the requested setup:

```bash
export PATH=/opt/homebrew/opt/rustup/bin:/Users/roger/.cargo/bin:$PATH
source /Users/roger/export-esp.sh
./scripts/build.sh
```

Result: PASS, exit 0. The script reran validation successfully and completed
`cargo +esp build --release` for this worktree in 23.80 seconds.

### Diff check

```bash
git diff --check
```

Result: PASS, exit 0.

## Files changed in the review-fix rounds

- `src/app/mod.rs` — five-selection geometry and ink test.
- `src/app/screens/atlas_home.rs` — pure row-rectangle helper and renderer
  reuse.
- `src/app/router.rs` — pure visible-Weather-refresh predicate and policy test.
- `src/main.rs` — successful Weather fetch only requests redraw for visible
  Weather surfaces through the existing refresh coordinator.
- `.superpowers/sdd/2026-09-03-atlas-lite-m0-m1/task-4-report.md` — auditable
  evidence for both review-fix rounds and normal Markdown fences/code spans.

## Commit

First review-fix commit: `3833fca test: cover Atlas Lite Home geometry`.

This round's separate commit: `fix: avoid static Home weather refresh`.

## Limitations and preserved state

- No physical board validation was performed. Display, input, SD, RTC, PMIC,
  Wi-Fi, refresh/ghosting behavior, and power behavior remain **NOT TESTED**
  physically. The embedded build is not physical-board evidence.
- `target/.rustc_info.json` was pre-existing dirty state and remains unstaged.
- The ESP-IDF build updated tracked generated `target/release` artifacts; they
  remain unstaged and are not part of this fix.
- `BOOTSTRAP-MANIFEST.json` and `LUNA-MAX-PROMPT.md` remain untracked and
  unstaged.
- No push, PR, merge, deploy, or changes to authoritative documents were made.

## Corrective verification after global review

The earlier build paragraph above is superseded: it did not prove that the
embedded target was selected and its reported `23.80 seconds` result was not
an auditable Xtensa artifact check. The repository now pins the Rustmix
ESP-IDF `v5.4.3` / global-tools setup and `ldproxy` in `.cargo/config.toml`,
and `scripts/build.sh` selects and inspects the Xtensa target explicitly.

The official commands were rerun from the final M1 worktree on 2026-09-04:

```text
./scripts/validate.sh: PASS, exit 0, 320 passed; 0 failed
./scripts/build.sh: PASS, exit 0
embedded-build-target=xtensa-esp32s3-espidf
embedded-build-artifact-type=ELF 32-bit LSB executable, Tensilica Xtensa
embedded-build-artifact-sha256=54e27ab505ae0cac9962968586c8192d405582a467aac26ffc250f7bc824af07
```

This remains compilation evidence only. The physical board matrix remains
**NOT TESTED**.
