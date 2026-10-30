# Technical Design: PDF Cross-Probing Zoom and Centering

## Problem & Context
When cross-probing components from a boardview into a PDF schematic, unzoomed views (100% / 1.0) make small SMD component designators and connection net labels difficult to read without manual zooming. On the other hand, if an engineer has already zoomed in to inspect nearby traces, resetting their zoom destroys their working magnification context.

## Architectural Changes

### 1. `PdfSchematicViewer::cross_probe`
In [`crates/ui-egui/src/pdf.rs`](file:///media/no_raid/devel/boarview/BoardViewCraft/crates/ui-egui/src/pdf.rs):
```rust
pub fn cross_probe(&mut self, query: &str) -> bool {
    let hits = if let Ok(index) = self.index.lock() {
        index.search(query)
    } else {
        Vec::new()
    };

    if hits.is_empty() {
        self.search_results.clear();
        self.selected_result_idx = 0;
        return false;
    }

    self.active_search = query.to_string();
    self.search_results = hits;
    self.selected_result_idx = 0;

    // Adaptive zoom: enforce 200% (2.0x) minimum if unzoomed (<= 1.0), otherwise preserve user's zoom
    if self.zoom <= 1.0 {
        self.zoom = 2.0;
    }

    if let Some(first) = self.search_results.first() {
        self.current_page = first.page_number;
        self.pending_center_on_match = true;
    }
    true
}
```

### 2. Viewport Centering Synchronization
In [`crates/ui-egui/src/lib.rs`](file:///media/no_raid/devel/boarview/BoardViewCraft/crates/ui-egui/src/lib.rs):
The centering calculation already uses `effective_scale = auto_scale * self.pdf_viewer.zoom;`.
When `pending_center_on_match` is processed:
1. `effective_scale` uses the updated `self.pdf_viewer.zoom` (e.g. `2.0 * auto_scale`).
2. `comp_raster_x` and `comp_raster_y` scale with `effective_scale`.
3. `pan_offset` sets the match's raster position directly to `render_size * 0.5 - comp_raster`, placing it dead-center in the viewport.

### 3. Verification & Unit Tests
Add unit tests in `crates/ui-egui/src/pdf.rs` verifying:
- A search hit with initial `zoom = 1.0` results in `zoom = 2.0`.
- A search hit with initial `zoom = 0.8` results in `zoom = 2.0`.
- A search hit with initial `zoom = 2.5` maintains `zoom = 2.5`.
- A search hit with initial `zoom = 1.5` maintains `zoom = 1.5`.
- `pending_center_on_match` is set to `true` in all matching cases.
