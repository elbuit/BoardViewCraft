# Drag and Drop Loading Specification

## Purpose

Define the behavior for dragging and dropping files onto the BoardViewCraft application window to load boardview CAD files and overlay images seamlessly without opening system file dialogs.

## Requirements

### Requirement: Drag and Drop Boardview File Loading

The system MUST detect when a boardview file is dropped onto the application window and parse and display it.

#### Scenario: Dropping a supported boardview file
- GIVEN the application is running and visible
- WHEN the user drags and drops a supported boardview file (e.g. `.cad`, `.brd`, `.fz`, `.bdv`) onto the window
- THEN the system SHALL load and parse the boardview
- AND the canvas SHALL display the board centered and fitted
- AND the status bar SHALL update with the filename, part count, and net count.

#### Scenario: Dropping an unsupported file format
- GIVEN the application is running
- WHEN the user drops an unsupported or corrupt file (e.g. an arbitrary `.txt` or corrupted binary)
- THEN the system MUST NOT crash
- AND the system SHALL display a clear error message in the status bar indicating the unrecognized format.

### Requirement: Drag and Drop Overlay Photo Loading

The system SHOULD detect when an image file is dropped and load it into the active side's overlay manager.

#### Scenario: Dropping an image file
- GIVEN a boardview is loaded
- WHEN the user drops a valid image file (`.png`, `.jpg`, `.webp`, `.bmp`) onto the canvas
- THEN the system SHALL load the image as the optical overlay for the currently active side (Top or Bottom)
- AND the side panel SHALL switch to the Overlay tab.
