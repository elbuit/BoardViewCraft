#!/usr/bin/env bash
# Shared setup for BoardViewCraft packaging scripts.
# Sourced via: . "$(dirname "${BASH_SOURCE[0]}")/../env.sh"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export ROOT

workspace_version() {
  awk '
    /^\[/ { in_pkg = ($0 == "[workspace.package]") ; next }
    in_pkg && $1 == "version" { gsub(/[" ]/, "", $3); print $3; exit }
  ' "$ROOT/Cargo.toml"
  return
}

VERSION="${BVC_VERSION:-$(workspace_version)}"
if [[ -z "$VERSION" ]]; then
  echo "error: could not read [workspace.package] version from $ROOT/Cargo.toml" >&2
  exit 1
fi
export VERSION

DIST="${DIST:-$ROOT/dist/release}"
mkdir -p "$DIST"
export DIST

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"

warn() {
  if [[ -n "${GITHUB_ACTIONS:-}" ]]; then echo "::warning::$*"; else echo "warning: $*" >&2; fi
  return
}
