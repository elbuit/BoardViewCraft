//! Tabs of the central area (Top, Bottom, Both, PDF) and the layout of the Both tab.

use crate::canvas::Rotation;
use crate::i18n::{self, Language};
use bvc_core::Side;
use egui::Rect;
use serde::{Deserialize, Serialize};

/// What to show in a central view pane: Top, Bottom, Both sides together, Schematic PDF, or CAD Schematic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PaneContent {
    #[default]
    Top,
    Bottom,
    Both,
    Pdf,
    CadSchematic,
}

impl PaneContent {
    pub const ALL: [PaneContent; 5] = [
        PaneContent::Top,
        PaneContent::Bottom,
        PaneContent::Both,
        PaneContent::Pdf,
        PaneContent::CadSchematic,
    ];

    pub fn label(self, lang: Language) -> &'static str {
        match self {
            PaneContent::Top => i18n::tr(lang, "tabs.top"),
            PaneContent::Bottom => i18n::tr(lang, "tabs.bottom"),
            PaneContent::Both => i18n::tr(lang, "tabs.both"),
            PaneContent::Pdf => i18n::tr(lang, "tabs.pdf"),
            PaneContent::CadSchematic => i18n::tr(lang, "tabs.cad_schematic"),
        }
    }

    /// Board sides shown by this pane.
    pub fn sides(self) -> &'static [Side] {
        match self {
            PaneContent::Top => &[Side::Top],
            PaneContent::Bottom => &[Side::Bottom],
            PaneContent::Both => &[Side::Top, Side::Bottom],
            PaneContent::Pdf | PaneContent::CadSchematic => &[],
        }
    }
}

/// Tab shown in the central area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BoardTab {
    #[default]
    Top,
    Bottom,
    Both,
    Pdf,
    Split,
}

impl BoardTab {
    /// Tabs in display order.
    pub const ALL: [BoardTab; 5] = [BoardTab::Top, BoardTab::Bottom, BoardTab::Both, BoardTab::Pdf, BoardTab::Split];

    pub fn label(self, lang: Language) -> &'static str {
        match self {
            BoardTab::Top => i18n::tr(lang, "tabs.top"),
            BoardTab::Bottom => i18n::tr(lang, "tabs.bottom"),
            BoardTab::Both => i18n::tr(lang, "tabs.both"),
            BoardTab::Pdf => i18n::tr(lang, "tabs.pdf"),
            BoardTab::Split => i18n::tr(lang, "tabs.split"),
        }
    }

    /// Board sides whose canvases this tab shows.
    pub fn sides(self) -> &'static [Side] {
        match self {
            BoardTab::Top => &[Side::Top],
            BoardTab::Bottom => &[Side::Bottom],
            BoardTab::Both => &[Side::Top, Side::Bottom],
            BoardTab::Pdf => &[],
            BoardTab::Split => &[Side::Top],
        }
    }

    /// The single-side tab of `side` (`Both` and `Split` map to Top).
    pub fn for_side(side: Side) -> Self {
        if side == Side::Bottom { BoardTab::Bottom } else { BoardTab::Top }
    }

    /// Space: Top <-> Bottom; from Both, PDF or Split, Top.
    pub fn flipped(self) -> Self {
        if self == BoardTab::Top { BoardTab::Bottom } else { BoardTab::Top }
    }

    /// The tab restored at startup: PDF and Split tabs need a document, which is not open yet.
    pub fn at_startup(self, pdf_loaded: bool) -> Self {
        if (self == BoardTab::Pdf || self == BoardTab::Split) && !pdf_loaded { BoardTab::Top } else { self }
    }
}

/// Arrangement of the two canvases in the Both tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BothLayout {
    /// Vertical when the rotated board is at least as wide as tall, otherwise horizontal.
    #[default]
    Auto,
    /// Top side above the bottom side.
    Vertical,
    /// Top side on the left, bottom side on the right.
    Horizontal,
}

impl BothLayout {
    /// Layouts in display order.
    pub const ALL: [BothLayout; 3] = [BothLayout::Auto, BothLayout::Vertical, BothLayout::Horizontal];

    pub fn label(self, lang: Language) -> &'static str {
        match self {
            BothLayout::Auto => i18n::tr(lang, "layout.auto"),
            BothLayout::Vertical => i18n::tr(lang, "layout.vertical"),
            BothLayout::Horizontal => i18n::tr(lang, "layout.horizontal"),
        }
    }

    /// `Vertical` or `Horizontal`: Auto decided by the board's on-screen aspect (after rotation).
    /// Without a board Auto stacks vertically.
    pub fn resolve(self, bbox: Option<bvc_core::Rect>, rotation: Rotation) -> BothLayout {
        match self {
            BothLayout::Auto => {
                let Some(bbox) = bbox else { return BothLayout::Vertical };
                let (w, h) = match rotation {
                    Rotation::Deg0 | Rotation::Deg180 => (bbox.width(), bbox.height()),
                    Rotation::Deg90 | Rotation::Deg270 => (bbox.height(), bbox.width()),
                };
                if w >= h { BothLayout::Vertical } else { BothLayout::Horizontal }
            }
            fixed => fixed,
        }
    }
}

/// Space between the two canvases of the Both tab, where a separator line is drawn.
pub const BOTH_GAP: f32 = 6.0;

/// Space between the two panes in split view, where a draggable separator or divider is drawn.
pub const SPLIT_GAP: f32 = 6.0;

/// Rectangles of the board area and PDF area in Split tab inside `area`.
/// `split_ratio` is between 0.1 and 0.9 (default ~0.5), representing board's share of width/height.
pub fn split_rects(area: Rect, horizontal: bool, split_ratio: f32) -> (Rect, Rect) {
    let ratio = split_ratio.clamp(0.15, 0.85);
    let half_gap = SPLIT_GAP * 0.5;
    if horizontal {
        // Left is Board, Right is PDF
        let split_x = area.min.x + area.width() * ratio;
        let board_rect = Rect::from_min_max(area.min, egui::pos2((split_x - half_gap).max(area.min.x), area.max.y));
        let pdf_rect = Rect::from_min_max(egui::pos2((split_x + half_gap).min(area.max.x), area.min.y), area.max);
        (board_rect, pdf_rect)
    } else {
        // Top is Board, Bottom is PDF
        let split_y = area.min.y + area.height() * ratio;
        let board_rect = Rect::from_min_max(area.min, egui::pos2(area.max.x, (split_y - half_gap).max(area.min.y)));
        let pdf_rect = Rect::from_min_max(egui::pos2(area.min.x, (split_y + half_gap).min(area.max.y)), area.max);
        (board_rect, pdf_rect)
    }
}

/// Rectangles of the top and bottom canvases when `Both` is displayed inside `area`.
/// If `horizontal_layout` is true, top is left, bottom is right.
/// If false, top is above, bottom is below.
pub fn both_rects_oriented(area: Rect, horizontal_layout: bool) -> (Rect, Rect) {
    let half_gap = BOTH_GAP * 0.5;
    if horizontal_layout {
        let mid = area.center().x;
        let top = Rect::from_min_max(area.min, egui::pos2((mid - half_gap).max(area.min.x), area.max.y));
        let bottom = Rect::from_min_max(egui::pos2((mid + half_gap).min(area.max.x), area.min.y), area.max);
        (top, bottom)
    } else {
        let mid = area.center().y;
        let top = Rect::from_min_max(area.min, egui::pos2(area.max.x, (mid - half_gap).max(area.min.y)));
        let bottom = Rect::from_min_max(egui::pos2(area.min.x, (mid + half_gap).min(area.max.y)), area.max);
        (top, bottom)
    }
}

/// Rectangles of the top and bottom canvases of the Both tab inside `area`: two halves split
/// along the resolved layout, `BOTH_GAP` apart.
pub fn both_rects(area: Rect, layout: BothLayout, bbox: Option<bvc_core::Rect>, rotation: Rotation) -> (Rect, Rect) {
    let is_horizontal = layout.resolve(bbox, rotation) == BothLayout::Horizontal;
    both_rects_oriented(area, is_horizontal)
}

/// Returns the board canvases shown for a pane with content `content` inside `area`.
/// `split_horizontal`: When inside a 2-division view, indicates whether the division was horizontal.
/// If `Some(true)` (split horizontal), `Both` will arrange boards vertically (opposite: top above bottom).
/// If `Some(false)` (split vertical), `Both` will arrange boards horizontally (opposite: top left, bottom right).
/// If `None` (1 pane view), `Both` will use `layout.resolve(bbox, rotation)`.
pub fn pane_canvas_rects(
    content: PaneContent,
    area: Rect,
    split_horizontal: Option<bool>,
    layout: BothLayout,
    bbox: Option<bvc_core::Rect>,
    rotation: Rotation,
) -> Vec<(Side, Rect)> {
    match content {
        PaneContent::Top => vec![(Side::Top, area)],
        PaneContent::Bottom => vec![(Side::Bottom, area)],
        PaneContent::Both => {
            let (top, bottom) = match split_horizontal {
                Some(true) => both_rects_oriented(area, false), // split horizontal -> plates vertical
                Some(false) => both_rects_oriented(area, true), // split vertical -> plates horizontal
                None => both_rects(area, layout, bbox, rotation),
            };
            vec![(Side::Top, top), (Side::Bottom, bottom)]
        }
        PaneContent::Pdf | PaneContent::CadSchematic => Vec::new(),
    }
}

/// The canvases `tab` shows inside `area`, as `(side, rect)` (none for the PDF tab).
#[allow(dead_code)]
pub fn canvas_rects(
    tab: BoardTab,
    area: Rect,
    layout: BothLayout,
    bbox: Option<bvc_core::Rect>,
    rotation: Rotation,
    split_side: Side,
) -> Vec<(Side, Rect)> {
    match tab {
        BoardTab::Top => vec![(Side::Top, area)],
        BoardTab::Bottom => vec![(Side::Bottom, area)],
        BoardTab::Both => {
            let (top, bottom) = both_rects(area, layout, bbox, rotation);
            vec![(Side::Top, top), (Side::Bottom, bottom)]
        }
        BoardTab::Pdf => Vec::new(),
        BoardTab::Split => {
            let side = if split_side == Side::Bottom { Side::Bottom } else { Side::Top };
            vec![(side, area)]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2};

    fn wide_board() -> bvc_core::Rect {
        bvc_core::Rect { min_x: 0.0, min_y: 0.0, max_x: 4000.0, max_y: 2000.0 }
    }

    fn tall_board() -> bvc_core::Rect {
        bvc_core::Rect { min_x: 0.0, min_y: 0.0, max_x: 1500.0, max_y: 3000.0 }
    }

    fn area() -> Rect {
        Rect::from_min_size(pos2(10.0, 40.0), vec2(1200.0, 800.0))
    }

    fn assert_separate_inside(top: Rect, bottom: Rect, area: Rect) {
        assert!(area.contains_rect(top) && area.contains_rect(bottom), "{top:?} {bottom:?} in {area:?}");
        assert!(top.is_positive() && bottom.is_positive());
        let overlap = top.intersect(bottom);
        assert!(!overlap.is_positive(), "{top:?} overlaps {bottom:?}");
    }

    #[test]
    fn auto_stacks_a_wide_board_and_puts_a_tall_one_side_by_side() {
        let (top, bottom) = both_rects(area(), BothLayout::Auto, Some(wide_board()), Rotation::Deg0);
        assert!(top.max.y <= bottom.min.y, "top above bottom");
        assert_eq!((top.width(), bottom.width()), (1200.0, 1200.0));
        assert!((bottom.min.y - top.max.y - BOTH_GAP).abs() < 1e-3);
        assert_separate_inside(top, bottom, area());

        let (top, bottom) = both_rects(area(), BothLayout::Auto, Some(tall_board()), Rotation::Deg180);
        assert!(top.max.x <= bottom.min.x, "top left of bottom");
        assert_eq!((top.height(), bottom.height()), (800.0, 800.0));
        assert_separate_inside(top, bottom, area());
    }

    #[test]
    fn auto_follows_the_rotated_aspect() {
        for rotation in [Rotation::Deg90, Rotation::Deg270] {
            assert_eq!(BothLayout::Auto.resolve(Some(wide_board()), rotation), BothLayout::Horizontal);
            assert_eq!(BothLayout::Auto.resolve(Some(tall_board()), rotation), BothLayout::Vertical);
        }
        let square = bvc_core::Rect { min_x: 0.0, min_y: 0.0, max_x: 100.0, max_y: 100.0 };
        assert_eq!(BothLayout::Auto.resolve(Some(square), Rotation::Deg0), BothLayout::Vertical);
        assert_eq!(BothLayout::Auto.resolve(None, Rotation::Deg90), BothLayout::Vertical);
    }

    #[test]
    fn fixed_layouts_ignore_the_board_aspect() {
        let (top, bottom) = both_rects(area(), BothLayout::Horizontal, Some(wide_board()), Rotation::Deg0);
        assert!(top.max.x <= bottom.min.x);
        assert_separate_inside(top, bottom, area());
        let (top, bottom) = both_rects(area(), BothLayout::Vertical, Some(tall_board()), Rotation::Deg0);
        assert!(top.max.y <= bottom.min.y);
        assert_separate_inside(top, bottom, area());
    }

    #[test]
    fn each_tab_shows_only_its_canvases() {
        let bbox = Some(wide_board());
        let r = |tab| canvas_rects(tab, area(), BothLayout::Auto, bbox, Rotation::Deg0, Side::Top);
        assert_eq!(r(BoardTab::Top), vec![(Side::Top, area())]);
        assert_eq!(r(BoardTab::Bottom), vec![(Side::Bottom, area())]);
        let both = r(BoardTab::Both);
        assert_eq!(both.iter().map(|(s, _)| *s).collect::<Vec<_>>(), [Side::Top, Side::Bottom]);
        assert!(r(BoardTab::Pdf).is_empty());
        for tab in BoardTab::ALL {
            assert_eq!(r(tab).iter().map(|(s, _)| *s).collect::<Vec<_>>(), tab.sides());
        }
        let split_bottom = canvas_rects(BoardTab::Split, area(), BothLayout::Auto, bbox, Rotation::Deg0, Side::Bottom);
        assert_eq!(split_bottom, vec![(Side::Bottom, area())]);
    }

    #[test]
    fn space_and_startup_tab_rules() {
        assert_eq!(BoardTab::Top.flipped(), BoardTab::Bottom);
        assert_eq!(BoardTab::Bottom.flipped(), BoardTab::Top);
        assert_eq!(BoardTab::Both.flipped(), BoardTab::Top);
        assert_eq!(BoardTab::Pdf.flipped(), BoardTab::Top);
        assert_eq!(BoardTab::Split.flipped(), BoardTab::Top);
        assert_eq!(BoardTab::for_side(Side::Bottom), BoardTab::Bottom);
        assert_eq!(BoardTab::for_side(Side::Both), BoardTab::Top);
        assert_eq!(BoardTab::Pdf.at_startup(false), BoardTab::Top);
        assert_eq!(BoardTab::Pdf.at_startup(true), BoardTab::Pdf);
        assert_eq!(BoardTab::Split.at_startup(false), BoardTab::Top);
        assert_eq!(BoardTab::Split.at_startup(true), BoardTab::Split);
        assert_eq!(BoardTab::Both.at_startup(false), BoardTab::Both);
    }

    #[test]
    fn split_rects_respect_gap_and_ratio() {
        let area = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 600.0));
        let (board_h, pdf_h) = split_rects(area, true, 0.5);
        assert!((board_h.width() - (500.0 - SPLIT_GAP * 0.5)).abs() < 1e-4);
        assert!((pdf_h.width() - (500.0 - SPLIT_GAP * 0.5)).abs() < 1e-4);
        assert!(pdf_h.min.x > board_h.max.x);

        let (board_v, pdf_v) = split_rects(area, false, 0.4);
        assert!((board_v.height() - (240.0 - SPLIT_GAP * 0.5)).abs() < 1e-4);
        assert!((pdf_v.height() - (360.0 - SPLIT_GAP * 0.5)).abs() < 1e-4);
        assert!(pdf_v.min.y > board_v.max.y);
    }

    #[test]
    fn tab_and_layout_labels_are_translated() {
        for &lang in &Language::ALL {
            for tab in BoardTab::ALL {
                assert!(!tab.label(lang).starts_with("tabs."), "{lang:?} {tab:?}");
            }
            for layout in BothLayout::ALL {
                assert!(!layout.label(lang).starts_with("layout."), "{lang:?} {layout:?}");
            }
        }
        assert_eq!(BoardTab::Both.label(Language::English), "Both");
    }
}
