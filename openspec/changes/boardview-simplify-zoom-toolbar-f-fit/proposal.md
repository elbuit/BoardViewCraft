# Proposal: Simplify Zoom Controls in Toolbar & Menus and Refine 'F' Key Board Fit

- **Change Name:** `boardview-simplify-zoom-toolbar-f-fit`
- **Status:** Proposed
- **Author:** sdd-propose
- **Date:** 2026-10-23
- **Target Workspace:** `BoardViewCraft` (`crates/bvc-app`)
- **Reference Codebases:** `OpenBoardView`, `cadcraft`

---

## 1. Intent & Problem Statement

In the previous iteration of the top toolbar and zoom controls, several zoom-related buttons and entries were introduced alongside existing actions:
1. **Redundant & Cluttered Toolbar Controls:**
   - The top toolbar currently displays both a standalone "Fit Window (F)" / "Ajustar ventana (F)" button, a "Zoom" dropdown menu containing another "Fit" / "Ajustar" item, and a separate "1:1" button.
   - The standalone "1:1" button duplicates the "100%" preset present in the "Zoom" dropdown.
   - The "Fit Window (F)" button in the toolbar occupies horizontal bar space that makes the toolbar crowded, especially at lower resolutions or when long file paths / names are open.
   - Users requested cleaning up and simplifying the zoom controls:
     - Remove "ajustar 1:1" / "1:1" button and "ajustar ventana" button/menu entries where requested.
     - Keep the Zoom menu clean with discrete percentage presets (25%, 50%, 100%, 150%, 200%).
2. **Dedicated, Responsive 'F' Shortcut for Fit-to-View:**
   - Pressing the `F` key (case-insensitive: both `Key::F` with or without shift, or when handling key press) should immediately frame the PCB bounds within the visible canvas viewport, ensuring seamless navigation without needing to click redundant toolbar buttons.
   - Hotkey handling should be clean, centralized in the global shortcut block, and directly invoke board bounding box fitting to the canvas rect.

---

## 2. Proposed Changes & Technical Architecture

### 2.1 Toolbar Simplification (`crates/bvc-app/src/main.rs`)

1. **Remove Standalone "Fit Window" & "1:1" Toolbar Buttons:**
   - Remove the standalone `Fit Window (F)` button from the horizontal toolbar.
   - Remove the standalone `1:1` (`toolbar.zoom_reset`) button from the horizontal toolbar.
2. **Streamline the Zoom Dropdown Menu:**
   - In `ui.menu_button(i18n::tr(lang, "toolbar.zoom"), |ui| { ... })`:
     - Retain clean, predictable magnification presets:
       - `25%` (`toolbar.zoom_preset_25`)
       - `50%` (`toolbar.zoom_preset_50`)
       - `100%` (`toolbar.zoom_preset_100`)
       - `150%` (`toolbar.zoom_preset_150`)
       - `200%` (`toolbar.zoom_preset_200`)
     - Remove redundant "Fit" / "Ajustar ventana" / "1:1" entries from the Zoom dropdown, or keep "Fit (F)" solely if desired as a submenu action; per user request: "Remove 'ajustar 1:1' and 'ajustar ventana' button/menu entries from the toolbar/menus where requested. Keep only clean zoom percentages and intuitive 'F' key shortcut."
3. **Actions Menu / Menus Cleanup:**
   - Verify no leftover "Ajustar ventana" / "1:1" entries linger in the Actions dropdown or toolbar menus.

### 2.2 Global 'F' / 'f' Hotkey Fit Action

1. **Hotkey Handling in `BoardViewApp::ui`:**
   - In the shortcut evaluation block (`if !self.preferences_open`), capture `f_pressed`:
     ```rust
     let (r_pressed, shift_down, space_pressed, m_pressed, esc_pressed, f_pressed) = ui.input(|i| {
         (
             i.key_pressed(egui::Key::R),
             i.modifiers.shift,
             i.key_pressed(egui::Key::Space),
             i.key_pressed(egui::Key::M),
             i.key_pressed(egui::Key::Escape),
             i.key_pressed(egui::Key::F),
         )
     });
     ```
   - When `f_pressed` is true (and user is not currently focusing a search text input in the side panel):
     - Check if `self.board` has a bounding box.
     - Compute the central canvas inner rect (available viewport).
     - Call `self.canvas_state.viewport.fit_to_rect(bbox, canvas_rect)`.
     - Request repaint (`ui.ctx().request_repaint()`).
     - Update status message or log to indicate viewport fitted: e.g., `"Viewport fitted to PCB bounds (F)"`.
2. **Preventing Conflict with Search Input:**
   - Ensure `f_pressed` does not trigger if a text edit field in the side panel (e.g. part search or net search) currently has keyboard focus (`ui.ctx().wants_keyboard_input()`).

### 2.3 Localization & Unused Strings Cleanup (`crates/bvc-app/src/i18n.rs`)

- Remove or deprecate unused translation keys:
  - `toolbar.zoom_reset` ("1:1")
  - `toolbar.fit_window` ("Fit Window (F)" / "Ajustar ventana (F)" / "Ajustar finestra (F)") if no longer referenced.
  - `toolbar.zoom_preset_fit` if removed from dropdown.
- Keep clean entries for `toolbar.zoom`, and presets `25%`, `50%`, `100%`, `150%`, `200%`.

---

## 3. Impact Assessment & Compatibility

- **User Ergonomics:** Cleaner, uncluttered toolbar with more horizontal breathing room for CAD file name and details.
- **Workflow Continuity:** `F` key is the industry-standard hotkey for "Fit to screen / Frame selection" across CAD and 3D modeling tools (KiCad, Blender, CADCraft, OpenBoardView).
- **Breaking Changes:** None. Eliminates redundancy and improves shortcut responsiveness.

---

## 4. Verification Plan

1. **Compilation & Linting:**
   - Run `cargo check --workspace` and `cargo test --workspace` to ensure zero warnings or broken references.
2. **Manual / Interactive Verification:**
   - Launch `cargo run -p bvc-app` (or test via automated canvas unit tests).
   - Verify toolbar UI has clean layout: Open CAD, Actions, Preferences, Side toggle, Rotate, Zoom dropdown (with 25%, 50%, 100%, 150%, 200%), and file label. No redundant "1:1" or "Ajustar ventana" buttons.
   - Load a sample board (`MS-V375 REV 7.0.cad`).
   - Pan and zoom far out or into a specific pad.
   - Press `F`: verify board immediately centers and scales to ~90% of canvas area.
   - Focus the search input field in the side panel and type text containing the letter "f": verify board does not re-fit while typing.
