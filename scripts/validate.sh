#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
source "$ROOT/scripts/rust-toolchain.sh"

cargo +stable fmt --all -- --check
./scripts/validate_source_contract.sh
./scripts/test-host.sh
