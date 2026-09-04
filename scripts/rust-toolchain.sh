#!/usr/bin/env bash
# Source this helper from repository scripts that invoke Rust tooling.
# It deliberately does not change the caller's shell options.

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  echo 'ERROR: source scripts/rust-toolchain.sh from a Bash script; do not execute it directly.' >&2
  exit 2
fi

if [[ "${ATLAS_RUST_TOOLCHAIN_HELPER_LOADED:-0}" == '1' ]]; then
  return 0
fi

atlas_find_rustup() {
  local candidate
  candidate="${ATLAS_RUSTUP_BIN:-}"
  if [[ -n "$candidate" && -x "$candidate" ]]; then
    printf '%s\n' "$candidate"
    return 0
  fi

  candidate="$(command -v rustup 2>/dev/null || true)"
  if [[ -n "$candidate" && -x "$candidate" ]]; then
    printf '%s\n' "$candidate"
    return 0
  fi

  local cargo_home="${CARGO_HOME:-}"
  local rustup_home="${RUSTUP_HOME:-}"
  local home_dir="${HOME:-}"
  local candidates=()
  [[ -n "$cargo_home" ]] && candidates+=("$cargo_home/bin/rustup")
  [[ -n "$home_dir" ]] && candidates+=("$home_dir/.cargo/bin/rustup")
  [[ -n "$rustup_home" ]] && candidates+=("$(dirname "$rustup_home")/.cargo/bin/rustup")

  for candidate in "${candidates[@]}"; do
    if [[ -x "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  return 1
}

atlas_prepend_path() {
  local directory="$1"
  [[ -d "$directory" ]] || return 0
  case ":${PATH:-}:" in
    *":$directory:"*) ;;
    *) PATH="$directory:${PATH:-}"; export PATH ;;
  esac
}

atlas_add_cargo_bin_paths() {
  local discovered_rustup_home="${RUSTUP_HOME:-}"
  if [[ -z "$discovered_rustup_home" ]]; then
    discovered_rustup_home="$("$ATLAS_RUSTUP_BIN" show home 2>/dev/null || true)"
  fi

  local cargo_home="${CARGO_HOME:-}"
  local home_dir="${HOME:-}"
  local candidates=()
  [[ -n "$cargo_home" ]] && candidates+=("$cargo_home/bin")
  [[ -n "$home_dir" ]] && candidates+=("$home_dir/.cargo/bin")
  [[ -n "$discovered_rustup_home" ]] && candidates+=("$(dirname "$discovered_rustup_home")/.cargo/bin")
  [[ -n "$ATLAS_RUSTUP_BIN" ]] && candidates+=("$(dirname "$ATLAS_RUSTUP_BIN")")

  local directory
  for directory in "${candidates[@]}"; do
    atlas_prepend_path "$directory"
  done
}

atlas_prepare_toolchain() {
  local toolchain="$1"
  local cargo_path
  local rustc_path
  if ! cargo_path="$("$ATLAS_RUSTUP_BIN" which cargo --toolchain "$toolchain" 2>/dev/null)" || [[ ! -x "$cargo_path" ]]; then
    echo "ERROR: rustup toolchain '$toolchain' is missing cargo; install it with 'rustup toolchain install $toolchain'." >&2
    return 1
  fi
  if ! rustc_path="$("$ATLAS_RUSTUP_BIN" which rustc --toolchain "$toolchain" 2>/dev/null)" || [[ ! -x "$rustc_path" ]]; then
    echo "ERROR: rustup toolchain '$toolchain' is missing rustc; install it with 'rustup toolchain install $toolchain'." >&2
    return 1
  fi

  atlas_prepend_path "$(dirname "$cargo_path")"
  atlas_prepend_path "$(dirname "$rustc_path")"
  if command -v ldproxy >/dev/null 2>&1; then
    ATLAS_LDPROXY_BIN="$(command -v ldproxy)"
    export ATLAS_LDPROXY_BIN
  fi
}

if ! ATLAS_RUSTUP_BIN="$(atlas_find_rustup)"; then
  echo 'ERROR: rustup was not found. Install rustup and the stable/esp toolchains, or set ATLAS_RUSTUP_BIN to its executable.' >&2
  return 127
fi
export ATLAS_RUSTUP_BIN

atlas_add_cargo_bin_paths
if ! atlas_prepare_toolchain stable; then
  return 127
fi

cargo() {
  local toolchain=stable
  case "${1:-}" in
    +stable)
      shift
      ;;
    +esp)
      toolchain=esp
      shift
      ;;
  esac
  if ! atlas_prepare_toolchain "$toolchain"; then
    return 127
  fi
  "$ATLAS_RUSTUP_BIN" run "$toolchain" cargo "$@"
}

rustc() {
  local toolchain=stable
  case "${1:-}" in
    +stable)
      shift
      ;;
    +esp)
      toolchain=esp
      shift
      ;;
  esac
  if ! atlas_prepare_toolchain "$toolchain"; then
    return 127
  fi
  "$ATLAS_RUSTUP_BIN" run "$toolchain" rustc "$@"
}

ATLAS_RUST_TOOLCHAIN_HELPER_LOADED=1
