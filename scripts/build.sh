#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

./scripts/validate.sh

TARGET="xtensa-esp32s3-espidf"
ARTIFACT="target/xtensa-esp32s3-espidf/release/waveshare-epd397-rust-app"

cargo +esp build --release --target xtensa-esp32s3-espidf

if [[ ! -f "$ARTIFACT" ]]; then
  echo "embedded-build=failed error=missing-artifact path=$ARTIFACT" >&2
  exit 1
fi

ARTIFACT_TYPE="$(file "$ARTIFACT")"
if [[ "$ARTIFACT_TYPE" != *"ELF"* || "$ARTIFACT_TYPE" != *"Tensilica Xtensa"* ]]; then
  echo "embedded-build=failed error=unexpected-artifact-format target=$TARGET artifact=$ARTIFACT_TYPE" >&2
  exit 1
fi

ARTIFACT_SHA256="$(shasum -a 256 "$ARTIFACT" | awk '{print $1}')"
printf 'embedded-build-target=%s\n' "$TARGET"
printf 'embedded-build-artifact=%s\n' "$ARTIFACT"
printf 'embedded-build-artifact-type=%s\n' "$ARTIFACT_TYPE"
printf 'embedded-build-artifact-sha256=%s\n' "$ARTIFACT_SHA256"
echo 'embedded-build=ok'
