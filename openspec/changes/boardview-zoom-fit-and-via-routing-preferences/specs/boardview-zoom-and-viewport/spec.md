# Spec: BoardView Zoom and Viewport

## Capability: `boardview-zoom-and-viewport`

### Requirement: Primary Fit-to-Window Option in Zoom Menu
The application SHALL provide "Fit to Window (F)" as the first selectable item in the Zoom dropdown menu.

#### Scenario: User opens zoom menu
- **Given** a boardview is loaded or ready in the application
- **When** the user clicks on the "Zoom" dropdown menu in the toolbar
- **Then** the first menu item SHALL be "Fit to Window (F)" (or localized equivalent)
- **And** clicking this item SHALL adjust the viewport scale and center to fit the entire board bounding box inside the visible canvas.

---

### Requirement: Keyboard Shortcut 'F' Fit-to-Window
The application SHALL support the `F` key to immediately frame and fit the board to the window.

#### Scenario: User presses 'F' without active text input
- **Given** a board with a valid bounding box is loaded
- **And** no text edit input field currently holds keyboard focus
- **When** the user presses the `F` key
- **Then** the viewport SHALL calculate the scale and center to fit the board with a padding margin
- **And** the status bar SHALL notify "Viewport fitted to PCB bounds (F)".

---

### Requirement: Automatic Viewport Fit on Board Load
The application SHALL automatically fit the loaded board to the viewport window when a boardview file is opened.

#### Scenario: Board file loaded successfully
- **Given** the user selects and opens a board file (.cad, .tvw, .brd, etc.)
- **When** parsing finishes successfully and the board contains a non-empty bounding box
- **Then** the application SHALL queue and execute an automatic fit to the visible canvas rect
- **And** the board outline and components SHALL be fully visible without manual zooming.
