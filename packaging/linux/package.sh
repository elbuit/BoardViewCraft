#!/usr/bin/env bash
# Build and package BoardViewCraft for Linux (<arch> is x86_64, aarch64, or riscv64):
#
#   $DIST/boardviewcraft-<version>-linux-<arch>.AppImage
#   $DIST/boardviewcraft-<version>-linux-<arch>.deb
#   $DIST/boardviewcraft-<version>-linux-<arch>.rpm
#   $DIST/boardviewcraft-<version>-linux-<arch>.tar.gz
#
# Usage: packaging/linux/package.sh [--skip-build] [--formats "appimage deb rpm tar"]
#
set -euo pipefail

# shellcheck source=../env.sh
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
HERE="$ROOT/packaging/linux"
APP_ID="boardviewcraft"

SKIP_BUILD=0
FORMATS="appimage deb rpm tar"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-build) SKIP_BUILD=1; shift ;;
    --formats) FORMATS="$2"; shift 2 ;;
    -h | --help) echo "Usage: packaging/linux/package.sh [--skip-build] [--formats \"appimage deb rpm tar\"]"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

ARCH="${CROSS_ARCH:-$(uname -m)}"
case "$ARCH" in
  x86_64) ARCH=x86_64; DEB_ARCH=amd64 ;;
  aarch64 | arm64) ARCH=aarch64; DEB_ARCH=arm64 ;;
  riscv64) ARCH=riscv64; DEB_ARCH=riscv64 ;;
  *) echo "unsupported architecture $ARCH" >&2; exit 2 ;;
esac

export BVC_MAINTAINER="${BVC_MAINTAINER:-BoardViewCraft maintainers <elbuit@users.noreply.github.com>}"
export RELEASE="${RELEASE:-1}"
export NFPM_ARCH="$DEB_ARCH"

BASENAME="boardviewcraft-$VERSION-linux-$ARCH"
echo "==> BoardViewCraft $VERSION for Linux $ARCH ($FORMATS)"

if [[ "$SKIP_BUILD" = 0 ]]; then
  if [[ -n "${CROSS_TARGET:-}" ]]; then
    (cd "$ROOT" && cargo build --release --package boardviewcraft --target "$CROSS_TARGET" --ignore-rust-version)
  else
    (cd "$ROOT" && cargo build --release --package boardviewcraft --ignore-rust-version)
  fi
fi

BIN="$CARGO_TARGET_DIR/${CROSS_TARGET:+$CROSS_TARGET/}release"
WORK="$CARGO_TARGET_DIR/linux-package"
STAGE="$WORK/root"
rm -rf "$WORK"

# ---- stage an FHS tree -----------------------------------------------------------
install -Dm755 "$BIN/boardviewcraft" "$STAGE/usr/bin/boardviewcraft"
"${CROSS_COMPILE:-}strip" "$STAGE/usr/bin/boardviewcraft" 2>/dev/null || true
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
  # Ensure both boardviewcraft.png and io.github.elbuit.BoardViewCraft.png exist in every size
  find "$STAGE/usr/share/icons" -name "*.png" | while read -r icon; do
    dir="$(dirname "$icon")"
    if [[ ! -f "$dir/io.github.elbuit.BoardViewCraft.png" && -f "$dir/boardviewcraft.png" ]]; then
      cp "$dir/boardviewcraft.png" "$dir/io.github.elbuit.BoardViewCraft.png"
    fi
    if [[ ! -f "$dir/boardviewcraft.png" && -f "$dir/io.github.elbuit.BoardViewCraft.png" ]]; then
      cp "$dir/io.github.elbuit.BoardViewCraft.png" "$dir/boardviewcraft.png"
    fi
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

# ---- .deb and .rpm (via nfpm) ----------------------------------------------------
if has deb || has rpm; then
  if command -v nfpm >/dev/null 2>&1; then
    NFPM_CONFIG="$WORK/nfpm.yaml"
    sed -e "s|\${STAGE}|$STAGE|g" "$HERE/nfpm.yaml" > "$NFPM_CONFIG"

    if has deb; then
      nfpm package -f "$NFPM_CONFIG" -p deb -t "$DIST"
      echo "wrote $DIST (deb)"
    fi
    if has rpm; then
      nfpm package -f "$NFPM_CONFIG" -p rpm -t "$DIST"
      echo "wrote $DIST (rpm)"
    fi
  else
    echo "warning: nfpm not found; skipping .deb and .rpm (install from https://nfpm.goreleaser.com)" >&2
  fi
fi

# ---- AppImage --------------------------------------------------------------------
if has appimage; then
  if [[ "$ARCH" == "riscv64" ]]; then
    echo "skipping AppImage on riscv64 (not supported by runtime)"
  else
    APPDIR="$WORK/BoardViewCraft.AppDir"
    cp -R "$STAGE" "$APPDIR"
    ln -s usr/bin/boardviewcraft "$APPDIR/AppRun"
    cp "$HERE/boardviewcraft.desktop" "$APPDIR/boardviewcraft.desktop"

    ICON_SRC="$STAGE/usr/share/icons/hicolor/256x256/apps/boardviewcraft.png"
    if [[ -f "$ICON_SRC" ]]; then
      cp "$ICON_SRC" "$APPDIR/boardviewcraft.png"
      cp "$ICON_SRC" "$APPDIR/io.github.elbuit.BoardViewCraft.png"
      ln -s "boardviewcraft.png" "$APPDIR/.DirIcon"
    fi

    TOOL="${APPIMAGETOOL:-$(command -v appimagetool || true)}"
    if [[ -z "$TOOL" ]]; then
      TOOL="$CARGO_TARGET_DIR/appimagetool-$ARCH.AppImage"
      if [[ ! -x "$TOOL" ]]; then
        echo "Downloading appimagetool for $ARCH..."
        curl --proto '=https' -fsSL -o "$TOOL" "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$ARCH.AppImage" || true
        chmod +x "$TOOL" 2>/dev/null || true
      fi
    fi

    if [[ -x "$TOOL" ]]; then
      OUT="$(cd "$DIST" && pwd)/$BASENAME.AppImage"
      APPDIR="$(cd "$APPDIR" && pwd)"
      TOOL="$(cd "$(dirname "$TOOL")" && pwd)/$(basename "$TOOL")"

      echo "Building AppImage -> $OUT"
      (cd "$DIST" && ARCH="$ARCH" APPIMAGE_EXTRACT_AND_RUN=1 "$TOOL" --no-appstream "$APPDIR" "$OUT") || true
      echo "wrote $OUT"
    fi
  fi
fi

echo "==> done"
ls -lh "$DIST"
