# Task 5 report — Atlas Lite branding

## Status and scope

Task 5 changes only the product-facing branding and package description. The
base was `99def27 fix: avoid static Home weather refresh`.

- `PRODUCT_NAME` now renders `Atlas Lite` in the existing Device Info screen.
- Cargo's package description identifies Atlas Lite and preserves its Rustmix
  Wave basis.
- The README identifies the inherited Rustmix material as upstream reference
  and states the physical hardware status explicitly.
- No REST, drivers, handles, refresh policy, future product surfaces, or
  authoritative architecture/implementation documents were changed.

## TDD evidence

### RED

Before changing the product value, the existing build-info test was changed to
expect `Atlas Lite`.

```bash
export PATH=/opt/homebrew/opt/rustup/bin:/Users/roger/.cargo/bin:$PATH
source /Users/roger/export-esp.sh
HOST_TRIPLE="$(rustc +stable -vV | sed -n 's/^host: //p')"
cargo +stable test --target "$HOST_TRIPLE" --lib \
  build_info::tests::exposes_atlas_lite_product_metadata
```

Result: FAIL (exit 101). The assertion reported actual
`Rustmix Wave / EPD397` versus expected `Atlas Lite`.

### GREEN

After changing only `PRODUCT_NAME`, the same focused test passed.

Result: PASS (1 passed; 0 failed; 316 filtered out).

## Naming decisions

- Changed the user-visible `PRODUCT_NAME` and Cargo `description` to Atlas
  Lite.
- Preserved `name = "waveshare-epd397-rust-app"`, the `[[bin]]` name,
  `Cargo.lock`, and all release artifact paths. The release/flash scripts,
  their self-test, and the source-contract validator explicitly consume the
  existing binary and artifact path.
- Preserved `PRODUCT_SLUG = "rustmix-wave-epd397"` and Rustmix serial log
  markers as low-level upstream diagnostics, per the scoped branding rule.
- Preserved `authors = ["Piyush Daiya"]`, `license = "MIT"`, and the README's
  Rustmix Wave attribution/link. The README makes no claim that Atlas Lite is
  an official upstream release.

## Validation

### Full host validation

```bash
export PATH=/opt/homebrew/opt/rustup/bin:/Users/roger/.cargo/bin:$PATH
source /Users/roger/export-esp.sh
./scripts/validate.sh
```

Result: PASS (exit 0). Source-contract validation passed; native target
`aarch64-apple-darwin`; host suite `317 passed; 0 failed`; and
`host-test-native-target-isolation=ok`.

### ESP-IDF target build

```bash
export PATH=/opt/homebrew/opt/rustup/bin:/Users/roger/.cargo/bin:$PATH
source /Users/roger/export-esp.sh
./scripts/build.sh
```

Result: PASS (exit 0). The script reran host validation and completed
`cargo +esp build --release` for
`waveshare-epd397-rust-app v1.0.0`.

### Diff check

```bash
git diff --check
```

Result: PASS (exit 0).

## Hardware

**NOT TESTED.** No physical Waveshare board was flashed or exercised. The
successful ESP-IDF build is target compilation evidence only; it does not
verify boot, e-paper refresh/ghosting, input, SD, RTC, PMIC/battery, Wi-Fi,
audio, sleep/wake, or power behavior.

## Commit

`chore: brand firmware as Atlas Lite`

## Limitations and preserved state

- No push, PR, merge, deployment, or release was performed.
- `target/.rustc_info.json`, generated `target/` artifacts, and existing ZIP
  extras were deliberately left unstaged.
- Existing untracked bootstrap extras were left untouched.

## Corrective verification after global review

The earlier embedded-build paragraph is superseded because it did not record
the target identity or inspect the artifact format. The final repository pins
the Rustmix ESP-IDF `v5.4.3` / global-tools setup and `ldproxy`, and the build
helper now selects the Xtensa target explicitly.

Final verification on 2026-09-04:

```text
./scripts/validate.sh: PASS, exit 0, 320 passed; 0 failed
./scripts/build.sh: PASS, exit 0
embedded-build-target=xtensa-esp32s3-espidf
embedded-build-artifact-type=ELF 32-bit LSB executable, Tensilica Xtensa
embedded-build-artifact-sha256=54e27ab505ae0cac9962968586c8192d405582a467aac26ffc250f7bc824af07
```

This confirms target compilation, not physical hardware behavior; all M1
physical checks remain **NOT TESTED**.
