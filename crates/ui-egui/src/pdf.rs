//! High-fidelity PDF schematic renderer and cross-probing indexer using native pdfcraft-render.
//!
//! Features:
//! - High-fidelity in-process vector rasterization of schematic pages into pixel-crisp textures.
//! - Bounding-box text token extraction for instant, exact cross-probing of component designators and nets.
//! - Multi-page navigation, pan & zoom, and highlight overlays.
//! - Pure Rust in-process implementation (zero dependency on poppler-tools / CLI binaries).

use egui::{ColorImage, Pos2, Rect, TextureHandle, TextureOptions, Vec2};
use pdfcraft_render::{PageRenderer, RenderConfig, RenderRequest, RequestKind, text::TextFilter};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

/// Rasterization resolution levels: normal (150 DPI) and zoomed-in high-fidelity (300 DPI)
const DPI_NORMAL: u32 = 150;
const DPI_HIGH: u32 = 300;

struct PageRasterResult {
    page: usize,
    dpi: u32,
    dimensions: Vec2,
    color_image: ColorImage,
}

/// A recognized token occurrence with exact page coordinates for cross-probing.
#[derive(Clone, Debug, PartialEq)]
pub struct SchematicOccurrence {
    pub page_number: usize,
    pub bbox: Rect,
    pub text: String,
}

/// Inverted index for instant symbol and net lookups across schematic pages.
#[derive(Clone, Debug, Default)]
pub struct SchematicIndex {
    tokens: HashMap<String, Vec<SchematicOccurrence>>,
    pub page_sizes: HashMap<usize, Vec2>,
    pub page_count: usize,
}

impl SchematicIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn normalize_key(raw: &str) -> String {
        raw.trim()
            .to_ascii_uppercase()
            .replace(['.', ',', ';', ':', '-', '_'], "")
    }

    pub fn add_token(&mut self, text: &str, page_number: usize, bbox: Rect) {
        let key = Self::normalize_key(text);
        if key.is_empty() {
            return;
        }

        let occ = SchematicOccurrence {
            page_number,
            bbox,
            text: text.to_string(),
        };

        self.tokens.entry(key).or_default().push(occ);
    }

    pub fn search(&self, query: &str) -> Vec<SchematicOccurrence> {
        let key = Self::normalize_key(query);
        if key.is_empty() {
            return Vec::new();
        }

        if let Some(exact) = self.tokens.get(&key) {
            return exact.clone();
        }

        let mut results = Vec::new();
        for (token_key, occs) in &self.tokens {
            if token_key.contains(&key) {
                results.extend(occs.clone());
            }
        }
        results
    }

    /// Finds the token occurrence whose bounding box contains `point_pt` on the given `page`.
    pub fn find_token_at(&self, page: usize, point_pt: Pos2) -> Option<SchematicOccurrence> {
        let margin = 4.0; // Margin in PDF points for comfortable click hit-testing
        for list in self.tokens.values() {
            for occ in list {
                if occ.page_number == page {
                    let expanded_bbox = occ.bbox.expand(margin);
                    if expanded_bbox.contains(point_pt) {
                        return Some(occ.clone());
                    }
                }
            }
        }
        None
    }
}

/// Cached texture entry with its rendered DPI level
struct CachedPageTexture {
    handle: TextureHandle,
    dim: Vec2,
    dpi: u32,
}

/// Embedded schematic viewer and raster renderer with asynchronous loading and multi-LOD.
pub struct PdfSchematicViewer {
    pub path: Option<PathBuf>,
    pub filename: Option<String>,
    pub current_page: usize,
    pub total_pages: usize,
    pub index: Arc<Mutex<SchematicIndex>>,
    pub is_indexing: Arc<Mutex<bool>>,
    pub active_search: String,
    pub search_results: Vec<SchematicOccurrence>,
    pub selected_result_idx: usize,
    pub zoom: f32,
    pub pan_offset: Vec2,
    pub is_open: bool,
    pub pending_center_on_match: bool,

    // High-fidelity page textures cached by page number
    page_textures: HashMap<usize, CachedPageTexture>,

    // PDF raw bytes shared with workers
    doc_bytes: Option<Arc<Vec<u8>>>,

    // Background raster worker communication
    raster_tx: Sender<(Arc<Vec<u8>>, usize, u32)>,
    raster_rx: Receiver<PageRasterResult>,
    requested_pages: HashSet<(usize, u32)>,
}

impl Default for PdfSchematicViewer {
    fn default() -> Self {
        let (res_tx, res_rx) = channel();
        let (req_tx, req_rx) = channel::<(Arc<Vec<u8>>, usize, u32)>();

        // Spawn dedicated background rasterization worker using native pdfcraft-render
        thread::Builder::new()
            .name("bvc-pdf-rasterizer".into())
            .spawn(move || {
                while let Ok((bytes, page, dpi)) = req_rx.recv() {
                    let mut renderer = PageRenderer::new(bytes, RenderConfig::default());
                    let scale = dpi as f32 / 72.0;
                    let req = RenderRequest {
                        page: page.saturating_sub(1),
                        scale,
                        kind: RequestKind::Pixels,
                        ..Default::default()
                    };
                    let rendered = renderer.render(req);
                    if rendered.error.is_none() && rendered.width > 0 && rendered.height > 0 && !rendered.rgba.is_empty() {
                        let (w, h) = (rendered.width as usize, rendered.height as usize);
                        let color_image = ColorImage::from_rgba_unmultiplied([w, h], &rendered.rgba);
                        let _ = res_tx.send(PageRasterResult {
                            page,
                            dpi,
                            dimensions: Vec2::new(w as f32, h as f32),
                            color_image,
                        });
                    }
                }
            })
            .expect("Failed to spawn PDF raster thread");

        Self {
            path: None,
            filename: None,
            current_page: 1,
            total_pages: 1,
            index: Arc::new(Mutex::new(SchematicIndex::new())),
            is_indexing: Arc::new(Mutex::new(false)),
            active_search: String::new(),
            search_results: Vec::new(),
            selected_result_idx: 0,
            zoom: 1.0,
            pan_offset: Vec2::ZERO,
            is_open: false,
            pending_center_on_match: false,
            page_textures: HashMap::new(),
            doc_bytes: None,
            raster_tx: req_tx,
            raster_rx: res_rx,
            requested_pages: HashSet::new(),
        }
    }
}

impl PdfSchematicViewer {
    pub fn open_file(&mut self, path: &Path) -> Result<(), String> {
        let bytes = std::fs::read(path).map_err(|e| format!("Failed to read PDF file: {e}"))?;
        let bytes_arc = Arc::new(bytes);

        // 1. Inspect PDF metadata using pdfcraft-render
        let info = pdfcraft_render::inspect(Arc::clone(&bytes_arc), None)
            .map_err(|e| format!("Failed to inspect PDF: {e}"))?;
        let total_pages = info.pages.len().max(1);

        let path_buf = path.to_path_buf();
        self.path = Some(path_buf);
        self.filename = Some(
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("schematic.pdf")
                .to_string(),
        );
        self.total_pages = total_pages;
        self.current_page = 1;
        self.page_textures.clear();
        self.requested_pages.clear();
        self.search_results.clear();
        self.zoom = 1.0;
        self.pan_offset = Vec2::ZERO;
        self.is_open = true;
        self.doc_bytes = Some(Arc::clone(&bytes_arc));

        // Seed page dimensions immediately from inspect results
        {
            if let Ok(mut idx) = self.index.lock() {
                idx.page_count = total_pages;
                idx.page_sizes.clear();
                for (i, p) in info.pages.iter().enumerate() {
                    idx.page_sizes.insert(i + 1, Vec2::new(p.width, p.height));
                }
            }
        }

        // 2. Launch background indexing with PageText
        let index_arc = Arc::clone(&self.index);
        let indexing_flag = Arc::clone(&self.is_indexing);
        if let Ok(mut flag) = indexing_flag.lock() {
            *flag = true;
        }

        let bg_bytes = Arc::clone(&bytes_arc);
        thread::Builder::new()
            .name("bvc-pdf-indexer".into())
            .spawn(move || {
                Self::build_index_in_background(&bg_bytes, total_pages, &index_arc);
                if let Ok(mut flag) = indexing_flag.lock() {
                    *flag = false;
                }
            })
            .expect("Failed to spawn PDF indexer thread");

        // 3. Request immediate background rasterization of page 1 and prefetch page 2
        self.request_page_raster(1, DPI_NORMAL);
        if self.total_pages > 1 {
            self.request_page_raster(2, DPI_NORMAL);
        }

        Ok(())
    }

    /// Extract text tokens and bounding boxes using pdfcraft-render in the background.
    fn build_index_in_background(
        bytes: &Arc<Vec<u8>>,
        pages: usize,
        index_arc: &Arc<Mutex<SchematicIndex>>,
    ) {
        let mut renderer = PageRenderer::new(Arc::clone(bytes), RenderConfig::default());

        for page_idx in 0..pages {
            let page_num = page_idx + 1;
            if let Ok(page_text) = renderer.filtered_text(page_idx, TextFilter::default()) {
                if page_text.glyphs.is_empty() {
                    continue;
                }

                // Group consecutive glyphs into words using word_at boundaries
                let mut visited_glyphs = HashSet::new();
                let num_glyphs = page_text.glyphs.len();

                let mut page_tokens = Vec::new();

                for g_idx in 0..num_glyphs {
                    if visited_glyphs.contains(&g_idx) {
                        continue;
                    }
                    if let Some((first, last)) = page_text.word_at(g_idx) {
                        for k in first..=last {
                            visited_glyphs.insert(k);
                        }
                        let word_text = page_text.text_of(first..last + 1);
                        let trimmed = word_text.trim();
                        if !trimmed.is_empty() {
                            let rects = page_text.glyph_rects(&(first..=last).collect::<Vec<_>>());
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
                                page_tokens.push((trimmed.to_string(), bbox));
                            }
                        }
                    }
                }

                if let Ok(mut idx) = index_arc.lock() {
                    for (token, bbox) in page_tokens {
                        idx.add_token(&token, page_num, bbox);
                    }
                }
            }
        }
    }

    fn request_page_raster(&mut self, page: usize, dpi: u32) {
        if self.requested_pages.contains(&(page, dpi)) {
            return;
        }
        if let Some(bytes) = &self.doc_bytes {
            self.requested_pages.insert((page, dpi));
            let _ = self.raster_tx.send((Arc::clone(bytes), page, dpi));
        }
    }

    /// Pumps asynchronous raster results and ensures the current page texture is loaded.
    /// Automatically upgrades resolution to high-DPI (300 DPI) when zoomed in (> 1.8x).
    pub fn ensure_current_page_texture(&mut self, ctx: &egui::Context) -> Option<(TextureHandle, Vec2)> {
        // 1. Drain any completed raster textures from background worker
        while let Ok(res) = self.raster_rx.try_recv() {
            let handle = ctx.load_texture(
                format!("pdf_p{}_d{}", res.page, res.dpi),
                res.color_image,
                TextureOptions::LINEAR,
            );
            self.page_textures.insert(
                res.page,
                CachedPageTexture {
                    handle,
                    dim: res.dimensions,
                    dpi: res.dpi,
                },
            );
            ctx.request_repaint();
        }

        // 2. Select appropriate DPI for current zoom level
        let target_dpi = if self.zoom > 1.8 { DPI_HIGH } else { DPI_NORMAL };

        // Check if we need to request the page or upgrade its resolution
        let need_request = match self.page_textures.get(&self.current_page) {
            Some(cached) => cached.dpi < target_dpi,
            None => true,
        };

        if need_request {
            self.request_page_raster(self.current_page, target_dpi);
        }

        // 3. Return the best available texture for the current page
        self.page_textures
            .get(&self.current_page)
            .map(|t| (t.handle.clone(), t.dim))
    }

    /// Triggers search and cross-probing navigation to a specific component or net.
    /// Automatically centers the matched component and scales zoom:
    /// - If zoom <= 1.0, increases zoom to 3.0 (300%).
    /// - If zoom > 1.0, preserves the user's active zoom level.
    pub fn cross_probe(&mut self, query: &str) -> bool {
        let results = if let Ok(idx) = self.index.lock() {
            idx.search(query)
        } else {
            Vec::new()
        };

        if !results.is_empty() {
            self.active_search = query.to_string();
            self.search_results = results;
            self.selected_result_idx = 0;
            self.current_page = self.search_results[0].page_number;
            self.is_open = true;

            // Adaptive zoom elevation: if unzoomed (<= 1.0), elevate to 3.0 (300%).
            if self.zoom <= 1.0 {
                self.zoom = 3.0;
            }

            self.pending_center_on_match = true;
            true
        } else {
            false
        }
    }

    pub fn next_match(&mut self) {
        if !self.search_results.is_empty() {
            self.selected_result_idx = (self.selected_result_idx + 1) % self.search_results.len();
            self.current_page = self.search_results[self.selected_result_idx].page_number;
            self.pending_center_on_match = true;
        }
    }

    pub fn prev_match(&mut self) {
        if !self.search_results.is_empty() {
            if self.selected_result_idx == 0 {
                self.selected_result_idx = self.search_results.len() - 1;
            } else {
                self.selected_result_idx -= 1;
            }
            self.current_page = self.search_results[self.selected_result_idx].page_number;
            self.pending_center_on_match = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross_probe_adaptive_zoom_elevation() {
        let mut viewer = PdfSchematicViewer::default();

        // Populate index with a synthetic occurrence for C35
        {
            let mut idx = viewer.index.lock().unwrap();
            idx.add_token("C35", 1, Rect::from_min_max(Pos2::new(100.0, 100.0), Pos2::new(150.0, 120.0)));
        }

        // 1. Initial zoom 1.0 (default) should escalate to 3.0 (300%)
        viewer.zoom = 1.0;
        let matched = viewer.cross_probe("C35");
        assert!(matched);
        assert_eq!(viewer.zoom, 3.0);
        assert!(viewer.pending_center_on_match);

        // 2. Initial zoom 0.75 (zoomed out) should escalate to 3.0
        viewer.zoom = 0.75;
        let matched = viewer.cross_probe("C35");
        assert!(matched);
        assert_eq!(viewer.zoom, 3.0);

        // 3. Initial zoom 3.5 (already magnified) should be preserved
        viewer.zoom = 3.5;
        let matched = viewer.cross_probe("C35");
        assert!(matched);
        assert_eq!(viewer.zoom, 3.5);
    }

    #[test]
    fn test_native_pdfcraft_render_loading_and_indexing() {
        let pdf_path = Path::new("tmp/MS-V372 REV 1.0.pdf");
        if !pdf_path.exists() {
            return;
        }

        let mut viewer = PdfSchematicViewer::default();
        let res = viewer.open_file(pdf_path);
        assert!(res.is_ok(), "Failed to open PDF with pdfcraft-render: {:?}", res);
        assert!(viewer.total_pages > 0);

        // Wait a short moment for background indexing to populate
        for _ in 0..50 {
            if let Ok(flag) = viewer.is_indexing.lock() {
                if !*flag {
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        let idx = viewer.index.lock().unwrap();
        assert!(idx.page_count > 0);
        assert!(!idx.page_sizes.is_empty());
    }
}
