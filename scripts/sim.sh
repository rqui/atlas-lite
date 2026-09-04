#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
export PATH="$ROOT/.atlas-lite-toolchain:$PATH"
HOST_TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"

cargo run --target "$HOST_TRIPLE" --features simulator --bin atlas-lite-sim -- "$@"
