# SD-card setup

Use a FAT-formatted SD card. Rustmix Wave mounts it at `/sdcard` and expects the following product tree:

```text
/RUSTMIX/
  WEATHER.TXT
  ALARMS.TXT
  DISPLAY.TXT
  BOOKS/
  READER/
    CACHE/
  VOICE/
  SLEEP/
    *.BMP
  APPS/
    HGRID/
    SUDOKU/
    MINES/
    TILTMAZE/
    M2048/
    SOKOBAN/
    DICT/
      INDEX.TXT
      DATA/*.JSN
    CALENDAR/
      EVENTS.TXT
      US2026.TXT
```

## Atlas Lite Wi-Fi: never use SD credentials

Atlas Lite production firmware does not read or create `/RUSTMIX/WIFI.TXT`.
Do not put an Atlas API token, Wi-Fi password, or Wi-Fi credentials on an SD
card, and do not create `WIFI.TXT` for Atlas Lite. Atlas Lite loads
`device_id`, `atlas_url`, `api_token`, `wifi_ssid`, and `wifi_credentials` from
the ESP32 default NVS namespace. The SD card is for product data and cache,
not secret configuration.

Normal product provisioning is now the first-boot setup flow:

1. Start an unconfigured Atlas Lite.
2. Read the temporary AP SSID, generated password, and local URL from e-paper.
3. Join that AP from a phone and submit Wi-Fi SSID/password plus the HTTPS Atlas base URL.
4. The bounded local portal writes those values to NVS, shuts down, and reboots.
5. Approve the short pairing code in `Atlas Web > Settings > Devices`.

The portal never asks for an Atlas API token. The device creates its own key material before pairing and stores the eventual bearer only in NVS.

The legacy development-only intake helper remains available for diagnostics:

```bash
./scripts/provision-atlas-lite.sh
```

The helper prompts for secret values with terminal echo disabled and does not
write a file, echo a value, or put a value in a command argument. Its physical
serial/NVS write is not wired yet: it reports
`physical-write=pending reason=serial-provisioning-receiver-not-wired` and does
not complete target provisioning. Do not treat the helper's intake as a
successful device write or work around it by placing secrets on SD. It is not
part of the normal product flow.

### Migrating an existing `WIFI.TXT`

If a card contains `/RUSTMIX/WIFI.TXT` from an earlier Rustmix bring-up:

1. Stop the device and unmount the card before handling the file.
2. Do not reuse or copy that file for Atlas Lite production.
3. Keep it only on a separately controlled legacy Rustmix card if that older
   firmware still needs it.
4. Otherwise remove it from the card after any required legacy transition:

   ```bash
   rm -- /Volumes/YOUR_SD_CARD/RUSTMIX/WIFI.TXT
   ```

Removing the file does not provision Atlas Lite. Restart an unconfigured device
and use its temporary setup AP.

## Legacy Rustmix Wi-Fi bring-up only

The following format is retained for preserved Rustmix compatibility. It is
not an Atlas Lite production configuration and must not be used to provision
Atlas Lite:

```text
/RUSTMIX/WIFI.TXT
ssid=YOUR_NETWORK
password=YOUR_PASSWORD
timezone=America/New_York
ntp_server=pool.ntp.org
```

Never commit real credentials. Remove this legacy file before repurposing the
card for Atlas Lite.

## Install bundled examples

```bash
./scripts/install-sd-examples.sh /Volumes/YOUR_SD_CARD
```

Existing paths are preserved by default. Use `--force` only when deliberately replacing bundled example files:

```bash
./scripts/install-sd-examples.sh --force /Volumes/YOUR_SD_CARD
```

The generic installer preserves an existing Dictionary and Calendar tree. Use the dedicated installers for intentional complete-pack replacement.

## Weather

Optional `/RUSTMIX/WEATHER.TXT` example:

```text
provider=open-meteo
location=New York, NY
latitude=40.7128
longitude=-74.0060
timezone=America/New_York
refresh_minutes=30
```

## Alarms

Optional `/RUSTMIX/ALARMS.TXT` example:

```text
snooze_minutes=10
alarm=Workday,07:30,weekdays,on,recurring
alarm=Weekend,09:00,weekends,off,recurring
alarm=Appointment,16:45,2026-06-10,on,once
```

Calendar personal events remain separate from alarms.

## Display preferences

`/RUSTMIX/DISPLAY.TXT` supports:

```text
font_family=inter|atkinson-hyperlegible
font_size=compact|standard|large
```

## Sleep images

Files below `/RUSTMIX/SLEEP` must be uncompressed monochrome Windows BMP files:

```text
800 × 480
1-bpp
```

Install bundled samples:

```bash
./scripts/install-sleep-images.sh /Volumes/YOUR_SD_CARD
```

## Reader books and state

Copy TXT, EPUB, or FAT-friendly `.EPU` books into:

```text
/RUSTMIX/BOOKS
```

The device creates Reader state automatically:

```text
/RUSTMIX/READER/STATE.TXT
/RUSTMIX/READER/POSITS.TXT
/RUSTMIX/READER/RECENT.TXT
/RUSTMIX/READER/MARKS.TXT
/RUSTMIX/READER/PREFS.TXT
/RUSTMIX/READER/CACHE/<8HEX>.CCH
```

Reader writes use `.TMP` and `.BAK` siblings for recovery.

## Voice Notes

The device creates:

```text
/RUSTMIX/VOICE/VOICE###.WAV
/RUSTMIX/VOICE/INDEX.TXT
/RUSTMIX/VOICE/META.TXT
/RUSTMIX/VOICE/SETTINGS.TXT
```

Do not hand-edit sidecars while the device is active.

## Complete Dictionary pack

Install from a local `rustmix-x4-firmware` checkout:

```bash
./scripts/install-dictionary-x4-pack.sh \
  --force \
  --x4-repo /Users/piyushdaiya/Documents/projects/rustmix-x4-firmware \
  /Volumes/YOUR_SD_CARD
```

Verify representative lookups:

```bash
./scripts/verify-dictionary-x4-pack.sh /Volumes/YOUR_SD_CARD
```

## U.S.-only Calendar pack

Install from a local X4 checkout:

```bash
./scripts/install-calendar-x4-pack.sh \
  --force \
  --x4-repo /Users/piyushdaiya/Documents/projects/rustmix-x4-firmware \
  /Volumes/YOUR_SD_CARD
```

The installer includes `EVENTS.TXT` and `US2026.TXT`, and explicitly excludes `HINDU26.TXT`.

Calendar personal-event writes use:

```text
EVENTS.TMP -> EVENTS.TXT
EVENTS.BAK retained for rollback
```
