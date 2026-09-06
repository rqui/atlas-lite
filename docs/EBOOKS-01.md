# EBOOKS-01 — Remote EPUB reader

## Boundary

Books are fetched from Atlas through the existing authenticated HTTPS client.
Atlas Lite never receives pre-paginated pages and does not require microSD.
The device owns the `480 x 800` viewport, typography, wrapping, page turns and
e-paper refresh decisions. It deliberately reuses the existing Reader line
pagination and anchor model rather than adding a second layout engine.

Only safe `heading`, `paragraph` and `break` blocks are accepted. The device
does not accept EPUB ZIP, HTML, CSS, scripts, resources or fonts from the
network. The server contract is documented with the matching endpoint limits
in Atlas `docs/implementation/EBOOKS-01.md`.

## Bounded remote model

| Item | Limit |
| --- | ---: |
| Books list rows | 32 |
| Manifest spine / TOC rows | 128 / 128 |
| Segment blocks | 24 |
| Segment response | 24 KiB |
| One block text | 2,048 UTF-8 bytes |
| In-memory retained segments | current plus one adjacent |
| Remote page cache | 8 pages |

An anchor is `{ spine_item, block, character_offset }`. It is the canonical
resume/bookmark position; local page counts are never synchronized. Changing
font size repaginates from the retained logical blocks. Progress synchronizes
on a chapter change, Reader exit, and every eight page turns. Failed writes do
not interrupt reading; they remain available for the next session-safe retry.
No SD persistence is claimed in this release.

## Navigation and power

Books is a top-level Atlas Home item. It opens a list, book detail, TOC,
bookmarks and the reader. In the reader, Down advances, Up goes back, Select
saves a bookmark, and long BOOT returns hierarchically. Remote fetch and render
operations use the existing sleep-inhibitor boundary. A cached page never
holds Wi-Fi awake, and one Down event after light sleep remains exactly one
page action.

## Scopes and pairing

New pairings request `books:read` and `reading:write`. Existing pairings are
not altered. A valid Atlas credential that receives `403 AUTH_FORBIDDEN` from
a Books route displays **Re-pair device to enable Books** and keeps the current
NVS pairing intact, including Library, Search, Views and Capture access.

## Typography

The Reader font coverage includes the bounded Latin-1 and typographic set
needed for Spanish, Catalan and English: accented vowels, `ñ`, `ç`, `¿`, `¡`,
guillemets, curly quotes, en/em dash and ellipsis. This does not add CJK packs
or modify the microSD stack.

## Validation limits

Host tests cover DTO bounds, cursor validity, pagination, TOC/bookmark anchors,
font-size repagination, Latin glyph preservation, offline navigation from
cached pages, single reconnect-on-demand and the scope-upgrade message. The
existing input, sleep and panel-refresh tests remain part of the release
validation. Hardware, flash, current consumption and the existing microSD
first-access problem are outside this software validation.
