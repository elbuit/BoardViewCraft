# Spec: PCB Custom Theme Profile and Color Persistence

## Capability: `boardview-preferences-and-visualization`

### Requirement: Custom PCB Theme Selection and Initialization
The application SHALL allow the user to select `Custom` as their PCB theme and SHALL initialize custom color tokens from whichever PCB theme is currently active.

#### Scenario: User switches PCB theme to Custom
- **Given** the Preferences window is open with a preset theme active (e.g., Cadcraft, Dark, Light, Warm, Sweet Cold)
- **When** the user selects "Custom..." from the PCB Theme dropdown
- **Then** `custom_pcb_tokens` SHALL be populated with the color tokens of the active theme
- **And** the UI SHALL display color pickers for the PCB elements.

---

### Requirement: Interactive PCB Element Color Customization
The application SHALL provide interactive color pickers for key PCB elements when `PcbTheme::Custom` is selected:
- Canvas background
- Board outline and fill
- Part body and part designator text
- Pin default, unconnected, active-side highlight, opposite-side highlight
- Via active-side, opposite-side, and highlight
- Net web lines

#### Scenario: User modifies a PCB color in Custom mode
- **Given** `PcbTheme::Custom` is active in Preferences
- **When** the user modifies any color picker (e.g., Canvas Background)
- **Then** the canvas cache SHALL invalidate immediately
- **And** the canvas viewport SHALL redraw using the updated color values.

---

### Requirement: Atomic Persistence of Custom PCB Colors
The application SHALL persist `custom_pcb_tokens` in `preferences.json` atomically upon change or modal close.

#### Scenario: User saves preferences and restarts application
- **Given** the user customized PCB colors and closed Preferences
- **When** the application restarts or reloads preferences
- **Then** `preferences.json` SHALL deserialize `custom_pcb_tokens` with exact RGBA values
- **And** the PCB canvas SHALL display with the stored custom colors.
