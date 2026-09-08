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
raw connection enums are not user-facing. Books has deterministic local cover
cards without image downloads; the physical follow-up enlarges those covers to
78 x 112 px so they read as book covers rather than small list icons. Library
uses eleven 56 px rows with the large local UI strike. A short `Select` on a
parent expands/collapses its children, holding `Select` opens the parent note,
and a short `Select` on a leaf opens it normally. Long BOOT still returns.

## Panel performance

Existing timing logs retain render and fullscreen partial-transfer duration.
The current driver has evidence only for `PartialFullscreen`; no SSD1677
regional-window sequence is enabled. See `EINK-PARTIAL-WINDOW-01` for required
button/state/render/transfer/BUSY physical traces before that experiment.

## Validation limits

`./scripts/test-host.sh` passed (433 unit tests plus all integration suites),
including the Books flow without an SD card. The focused transport suite adds
the persistent-worker source contract and passes 16 tests.
`cargo +esp build --release --target xtensa-esp32s3-espidf` completed for the
real ESP32-S3 target. The release ELF is 2.4 MiB in this environment.

`cargo +esp clippy --release --target xtensa-esp32s3-espidf -- -D warnings`
still fails on 33 pre-existing repository-wide diagnostics (for example
`alarm.rs`, typography, cache, queue and reader); no unrelated cleanup was
folded into this repair.

Worker stack high-water, heap evolution, actual HTTP statuses and SD-card
removal remain mandatory physical-device validation.
