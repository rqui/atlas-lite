# EINK-PHYSICAL-UI-REPAIR-03

## Scope and physical finding

This correction was extended after physical validation at `0f9d31ec`. It now
also replaces the undersized Home bitmap strikes and moves essential display
and regional preferences to internal NVS. Atlas HTTP, the serialized worker,
Books API contracts, Reader geometry, SD drivers and SSD1677 transfer commands
remain unchanged.

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

The supplied photograph was treated as the visual specification, with the
physical follow-up taking precedence: a solid black masthead, white brand and
status, dominant editorial hero, nearly full-width flat navigation, six local
monochrome icons, thin separators, one black selected row and inverted badges.
There are no cards, grid, rail or permanent developer control footer.

The centralized logical `AtlasHomeGeometry` is based on a 480x800 portrait
surface:

- horizontal margin: 10 px;
- solid black topbar: 0..56;
- hero: 62..172 (110 px), with the supplied 456x106 monochrome Atlas mark
  centered at `(12,64)`;
- section baseline/divider: 203 / 214;
- six flat rows: top 216, 84 px each, ending at 720;
- no product Home footer.

The representative Home fixture supplies 20:57, connected Wi-Fi, 100% battery,
4 Library roots / 19 notes, 12 Books, 68% progress and Library as
the selected row. Rendering remains snapshot-only; Home performs no request for
those values. With no SD, `DisplayPreferences::default()` still resolves the
firmware-local product strikes.

The former effective default raster line heights were 18/23/29/34 px and
Standard and Large incorrectly resolved to the same `*_LARGE_*` constants.
The generator now produces distinct physical 1-bpp strikes (no runtime scale).
The effective Standard line heights printed at boot are:

- status/detail: 20 px;
- body/brand/badges: 24 px;
- menu heading: 32 px;
- hero: 44 px;
- default Reader Serif Large: 28 px.

Large resolves to 22/28/36/48 px and Compact retains the smaller family-specific
strikes. Tests compare real `line_height()` values for every role. The topbar
uses the exact official 29x32 `draw_atlas_mark` bitmap from `0f9d31ec`; the
temporary 34x34 adaptation was removed. Home icons remain documents, open book,
magnifier, grid, microphone and sliders.

The Home hero is a build-time 1-bit conversion of the supplied 2804x561 artwork
(source SHA-256 `b48fbe5f71d1018533257f81727ab759fe793087d6245c8dd274360329837179`).
The generator removes the near-white source canvas, whose mark bounds are
880x205, scales that mark to 360x84 and centers it inside the 456x106 embedded
bitmap. It preserves the mark's aspect ratio to within one output pixel and
needs no PNG decoder, SD card or network access.

## Home summary warmup

After the initial frame is visible and Wi-Fi plus the configured Atlas client
are ready, Home queues the existing bounded Library and Books list loaders as
one deduplicated warmup. The single serialized Atlas worker executes them in
sequence. Completion updates both snapshots and causes at most one refresh,
only when Home, Library or Books is still visible. It never fetches manifests,
progress, bookmarks or segments merely to decorate Home.

A six-byte versioned record in the separate `atlashome` NVS namespace stores
only Library root/note counts, their partial flag, Books count/`has_more`, and
an optional already-known resume percentage. It contains no titles, bodies,
IDs or book content. Boot hydrates Home from this last-known summary before
Wi-Fi; a successful network warmup rewrites NVS only when those bytes changed.

## Essential preferences without SD

The SD-only `DISPLAY.TXT` path caused font size to reset whenever `/sdcard` was
unavailable. The firmware now owns a separate `atlasui` NVS namespace with a
bounded versioned record for font family, font size, timezone and temperature
unit. On first boot without this record, a valid `DISPLAY.TXT` is imported once;
otherwise safe defaults are persisted. Later changes write NVS directly.
Factory reset clears this namespace, while Wi-Fi reset and Atlas unpair do not.

The neutral global default remains UTC. Display settings now exposes timezone
selection and supports `UTC`, `Europe/Madrid` (EU DST rules) and
`America/New_York`; no location is hardcoded globally. SD configuration remains
a migration compatibility source, not the availability boundary for essential
product settings. Existing SDMMC `ENODEV` and voice-delivery storage failures
remain visible and are deliberately left for separate storage work.

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
  --framebuffer-pgm dist/visual-evidence/atlas-home-logo-warmup-06.pgm
printf 'fixture=reader\n' | ./scripts/sim.sh --headless \
  --framebuffer-pgm dist/visual-evidence/atlas-reader-reference.pgm
```

The reviewed Home frame is exactly 480x800. It contains the black topbar, visible
white official Atlas mark and ATLAS brand, right-built status group, centered
Atlas hero mark, 32 px menu labels, six visible icons, flat list, black
selected row and no cards or developer footer.

The reviewed Reader frame uses multiple paragraphs, fills all 24 derived lines,
places long proportional lines near the right margin and ends with
`g p q y j`. Tests assert that every source line is no wider than 460 px, side
guard bands contain no body ink and the final line remains entirely above the
bottom bound.

Evidence hashes:

- Home PGM: `8b6457588878db7aa14ea490e095c54194d5d931d5b2c0f9481e4acbdb140a88`;
- Home PNG: `4a8ee6055f10151f6e22f189e2c1d8ffe9e94223bcb872d6a6b06cc674f01300`;
- Reader PGM: `59ab2e7985fcf4a5d072d4ba5aa6cd5b62d7aa1cb97a566a48c4040f4594c857`.

## Validation performed

- `./scripts/test-host.sh`: pass, including 453 library unit tests and all
  integration binaries;
- focused Atlas Books integration: 6/6 pass;
- focused Home geometry/rendering: 5/5 pass;
- focused Reader pixel-wrap, descender, remote-frame and root-Back tests: pass;
- `cargo +stable fmt --all -- --check`: pass;
- `git diff --check`: pass;
- clean isolated `cargo +esp build --release --target
  xtensa-esp32s3-espidf`: pass; output is a statically linked 32-bit Tensilica
  Xtensa ELF of 2,546,576 bytes with SHA-256
  `43da4d3b7299ef0df5d4472189f9500d9ad5f58fd693e0347058cabb91dc851b`.
  The logo replacement adds 4,096 bytes (0.16%) over the prior 2,542,480-byte
  ELF. The bitmap stays in flash and adds no decoded runtime image buffer.

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
