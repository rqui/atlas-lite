#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
export PATH="$ROOT/.atlas-lite-toolchain:$PATH"
CARGO_TARGET_DIR="$ROOT/target"
export CARGO_TARGET_DIR
HOST_TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"

cargo run --manifest-path "$ROOT/tools/atlas-lite-sim/Cargo.toml" \
  --target "$HOST_TRIPLE" --bin atlas-lite-sim -- "$@"
