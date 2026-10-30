# Delta Spec: PDF Cross-Probing Zoom and Centering

## Purpose
Specifies the centering and adaptive zoom magnification behavior when a component in BoardViewCraft is clicked and synchronized with its counterpart in the PDF schematic viewer.

## Requirements

### Requirement: Adaptive Zoom Escalation on Cross-Probing
When a component is cross-probed to the PDF viewer, the system SHALL ensure the schematic is clearly legible by applying a minimum zoom level of 200% (2.0x) if the viewer is currently unzoomed, while strictly preserving any existing magnification higher than 100%.

#### Scenario: Cross-probing with default or sub-1.0x zoom
- **GIVEN** an active PDF schematic viewer with `zoom <= 1.0` (e.g. 1.0 default)
- **WHEN** the user selects a component in the boardview with matching tokens in the schematic
- **THEN** the system SHALL set `zoom = 2.0` (200% magnification)
- **AND** the system SHALL set `pending_center_on_match = true`

#### Scenario: Cross-probing with existing zoom greater than 100%
- **GIVEN** an active PDF schematic viewer with `zoom = 2.5` (250% magnification)
- **WHEN** the user selects a component in the boardview
- **THEN** the system SHALL maintain `zoom = 2.5`
- **AND** the system SHALL set `pending_center_on_match = true`

#### Scenario: Cross-probing with moderate zoom greater than 100%
- **GIVEN** an active PDF schematic viewer with `zoom = 1.25` (125% magnification)
- **WHEN** the user selects a component in the boardview
- **THEN** the system SHALL maintain `zoom = 1.25`
- **AND** the system SHALL set `pending_center_on_match = true`

### Requirement: Centered Match Alignment
When `pending_center_on_match` is processed, the system SHALL calculate the viewport `pan_offset` such that the center of the matching schematic token coincides exactly with the center of the rendered PDF canvas viewport at the current effective scale.

#### Scenario: Centering at 200% zoom
- **GIVEN** a match token at coordinates `(X, Y)` on page `P`
- **AND** `zoom` is 2.0
- **WHEN** the PDF canvas is rendered
- **THEN** the canvas viewport center coincides with the screen projection of `(X, Y)`
- **AND** `pending_center_on_match` is cleared to `false`
