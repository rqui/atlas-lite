# Consolidated physical smoke test

For screen names, navigation controls, and reference images, see [`USER_GUIDE.md`](USER_GUIDE.md).

Run this checklist after a release build or any cross-cutting runtime change.

Every unchecked M8 item below is `NOT TESTED`; a host test or successful build
must never be substituted for a physical result.

## Atlas Lite M8 provisioning, pairing, power and OTA

1. Erase only the Atlas Lite NVS namespace and boot; confirm the temporary AP,
   generated 12-character password, and local URL appear on e-paper.
2. Confirm the portal expires after 10 minutes, bounds clients/requests/body,
   rejects CSRF replay, and shuts down immediately after a successful NVS save.
3. Provision a WPA2 network and HTTPS Atlas URL; power-cut during save, reboot,
   and verify the device either resumes safely or returns to setup without a
   secret on SD/logs.
4. Pair through Atlas Web Settings > Devices. Confirm the exact scopes are
   `notes:read`, `search:read`, `views:read`, `capture:write`, the code expires,
   denial is safe, approval is single-use, and reboot/lost-poll recovery reuses
   the same pending key material.
5. Revoke from Atlas Web and confirm the device can no longer read/capture.
   Test local Unpair and confirm re-pairing; remove any stale server credential.
6. Measure current at boot, Wi-Fi join, Home sync, reading, idle, sleep-image,
   and wake. Record board revision, supply voltage, firmware SHA, current and
   duration. Do not enable MCU deep sleep until GPIO45 and Power-key wake pass.
7. From Product Settings exercise restart, Reset Wi-Fi, Unpair Atlas, and
   Factory reset. Confirm server data and `/ATLAS/` SD data are untouched.
8. Install a correctly signed newer OTA image; verify the old slot remains
   bootable until the first-frame checkpoint validates the new image.
9. Repeat with bad signature, wrong hash, oversize image, interrupted download,
   interrupted slot write, and a boot-failing image. Confirm rejection or
   automatic rollback without bricking the device.
10. Verify the candidate `SHA256SUMS`, record `espflash --version`, enter ROM
    download mode, and recover with `flash-atlas-lite.sh --port ...`. Do not
    use guessed raw offsets or automatic erase.

## Build and boot

1. Run `./scripts/validate.sh`.
2. Run `cargo +esp build --release --target xtensa-esp32s3-espidf`.
3. Flash with `./scripts/flash.sh monitor`.
4. Confirm boot reaches the Home screen without panic or reset loops.
5. Confirm the displayed version is `1.0.0` and the repository-cleanup readiness marker appears.

## Power key and display refresh

1. Press Power briefly and confirm the display-maintenance menu opens.
2. Select `Clear ghosting now` and confirm a clean global refresh returns to the underlying screen.
3. Press Power briefly, select Cancel, and confirm no sleep transition.
4. Hold Power and confirm random sleep-image mode starts and network services suspend.
5. Wait for the wake quiet guard and press Power to restore the prior route.

## Reader

1. Open one TXT book and one EPUB or `.EPU` book.
2. Confirm staged loading, page navigation, Reader Options, preferences, TOC behavior, and bookmark add/remove.
3. Reboot and confirm Continue Reading restores the prior book and page.
4. Confirm `/RUSTMIX/READER/POSITS.TXT` and `CACHE/<8HEX>.CCH` exist.

## Dictionary

1. Open `Tools > Dictionary`.
2. Confirm `CAB`, `BARN`, and `CALENDAR` exact lookup.
3. Confirm `AAR*` prefix lookup and result cycling.
4. Press BOOT briefly and confirm `NAV H` / `NAV V` switches without moving the selected key.
5. Hold BOOT and confirm hierarchical Back.

## Calendar

1. Open `Productivity > Calendar`.
2. Confirm U.S. event markers and daily agenda rendering.
3. Create, edit, and delete one personal event.
4. Confirm U.S. holiday rows remain read-only.
5. Confirm agenda summary, pagination, first row, and footer do not overlap.
6. Confirm `EVENTS.TMP` is absent after successful write and `EVENTS.BAK` is retained.

## Voice Notes

1. Record a note, pause, resume, and save.
2. Confirm a new `VOICE###.WAV` file persists after reboot.
3. Confirm gain selection persists, metadata is readable, playback works, and delete confirmation works.
4. Confirm LAN export displays a path and protected sidecars are not exposed.

## Network, alarms, and settings

1. Confirm Wi-Fi connection and SNTP status.
2. Start the explicit Wi-Fi transfer portal, access it with the displayed code, then stop it.
3. Confirm an alarm can sound, snooze, and dismiss.
4. Confirm alarm behavior is not hidden by the Power-key display menu.
5. Confirm Display settings persist after reboot.

## Games and sensors

1. Open Sudoku and verify rotary movement, BOOT-short axis toggle, edit, and commit.
2. Open one motion game and verify debounced IMU movement.
3. Open Environment and Motion diagnostic screens.
4. Run the audio test chime.

## Text-editor layout alignment

1. Open Voice Notes, select a saved WAV, and choose **Edit friendly title**.
2. Confirm the header reads **VOICE NOTE TITLE / EDIT FRIENDLY TITLE**.
3. Confirm the shared grid keyboard is visible and defaults to `NAV H`.
4. Press BOOT briefly and confirm `NAV V` appears without moving the selected key.
5. Use `SAVE` to persist a friendly title and confirm the internal `VOICE###.WAV` filename remains unchanged.
6. Reopen the title editor, hold BOOT, and confirm the edit is cancelled without saving.
7. Open Calendar, create or edit a personal event, and confirm the status strip shows a compact `YYYY-MM-DD` date plus `NAV H` or `NAV V` without overlap.
8. Confirm the Calendar editor footer is fully visible.
