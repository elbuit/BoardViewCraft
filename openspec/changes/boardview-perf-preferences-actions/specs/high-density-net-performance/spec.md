# Specification: High-Density Net Highlighting & Spatial Performance

- **Capability:** `high-density-net-performance`
- **Change:** `boardview-perf-preferences-actions`
- **Status:** Draft / Ready for Review
- **Target Components:** `bvc-core::Board`, `bvc-core::SpatialGrid`, `bvc-app::canvas`, `bvc-app::render`

---

## 1. Overview & Problem

In dense real-world boardview CAD files (e.g., `MS-V375 REV 7.0.cad`), primary power nets and ground planes contain thousands of elements (e.g. `GND` with 2,481 pins and 2,274 vias = 4,755 total nodes).

Prior to this specification, the rendering pipeline evaluated every pin and via on every viewport change through parallel scans over all board entities:
1. Dynamic string comparisons (`pin.net_name == selected_net` / `via.net_name == selected_net`) executed on thousands of elements per frame.
2. Circle tessellation generated tens of thousands of vertices for microscopic vias whose screen radius was sub-pixel ($r_{\text{screen}} < 0.75\,\text{px}$).
3. Entire pin and via vectors were evaluated without spatial frustum culling, consuming Rayon CPU worker cycles on off-screen geometry.

This specification establishes requirements for spatial grid indexing, index-based direct net queries, and sub-pixel Level-of-Detail (LOD) decimation to ensure smooth 60+ FPS interaction on complex hardware designs.

---

## 2. Requirements

### 2.1 Spatial Grid Partitioning (`bvc-core`)

- **REQ-PERF-010 [Spatial Acceleration Structure]:**
  The `Board` struct in `bvc-core` MUST contain a 2D spatial acceleration structure (`SpatialGrid`) that indexes pins and vias by their physical 2D coordinates in mils.

- **REQ-PERF-011 [Grid Cell Dimensions]:**
  The spatial grid SHALL use fixed-size square buckets in board coordinate space (defaulting to 500.0 mils per cell side, or adaptively sized based on the board bounding box to prevent empty bucket proliferation).

- **REQ-PERF-012 [Entity Partitioning & Indexing]:**
  The `SpatialGrid` MUST store zero-copy entity index references (`usize` indices referencing `Board.pins` and `Board.vias`) rather than cloning entity structs.

- **REQ-PERF-013 [Frustum Range Query]:**
  The `SpatialGrid` MUST provide a query interface `query_range(&self, query_rect: Rect) -> (Vec<usize>, Vec<usize>)` (or an iterator yielding visible pin indices and via indices) with worst-case complexity proportional to $O(\text{visible cells} + \text{intersecting entities})$ rather than $O(N_{\text{total}})$.

- **REQ-PERF-014 [Index Lifecycle & Determinism]:**
  The spatial grid MUST be constructed or rebuilt in `Board::rebuild_indices` upon loading a board or mutating pin/via collections, taking less than 15ms for boards with up to 100,000 entities.

- **REQ-PERF-015 [Side Filtering Integration]:**
  The spatial query MUST either partition entities by `Side` (`Top`, `Bottom`, `Both`) or support zero-allocation filtering using `Side::visible_on(view_side)` on retrieved candidate indices.

---

### 2.2 Direct Index-Based Highlight Mesh Construction

- **REQ-PERF-020 [Pre-Indexed Net Entities]:**
  The `Board` struct SHALL maintain precomputed indices for every named net. `Net` MUST contain `pin_indices: Vec<usize>` and `via_indices: Vec<usize>`.

- **REQ-PERF-021 [Zero String Comparison in Render Loop]:**
  The rendering pass in `bvc-app::canvas` MUST NOT execute string equality checks (`pin.net_name == *selected_net`) inside the per-element tessellation loop. Net highlighting MUST resolve target pins and vias directly through index lookups.

- **REQ-PERF-022 [O(1) Net Membership Structure]:**
  When a net is selected, `bvc-app` or `bvc-core` MUST provide an $O(1)$ fast membership mechanism (such as a pre-allocated boolean bitset, `HashSet<usize>`, or precomputed net ID index) so that rendering loops can classify whether an arbitrary entity belongs to the active net in $O(1)$ time.

- **REQ-PERF-023 [Dual-Pass Tessellation Architecture]:**
  The tessellation pipeline SHALL partition geometry into:
  1. Base pass: Non-highlighted background pins and vias intersecting the visible world rect.
  2. Highlight pass: Pins and vias belonging to `selected_net` intersecting the visible world rect, rendered with distinct z-order and highlight tokens.

---

### 2.3 Sub-Pixel LOD Culling and Clustering

- **REQ-PERF-030 [Screen Space Radius Metric]:**
  For every circular via and pin, the renderer MUST calculate the screen-space radius in logical pixels:
  $$r_{\text{screen}} = r_{\text{world}} \times \text{scale}$$

- **REQ-PERF-031 [Sub-Pixel Culling for Dense Nets]:**
  When a net contains more than 500 nodes (e.g. `GND`, `VCC`), and $r_{\text{screen}} < 0.75\,\text{px}$, the renderer MUST cull via circle geometry or replace circular tessellation with a single 1-quad or point primitive.

- **REQ-PERF-032 [Visual Pop Suppression]:**
  When transitioning between full circle tessellation ($r_{\text{screen}} \ge 1.5\,\text{px}$), simplified quad primitives ($0.75\,\text{px} \le r_{\text{screen}} < 1.5\,\text{px}$), and culled states ($r_{\text{screen}} < 0.75\,\text{px}$), the visual appearance MUST maintain legibility without sudden full-board flickering.

- **REQ-PERF-033 [Performance Budget]:**
  On cache-miss frames (such as continuous panning or zooming) with `GND` selected on `MS-V375 REV 7.0.cad` (4,755 nodes), total mesh generation time across Rayon threads MUST NOT exceed 8.0ms on modern x86_64 hardware (8+ cores).

---

## 3. Scenarios & Acceptance Criteria

### Scenario 1: Spatial Frustum Query Eliminates Off-Screen Elements
- **Given** a board with 10,000 pins and 10,000 vias loaded in `BoardViewCraft`,
- **When** the user zooms into a single IC occupying 5% of the total board area,
- **Then** `SpatialGrid::query_range` returns only candidate indices residing in cells intersecting the visible viewport rect,
- **And** the number of pins submitted for tessellation is less than 10% of total board pins.

### Scenario 2: High-Density Net Selection Without String Comparison
- **Given** a loaded board where `GND` has 4,755 associated nodes,
- **When** the user selects a `GND` pin from the side panel or canvas,
- **Then** the renderer retrieves `net.pin_indices` and `net.via_indices` directly from `board.nets["GND"]`,
- **And** zero `String::eq` or `&str == &str` operations are executed during the vertex generation loop.

### Scenario 3: LOD Decimation on Wide Zoom
- **Given** a board with `GND` selected,
- **When** the user zooms out to view the entire board such that via screen radius $r_{\text{screen}} < 0.75\,\text{px}$,
- **Then** dense ground vias are culled or replaced by single-pixel quad points,
- **And** the total vertex count submitted to egui decreases by at least 60% compared to full 16-segment circle tessellation,
- **And** frame render time remains under 8ms.

---

## 4. Verification Contracts

| Verification Target | Test Method | Pass Criteria |
| :--- | :--- | :--- |
| `SpatialGrid` boundary indexing | Unit test (`test_spatial_grid_query`) | Points at corners and edges of grid cells are accurately returned without false negatives. |
| Spatial query speed | Benchmark / Integration test | Querying a 1000x1000 mil frustum across 50,000 pins takes $< 100\,\mu\text{s}$. |
| Net index lookups | Unit test (`test_net_membership_fast_path`) | Membership test returns identical truth values to string comparison while executing in $O(1)$. |
| Tessellation frame budget | Integration test (`bench_dense_net_tessellation`) | With 4,755 GND nodes selected, mesh generation across worker threads completes in $< 8.0\,\text{ms}$. |
| Sub-pixel LOD decimation | Unit test (`test_lod_decimation_threshold`) | Elements with $r_{\text{screen}} < 0.75\,\text{px}$ on nets $> 500$ nodes produce simplified or culled vertex output. |
