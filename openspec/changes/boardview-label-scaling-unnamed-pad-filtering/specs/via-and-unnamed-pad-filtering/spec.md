# Capability: via-and-unnamed-pad-filtering

## Requirements

### Requirement: Toggle for Vias and Unnamed Vias
The viewer MUST provide user preferences to toggle visibility of all vias and specifically unnamed vias/test pads.

#### Scenario: Hide unnamed vias
- **GIVEN** a board containing routing/stitching vias or test points with empty or UNCONNECTED net names
- **WHEN** the preference `show_unnamed_vias` is disabled (`false`)
- **THEN** unnamed vias that are not highlighted by active selection or hover SHALL NOT be rendered on canvas.

#### Scenario: Active net or hover overrides unnamed filter
- **GIVEN** an unnamed via or test point
- **WHEN** it is hovered by the mouse or its net is selected
- **THEN** it SHALL be rendered with full highlight/hover indicators regardless of the `show_unnamed_vias` preference.

#### Scenario: Global via visibility toggle
- **GIVEN** `show_vias` preference is toggled to `false`
- **WHEN** rendering the board
- **THEN** unselected vias SHALL be hidden from the viewport.
