# Capability: pin-visibility-toggle

## Requirements

### Requirement: Toolbar Pin Visibility Toggle
The application MUST provide a top toolbar button to toggle component pin visibility on and off.

#### Scenario: Toggle pins off from toolbar
- **GIVEN** a board loaded with component pins visible
- **WHEN** the user clicks the "Pins" button in the top toolbar
- **THEN** unselected component pins SHALL be hidden from the viewport, and the button label SHALL reflect the OFF state.

#### Scenario: Active selection or hover remains visible
- **GIVEN** pin visibility is toggled off (`show_pins == false`)
- **WHEN** a pin is selected, hovered, or belongs to an actively selected net
- **THEN** it SHALL remain rendered and highlighted on the canvas.

#### Scenario: Preferences persistence
- **GIVEN** the user changes the pin visibility state via the toolbar or preferences dialog
- **WHEN** the application is restarted
- **THEN** the `show_pins` preference setting SHALL be persisted and restored.
