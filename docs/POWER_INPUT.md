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

The earlier input-only checkpoint ran its ten adapter regressions, format,
source contracts and host suite successfully. The sleep/wake checkpoint adds
the shared handoff and panel-base regressions described below; final command
results and install-bundle provenance are recorded in PR #10. The prior logo
bitmap is unchanged.

## Light-sleep handoff and panel restoration

The automatic path is now an explicit two-owner protocol:

1. The UI checks ordinary work inhibitors, puts the panel controller to sleep
   and suspends networking. A network-suspend failure immediately initializes
   the panel again and records that a base frame is required.
2. The UI sends `AttemptLightSleep` to the sole GPIO owner. That command is
   ordered behind all copied raw edges. `InputService` drains them, reconciles
   pin levels, applies the same 25-ms adapter debounce and rejects the attempt
   if semantic output, raw input, a candidate debounce, or a held key exists.
3. It records the ISR edge epoch, arms the existing GPIO4/5/6 low-level wake
   sources and checks the same shared handoff predicate again. An ISR increments
   that epoch before queueing its edge. An edge during panel/network preparation
   is therefore either in the ordered input state and cancels, or arrives after
   wake arming and is a configured ESP-IDF wake source.
4. The service restores both-edge capture and rearms outside ISR after a
   rejected sleep or ESP-IDF return, reconciles the live key level, and replies
   `CancelledForInput` or `SleptAndWoke`. It never creates a synthetic button
   event; the normal adapter FIFO supplies the one wake action.
5. In either result the UI initializes the panel, marks controller RAM as
   needing a base, and keeps cached route/note/page/selection in RAM. Networking
   stays suspended until an explicit Atlas request exists; that request is then
   resumed once by the existing pending-request path. A real sleep error is
   logged and follows the same panel recovery path.

`PanelRefreshCoordinator` now distinguishes `base_required` from a completed
global refresh. Controller initialization and panel rail loss only set the
flag. The next refresh is planned as a global base, and partial count/flag are
committed only after `show_base()` returns success. A failed base leaves the
flag set, so no partial can follow it. This also covers a cancelled MCU sleep
after the panel controller was powered down. No LUT, region transport, command
sequence or periodic-cleanup threshold changed.

### Software regression coverage for this block

The same `CaptureAdapter::permits_sleep_handoff` used by ESP-IDF is exercised
against the one-shot GPIO fake: clean offer; complete press during preparation;
raw edge; unresolved debounce; held key; epoch change after wake arming; and a
low-level wake press reconciled to exactly one ordinary navigation event. The
existing ten GPIO regressions remain. `PanelRefreshCoordinator` covers pending
base, failed base (not committed), successful base, and the prohibition on a
partial before that base. These are host tests, not physical proof.

## Remaining physical validation

- Verify the ESP-IDF GPIO wake handoff on battery with an 80--150 ms press
  during panel/network preparation and at the final sleep edge; each should
  yield exactly one action without a second press.
- Verify held navigation keys do not produce an immediate sleep/wake loop, and
  BOOT never invokes startup recovery during a light-sleep return.
- Verify the first physical update after wake is a real base and that route,
  note, page and selection remain visible without Wi-Fi/SD access.
- Measure wake latency and active/Wi-Fi-suspended/light-sleep current with USB
  disconnected. These remain **NOT TESTED / NOT MEASURED**.

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
