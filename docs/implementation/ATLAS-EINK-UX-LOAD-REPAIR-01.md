# ATLAS-EINK-UX-LOAD-REPAIR-01

## Observed causes

`NOT LOADED` was a UI label for an unconfigured or not-yet-fetched bounded
snapshot, not a server response. It obscured the distinction between first
load, empty data, offline failure and denied access. Books had the same
problem: all non-403 failures became `BOOKS REQUEST FAILED`.

The target transport also created and joined a new 64 KiB `atlas-https` task
for every request. On the physical ESP32-S3, after allocation churn, the
largest internal free block was about 64.5 KiB; task creation consequently
failed with ENOMEM before a request could be sent.

## Repair

The transport starts one serialized Atlas HTTP worker when the validated Atlas
configuration is constructed. Each typed request uses a bounded reply channel;
there is one active transaction, and the HTTP connection/response is dropped
before the next work item. The worker logs its own FreeRTOS stack high-water
mark and internal heap/largest-block at start and after every request. The
64 KiB stack was deliberately not reduced without physical high-water
evidence; the improvement is removal of repeated task allocation.

Structured `atlas-http` records identify `library-list`, `books-list`,
`book-manifest`, `reading-progress`, `bookmarks`, and `book-segment` with a
redacted logical path, status, bounded body bytes, duration and transport
result. Secrets, token and book IDs are never logged.

## Storage

Remote Atlas Books does not instantiate the local Reader EPUB session. Online
reading remains independent from `/sdcard`; a healthy card now optionally owns
bounded stale list, manifest, progress, bookmark and recent-segment records in
`/sdcard/ATLAS/CACHE/BOOKS`. A mounted VFS that then returns `ENODEV` causes the
cache repository probe to fail visibly and the remote route continues without
persistence. Atlas remains authoritative and corrupt/incomplete cache
inventories fail closed.

## UI

Home uses the supplied monochrome hero followed by a compact six-row menu. The
redundant second `Atlas` label was removed; the hero's visible ink has equal
19 px whitespace above and below before the first menu row.
Counters are derived only from snapshots; unknown is `—`, bounded incomplete
data is `N+`, and no Home render fetches.

Library and Books display `Loading…`, `No notes`/`No books`, `Offline`,
`Authorization required`, or `Unable to load` as applicable. `NOT LOADED` and
raw connection enums are not user-facing. When an EPUB contains a supported
cover, Books requests Atlas Server's fixed 104 x 142 monochrome PBM rendition
while opening the book, validates its exact header and payload length, persists
it in the bounded SD cache and uses the deterministic local monogram as a
fallback. No general image decoder or attacker-controlled bitmap dimensions
are introduced on the ESP32-S3. Library
uses twelve framed 50 px hierarchy rows with the heading strike, explicit
expand/collapse glyphs, indentation and child-count badges. A short `Select` on a
parent expands/collapses its children, holding `Select` opens the parent note,
and a short `Select` on a leaf opens it normally. A short BOOT now moves back
one local level; holding BOOT consumes the existing cleanup/back transitions
until Atlas Home is reached. Search's former BOOT keyboard-axis action moves to
held `Select` so the two-axis keyboard remains usable.

The Books cache remains deliberately bounded to metadata, the cover and recent
segments. It is not a complete downloaded EPUB: a book can only continue
offline through segments already cached. Full-book pin/download is separate
work because it needs a larger, independently bounded SD layout and download
state rather than silently weakening the 512 KiB general cache budget.

## Panel performance

Existing timing logs retain render and fullscreen partial-transfer duration.
The current driver has evidence only for `PartialFullscreen`; no SSD1677
regional-window sequence is enabled. See `EINK-PARTIAL-WINDOW-01` for required
button/state/render/transfer/BUSY physical traces before that experiment.

## Validation limits

`./scripts/test-host.sh` passed (454 unit tests plus all integration suites),
including the Books flow without an SD card. The focused transport suite adds
the persistent-worker and e-ink-cover contracts and passes 16 tests; the Books
integration suite passes 9 tests, including live-cover persistence and offline
reopen.
`cargo +esp build --release --target xtensa-esp32s3-espidf` completed for the
real ESP32-S3 target. The release ELF grew from 2,673,552 to 2,681,744 bytes
(8,192 bytes, about 0.31%) in this environment.

`cargo +esp clippy --release --target xtensa-esp32s3-espidf -- -D warnings`
still fails on 33 pre-existing repository-wide diagnostics (for example
`alarm.rs`, typography, cache, queue and reader); no unrelated cleanup was
folded into this repair.

Worker stack high-water, heap evolution, actual HTTP statuses and SD-card
removal remain mandatory physical-device validation.

## Physical follow-up: ESP-IDF virtual root

The first hardware run with a repaired FAT card mounted `/sdcard` and created
the fixed `/sdcard/ATLAS` layout, but Atlas Capture still reported a storage
write failure. Its background delivery scan independently returned
`NotFound`. The capture-specific parent-symlink guard walked beyond the valid
mount point and required metadata for ESP-IDF's synthetic `/` VFS root. The
host filesystem exposes that entry; this ESP-IDF VFS does not.

Root validation now accepts `NotFound` only for the terminal filesystem root
whose `Path::parent()` is absent. Every addressable ancestor, including
`/sdcard`, remains checked and any missing inner ancestor, symlink, permission
failure or other I/O error still fails closed. Atlas Capture also logs the
secret-free error kind and errno for every storage-start I/O failure, rather
than emitting those diagnostics only for `EIO` or `ENODEV`.
