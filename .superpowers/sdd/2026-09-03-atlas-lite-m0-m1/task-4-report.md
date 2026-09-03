# Task 4 report — Atlas Lite diagnostics Home geometry review fix

## Status

The original Task 4 implementation was produced in commit \`3bbc5ea feat: add
Atlas Lite diagnostics home\`. The original implementer reached its usage limit
before emitting the required report, so this audit report and the geometry
coverage fix were completed in a separate implementation round.

The review found no blocking functional defect in the original implementation.
This round addresses the two Important findings: the missing report and the
lack of robust five-row geometry/render coverage.

## Verified findings before editing

- \`task-4-report.md\` was absent before this round.
- The original render assertion in \`src/app/mod.rs\` checked only two native
  framebuffer pixels for the default selection.
- The original Home renderer drew five menu rows, but no test iterated all five
  selections, checked the logical \`480x800\` portrait bounds, or compared
  selected and unselected stroke ink for every row.

## Scope

- Added a pure \`atlas_home_menu_rect\` geometry helper and reused it from the
  existing renderer so test geometry cannot drift from product geometry.
- Added one host render test that covers all five Atlas Home selections,
  logical portrait bounds, native coordinate mapping, selected-row ink, and
  unselected-row ink.
- No REST, hardware, driver, refresh, document-authority, or future-surface
  behavior was changed. \`SELECT PENDING\` remains unchanged.
- The existing diagnostics redaction, legacy routes/platform behavior, and
  shared refresh path remain intact.

## TDD evidence

### Original Task 4

The original RED/GREEN evidence was not captured. It is intentionally not
reconstructed or inferred from the existing commit.

### This review-fix round — RED

The geometry test was added before the production helper or renderer was
changed.

Command:

\`\`\`bash
export PATH=/opt/homebrew/opt/rustup/bin:/Users/roger/.cargo/bin:$PATH
cargo test atlas_home_menu_geometry_and_ink_cover_every_selection
\`\`\`

Result: exit 101. Compilation failed because the new test imported the not-yet
implemented \`atlas_home_menu_rect\` helper:

\`\`\`text
error[E0432]: unresolved import
\`crate::app::screens::atlas_home::atlas_home_menu_rect\`
no \`atlas_home_menu_rect\` in \`app::screens::atlas_home\`
\`\`\`

### This review-fix round — GREEN

After adding the minimal pure helper and routing the renderer through it, the
same focused command passed:

\`\`\`text
running 1 test
test app::tests::atlas_home_menu_geometry_and_ink_cover_every_selection ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 315 filtered out
\`\`\`

The command also compiled the \`main\` test target, which has no test cases.
After formatting, the focused command was run once more with the same result.

## Validation this round

### Focused test

Final focused command:

\`\`\`bash
export PATH=/opt/homebrew/opt/rustup/bin:/Users/roger/.cargo/bin:$PATH
cargo test atlas_home_menu_geometry_and_ink_cover_every_selection
\`\`\`

Result: exit 0; \`1 passed\`, \`0 failed\`, \`315 filtered out\` in the library test
binary.

### Full validation

The first required-shell attempt was made exactly with the requested setup:

\`\`\`bash
export PATH=/opt/homebrew/opt/rustup/bin:/Users/roger/.cargo/bin:$PATH
source /Users/roger/export-esp.sh
./scripts/validate.sh
\`\`\`

It exited 1 before tests because formatting checks reported two newly added
line-wrap differences in \`src/app/mod.rs\` and
\`src/app/screens/atlas_home.rs\`. No functional or source-contract failure was
reported in that attempt.

After \`cargo fmt --all\`, the same required-shell command was rerun and exited
0. Counts/results:

- all listed contract, syntax, lexical, and source checks: \`ok\`;
- native host target: \`aarch64-apple-darwin\`;
- host suite: \`316 passed; 0 failed; 0 ignored; 0 measured\`;
- \`host-test-native-target-isolation=ok\`.

### Diff check

\`\`\`bash
git diff --check
\`\`\`

Result: exit 0.

## Files changed in this round

- \`src/app/mod.rs\` — five-selection geometry and ink test.
- \`src/app/screens/atlas_home.rs\` — pure row-rectangle helper and renderer
  reuse.
- \`.superpowers/sdd/2026-09-03-atlas-lite-m0-m1/task-4-report.md\` — this
  auditable evidence report.

## Commit

Separate commit requested for this fix:

\`\`\`text
test: cover Atlas Lite Home geometry
\`\`\`

The final commit SHA is reported in the implementation handoff after commit
creation.

## Limitations and preserved state

- No physical board validation was performed; display, input, SD, RTC, PMIC,
  Wi-Fi, refresh behavior, and power behavior remain hardware-pending.
- \`target/.rustc_info.json\` was pre-existing dirty state and remains unstaged.
- \`BOOTSTRAP-MANIFEST.json\` and \`LUNA-MAX-PROMPT.md\` remain untracked and
  unstaged.
- No push, PR, merge, deploy, or changes to authoritative documents were made.
