# BoardViewCraft

> High-performance, modern, pure Rust viewer for hardware repair and PCB inspection.

BoardViewCraft is an open-source boardview inspection application built from the ground up with **Rust**, **egui**, and hardware-accelerated **wgpu**. It is designed to provide fluid navigation, clear part and pad rendering, and intuitive diagnostics for dense multi-layer PCB designs, with the board schematic PDF side by side.

---

## Features

- **Fast Canvas**: Parallel spatial grid queries (`Rayon`) and tessellated meshes (`Lyon`). Cached meshes are re-mapped while zooming and panning, so navigation stays smooth even on boards with tens of thousands of pins and vias.
- **Readable Board Rendering**: Filled part bodies, component outlines, and real pad shapes (round, rectangular, rotated) read from GenCAD `$PADS` data, so connectors such as PCIe edge fingers look like the physical board.
- **Bijective Coordinates**: Pan, zoom, flip (TOP/BOTTOM), rotate (90° / 180°), and horizontal/vertical mirror with consistent coordinate mapping.
- **Component Search**: Search parts and nets from the side panel. Picking a result selects only that part and centers it on the board.
- **Schematic PDF Viewer**:
  - Tiled rendering of zoomed pages, background text indexing, and page previews for instant jumps.
  - Text search box inside the PDF panel (`Enter` jumps to the next hit).
  - Cross-probing: selecting a part on the board finds it in the schematic, and clicking a part in the PDF selects it on the board.
  - Each board remembers its own schematic PDF; opening another board closes the current PDF and restores the one associated with the new board.
- **Net Flight Lines**:
  - **Radial Mode**: Direct flight lines radiating from the selected pin/via.
  - **Web Mode**: Minimum Spanning Tree (MST) linking all nodes on the active net.
  - Invisible when nothing is selected to prevent visual clutter.
- **Visual Pin Net Identification**:
  - Selected pins are rendered as an outline circle with the connected net name (e.g. `GND`, `+12V_GPU`) centered inside in high contrast.
  - Opposite-side pins of the selected net rendered in hollow outline X-Ray mode.
- **Overlay Photos**: Align a photo of the real board over each side and adjust it from the Actions menu.
- **Focused Side Panel**: Shows only the active tool (part details, net details, search, or photo adjustment).
- **Independent Theming**:
  - **Application UI**: Light and Dark modes.
  - **PCB Themes**: Light, Dark, Warm, Sweet Cold, Cadcraft, and a fully customizable **Custom** theme with editable color tokens and custom name persistence.
- **Multilingual Support**: English, Spanish, Catalan, French, German, and Italian.
- **Board Formats**: GenCAD (`.cad`), TVW, KiCad, BRD/BRD2, BDV, ASC, CPD, FZ, and XZZ boards.

---

## Menus

| Menu | Entries |
| :--- | :--- |
| **File** | Open Board, Open Recent, Preferences, Close Board |
| **Actions** | Board actions and Adjust Overlay Photo |
| **View** | Zoom, show/hide Pins and Vias |
| **Help** | About (version and build information) |

---

## Quick Start

### Download

Prebuilt packages for Linux (AppImage, tar.gz, Flatpak), Windows, and macOS are published on the [Releases](https://github.com/elbuit/BoardViewCraft/releases) page.

### Prerequisites (building from source)
- [Rust toolchain](https://www.rust-lang.org/tools/install) (Rust 1.95+)

### Building and Running

```bash
# Clone the repository
git clone https://github.com/elbuit/BoardViewCraft.git
cd BoardViewCraft

# Run with Cargo
cargo run -p boardviewcraft --release
```

### Running Tests

```bash
cargo test --workspace --release
```

---

## Keyboard and Mouse

| Shortcut | Action |
| :--- | :--- |
| `F` | Auto-fit board to window |
| `Space` | Flip board side (TOP / BOTTOM) |
| `R` | Rotate 90° Clockwise |
| `Shift + R` | Rotate 90° Counter-Clockwise |
| `M` | Mirror X (Horizontal) |
| `Shift + M` | Mirror Y (Vertical) |
| `Esc` | Clear selection / Reset flight lines |
| `Ctrl + ,` | Open Preferences |
| `Scroll` | Zoom into cursor position (2.5× per wheel notch) |
| `Drag` | Pan canvas |
| `Right Click` / `Ctrl + Click` | Context menu |
| `PageUp` / `PageDown`, `←` / `→` | Previous / next PDF page |
| `+` / `-` | Zoom PDF page |

Board shortcuts are ignored while a text field has focus.

---

## Project Architecture

```
BoardViewCraft/
├── apps/
│   └── boardviewcraft/ # Application entry point
├── crates/
│   ├── bvc-core/       # Core geometry, board parsers, spatial grid, models
│   └── ui-egui/        # egui/wgpu canvas, PDF viewer, tessellator, themes, i18n, preferences
├── packaging/          # Linux, Windows, and macOS packaging scripts
└── assets/             # Fonts, icons, and test assets
```

---

## License

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
