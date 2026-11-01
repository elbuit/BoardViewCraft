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
//! - Instant page jumps: low-res previews of every page (search-result pages first) prefetched in
//!   the background, sharp rasters of visited pages kept within a byte budget, and background work
//!   held while a page the viewer jumped to has no raster yet.
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

/// Matches of a query across `pages` pages as (page index, glyph range), in page order.
/// `find(page, whole_words)` returns a page's matches. Whole-word matches win across the whole
/// document, so "C212" never lists "BC212" or "MC212" while an exact "C212" exists anywhere;
/// substring matches are used only when no page has a whole-word match (returned flag `true`).
fn document_matches(pages: usize, mut find: impl FnMut(usize, bool) -> Vec<Range<usize>>) -> (Vec<(usize, Range<usize>)>, bool) {
    let mut collect = |whole: bool| -> Vec<(usize, Range<usize>)> {
        (0..pages).flat_map(|p| find(p, whole).into_iter().map(move |r| (p, r))).collect()
    };
    let exact = collect(true);
    if exact.is_empty() {
        (collect(false), true)
    } else {
        (exact, false)
    }
}

/// The request tag for a raster at `scale`: equal tags mean the same scale (to 1/65536).
pub fn scale_tag(scale: f32) -> u64 {
    (f64::from(scale) * 65536.0).round() as u64
}

/// A rendered raster as texture data, as pdfcraft's `texture_image` (canvas/cache.rs). Its words
/// are premultiplied RGBA bytes, exactly `Color32`s, so the renderer's buffer becomes the image
/// without a per-pixel copy on the UI thread; a buffer that cannot be reused is copied.
fn texture_image(size: [usize; 2], pixels: pdfcraft_render::Pixels) -> ColorImage {
    match bytemuck::allocation::try_cast_vec::<u32, Color32>(pixels.into_words()) {
        Ok(px) if px.len() == size[0].saturating_mul(size[1]) => ColorImage::new(size, px),
        Ok(px) => ColorImage::from_rgba_premultiplied(size, bytemuck::cast_slice(&px)),
        Err((_, words)) => ColorImage::from_rgba_premultiplied(size, bytemuck::cast_slice(&words)),
    }
}

/// Seconds the display scale must stay unchanged before sharp re-renders are requested,
/// so a zoom gesture does not restart full-page interprets on every frame.
const SETTLE_SECS: f64 = 0.12;

/// Rendered results converted into textures per frame at most.
const RESULTS_PER_FRAME: usize = 8;

/// The tag of a text-layer request (as pdfcraft's canvas tags them).
const TEXT_TAG: u64 = 1 << 62;

/// The tag bit of a page preview (pdfcraft's thumbnails), so it never matches a page raster.
const PREVIEW_TAG: u64 = 1 << 61;

/// Longest device-pixel side of a page preview: shown at once when the viewer jumps to a page
/// whose sharp raster is not cached, until the sharp one arrives.
pub const PREVIEW_SIDE: f32 = 640.0;

/// Byte allowance of cached page previews (RGBA8); search-result pages are admitted first.
pub const PREVIEW_BYTES: usize = 64 * 1024 * 1024;

/// Preview requests queued at once (each costs about a page interpret).
const PREVIEW_BATCH: usize = 8;

/// Byte allowance and count of cached sharp page rasters, as pdfcraft's `PAGE_BYTES` and
/// `MAX_CACHED_PAGES`: pages turned away from stay cached, nearest the current page first.
pub const PAGE_BYTES: usize = 128 * 1024 * 1024;
const MAX_CACHED_PAGES: usize = 24;

/// Work counters of the viewer's render loop (diagnostics and regression tests).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewerStats {
    /// Times a different queue was handed to the pool (each may cancel in-flight renders).
    pub queue_changes: u64,
    /// Rendered results turned into textures.
    pub results_used: u64,
    /// Rendered results dropped because their request was no longer queued.
    pub results_dropped: u64,
    /// Pixel results turned into textures (page rasters, previews and tiles).
    pub rasters_used: u64,
    /// Device pixels of the rasters turned into textures.
    pub raster_pixels: u64,
    /// Worker milliseconds the rasters turned into textures took to render.
    pub raster_millis: u64,
    /// UI-thread microseconds spent turning rasters into textures (conversion and upload queueing).
    pub convert_micros: u64,
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

/// Scale of the preview of a page `pts` (width, height in PDF points): [`PREVIEW_SIDE`] long.
fn preview_scale(pts: (f32, f32)) -> f32 {
    PREVIEW_SIDE / pts.0.max(pts.1).max(1.0)
}

fn preview_request(page_0: usize, pts: (f32, f32)) -> RenderRequest {
    let scale = preview_scale(pts);
    RenderRequest { page: page_0, kind: RequestKind::Pixels, tile: None, scale, tag: PREVIEW_TAG | scale_tag(scale), preview: true }
}

/// The pages of `order` (most wanted first) that fit `budget` bytes and `max_count` entries,
/// `bytes(page)` being each one's size. Pages that do not fit are skipped, not the rest.
pub fn admit_by_budget(order: &[usize], bytes: impl Fn(usize) -> usize, budget: usize, max_count: usize) -> Vec<usize> {
    let mut used = 0usize;
    let mut admitted = Vec::new();
    for &p in order {
        if admitted.len() >= max_count {
            break;
        }
        let b = bytes(p);
        if used.saturating_add(b) <= budget {
            used += b;
            admitted.push(p);
        }
    }
    admitted
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
    /// Text typed in the panel's search box.
    pub search_input: String,
    /// Board part last cross-probed, so a panel search is not overridden by the selection.
    pub probed_part: Option<usize>,
    pub search_results: Vec<SchematicOccurrence>,
    pub selected_result_idx: usize,
    pub zoom: f32,
    pub pan_offset: Vec2,
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

    // The current results are substring matches because no indexed page had a whole-word match
    search_is_fallback: bool,

    // Whole-page rasters (or backdrops of tiled pages) cached by page number (1-based)
    page_textures: HashMap<usize, CachedPageTexture>,

    // Low-res previews by page number (1-based), prefetched in the background
    previews: HashMap<usize, CachedPageTexture>,

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
            search_input: String::new(),
            probed_part: None,
            search_results: Vec::new(),
            selected_result_idx: 0,
            zoom: 1.0,
            pan_offset: Vec2::ZERO,
            pending_center_on_match: false,
            page_texts: HashMap::new(),
            text_failed: HashSet::new(),
            text_slots: 1,
            search_jump_pending: false,
            search_is_fallback: false,
            page_textures: HashMap::new(),
            previews: HashMap::new(),
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
        self.previews.clear();
        self.tiles.clear();
        self.last_queue.clear();
        self.failed_pages.clear();
        self.settled_scale = 0.0;
        self.force_settle = true;
        self.active_search.clear();
        self.probed_part = None;
        self.search_results.clear();
        self.page_texts.clear();
        self.text_failed.clear();
        self.search_jump_pending = false;
        self.search_is_fallback = false;
        self.zoom = 1.0;
        self.pan_offset = Vec2::ZERO;

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

    /// Closes the document: drops its render pool, textures, text layers and search state,
    /// leaving the viewer as it is before any PDF was opened.
    pub fn close(&mut self) {
        *self = Self::default();
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
        // Nothing to wait for when the scale is the settled one: a jump to a page of another size
        // settles at once (force_settle), and its tiles must stay queued on the next frames.
        let settled = self.force_settle
            || self.settled_scale <= 0.0
            || (wanted - self.settled_scale).abs() <= wanted * 1e-4
            || now - self.last_scale_change_time > SETTLE_SECS;
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
        let page_req = (!page_ok && !self.failed_pages.contains(&page)).then(|| pixels_request(page - 1, page_scale, None, tiled));
        // With a preview on screen the visible tiles come first: each costs a fraction of the
        // whole-page backdrop (they clip the interpret) and together they cover the viewport.
        let tiles_first = tiled && self.previews.contains_key(&page);
        if !tiles_first {
            queue.extend(page_req);
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
            let viewport_px = viewport.width().max(viewport.height()) * ppp;
            let wanted_tiles = tiles::visible_tiles(pts, scale, max_side, norm, viewport_px);
            visible_keys.extend(wanted_tiles.iter().map(|t| tiles::TileKey::new(page, tag, *t)));
            if settled && !self.failed_pages.contains(&page) {
                for t in self.tiles.missing(page, tag, &wanted_tiles) {
                    queue.push(pixels_request(page - 1, scale, Some(t), false));
                }
            }
        }
        if tiles_first {
            queue.extend(page_req);
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
        self.retain_page_textures(page);

        // 4. Background work, held while the current page has no raster at all (a jump), as
        //    pdfcraft holds thumbnails and text: every worker goes to the page being looked at.
        let hold = !self.page_textures.contains_key(&page) && !self.failed_pages.contains(&page);
        let (result_previews, other_previews) = self.plan_previews();
        if !hold {
            // Previews of the search-result pages first (a match navigation jumps there), then
            // the text layers still to index, the current page first, then the other previews.
            queue.extend(result_previews.iter().take(PREVIEW_BATCH).copied());
            queue.extend(
                (0..self.total_pages)
                    .map(|k| (page - 1 + k) % self.total_pages)
                    .filter(|p| !self.page_texts.contains_key(p) && !self.text_failed.contains(p))
                    .take(self.text_slots)
                    .map(text_request),
            );
            let room = PREVIEW_BATCH.saturating_sub(result_previews.len());
            queue.extend(other_previews.iter().take(room).copied());
        }

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

    /// Whether a document is open (its tab may be hidden).
    pub fn is_loaded(&self) -> bool {
        self.doc_info.is_some()
    }

    /// Background work while the page is not painted (its tab hidden): collects finished
    /// results and keeps indexing the text layers still missing, without rendering any page, so
    /// a cross-probe from the board finds its hits and the tab opens at the first one.
    pub fn pump_background(&mut self, ctx: &egui::Context) {
        let Some(pool) = &self.render_pool else { return };
        let page = self.current_page.clamp(1, self.total_pages);
        // Replacing the paint queue cancels the rasters of a page nobody is looking at.
        let queue: Vec<RenderRequest> = (0..self.total_pages)
            .map(|k| (page - 1 + k) % self.total_pages)
            .filter(|p| !self.page_texts.contains_key(p) && !self.text_failed.contains(p))
            .take(self.text_slots)
            .map(text_request)
            .collect();
        if queue != self.last_queue {
            pool.set_queue(queue.clone());
            self.last_queue = queue;
            self.stats.queue_changes += 1;
        }
        let max_side = ctx.input(|i| i.max_texture_side) as f32;
        self.receive(ctx, max_side);
        if !self.last_queue.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
    }

    /// Keeps the current page and its neighbours, then the pages turned away from, nearest the
    /// current page first, within [`PAGE_BYTES`] and [`MAX_CACHED_PAGES`] (as pdfcraft does), so
    /// returning to a visited page (or match) shows its sharp raster at once.
    fn retain_page_textures(&mut self, page: usize) {
        let mut order: Vec<usize> = self.page_textures.keys().copied().collect();
        order.sort_by_key(|p| (p.abs_diff(page) > 1, p.abs_diff(page), *p));
        let bytes = |p: usize| self.page_textures.get(&p).map_or(0, |t| tiles::rgba_bytes(t.handle.size()[0], t.handle.size()[1]));
        let kept: HashSet<usize> = admit_by_budget(&order, bytes, PAGE_BYTES, MAX_CACHED_PAGES).into_iter().collect();
        self.page_textures.retain(|p, _| kept.contains(p) || p.abs_diff(page) <= 1);
    }

    /// Pages (1-based) whose preview is admitted, most wanted first: the search-result pages in
    /// result order, then every page by distance from the current one, within [`PREVIEW_BYTES`].
    pub fn preview_order(&self) -> Vec<usize> {
        let mut order: Vec<usize> = Vec::new();
        for hit in &self.search_results {
            if !order.contains(&hit.page_number) {
                order.push(hit.page_number);
            }
        }
        let cur = self.current_page;
        let mut rest: Vec<usize> = (1..=self.total_pages).filter(|p| !order.contains(p)).collect();
        rest.sort_by_key(|p| (p.abs_diff(cur), *p));
        order.extend(rest);
        let bytes = |p: usize| {
            let s = self.page_size_pts(p);
            let sc = preview_scale((s.x, s.y));
            tiles::rgba_bytes(device_pixels(s.x.max(1.0), sc) as usize, device_pixels(s.y.max(1.0), sc) as usize)
        };
        admit_by_budget(&order, bytes, PREVIEW_BYTES, usize::MAX)
    }

    /// Drops previews no longer admitted; returns the missing previews of search-result pages and
    /// of the other admitted pages, each most wanted first.
    fn plan_previews(&mut self) -> (Vec<RenderRequest>, Vec<RenderRequest>) {
        let admitted = self.preview_order();
        let keep: HashSet<usize> = admitted.iter().copied().collect();
        self.previews.retain(|p, _| keep.contains(p));
        let results: HashSet<usize> = self.search_results.iter().map(|o| o.page_number).collect();
        let (mut hits, mut others) = (Vec::new(), Vec::new());
        for p in admitted {
            if self.previews.contains_key(&p) || self.failed_pages.contains(&p) {
                continue;
            }
            let s = self.page_size_pts(p);
            let req = preview_request(p - 1, (s.x, s.y));
            if results.contains(&p) { hits.push(req) } else { others.push(req) }
        }
        (hits, others)
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
            let t0 = std::time::Instant::now();
            let image = texture_image([w, h], res.rgba);
            self.stats.rasters_used += 1;
            self.stats.raster_pixels += (w * h) as u64;
            self.stats.raster_millis += u64::from(res.millis);
            match res.request.tile {
                Some(t) => {
                    let handle = ctx.load_texture(format!("pdf_p{page_1}_t{tag}_{}_{}", t.x, t.y), image, TextureOptions::LINEAR);
                    self.tiles.insert(tiles::TileKey::new(page_1, tag, t), t, handle);
                }
                None => {
                    let handle = ctx.load_texture(format!("pdf_p{page_1}_t{tag}"), image, TextureOptions::LINEAR);
                    let cache = if tag & PREVIEW_TAG != 0 { &mut self.previews } else { &mut self.page_textures };
                    cache.insert(page_1, CachedPageTexture { handle, tag });
                }
            }
            self.stats.convert_micros += t0.elapsed().as_micros() as u64;
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
        let was_fallback = self.search_is_fallback;
        self.perform_instant_search(&query);
        if was_fallback && !self.search_is_fallback && !self.search_results.is_empty() {
            // A whole-word match just got indexed: the substring hit shown so far is dropped.
            self.search_jump_pending = false;
            self.select_match(0);
        } else if let Some(idx) = selected.and_then(|(page, range)| {
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
        // The sharp raster (or backdrop), else the low-res preview until it arrives.
        if let Some(t) = self.page_textures.get(&self.current_page).or_else(|| self.previews.get(&self.current_page)) {
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

        // Whole-word matches across the indexed pages ("U12", never "BU12"); substrings only
        // when no indexed page has a whole-word match.
        let texts = &self.page_texts;
        let (matches, fallback) = document_matches(self.total_pages, |p, whole| {
            texts.get(&p).map(|t| t.find_opts(trimmed, false, whole)).unwrap_or_default()
        });
        self.search_is_fallback = fallback;

        for (p_idx, range) in matches {
            let Some(text) = self.page_texts.get(&p_idx) else { continue };
            let page_1 = p_idx + 1;
            {
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

    /// Runs a free-text search typed in the PDF panel and jumps to its first hit, keeping the
    /// current zoom. Before indexing finishes, it jumps once the first hit arrives.
    pub fn search(&mut self, query: &str) -> bool {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            self.active_search.clear();
            self.search_results.clear();
            self.search_jump_pending = false;
            return false;
        }

        self.perform_instant_search(trimmed);
        self.selected_result_idx = 0;
        self.search_jump_pending = self.search_results.is_empty() && !self.text_indexed();
        if self.search_results.is_empty() {
            false
        } else {
            self.select_match(0);
            true
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

    /// Rendered words become the texture's pixels unchanged (premultiplied R, G, B, A bytes).
    #[test]
    fn test_texture_image_keeps_pixels() {
        let bytes: [[u8; 4]; 6] = [[255, 0, 0, 255], [0, 128, 0, 255], [0, 0, 64, 128], [10, 20, 30, 40], [0, 0, 0, 0], [255, 255, 255, 255]];
        let words: Vec<u32> = bytes.iter().map(|b| u32::from_ne_bytes(*b)).collect();
        let image = texture_image([3, 2], pdfcraft_render::Pixels::from(words));
        assert_eq!(image.size, [3, 2]);
        let want: Vec<Color32> = bytes.iter().map(|b| Color32::from_rgba_premultiplied(b[0], b[1], b[2], b[3])).collect();
        assert_eq!(image.pixels, want);
    }

    /// The renderer's buffer becomes the texture without a copy on the UI thread, as pdfcraft does.
    #[test]
    fn test_texture_image_reuses_buffer() {
        let words = vec![u32::from_ne_bytes([9, 8, 7, 255]); 64 * 64];
        let ptr = words.as_ptr() as usize;
        let image = texture_image([64, 64], pdfcraft_render::Pixels::from(words));
        assert_eq!(image.pixels.as_ptr() as usize, ptr, "pixels were copied");
    }

    /// A buffer that does not match the size still converts (the safe copying path).
    #[test]
    fn test_texture_image_mismatched_size_copies() {
        let words = vec![u32::from_ne_bytes([1, 2, 3, 4]); 2];
        let image = texture_image([1, 2], pdfcraft_render::Pixels::from(words));
        assert_eq!(image.pixels, vec![Color32::from_rgba_premultiplied(1, 2, 3, 4); 2]);
    }

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
        // No tile is larger than the viewport needs (800 x 600 px: 1024 px tiles).
        assert!(tiles.iter().all(|r| r.tile.is_some_and(|t| t.w <= 1024 && t.h <= 1024)), "{tiles:?}");
        let whole = tiles::visible_tiles((size.x, size.y), scale, 2048.0, [0.0, 0.0, 1.0, 1.0], f32::INFINITY);
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

    /// A search typed in the PDF panel jumps to its first hit without the cross-probe zoom, and
    /// a search issued before indexing finishes jumps once its hit arrives.
    #[test]
    fn test_panel_search_jumps_to_first_hit_keeping_zoom() {
        let Some((mut viewer, sim)) = open_fixture() else { return };
        assert!(!viewer.search("R200"), "nothing is indexed right after open");
        sim.wait_until(&mut viewer, |v| v.current_page == 34);
        assert_eq!(viewer.search_results[viewer.selected_result_idx].text, "R200");
        assert_eq!(viewer.zoom, 1.0);

        assert!(viewer.search("R201"));
        assert_eq!((viewer.current_page, viewer.search_results[0].text.as_str()), (34, "R201"));
        assert!(viewer.pending_center_on_match);

        assert!(!viewer.search("   "));
        assert!(viewer.search_results.is_empty() && viewer.active_search.is_empty());
    }

    /// Pages as (whole-word hits, substring hits) for `document_matches`.
    fn finder(pages: &[(Vec<Range<usize>>, Vec<Range<usize>>)]) -> impl FnMut(usize, bool) -> Vec<Range<usize>> + '_ {
        move |p, whole| if whole { pages[p].0.clone() } else { pages[p].1.clone() }
    }

    /// "C212" found as a whole word anywhere drops "BC212"/"MC212" substring hits on every page,
    /// including pages with no exact match.
    #[test]
    fn test_document_matches_prefers_whole_words_across_pages() {
        let pages = vec![
            (vec![], vec![3..7]),           // only "BC212"
            (vec![10..14], vec![10..14, 20..24]), // "C212" and "MC212"
            (vec![], vec![]),
        ];
        let (hits, fallback) = document_matches(pages.len(), finder(&pages));
        assert_eq!(hits, vec![(1, 10..14)]);
        assert!(!fallback);
    }

    /// With no whole-word hit in the whole document, substring hits are returned.
    #[test]
    fn test_document_matches_falls_back_to_substrings() {
        let pages = vec![(vec![], vec![3..7]), (vec![], vec![]), (vec![], vec![1..5, 8..12])];
        let (hits, fallback) = document_matches(pages.len(), finder(&pages));
        assert_eq!(hits, vec![(0, 3..7), (2, 1..5), (2, 8..12)]);
        assert!(fallback);
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

    /// Opening must not extract text: the first paint queues only the visible page (background
    /// work waits for it), then a bounded number of text layers behind the rasters, all through
    /// the pool's one shared parser.
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
        assert!(queue.iter().all(|r| r.kind == RequestKind::Pixels && r.tag & PREVIEW_TAG == 0), "{queue:?}");

        // Once the page is on screen, the background work follows.
        sim.wait_until(&mut viewer, |v| v.page_textures.contains_key(&1));
        let queue = viewer.last_queue.clone();
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
        // Page 1 and its prefetched neighbour, plus at most one preview per page.
        assert!(pool.page_interpretations as usize <= 2 + viewer.total_pages, "{pool:?}");
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

    /// With the PDF tab hidden nothing paints, yet the pump keeps indexing the text layers (and
    /// renders no page), so a board cross-probe finds its hits and the tab opens at the first one.
    #[test]
    fn test_background_pump_indexes_text_without_painting() {
        let Some((mut viewer, sim)) = open_fixture() else { return };
        assert!(!viewer.cross_probe("U12"), "nothing indexed yet");
        let t0 = std::time::Instant::now();
        while !viewer.text_indexed() {
            assert!(t0.elapsed() < std::time::Duration::from_secs(120), "pump made no progress");
            sim.pump(&mut viewer);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(viewer.last_queue.iter().all(|r| r.kind == RequestKind::Text), "{:?}", viewer.last_queue);
        assert!(viewer.page_textures.is_empty() && viewer.previews.is_empty(), "no page rendered");
        assert!(!viewer.search_results.is_empty());
        let hit = &viewer.search_results[viewer.selected_result_idx];
        assert_eq!(viewer.current_page, hit.page_number, "the pending cross-probe jumped to its hit");
        sim.pump(&mut viewer);
        assert!(viewer.last_queue.is_empty(), "nothing left to do once indexed");
        // Showing the tab paints the hit's page.
        sim.wait_until(&mut viewer, |v| v.page_textures.contains_key(&v.current_page));
    }

    #[test]
    fn test_admit_by_budget_skips_what_does_not_fit() {
        let sizes = [(1usize, 40usize), (2, 70), (3, 30), (4, 20)];
        let bytes = |p: usize| sizes.iter().find(|s| s.0 == p).unwrap().1;
        assert_eq!(admit_by_budget(&[1, 2, 3, 4], bytes, 100, usize::MAX), vec![1, 3, 4]);
        assert_eq!(admit_by_budget(&[2, 1, 3, 4], bytes, 100, 2), vec![2, 3]);
        assert!(admit_by_budget(&[1], bytes, 10, 5).is_empty());
    }

    /// Opens the fixture with every text layer indexed and every preview rendered.
    fn idle_fixture(ppp: f32) -> Option<(PdfSchematicViewer, FrameSim)> {
        let (mut viewer, _) = open_fixture()?;
        let sim = FrameSim::new(Rect::from_min_size(pos2(0.0, 60.0), vec2(860.0, 900.0)), ppp);
        sim.index_all_text(&mut viewer);
        sim.wait_until(&mut viewer, |v| v.last_queue.is_empty());
        Some((viewer, sim))
    }

    /// Every page gets a preview in the background (this document fits the allowance), and the
    /// search-result pages come first in the preview order.
    #[test]
    fn test_previews_prefetched_with_result_pages_first() {
        let Some((mut viewer, _sim)) = idle_fixture(1.0) else { return };
        assert_eq!(viewer.previews.len(), viewer.total_pages);
        viewer.perform_instant_search("U12");
        let mut pages: Vec<usize> = viewer.search_results.iter().map(|o| o.page_number).collect();
        pages.dedup();
        assert!(pages.len() > 1 && !pages.contains(&1), "{pages:?}");
        assert_eq!(viewer.preview_order()[..pages.len()], pages[..]);
    }

    /// A jump to a page with no sharp raster shows its preview on the very frame of the jump,
    /// and every worker goes to the target page: no text or preview is queued until it arrives.
    #[test]
    fn test_jump_paints_preview_at_once_and_holds_background_work() {
        let Some((mut viewer, sim)) = idle_fixture(1.0) else { return };
        assert!(viewer.cross_probe("U12"));
        let target = viewer.current_page;
        assert!(!viewer.page_textures.contains_key(&target), "target page not cached before the jump");
        assert!(sim.frame(&mut viewer), "the preview is painted on the jump frame");
        let queue = viewer.last_queue.clone();
        assert_eq!(queue.first().map(|r| (r.page, r.kind)), Some((target - 1, RequestKind::Pixels)));
        assert!(
            queue.iter().all(|r| r.kind == RequestKind::Pixels && r.tag & PREVIEW_TAG == 0 && r.page.abs_diff(target - 1) <= 1),
            "{queue:?}"
        );
    }

    /// A page visited earlier keeps its sharp raster: returning to it paints at once, unqueued.
    #[test]
    fn test_visited_page_keeps_sharp_raster() {
        let Some((mut viewer, sim)) = idle_fixture(1.0) else { return };
        viewer.perform_instant_search("U12");
        let last = viewer.search_results.len() - 1;
        viewer.select_match(0);
        let first_page = viewer.current_page;
        sim.wait_until(&mut viewer, |v| v.last_queue.is_empty());
        viewer.select_match(last);
        assert!(viewer.current_page.abs_diff(first_page) > 1, "results far apart");
        sim.wait_until(&mut viewer, |v| v.last_queue.is_empty());
        viewer.select_match(0);
        assert!(viewer.page_textures.contains_key(&first_page), "visited page still cached");
        assert!(sim.frame(&mut viewer));
        assert!(viewer.last_queue.iter().all(|r| r.page != first_page - 1), "{:?}", viewer.last_queue);
    }

    /// A cross-probe that raises the zoom into tiles settles at once, and its tiles stay queued on
    /// the next frame instead of being cancelled until the settle timer runs out.
    #[test]
    fn test_cross_probe_into_tiles_keeps_tiles_queued() {
        let Some((mut viewer, sim)) = idle_fixture(2.0) else { return };
        assert!(viewer.cross_probe("U12"));
        let target = viewer.current_page;
        assert!(sim.frame(&mut viewer), "the preview is painted on the jump frame");
        let first = viewer.last_queue.clone();
        let tiles: Vec<_> = first.iter().filter(|r| r.page == target - 1 && r.tile.is_some()).copied().collect();
        assert!(!tiles.is_empty(), "zoom 3 at ppp 2 is tiled: {first:?}");
        // With a preview on screen the visible tiles precede the whole-page backdrop.
        let backdrop = first.iter().position(|r| r.page == target - 1 && r.tile.is_none());
        let first_tile = first.iter().position(|r| r.tile.is_some()).unwrap();
        assert!(backdrop.is_none_or(|b| first_tile < b), "{first:?}");
        sim.frame(&mut viewer);
        for r in tiles {
            let key = tiles::TileKey::new(target, r.tag, r.tile.unwrap());
            assert!(viewer.last_queue.contains(&r) || viewer.tiles.contains(&key), "{r:?} dropped");
        }
    }

    /// Drives the viewer like the PDF panel in `lib.rs` does: a `panel` rect, the page fitted at
    /// 95% and scaled by `zoom`, centred on the selected match when one is pending.
    /// One frame-driven fixture test at a time: each opens a pool of almost every core and renders
    /// the whole document in the background, and overlapping them in a debug build could push a
    /// page past the pool's stuck-render watchdog.
    static FIXTURE_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct FrameSim {
        ctx: egui::Context,
        panel: Rect,
        ppp: f32,
        start: std::time::Instant,
        _serial: std::sync::MutexGuard<'static, ()>,
    }

    impl FrameSim {
        fn new(panel: Rect, ppp: f32) -> Self {
            let serial = FIXTURE_SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            Self { ctx: egui::Context::default(), panel, ppp, start: std::time::Instant::now(), _serial: serial }
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

        /// One frame with the PDF tab hidden: only the background pump runs.
        fn pump(&self, viewer: &mut PdfSchematicViewer) {
            self.ctx.begin_pass(egui::RawInput {
                time: Some(self.start.elapsed().as_secs_f64()),
                max_texture_side: Some(8192),
                ..Default::default()
            });
            viewer.pump_background(&self.ctx);
            self.ctx.end_pass().textures_delta.clear();
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

        /// Runs frames until `done` (generous bound: not a timing check).
        fn wait_until(&self, viewer: &mut PdfSchematicViewer, done: impl Fn(&PdfSchematicViewer) -> bool) {
            let t0 = std::time::Instant::now();
            loop {
                self.frame(viewer);
                if done(viewer) {
                    return;
                }
                assert!(t0.elapsed() < std::time::Duration::from_secs(120), "viewer stalled");
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
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

    /// User CPU plus system CPU seconds of this process so far (Linux `/proc/self/stat`).
    fn process_cpu_secs() -> Option<f64> {
        let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
        // Fields after the parenthesised command name; utime and stime are fields 14 and 15.
        let rest = &stat[stat.rfind(')')? + 2..];
        let f: Vec<&str> = rest.split_whitespace().collect();
        let ticks: f64 = f.get(11)?.parse::<f64>().ok()? + f.get(12)?.parse::<f64>().ok()?;
        Some(ticks / 100.0)
    }

    /// Measures a panel search jump at deep zoom on the fixture (prints numbers): the time until
    /// the backdrop and every visible tile of the hit's page are on screen, the tiles requested,
    /// worker raster time, UI-thread texture conversion time and process CPU time.
    /// `BVC_ZOOM` (default 18), `BVC_QUERY` (default R201) and `BVC_PPP` (default 1) vary it.
    /// `cargo test -p bvc-ui-egui --release bench_deep_zoom_search -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn bench_deep_zoom_search() {
        let env = |k: &str| std::env::var(k).ok();
        let ppp: f32 = env("BVC_PPP").and_then(|v| v.parse().ok()).unwrap_or(1.0);
        let zoom: f32 = env("BVC_ZOOM").and_then(|v| v.parse().ok()).unwrap_or(18.0);
        let query = env("BVC_QUERY").unwrap_or_else(|| "R201".to_string());
        let panel = env("BVC_PANEL")
            .and_then(|v| v.split_once('x').and_then(|(w, h)| Some(vec2(w.parse().ok()?, h.parse().ok()?))))
            .unwrap_or(vec2(860.0, 900.0));
        let path = fixture_path();
        if !path.exists() {
            return;
        }
        let mut viewer = PdfSchematicViewer::default();
        viewer.open_file(&path).expect("fixture opens");
        let sim = FrameSim::new(Rect::from_min_size(pos2(0.0, 60.0), panel), ppp);
        let limit = std::time::Duration::from_secs(120);
        // BVC_EARLY=1 searches as soon as the first page shows, while text and previews are
        // still being prepared in the background; otherwise after everything is idle.
        if env("BVC_EARLY").is_some() {
            sim.wait_until(&mut viewer, |v| v.page_textures.contains_key(&1));
        } else {
            sim.run_until_idle(&mut viewer, limit);
            sim.index_all_text(&mut viewer);
            sim.run_until_idle(&mut viewer, limit);
        }

        viewer.zoom = zoom;
        let before = viewer.stats();
        let pool_before = viewer.render_pool.as_ref().map(RenderPool::stats).unwrap_or_default();
        let cpu0 = process_cpu_secs().unwrap_or(0.0);
        let t = std::time::Instant::now();
        if !viewer.search(&query) {
            sim.wait_until(&mut viewer, |v| !v.search_results.is_empty());
        }
        let found_at = t.elapsed().as_secs_f64();
        let page = viewer.current_page;
        let mut tiles_seen: HashSet<(u64, u32, u32, u32, u32)> = HashSet::new();
        let (mut backdrop_at, mut sharp_at, mut idle_at) = (None, None, None);
        let mut ui_frame_max = 0.0f64;
        while t.elapsed() < limit {
            let f0 = std::time::Instant::now();
            sim.frame(&mut viewer);
            ui_frame_max = ui_frame_max.max(f0.elapsed().as_secs_f64());
            let now = t.elapsed().as_secs_f64();
            for r in viewer.last_queue.iter().filter(|r| r.page == page - 1) {
                if let Some(tl) = r.tile {
                    tiles_seen.insert((r.tag, tl.x, tl.y, tl.w, tl.h));
                }
            }
            if backdrop_at.is_none() && viewer.page_textures.contains_key(&page) {
                backdrop_at = Some(now);
            }
            let tiles_pending = viewer.last_queue.iter().any(|r| r.page == page - 1 && r.tile.is_some());
            if sharp_at.is_none() && !tiles_seen.is_empty() && !tiles_pending {
                sharp_at = Some(now);
            }
            if sharp_at.is_some() && viewer.last_queue.is_empty() {
                idle_at = Some(now);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
        let cpu = process_cpu_secs().unwrap_or(0.0) - cpu0;
        let s = viewer.stats();
        let pool = viewer.render_pool.as_ref().map(RenderPool::stats).unwrap_or_default();
        let size = viewer.page_size_pts(page);
        let scale = viewer.settled_scale;
        let (dw, dh) = (device_pixels(size.x, scale), device_pixels(size.y, scale));
        let side = tiles_seen.iter().map(|t| t.3.max(t.4)).max().unwrap_or(0);
        let tile_px: u64 = tiles_seen.iter().map(|t| u64::from(t.3) * u64::from(t.4)).sum();
        println!(
            "deep zoom {zoom} ppp {ppp} '{query}' -> page {page} ({}x{} pt) scale {scale:.2} px/pt page {dw}x{dh} px, viewport {:.0} px",
            size.x, size.y, panel.x * panel.y * ppp * ppp
        );
        println!(
            "  found {found_at:.3}s backdrop {backdrop_at:?}s sharp {sharp_at:?}s idle {idle_at:?}s; tiles requested {} (max side {side}, {:.1} Mpx)",
            tiles_seen.len(),
            tile_px as f64 / 1e6
        );
        let used = s.rasters_used - before.rasters_used;
        println!(
            "  rasters used {used} ({:.1} Mpx), worker raster {} ms (avg {} ms), UI convert {:.1} ms, max frame {:.1} ms, dropped {}",
            (s.raster_pixels - before.raster_pixels) as f64 / 1e6,
            s.raster_millis - before.raster_millis,
            (s.raster_millis - before.raster_millis) / used.max(1),
            (s.convert_micros - before.convert_micros) as f64 / 1e3,
            ui_frame_max * 1e3,
            s.results_dropped - before.results_dropped
        );
        println!(
            "  process CPU {cpu:.2}s; pool interprets {} rasterizations {} tile requests {} text {}",
            pool.page_interpretations - pool_before.page_interpretations,
            pool.rasterizations - pool_before.rasterizations,
            pool.tile_requests - pool_before.tile_requests,
            pool.text_interpretations - pool_before.text_interpretations
        );
    }
}
