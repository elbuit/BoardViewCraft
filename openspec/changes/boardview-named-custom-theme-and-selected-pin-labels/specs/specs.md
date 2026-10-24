# Spec: Named Custom PCB Theme and Selected Pin Labels

## Requirements

### Requirement: Named Custom PCB Theme
The system MUST allow users to assign and persist a custom name to their custom PCB theme profile in Preferences.
- The `Preferences` struct MUST store an optional custom theme name `custom_pcb_theme_name: Option<String>`.
- When `PcbTheme::Custom` is selected, the UI in Preferences MUST display a text input field to edit this name.
- If no custom name is set, it defaults to "Custom".
- The name MUST persist across application restarts.

### Requirement: Selected Pin Net Label and Outline/Negative Presentation
The canvas rendering MUST render selected pins with special emphasis:
- When a pin is selected (directly or as part of the selected net):
  - The pin MUST display the associated net name (e.g. `GND`, `+3V3`, `VCC`) or pin/net designation.
  - The pin MUST be rendered in negative contrast over the pin or as an outline circle with the net name clearly centered inside.
  - This visual presentation MUST ONLY apply to selected pins; unselected pins MUST NOT be rendered with net names or outline emphasis.
