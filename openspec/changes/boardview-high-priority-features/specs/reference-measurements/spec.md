# Reference Measurements Specification

## Purpose

Define the requirements for displaying, importing, and editing reference measurement values (diode mode in mV, resistance in Ω, and voltages in V) on pins and nets for component-level hardware repair diagnostics.

## Requirements

### Requirement: Display of Reference Values

The system MUST display reference measurement values for selected pins and nets in the side details panel and hover tooltips when data is available.

#### Scenario: Inspecting pin with diode mode reference value
- GIVEN a board with measurement data loaded
- WHEN the user clicks on a pin with an associated diode mode measurement
- THEN the Pin Details panel SHALL display the reference diode drop (e.g. `450 mV`) and impedance/voltage if present.

#### Scenario: Hover tooltip with measurements
- GIVEN a pin that has reference measurement values
- WHEN the user hovers the cursor over the pin on the canvas
- THEN the hover tooltip SHALL include the net name and the diode/voltage reference value.

### Requirement: User Measurement Entry and Editing

The system MUST allow the technician to record real measured values directly on any pin or net.

#### Scenario: Adding a live diode mode reading
- GIVEN a selected pin in the side panel
- WHEN the technician enters a measured diode value (e.g. `0 mV` indicating a short to ground) in the measurement input box
- THEN the system SHALL save the value in the board's active session measurements
- AND if the value deviates significantly from the reference value, it SHOULD be highlighted in warning/error color (e.g., red for dead short).

### Requirement: Extraction of Measurements from Boardview Files

Parsers for formats that embed diode mode or voltage ratings (such as specific `.fz`, `.cad`, or `.bdv` files) SHOULD extract and populate the measurement fields during file loading.

#### Scenario: Loading a boardview file with embedded diode values
- GIVEN a boardview file containing embedded net/pin voltage or diode ratings
- WHEN the parser processes the file
- THEN the resulting `Board` data structure SHALL contain populated measurement fields for those pins.
