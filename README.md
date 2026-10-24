# BoardViewCraft

> High-performance, modern, pure Rust viewer for hardware repair and PCB inspection.

BoardViewCraft is an open-source boardview inspection application built from the ground up with **Rust**, **egui**, and hardware-accelerated **wgpu**. It is designed to provide fluid 60+ FPS navigation, subpixel LOD rendering, and intuitive diagnostics for dense multi-layer PCB designs.

---

## Features

- **Blazing Fast Canvas & LOD**: Parallel spatial grid queries (`Rayon`) and tessellated mesh decimation (`Lyon`) ensuring smooth 60+ FPS navigation even on boards with tens of thousands of pins and vias.
- **Bijective Coordinates**: Pan, zoom, flip (TOP/BOTTOM), rotate (90° / 180°), and horizontal/vertical mirror with consistent coordinate mapping.
- **Net Flight Lines**:
  - **Radial Mode**: Clear direct flight lines radiating from the selected pin/via.
  - **Web Mode**: Spiderweb Minimum Spanning Tree (MST) linking all nodes on the active net.
  - Invisible when nothing is selected to prevent visual clutter.
- **Visual Pin Net Identification**:
  - Selected pins are rendered as an outline circle with the connected net name (e.g. `GND`, `+12V_GPU`) centered inside in high contrast.
  - Opposite-side pins of the selected net rendered in hollow outline X-Ray mode.
- **Independent Theming**:
  - **Application UI**: Clean Light and Dark modes.
  - **PCB Themes**: Light, Dark, Warm, Sweet Cold, Cadcraft, and fully customizable **Custom** theme with customizable color tokens and custom name persistence.
- **Multilingual Support**: Built-in support for English, Spanish, and Catalan.
- **Hardware Formats Supported**: Native support for CAD / TVW formats and board structures.

---

## Quick Start

### Prerequisites
- [Rust toolchain](https://www.rust-lang.org/tools/install) (Rust 1.90+)

### Building and Running

```bash
# Clone the repository
git clone https://github.com/your-username/boardviewcraft.git
cd boardviewcraft

# Run with Cargo
cargo run -p bvc-app --release
```

*(If using an older system compiler than egui lock requirements, add `--ignore-rust-version`)*:
```bash
cargo run -p bvc-app --release --ignore-rust-version
```

### Running Tests

```bash
cargo test --all --ignore-rust-version
```

---

## Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `F` | Auto-fit board to window |
| `Space` | Flip board side (TOP / BOTTOM) |
| `R` | Rotate 90° Clockwise |
| `Shift + R` | Rotate 90° Counter-Clockwise |
| `M` | Mirror X (Horizontal) |
| `Shift + M` | Mirror Y (Vertical) |
| `Esc` | Clear selection / Reset flight lines |
| `Ctrl + Scroll` | Zoom into cursor position |
| `Middle Click / Drag` | Pan canvas |

---

## Project Architecture

```
BoardViewCraft/
├── crates/
│   ├── bvc-core/       # Core geometry, board parsing (CAD/TVW), spatial grid, models
│   └── bvc-app/        # Egui/wgpu canvas, tessellator, themes, i18n, preferences
└── assets/             # Fonts, icons, and test assets
```

---

## License

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
