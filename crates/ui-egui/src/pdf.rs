//! High-fidelity PDF schematic renderer and search engine with instant indexing and multi-hit navigation.
//!
//! Features:
//! - Background text indexing through the `RenderPool` (`RequestKind::Text`, one shared parser),
//!   queued behind the visible rasters so opening never blocks and the page shows at once.
//! - Exact multi-occurrence search across pages using `PageText::find_opts` and `glyph_rects`;
//!   an active search refreshes as page texts arrive, and a cross-probe whose designator is not
//!   indexed yet jumps to its first hit when it appears.
//! - Multi-worker asynchronous page rasterization via `RenderPool` (multithreaded, cancel-in-flight).
//! - Dedicated high-priority rendering for requested/navigated pages.
//! - Exact device scale (1 texel per device pixel) keeping vector graphics and text razor-sharp.
//! - Tiled rendering past `tiles::TILE_THRESHOLD`: a low-res backdrop plus only the visible tiles.
//! - Multi-hit selection and navigation list support.

use egui::{pos2, vec2, Color32, ColorImage, Pos2, Rect, TextureHandle, TextureOptions, Vec2};
use pdfcraft_render::{
    device_pixels, DocInfo, PageText, RenderConfig, RenderPool, RenderRequest, RequestKind, Tile,
};
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub mod tiles;

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

/// Convert rendered Pixels (premultiplied RGBA8, as the renderer produces them) into egui::ColorImage
fn texture_image(size: [usize; 2], pixels: pdfcraft_render::Pixels) -> ColorImage {
    ColorImage::from_rgba_premultiplied(size, &pixels)
}

/// Seconds the display scale must stay unchanged before sharp re-renders are requested,
/// so a zoom gesture does not restart full-page interprets on every frame.
const SETTLE_SECS: f64 = 0.12;

/// Rendered results converted into textures per frame at most.
const RESULTS_PER_FRAME: usize = 8;

/// The tag of a text-layer request (as pdfcraft's canvas tags them).
const TEXT_TAG: u64 = 1 << 62;

/// Work counters of the viewer's render loop (diagnostics and regression tests).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewerStats {
    /// Times a different queue was handed to the pool (each may cancel in-flight renders).
    pub queue_changes: u64,
    /// Rendered results turned into textures.
    pub results_used: u64,
    /// Rendered results dropped because their request was no longer queued.
    pub results_dropped: u64,
}

/// Cached whole-page raster (or low-res backdrop of a tiled page) with its rendered scale tag
struct CachedPageTexture {
    handle: TextureHandle,
    tag: u64,
}

fn text_request(page_0: usize) -> RenderRequest {
    RenderRequest { page: page_0, kind: RequestKind::Text, tile: None, scale: 1.0, tag: TEXT_TAG, preview: false }
}

fn pixels_request(page_0: usize, scale: f32, tile: Option<Tile>, preview: bool) -> RenderRequest {
    RenderRequest { page: page_0, kind: RequestKind::Pixels, tile, scale, tag: scale_tag(scale), preview }
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

    // Page text layers indexed so far for whole-document search (0-based page index -> PageText)
    pub page_texts: HashMap<usize, Arc<PageText>>,

    // Pages (0-based) whose text layer could not be extracted: never re-requested
    text_failed: HashSet<usize>,

    // Text requests queued at once: the remaining workers stay free for the visible rasters
    text_slots: usize,

    // A cross-probe/search found nothing yet while pages were still being indexed:
    // jump to its first hit once one is indexed
    search_jump_pending: bool,

    // Whole-page rasters (or backdrops of tiled pages) cached by page number (1-based)
    page_textures: HashMap<usize, CachedPageTexture>,

    // Tiles of the current page when it is too large to rasterize whole
    tiles: tiles::TileCache<TextureHandle>,

    // Multithreaded RenderPool matching pdfcraft's engine
    render_pool: Option<RenderPool>,

    // The queue last handed to the pool; results outside it are obsolete
    last_queue: Vec<RenderRequest>,

    // Pages (1-based) the renderer failed on: never re-requested
    failed_pages: HashSet<usize>,

    // Settle timer tracking display scale changes to avoid worker churning
    settled_scale: f32,
    last_observed_scale: f32,
    last_scale_change_time: f64,
    // Adopt the next display scale at once (cross-probe / match navigation)
    force_settle: bool,

    stats: ViewerStats,
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
            text_failed: HashSet::new(),
            text_slots: 1,
            search_jump_pending: false,
            page_textures: HashMap::new(),
            tiles: tiles::TileCache::default(),
            render_pool: None,
            last_queue: Vec::new(),
            failed_pages: HashSet::new(),
            settled_scale: 0.0,
            last_observed_scale: 0.0,
            last_scale_change_time: 0.0,
            force_settle: false,
            stats: ViewerStats::default(),
        }
    }
}

impl PdfSchematicViewer {
    /// Opens a PDF document using pdfcraft's native inspection. Nothing is rendered or extracted
    /// here: the first paint queues the visible page, and the text layers are indexed behind it.
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
        self.tiles.clear();
        self.last_queue.clear();
        self.failed_pages.clear();
        self.settled_scale = 0.0;
        self.force_settle = true;
        self.active_search.clear();
        self.search_results.clear();
        self.page_texts.clear();
        self.text_failed.clear();
        self.search_jump_pending = false;
        self.zoom = 1.0;
        self.pan_offset = Vec2::ZERO;
        self.is_open = true;

        // 2. One render pool (one shared parser) for rasters and text layers. Like pdfcraft, leave
        //    a core to the UI thread; text indexing takes at most half of the workers.
        let threads = std::thread::available_parallelism()
            .map(|n| n.get().clamp(2, 8) - 1)
            .unwrap_or(3);
        self.text_slots = (threads / 2).max(1);
        // The first paint of the page queues its raster at the exact display scale.
        self.render_pool = Some(RenderPool::new(bytes_arc, threads, RenderConfig::default()));

        Ok(())
    }

    /// Work counters of the render loop since the viewer was created.
    pub fn stats(&self) -> ViewerStats {
        self.stats
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

    /// Pumps rendered results, plans the rasters of the current page laid out at `page_rect`
    /// (screen points) inside `viewport`, and paints them: the whole-page raster (or the low-res
    /// backdrop of a tiled page) first, then the cached tiles on top. Returns whether anything
    /// was painted (false while the first raster of the page is still rendering).
    pub fn paint_current_page(
        &mut self,
        ctx: &egui::Context,
        painter: &egui::Painter,
        page_rect: Rect,
        viewport: Rect,
    ) -> bool {
        let ppp = ctx.pixels_per_point();
        let now = ctx.input(|i| i.time);
        let max_side = ctx.input(|i| i.max_texture_side) as f32;
        let page = self.current_page;
        let size = self.page_size_pts(page);
        let pts = (size.x.max(1.0), size.y.max(1.0));

        // Exactly the device scale: a raster at any other scale is resampled on screen and blurs.
        let wanted = (page_rect.width() * ppp / pts.0).max(0.01);
        if (wanted - self.last_observed_scale).abs() > wanted * 1e-4 {
            self.last_observed_scale = wanted;
            self.last_scale_change_time = now;
        }
        let settled = self.force_settle || self.settled_scale <= 0.0 || now - self.last_scale_change_time > SETTLE_SECS;
        if settled {
            self.settled_scale = wanted;
            self.force_settle = false;
        } else {
            let wait = SETTLE_SECS - (now - self.last_scale_change_time);
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(wait.max(0.0)));
        }

        self.receive(ctx, max_side);

        // Until the zoom settles, rasters stay at the settled scale and are drawn stretched.
        let scale = self.settled_scale;
        let tag = scale_tag(scale);
        let tiled = tiles::is_tiled(pts, scale, max_side);
        let mut queue = Vec::new();

        // 1. The page itself: whole and sharp when small, a low-res backdrop when tiled.
        //    Under tiles, a whole-page raster sharper than the backdrop is a better backdrop: keep it.
        let page_scale = if tiled { tiles::base_scale(pts, max_side) } else { scale };
        let page_ok = self.page_textures.get(&page).is_some_and(|t| {
            t.tag == scale_tag(page_scale) || (tiled && tiles::tag_scale(t.tag) >= page_scale)
        });
        if !page_ok && !self.failed_pages.contains(&page) {
            queue.push(pixels_request(page - 1, page_scale, None, tiled));
        }

        // 2. Tiles intersecting the viewport, nearest its centre first.
        let mut visible_keys = HashSet::new();
        let vis = page_rect.intersect(viewport);
        if tiled && vis.is_positive() {
            let (w, h) = (page_rect.width(), page_rect.height());
            let norm = [
                (vis.min.x - page_rect.min.x) / w,
                (vis.min.y - page_rect.min.y) / h,
                (vis.max.x - page_rect.min.x) / w,
                (vis.max.y - page_rect.min.y) / h,
            ];
            let wanted_tiles = tiles::visible_tiles(pts, scale, max_side, norm);
            visible_keys.extend(wanted_tiles.iter().map(|t| tiles::TileKey::new(page, tag, *t)));
            if settled && !self.failed_pages.contains(&page) {
                for t in self.tiles.missing(page, tag, &wanted_tiles) {
                    queue.push(pixels_request(page - 1, scale, Some(t), false));
                }
            }
        }
        self.tiles.evict(page, tag, &visible_keys);

        // 3. Prefetch the adjacent pages as they would be drawn at this scale.
        for n in [page + 1, page.saturating_sub(1)] {
            if n == page || n == 0 || n > self.total_pages || self.failed_pages.contains(&n) {
                continue;
            }
            let s = self.page_size_pts(n);
            let n_pts = (s.x.max(1.0), s.y.max(1.0));
            let n_tiled = tiles::is_tiled(n_pts, scale, max_side);
            let n_scale = if n_tiled { tiles::base_scale(n_pts, max_side) } else { scale };
            if self.page_textures.get(&n).is_none_or(|t| t.tag != scale_tag(n_scale)) {
                queue.push(pixels_request(n - 1, n_scale, None, n_tiled));
            }
        }
        self.page_textures.retain(|p, _| p.abs_diff(page) <= 1);

        // 4. Text layers still to index, the current page first, behind every raster.
        queue.extend(
            (0..self.total_pages)
                .map(|k| (page - 1 + k) % self.total_pages)
                .filter(|p| !self.page_texts.contains_key(p) && !self.text_failed.contains(p))
                .take(self.text_slots)
                .map(text_request),
        );

        // Replacing the queue cancels in-flight renders that are no longer wanted.
        if let Some(pool) = &self.render_pool
            && queue != self.last_queue
        {
            pool.set_queue(queue.clone());
            self.last_queue = queue;
            self.stats.queue_changes += 1;
        }
        if !self.last_queue.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }

        self.paint(painter, page_rect, viewport, pts, tag, tiled, ppp)
    }

    /// Converts a bounded batch of finished renders into textures. Results that are no longer
    /// queued (an obsolete scale, page or tile) are dropped without allocating anything.
    fn receive(&mut self, ctx: &egui::Context, max_side: f32) {
        let Some(pool) = &self.render_pool else { return };
        let mut texts_arrived = false;
        for _ in 0..RESULTS_PER_FRAME {
            let Some(res) = pool.try_recv() else { break };
            if !self.last_queue.contains(&res.request) {
                self.stats.results_dropped += 1;
                continue;
            }
            self.stats.results_used += 1;
            ctx.request_repaint();
            if res.request.kind == RequestKind::Text {
                match res.text {
                    Some(text) if res.error.is_none() => {
                        self.page_texts.insert(res.request.page, text);
                    }
                    _ => {
                        self.text_failed.insert(res.request.page);
                    }
                }
                texts_arrived = true;
                continue;
            }
            let page_1 = res.request.page + 1;
            if res.error.is_some() {
                self.failed_pages.insert(page_1);
                continue;
            }
            let (w, h) = (res.width as usize, res.height as usize);
            if w == 0 || h == 0 || w as f32 > max_side || h as f32 > max_side || res.rgba.len() != tiles::rgba_bytes(w, h) {
                continue;
            }
            let tag = res.request.tag;
            let image = texture_image([w, h], res.rgba);
            match res.request.tile {
                Some(t) => {
                    let handle = ctx.load_texture(format!("pdf_p{page_1}_t{tag}_{}_{}", t.x, t.y), image, TextureOptions::LINEAR);
                    self.tiles.insert(tiles::TileKey::new(page_1, tag, t), t, handle);
                }
                None => {
                    let handle = ctx.load_texture(format!("pdf_p{page_1}_t{tag}"), image, TextureOptions::LINEAR);
                    self.page_textures.insert(page_1, CachedPageTexture { handle, tag });
                }
            }
        }
        if texts_arrived {
            self.refresh_search();
        }
    }

    /// Whether every page's text layer has been indexed (or failed).
    pub fn text_indexed(&self) -> bool {
        self.page_texts.len() + self.text_failed.len() >= self.total_pages
    }

    /// Re-runs the active search over the pages indexed so far, keeping the selected hit, and
    /// performs a pending cross-probe jump once its first hit is indexed.
    fn refresh_search(&mut self) {
        if self.active_search.is_empty() {
            return;
        }
        let selected = self
            .search_results
            .get(self.selected_result_idx)
            .map(|o| (o.page_number, o.glyph_range.clone()));
        let query = self.active_search.clone();
        self.perform_instant_search(&query);
        if let Some(idx) = selected.and_then(|(page, range)| {
            self.search_results.iter().position(|o| o.page_number == page && o.glyph_range == range)
        }) {
            self.selected_result_idx = idx;
        } else if self.search_jump_pending && !self.search_results.is_empty() {
            self.search_jump_pending = false;
            self.select_match(0);
        } else {
            self.selected_result_idx = self.selected_result_idx.min(self.search_results.len().saturating_sub(1));
        }
        if self.text_indexed() {
            self.search_jump_pending = false;
        }
    }

    /// Paints the current page's raster stretched over `page_rect`, then (when tiled) its cached
    /// tiles: earlier zooms first, stretched, the tiles at `tag` on top, snapped to device pixels.
    #[allow(clippy::too_many_arguments)]
    fn paint(
        &self,
        painter: &egui::Painter,
        page_rect: Rect,
        viewport: Rect,
        pts: (f32, f32),
        tag: u64,
        tiled: bool,
        ppp: f32,
    ) -> bool {
        let full_uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        let mut painted = false;
        if let Some(t) = self.page_textures.get(&self.current_page) {
            painter.image(t.handle.id(), page_rect, full_uv, Color32::WHITE);
            painted = true;
        }
        if !tiled {
            return painted;
        }
        let snap = |p: Pos2| pos2((p.x * ppp).round() / ppp, (p.y * ppp).round() / ppp);
        let (w, h) = (page_rect.width(), page_rect.height());
        for (key, t, tex) in self.tiles.paint_order(self.current_page, tag) {
            let s = tiles::tag_scale(key.tag);
            let (dw, dh) = (device_pixels(pts.0, s) as f32, device_pixels(pts.1, s) as f32);
            let min = page_rect.min + vec2(t.x as f32 / dw * w, t.y as f32 / dh * h);
            let max = page_rect.min + vec2((t.x + t.w) as f32 / dw * w, (t.y + t.h) as f32 / dh * h);
            let r = Rect::from_min_max(snap(min), snap(max));
            if r.intersects(viewport) {
                painter.image(tex.id(), r, full_uv, Color32::WHITE);
                painted = true;
            }
        }
        painted
    }

    /// Evaluates `active_search` across the pages whose `PageText` is indexed so far.
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
            // Render the target page at its new scale at once, without waiting for a settle
            self.force_settle = true;
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
        // Not indexed yet: jump when the first hit arrives.
        self.search_jump_pending = self.search_results.is_empty() && !self.text_indexed();

        // Adaptive zoom elevation: if unzoomed (<= 1.0), elevate to 3.0 (300%).
        if self.zoom <= 1.0 {
            self.zoom = 3.0;
        }

        if !self.search_results.is_empty() {
            let target_page = self.search_results[0].page_number;
            self.current_page = target_page;
            self.pending_center_on_match = true;
            // Force immediate sharp rasterization of the target page at the elevated zoom
            self.force_settle = true;
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

    /// Opens the fixture and lays out its first page `width` points wide at `origin` inside
    /// `viewport`; returns the viewer, the page rect and the queue handed to the pool.
    fn plan_fixture(origin: Pos2, width: f32, viewport: Rect) -> Option<(PdfSchematicViewer, Rect, Vec<RenderRequest>)> {
        // Tests run from the crate directory; the fixture lives in the workspace `tmp/`.
        let pdf_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/MS-V375 REV 7.0.pdf");
        if !pdf_path.exists() {
            return None;
        }
        let mut viewer = PdfSchematicViewer::default();
        viewer.open_file(&pdf_path).ok()?;
        let size = viewer.page_size_pts(1);
        let page_rect = Rect::from_min_size(origin, vec2(width, width * size.y / size.x));
        let ctx = egui::Context::default();
        let painter = egui::Painter::new(ctx.clone(), egui::LayerId::background(), viewport);
        viewer.paint_current_page(&ctx, &painter, page_rect, viewport);
        let queue = viewer.last_queue.clone();
        Some((viewer, page_rect, queue))
    }

    #[test]
    fn test_zoomed_page_requests_only_visible_tiles_at_exact_scale() {
        let viewport = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
        // A page laid out 30000 points wide around the viewport: far past the old 8192 px clamp.
        let Some((viewer, page_rect, queue)) = plan_fixture(pos2(-14000.0, -9000.0), 30000.0, viewport) else {
            return;
        };
        let size = viewer.page_size_pts(1);
        let scale = 30000.0 / size.x;

        // The low-res backdrop of the current page comes first.
        let first = queue.first().expect("backdrop requested");
        assert_eq!((first.page, first.tile, first.preview), (0, None, true));
        assert!(size.x.max(size.y) * first.scale <= tiles::BASE_SIDE + 1.0);

        // Then tiles at the exact, unclamped device scale, each one under the viewport.
        let tiles: Vec<_> = queue.iter().filter(|r| r.page == 0 && r.tile.is_some()).collect();
        assert!(!tiles.is_empty());
        let (dw, dh) = (device_pixels(size.x, scale) as f32, device_pixels(size.y, scale) as f32);
        let (w, h) = (page_rect.width(), page_rect.height());
        for r in &tiles {
            assert!((r.scale - scale).abs() < 1e-3, "tile scale {} != {scale}", r.scale);
            let t = r.tile.unwrap();
            let min = page_rect.min + vec2(t.x as f32 / dw * w, t.y as f32 / dh * h);
            let max = page_rect.min + vec2((t.x + t.w) as f32 / dw * w, (t.y + t.h) as f32 / dh * h);
            assert!(Rect::from_min_max(min, max).intersects(viewport), "{t:?} is off screen");
        }
        let whole = tiles::visible_tiles((size.x, size.y), scale, 2048.0, [0.0, 0.0, 1.0, 1.0]);
        assert!(tiles.len() < whole.len() / 4, "{} of {} tiles requested", tiles.len(), whole.len());
    }

    #[test]
    fn test_small_page_renders_whole_without_tiles() {
        let viewport = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
        let page_rect = Rect::from_min_size(pos2(50.0, 50.0), vec2(700.0, 1.0));
        let Some((viewer, _, queue)) = plan_fixture(page_rect.min, page_rect.width(), viewport) else { return };
        let size = viewer.page_size_pts(1);
        let first = queue.first().expect("page requested");
        assert_eq!((first.page, first.tile, first.preview), (0, None, false));
        assert!((first.scale - 700.0 / size.x).abs() < 1e-3);
        assert!(queue.iter().all(|r| r.tile.is_none()));
    }

    #[test]
    fn test_ms_v375_instant_search_multiple_occurrences() {
        let Some((mut viewer, sim)) = open_fixture() else { return };
        sim.index_all_text(&mut viewer);

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

    /// Opening must not extract text: the first paint queues the visible page first and only a
    /// bounded number of text layers behind it, all through the pool's one shared parser.
    #[test]
    fn test_open_defers_text_and_queues_visible_page_first() {
        let Some((mut viewer, sim)) = open_fixture() else { return };
        assert!(viewer.page_texts.is_empty(), "open_file must not extract text");
        let pool = viewer.render_pool.as_ref().expect("pool").stats();
        assert_eq!(pool.text_interpretations, 0);

        sim.frame(&mut viewer);
        let queue = viewer.last_queue.clone();
        let first = queue.first().expect("visible page queued");
        assert_eq!((first.page, first.kind), (0, RequestKind::Pixels));
        let texts: Vec<_> = queue.iter().filter(|r| r.kind == RequestKind::Text).collect();
        assert!(!texts.is_empty() && texts.len() <= viewer.text_slots, "{} text requests", texts.len());
        assert_eq!(texts[0].page, 0, "the current page's text is indexed first");
        let first_text = queue.iter().position(|r| r.kind == RequestKind::Text).unwrap();
        assert!(queue[..first_text].iter().all(|r| r.kind == RequestKind::Pixels), "text queued behind rasters");
    }

    /// Indexing a whole static document interprets every page's text exactly once on one parser,
    /// renders nothing twice and wastes no result.
    #[test]
    fn test_text_index_runs_once_per_page_on_shared_parser() {
        let Some((mut viewer, sim)) = open_fixture() else { return };
        sim.index_all_text(&mut viewer);
        let pool = viewer.render_pool.as_ref().expect("pool").stats();
        assert_eq!(viewer.page_texts.len(), viewer.total_pages);
        assert_eq!(pool.parser_builds, 1, "{pool:?}");
        assert_eq!(pool.text_interpretations as usize, viewer.total_pages, "{pool:?}");
        // Page 1 and its prefetched neighbour, once each.
        assert!(pool.page_interpretations <= 2, "{pool:?}");
        assert_eq!(viewer.stats().results_dropped, 0);
    }

    /// A cross-probe issued before its designator is indexed jumps to the first hit when it arrives.
    #[test]
    fn test_cross_probe_before_indexing_jumps_when_hit_arrives() {
        let Some((mut viewer, sim)) = open_fixture() else { return };
        assert!(!viewer.cross_probe("U12"), "nothing indexed yet");
        assert_eq!(viewer.current_page, 1);
        sim.index_all_text(&mut viewer);
        assert!(viewer.search_results.len() > 1);
        let hit = &viewer.search_results[viewer.selected_result_idx];
        assert_eq!(viewer.current_page, hit.page_number);
        assert!(hit.text.contains("U12"));
    }

    /// Drives the viewer like the PDF panel in `lib.rs` does: a `panel` rect, the page fitted at
    /// 95% and scaled by `zoom`, centred on the selected match when one is pending.
    struct FrameSim {
        ctx: egui::Context,
        panel: Rect,
        ppp: f32,
        start: std::time::Instant,
    }

    impl FrameSim {
        fn new(panel: Rect, ppp: f32) -> Self {
            Self { ctx: egui::Context::default(), panel, ppp, start: std::time::Instant::now() }
        }

        /// One frame; returns whether the page painted anything.
        fn frame(&self, viewer: &mut PdfSchematicViewer) -> bool {
            let mut raw = egui::RawInput {
                time: Some(self.start.elapsed().as_secs_f64()),
                max_texture_side: Some(8192),
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1920.0, 1080.0))),
                ..Default::default()
            };
            raw.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(self.ppp);
            self.ctx.begin_pass(raw);
            let rect = self.panel;
            let base = viewer.page_size_pts(viewer.current_page) * PT;
            let fit = ((rect.width() * 0.95) / base.x.max(1.0)).min((rect.height() * 0.95) / base.y.max(1.0));
            let display = fit * viewer.zoom;
            let size = base * display;
            if viewer.pending_center_on_match
                && let Some(hit) = viewer.search_results.get(viewer.selected_result_idx)
                && hit.page_number == viewer.current_page
            {
                let c = hit.bbox.center() * PT * display;
                viewer.pan_offset = vec2(size.x * 0.5 - c.x, size.y * 0.5 - c.y);
                viewer.pending_center_on_match = false;
            }
            let page_rect = Rect::from_min_size(rect.min + (rect.size() - size) * 0.5 + viewer.pan_offset, size);
            let painter = egui::Painter::new(self.ctx.clone(), egui::LayerId::background(), rect);
            let painted = viewer.paint_current_page(&self.ctx, &painter, page_rect, rect);
            // No renderer applies the texture uploads here (epaint asserts on dropping them).
            self.ctx.end_pass().textures_delta.clear();
            painted
        }

        /// Runs frames until every page's text is indexed (generous bound: not a timing check).
        fn index_all_text(&self, viewer: &mut PdfSchematicViewer) {
            let t0 = std::time::Instant::now();
            while !viewer.text_indexed() {
                assert!(t0.elapsed() < std::time::Duration::from_secs(120), "text indexing stalled");
                self.frame(viewer);
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            // Let the last refresh settle the queue.
            self.frame(viewer);
        }

        /// Runs ~60 Hz frames until the queue drains (or `limit`); returns (first paint, drained).
        fn run_until_idle(&self, viewer: &mut PdfSchematicViewer, limit: std::time::Duration) -> (Option<f64>, Option<f64>) {
            let t0 = std::time::Instant::now();
            let mut first = None;
            while t0.elapsed() < limit {
                let painted = self.frame(viewer);
                if painted && first.is_none() {
                    first = Some(t0.elapsed().as_secs_f64());
                }
                if first.is_some() && viewer.last_queue.is_empty() {
                    return (first, Some(t0.elapsed().as_secs_f64()));
                }
                std::thread::sleep(std::time::Duration::from_millis(16));
            }
            (first, None)
        }
    }

    fn open_fixture() -> Option<(PdfSchematicViewer, FrameSim)> {
        let path = fixture_path();
        if !path.exists() {
            return None;
        }
        let mut viewer = PdfSchematicViewer::default();
        viewer.open_file(&path).ok()?;
        Some((viewer, FrameSim::new(Rect::from_min_size(pos2(0.0, 60.0), vec2(860.0, 900.0)), 1.0)))
    }

    fn fixture_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/MS-V375 REV 7.0.pdf")
    }

    /// Measures open, first paint and a cross-probe jump on the fixture (prints numbers).
    /// `cargo test -p bvc-ui-egui --release bench_open_and_cross_probe -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn bench_open_and_cross_probe() {
        let path = fixture_path();
        if !path.exists() {
            return;
        }
        let ppp: f32 = std::env::var("BVC_PPP").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
        let mut viewer = PdfSchematicViewer::default();
        let t = std::time::Instant::now();
        viewer.open_file(&path).expect("fixture opens");
        println!("open_file: {:.3}s ({} pages)", t.elapsed().as_secs_f64(), viewer.total_pages);

        let sim = FrameSim::new(Rect::from_min_size(pos2(0.0, 60.0), vec2(860.0, 900.0)), ppp);
        let limit = std::time::Duration::from_secs(60);
        let (first, idle) = sim.run_until_idle(&mut viewer, limit);
        let pool = viewer.render_pool.as_ref().map(RenderPool::stats);
        println!("first page: painted {first:?}s idle {idle:?}s viewer {:?} pool {pool:?}", viewer.stats());

        let t = std::time::Instant::now();
        sim.index_all_text(&mut viewer);
        let pool = viewer.render_pool.as_ref().map(RenderPool::stats);
        println!("text indexed in {:.3}s more, pool {pool:?}", t.elapsed().as_secs_f64());

        let before = viewer.stats();
        let t = std::time::Instant::now();
        assert!(viewer.cross_probe("U12"));
        println!("cross_probe search: {:.3}s -> page {}", t.elapsed().as_secs_f64(), viewer.current_page);
        let (first, idle) = sim.run_until_idle(&mut viewer, limit);
        let s = viewer.stats();
        let pool = viewer.render_pool.as_ref().map(RenderPool::stats);
        println!(
            "cross-probe page: painted {first:?}s idle {idle:?}s queue_changes {} used {} dropped {} pool {pool:?}",
            s.queue_changes - before.queue_changes,
            s.results_used - before.results_used,
            s.results_dropped - before.results_dropped
        );

        // A cross-probe right after open, while the text layers are still being indexed.
        let mut viewer = PdfSchematicViewer::default();
        viewer.open_file(&path).expect("fixture opens");
        let t = std::time::Instant::now();
        while !sim.frame(&mut viewer) {
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
        viewer.cross_probe("U12");
        let mut jumped = None;
        while t.elapsed() < limit {
            let painted = sim.frame(&mut viewer);
            if viewer.current_page != 1 && jumped.is_none() {
                jumped = Some(t.elapsed().as_secs_f64());
            }
            if jumped.is_some() && painted && viewer.page_textures.contains_key(&viewer.current_page) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
        println!(
            "early cross-probe: jumped at {jumped:?}s, page {} painted at {:.3}s (indexed {}/{})",
            viewer.current_page,
            t.elapsed().as_secs_f64(),
            viewer.page_texts.len(),
            viewer.total_pages
        );
    }
}
