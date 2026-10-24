# Change Proposal: Named Custom Themes and Selected Pin Labels

## Intent & Scope
Allow saving the custom PCB theme profile with a user-defined name in Preferences, and improve the visual feedback for selected pins on the canvas:
1. **Named Custom Themes**: Provide a name input field in preferences for the custom theme (e.g. "Mi Tema Laboratorio") so the user can label and persist their custom theme configuration.
2. **Selected Pin Net Labeling**: Display the net name (or pin/net designation, e.g., "GND", "+5V") on selected pins rather than only pin index/number.
3. **Negative / Outline Pin Presentation**: For selected pins, render the label in negative contrast over the pin or render the selected pin as an outline circle with the net name clearly centered inside, ensuring high readability without cluttering unselected pins.

## Proposed Changes
### `BoardViewCraft/crates/bvc-app/src/preferences.rs`
- Add `custom_pcb_theme_name: Option<String>` to `Preferences` (defaulting to e.g. "Custom").
- Add a text edit in the Preferences window allowing the user to rename their custom PCB theme.

### `BoardViewCraft/crates/bvc-app/src/canvas.rs`
- In `show_canvas` (or dedicated selected-pin overlay pass):
  - Check whether a pin is actively selected or part of the active net selection.
  - For selected pins, render the pin with distinctive outline styling (e.g., hollow circle stroke with high contrast color) or negative contrast fill.
  - Draw the pin's net name (e.g., `pin.net_name` or `net.name`) inside or centered on the pin with inverted/high-contrast text.
  - Unselected pins remain rendered as standard solid pins without large net annotations to maintain canvas performance and clean layout.

## Affected Areas
- `crates/bvc-app/src/preferences.rs`
- `crates/bvc-app/src/canvas.rs`
- `crates/bvc-app/src/i18n.rs` (labels for theme name input)

## Risks & Mitigations
- **Canvas Text Clutter / Performance**: Only selected pins receive the net name rendering, avoiding font generation overhead for thousands of board pins simultaneously.
- **Pin Radius Fit**: Net names like "VCC_CORE_SOMETHING" may exceed small pin radii; font size will be scaled or clipped gracefully to the pin circle.

## Verification
- Run `cargo test --manifest-path BoardViewCraft/crates/bvc-app/Cargo.toml --ignore-rust-version`.
- Manual verification of custom theme name persistence and selected pin outline/negative net rendering.
