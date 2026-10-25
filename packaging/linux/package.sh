#!/usr/bin/env bash
# Build and package BoardViewCraft as an AppImage (and optional tar.gz)
# Mirroring CADCraft packaging strategy (packaging/linux/package.sh)
#
# Usage: packaging/linux/package.sh [--skip-build] [--formats "appimage tar"]
#
set -euo pipefail

# shellcheck source=../env.sh
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
HERE="$ROOT/packaging/linux"
APP_ID="boardviewcraft"

SKIP_BUILD=0
FORMATS="appimage tar"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-build) SKIP_BUILD=1; shift ;;
    --formats) FORMATS="$2"; shift 2 ;;
    -h | --help) echo "Usage: packaging/linux/package.sh [--skip-build] [--formats \"appimage tar\"]"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

ARCH="$(uname -m)"
case "$ARCH" in
  x86_64) ARCH=x86_64 ;;
  aarch64 | arm64) ARCH=aarch64 ;;
  *) echo "unsupported architecture $ARCH" >&2; exit 2 ;;
esac

BASENAME="boardviewcraft-$VERSION-linux-$ARCH"
echo "==> BoardViewCraft $VERSION for Linux $ARCH ($FORMATS)"

if [[ "$SKIP_BUILD" = 0 ]]; then
  (cd "$ROOT" && cargo build --release --package bvc-app --ignore-rust-version)
fi

BIN="$CARGO_TARGET_DIR/release"
WORK="$CARGO_TARGET_DIR/linux-package"
STAGE="$WORK/root"
rm -rf "$WORK"

# ---- stage an FHS tree -----------------------------------------------------------
install -Dm755 "$BIN/boardviewcraft" "$STAGE/usr/bin/boardviewcraft"
strip "$STAGE/usr/bin/boardviewcraft" 2>/dev/null || true
install -Dm644 "$HERE/boardviewcraft.desktop" "$STAGE/usr/share/applications/boardviewcraft.desktop"
install -Dm644 "$HERE/boardviewcraft.desktop" "$STAGE/usr/share/applications/io.github.elbuit.BoardViewCraft.desktop"

# Metainfo
if [[ -f "$HERE/io.github.elbuit.BoardViewCraft.metainfo.xml.in" ]]; then
  DATE="$(date +%Y-%m-%d)"
  sed -e "s/@VERSION@/$VERSION/g" -e "s/@DATE@/$DATE/g" \
    "$HERE/io.github.elbuit.BoardViewCraft.metainfo.xml.in" > "$WORK/io.github.elbuit.BoardViewCraft.metainfo.xml"
  install -Dm644 "$WORK/io.github.elbuit.BoardViewCraft.metainfo.xml" "$STAGE/usr/share/metainfo/io.github.elbuit.BoardViewCraft.metainfo.xml"
fi

mkdir -p "$STAGE/usr/share/icons"
if [[ -d "$ROOT/assets/app-icon/hicolor" ]]; then
  cp -R "$ROOT/assets/app-icon/hicolor" "$STAGE/usr/share/icons/"
  # Rename app icons if named cadcraft
  find "$STAGE/usr/share/icons" -name "*cadcraft.png" | while read -r icon; do
    dir="$(dirname "$icon")"
    mv "$icon" "$dir/boardviewcraft.png"
  done
fi

has() { local format="$1"; case " $FORMATS " in *" $format "*) return 0 ;; *) return 1 ;; esac; }

# ---- .tar.gz ---------------------------------------------------------------------
if has tar; then
  mkdir -p "$WORK/tar"
  cp -R "$STAGE/usr" "$WORK/tar/$BASENAME"
  tar -C "$WORK/tar" -czf "$DIST/$BASENAME.tar.gz" "$BASENAME"
  echo "wrote $DIST/$BASENAME.tar.gz"
fi

# ---- AppImage --------------------------------------------------------------------
if has appimage; then
  APPDIR="$WORK/BoardViewCraft.AppDir"
  cp -R "$STAGE" "$APPDIR"
  ln -s usr/bin/boardviewcraft "$APPDIR/AppRun"
  cp "$HERE/boardviewcraft.desktop" "$APPDIR/boardviewcraft.desktop"

  ICON_SRC="$STAGE/usr/share/icons/hicolor/256x256/apps/boardviewcraft.png"
  if [[ -f "$ICON_SRC" ]]; then
    cp "$ICON_SRC" "$APPDIR/boardviewcraft.png"
    ln -s "boardviewcraft.png" "$APPDIR/.DirIcon"
  fi

  TOOL="${APPIMAGETOOL:-$(command -v appimagetool || true)}"
  if [[ -z "$TOOL" ]]; then
    TOOL="$CARGO_TARGET_DIR/appimagetool-$ARCH.AppImage"
    if [[ ! -x "$TOOL" ]]; then
      echo "Downloading appimagetool for $ARCH..."
      curl --proto '=https' -fsSL -o "$TOOL" "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$ARCH.AppImage"
      chmod +x "$TOOL"
    fi
  fi

  OUT="$(cd "$DIST" && pwd)/$BASENAME.AppImage"
  APPDIR="$(cd "$APPDIR" && pwd)"
  TOOL="$(cd "$(dirname "$TOOL")" && pwd)/$(basename "$TOOL")"

  echo "Building AppImage -> $OUT"
  (cd "$DIST" && ARCH="$ARCH" APPIMAGE_EXTRACT_AND_RUN=1 "$TOOL" --no-appstream "$APPDIR" "$OUT")
  echo "wrote $OUT"
fi

echo "==> done"
ls -lh "$DIST"
