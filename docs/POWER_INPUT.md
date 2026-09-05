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

GPIO4/5/6 use falling-edge capture; GPIO0 uses both edges and is classified as
short/long Back without waiting for release. The ISR only timestamps,
25-ms-debounces and appends to a 16-entry fixed FIFO. It neither logs,
allocates nor waits. The UI task consumes events FIFO order and re-arms each
ESP-IDF interrupt. Overflow is counted rather than overwriting/reordering an
event. A queued event inhibits sleep entry, so input racing sleep is processed
after wake. No auto-repeat is generated.

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
