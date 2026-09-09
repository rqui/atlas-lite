# EINK-UI-RESTYLE-02

## Reference and Home

The physical reference uses a very short white status strip, an oversized
`Home` hero, a readable sentence below it, a flat list, thin separators,
compact count badges, and exactly one inverted active row. Atlas Lite now uses
that language rather than its former bordered-card grid.

Logical portrait Home geometry is: status `0..48`; hero title baseline `96`;
hero copy baseline `138`; divider `158`; Library `178..270`; Books `270..362`;
then Search, Views, Capture and Settings in four 78-pixel rows ending at 674.
Only Library and Books own 42x30 count badges. Home remains snapshot-only and
does not issue requests to calculate those values.

## Typography

No bitmap raster is scaled. The default persisted `Standard` UI mapping moves
one actual raster strike up: Detail `17 -> 18`, Body `20 -> 23`, Heading
`26 -> 29`, Large `31 -> 34` for Inter (with equivalent Atkinson strikes).
`Compact` remains an explicit density setting. Reader's default moves from
Medium to Large, using the 28-pixel serif/Literata strike; existing Medium is
also rendered with the larger reader role so old preferences become legible.

## Reader geometry

The local Reader has a 36-pixel compact header and a 28-pixel optional footer.
With progress shown its text viewport is `x=14..466`, `y=39..769`: 452 by 730
pixels. With progress hidden, it extends to `y=797`. The previous shared
status/card/frame consumed a 70-pixel header, 42-pixel status card, frame and
24-pixel side margins. The new Large serif layout is 25 columns by about 21
logical lines per page; the physical viewport can show roughly 24 rendered
28-pixel lines before the pagination budget is reached. Atlas Books uses the
same near-full-screen reading surface and no footer control reservation.

## Memory and workers

ESP-IDF's `uxTaskGetStackHighWaterMark` calls `prvTaskCheckFreeStackSpace` and
therefore reports the minimum stack *free* since task start, not bytes used.
The observed `61612` from a 65536-byte Atlas worker means roughly 3924 bytes
were consumed at the measurement point.

Atlas HTTP remains one persistent serialized 65536-byte worker; it was not
returned to per-request tasks. The remaining OOM was in voice delivery: its
temporary 65536-byte task constructed `Self::new`, which allocated a second
persistent 65536-byte Atlas worker inside it. Voice delivery now performs its
existing direct streaming upload without that nested worker and uses a
24 KiB task with its own minimum-free-stack log. This is an evidence-based
allocation reduction, but voice TLS high-water remains physical validation.

## Other repairs

Long Back at root Home already has an ignored route path. Its guard is retained
and its no-refresh condition is explicit: no route change and no panel refresh.
The product default regional profile is now neutral `UTC`, rather than the
sample-specific `America/New_York`; topbar time continues to read the persisted
regional state and never schedules a clock-only refresh.

SD `ENODEV` after a successful mount remains a real bus/card health warning.
It is not hidden. Remote Atlas Books remains independent of `/sdcard`.

## Validation and deferred work

Host geometry tests cover the compact status bar, hero/list bounds, six
navigable Home rows and inversion. Reader geometry tests cover the 452-pixel
viewport. Target compilation and physical checks remain required for actual
font appearance, voice stack margin, repeated Books sequences, SD removal and
root Back panel trace.

`EINK-PARTIAL-WINDOW-01` remains separate. The current coordinator still uses
fullscreen partial transfer; this change avoids no-op refreshes but does not
alter SSD1677 commands or introduce partial-window updates.
