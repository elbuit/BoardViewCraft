//! High-fidelity PDF schematic renderer and cross-probing indexer.
//!
//! Features:
//! - High-fidelity vector rasterization of schematic pages into pixel-crisp textures.
//! - Bounding-box text token extraction for instant, exact cross-probing of component designators and nets.
//! - Multi-page navigation, pan & zoom, and highlight overlays.

use egui::{ColorImage, Pos2, Rect, TextureHandle, TextureOptions, Vec2};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

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
}

/// Embedded schematic viewer and raster renderer.
pub struct PdfSchematicViewer {
    pub path: Option<PathBuf>,
    pub filename: Option<String>,
    pub current_page: usize,
    pub total_pages: usize,
    pub index: SchematicIndex,
    pub active_search: String,
    pub search_results: Vec<SchematicOccurrence>,
    pub selected_result_idx: usize,
    pub zoom: f32,
    pub pan_offset: Vec2,
    pub is_open: bool,

    // High-fidelity page textures cached by page number
    page_textures: HashMap<usize, TextureHandle>,
    page_dimensions: HashMap<usize, Vec2>,
}

impl Default for PdfSchematicViewer {
    fn default() -> Self {
        Self {
            path: None,
            filename: None,
            current_page: 1,
            total_pages: 1,
            index: SchematicIndex::new(),
            active_search: String::new(),
            search_results: Vec::new(),
            selected_result_idx: 0,
            zoom: 1.0,
            pan_offset: Vec2::ZERO,
            is_open: false,
            page_textures: HashMap::new(),
            page_dimensions: HashMap::new(),
        }
    }
}

impl PdfSchematicViewer {
    pub fn open_file(&mut self, path: &Path) -> Result<(), String> {
        self.path = Some(path.to_path_buf());
        self.filename = Some(
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("schematic.pdf")
                .to_string(),
        );

        // 1. Read document info and total pages
        let total_pages = Self::query_page_count(path)?;
        self.total_pages = total_pages.max(1);
        self.current_page = 1;

        // 2. Build cross-probing index with bounding box coordinates
        self.index = Self::build_index(path, self.total_pages);
        self.page_textures.clear();
        self.page_dimensions.clear();
        self.search_results.clear();
        self.zoom = 1.0;
        self.pan_offset = Vec2::ZERO;
        self.is_open = true;
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

    /// Ensure the current page texture is rasterized at high resolution and cached.
    pub fn ensure_current_page_texture(&mut self, ctx: &egui::Context) -> Option<(TextureHandle, Vec2)> {
        let page = self.current_page;
        if let (Some(tex), Some(&dim)) = (self.page_textures.get(&page), self.page_dimensions.get(&page)) {
            return Some((tex.clone(), dim));
        }

        let Some(path) = &self.path else { return None };

        // Rasterize page to high-res PNG (150 DPI) in memory via pdftoppm stdout
        let child = Command::new("pdftoppm")
            .arg("-png")
            .arg("-r")
            .arg("150")
            .arg("-f")
            .arg(page.to_string())
            .arg("-l")
            .arg(page.to_string())
            .arg(path)
            .output();

        if let Ok(output) = child {
            if let Ok(dyn_img) = image::load_from_memory(&output.stdout) {
                let rgba = dyn_img.to_rgba8();
                let (width, height) = (rgba.width() as usize, rgba.height() as usize);
                let color_img = ColorImage::from_rgba_unmultiplied([width, height], rgba.as_raw());
                let handle = ctx.load_texture(
                    format!("pdf_page_{page}"),
                    color_img,
                    TextureOptions::LINEAR,
                );
                let dim = Vec2::new(width as f32, height as f32);
                self.page_textures.insert(page, handle.clone());
                self.page_dimensions.insert(page, dim);
                return Some((handle, dim));
            }
        }
        None
    }

    /// Cross-probing trigger: locate designator or net on the schematic.
    pub fn cross_probe(&mut self, query: &str) -> bool {
        let hits = self.index.search(query);
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
        }
        true
    }

    pub fn next_match(&mut self) {
        if !self.search_results.is_empty() {
            self.selected_result_idx = (self.selected_result_idx + 1) % self.search_results.len();
            self.current_page = self.search_results[self.selected_result_idx].page_number;
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
        }
    }
}
