# Atlas Lite simulation and verification

The native simulator is a separate host-only Cargo package under
`tools/atlas-lite-sim/`; the root firmware package does not declare or select
it. It reuses the product `FrameBuffer -> AppState ->
AppState::apply(ButtonEvent) -> app::render_current_screen` path. It presents
the logical portrait canvas at `480 x 800` and exposes the real native packed
framebuffer at `800 x 480` (`48,000` bytes). Rendering is headless and
deterministic; no window library or ESP-IDF driver is linked.

## Launch

Run the host-only simulator with:

```bash
./scripts/sim.sh
printf 'down\nenter\nesc\n' | ./scripts/sim.sh
```

With no input it reads stdin until EOF and prints a deterministic route and
frame summary. `scripts/sim.sh` selects the local Rust host target and keeps
the simulator package outside the firmware build graph. Its generated
`target/` and `.atlas-lite-toolchain/` contents are ignored.

The repository scripts source `scripts/rust-toolchain.sh`, which discovers
rustup and resolves the stable/esp toolchains without requiring a global
`cargo` or `rustc` on `PATH`. It also exposes the existing `cargo +stable` and
`cargo +esp` forms and adds Cargo-installed tools such as `ldproxy` when they
are present. Missing rustup or a requested toolchain produces an explicit
setup error.

Keyboard semantics are represented by `SimulatorKey`: Up/Down are rotary
previous/next, Enter selects, Escape backs hierarchically, H/Home returns to
the product Home, and P opens the simulated power menu. The semantic mapping
is tested independently of physical key names.

The fixed host model includes Display (`480 x 800` logical, `800 x 480`
native), Input, SD (`mounted`, `missing`,
`error`), Wi-Fi (`connected`, `connecting`, `offline`, `failed`), Battery
(`100%`, `50%`, `10%`), RTC, and Atlas connection (`unconfigured`, `connecting`,
`connected`, `unauthorized`, `forbidden`, `timeout`, `server_error`, `offline`).
It contains no credentials or secret-bearing fields. Selecting a fake hardware
snapshot applies its Wi-Fi, SD, battery, and RTC values to the real
`AppState` snapshots used by the product renderer; Atlas connection remains a
host transport seam until M2.

## Evidence boundaries

`HOST TESTED` means native Rust code and scripts ran on the development host.
`SIMULATOR TESTED` means the host simulator reused the product framebuffer,
router, state transitions and renderers. It proves application navigation,
placeholder rendering, geometry bounds, selected-row ink, deterministic labels,
secret-free diagnostics, and byte-identical repeated frames. Headless tests are
the CI-grade simulator evidence; an interactive window smoke, if available,
is only a launch check.

`TARGET BUILD TESTED` means the ESP32-S3 firmware compiled. It does not prove
the panel, PMIC, SD card, RTC, radio, audio, buttons, timing, power or startup
on a board. `QEMU TESTED` and `HARDWARE TESTED` require their own evidence and
are never inferred from host, simulator or build output.

The simulator does not prove electrical behavior, e-paper refresh timing or
ghosting, ESP-IDF startup, Wi-Fi transport, SD reliability, battery readings,
audio, sleep current or panel wiring. It never instantiates ESP-IDF drivers.

## Headless coverage

The simulator-focused Rust tests render Home plus Library, Search, Views,
Capture and Settings placeholders. They assert logical/native dimensions,
in-bounds framebuffer output, distinguishable selected rows, semantic route
navigation, hierarchical Back, stable fake-hardware labels, secret redaction,
and byte-identical repeated rendering. Framebuffer bytes are preferred over
brittle content snapshots.

## Fake hardware states

Display is fixed at logical `480 x 800` and native `800 x 480`. Input is
semantic. SD is `mounted`, `missing` or `error`; Wi-Fi is `connected`,
`connecting`, `offline` or `failed`; battery is `100%`, `50%` or `10%`; RTC has
ready/unavailable/integrity-lost states; Atlas is `unconfigured`, `connecting`,
`connected`, `unauthorized`, `forbidden`, `timeout`, `server_error` or
`offline`. These snapshots contain no credentials.

## QEMU and Wokwi

QEMU is `DEFERRED` for M1.5. A strictly bounded availability check found no
non-invasive ESP32-S3 execution path for this current board ELF; continuing
would require board-peripheral emulation. No custom drivers, SSD1677/PMIC/audio
models or custom Wokwi chips were created. Wokwi is future-only and physical
hardware verification remains a separate milestone/evidence class.
