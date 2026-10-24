# Proposal: BoardViewCraft Zoom Presets, Screen-Space Text LOD & Cross-Platform Preferences Persistence

- **Change Name:** `boardview-zoom-presets-text-lod-persistence`
- **Status:** Draft / Proposed
- **Author:** sdd-propose
- **Date:** 2026-10-23
- **Target Workspaces:** `BoardViewCraft` (`bvc-app`, `bvc-core`)
- **Reference Codebases:** `OpenBoardView`, `cadcraft`

---

## 1. Intent & Problem Statement

Technicians and diagnostic hardware engineers frequently navigate between macro-level board overviews and microscopic pin-level component inspections. Three critical friction points currently degrade user experience and workflow ergonomics in BoardViewCraft:

1. **Text Label Saturation & Screen Clutter on Zoom-Out:**
   - When inspecting the full board or zooming out, component designators and pin labels (rendered as egui text painters) saturate the screen viewport, colliding into illegible dense blobs that obscure traces, vias, and pad geometry.
   - Component labels are currently governed only by a blunt static zoom check (`vp.scale > 0.03`), and pin annotations are not yet dynamically decimated based on screen-projected footprint dimensions or pin pixel size.
   - Screen-space LOD (Level of Detail) decimation is required so that:
     - Part designator labels are only drawn if their screen-projected component footprint is large enough to comfortably accommodate legible text without overlapping adjacent components (or when specifically selected).
     - Pin annotations/numbers are only rendered when zoomed in sufficiently close (e.g. pin screen radius $\ge 8.0\,\text{px}$ or bounding area allows clear legibility).

2. **Rigid Viewport Zoom Controls (Missing Zoom Presets & Viewport-Aware Fit):**
   - The top toolbar currently only features a static "1:1" button (`vp.scale = 0.05`) and a basic window fit (`fit_to_rect`).
   - Standard CAD and diagnostic workflows require intuitive, discrete magnification steps:
     - Preset zoom levels in View / Toolbar: **25%**, **50%**, **100% (1:1)**, **150%**, **200%**.
     - An enhanced "Fit to Window" / "Zoom to Fit" action that accurately calculates scale and center based on the canvas viewport dimensions and the board's bounding box, with shortcuts and toolbar buttons.

3. **Loss of User Configuration Between Sessions (Lack of Preferences Persistence):**
   - Theme settings, custom color palettes, UI language (English/Spanish/Catalan), and visualization toggles (net spiderweb, opposite-side x-ray, adaptive LOD) reset to defaults every time the application is closed.
   - Following CADCraft's battle-tested cross-platform config architecture (`cadcraft/apps/cadcraft/src/workspace.rs`), BoardViewCraft needs an atomic, corruption-resistant configuration persistence mechanism (`preferences.json`) targeting native OS directories:
     - Linux: `$BOARDVIEWCRAFT_CONFIG_DIR`, `$XDG_CONFIG_HOME/boardviewcraft`, or `~/.config/boardviewcraft`
     - Windows: `%APPDATA%\BoardViewCraft`
     - macOS: `~/Library/Application Support/BoardViewCraft`
     - Atomic write via temporary file (`.preferences-<pid>-<seq>.tmp`) and rename.

---

## 2. Scope & Technical Architecture

### 2.1 Zoom-Dependent Screen-Space Text LOD Decimation

- **Component Designators (Part Labels):**
  - Implement a dual-tier decimation check in `canvas::show_canvas`:
    1. Preference gate: `prefs.show_part_labels` must be enabled (always draw if component is selected, regardless of gate).
    2. Screen-space footprint dimension threshold:
       - Calculate projected screen dimensions of the component's bounding box: `w_screen = bbox.width() * vp.scale`, `h_screen = bbox.height() * vp.scale`.
       - Render the part label only if `w_screen >= 24.0` and `h_screen >= 12.0` (or if the component is active in `selected_part_idx`), ensuring labels never overpower sub-miniature passives (0201/0402 capacitors and resistors) on full-board view.
       - Font size scales adaptively or clamps cleanly to avoid visual vibration.

- **Pin Annotations & Numbers:**
  - Implement pin text rendering gated by `prefs.show_pin_annotations`.
  - Pin numbers/names (e.g. "1", "A1", "VCC") are rendered on pins when:
    - The pin's screen radius `r_screen = pin.radius * vp.scale >= 8.0` pixels, or
    - The pin is directly hovered or selected, or belongs to the currently active net.
  - Pin text is centered within the pin circular or quad bounds with crisp contrast against `pin_default` / `pin_highlight`.

### 2.2 Zoom Presets & Viewport-Aware "Fit to Window"

- **Zoom Level Definition:**
  - Define baseline reference scale: $100\%$ ($1:1$) corresponds to $\text{scale} = 0.05$ pixels/mil (or board 1 mil = 0.05 px).
  - Presets:
    - $25\% \implies \text{scale} = 0.0125$
    - $50\% \implies \text{scale} = 0.0250$
    - $100\% \implies \text{scale} = 0.0500$ ($1:1$)
    - $150\% \implies \text{scale} = 0.0750$
    - $200\% \implies \text{scale} = 0.1000$
  - Add ComboBox or segmented preset buttons in the toolbar (`25%`, `50%`, `100%`, `150%`, `200%`) alongside smooth mouse wheel zooming.

- **Fit to Window / Zoom to Fit:**
  - Refine `Viewport::fit_to_rect` in `crates/bvc-app/src/canvas.rs`:
    - Accurately compute canvas inner rect dimensions excluding panels.
    - Include comfortable padding margin ($40\,\text{px}$).
    - Support hotkey `F` and dedicated Toolbar / Menu action.

### 2.3 Cross-Platform Preferences Persistence (`preferences.json`)

- **Storage Location Resolution:**
  - Implement `bvc-app::preferences::config_dir()` / `preferences_path()` mirroring `CADCraft`:
    - Check environment variable override `BOARDVIEWCRAFT_CONFIG_DIR`.
    - Linux/BSD: `$XDG_CONFIG_HOME/boardviewcraft` fallback to `$HOME/.config/boardviewcraft`.
    - macOS: `$HOME/Library/Application Support/BoardViewCraft`.
    - Windows: `%APPDATA%\BoardViewCraft`.
- **Atomic Save Pipeline:**
  - Serialize `Preferences` to formatted JSON with schema versioning (`{"version": 1, "preferences": ...}`).
  - Guard payload size ($< 64\,\text{KiB}$).
  - Write payload to unique atomic temp file `.preferences-<pid>-<counter>.tmp` with Unix permissions `0o600`.
  - Flush, `sync_all()`, close file descriptor, and atomically rename to `preferences.json`.
- **Lifecycle Integration:**
  - **Startup (`BoardViewApp::new`):**
    - Load `preferences.json` if present. If missing or corrupted, log a warning, fall back gracefully to `Preferences::default()`, and proceed without crashing.
    - Apply stored theme, custom palette, and language to egui context immediately.
  - **On Change:**
    - Whenever `show_preferences_window` returns `true` (or when theme/language switches), trigger atomic persistence write.
  - **On Shutdown / Drop:**
    - Perform final flush save if modified.

---

## 3. User Experience & Visual Design

1. **Toolbar Additions:**
   - Zoom presets dropdown or quick buttons (`25%`, `50%`, `100%`, `150%`, `200%`).
   - "Fit to Window" button with shortcut reminder `(F)`.
2. **Decluttered Canvas:**
   - Zooming out displays a clear, unobstructed topology of the board outline, component boundaries, and traces without text collisions.
   - Zooming in smoothly transitions component designators and pin identifiers into view only where legible.
3. **Session Continuity:**
   - Launching BoardViewCraft restores user theme preferences, custom colors, and language choice seamlessly across restarts.

---

## 4. Testing & Verification Plan

1. **Unit Tests (`crates/bvc-app`):**
   - Test `config_dir_from(platform, env_fn)` across Linux, macOS, and Windows environment mock mappings.
   - Test atomic write and recovery of `Preferences` JSON serialization and deserialization.
   - Test corrupted or truncated file handling (fallback to defaults without panicking).
   - Test LOD threshold calculations for part bounding boxes and pin radii.
   - Test zoom preset calculations and `fit_to_rect` scale clamping.

2. **Integration & Manual UI Validation:**
   - Load large real-world board: `MS-V375 REV 7.0.cad` (13,000+ pins, 2,800+ components).
   - Test full-board zoom out: observe that text clutter is decimated and FPS remains at 60+.
   - Zoom in to discrete ICs and connectors: observe clean appearance of part names and pin numbers.
   - Click zoom presets (25%, 50%, 100%, 150%, 200%) and verify scale updates correctly.
   - Change theme, custom colors, and language; exit application and relaunch; verify all settings persist.
