//! Design tokens and theme styling matching cadcraft aesthetic.

use egui::Color32;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemePreset {
    #[default]
    CadcraftDark,
    #[serde(alias = "OpenBoardViewClassic")]
    WarmMode,
    SweetBlue,
    HighContrast,
    Custom,
}

impl ThemePreset {
    pub const ALL: [ThemePreset; 5] = [
        ThemePreset::CadcraftDark,
        ThemePreset::WarmMode,
        ThemePreset::SweetBlue,
        ThemePreset::HighContrast,
        ThemePreset::Custom,
    ];

    pub fn display_name(&self) -> &'static str {
        match self {
            ThemePreset::CadcraftDark => "Cadcraft Dark",
            ThemePreset::WarmMode => "Warm Mode",
            ThemePreset::SweetBlue => "Sweet Blue",
            ThemePreset::HighContrast => "High Contrast",
            ThemePreset::Custom => "Custom...",
        }
    }

    pub fn tokens(&self) -> Tokens {
        match self {
            ThemePreset::CadcraftDark => Tokens::DARK,
            ThemePreset::WarmMode => Tokens::WARM_MODE,
            ThemePreset::SweetBlue => Tokens::SWEET_BLUE,
            ThemePreset::HighContrast => Tokens::HIGH_CONTRAST,
            ThemePreset::Custom => Tokens::DARK,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tokens {
    pub chrome: Color32,
    pub chrome_dark: Color32,
    pub panel: Color32,
    pub panel_header: Color32,
    pub tab_active: Color32,
    pub control: Color32,
    pub control_hover: Color32,
    pub border: Color32,
    pub canvas: Color32,
    pub grid_minor: Color32,
    pub grid_major: Color32,
    pub axis_x: Color32,
    pub axis_y: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_faint: Color32,
    pub accent: Color32,
    pub selection: Color32,
    pub hover: Color32,

    // Board specific styling
    pub board_outline: Color32,
    pub board_fill: Color32,
    pub part_body: Color32,
    pub part_text: Color32,
    pub pin_default: Color32,
    pub pin_unconnected: Color32,
    pub pin_highlight_cur_side: Color32,
    pub pin_highlight_opp_side: Color32,
    pub via_cur_side: Color32,
    pub via_opp_side: Color32,
    pub via_highlight: Color32,
    pub net_web: Color32,
    pub status_bar_bg: Color32,
    pub status_bar_text: Color32,
}

impl Tokens {
    pub const DARK: Tokens = Tokens {
        chrome: Color32::from_rgb(0x3b, 0x44, 0x53),
        chrome_dark: Color32::from_rgb(0x31, 0x38, 0x44),
        panel: Color32::from_rgb(0x28, 0x2e, 0x38),
        panel_header: Color32::from_rgb(0x22, 0x27, 0x30),
        tab_active: Color32::from_rgb(0x4e, 0x5a, 0x6e),
        control: Color32::from_rgb(0x3d, 0x46, 0x56),
        control_hover: Color32::from_rgb(0x4c, 0x58, 0x6c),
        border: Color32::from_rgb(0x1c, 0x21, 0x28),
        canvas: Color32::from_rgb(0x1a, 0x1e, 0x24),
        grid_minor: Color32::from_rgb(0x22, 0x28, 0x32),
        grid_major: Color32::from_rgb(0x2b, 0x33, 0x40),
        axis_x: Color32::from_rgb(0x70, 0x30, 0x30),
        axis_y: Color32::from_rgb(0x30, 0x60, 0x35),
        text: Color32::from_rgb(0xdd, 0xe2, 0xe8),
        text_dim: Color32::from_rgb(0x9a, 0xa3, 0xb0),
        text_faint: Color32::from_rgb(0x6c, 0x75, 0x82),
        accent: Color32::from_rgb(0x3d, 0x8b, 0xfd),
        selection: Color32::from_rgb(0xff, 0xc8, 0x32), // Gold/Amber highlight for selected pin
        hover: Color32::from_rgb(0x6b, 0xad, 0xff),

        // Board elements
        board_outline: Color32::from_rgb(0xb0, 0xb8, 0xc4),
        board_fill: Color32::from_rgba_premultiplied(0x1e, 0x26, 0x30, 0x80),
        part_body: Color32::from_rgba_premultiplied(0x38, 0x42, 0x52, 0x90),
        part_text: Color32::from_rgb(0xc0, 0xc6, 0xd0),
        pin_default: Color32::from_rgb(0xdc, 0x50, 0x40), // Reddish-coral for standard pins
        pin_unconnected: Color32::from_rgb(0x70, 0x78, 0x84), // Gray for unconnected pins
        pin_highlight_cur_side: Color32::from_rgb(0x32, 0xdd, 0x66), // Bright vivid emerald green for selected net on active side
        pin_highlight_opp_side: Color32::from_rgb(0x1b, 0x7d, 0xa8), // Deep cyan/blue for selected net on opposite side
        via_cur_side: Color32::from_rgb(0xe0, 0xa0, 0x30),
        via_opp_side: Color32::from_rgb(0x70, 0x55, 0x20),
        via_highlight: Color32::from_rgb(0x32, 0xdd, 0x66),
        net_web: Color32::from_rgba_premultiplied(0x32, 0xdd, 0x66, 0xcc), // Semi-transparent emerald green line for net web
        status_bar_bg: Color32::from_rgb(0x18, 0x1c, 0x22),
        status_bar_text: Color32::from_rgb(0xaa, 0xb2, 0xbd),
    };

    pub const WARM_MODE: Tokens = Tokens {
        chrome: Color32::from_rgb(0x28, 0x28, 0x28),
        chrome_dark: Color32::from_rgb(0x1c, 0x1c, 0x1c),
        panel: Color32::from_rgb(0x22, 0x22, 0x22),
        panel_header: Color32::from_rgb(0x18, 0x18, 0x18),
        tab_active: Color32::from_rgb(0x3a, 0x3a, 0x3a),
        control: Color32::from_rgb(0x33, 0x33, 0x33),
        control_hover: Color32::from_rgb(0x44, 0x44, 0x44),
        border: Color32::from_rgb(0x14, 0x14, 0x14),
        canvas: Color32::from_rgb(0x11, 0x11, 0x11), // charcoal black
        grid_minor: Color32::from_rgb(0x1c, 0x1c, 0x1c),
        grid_major: Color32::from_rgb(0x2a, 0x2a, 0x2a),
        axis_x: Color32::from_rgb(0x80, 0x20, 0x20),
        axis_y: Color32::from_rgb(0x20, 0x80, 0x20),
        text: Color32::from_rgb(0xee, 0xee, 0xee),
        text_dim: Color32::from_rgb(0xaa, 0xaa, 0xaa),
        text_faint: Color32::from_rgb(0x66, 0x66, 0x66),
        accent: Color32::from_rgb(0xff, 0xd0, 0x00), // bright yellow
        selection: Color32::from_rgb(0xff, 0xd0, 0x00), // bright yellow
        hover: Color32::from_rgb(0xff, 0xe0, 0x55),

        // Board elements
        board_outline: Color32::from_rgb(0xff, 0xff, 0xff), // crisp white
        board_fill: Color32::from_rgba_premultiplied(0x18, 0x18, 0x18, 0x40),
        part_body: Color32::from_rgba_premultiplied(0x40, 0x40, 0x40, 0x80),
        part_text: Color32::from_rgb(0xdd, 0xdd, 0xdd),
        pin_default: Color32::from_rgb(0xcc, 0x44, 0x33), // orange-red
        pin_unconnected: Color32::from_rgb(0x60, 0x60, 0x60),
        pin_highlight_cur_side: Color32::from_rgb(0xff, 0xd0, 0x00), // bright yellow/gold
        pin_highlight_opp_side: Color32::from_rgb(0x00, 0x88, 0xff), // royal blue
        via_cur_side: Color32::from_rgb(0xbb, 0x88, 0x22),
        via_opp_side: Color32::from_rgb(0x55, 0x44, 0x18),
        via_highlight: Color32::from_rgb(0xff, 0xd0, 0x00),
        net_web: Color32::from_rgba_premultiplied(0xff, 0xd0, 0x00, 0xb4), // rgba(255, 208, 0, 180)
        status_bar_bg: Color32::from_rgb(0x16, 0x16, 0x16),
        status_bar_text: Color32::from_rgb(0xaa, 0xaa, 0xaa),
    };

    pub const HIGH_CONTRAST: Tokens = Tokens {
        chrome: Color32::from_rgb(0x1a, 0x1a, 0x1a),
        chrome_dark: Color32::from_rgb(0x0a, 0x0a, 0x0a),
        panel: Color32::from_rgb(0x14, 0x14, 0x14),
        panel_header: Color32::from_rgb(0x08, 0x08, 0x08),
        tab_active: Color32::from_rgb(0x2a, 0x2a, 0x2a),
        control: Color32::from_rgb(0x26, 0x26, 0x26),
        control_hover: Color32::from_rgb(0x38, 0x38, 0x38),
        border: Color32::from_rgb(0x55, 0x55, 0x55),
        canvas: Color32::from_rgb(0x00, 0x00, 0x00), // true OLED black
        grid_minor: Color32::from_rgb(0x18, 0x18, 0x18),
        grid_major: Color32::from_rgb(0x33, 0x33, 0x33),
        axis_x: Color32::from_rgb(0xff, 0x00, 0x00),
        axis_y: Color32::from_rgb(0x00, 0xff, 0x00),
        text: Color32::from_rgb(0xff, 0xff, 0xff),
        text_dim: Color32::from_rgb(0xcc, 0xcc, 0xcc),
        text_faint: Color32::from_rgb(0x88, 0x88, 0x88),
        accent: Color32::from_rgb(0x00, 0xff, 0x66), // neon spring green
        selection: Color32::from_rgb(0xff, 0xff, 0x00), // electric yellow
        hover: Color32::from_rgb(0x00, 0xff, 0xff),

        // Board elements
        board_outline: Color32::from_rgb(0xff, 0xff, 0x00), // electric yellow
        board_fill: Color32::from_rgba_premultiplied(0x10, 0x10, 0x10, 0x60),
        part_body: Color32::from_rgba_premultiplied(0x60, 0x60, 0x60, 0x90),
        part_text: Color32::from_rgb(0xff, 0xff, 0xff),
        pin_default: Color32::from_rgb(0xff, 0x33, 0x33),
        pin_unconnected: Color32::from_rgb(0x55, 0x55, 0x55), // dark gray
        pin_highlight_cur_side: Color32::from_rgb(0x00, 0xff, 0x66), // neon spring green
        pin_highlight_opp_side: Color32::from_rgb(0xff, 0x00, 0xcc), // magenta
        via_cur_side: Color32::from_rgb(0xff, 0xaa, 0x00),
        via_opp_side: Color32::from_rgb(0x88, 0x55, 0x00),
        via_highlight: Color32::from_rgb(0x00, 0xff, 0x66),
        net_web: Color32::from_rgba_premultiplied(0x00, 0xff, 0x66, 0xdd),
        status_bar_bg: Color32::from_rgb(0x0c, 0x0c, 0x0c),
        status_bar_text: Color32::from_rgb(0xee, 0xee, 0xee),
    };

    pub const SWEET_BLUE: Tokens = Tokens {
        chrome: Color32::from_rgb(0x1e, 0x24, 0x30),
        chrome_dark: Color32::from_rgb(0x16, 0x1b, 0x24),
        panel: Color32::from_rgb(0x1a, 0x20, 0x2c),
        panel_header: Color32::from_rgb(0x13, 0x18, 0x22),
        tab_active: Color32::from_rgb(0x2d, 0x37, 0x48),
        control: Color32::from_rgb(0x27, 0x32, 0x44),
        control_hover: Color32::from_rgb(0x35, 0x43, 0x5a),
        border: Color32::from_rgb(0x12, 0x16, 0x1f),
        canvas: Color32::from_rgb(0x0f, 0x14, 0x1d), // deep navy pastel canvas
        grid_minor: Color32::from_rgb(0x17, 0x1f, 0x2c),
        grid_major: Color32::from_rgb(0x22, 0x2d, 0x3d),
        axis_x: Color32::from_rgb(0x7a, 0x38, 0x38),
        axis_y: Color32::from_rgb(0x30, 0x6a, 0x78),
        text: Color32::from_rgb(0xde, 0xe7, 0xf3),
        text_dim: Color32::from_rgb(0x93, 0xa5, 0xbe),
        text_faint: Color32::from_rgb(0x5c, 0x6e, 0x87),
        accent: Color32::from_rgb(0x58, 0xa6, 0xff), // agy soft pastel sky blue
        selection: Color32::from_rgb(0xff, 0x9e, 0x3b), // agy warm pastel orange
        hover: Color32::from_rgb(0x79, 0xc0, 0xff),

        // Board elements
        board_outline: Color32::from_rgb(0x79, 0xa8, 0xd8), // soft steel blue border
        board_fill: Color32::from_rgba_premultiplied(0x15, 0x20, 0x30, 0x90),
        part_body: Color32::from_rgba_premultiplied(0x2c, 0x3c, 0x54, 0x90),
        part_text: Color32::from_rgb(0xd0, 0xdf, 0xf0),
        pin_default: Color32::from_rgb(0x4d, 0x90, 0xcd), // pastel soft slate blue
        pin_unconnected: Color32::from_rgb(0x55, 0x64, 0x78),
        pin_highlight_cur_side: Color32::from_rgb(0xff, 0x9e, 0x3b), // agy warm pastel orange for selected net
        pin_highlight_opp_side: Color32::from_rgb(0x79, 0xc0, 0xff), // agy pastel cyan-blue
        via_cur_side: Color32::from_rgb(0xd9, 0x8a, 0x32), // soft warm amber
        via_opp_side: Color32::from_rgb(0x5a, 0x48, 0x2c),
        via_highlight: Color32::from_rgb(0xff, 0xb4, 0x54),
        net_web: Color32::from_rgba_premultiplied(0xff, 0x9e, 0x3b, 0xd0), // warm orange net web
        status_bar_bg: Color32::from_rgb(0x13, 0x18, 0x22),
        status_bar_text: Color32::from_rgb(0x93, 0xa5, 0xbe),
    };

    pub fn egui_visuals(&self, is_light: bool) -> egui::Visuals {
        let mut visuals = if is_light {
            egui::Visuals::light()
        } else {
            egui::Visuals::dark()
        };
        visuals.panel_fill = self.panel;
        visuals.window_fill = self.panel;
        visuals.widgets.noninteractive.bg_fill = self.panel;
        visuals.widgets.noninteractive.fg_stroke.color = self.text;
        visuals.widgets.inactive.bg_fill = self.control;
        visuals.widgets.inactive.fg_stroke.color = self.text;
        visuals.widgets.hovered.bg_fill = self.control_hover;
        visuals.widgets.hovered.fg_stroke.color = self.text;
        visuals.widgets.active.bg_fill = self.tab_active;
        visuals.widgets.active.fg_stroke.color = self.text;
        visuals
    }
}

#[cfg(test)]
/// Calculate relative luminance for WCAG AA contrast ratio calculations
fn relative_luminance(c: Color32) -> f64 {
    fn channel(v: u8) -> f64 {
        let s = v as f64 / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
}

#[cfg(test)]
/// Calculate WCAG contrast ratio between two colors
pub fn contrast_ratio(c1: Color32, c2: Color32) -> f64 {
    let l1 = relative_luminance(c1);
    let l2 = relative_luminance(c2);
    let lighter = l1.max(l2);
    let darker = l1.min(l2);
    (lighter + 0.05) / (darker + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_tokens_presets() {
        for preset in ThemePreset::ALL {
            let tokens = preset.tokens();
            // All 4 presets must instantiate with valid non-zero RGB components
            assert!(tokens.canvas.r() > 0 || tokens.canvas.g() > 0 || tokens.canvas.b() > 0 || preset == ThemePreset::HighContrast);
            assert!(tokens.accent.r() > 0 || tokens.accent.g() > 0 || tokens.accent.b() > 0);
            assert!(tokens.selection.r() > 0 || tokens.selection.g() > 0 || tokens.selection.b() > 0);
            assert!(tokens.pin_highlight_cur_side.r() > 0 || tokens.pin_highlight_cur_side.g() > 0 || tokens.pin_highlight_cur_side.b() > 0);
            assert!(tokens.board_outline.r() > 0 || tokens.board_outline.g() > 0 || tokens.board_outline.b() > 0);
        }
    }

    #[test]
    fn test_theme_contrast_ratios() {
        // Text color against panel background in all 4 themes satisfies minimum WCAG AA contrast ratio (>= 4.5:1)
        for preset in ThemePreset::ALL {
            let tokens = preset.tokens();
            let ratio = contrast_ratio(tokens.text, tokens.panel);
            assert!(
                ratio >= 4.5,
                "Theme {:?} failed WCAG AA text contrast: ratio was {:.2}:1, expected >= 4.5:1",
                preset,
                ratio
            );
        }
    }
}
