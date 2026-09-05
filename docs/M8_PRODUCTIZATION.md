# Atlas Lite M8 productization

Status: software candidate; all physical results are **NOT TESTED**.

## First boot and provisioning

An unconfigured device creates a temporary WPA2 Atlas Lite AP. The e-paper
screen shows its generated SSID, generated 12-character password, and local
setup URL. The local HTTP page accepts only Wi-Fi SSID/password and an Atlas
HTTPS base URL. The sole exception is `http://` to a literal RFC1918 IPv4
address (`10/8`, `172.16/12`, or `192.168/16`) for explicit LAN development;
hostnames, loopback, link-local and public addresses remain HTTPS-only. This
exception performs no DNS resolution and clients never follow redirects. Product
Settings marks it `LAN HTTP / DEVELOPMENT`. The portal has a 512-byte request
limit, bounded sockets/submissions, a RAM-only CSRF/session proof, and a
ten-minute lifetime. A successful write goes to the dedicated NVS namespace and
immediately reboots; no secret is written to microSD or logs. Holding BOOT during
startup clears this local namespace and restarts into the setup AP, providing a
bounded recovery path for bad Wi-Fi or Atlas configuration.

## Pairing

After Wi-Fi joins, the device creates and persists a complete canonical
`at_v1` credential plus separate poll material before its first request. It
sends only token ID, salt, verifier, device metadata, short code, poll proof,
and these exact scopes:

```text
notes:read
search:read
views:read
capture:write
```

Atlas Web resolves the code, displays the capabilities, and explicitly approves
or denies it. The server stores only digests until approval, materializes the
same deterministic integration key on approval, and never returns the bearer.
The device promotes its already-persisted bearer after an `approved` poll. This
makes lost responses and reboot recovery idempotent without sending a plaintext
device secret back from Atlas.

## Settings and reset boundaries

The Atlas Settings screen shows server, Wi-Fi/RSSI, sync, device ID, firmware,
battery, and storage state. It provides signed update, restart, Reset Wi-Fi,
Unpair Atlas, and Factory reset. Sleep remains the physically proven Power-key
long-press flow. Reset Wi-Fi removes only Wi-Fi fields; Unpair first receives
server confirmation that the paired key is revoked, then removes the local
credential/pending material; Factory reset clears the Atlas Lite NVS namespace.
None of these actions delete Atlas Server data or `/ATLAS/` SD cache/audio.

## Power ruling

The existing central e-paper refresh coordinator, panel sleep, retained sleep
image, Wi-Fi suspension, AXP2101 Power-key polling, and GPIO45 RTC-alarm path are
preserved. The product now uses a bounded host-testable 15-second Wi-Fi suspend
and 60-second ESP32-S3 light-sleep policy with GPIO4/5/6 wake and fixed ISR
input capture. It does not automatically deep-sleep, and does not claim the
PMIC/I2C key or GPIO45 as a wake source. See
[`POWER_INPUT.md`](POWER_INPUT.md) for states, inhibitors and the physical
measurement checklist. Actual currents and board wake behaviour remain **NOT
TESTED**.

## OTA and recovery

`partitions.csv` provides `otadata` and two 6 MiB OTA slots. The updater uses a
fixed compile-time HTTPS origin, signed bounded manifest, strict version/size,
streaming SHA-256, and ESP-IDF inactive-slot commit. Firmware marks a pending
image valid only after all fatal initialization and Atlas client construction
reach the main-loop checkpoint. A build that renders but fails later remains
rollbackable. A bad or interrupted update must retain or roll back to the prior slot. See
[`RELEASE.md`](RELEASE.md) and [`PHYSICAL_SMOKE_TEST.md`](PHYSICAL_SMOKE_TEST.md).

## Rustmix cleanup ruling

No platform or legacy product module is deleted in this branch. Atlas routes
hide the old product shell, while display, power, audio, SD, network, sensor and
input code remain upstream-aligned. Without physical M8 stability and measured
binary/build/source deltas, deletion would increase merge and regression risk.
A dedicated cleanup diff is deferred until the physical gate supplies evidence.
