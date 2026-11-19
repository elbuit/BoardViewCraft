<img src="assets/app-icon/boardviewcraft-mark.svg" alt="BoardViewCraft logo" width="96" align="right">

# BoardViewCraft

> High-performance, modern, pure Rust viewer for hardware repair and PCB inspection.

BoardViewCraft is an open-source boardview and schematic inspection workbench built from the ground up with **Rust**, **egui**, and hardware-accelerated **wgpu**. It provides ultra-fast navigation, clear component rendering, real pad geometries, camera overlay photo alignment, and native dual-pane split inspection with PDF and vector CAD schematics.

![BoardViewCraft: Main workbench view with quick toolbar, synchronized Both view, net flight lines, and inspection panel](docs/screenshots/boardviewcraft-v024.png)

---

## Features

- **Multi-Net Pinning & Inspection**:
  - Pin multiple nets simultaneously, each highlighted in a dedicated, distinct high-contrast color across both faces of the board.
  - Active net selection remains highlighted while pinned nets maintain persistent visual traces for comparison and bus tracing.
- **Related Nets Discovery**:
  - Automatically identifies and displays related signals connected through passive bridge components (series resistors, low-value inductors, coupling capacitors).
- **Measurement Tolerance Comparison**:
  - Evaluates recorded technician measurements against expected reference values with configurable tolerance thresholds.
  - Displays instant visual pass/fail/deviation badges (green/yellow/red) in both pin lists and component overview.
- **Smart Board Orientation**:
  - Automatically detects vertical boards and PCIe edge connectors, rotating them to comfortable landscape orientation with PCIe connectors positioned at the bottom.
- **Unified Quick View & Pads Controls**:
  - Consolidated view toggles (vias, pins, test points, substrate hatching) and pad styling into a single streamlined toolbar menu.
- **Fast Canvas**: Parallel spatial grid queries (`Rayon`) and hardware-accelerated tessellation (`Lyon`). Cached geometry meshes are re-mapped during pan/zoom for 60+ FPS navigation on dense PCBs with tens of thousands of pins and vias.
- **Quick Navigation Toolbar**:
  - History back/forward navigation (`<` / `>`) tracking inspected parts and nets.
  - One-click **Top** and **Bottom** view switching.
  - Integrated, searchable **Parts** and **Nets** combo boxes with instant auto-completion directly in the toolbar.
  - Quick zoom controls (`+`, `-`, reset).
- **Flexible Layouts & Dual-Pane Split View**:
  - **Single Pane**, **Split Vertical** (side by side), or **Split Horizontal** (stacked).
  - Any pane can display **Top**, **Bottom**, **Both** (top and bottom with synchronized or independent view), **PDF Schematic**, or **CAD Schematic**.
  - Configurable inspection side panel position (**Left** or **Right**).
  - Interactive resizable divider with reset shortcut.

  ![BoardViewCraft: Split view with board layout on the left, PDF schematic on the right with page scrubber, and side panel](docs/screenshots/boardviewcraft-v020.png)
- **Readable Board Rendering & Visual Flight Lines**:
  - Filled part bodies, component outlines, pin names, and physical pad geometries (round, rectangular, rotated) read from GenCAD `$PADS` and CAD records.
  - Clean pin selection: clicking an individual pin highlights only that pin and its connected net, without tinting the parent component body.
  - Accurate rotated component body outlines automatically synthesized for diagonal components (e.g. 45° or custom-angled parts) when file formats omit vector contours.
  - Radial and web net flight lines showing all connected pins on selected nets.
  - Optional dashed / dotted net flight lines mode to reduce visual clutter on dense nets.
  - **Opposite-Layer Differentiation**: Opposite-side vias and net lines crossing layers render at 50% opacity, making through-hole and multi-layer connections instantly distinguishable at a glance.
- **Bijective Coordinates**: Pan, zoom, flip (TOP/BOTTOM), rotate (90° / 180°), and horizontal/vertical mirror with consistent coordinate mapping.
- **Advanced Search**:
  - Filter parts and nets with full wildcard matching (`*`, `?`), prefix chips (`U`, `R`, `C`, `Q`, `D`, `L`, `F`, `J`), comma-separated OR queries, and side filters (Top, Bottom, All).
- **Schematic PDF Viewer**:
  - Tiled multi-threaded rendering of zoomed pages, background text indexing, and vertical scrubber slider with preview badge for instant page jumping.
  - Bidirectional cross-probing: clicking a part or net on the board navigates to the schematic, and clicking references in the PDF navigates and centers the PCB.
  - Wheel scrolling for clean page turning and `Ctrl + Wheel` / pinch for precision zooming.
- **Vector CAD Schematics**:
  - Native vector parsing and rendering for KiCad (`.kicad_sch`) and Eagle (`.sch`) files.
  - Sheet switching, symbol graphics, net label inspection, and cross-probing.
- **Technician Notes & Reference Measurements**:
  - Attach structured inspection notes and physical test readings to individual component pins with dedicated inputs for **Diode Drop (D)**, **Resistance (R)**, **Voltage (V)**, and technician comments / diagnostic solutions.
  - **OpenBoardData (`.obdata`) Support**: Native import and parsing of OpenBoardData files. Automatically detects and loads matching companion `.obdata` files in the board directory, and highlights pins with recorded measurements on the PCB canvas.
  - Quick right-click context menu (**Add / Edit Measurement**) on any pin or component directly from the board canvas or side panel.
  - Component pin measurements overview: selecting any component automatically displays all recorded measurements across its pins in the side panel, cleanly omitting unmeasured entries.
  - Automatic extraction and display of repair metadata from OpenBoardData, XZZ, and TVW formats: component values (e.g. 560Ω, 100nF), diode readings, net voltages, and signal descriptions directly in the side panel and exported TSV reports.
  - Visual `📝` badges and inline measurement values in virtualized pin lists for instant test point identification.
  - Dedicated side panel tabs for Parts, Nets, and Notes.
  - Export full service inspection reports and complete Bill of Materials (BOM) in TSV.

  ![BoardViewCraft: Pin measurements modal dialog and component pin measurement inspection panel](docs/screenshots/boardviewcraft-v024-measurements.png)
- **Camera Overlays**:
  - Load real PCB inspection photos over Top and Bottom views.
  - Interactive multi-point pin alignment with opacity blending.
- **Independent Theming & Deep Customization**:
  - Application UI (Light/Dark) and rich PCB color presets: **Blue Carrot** (default), Cadcraft, Dark, Light, Warm, Sweet Cold, Cyberpunk, and **Olive** (realistic green board with silver solder pads and natural component tones).
  - **Substrate Patterns & Hatching**: selectable PCB substrate styling (Solid fill, Horizontal lines, Diagonal 45°, Crosshatch diamonds, or Square grid) rendered using a multi-scale supersampled mipmap texture tile chain for smooth, flicker-free antialiasing at every zoom level.
  - **Color Components by Type**: optional toggle in Preferences to color component bodies by their electrical function (resistors in black, ceramic capacitors in brown, ICs/transistors in matte black) across Olive, Cyberpunk, Dark, or custom themes.
  - In-depth theme editor: modify all color tokens, dedicated ground pin (GND) highlighting, toggle dashed flight lines, and clone existing presets into custom named themes.
  - Revert overrides at any time with one click.
- **Display & UI Scaling**:
  - Configurable interface scaling in Preferences (Auto-detecting desktop text scale, or manual presets from 80% to 175%) for sharp rendering on HiDPI and 4K displays.
- **Extensive Format Support**:
  - GenCAD (`.cad`), TVW (with physical PCIe fingers and layer decoding), Specctra DSN (`.dsn`), KiCad (`.kicad_pcb`), Eagle (`.brd`), EasyEDA (Standard & Pro), Altium Designer (`.PcbDoc`), Allegro (`.brd`), Boardview (BRD/BRD2, BDV, BV2, FlexBV, CST binary), `.pcb` / `.xzz` multi-layer boardview, OpenBoardData (`.obdata`), ODB++ (directories and `.tar.gz`/`.tgz`), IPC-2581, and IPC-D-356.
  - Transparent `.zip`, `.tar.gz`, and `.tgz` archive extraction and file sniffing.
- **Multilingual Support**: English, Spanish, Catalan, French, German, and Italian.

---

## Menus

| Menu | Entries |
| :--- | :--- |
| **File** | Open Board, Open PDF Schematic, Import OpenBoardData, Open Recent, Preferences, Close Board |
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
| `F` | Auto-fit the visible board view(s) |
| `Space` | Switch between the Top and Bottom tabs |
| `R` | Rotate 90° Clockwise |
| `Shift + R` | Rotate 90° Counter-Clockwise |
| `M` | Mirror X (Horizontal) |
| `Shift + M` | Mirror Y (Vertical) |
| `Esc` | Clear selection / Reset flight lines |
| `Ctrl + ,` (`Cmd + ,` on macOS) | Open / close Preferences |
| `Scroll` | Zoom into cursor position (2.5× per wheel notch) |
| `Drag` | Pan canvas |
| Arrow keys / `+` / `-` | Pan / zoom the active board view (last hovered or clicked) |
| `Right Click` / `Ctrl + Click` | Context menu |
| `PageUp` / `PageDown`, `←` / `→` | Previous / next PDF page |
| `+` / `-` | Zoom PDF page |

Board shortcuts are ignored while a text field has focus or Preferences is open, and match their modifiers exactly (`R` does not fire on `Shift + R`).

The defaults above for fit, Top/Bottom switch, rotate, mirror, board pan/zoom (arrow keys, `+` / `=`, `-`) and Preferences can be rebound in **Preferences → Keyboard shortcuts** (click a shortcut, press the new key; `Esc` cancels). A key already used by another action is refused. `Esc` / `Enter` and the PDF tab keys stay fixed.

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
