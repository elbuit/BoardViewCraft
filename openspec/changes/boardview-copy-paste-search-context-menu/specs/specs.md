# Spec: Copy & Paste for Search and Context Menu Copy Actions

## Requirements

### Requirement: Search Input Pasting
The side panel part and net search input boxes MUST support inserting clipboard text:
- When a search input field (`filter_part` or `filter_net`) is focused, pressing `Ctrl+V` (or `Cmd+V` on macOS) MUST paste the clipboard contents into the input field.
- If right-clicked, the search input field MAY offer a context menu with "Paste" action.

### Requirement: Search Results Context Menu
Each item in the Part and Net search result lists MUST provide a right-click context menu:
- **Part Result**:
  - Right-clicking a part result row MUST open a context menu with:
    - "Copy Part Name" (copies e.g. `U12`).
    - "Copy Part Details (TSV)" (copies full part TSV export identical to detail view).
- **Net Result**:
  - Right-clicking a net result row MUST open a context menu with:
    - "Copy Net Name" (copies e.g. `+12V_GPU`).
    - "Copy Net Details (TSV)" (copies full net TSV export identical to detail view).

### Requirement: Component Canvas Context Menu
Secondary click (right-click) or Ctrl+Left click on a component body / bounding box on the PCB canvas (when no pin is directly hit) MUST open a component context menu:
- Displays component identifier (e.g. `U1`).
- Displays component side (`Top` / `Bottom`) and total pins count.
- Action: "Copy Part Name" (copies e.g. `U1`).
- Action: "Copy Part Details (TSV)".
- Selecting the context menu action MUST copy the corresponding string to the system clipboard and dismiss the menu.
- Pressing `Escape` or clicking outside MUST close the menu.

### Requirement: Localization
All new labels and actions in context menus MUST be localized across all supported languages (EN, ES, CA, DE, FR, IT).
