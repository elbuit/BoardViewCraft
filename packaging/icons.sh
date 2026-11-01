#!/usr/bin/env bash
# Regenerate the app icons from assets/app-icon/boardviewcraft.svg, the master vector.
#
# Needs: resvg (brew install resvg / cargo install resvg --locked) and python3 (stdlib only). The
# macOS .icns uses iconutil when it exists (macOS) and a small stdlib writer otherwise (Linux,
# Windows), so it can be regenerated anywhere. The outputs are committed, so building and
# packaging never need these tools.
#
#   packaging/icons.sh
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIR="$ROOT/assets/app-icon"
SVG="$DIR/boardviewcraft.svg"
ID="io.github.elbuit.BoardViewCraft"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

command -v resvg >/dev/null || { echo "error: resvg not found (cargo install resvg --locked)" >&2; exit 1; }

# The master is a full-bleed 1024 rounded tile: right for Windows and Linux. macOS icons follow
# Apple's grid instead: an 824 px body centred on a transparent 1024 canvas.
MAC="$TMP/macos.svg"
{
  echo '<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 1024 1024">'
  echo '<image x="100" y="100" width="824" height="824" xlink:href="'"$SVG"'"/>'
  echo '</svg>'
} >"$MAC"

render() {
  local src="$1" size="$2" out="$3"
  resvg -w "$size" -h "$size" "$src" "$out" </dev/null
}

# 1024 px PNG on Apple's grid (also the runtime Dock icon on macOS, see apps/boardviewcraft/src/main.rs).
render "$MAC" 1024 "$DIR/boardviewcraft-1024.png"

# Linux hicolor theme (full bleed; hicolor/256x256 is also the runtime icon on Windows and Linux).
# The desktop file and metainfo use the app id; the AppImage step in packaging/linux/package.sh
# also reads boardviewcraft.png, so both names are written.
for s in 16 24 32 48 64 128 256 512; do
  mkdir -p "$DIR/hicolor/${s}x${s}/apps"
  render "$SVG" "$s" "$DIR/hicolor/${s}x${s}/apps/$ID.png"
  cp "$DIR/hicolor/${s}x${s}/apps/$ID.png" "$DIR/hicolor/${s}x${s}/apps/boardviewcraft.png"
done
mkdir -p "$DIR/hicolor/scalable/apps"
cp "$SVG" "$DIR/hicolor/scalable/apps/$ID.svg"

# Windows .ico: PNG-compressed entries, 16-256 px. Embedded in boardviewcraft.exe by build.rs.
ICO_PNGS=()
for s in 16 20 24 32 40 48 64 128 256; do
  render "$SVG" "$s" "$TMP/ico-$s.png"
  ICO_PNGS+=("$TMP/ico-$s.png")
done
python3 - "$DIR/boardviewcraft.ico" "${ICO_PNGS[@]}" <<'PY'
import struct, sys
out, pngs = sys.argv[1], sys.argv[2:]
blobs = [open(p, "rb").read() for p in pngs]
head = struct.pack("<HHH", 0, 1, len(blobs))
entries, data, offset = b"", b"", 6 + 16 * len(blobs)
for b in blobs:
    w, h = struct.unpack(">II", b[16:24])  # IHDR
    entries += struct.pack("<BBBBHHII", w % 256, h % 256, 0, 0, 1, 32, len(b), offset)
    data += b
    offset += len(b)
open(out, "wb").write(head + entries + data)
PY

# macOS .icns (16-1024 px, Apple's grid).
SET="$TMP/boardviewcraft.iconset"
mkdir -p "$SET"
for s in 16 32 128 256 512; do
  render "$MAC" "$s" "$SET/icon_${s}x${s}.png"
  render "$MAC" $((s * 2)) "$SET/icon_${s}x${s}@2x.png"
done
if command -v iconutil >/dev/null; then
  iconutil -c icns -o "$DIR/boardviewcraft.icns" "$SET"
else
  # No iconutil (Linux, Windows): write the same PNG-based slots directly (stdlib only).
  python3 - "$DIR/boardviewcraft.icns" "$SET" <<'PY'
import struct, sys
out, iconset = sys.argv[1], sys.argv[2]
# OSType -> iconset file; every slot holds PNG data, as iconutil writes for these types.
slots = [
    ("icp4", "icon_16x16.png"), ("ic11", "icon_16x16@2x.png"),
    ("icp5", "icon_32x32.png"), ("ic12", "icon_32x32@2x.png"),
    ("ic07", "icon_128x128.png"), ("ic13", "icon_128x128@2x.png"),
    ("ic08", "icon_256x256.png"), ("ic14", "icon_256x256@2x.png"),
    ("ic09", "icon_512x512.png"), ("ic10", "icon_512x512@2x.png"),
]
body = b""
for ostype, name in slots:
    png = open(f"{iconset}/{name}", "rb").read()
    body += ostype.encode() + struct.pack(">I", 8 + len(png)) + png
open(out, "wb").write(b"icns" + struct.pack(">I", 8 + len(body)) + body)
PY
fi
echo "icons written to $DIR"
