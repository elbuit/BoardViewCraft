# Keyboard Pan, Zoom, and Shortcuts Specification

## Purpose

Define the requirements for full keyboard navigation (panning with arrows, zooming with `+`/`-`) and customizable keyboard shortcut bindings.

## Requirements

### Requirement: Arrow Key Panning

The system MUST allow shifting the viewport center using the arrow keys when the canvas has focus and no text entry widget is active.

#### Scenario: Panning with arrow keys
- GIVEN a boardview is loaded and displayed on the canvas
- WHEN the user presses the `Up`, `Down`, `Left`, or `Right` arrow key
- THEN the viewport center SHALL move in world coordinates in the corresponding direction
- AND the canvas repaint SHALL maintain smooth 60 fps interactivity.

#### Scenario: Arrow keys during text input
- GIVEN the search text input or another text field in the side panel is focused
- WHEN the user presses an arrow key to navigate cursor position inside the text field
- THEN the canvas viewport center MUST NOT move.

### Requirement: Keyboard Zoom

The system MUST allow zooming the viewport using `+` and `-` keys.

#### Scenario: Zooming in and out via keyboard
- GIVEN a boardview is loaded
- WHEN the user presses `+` (or numpad `+`)
- THEN the viewport scale SHALL increase centered on the current view center.
- WHEN the user presses `-` (or numpad `-`)
- THEN the viewport scale SHALL decrease centered on the current view center.

### Requirement: Customizable Action Keybindings

The system SHOULD provide a keybinding configuration in the Preferences window allowing users to remap primary actions (Rotate CW, Flip Side, Mirror X/Y, Fit, Toggle Vias).

#### Scenario: Remapping a shortcut key
- GIVEN the Preferences window opened to the Keybindings tab
- WHEN the user changes the binding for "Flip Side" to a custom key
- THEN the system SHALL save the new binding in preferences
- AND pressing that key in the main canvas SHALL toggle Top/Bottom side.
