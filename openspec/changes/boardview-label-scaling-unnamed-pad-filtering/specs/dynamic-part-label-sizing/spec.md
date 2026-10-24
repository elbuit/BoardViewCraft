# Capability: dynamic-part-label-sizing

## Requirements

### Requirement: Proportional Component Label Font Size
The viewer MUST compute component label font size based on the physical bounding box dimensions of the component rather than a static 12.0 pt font size.

#### Scenario: Large IC label rendering
- **GIVEN** a component with large bounding box (e.g. width >= 100 mils and height >= 100 mils)
- **WHEN** rendered on the canvas
- **THEN** its designator text SHALL be rendered with an enlarged font size up to 28.0 pt proportional to its projected screen dimensions.

#### Scenario: Small passive component label rendering
- **GIVEN** a small passive component (e.g. 0402 capacitor or resistor)
- **WHEN** rendered on the canvas
- **THEN** its designator text SHALL be clamped to a minimum legible size (>= 10.0 pt) or decimated by LOD when projected dimensions are below visibility thresholds.

#### Scenario: Selection override
- **GIVEN** any component that is currently selected
- **WHEN** rendered on the canvas
- **THEN** its designator label MUST remain visible regardless of unselected LOD culling.
