# EINK-PHYSICAL-UI-REPAIR-03

## Scope and physical finding

This correction starts at `fa83d2c6`. It changes Home composition, Reader
geometry, framebuffer evidence and diagnostics only. Atlas HTTP, the serialized
64 KiB worker, Books API contracts, SD drivers and SSD1677 transfer commands are
unchanged.

The physical log was misleading. It still announced
`home-dashboard-redesign-ready ... cards=high-contrast` and
`reader-high-contrast-layout-ready ... clip=right,bottom`, although those
strings no longer described the renderers selected by the router. Both markers
are removed and replaced with effective layout data.

There are two Home source files, but only one is executable for the product
root. The exact call path is:

`firmware::run` / input loop -> `refresh_screen` ->
`app::render_current_screen` -> `OrientedFrameBuffer(Portrait)` ->
`screens::render_active_screen` -> `ScreenRoute::Home` ->
`AtlasRoute::Home` -> `atlas_home::render_atlas_home` -> native packed
`FrameBuffer` -> `Epaper397::show_base` or `show_partial_fullscreen`.

`screens/home.rs::render_home` is legacy source and is not selected by
`render_active_screen`. Therefore a physical cards/grid Home cannot be produced
by the current `fa83d2c6` dispatch path. The old serial marker itself was stale;
if cards were also visible, the flashed artifact did not contain the expected
framebuffer implementation. The prior runtime did not print a Git SHA, so the
exact older artifact cannot be identified retrospectively from that log alone.
The new unique `atlas-home-reference-ready` and `ui-fonts` markers make the next
physical run distinguishable.

## Reference inventory and Home geometry

The supplied photograph was treated as the visual specification. Its useful
language is a compact white status row with time/date/Wi-Fi/battery, small Atlas
brand, dominant editorial hero, immediate section label, nearly full-width flat
navigation, thin separators, one black selected row, compact inverted badges,
small side margins and a light bottom control strip. It does not use a black
header, card dashboard, primary/secondary card grid or a large `Home` label.

The centralized logical `AtlasHomeGeometry` is based on a 480x800 portrait
surface:

- horizontal margin: 10 px;
- status: 0..48;
- hero: 54..174 (120 px), with `Capture that` / `thought.`;
- section baseline/divider: 204 / 216;
- six flat rows: top 222, 72 px each, ending at 654;
- existing real physical-control footer: top 746.

The representative Home fixture supplies 8:24 AM, Thu Jan 1, connected Wi-Fi,
20% battery, 4 Library roots / 19 notes, 12 Books, 68% progress and Library as
the selected row. Rendering remains snapshot-only; Home performs no request for
those values. With no SD, `DisplayPreferences::default()` still resolves the
firmware-local product strikes.

The effective default raster line heights printed at boot are:

- status/detail: 18 px;
- body/menu metadata: 23 px;
- menu heading: 29 px;
- hero: 34 px;
- default Reader Serif Large: 28 px.

Those are the same product strikes selected by the prior default preferences;
the repair changes composition and makes the effective selection observable,
not the bitmap assets themselves. The old firmware logged an aspirational
layout marker rather than these resolved strikes. With the repair, the boot
line reports the active profile and all five effective raster heights.

## Reader root cause and unified viewport

The Atlas remote Reader call path is:

`Home Select Books` -> bounded `books-list` -> `book-manifest` ->
`reading-progress` -> `bookmarks` -> `book-segment` ->
`AtlasBooksState::page_from_segment` -> `paginate_reflowable_text` ->
`render_active_screen` -> `atlas_books::render_atlas_books(Reader)` ->
`OrientedFrameBuffer` -> native 800x480 framebuffer -> panel transfer.

Local TXT and EPUB use `ReaderPreferences::layout`, their chapter/cache readers,
`screens::reader::render_page`, the same orientation adapter and the same panel
transfer.

Previously pagination stopped at a hard-coded character count while final draw
used proportional bitmap advances and a separate clip rectangle. A legal line
could therefore be wider than the physical viewport and lose its right-hand
pixels. Vertical page counts were also hard-coded independently of glyph
ascent/descent. The local renderer rejected only a baseline at or beyond the
bottom, not a glyph box whose descender crossed it. In the observed default,
the conservative 21-line budget also left about 100 px unused, creating the
bottom-cut/under-filled appearance.

Old default geometry was divergent:

- Atlas remote clip: `x=14..466`, `y=51..792` (452x741);
- Atlas remote baselines: first 80, last budgeted 680 at 28+2 px;
- local clip: `x=14..466`, `y=39..769` (452x730);
- local baselines: first 67, last budgeted 667;
- wrap: 25 characters for default Serif Large, regardless of glyph width.

`ReaderViewport` is now the single logical geometry used to derive wrap width,
line count, pagination, drawing and the final safety clip. For default portrait
with progress hidden it is:

- logical screen: 480x800 over a native 800x480 packed framebuffer;
- header: 48 px;
- footer: 0 px;
- viewport: `x=10`, `y=54`, `w=460`, `h=742`;
- line height/step: 28 / 30 px;
- first/last rendered baseline: 84 / 774;
- lines: 24;
- final guard: `x=10..470`, `y=54..796`.

With progress enabled, the footer is exactly 28 px, the viewport bottom is 768
and the derived default page has 23 lines with final baseline 744. Turning
progress off reclaims those 28 px; no invisible footer remains.

Every character is now measured with the same bitmap strike used for drawing.
The line is advanced to the next page before exceeding 460 px. Vertical extent
uses the strike's stored glyph boxes plus the bounded accent/cedilla extension;
the last baseline is rounded down to a complete line. The clip remains only a
framebuffer safety guard and is logged as `clip=none` because valid pagination
does not depend on it.

Portrait rotation is applied exactly once in `OrientedFrameBuffer`:
logical `(x,y)` maps to native `(y,479-x)`. A host-only test draws the complete
`(0,0)-(479,799)` border and Reader viewport corners, proving all four logical
corners map inside the native 800x480 frame.

## Back and performance telemetry

`AppState::apply_hierarchical_back` now returns whether Back changed meaningful
UI state. At root Atlas Home it returns false. Firmware skips `refresh_screen`,
and the simulator leaves its dirty flag false. Logs now include both the main
and Atlas sub-route so leaving Books no longer appears as `home -> home`.

Input and display timing retain the existing fullscreen-partial implementation
and add explicit `button-received`, `state-updated`, `render-start/end`,
`transfer-start/end` and `busy=released` stages. No SSD1677 partial-window
commands changed; that remains `EINK-PARTIAL-WINDOW-01`.

## Host framebuffer evidence

Generate the exact product framebuffer with:

```sh
printf 'fixture=home\n' | ./scripts/sim.sh --headless \
  --framebuffer-pgm dist/visual-evidence/atlas-home-reference.pgm
printf 'fixture=reader\n' | ./scripts/sim.sh --headless \
  --framebuffer-pgm dist/visual-evidence/atlas-reader-reference.pgm
```

The reviewed Home frame is visually comparable to the supplied reference in
composition: compact status, secondary Atlas brand, two-line hero, flat list,
thin dividers, one full-width black row, inverted badge and minimal chrome. It
does not reproduce the photograph's decorative icons or rounded cards because
the requested Atlas adaptation explicitly requires a flat list and no fake
actions.

The reviewed Reader frame uses multiple paragraphs, fills all 24 derived lines,
places long proportional lines near the right margin and ends with
`g p q y j`. Tests assert that every source line is no wider than 460 px, side
guard bands contain no body ink and the final line remains entirely above the
bottom bound.

Evidence hashes:

- Home PGM: `66494984357d6bfe3ebeef43c0d2e48c7cc4c348e0b935336b3f7c4ae5b13050`;
- Reader PGM: `59ab2e7985fcf4a5d072d4ba5aa6cd5b62d7aa1cb97a566a48c4040f4594c857`.

## Validation performed

- `./scripts/test-host.sh`: pass, including 439 library unit tests and all
  integration binaries;
- focused Atlas Books integration: 6/6 pass;
- focused Home geometry/rendering: 5/5 pass;
- focused Reader pixel-wrap, descender, remote-frame and root-Back tests: pass;
- `cargo +stable fmt --all -- --check`: pass;
- `git diff --check`: pass;
- clean isolated `cargo +esp build --release --target
  xtensa-esp32s3-espidf`: pass; output is a statically linked 32-bit Tensilica
  Xtensa ELF with SHA-256
  `24f19a707966975e720daaae0047166bf9ca4ebcecce1d04e6a5ad7d5bbb3458`.

The repository currently versions a large historical `target/` tree containing
a recursive `esp-idf-sys .../out/target` copy. A second in-place build can hit
macOS `File name too long`; the final target proof temporarily excluded that
generated tree and used a clean external Cargo target directory. No generated
`target/` change is included in this repair.

## Validation boundary

Host tests and generated frames prove deterministic application geometry and
logical/native mapping. The ESP32-S3 build proves target compilation only.
Physical validation is still mandatory for perceived raster size, panel
orientation, ghosting, the new serial markers and timing, and button/refresh
behavior. No deploy, release, flash or hardware claim is part of this task.
