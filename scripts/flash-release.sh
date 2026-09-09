#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
usage() {
  cat >&2 <<TXT
usage: $0 --port PORT

Flash this self-contained Atlas Lite initial-install bundle. The port is
mandatory; the helper never guesses a device and never erases flash.
TXT
}
PORT=""
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --port) [[ "$#" -ge 2 ]] || { usage; exit 1; }; PORT="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) usage; exit 1 ;;
  esac
done
if [[ -z "$PORT" ]]; then
  echo 'release-flash=failed error=explicit-port-required' >&2
  usage
  exit 1
fi
cd "$SCRIPT_DIR"
for required in atlas-lite.elf bootloader.bin partition-table.bin espflash.toml manifest.json SHA256SUMS; do
  [[ -s "$required" ]] || { echo "release-flash=failed error=missing-bundle-artifact path=$required" >&2; exit 1; }
done
if ! command -v espflash >/dev/null 2>&1; then
  echo 'release-flash=failed error=espflash-not-found' >&2
  echo 'Install espflash, record espflash --version, then retry.' >&2
  exit 1
fi
if command -v shasum >/dev/null 2>&1; then shasum -a 256 -c SHA256SUMS; else sha256sum -c SHA256SUMS; fi
if ! grep -Fqx 'bootloader = "bootloader.bin"' espflash.toml || ! grep -Fqx 'partition_table = "partition-table.bin"' espflash.toml; then
  echo 'release-flash=failed error=invalid-local-espflash-config' >&2
  exit 1
fi
exec espflash --skip-update-check flash --chip esp32s3 --port "$PORT" --monitor atlas-lite.elf
