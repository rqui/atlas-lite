#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
FIXTURE="$TMP/fixture"
FAKEBIN="$TMP/fakebin"
mkdir -p "$FIXTURE/.cargo" "$FIXTURE/scripts" "$FAKEBIN"
cp "$ROOT/Cargo.toml" "$FIXTURE/Cargo.toml"
cp "$ROOT/partitions.csv" "$FIXTURE/partitions.csv"
cp "$ROOT/.cargo/config.toml" "$FIXTURE/.cargo/config.toml"
cp "$ROOT/scripts/build-release-firmware.sh" "$FIXTURE/scripts/build-release-firmware.sh"
cp "$ROOT/scripts/flash-release.sh" "$FIXTURE/scripts/flash-release.sh"
cp "$ROOT/scripts/flash.sh" "$FIXTURE/scripts/flash.sh"
chmod +x "$FIXTURE/scripts/"*.sh

cat > "$FAKEBIN/cargo" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
if [[ "$*" == '+esp --version' ]]; then
  echo 'cargo 1.97.0-test'
  exit 0
fi
if [[ "$*" == '+esp build --release --target xtensa-esp32s3-espidf' ]]; then
  root="${CARGO_TARGET_DIR:?missing CARGO_TARGET_DIR}/xtensa-esp32s3-espidf/release"
  build="$root/build/esp-idf-sys-test/out/build"
  mkdir -p "$build/bootloader" "$build/partition_table"
  printf 'atlas-lite-final-elf\n' > "$root/waveshare-epd397-rust-app"
  printf 'auxiliary-esp-idf-app\n' > "$build/libespidf.bin"
  printf 'boot\n' > "$build/bootloader/bootloader.bin"
  printf 'table\n' > "$build/partition_table/partition-table.bin"
  cat > "$build/flasher_args.json" <<JSON
{"flash_settings":{"flash_size":"16MB","flash_freq":"80m"},"bootloader":{"offset":"0x0","file":"bootloader/bootloader.bin"},"partition-table":{"offset":"0x8000","file":"partition_table/partition-table.bin"},"app":{"offset":"0x20000","file":"libespidf.bin"},"extra_esptool_args":{"chip":"esp32s3"}}
JSON
  exit 0
fi
if [[ "$*" == '+esp espflash --version' ]]; then
  exit 0
fi
if [[ "$*" == '+esp espflash flash '* ]]; then
  printf '%s\n' "$*" > "${FAKE_CARGO_ESPFLASH_ARGS:?}"
  exit 0
fi
echo "fake cargo unexpected arguments: $*" >&2
exit 1
SH
chmod +x "$FAKEBIN/cargo"
cat > "$FAKEBIN/git" <<'SH'
#!/usr/bin/env bash
if [[ "$*" == 'rev-parse HEAD' ]]; then
  echo 0123456789012345678901234567890123456789
  exit 0
fi
exit 1
SH
chmod +x "$FAKEBIN/git"
cat > "$FAKEBIN/esptool.py" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == 'version' ]]; then
  echo '4.12.0'
  exit 0
fi
if [[ "${1:-}" == '--chip' && "${2:-}" == 'esp32s3' && "${3:-}" == 'elf2image' ]]; then
  printf '%s\n' "$*" >> "${FAKE_ESPTOOL_ARGS:?}"
  shift 3
  output=''
  input=''
  while [[ "$#" -gt 0 ]]; do
    case "$1" in
      --output) output="$2"; shift 2 ;;
      --*) shift 2 ;;
      *) input="$1"; shift ;;
    esac
  done
  [[ -n "$output" && -n "$input" && -f "$input" ]] || exit 1
  { printf 'converted-from-final-elf\n'; cat "$input"; } > "$output"
  exit 0
fi
if [[ "${1:-}" == '--chip' && "${2:-}" == 'esp32s3' && "${3:-}" == 'image_info' ]]; then
  [[ -s "${4:-}" ]] || exit 1
  printf 'File size: 42 (bytes)\nImage version: 1\nValidation Hash: 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef (valid)\n'
  exit 0
fi
echo "fake esptool unexpected arguments: $*" >&2
exit 1
SH
chmod +x "$FAKEBIN/esptool.py"
cat > "$FAKEBIN/espflash" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
[[ -f espflash.toml ]] || { echo 'missing local config' >&2; exit 1; }
printf '%s|%s\n' "$PWD" "$*" > "${FAKE_ESPFLASH_ARGS:?}"
SH
chmod +x "$FAKEBIN/espflash"

(
  cd "$FIXTURE"
  FAKE_ESPTOOL_ARGS="$TMP/esptool-args.txt" ATLAS_ESPTOOL="$FAKEBIN/esptool.py" PATH="$FAKEBIN:$PATH" ./scripts/build-release-firmware.sh --skip-validate
)

BUNDLE="$FIXTURE/dist/atlas-lite-install-v1.0.0"
grep -Fqx 'CARGO_WORKSPACE_DIR = { value = "", relative = true }' "$FIXTURE/.cargo/config.toml"
for required in atlas-lite.elf application.bin bootloader.bin partition-table.bin esp-idf-flasher-args.json espflash.toml flash-atlas-lite.sh manifest.json FLASHING.txt SHA256SUMS; do
  [[ -s "$BUNDLE/$required" ]] || { echo "release-flash-workflow-selftest=failed missing=$required" >&2; exit 1; }
done
jq -e '.source_commit == "0123456789012345678901234567890123456789" and .chip == "esp32s3" and .flash_size == "16MB" and .application_image.tool == "esptool.py" and .application_image.tool_version == "4.12.0" and .application_image.input == "atlas-lite.elf" and .application_image.output == "application.bin" and .application_image.target_partition == "ota_0" and (.esp_idf_flasher_args_role | contains("app.file is not used"))' "$BUNDLE/manifest.json" >/dev/null
grep -Fqx -- '--chip esp32s3 elf2image --flash_mode dio --flash_freq 80m --flash_size 16MB --output dist/atlas-lite-install-v1.0.0/application.bin dist/atlas-lite-install-v1.0.0/atlas-lite.elf' "$TMP/esptool-args.txt"
grep -Fqx 'converted-from-final-elf' "$BUNDLE/application.bin"
if grep -Fq 'auxiliary-esp-idf-app' "$BUNDLE/application.bin"; then
  echo 'release-flash-workflow-selftest=failed auxiliary-app-was-packaged' >&2
  exit 1
fi
grep -Fqx 'bootloader = "bootloader.bin"' "$BUNDLE/espflash.toml"
grep -Fqx 'partition_table = "partition-table.bin"' "$BUNDLE/espflash.toml"
grep -Fqx 'target_app_partition = "ota_0"' "$BUNDLE/espflash.toml"
unzip -Z1 "$FIXTURE/dist/atlas-lite-install-v1.0.0.zip" | grep -Fqx 'atlas-lite-install-v1.0.0/bootloader.bin'

EXTRACTED="$TMP/extracted bundle"
mkdir -p "$EXTRACTED"
unzip -q "$FIXTURE/dist/atlas-lite-install-v1.0.0.zip" -d "$EXTRACTED"
PACKAGE="$EXTRACTED/atlas-lite-install-v1.0.0"
rm -rf "$FIXTURE/dist"
FAKE_ESPTOOL_ARGS="$TMP/esptool-args.txt" "$FAKEBIN/esptool.py" --chip esp32s3 elf2image --flash_mode dio --flash_freq 80m --flash_size 16MB --output "$TMP/recomputed-application.bin" "$PACKAGE/atlas-lite.elf"
cmp "$PACKAGE/application.bin" "$TMP/recomputed-application.bin"
FAKE_ESPFLASH_ARGS="$TMP/espflash-args.txt" PATH="$FAKEBIN:$PATH" "$PACKAGE/flash-atlas-lite.sh" --port /dev/cu.TEST
[[ "$(cat "$TMP/espflash-args.txt")" == "$PACKAGE|--skip-update-check flash --chip esp32s3 --port /dev/cu.TEST --monitor atlas-lite.elf" ]]

if FAKE_ESPFLASH_ARGS="$TMP/espflash-args.txt" PATH="$FAKEBIN:$PATH" "$PACKAGE/flash-atlas-lite.sh" >/dev/null 2>&1; then
  echo 'release-flash-workflow-selftest=failed missing-explicit-port-accepted' >&2
  exit 1
fi
printf 'corrupt\n' >> "$PACKAGE/application.bin"
if FAKE_ESPFLASH_ARGS="$TMP/espflash-args.txt" PATH="$FAKEBIN:$PATH" "$PACKAGE/flash-atlas-lite.sh" --port /dev/cu.TEST >/dev/null 2>&1; then
  echo 'release-flash-workflow-selftest=failed checksum-mismatch-accepted' >&2
  exit 1
fi
FAKE_CARGO_ESPFLASH_ARGS="$TMP/cargo-espflash-args.txt" PATH="$FAKEBIN:$PATH" "$FIXTURE/scripts/flash.sh" --port /dev/cu.TEST
[[ "$(cat "$TMP/cargo-espflash-args.txt")" == '+esp espflash flash --release --target xtensa-esp32s3-espidf --chip esp32s3 --port /dev/cu.TEST --monitor' ]]

echo 'release-flash-workflow-selftest=ok'
