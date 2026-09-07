#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 SOURCE.png OUTPUT.bin" >&2
  exit 2
fi

SOURCE="$1"
OUTPUT="$2"
WIDTH=454
HEIGHT=76

mkdir -p "$(dirname "$OUTPUT")"
magick "$SOURCE" \
  -filter Lanczos \
  -resize "${WIDTH}x${HEIGHT}" \
  -background white \
  -gravity center \
  -extent "${WIDTH}x${HEIGHT}" \
  -colorspace Gray \
  -threshold 50% \
  -depth 1 \
  "MONO:$OUTPUT"

EXPECTED_BYTES=$(((WIDTH + 7) / 8 * HEIGHT))
ACTUAL_BYTES=$(wc -c < "$OUTPUT" | tr -d ' ')
if [[ "$ACTUAL_BYTES" -ne "$EXPECTED_BYTES" ]]; then
  echo "unexpected hero size: $ACTUAL_BYTES bytes (expected $EXPECTED_BYTES)" >&2
  exit 1
fi
