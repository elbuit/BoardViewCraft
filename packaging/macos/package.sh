#!/usr/bin/env bash
# packaging/macos/package.sh
# Build, sign and package BoardViewCraft into a macOS .app and .dmg
#
# Usage: packaging/macos/package.sh [--arch universal|aarch64|x86_64] [--skip-build]

set -euo pipefail

# shellcheck source=../env.sh
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
HERE="$ROOT/packaging/macos"

ARCH=universal
SKIP_BUILD=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --arch) ARCH="$2"; shift 2 ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    -h | --help) echo "Usage: packaging/macos/package.sh [--arch universal|aarch64|x86_64] [--skip-build]"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

case "$ARCH" in
  universal) TARGETS=(aarch64-apple-darwin x86_64-apple-darwin) ;;
  aarch64) TARGETS=(aarch64-apple-darwin) ;;
  x86_64) TARGETS=(x86_64-apple-darwin) ;;
  *) echo "unknown --arch $ARCH" >&2; exit 2 ;;
esac

export MACOSX_DEPLOYMENT_TARGET=11.0
WORK="$CARGO_TARGET_DIR/macos-package-$ARCH"
APP="$WORK/BoardViewCraft.app"
DMG="$DIST/boardviewcraft-$VERSION-macos-$ARCH.dmg"
ZIP="$DIST/boardviewcraft-$VERSION-macos-$ARCH.zip"

echo "==> Building BoardViewCraft $VERSION for macOS ($ARCH)..."

if [[ "$SKIP_BUILD" = 0 ]]; then
  for t in "${TARGETS[@]}"; do
    echo "==> Compiling for target $t..."
    cargo build --manifest-path "$ROOT/Cargo.toml" --package bvc-app --release --target "$t" --ignore-rust-version
  done
fi

rm -rf "$WORK"
mkdir -p "$APP/Contents/MacOS"
mkdir -p "$APP/Contents/Resources"

# Generate Info.plist
sed "s/@VERSION@/$VERSION/g" "$HERE/Info.plist.in" > "$APP/Contents/Info.plist"

# Bundle executable (lipo if universal)
if [[ "${#TARGETS[@]}" -eq 2 ]]; then
  echo "==> Creating universal binary with lipo..."
  lipo -create \
    "$CARGO_TARGET_DIR/${TARGETS[0]}/release/boardviewcraft" \
    "$CARGO_TARGET_DIR/${TARGETS[1]}/release/boardviewcraft" \
    -output "$APP/Contents/MacOS/boardviewcraft"
else
  cp "$CARGO_TARGET_DIR/${TARGETS[0]}/release/boardviewcraft" "$APP/Contents/MacOS/boardviewcraft"
fi
chmod +x "$APP/Contents/MacOS/boardviewcraft"

# Ad-hoc sign bundle
if command -v codesign >/dev/null 2>&1; then
  echo "==> Codesigning .app bundle (ad-hoc)..."
  codesign --force --deep --sign - --entitlements "$HERE/entitlements.plist" "$APP"
fi

# Create portable .zip
(cd "$WORK" && zip -r -y "$ZIP" "BoardViewCraft.app")
echo "==> Created portable macOS ZIP: $ZIP"

# Create .dmg if hdiutil is available (native macOS)
if command -v hdiutil >/dev/null 2>&1; then
  echo "==> Creating macOS DMG..."
  DMG_STAGE="$WORK/dmg-stage"
  mkdir -p "$DMG_STAGE"
  cp -R "$APP" "$DMG_STAGE/"
  ln -s /Applications "$DMG_STAGE/Applications"
  rm -f "$DMG"
  hdiutil create -volname "BoardViewCraft" -srcfolder "$DMG_STAGE" -ov -format UDZO "$DMG"
  echo "==> Created macOS DMG: $DMG"
fi
