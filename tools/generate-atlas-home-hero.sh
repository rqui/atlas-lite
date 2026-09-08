#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 SOURCE.png OUTPUT.bin" >&2
  exit 2
fi

SOURCE="$1"
OUTPUT="$2"
WIDTH=456
HEIGHT=106
MARK_WIDTH=360
MARK_HEIGHT=84

mkdir -p "$(dirname "$OUTPUT")"
magick "$SOURCE" \
  -fuzz 10% \
  -trim \
  +repage \
  -filter Lanczos \
  -resize "${MARK_WIDTH}x${MARK_HEIGHT}" \
  -background white \
  -gravity center \
  -extent "${WIDTH}x${HEIGHT}" \
  -colorspace Gray \
  -threshold 50% \
  -negate \
  -depth 1 \
  "MONO:$OUTPUT"

EXPECTED_BYTES=$(((WIDTH + 7) / 8 * HEIGHT))
ACTUAL_BYTES=$(wc -c < "$OUTPUT" | tr -d ' ')
if [[ "$ACTUAL_BYTES" -ne "$EXPECTED_BYTES" ]]; then
  echo "unexpected hero size: $ACTUAL_BYTES bytes (expected $EXPECTED_BYTES)" >&2
  exit 1
fi
