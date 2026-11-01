# Feature: app-icon

## Objective
Adopt the new BoardViewCraft app icon on every supported OS, generated the way pdfcraft does it (`../pdfcraft/assets/app-icon/README.md`, `../pdfcraft/packaging/icons.sh`).

## Problem / Why
User request 2026-11-01: the icon is provided as `tmp/icono.jpeg` (2048 px, no alpha; white outside the rounded tile). The same design exists as a vector, `tmp/icono.svg` (viewBox 0 0 1024). BoardViewCraft today only ships Linux hicolor PNGs: no window icon, no Windows exe icon, no macOS .icns.

## Decisions (orchestrator defaults)
- Master: `tmp/icono.svg` copied to `assets/app-icon/boardviewcraft.svg` (vector, transparent corners), rendered with `resvg` like pdfcraft. The JPEG is only the visual reference (it has no transparency).
- Outputs as pdfcraft: Linux hicolor 16–512 px + scalable SVG (app id `io.github.elbuit.BoardViewCraft`, keep existing `boardviewcraft.png` names if packaging uses them), Windows `.ico` (16, 20, 24, 32, 40, 48, 64, 128, 256; PNG entries), macOS `.icns` (16–1024 incl. @2x) and `boardviewcraft-1024.png` on Apple's grid (824 px body on a transparent 1024 canvas). `iconutil` is macOS only: build the .icns with Pillow (or another Linux-capable writer) instead.
- Wiring as pdfcraft: window icon at runtime, exe icon resource on Windows via build.rs, `.icns` in the macOS bundle (Info.plist), WiX product icon, desktop/flatpak/metainfo icon names.

## Tasks
- [x] T1 — `packaging/icons.sh` adapted from pdfcraft and the generated assets + `assets/app-icon/README.md` (route: delegated writer)
- [ ] T2 — Wire the icons per OS: runtime window icon, Windows exe resource, macOS bundle, WiX, Linux desktop/flatpak (route: same delegated writer)

## Acceptance criteria
- All sizes present with transparency outside the rounded tile; .ico and .icns open with Pillow and list the expected sizes.
- `cargo build --release` ok; `cargo test -p bvc-ui-egui --release` passes; window icon loads (unit test decoding the embedded PNG).

## Delivery strategy
ask-on-risk. Forecast small code (~100 lines) plus binary assets.

## Progress
- Branch `feat/app-icon` from `0686a76` (stacked on feat/board-tabs-layout).
- T1 done in `c5dacf3` (route: delegated writer). `packaging/icons.sh` (pdfcraft port) renders with resvg 0.49; `.icns` via `iconutil` on macOS, otherwise a stdlib writer of the PNG slots icp4/ic11/icp5/ic12/ic07/ic13/ic08/ic14/ic09/ic10 (Pillow's ICNS writer omits 16 and 32 px 1x). Evidence: hicolor 8 sizes x 2 names RGBA, centre opaque, corners alpha 0 from 48 px (rx=96 makes 16/24/32 px corner pixels antialiased: 128-143/48/0-16); 1024 PNG alpha bbox (100,100,924,924); Pillow reads `.ico` sizes 16,20,24,32,40,48,64,128,256 and decodes all 10 `.icns` entries 16-1024 px. Visual check vs `tmp/icono.jpeg` at 512 px: same layout; mean abs diff RGB 2.4/4.1/5.3, mostly edge antialiasing; SVG trace outlines slightly rougher and blue slightly more saturated.
