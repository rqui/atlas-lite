# Atlas Lite: power and input contract

This is an implementation contract, not a physical-current result. Physical
validation remains **NOT TESTED**.

## States and timers

`Boot -> Active -> Wi-Fi suspended -> Light sleep -> Active` is the automatic
path. User interaction is the sole idle reset: background rendering, telemetry,
logging, battery polling and maintenance do not reset it. At 15 seconds of
interaction-idle the station/SNTP services are stopped. At 60 seconds the
firmware enters ESP-IDF light sleep, retaining RAM and the e-paper image. It
does not enter deep sleep automatically and does not show a splash on wake.

Before light sleep, the central refresh owner must be idle, the panel is put to
sleep through the existing driver, and Wi-Fi services are suspended. After wake
the panel is initialized and the refresh coordinator is reset. Wi-Fi is not
eagerly reconnected: cached navigation remains usable; the next operation that
needs Atlas owns reconnecting it.

## Sleep inhibitors

Light sleep is forbidden during recording, playback, WAV finalization, NVS or
SD writes, an HTTP request in flight, pairing, OTA, panel refresh, or pending
input. A durably persisted upload waiting for a future retry is **not** an
inhibitor. Unhealthy SD also prevents voice workers from being launched. An
empty voice-delivery outcome is not treated as a pending upload.

USB/VBUS is a development-mode inhibitor so the serial console remains awake.
Testing automatic sleep over USB is therefore an explicit future hardware test;
normal current/wake measurement must be performed on battery power.

## Wake sources

The implementation uses the active-low physical navigation keys on GPIO4,
GPIO5 and GPIO6 as low-level light-sleep wake sources. ESP-IDF documents GPIO
wake for light sleep on ESP32-S3 digital IO; this is not an EXT0/EXT1 or deep
sleep claim. GPIO0 BOOT remains runtime Back/context input but is not an
automatic wake source in this candidate. The AXP2101 Power key is polled over
I2C and is **not** claimed as a light-sleep wake source. GPIO45 RTC alarm is
also not used for light sleep or deep sleep wake.

The Waveshare schematic and ESP-IDF capability establish only a software/SoC
route. Board electrical wake, wake latency, key-held behaviour and current are
pending physical measurement.

## Input delivery

GPIO4/5/6 and GPIO0 use **both edges**. `esp-idf-hal 0.46.2`, `gpio.rs`,
`PinDriver::handle_isr` disables the interrupt before calling the subscriber;
`subscribe` explicitly requires rearming from **non-ISR** context after each
notification. UI-loop rearming could therefore capture only the first press
of a GPIO while a display or HTTP call blocked the UI.

`InputService` now owns all four pins in one `atlas-input` task (priority 8).
The ISR reads level/time and copies a raw edge into an 8-entry FreeRTOS queue;
it can request an ISR-safe scheduler yield. It does not debounce, rearm,
allocate, lock a Rust mutex, log, render or access the network. The service
drains notifications FIFO, rearms each pin outside ISR and reconciles levels
after rearming. The UI cannot operate these pins after ownership transfer.
BOOT's startup-only recovery check is unchanged.

The shared `CaptureAdapter` requires a stable level for 25 ms. It emits one
navigation action per press, requires a debounced release before the next,
and preserves both debounced BOOT edges for the nonblocking short/long Back
classifier. No auto-repeat. The UI receives a separate 16-event fixed FIFO.
Overflow drops the newest event, counts the loss and is logged by the UI only
when the count changes. GPIO servicing continues even when the UI FIFO is full.
Allocation, startup and rearm failures propagate rather than silently stopping
capture. Raw overflow also records the affected pin for rearm/reconciliation.

The task blocks indefinitely on its queue when no debounce is pending,
including while a stable key is held. Only unsettled transitions introduce a
finite debounce deadline. There is no idle polling timer or power-management
lock. This removes UI blocking as a capture dependency; it is not a claim
that arbitrarily short pulses or RTOS interrupt starvation can be recovered.

### Fixed memory budget

- Task stack: 4,096 bytes, one task (no task per press).
- Raw queue: 8 messages x at most 24 bytes = at most 192 bytes.
- UI queue: 16 events x at most 16 bytes = at most 256 bytes.
- Debounce state: at most 104 bytes; one reply slot for startup/sleep status.
- These payload bounds are compile-time assertions on the Xtensa build.
  FreeRTOS queue/TCB, pthread, Arc and four subscription callback bookkeeping
  are additional fixed platform overhead, not included in the 4,648-byte
  stack/edge/event/adapter subtotal. No per-edge heap allocation or growth.

### Software regression coverage

`tests/input_capture_adapter.rs` runs the actual shared adapter against GPIO
fakes that disable interrupts **before** notification, reject ISR rearm and
require explicit task rearm to capture the next edge. It covers three presses
of one GPIO with 700 ms of blocked UI, twelve with 2.5 seconds of blocked UI,
Down/Down/Select ordering, simultaneous-key FIFO, BOOT press/release through
HTTP wait, bounce, sustained keys, observable overflow, level reconciliation
and rearm error propagation. The fake's independent deadline scheduler models
the input task; no test injects semantic events directly into the UI FIFO.

Input-only verification: the 10 adapter regressions passed; `scripts/build.sh`
ran `scripts/validate.sh` (format, source contracts and all 589 host tests) and
linked the release `xtensa-esp32s3-espidf` ELF, exit 0. `git diff --check` passed.
Two pre-existing header assertions were updated to the already-committed logo's
pixel coordinate, and rustfmt normalized the bitmap array's line breaks;
the bitmap values and renderer are unchanged. No branding review or ZIP build.

## Preserved pending sleep/wake findings (not closed by input correction)

Light-sleep commands are sent to the input task to keep GPIO ownership unique.
The existing GPIO4/5/6 wake sources are retained, and any-edge capture is
restored by that task after wake/rejected sleep. A pending/held input cancels
its sleep attempt. This does **not** yet prove the complete race-free handshake
between UI queue checks, panel/network preparation and actual sleep entry.

- Close and test the whole preparation-to-sleep race, including a complete
  press/release during that window; prove the wake action occurs exactly once.
- Preserve route, note, page and selection across physical wake without NVS
  clearing. These states are not moved into or changed by the input service.
- After panel initialization, require an actual base image before any partial
  refresh; the current `reset_after_external_global(AfterWake)` must not be
  treated as proof of an executed global refresh. This remains a separate fix.
- Physical capture while refreshing/HTTP, GPIO wake latency/held-key behavior
  and idle/current measurements remain **NOT TESTED / NOT MEASURED**.

No ZIP or physical validation is part of this input-only correction.

## Physical acceptance checklist

1. On battery, leave the device untouched: confirm Wi-Fi stops near 15 s and
   light sleep begins near 60 s without a new e-paper image.
2. Press each of UP/SELECT/DOWN once from light sleep; verify exactly one
   intended navigation action and no held-key wake loop.
3. During a normal refresh, global clear and an intentionally slow HTTP
   request, make 80--150 ms key presses; verify their FIFO order and selected
   row.
4. Verify recording, playback, WAV finalization, pairing, upload-in-flight and
   storage writes do not sleep; verify a durable queued upload may sleep.
5. Measure active, Wi-Fi-suspended and light-sleep current at the battery.
   Record instrument, voltage, USB disconnected state, firmware SHA and wake
   latency. Do not report a current result before this procedure completes.
