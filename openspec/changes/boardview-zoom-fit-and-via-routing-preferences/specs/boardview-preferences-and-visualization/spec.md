# Spec: BoardView Preferences and Via Routing Visualization

## Capability: `boardview-preferences-and-visualization`

### Requirement: Via Display Mode Configuration
The application SHALL allow the user to select among three via routing display modes in Preferences:
1. `FastestPath` (Shortest Euclidean MST path)
2. `Radial` (Radial/star topology from net centroid or pin anchors)
3. `Hidden` (Vias excluded from flight lines)

#### Scenario: User chooses Via Display Mode in preferences
- **Given** the Preferences window is open
- **When** the user selects "Radial", "Camino más rápido" (Fastest Path), or "No mostrar" (Hidden) from the Via Display Mode setting
- **Then** the preference SHALL be persisted atomically to `preferences.json`
- **And** the canvas flight lines SHALL invalidate their mesh cache and re-render according to the selected mode.

---

### Requirement: Net Flight Line Rendering According to Mode
The canvas rendering pipeline SHALL filter or route net connection web lines based on the active `ViaDisplayMode`.

#### Scenario: Fastest Path mode active
- **Given** a net with pins and vias is selected
- **And** `ViaDisplayMode` is set to `FastestPath`
- **When** the net flight line mesh is computed
- **Then** the mesh SHALL include Minimum Spanning Tree segments interconnecting pins and vias.

#### Scenario: Hidden mode active
- **Given** a net with pins and vias is selected
- **And** `ViaDisplayMode` is set to `Hidden`
- **When** the net flight line mesh is computed
- **Then** via positions SHALL NOT be included in the flight line node list
- **And** flight lines SHALL connect only pin nodes.

#### Scenario: Radial mode active
- **Given** a net with pins and vias is selected
- **And** `ViaDisplayMode` is set to `Radial`
- **When** the net flight line mesh is computed
- **Then** via nodes SHALL connect radially to the net centroid or nearest pin.
