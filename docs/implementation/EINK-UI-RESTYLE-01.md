# EINK-UI-RESTYLE-01

## Scope

Restyle the existing Atlas Home, Library and Books routes as a bounded,
monochrome e-paper interface.  The work uses only state already owned by the
device: it introduces no request, endpoint, metadata source, cover download,
or change to reader anchors and pagination.

## Plan

1. Replace the product shell with one 70px Atlas top bar: mark and context on
   the left; RTC time, vector Wi-Fi state, vector battery and percentage on
   the right. Unknown values are rendered as `--:--` and `--%`.
2. Remove routine `ONLINE`/`LIVE`/`REMOTE` strips. Retain a compact message
   only for actionable exceptional state (syncing, offline cache, error,
   re-pair, or partial data).
3. Make Home a six-row information hierarchy and derive counts strictly from
   the bounded snapshots. A partial library/book list is labelled with `+`,
   never presented as an authoritative total.
4. Keep Library hierarchy, selection and collapse behavior intact while adding
   aligned known direct-child counts.
5. Make Books a card/detail/reader hierarchy. Progress is displayed only when
   it was returned by the existing progress request for the opened book.
6. Add host tests for top-bar fallbacks, bounded list positions/counts and the
   existing navigation behavior. Validate every supported typography profile.

## Refresh investigation

The current renderer is deterministic and operates before the single
`PanelRefreshCoordinator` transfer boundary. The panel driver currently has
evidence only for fullscreen partial refresh. This change records render and
transfer timings but does not guess at SSD1677 regional-window commands.

`EINK-PARTIAL-WINDOW-01` remains a hardware follow-up: it requires traces for
button capture, state transition, framebuffer render, panel transfer start and
complete, plus BUSY release, on the physical panel before a windowed command
sequence may be introduced.

## Non-goals

No network traffic from rendering, no fixed cover art, no API contract change,
no reader re-pagination, no firmware release or flashing.
