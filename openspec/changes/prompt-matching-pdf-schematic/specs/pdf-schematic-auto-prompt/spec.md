# Capability Spec: Co-located PDF Schematic Auto-Prompt

## Purpose
Enables BoardViewCraft to discover co-located PDF schematic files matching the base stem of an opened boardview and prompt the user to load the schematic with cross-probing.

## Requirements

### Requirement: Sibling Schematic PDF Detection
When a boardview file is opened from an explicit filesystem path (via Open Board dialog, Recent Files list, Drag & Drop, or CLI parameter), the system SHALL check if a sibling PDF document exists in the same directory sharing the exact file stem.

#### Scenario: Matching lowercase .pdf exists
- **GIVEN** a board file `/path/to/MS-V373.cad`
- **AND** a sibling file `/path/to/MS-V373.pdf` exists
- **WHEN** the user opens `/path/to/MS-V373.cad`
- **THEN** the system SHALL set the pending schematic prompt with path `/path/to/MS-V373.pdf`

#### Scenario: Matching uppercase .PDF exists
- **GIVEN** a board file `/path/to/MS-V373.cad`
- **AND** a sibling file `/path/to/MS-V373.PDF` exists
- **WHEN** the user opens `/path/to/MS-V373.cad`
- **THEN** the system SHALL set the pending schematic prompt with path `/path/to/MS-V373.PDF`

#### Scenario: No sibling PDF exists
- **GIVEN** a board file `/path/to/MS-V373.cad`
- **AND** no file named `MS-V373.pdf` or `MS-V373.PDF` exists in `/path/to/`
- **WHEN** the user opens `/path/to/MS-V373.cad`
- **THEN** no prompt SHALL be presented and the board opens normally

#### Scenario: Schematic PDF is already opened
- **GIVEN** a board file `/path/to/MS-V373.cad` with sibling `MS-V373.pdf`
- **AND** `MS-V373.pdf` is already open in the PDF schematic viewer
- **WHEN** the board is opened
- **THEN** no prompt SHALL be displayed

### Requirement: Non-Blocking Confirmation Dialog
When a pending schematic candidate is detected, the application SHALL display a clear modal confirmation prompt asking whether the user wants to open the schematic.

#### Scenario: User accepts prompt
- **GIVEN** a pending schematic prompt for `/path/to/MS-V373.pdf`
- **WHEN** the user clicks "Open" or presses `Enter`
- **THEN** the application SHALL open `/path/to/MS-V373.pdf` in the schematic viewer
- **AND** the pending schematic prompt SHALL be cleared

#### Scenario: User dismisses prompt
- **GIVEN** a pending schematic prompt for `/path/to/MS-V373.pdf`
- **WHEN** the user clicks "Cancel" or presses `Escape`
- **THEN** the application SHALL clear the prompt without loading the PDF
- **AND** the board view remains open and interactive

### Requirement: Internationalization Parity
All labels, headings, and button captions for the schematic auto-prompt SHALL be localized across all supported languages (`en`, `es`, `ca`, `fr`, `de`, `it`).
