# Atlas Lite M1 hardware validation record

**Record date:** 2026-09-03

**Target:** Waveshare ESP32-S3-ePaper-3.97

**Firmware source SHA documented:** `b025d18dfaa53b5b88282f47886a531485bdb1cb` (`chore: brand firmware as Atlas Lite`)

**Board identity:** `UNKNOWN / NOT TESTED`
**Board revision:** `UNKNOWN / NOT TESTED`

## Scope and evidence boundary

No physical Waveshare board, connected serial device, or flash target was
available in this session. The firmware was not flashed. Consequently, this
record contains no port, serial log, photograph, measurement, or inferred
device behaviour. Every physical check below is independently `NOT TESTED`.

The source SHA above is the M1 firmware revision evaluated before this
evidence-only document is committed. It identifies the firmware to be tested
when a board is available; it is not a statement that the firmware ran on a
device.

## Physical validation matrix

| Check | Physical result | Auditable reason / evidence |
| --- | --- | --- |
| boot | **NOT TESTED** | No physical board or serial/flash session was available. |
| e-paper first frame | **NOT TESTED** | No physical board was available to observe a panel frame. |
| partial refresh behavior | **NOT TESTED** | No physical board was available to observe refreshes or ghosting. |
| rotary movement | **NOT TESTED** | No physical board was available to operate the rotary control. |
| rotary select | **NOT TESTED** | No physical board was available to operate the rotary select. |
| BOOT short/long behavior | **NOT TESTED** | No physical board was available to exercise GPIO0 press durations. |
| Power behavior | **NOT TESTED** | No physical board was available to exercise the physical Power key or PMIC. |
| SD mount/detection | **NOT TESTED** | No physical board or removable SD card was available. |
| RTC read | **NOT TESTED** | No physical board was available to read the board RTC. |
| battery/charge snapshot | **NOT TESTED** | No physical board was available to read battery or charge state. |
| Wi-Fi connect behavior | **NOT TESTED** | No physical board was available to attempt a Wi-Fi connection. |
| sleep | **NOT TESTED** | No physical board was available to enter and observe sleep. |
| wake/restore | **NOT TESTED** | No physical board was available to observe wake or route restoration. |

## Non-physical evidence

### Host validation

With the required environment loaded, `./scripts/validate.sh` passed on
macOS `26.6.2` / `arm64`. It used stable `rustc 1.98.1`, stable `cargo 1.98.1`,
and selected the native target `aarch64-apple-darwin`; the host suite reported
`317 passed; 0 failed`. `git diff --check` also passed. Host validation checks
formatting, source contracts, and native library tests; it does not access a
Waveshare board. The full command record is in
`.superpowers/sdd/2026-09-03-atlas-lite-m0-m1/task-6-report.md`.

### Embedded build evidence already available

The prior Task 5 record for the same current firmware SHA reports that the
following documented command completed successfully:

```bash
./scripts/build.sh
```

That script first runs `./scripts/validate.sh` and then runs
`cargo +esp build --release`. This is ESP-IDF target-compilation evidence for
the documented source SHA only. The configured ESP toolchain identifies as
`rustc 1.97.0-nightly` / `cargo 1.97.0-nightly` with host
`aarch64-apple-darwin`. It does not flash firmware and is not evidence for any
row in the physical matrix.

### Relevant implementation, not hardware proof

The documented source retains the shared `src/panel_refresh.rs` coordinator,
native board-service ownership, and the existing power, SD, RTC, and Wi-Fi
paths. Their presence and host-test coverage do not establish the electrical
identity, revision, wiring, peripheral response, or observed behaviour of an
unavailable board. No driver was changed to reconcile a PMIC label.

## Required follow-up on a real board

Use the documented firmware SHA (or record a newer SHA explicitly), identify
the physical board/revision, flash through an observed session, and replace
each individual result only with direct evidence. A target build must continue
to be reported separately from those physical observations.
