//! High-fidelity PDF schematic renderer and search engine with instant indexing and multi-hit navigation.
//!
//! Features:
//! - Immediate multi-thread text indexing on document open (`rayon` parallel extraction of `PageText`).
//! - Exact multi-occurrence search across pages using `PageText::find_opts` and `glyph_rects`.
//! - Instant cross-probing: finding all component designators across all pages in milliseconds.
//! - Multi-worker asynchronous page rasterization via `RenderPool` (multithreaded, cancel-in-flight).
//! - Dedicated high-priority rendering for requested/navigated pages.
//! - Dynamic scale resolution (`scale = zoom * PT * ppp`) keeping vector graphics and text razor-sharp.
//! - Multi-hit selection and navigation list support.

use egui::{ColorImage, Pos2, Rect, TextureHandle, TextureOptions, Vec2};
use pdfcraft_render::{
    text::TextFilter, DocInfo, PageRenderer, PageText, RenderConfig, RenderPool, RenderRequest, RequestKind,
};
use rayon::prelude::*;
use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// PDF points to standard CSS/screen points ratio (96 DPI / 72 PDF points)
pub const PT: f32 = 96.0 / 72.0;

/// A cross-probing or search match occurrence on a page.
#[derive(Clone, Debug, PartialEq)]
pub struct SchematicOccurrence {
    pub page_number: usize,
    pub bbox: Rect,
    pub text: String,
    pub glyph_range: Range<usize>,
}

/// Helper methods matching previous index normalization
pub struct SchematicIndex;

impl SchematicIndex {
    pub fn normalize_key(raw: &str) -> String {
        raw.trim()
            .to_ascii_uppercase()
            .replace(['.', ',', ';', ':', '-', '_'], "")
    }
}

/// The request tag for a raster at `scale`: equal tags mean the same scale (to 1/65536).
pub fn scale_tag(scale: f32) -> u64 {
    (f64::from(scale) * 65536.0).round() as u64
}

/// Convert rendered raw Pixels into egui::ColorImage
fn texture_image(size: [usize; 2], pixels: pdfcraft_render::Pixels) -> ColorImage {
    ColorImage::from_rgba_unmultiplied(size, &pixels)
}

/// Cached texture entry with its rendered scale tag
struct CachedPageTexture {
    handle: TextureHandle,
    dim: Vec2,
    tag: u64,
}

/// Embedded schematic viewer matching pdfcraft's canvas engine architecture.
pub struct PdfSchematicViewer {
    pub path: Option<PathBuf>,
    pub filename: Option<String>,
    pub current_page: usize,
    pub total_pages: usize,
    pub doc_info: Option<DocInfo>,

    // Search and cross-probing state
    pub active_search: String,
    pub search_results: Vec<SchematicOccurrence>,
    pub selected_result_idx: usize,
    pub zoom: f32,
    pub pan_offset: Vec2,
    pub is_open: bool,
    pub pending_center_on_match: bool,

    // Page text layers pre-extracted for instant whole-document search (0-based page index -> PageText)
    pub page_texts: HashMap<usize, Arc<PageText>>,

    // High-fidelity page textures cached by page number (1-based)
    page_textures: HashMap<usize, CachedPageTexture>,

    // Multithreaded RenderPool matching pdfcraft's engine
    render_pool: Option<RenderPool>,

    // Last requested raster scale tag per page: (page, tag)
    last_requested_tags: HashMap<usize, u64>,

    // Settle timer tracking zoom adjustments to avoid worker churning
    last_zoom_change_time: f64,
    last_observed_zoom: f32,
}

impl Default for PdfSchematicViewer {
    fn default() -> Self {
        Self {
            path: None,
            filename: None,
            current_page: 1,
            total_pages: 1,
            doc_info: None,
            active_search: String::new(),
            search_results: Vec::new(),
            selected_result_idx: 0,
            zoom: 1.0,
            pan_offset: Vec2::ZERO,
            is_open: false,
            pending_center_on_match: false,
            page_texts: HashMap::new(),
            page_textures: HashMap::new(),
            render_pool: None,
            last_requested_tags: HashMap::new(),
            last_zoom_change_time: 0.0,
            last_observed_zoom: 1.0,
        }
    }
}

impl PdfSchematicViewer {
    /// Opens a PDF document using pdfcraft's native inspection and pre-indexes text in parallel.
    pub fn open_file(&mut self, path: &Path) -> Result<(), String> {
        let bytes = std::fs::read(path).map_err(|e| format!("Failed to read PDF file: {e}"))?;
        let bytes_arc = Arc::new(bytes);

        // 1. Inspect document structure, page count, and page dimensions
        let info = pdfcraft_render::inspect(Arc::clone(&bytes_arc), None)
            .map_err(|e| format!("Failed to inspect PDF: {e}"))?;
        let total_pages = info.pages.len().max(1);

        self.path = Some(path.to_path_buf());
        self.filename = Some(
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("schematic.pdf")
                .to_string(),
        );
        self.total_pages = total_pages;
        self.current_page = 1;
        self.doc_info = Some(info);
        self.page_textures.clear();
        self.last_requested_tags.clear();
        self.search_results.clear();
        self.zoom = 1.0;
        self.pan_offset = Vec2::ZERO;
        self.is_open = true;

        // 2. Extract ALL text layers in parallel using rayon across available CPU cores (takes ~10-20ms for whole PDF)
        let extracted_texts: Vec<(usize, Arc<PageText>)> = (0..total_pages)
            .into_par_iter()
            .filter_map(|p_idx| {
                let mut renderer = PageRenderer::new(Arc::clone(&bytes_arc), RenderConfig::default());
                renderer.filtered_text(p_idx, TextFilter::default()).ok().map(|t| (p_idx, Arc::new(t)))
            })
            .collect();

        self.page_texts = extracted_texts.into_iter().collect();

        // 3. Initialize RenderPool with worker threads matching system parallelism (e.g. 4)
        let threads = std::thread::available_parallelism()
            .map(|n| n.get().clamp(2, 8))
            .unwrap_or(4);
        let pool = RenderPool::new(bytes_arc, threads, RenderConfig::default());
        self.render_pool = Some(pool);

        // 4. Immediately trigger rasterization for page 1
        self.update_render_queue(1.0);

        Ok(())
    }

    /// Returns the page size in PDF points for the given page (1-based).
    pub fn page_size_pts(&self, page: usize) -> Vec2 {
        if let Some(info) = &self.doc_info {
            if let Some(p) = info.pages.get(page.saturating_sub(1)) {
                return Vec2::new(p.width, p.height);
            }
        }
        Vec2::new(1224.0, 792.0)
    }

    /// Computes the exact device scale needed so 1 device pixel = 1 texture pixel (pdfcraft formula).
    pub fn calculate_render_scale(&self, ppp: f32) -> f32 {
        let raw_scale = self.zoom * PT * ppp;
        let page_size = self.page_size_pts(self.current_page);
        let max_dim = page_size.x.max(page_size.y).max(1.0);
        let max_scale = 8192.0 / max_dim;
        raw_scale.clamp(0.25, max_scale)
    }

    /// Finds the token at the clicked PDF point coordinate on the given page.
    pub fn find_token_at(&self, page: usize, point_pt: Pos2) -> Option<SchematicOccurrence> {
        let page_0 = page.saturating_sub(1);
        let text = self.page_texts.get(&page_0)?;

        let nearest_idx = text.nearest(point_pt.x, point_pt.y)?;
        let (first, last) = text.word_at(nearest_idx)?;

        let glyph_indices: Vec<usize> = (first..=last).collect();
        let rects = text.glyph_rects(&glyph_indices);
        let first_rect = rects.first()?;

        let mut min_x = first_rect[0];
        let mut min_y = first_rect[1];
        let mut max_x = first_rect[2];
        let mut max_y = first_rect[3];
        for r in &rects[1..] {
            min_x = min_x.min(r[0]);
            min_y = min_y.min(r[1]);
            max_x = max_x.max(r[2]);
            max_y = max_y.max(r[3]);
        }

        let margin = 4.0;
        let bbox = Rect::from_min_max(Pos2::new(min_x, min_y), Pos2::new(max_x, max_y));
        if bbox.expand(margin).contains(point_pt) {
            Some(SchematicOccurrence {
                page_number: page,
                bbox,
                text: text.text_of(first..last + 1),
                glyph_range: first..last + 1,
            })
        } else {
            None
        }
    }

    /// Queues raster tasks to the `RenderPool` with the visible page strictly first.
    pub fn update_render_queue(&mut self, ppp: f32) {
        let Some(pool) = &self.render_pool else { return };

        let curr_0 = self.current_page.saturating_sub(1);
        let scale = self.calculate_render_scale(ppp);
        let tag = scale_tag(scale);

        let mut requests = Vec::new();

        // 1. Current page raster (highest priority, rendered immediately)
        requests.push(RenderRequest {
            page: curr_0,
            scale,
            kind: RequestKind::Pixels,
            tag,
            tile: None,
            preview: false,
        });

        // 2. Adjacent pages raster prefetch (page + 1 and page - 1 at normal reading scale ~1.5)
        let prefetch_scale = (1.5 * ppp).clamp(0.5, 3.0);
        let prefetch_tag = scale_tag(prefetch_scale);
        if self.current_page < self.total_pages {
            requests.push(RenderRequest {
                page: curr_0 + 1,
                scale: prefetch_scale,
                kind: RequestKind::Pixels,
                tag: prefetch_tag,
                tile: None,
                preview: false,
            });
        }
        if curr_0 > 0 {
            requests.push(RenderRequest {
                page: curr_0 - 1,
                scale: prefetch_scale,
                kind: RequestKind::Pixels,
                tag: prefetch_tag,
                tile: None,
                preview: false,
            });
        }

        pool.set_queue(requests);
    }

    /// Pumps completed jobs from the `RenderPool` and checks whether re-rendering is needed for sharp zoom.
    pub fn ensure_current_page_texture(&mut self, ctx: &egui::Context) -> Option<(TextureHandle, Vec2)> {
        let ppp = ctx.pixels_per_point();
        let current_time = ctx.input(|i| i.time);

        // Detect zoom changes to handle debounce settling
        if (self.zoom - self.last_observed_zoom).abs() > 0.001 {
            self.last_observed_zoom = self.zoom;
            self.last_zoom_change_time = current_time;
        }

        let is_settled = (current_time - self.last_zoom_change_time) > 0.12;

        if let Some(pool) = &self.render_pool {
            // Drain finished jobs from pool
            for _ in 0..8 {
                let Some(res) = pool.try_recv() else { break };
                if res.request.kind == RequestKind::Pixels
                    && res.error.is_none()
                    && res.width > 0
                    && res.height > 0
                    && !res.rgba.is_empty()
                {
                    let page_1 = res.request.page + 1;
                    let tag = res.request.tag;
                    let size = [res.width as usize, res.height as usize];
                    let color_image = texture_image(size, res.rgba);
                    let handle = ctx.load_texture(
                        format!("pdf_p{page_1}_t{tag}"),
                        color_image,
                        TextureOptions::LINEAR,
                    );
                    let dim = Vec2::new(res.width as f32, res.height as f32);
                    self.page_textures.insert(
                        page_1,
                        CachedPageTexture { handle, dim, tag },
                    );
                    ctx.request_repaint();
                }
            }
        }

        // Check if current page needs an updated, sharp raster at the current zoom level
        let wanted_scale = self.calculate_render_scale(ppp);
        let wanted_tag = scale_tag(wanted_scale);

        let need_fresh_render = match self.page_textures.get(&self.current_page) {
            Some(t) => t.tag != wanted_tag,
            None => true,
        };

        if need_fresh_render && (is_settled || !self.page_textures.contains_key(&self.current_page)) {
            if self.last_requested_tags.get(&self.current_page).copied() != Some(wanted_tag) {
                self.last_requested_tags.insert(self.current_page, wanted_tag);
                self.update_render_queue(ppp);
            }
        }

        self.page_textures
            .get(&self.current_page)
            .map(|t| (t.handle.clone(), t.dim))
    }

    /// Evaluates `active_search` across ALL pages instantly using pre-extracted `PageText`.
    pub fn perform_instant_search(&mut self, query: &str) {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            self.search_results.clear();
            return;
        }

        self.active_search = trimmed.to_string();
        let mut results = Vec::new();

        // Search across all pages in document order
        for p_idx in 0..self.total_pages {
            let Some(text) = self.page_texts.get(&p_idx) else { continue };
            let page_1 = p_idx + 1;

            // 1. Try whole-word search first (matches exact component reference e.g. "U12")
            let mut matches = text.find_opts(trimmed, false, true);
            // 2. If no whole-word match found, fall back to substring search
            if matches.is_empty() {
                matches = text.find_opts(trimmed, false, false);
            }

            for range in matches {
                let glyph_indices: Vec<usize> = range.clone().collect();
                let rects = text.glyph_rects(&glyph_indices);
                if let Some(first_rect) = rects.first() {
                    let mut min_x = first_rect[0];
                    let mut min_y = first_rect[1];
                    let mut max_x = first_rect[2];
                    let mut max_y = first_rect[3];
                    for r in &rects[1..] {
                        min_x = min_x.min(r[0]);
                        min_y = min_y.min(r[1]);
                        max_x = max_x.max(r[2]);
                        max_y = max_y.max(r[3]);
                    }
                    let bbox = Rect::from_min_max(Pos2::new(min_x, min_y), Pos2::new(max_x, max_y));
                    results.push(SchematicOccurrence {
                        page_number: page_1,
                        bbox,
                        text: text.text_of(range.clone()),
                        glyph_range: range,
                    });
                }
            }
        }

        results.sort_by(|a, b| {
            a.page_number
                .cmp(&b.page_number)
                .then(a.bbox.min.y.total_cmp(&b.bbox.min.y))
                .then(a.bbox.min.x.total_cmp(&b.bbox.min.x))
        });

        self.search_results = results;
    }

    /// Selects a specific match by index, navigating to its page and centering view.
    pub fn select_match(&mut self, idx: usize) {
        if idx < self.search_results.len() {
            self.selected_result_idx = idx;
            let target_page = self.search_results[idx].page_number;
            self.current_page = target_page;
            self.pending_center_on_match = true;
            // Force immediate raster request for the target page
            self.last_requested_tags.remove(&target_page);
            self.update_render_queue(1.0);
        }
    }

    /// Initiates cross-probing navigation to a component designator or net.
    /// Finds all matches instantly across the document and centers the first one.
    pub fn cross_probe(&mut self, query: &str) -> bool {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return false;
        }

        self.perform_instant_search(trimmed);
        self.selected_result_idx = 0;

        // Adaptive zoom elevation: if unzoomed (<= 1.0), elevate to 3.0 (300%).
        if self.zoom <= 1.0 {
            self.zoom = 3.0;
        }

        self.last_observed_zoom = self.zoom;

        if !self.search_results.is_empty() {
            let target_page = self.search_results[0].page_number;
            self.current_page = target_page;
            self.pending_center_on_match = true;
            // Clear tag to force immediate sharp rasterization of target page
            self.last_requested_tags.remove(&target_page);
            self.update_render_queue(1.0);
            true
        } else {
            false
        }
    }

    pub fn next_match(&mut self) {
        if !self.search_results.is_empty() {
            let next_idx = (self.selected_result_idx + 1) % self.search_results.len();
            self.select_match(next_idx);
        }
    }

    pub fn prev_match(&mut self) {
        if !self.search_results.is_empty() {
            let prev_idx = if self.selected_result_idx == 0 {
                self.search_results.len() - 1
            } else {
                self.selected_result_idx - 1
            };
            self.select_match(prev_idx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross_probe_adaptive_zoom_elevation() {
        let mut viewer = PdfSchematicViewer::default();

        viewer.zoom = 1.0;
        viewer.cross_probe("C35");
        assert_eq!(viewer.zoom, 3.0);

        viewer.zoom = 0.75;
        viewer.cross_probe("C35");
        assert_eq!(viewer.zoom, 3.0);

        viewer.zoom = 3.5;
        viewer.cross_probe("C35");
        assert_eq!(viewer.zoom, 3.5);
    }

    #[test]
    fn test_ms_v375_instant_search_multiple_occurrences() {
        let pdf_path = Path::new("tmp/MS-V375 REV 7.0.pdf");
        if !pdf_path.exists() {
            return;
        }

        let mut viewer = PdfSchematicViewer::default();
        let res = viewer.open_file(pdf_path);
        assert!(res.is_ok());

        // Perform instant cross probe for U12
        let matched = viewer.cross_probe("U12");
        assert!(matched, "U12 should be found in MS-V375 REV 7.0.pdf");
        assert!(
            viewer.search_results.len() > 1,
            "U12 should appear multiple times across schematic, got {}",
            viewer.search_results.len()
        );

        // Verify matches have valid pages and bboxes
        for hit in &viewer.search_results {
            assert!(hit.page_number >= 1 && hit.page_number <= viewer.total_pages);
            assert!(hit.bbox.is_positive());
        }
    }
}
