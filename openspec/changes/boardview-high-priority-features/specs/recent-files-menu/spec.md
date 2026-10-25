# Recent Files Menu Specification

## Purpose

Define the behavior for recording, persisting, and quickly reopening recently loaded boardview files.

## Requirements

### Requirement: Persistent Recent Files History

The system MUST record successfully opened boardview files and persist this list across application restarts in user preferences.

#### Scenario: Opening a new boardview adds to recent files
- GIVEN the application with an empty or existing recent files list
- WHEN the user successfully opens a boardview file
- THEN the file path SHALL be placed at the top (index 0) of the recent files list
- AND if the file path was already present, it SHALL be moved to the top without duplicates
- AND the list SHALL be capped at a maximum of 10 items.

### Requirement: Quick-Open from Recent Files Menu

The system MUST provide a menu in the top toolbar to access and reopen recent files.

#### Scenario: Reopening a recent file
- GIVEN the application has at least one valid file in the recent files list
- WHEN the user clicks "Open Recent" in the toolbar and selects an entry
- THEN the system SHALL load and display the selected boardview immediately.

#### Scenario: Recent file missing on disk
- GIVEN a file in the recent list that has been deleted or moved from disk
- WHEN the user clicks that file in the "Open Recent" menu
- THEN the system MUST display a warning in the status bar indicating the file could not be found
- AND the system MUST NOT crash.
