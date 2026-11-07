#!/usr/bin/env bash
# Build BoardViewCraft for FreeBSD and package it:
#   $DIST/boardviewcraft-<version>-freebsd-<arch>.tar.gz   a /usr/local-style tree (bin/, share/)
#
# Usage: packaging/freebsd/package.sh [--skip-build]
set -euo pipefail

# shellcheck source=../env.sh
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
LINUX="$ROOT/packaging/linux"
APP_ID="boardviewcraft"

SKIP_BUILD=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-build) SKIP_BUILD=1; shift ;;
    -h | --help) echo "Usage: packaging/freebsd/package.sh [--skip-build]"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

release_arch() {
  case "$1" in
    amd64 | x86_64) echo x86_64 ;;
    arm64 | aarch64) echo aarch64 ;;
    *) echo "unsupported architecture $1" >&2; return 2 ;;
  esac
}

case "${CARGO_BUILD_TARGET:-}" in
  "") ARCH="$(release_arch "$(uname -m)")" ;;
  aarch64-unknown-freebsd | x86_64-unknown-freebsd) ARCH="$(release_arch "${CARGO_BUILD_TARGET%%-*}")" ;;
  *) echo "CARGO_BUILD_TARGET=$CARGO_BUILD_TARGET is not a supported FreeBSD target" >&2; exit 2 ;;
esac

BASENAME="boardviewcraft-$VERSION-freebsd-$ARCH"
echo "==> BoardViewCraft $VERSION for FreeBSD $ARCH"

if [[ "$SKIP_BUILD" = 0 ]]; then
  (cd "$ROOT" && cargo build --release ${CARGO_BUILD_TARGET:+--target "$CARGO_BUILD_TARGET"} --package boardviewcraft --ignore-rust-version)
fi

BIN="$CARGO_TARGET_DIR/${CARGO_BUILD_TARGET:+$CARGO_BUILD_TARGET/}release"
WORK="$CARGO_TARGET_DIR/freebsd-package"
STAGE="$WORK/$BASENAME"
rm -rf "$WORK"

mkdir -p "$STAGE/bin" "$STAGE/share/applications" "$STAGE/share/metainfo" \
  "$STAGE/share/icons" "$STAGE/share/doc/boardviewcraft"

install -m 755 "$BIN/boardviewcraft" "$STAGE/bin/"
strip "$STAGE/bin/boardviewcraft" 2>/dev/null || true
install -m 644 "$LINUX/boardviewcraft.desktop" "$STAGE/share/applications/boardviewcraft.desktop"

if [[ -f "$LINUX/io.github.elbuit.BoardViewCraft.metainfo.xml.in" ]]; then
  DATE="$(date +%Y-%m-%d)"
  sed -e "s/@VERSION@/$VERSION/g" -e "s/@DATE@/$DATE/g" \
    "$LINUX/io.github.elbuit.BoardViewCraft.metainfo.xml.in" > "$STAGE/share/metainfo/io.github.elbuit.BoardViewCraft.metainfo.xml"
fi

if [[ -d "$ROOT/assets/app-icon/hicolor" ]]; then
  cp -R "$ROOT/assets/app-icon/hicolor" "$STAGE/share/icons/"
fi

for doc in README.md LICENSE LICENSE-MIT LICENSE-APACHE; do
  if [[ -f "$ROOT/$doc" ]]; then
    cp "$ROOT/$doc" "$STAGE/share/doc/boardviewcraft/"
  fi
done

mkdir -p "$DIST"
tar -C "$WORK" -czf "$DIST/$BASENAME.tar.gz" "$BASENAME"
echo "wrote $DIST/$BASENAME.tar.gz"
