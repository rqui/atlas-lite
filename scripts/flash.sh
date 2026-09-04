#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
if ! command -v cargo >/dev/null 2>&1; then
  source "$ROOT/scripts/rust-toolchain.sh"
fi
if [[ "$#" -ne 2 || "$1" != "--port" || -z "$2" ]]; then
  echo "usage: $0 --port PORT" >&2
  echo 'The development flasher requires an explicit serial port and never erases flash.' >&2
  exit 1
fi
PORT="$2"
if ! cargo +esp espflash --version >/dev/null 2>&1; then
  echo 'flash=failed error=cargo-espflash-not-found' >&2
  echo 'Install cargo-espflash (cargo install cargo-espflash --locked) before hardware use.' >&2
  exit 1
fi
# cargo-espflash detects esp-idf-sys and uses the generated bootloader and
# partition table for this build, rather than a raw-address writer.
exec cargo +esp espflash flash --release --target xtensa-esp32s3-espidf \
  --chip esp32s3 --port "$PORT" --monitor
