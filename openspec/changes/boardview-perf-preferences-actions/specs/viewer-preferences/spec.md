# Specification: Viewer Preferences & Themes

- **Capability:** `viewer-preferences`
- **Change:** `boardview-perf-preferences-actions`
- **Status:** Draft / Ready for Review
- **Target Components:** `bvc-app::theme`, `bvc-app::main`, `bvc-app::canvas`

---

## 1. Overview & Problem

Hardware repair technicians and reverse engineers operate in varied physical environments: dimly lit microscope workstations, bright lab benches, or situations where high-contrast color differentiation between adjacent signal pads is crucial for probing. Furthermore, existing users transitioning from OpenBoardView expect familiar color palettes (such as classic dark charcoal and gold highlights).

Currently, BoardViewCraft has a single hardcoded dark palette, lacks a settings dialog, and cannot toggle secondary visual features such as the "net web" (ratline connections across nodes on the selected net) or pin text annotations.

This specification defines the Preferences modal dialog, persistent runtime settings, theme palettes, and visualization toggles.

---

## 2. Requirements

### 2.1 Preferences Modal Window

- **REQ-PREF-010 [Preferences Dialog Invocation]:**
  The application MUST provide a Preferences modal or floating window (`egui::Window`) toggled via:
  1. Top panel menu item: `Preferences` (or `File -> Preferences...`).
  2. Keyboard shortcut: `Ctrl+,` (on Linux/Windows) and `Cmd+,` (on macOS).

- **REQ-PREF-011 [Window Ergonomics & Modality]:**
  The Preferences dialog SHALL be center-anchored upon opening, resizable or auto-sized with a minimum size of $420 \times 320\,\text{px}$, and dismissible via an `[x]` close button or hitting `Escape`.

- **REQ-PREF-012 [Immediate Visual Feedback]:**
  Changes made in the Preferences window (such as selecting a theme or toggling net web) MUST take effect immediately on the next canvas render frame without requiring application restart or manual "Apply" button clicks.

- **REQ-PREF-013 [Preferences Data Model]:**
  The application SHALL define a serializable preferences structure:
  ```rust
  #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
  pub struct AppPreferences {
      pub theme_preset: ThemePreset,
      pub show_net_web: bool,
      pub show_pin_names: bool,
      pub show_part_names: bool,
      pub high_density_lod: bool,
  }
  ```

---

### 2.2 Theme System & Presets

- **REQ-PREF-020 [Theme Preset Enumeration]:**
  The system MUST support four distinct presets represented by the `ThemePreset` enum:
  - `ThemePreset::Dark` (Cadcraft Dark)
  - `ThemePreset::OpenBoardView` (OpenBoardView Classic)
  - `ThemePreset::HighContrast` (High Contrast Probe Mode)
  - `ThemePreset::Light` (Light Mode / Printing)

- **REQ-PREF-021 [Cadcraft Dark Palette]:**
  `ThemePreset::Dark` MUST provide the default slate/navy aesthetic:
  - Canvas: `#1a1e24`
  - Board Outline: `#b0b8c4`
  - Board Fill: `rgba(30, 38, 48, 128)`
  - Active Side Net Highlight: `#32dd66` (vivid emerald)
  - Opposite Side Net Highlight: `#1b7da8` (deep blue)
  - Selected Pin Accent: `#ffc832` (gold)

- **REQ-PREF-022 [OpenBoardView Classic Palette]:**
  `ThemePreset::OpenBoardView` MUST match the visual identity of OpenBoardView:
  - Canvas: `#111111` or `#161616` (charcoal black)
  - Board Outline: `#ffffff` (crisp white)
  - Board Fill: `rgba(0, 0, 0, 0)` or subtle dark tint
  - Active Side Net Highlight: `#ffd000` (bright yellow/gold)
  - Opposite Side Net Highlight: `#0088ff` (royal blue)
  - Pin Default: `#cc4433` (orange-red)
  - Net Web Lines: `rgba(255, 208, 0, 180)`

- **REQ-PREF-023 [High Contrast Palette]:**
  `ThemePreset::HighContrast` MUST optimize visibility for difficult lighting and color-blind probing:
  - Canvas: `#000000` (true OLED black)
  - Board Outline: `#ffff00` (electric yellow)
  - Active Side Net Highlight: `#00ff66` (neon spring green)
  - Opposite Side Net Highlight: `#ff00cc` (magenta)
  - Unconnected Pin: `#444444` (dark gray outline)

- **REQ-PREF-024 [Light Mode Palette]:**
  `ThemePreset::Light` MUST provide clear document/export readability:
  - Canvas: `#f5f6f8` (off-white)
  - Panel & Chrome: `#e4e7eb` (soft light gray)
  - Board Outline: `#2b303a` (dark charcoal)
  - Board Fill: `rgba(220, 225, 230, 180)`
  - Active Side Net Highlight: `#0e8a16` (deep forest green)
  - Opposite Side Net Highlight: `#1d76db` (clear blue)
  - Text & Labels: `#1a1e24` (near black)

---

### 2.3 Visualization Toggles

- **REQ-PREF-030 [Net Web Visibility Toggle]:**
  The user MUST be able to toggle `show_net_web: bool`.
  - When `true`: Selecting a pin or net SHALL draw connecting ratlines between all visible nodes of the net.
  - When `false`: Selecting a pin or net SHALL highlight pins and vias but MUST NOT draw any connecting ratlines, eliminating visual clutter on multi-thousand node nets.

- **REQ-PREF-031 [Pin Text Annotations Toggle]:**
  The user MUST be able to toggle `show_pin_names: bool` and `show_part_names: bool`.
  - When enabled, text labels for pin numbers and part designators SHALL render above an adaptive zoom threshold ($> 0.15\,\text{pixels/mil}$).
  - When disabled, text rendering is skipped, freeing canvas space and reducing draw calls.

- **REQ-PREF-032 [LOD Decimation Toggle]:**
  The user MUST be able to toggle `high_density_lod: bool`.
  - When `true`: Sub-pixel via decimation and point clustering are enabled.
  - When `false`: Full circle tessellation is preserved regardless of zoom level (for users requiring pixel-accurate circular outlines on high-end GPUs).

---

## 3. Scenarios & Acceptance Criteria

### Scenario 1: Opening Preferences via Keyboard Shortcut
- **Given** the `BoardViewCraft` main window is focused,
- **When** the user presses `Ctrl+,` (or `Cmd+,` on macOS),
- **Then** the Preferences window appears centered on screen,
- **And** the window contains theme selection controls and visual toggle checkboxes.

### Scenario 2: Switching to OpenBoardView Classic Theme
- **Given** the Preferences window is open,
- **When** the user selects the `OpenBoardView Classic` radio button or dropdown option,
- **Then** the canvas background immediately changes to charcoal black (`#111111`),
- **And** selected net highlights immediately render in bright yellow (`#ffd000`),
- **And** all UI panels adopt the matching palette.

### Scenario 3: Hiding Net Web on High-Density Rail
- **Given** a dense net (e.g. `+3V3` or `GND`) is currently selected with hundreds of ratlines crisscrossing the screen,
- **When** the user unchecks the `Show Net Web` toggle in Preferences,
- **Then** all connecting line segments disappear from the canvas on the next frame,
- **And** the pad/via highlight circles remain visible.

---

## 4. Verification Contracts

| Verification Target | Test Method | Pass Criteria |
| :--- | :--- | :--- |
| Theme presets definition | Unit test (`test_theme_tokens_presets`) | All 4 presets (`DARK`, `OPEN_BOARD_VIEW`, `HIGH_CONTRAST`, `LIGHT`) instantiate with valid non-zero RGB components. |
| Palette contrast ratio | Unit test (`test_theme_contrast_ratios`) | Text color against panel background in all 4 themes satisfies minimum WCAG AA contrast ratio ($\ge 4.5:1$). |
| Net web preference adherence | Integration test (`test_net_web_disabled_rendering`) | When `preferences.show_net_web == false`, the generated mesh collection contains zero lines for net webs. |
| Preferences serialization roundtrip | Unit test (`test_preferences_serde`) | Serializing an `AppPreferences` instance to JSON and deserializing reconstructs identical fields. |
