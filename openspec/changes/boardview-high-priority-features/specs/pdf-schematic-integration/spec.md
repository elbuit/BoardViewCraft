# PDF Schematic Integration Specification

## Purpose

Define the requirements for integrating an embedded PDF schematic viewer with bidirectional cross-probing between boardview components/nets and the electrical schematic, referencing the PDF architecture and rendering engine from `pdfcraft`.

## Requirements

### Requirement: Embedded PDF Document Viewing

The system MUST allow associating and opening an electrical schematic PDF alongside the active boardview in a split-screen or secondary panel.

#### Scenario: Opening an associated schematic PDF
- GIVEN an active boardview session
- WHEN the user loads an electrical schematic PDF
- THEN the system SHALL render the PDF pages with smooth pan and zoom controls alongside the boardview layout.

### Requirement: Coordinate-Indexed Text Search

The system MUST index all text spans and coordinates across the PDF pages upon opening to enable instant lookup of component designators and signal nets.

#### Scenario: Background index creation
- GIVEN a multi-page schematic PDF is loaded
- WHEN the document is initialized
- THEN the system SHALL index all tokens (e.g. `U7400`, `R2310`, `PP3V3_S5`) with their corresponding page number and bounding box coordinates in under 2 seconds.

### Requirement: Boardview to PDF Cross-Probing

The system MUST jump to and visually highlight the relevant schematic symbol when a component or net is selected on the boardview canvas.

#### Scenario: Clicking a component jumps to schematic
- GIVEN a boardview and schematic PDF are both open and linked
- WHEN the user clicks on component `U7400` in the boardview
- THEN the PDF viewer SHALL navigate to the primary schematic page containing `U7400`
- AND the PDF viewer SHALL center on the component and draw a highlight bounding box around it.

#### Scenario: Stepping through multiple schematic occurrences
- GIVEN a net (such as `PP3V3_S5`) that appears on multiple schematic pages
- WHEN the user selects the net in BoardViewCraft
- THEN the PDF viewer SHALL provide "Next / Previous" navigation buttons to cycle through every occurrence of that net across the schematic pages.

### Requirement: PDF to Boardview Cross-Probing

The system SHOULD allow clicking or selecting text within the PDF schematic to select and focus the corresponding element on the boardview canvas.

#### Scenario: Clicking a net or component in PDF
- GIVEN the PDF viewer showing a schematic
- WHEN the user clicks on a recognized component designator or net label in the PDF
- THEN BoardViewCraft SHALL automatically select that component or net
- AND the boardview canvas SHALL center and highlight the matching pins and pads.
