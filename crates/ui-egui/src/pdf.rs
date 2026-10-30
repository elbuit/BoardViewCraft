//! High-fidelity PDF schematic renderer and cross-probing indexer.
//!
//! Features:
//! - High-fidelity vector rasterization of schematic pages into pixel-crisp textures.
//! - Bounding-box text token extraction for instant, exact cross-probing of component designators and nets.
//! - Multi-page navigation, pan & zoom, and highlight overlays.

use egui::{ColorImage, Pos2, Rect, TextureHandle, TextureOptions, Vec2};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
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
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '/' || *c == '#' || *c == '-')
            .collect()
    }

    pub fn add_token(&mut self, text: &str, page_number: usize, bbox: Rect) {
        let key = Self::normalize_key(text);
        if key.len() < 2 {
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

        // 1. Exact match
        if let Some(matches) = self.tokens.get(&key) {
            return matches.clone();
        }

        // 2. Substring match fallback
        let mut results = Vec::new();
        for (k, list) in &self.tokens {
            if k == &key || (key.len() >= 3 && k.contains(&key)) {
                results.extend(list.iter().cloned());
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

    // Background raster worker communication
    raster_tx: Sender<(PathBuf, usize, u32)>,
    raster_rx: Receiver<PageRasterResult>,
    requested_pages: HashSet<(usize, u32)>,
}

impl Default for PdfSchematicViewer {
    fn default() -> Self {
        let (res_tx, res_rx) = channel();
        let (req_tx, req_rx) = channel::<(PathBuf, usize, u32)>();

        // Spawn dedicated background rasterization worker
        thread::Builder::new()
            .name("bvc-pdf-rasterizer".into())
            .spawn(move || {
                while let Ok((path, page, dpi)) = req_rx.recv() {
                    if let Ok(dyn_img) = Self::rasterize_page_sync(&path, page, dpi) {
                        let rgba = dyn_img.to_rgba8();
                        let (w, h) = (rgba.width() as usize, rgba.height() as usize);
                        let color_image = ColorImage::from_rgba_unmultiplied([w, h], rgba.as_raw());
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
            raster_tx: req_tx,
            raster_rx: res_rx,
            requested_pages: HashSet::new(),
        }
    }
}

impl PdfSchematicViewer {
    pub fn open_file(&mut self, path: &Path) -> Result<(), String> {
        let path_buf = path.to_path_buf();
        self.path = Some(path_buf.clone());
        self.filename = Some(
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("schematic.pdf")
                .to_string(),
        );

        // 1. Read document info and total pages (nearly instantaneous)
        let total_pages = Self::query_page_count(path)?;
        self.total_pages = total_pages.max(1);
        self.current_page = 1;
        self.page_textures.clear();
        self.requested_pages.clear();
        self.search_results.clear();
        self.zoom = 1.0;
        self.pan_offset = Vec2::ZERO;
        self.is_open = true;

        // 2. Launch background indexing so opening the document is immediate
        let index_arc = Arc::clone(&self.index);
        let indexing_flag = Arc::clone(&self.is_indexing);
        if let Ok(mut flag) = indexing_flag.lock() {
            *flag = true;
        }

        let total_pages_val = self.total_pages;
        let bg_path = path_buf.clone();
        thread::Builder::new()
            .name("bvc-pdf-indexer".into())
            .spawn(move || {
                let idx = Self::build_index(&bg_path, total_pages_val);
                if let Ok(mut locked_idx) = index_arc.lock() {
                    *locked_idx = idx;
                }
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

    fn query_page_count(path: &Path) -> Result<usize, String> {
        if let Ok(out) = Command::new("pdfinfo").arg(path).output() {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                if line.starts_with("Pages:") {
                    if let Some(num_str) = line.split_whitespace().nth(1) {
                        if let Ok(n) = num_str.parse::<usize>() {
                            return Ok(n);
                        }
                    }
                }
            }
        }
        // Fallback: check file bytes
        if let Ok(bytes) = std::fs::read(path) {
            let str_data = String::from_utf8_lossy(&bytes);
            let count = str_data.matches("/Type /Page").count();
            if count > 0 {
                return Ok(count);
            }
        }
        Ok(1)
    }

    /// Extract XML word bounding boxes using pdftotext for instant, pixel-accurate cross-probing.
    fn build_index(path: &Path, pages: usize) -> SchematicIndex {
        let mut index = SchematicIndex::new();
        index.page_count = pages;

        let output = Command::new("pdftotext")
            .arg("-bbox")
            .arg(path)
            .arg("-")
            .output();

        if let Ok(out) = output {
            let text = String::from_utf8_lossy(&out.stdout);
            let mut current_page = 0;
            let mut page_width = 1224.0f32;
            let mut page_height = 792.0f32;

            for line in text.lines() {
                if let Some(p_idx) = line.find("<page ") {
                    let page_str = &line[p_idx..];
                    current_page += 1;
                    if let Some(w_s) = page_str.split("width=\"").nth(1).and_then(|s| s.split('\"').next()) {
                        if let Ok(w) = w_s.parse::<f32>() {
                            page_width = w;
                        }
                    }
                    if let Some(h_s) = page_str.split("height=\"").nth(1).and_then(|s| s.split('\"').next()) {
                        if let Ok(h) = h_s.parse::<f32>() {
                            page_height = h;
                        }
                    }
                    index.page_sizes.insert(current_page, egui::vec2(page_width, page_height));
                }

                if line.contains("<word ") && current_page >= 1 {
                    if let (Some(x1_s), Some(y1_s), Some(x2_s), Some(y2_s), Some(token)) = (
                        line.split("xMin=\"").nth(1).and_then(|s| s.split('\"').next()),
                        line.split("yMin=\"").nth(1).and_then(|s| s.split('\"').next()),
                        line.split("xMax=\"").nth(1).and_then(|s| s.split('\"').next()),
                        line.split("yMax=\"").nth(1).and_then(|s| s.split('\"').next()),
                        line.split('>').nth(1).and_then(|s| s.split('<').next()),
                    ) {
                        let trimmed = token.trim();
                        if !trimmed.is_empty() {
                            if let (Ok(x1), Ok(y1), Ok(x2), Ok(y2)) = (
                                x1_s.parse::<f32>(),
                                y1_s.parse::<f32>(),
                                x2_s.parse::<f32>(),
                                y2_s.parse::<f32>(),
                            ) {
                                let bbox = Rect::from_min_max(Pos2::new(x1, y1), Pos2::new(x2, y2));
                                index.add_token(trimmed, current_page, bbox);
                            }
                        }
                    }
                }
            }
        }

        index
    }

    fn rasterize_page_sync(path: &Path, page: usize, dpi: u32) -> Result<image::DynamicImage, String> {
        let output = Command::new("pdftoppm")
            .arg("-png")
            .arg("-r")
            .arg(dpi.to_string())
            .arg("-f")
            .arg(page.to_string())
            .arg("-l")
            .arg(page.to_string())
            .arg(path)
            .output()
            .map_err(|e| format!("pdftoppm execution failed: {e}"))?;

        if !output.status.success() {
            return Err("pdftoppm returned non-zero exit code".into());
        }

        image::load_from_memory(&output.stdout).map_err(|e| format!("Image decode failed: {e}"))
    }

    fn request_page_raster(&mut self, page: usize, dpi: u32) {
        if self.requested_pages.contains(&(page, dpi)) {
            return;
        }
        if let Some(path) = &self.path {
            self.requested_pages.insert((page, dpi));
            let _ = self.raster_tx.send((path.clone(), page, dpi));
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

        let page = self.current_page;
        let target_dpi = if self.zoom > 1.8 { DPI_HIGH } else { DPI_NORMAL };

        // 2. Check if current page texture is already available
        if let Some(cached) = self.page_textures.get(&page) {
            let res = (cached.handle.clone(), cached.dim);
            // If zoomed in and only low-DPI is cached, request high-DPI upgrade in background
            if target_dpi == DPI_HIGH && cached.dpi < DPI_HIGH {
                self.request_page_raster(page, DPI_HIGH);
            }
            return Some(res);
        }

        // 3. Texture not ready yet: request it immediately and prefetch adjacent pages
        self.request_page_raster(page, target_dpi);
        if page > 1 {
            self.request_page_raster(page - 1, DPI_NORMAL);
        }
        if page < self.total_pages {
            self.request_page_raster(page + 1, DPI_NORMAL);
        }

        None
    }

    /// Cross-probing trigger: locate designator or net on the schematic.
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

        if let Some(first) = self.search_results.first() {
            self.current_page = first.page_number;
            self.pending_center_on_match = true;
        }
        true
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
