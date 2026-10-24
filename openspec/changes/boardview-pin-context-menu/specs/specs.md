# Spec: Pin Context Menu on Secondary Click and Ctrl+Left Click

## Requirements

### Requirement: Context Menu Triggers
The canvas MUST detect contextual click gestures on pins:
- A secondary click (right-click) without significant drag movement MUST open the pin context menu when clicking on a pin.
- A primary click (left-click) while holding the `Ctrl` key without significant drag movement MUST open the pin context menu when clicking on a pin.
- Triggering the menu on a pin MUST also make that pin the actively selected pin.

### Requirement: Pin Context Menu Contents
When opened, the context menu MUST present:
- Header displaying the component name and pin number (e.g. `U1.1`).
- Connected net name (e.g. `Net: GND`).
- Pin layer (`Side: TOP` or `Side: BOTTOM`).
- Action: "Copy Net Name" which copies the connected net name to the system clipboard.
- Action: "Copy Pin Info" which copies `Component.Pin (Net)` to the system clipboard.
