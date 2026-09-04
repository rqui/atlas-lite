# Release generation

Rustmix Wave has three release helpers:

```text
scripts/build-release-firmware.sh  Build a coherent initial-install bundle
scripts/flash-release.sh           Flash a checked initial-install bundle safely
scripts/package-release.sh         Package the cleaned GitHub-ready source tree
```

## Supported initial-install candidate

The supported first-install candidate is a self-contained directory and ZIP:

```text
dist/atlas-lite-install-v<VERSION>/
dist/atlas-lite-install-v<VERSION>.zip
```

The directory contains an application ELF, the generated ESP-IDF application
image for provenance, matching bootloader and partition-table binaries,
`espflash.toml`, the generated `flasher_args.json`, manifest, checksums and an
installer. All come from one disposable release build, so stale artifacts from
another profile/worktree are rejected before packaging.

After checking hashes and with an explicit user-selected serial port, run:

```bash
cd dist/atlas-lite-install-v<VERSION>
shasum -a 256 -c SHA256SUMS
espflash --version
./flash-atlas-lite.sh --port /dev/cu.usbmodemXXXX
```

The installer invokes the documented ELF-aware path:

```bash
espflash flash --chip esp32s3 --port /dev/cu.usbmodemXXXX --monitor atlas-lite.elf
```

Its local `espflash.toml` explicitly names `bootloader.bin` and
`partition-table.bin`; it never selects a default/stale table. `espflash` is
not installed on the package-build host, so physical use must record the
installed version before writing a board.

Development flashing uses build-aware `cargo-espflash`, which detects
`esp-idf-sys` and uses its generated bootloader/table:

```bash
./scripts/flash.sh --port /dev/cu.usbmodemXXXX
```

## Safety warning: do not use raw-address flashing

Do **not** use `espflash write-bin` for any artifact from this repository.
`write-bin` is a raw-address operation. This candidate uses generated metadata
and an ELF-aware installer, not guessed offsets.

The earlier unverified `*-flash.bin` artifact and the `write-bin ... 0x0`
workflow have been removed.

## Future merged factory image

A merged factory-image workflow remains deferred. It may be added only after all
of the following have been validated on physical hardware:

- Bootloader offset and image
- Partition-table offset and image
- Factory application partition offset
- Flash mode, frequency, and size
- Recovery procedure from ROM download mode

Until then, the checked initial-install bundle is the only supported release
installation method.

## Build a firmware release

```bash
./scripts/build-release-firmware.sh
```

The script:

1. Runs `./scripts/validate.sh` unless `--skip-validate` is provided.
2. Builds in a disposable `CARGO_TARGET_DIR`.
3. Reads the single generated ESP-IDF `flasher_args.json` and validates chip,
   flash settings, offsets and the A/B partition capacity.
4. Copies the matching application ELF/image, bootloader and partition table.
5. Writes local `espflash.toml`, manifest and SHA-256 checksums.
6. Generates a ZIP that can be unpacked and installed without the source tree.

Output naming:

```text
dist/atlas-lite-install-v<VERSION>/atlas-lite.elf
dist/atlas-lite-install-v<VERSION>/bootloader.bin
dist/atlas-lite-install-v<VERSION>/partition-table.bin
dist/atlas-lite-install-v<VERSION>/espflash.toml
dist/atlas-lite-install-v<VERSION>/manifest.json
dist/atlas-lite-install-v<VERSION>/SHA256SUMS
dist/atlas-lite-install-v<VERSION>.zip
```

The application image is included only as checked provenance. The installer
flashes the ELF with its explicit IDF configuration, including the generated
`ota_0` target partition; no raw-address image is a supported installation
command.

## Atlas Lite OTA contract

The M8 partition table reserves two 6 MiB application slots plus `otadata` on
the 16 MiB target. Rollback is enabled. An update is accepted only when all of
these checks pass:

- manifest fetched from the compile-time `ATLAS_LITE_OTA_ORIGIN` over HTTPS;
- Ed25519 signature verifies with `ATLAS_LITE_OTA_PUBLIC_KEY_HEX`;
- artifact URL remains below the fixed origin and `/atlas-lite/` prefix;
- semantic version is newer, size is non-zero and at most 6 MiB;
- streamed application image length and SHA-256 match the signed manifest.

The canonical signed payload is `atlas-lite-ota-v1`, followed by version,
build, artifact URL, size, and lowercase SHA-256, one field per line with a
final newline. The manifest is published at
`/atlas-lite/stable/manifest.json`.

Builds without both compile-time values fail closed with `Updates not
configured`; users cannot enter an arbitrary URL. Interrupted writes leave the
current slot bootable. After reboot, ESP-IDF rollback remains armed until
firmware completes fatal board/configuration/network/client initialization and
reaches the main-loop-ready checkpoint, when it marks the running slot valid.

An OTA artifact must be the ESP-IDF application image for the configured OTA
partition, not the ELF and not a guessed merged/raw-address image. Publication
and signing remain release-operator steps and are not performed by this Draft
PR. The first signed artifact, update, rollback, and ROM recovery must be
validated on physical hardware before any release.

## Skip validation during a repeated local build

```bash
./scripts/build-release-firmware.sh --skip-validate
```

Use this only after the exact source tree has already passed
`./scripts/validate.sh`.

## Package cleaned source

```bash
./scripts/package-release.sh
```

This produces:

```text
dist/waveshare-epd397-rust-app-v<VERSION>-github-ready.zip
dist/waveshare-epd397-rust-app-v<VERSION>-github-ready.zip.sha256
```

The source package excludes Git metadata, build outputs, generated release
artifacts, local caches, patch scratch files, and extracted overlay directories.
