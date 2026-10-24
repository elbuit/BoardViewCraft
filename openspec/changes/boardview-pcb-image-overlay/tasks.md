# Tasks: PCB Background Image Overlay

- [x] 1. Create OpenSpec proposal, spec, and design documents <!-- id: 1 -->
- [x] 2. Add `image` dependency to `crates/bvc-app/Cargo.toml` <!-- id: 2 -->
- [x] 3. Create `crates/bvc-app/src/overlay.rs` with `OverlaySlot`, `OverlayManager`, texture loading, and projection math <!-- id: 3 -->
- [x] 4. Integrate overlay quad rendering and Alt+Drag/Alt+Wheel direct manipulation in `crates/bvc-app/src/canvas.rs` <!-- id: 4 -->
- [x] 5. Add `Tab::Overlay` in `crates/bvc-app/src/side_panel.rs` with sliders (opacity, position, size, rotation, lock) <!-- id: 5 -->
- [x] 6. Add i18n keys for all overlay controls in `crates/bvc-app/src/i18n.rs` (EN, ES, CA) <!-- id: 6 -->
- [x] 7. Wire `OverlayManager` into `BoardViewApp` in `crates/bvc-app/src/main.rs` <!-- id: 7 -->
- [x] 8. Compile, test workspace, and verify all unit tests pass <!-- id: 8 -->
