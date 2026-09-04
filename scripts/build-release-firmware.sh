#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
if ! command -v cargo >/dev/null 2>&1; then
  source "$ROOT/scripts/rust-toolchain.sh"
fi

SKIP_VALIDATE=0
if [[ "${1:-}" == "--skip-validate" ]]; then
  SKIP_VALIDATE=1
  shift
fi
if [[ "$#" -ne 0 ]]; then
  echo "usage: $0 [--skip-validate]" >&2
  exit 1
fi
if [[ "$SKIP_VALIDATE" -eq 0 ]]; then
  ./scripts/validate.sh
else
  echo 'release-firmware-validation=skipped'
fi

VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n1)"
SOURCE_SHA="$(git rev-parse HEAD)"
if [[ -z "$VERSION" || ! "$SOURCE_SHA" =~ ^[0-9a-f]{40}$ ]]; then
  echo 'release-firmware-build=failed error=unable-to-determine-provenance' >&2
  exit 1
fi

# Isolate the build so no stale profile, dependency hash or other worktree can
# contribute a bootloader or partition table to this installation candidate.
BUILD_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/atlas-lite-release-build.XXXXXX")"
trap 'rm -rf "$BUILD_ROOT"' EXIT
export CARGO_TARGET_DIR="$BUILD_ROOT/target"
TARGET="xtensa-esp32s3-espidf"
cargo +esp build --release --target xtensa-esp32s3-espidf

RELEASE_ROOT="$CARGO_TARGET_DIR/$TARGET/release"
ELF_SOURCE="$RELEASE_ROOT/waveshare-epd397-rust-app"
[[ -f "$ELF_SOURCE" ]] || { echo "release-firmware-build=failed error=missing-release-elf path=$ELF_SOURCE" >&2; exit 1; }

metadata=()
while IFS= read -r path; do metadata+=("$path"); done < <(find "$RELEASE_ROOT/build" -type f -path '*/esp-idf-sys-*/out/build/flasher_args.json' -print | sort)
if [[ "${#metadata[@]}" -ne 1 ]]; then
  echo "release-firmware-build=failed error=ambiguous-flasher-metadata count=${#metadata[@]}" >&2
  exit 1
fi
FLASHER_ARGS="${metadata[0]}"
command -v jq >/dev/null 2>&1 || { echo 'release-firmware-build=failed error=jq-not-found' >&2; exit 1; }
BUILD_DIR="$(dirname "$FLASHER_ARGS")"
bootloader_rel="$(jq -r '.bootloader.file // empty' "$FLASHER_ARGS")"
partition_rel="$(jq -r '."partition-table".file // empty' "$FLASHER_ARGS")"
application_rel="$(jq -r '.app.file // empty' "$FLASHER_ARGS")"
for relative in "$bootloader_rel" "$partition_rel" "$application_rel"; do
  [[ -n "$relative" && "$relative" != /* && "$relative" != *'..'* ]] || { echo 'release-firmware-build=failed error=unsafe-flasher-metadata-path' >&2; exit 1; }
done
BOOTLOADER_SOURCE="$BUILD_DIR/$bootloader_rel"
PARTITION_SOURCE="$BUILD_DIR/$partition_rel"
APPLICATION_SOURCE="$BUILD_DIR/$application_rel"
for artifact in "$BOOTLOADER_SOURCE" "$PARTITION_SOURCE" "$APPLICATION_SOURCE"; do
  [[ -s "$artifact" ]] || { echo "release-firmware-build=failed error=missing-generated-artifact path=$artifact" >&2; exit 1; }
done
if ! jq -e '.bootloader.offset == "0x0" and ."partition-table".offset == "0x8000" and .app.offset == "0x20000" and .extra_esptool_args.chip == "esp32s3" and .flash_settings.flash_size == "16MB" and .flash_settings.flash_freq == "80m"' "$FLASHER_ARGS" >/dev/null; then
  jq -c . "$FLASHER_ARGS" >&2
  echo 'release-firmware-build=failed error=unexpected-generated-flash-layout' >&2
  exit 1
fi

# Check the intended A/B layout and capacity before packaging the binary table.
python3 - partitions.csv <<'PY'
import csv, sys
entries = {}
with open(sys.argv[1], newline='') as source:
    for row in csv.reader(source):
        if not row or row[0].strip().startswith('#'):
            continue
        name, kind, subtype, offset, size, *_ = [item.strip() for item in row]
        entries[name] = (kind, subtype, int(offset, 0), int(size, 0))
expected = {
    'nvs': ('data', 'nvs', 0x9000, 0x6000), 'otadata': ('data', 'ota', 0xf000, 0x2000),
    'phy_init': ('data', 'phy', 0x11000, 0x1000), 'ota_0': ('app', 'ota_0', 0x20000, 0x600000),
    'ota_1': ('app', 'ota_1', 0x620000, 0x600000), 'storage': ('data', 'fat', 0xc20000, 0x3e0000),
}
if entries != expected:
    raise SystemExit('release-firmware-build=failed error=unexpected-partition-table')
ordered = sorted((offset, offset + size) for _, _, offset, size in entries.values())
if any(left[1] > right[0] for left, right in zip(ordered, ordered[1:])) or ordered[-1][1] > 0x1000000:
    raise SystemExit('release-firmware-build=failed error=invalid-partition-capacity')
PY

mkdir -p dist
BUNDLE_DIR="dist/atlas-lite-install-v${VERSION}"
ZIP_OUT="${BUNDLE_DIR}.zip"
rm -rf "$BUNDLE_DIR" "$ZIP_OUT"
mkdir -p "$BUNDLE_DIR"
cp "$ELF_SOURCE" "$BUNDLE_DIR/atlas-lite.elf"
cp "$APPLICATION_SOURCE" "$BUNDLE_DIR/application.bin"
cp "$BOOTLOADER_SOURCE" "$BUNDLE_DIR/bootloader.bin"
cp "$PARTITION_SOURCE" "$BUNDLE_DIR/partition-table.bin"
cp "$FLASHER_ARGS" "$BUNDLE_DIR/esp-idf-flasher-args.json"
cp scripts/flash-release.sh "$BUNDLE_DIR/flash-atlas-lite.sh"
chmod +x "$BUNDLE_DIR/flash-atlas-lite.sh"
cat > "$BUNDLE_DIR/espflash.toml" <<'TOML'
[flash]
mode = "dio"
size = "16MB"
frequency = "80MHz"

[idf]
bootloader = "bootloader.bin"
partition_table = "partition-table.bin"
target_app_partition = "ota_0"
TOML
cat > "$BUNDLE_DIR/manifest.json" <<JSON
{"schema":1,"product":"atlas-lite","source_commit":"$SOURCE_SHA","version":"$VERSION","target":"$TARGET","chip":"esp32s3","flash_size":"16MB","flashing_method":"espflash flash ELF with local espflash.toml","installer_tool":"espflash (not installed on the package-build host; verify espflash --version before physical use)","rust_toolchain":"$(cargo +esp --version)","esp_idf_version":"v5.4.3"}
JSON
cat > "$BUNDLE_DIR/FLASHING.txt" <<TXT
Atlas Lite initial-install candidate v${VERSION}
Source commit: ${SOURCE_SHA}

This bundle contains an application ELF plus matching ESP-IDF generated
bootloader and partition table from one isolated release build. Do not mix
files from another bundle or source checkout.

Before a user-authorized physical installation:
1. Verify checksums: shasum -a 256 -c SHA256SUMS
2. Install espflash and record its version: espflash --version
3. Back up/review recovery first. This installer never erases flash.
4. Use an explicit port only:
     ./flash-atlas-lite.sh --port /dev/cu.usbmodemXXXX

The helper runs espflash flash on atlas-lite.elf. Local espflash.toml selects
this bundle's bootloader.bin, partition-table.bin, and ota_0 application
partition. It does not use write-bin, raw addresses, a merged factory image, or
automatic port selection.

ROM/USB recovery and any partition migration/erase require separate explicit
authorization and physical validation. No hardware write was performed here.
TXT
(
  cd "$BUNDLE_DIR"
  shasum -a 256 atlas-lite.elf application.bin bootloader.bin partition-table.bin \
    esp-idf-flasher-args.json espflash.toml flash-atlas-lite.sh manifest.json FLASHING.txt > SHA256SUMS
)
(
  cd dist
  zip -qr "$(basename "$ZIP_OUT")" "$(basename "$BUNDLE_DIR")"
)
echo "release-firmware-bundle=$BUNDLE_DIR"
echo "release-firmware-checksums=$BUNDLE_DIR/SHA256SUMS"
echo "release-firmware-manifest=$BUNDLE_DIR/manifest.json"
echo "release-firmware-zip=$ZIP_OUT"
echo 'release-firmware-format=coherent-esp-idf-install-bundle'
echo 'release-firmware-build=ok'
