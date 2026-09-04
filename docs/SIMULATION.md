# Atlas Lite host simulation

The native simulator reuses the product `FrameBuffer -> AppState ->
AppState::apply(ButtonEvent) -> app::render_current_screen` path. It presents
the logical portrait canvas at `480 x 800` and exposes the real native packed
framebuffer at `800 x 480` (`48,000` bytes). Rendering is headless and
deterministic; no window library or ESP-IDF driver is linked.

Run it with:

```bash
./scripts/sim.sh
printf 'down\nenter\nesc\n' | ./scripts/sim.sh
```

Keyboard semantics are represented by `SimulatorKey`: Up/Down are rotary
previous/next, Enter selects, Escape backs hierarchically, H/Home returns to
the product Home, and P opens the simulated power menu. The semantic mapping
is tested independently of physical key names.

The explicit host model includes Display, Input, SD (`mounted`, `missing`,
`error`), Wi-Fi (`connected`, `connecting`, `offline`, `failed`), Battery
(`100%`, `50%`, `10%`), RTC, and Atlas connection (`unconfigured`, `connecting`,
`connected`, `unauthorized`, `forbidden`, `timeout`, `server_error`, `offline`).
It contains no credentials or secret-bearing fields.

The simulator proves application navigation, renderer reuse, geometry and
deterministic framebuffer output. It does not prove electrical behavior,
e-paper refresh timing/ghosting, ESP-IDF startup, Wi-Fi transport, SD
reliability, battery readings, or panel wiring. Hardware claims remain
separate evidence.

QEMU is `DEFERRED` for M1.5: booting the current ESP-IDF image would require
invasive board-peripheral emulation. Wokwi and custom chips are future-only
possibilities; no SSD1677 model is implemented here.
