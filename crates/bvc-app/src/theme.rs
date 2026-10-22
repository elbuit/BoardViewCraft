//! Design tokens and theme styling matching cadcraft aesthetic.

use egui::Color32;

#[derive(Clone, Copy, Debug, PartialEq)]
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
}
