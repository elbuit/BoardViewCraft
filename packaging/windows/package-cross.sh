#!/usr/bin/env bash
# packaging/windows/package-cross.sh
# Build Windows x64 release from Linux (e.g., Kubuntu) using cargo-xwin or mingw-w64.
set -euo pipefail

# shellcheck source=../env.sh
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"

DIST="${DIST:-$ROOT/dist/release}"
mkdir -p "$DIST"
TARGET="x86_64-pc-windows-msvc"

echo "==> Cross-compiling BoardViewCraft $VERSION for Windows x64 ($TARGET)..."

if command -v cargo-xwin >/dev/null 2>&1; then
    cargo xwin build --manifest-path "$ROOT/Cargo.toml" --package boardviewcraft --release --target "$TARGET" --ignore-rust-version
    EXE_SRC="$CARGO_TARGET_DIR/$TARGET/release/boardviewcraft.exe"
elif command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
    TARGET="x86_64-pc-windows-gnu"
    cargo build --manifest-path "$ROOT/Cargo.toml" --package boardviewcraft --release --target "$TARGET" --ignore-rust-version
    EXE_SRC="$CARGO_TARGET_DIR/$TARGET/release/boardviewcraft.exe"
else
    echo "error: Neither 'cargo-xwin' nor 'x86_64-w64-mingw32-gcc' was found." >&2
    echo "Install cargo-xwin (cargo install cargo-xwin) or mingw-w64 (sudo apt install mingw-w64)." >&2
    exit 1
fi

ZIP_OUT="$DIST/boardviewcraft-$VERSION-windows-x64-portable.zip"
STAGE_DIR="$CARGO_TARGET_DIR/windows-stage-x64"
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR"

cp "$EXE_SRC" "$STAGE_DIR/boardviewcraft.exe"
if [ -f "$ROOT/README.md" ]; then
    cp "$ROOT/README.md" "$STAGE_DIR/"
fi
if [ -f "$(dirname "${BASH_SOURCE[0]}")/license.rtf" ]; then
    cp "$(dirname "${BASH_SOURCE[0]}")/license.rtf" "$STAGE_DIR/"
fi
if [ -f "$ROOT/LICENSE-MIT" ]; then
    cp "$ROOT/LICENSE-MIT" "$STAGE_DIR/"
fi
if [ -f "$ROOT/LICENSE-APACHE" ]; then
    cp "$ROOT/LICENSE-APACHE" "$STAGE_DIR/"
fi

(cd "$STAGE_DIR" && zip -r "$ZIP_OUT" ./*)
echo "==> Successfully created Windows portable release: $ZIP_OUT"
