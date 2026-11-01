//! Tabs of the central area (Top, Bottom, Both, PDF) and the layout of the Both tab.

use crate::canvas::Rotation;
use crate::i18n::{self, Language};
use bvc_core::Side;
use egui::Rect;
use serde::{Deserialize, Serialize};

/// Tab shown in the central area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BoardTab {
    #[default]
    Top,
    Bottom,
    Both,
    Pdf,
}

impl BoardTab {
    /// Tabs in display order.
    pub const ALL: [BoardTab; 4] = [BoardTab::Top, BoardTab::Bottom, BoardTab::Both, BoardTab::Pdf];

    pub fn label(self, lang: Language) -> &'static str {
        match self {
            BoardTab::Top => i18n::tr(lang, "tabs.top"),
            BoardTab::Bottom => i18n::tr(lang, "tabs.bottom"),
            BoardTab::Both => i18n::tr(lang, "tabs.both"),
            BoardTab::Pdf => i18n::tr(lang, "tabs.pdf"),
        }
    }

    /// Board sides whose canvases this tab shows.
    pub fn sides(self) -> &'static [Side] {
        match self {
            BoardTab::Top => &[Side::Top],
            BoardTab::Bottom => &[Side::Bottom],
            BoardTab::Both => &[Side::Top, Side::Bottom],
            BoardTab::Pdf => &[],
        }
    }

    /// The single-side tab of `side` (`Both` maps to Top).
    pub fn for_side(side: Side) -> Self {
        if side == Side::Bottom { BoardTab::Bottom } else { BoardTab::Top }
    }

    /// Space: Top <-> Bottom; from Both or PDF, Top.
    pub fn flipped(self) -> Self {
        if self == BoardTab::Top { BoardTab::Bottom } else { BoardTab::Top }
    }

    /// The tab restored at startup: the PDF tab needs a document, which is not open yet.
    pub fn at_startup(self, pdf_loaded: bool) -> Self {
        if self == BoardTab::Pdf && !pdf_loaded { BoardTab::Top } else { self }
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

/// Rectangles of the top and bottom canvases of the Both tab inside `area`: two halves split
/// along the resolved layout, `BOTH_GAP` apart.
pub fn both_rects(area: Rect, layout: BothLayout, bbox: Option<bvc_core::Rect>, rotation: Rotation) -> (Rect, Rect) {
    let half_gap = BOTH_GAP * 0.5;
    match layout.resolve(bbox, rotation) {
        BothLayout::Horizontal => {
            let mid = area.center().x;
            let top = Rect::from_min_max(area.min, egui::pos2((mid - half_gap).max(area.min.x), area.max.y));
            let bottom = Rect::from_min_max(egui::pos2((mid + half_gap).min(area.max.x), area.min.y), area.max);
            (top, bottom)
        }
        _ => {
            let mid = area.center().y;
            let top = Rect::from_min_max(area.min, egui::pos2(area.max.x, (mid - half_gap).max(area.min.y)));
            let bottom = Rect::from_min_max(egui::pos2(area.min.x, (mid + half_gap).min(area.max.y)), area.max);
            (top, bottom)
        }
    }
}

/// The canvases `tab` shows inside `area`, as `(side, rect)` (none for the PDF tab).
pub fn canvas_rects(
    tab: BoardTab,
    area: Rect,
    layout: BothLayout,
    bbox: Option<bvc_core::Rect>,
    rotation: Rotation,
) -> Vec<(Side, Rect)> {
    match tab {
        BoardTab::Top => vec![(Side::Top, area)],
        BoardTab::Bottom => vec![(Side::Bottom, area)],
        BoardTab::Both => {
            let (top, bottom) = both_rects(area, layout, bbox, rotation);
            vec![(Side::Top, top), (Side::Bottom, bottom)]
        }
        BoardTab::Pdf => Vec::new(),
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
        let r = |tab| canvas_rects(tab, area(), BothLayout::Auto, bbox, Rotation::Deg0);
        assert_eq!(r(BoardTab::Top), vec![(Side::Top, area())]);
        assert_eq!(r(BoardTab::Bottom), vec![(Side::Bottom, area())]);
        let both = r(BoardTab::Both);
        assert_eq!(both.iter().map(|(s, _)| *s).collect::<Vec<_>>(), [Side::Top, Side::Bottom]);
        assert!(r(BoardTab::Pdf).is_empty());
        for tab in BoardTab::ALL {
            assert_eq!(r(tab).iter().map(|(s, _)| *s).collect::<Vec<_>>(), tab.sides());
        }
    }

    #[test]
    fn space_and_startup_tab_rules() {
        assert_eq!(BoardTab::Top.flipped(), BoardTab::Bottom);
        assert_eq!(BoardTab::Bottom.flipped(), BoardTab::Top);
        assert_eq!(BoardTab::Both.flipped(), BoardTab::Top);
        assert_eq!(BoardTab::Pdf.flipped(), BoardTab::Top);
        assert_eq!(BoardTab::for_side(Side::Bottom), BoardTab::Bottom);
        assert_eq!(BoardTab::for_side(Side::Both), BoardTab::Top);
        assert_eq!(BoardTab::Pdf.at_startup(false), BoardTab::Top);
        assert_eq!(BoardTab::Pdf.at_startup(true), BoardTab::Pdf);
        assert_eq!(BoardTab::Both.at_startup(false), BoardTab::Both);
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
