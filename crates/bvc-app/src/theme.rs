//! Design tokens and theme styling for BoardViewCraft.
//! Separated into Application UI theme (Light, Dark) and PCB Board theme (Light, Dark, Warm, Sweet Cold, Cadcraft).

use egui::Color32;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AppTheme {
    Light,
    #[default]
    Dark,
}

impl AppTheme {
    pub const ALL: [AppTheme; 2] = [AppTheme::Light, AppTheme::Dark];

    pub fn display_name(&self) -> &'static str {
        match self {
            AppTheme::Light => "Light",
            AppTheme::Dark => "Dark",
        }
    }

    pub fn tokens(&self) -> AppTokens {
        match self {
            AppTheme::Light => AppTokens::LIGHT,
            AppTheme::Dark => AppTokens::DARK,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PcbTheme {
    Light,
    Dark,
    Warm,
    #[serde(alias = "SweetBlue")]
    SweetCold,
    #[default]
    #[serde(alias = "CadcraftDark")]
    Cadcraft,
    Custom,
}

impl PcbTheme {
    pub const ALL: [PcbTheme; 6] = [
        PcbTheme::Cadcraft,
        PcbTheme::Dark,
        PcbTheme::Light,
        PcbTheme::Warm,
        PcbTheme::SweetCold,
        PcbTheme::Custom,
    ];

    pub fn display_name(&self) -> &'static str {
        match self {
            PcbTheme::Light => "Light",
            PcbTheme::Dark => "Dark",
            PcbTheme::Warm => "Warm",
            PcbTheme::SweetCold => "Sweet Cold",
            PcbTheme::Cadcraft => "Cadcraft",
            PcbTheme::Custom => "Custom...",
        }
    }

    pub fn tokens(&self) -> PcbTokens {
        match self {
            PcbTheme::Light => PcbTokens::LIGHT,
            PcbTheme::Dark => PcbTokens::DARK,
            PcbTheme::Warm => PcbTokens::WARM,
            PcbTheme::SweetCold => PcbTokens::SWEET_COLD,
            PcbTheme::Cadcraft => PcbTokens::CADCRAFT,
            PcbTheme::Custom => PcbTokens::CADCRAFT,
        }
    }
}


#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppTokens {
    pub chrome: Color32,
    pub chrome_dark: Color32,
    pub panel: Color32,
    pub panel_header: Color32,
    pub tab_active: Color32,
    pub control: Color32,
    pub control_hover: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_faint: Color32,
    pub accent: Color32,
    pub selection: Color32,
    pub hover: Color32,
    pub status_bar_bg: Color32,
    pub status_bar_text: Color32,
}

impl AppTokens {
    pub const DARK: AppTokens = AppTokens {
        chrome: Color32::from_rgb(0x3b, 0x44, 0x53),
        chrome_dark: Color32::from_rgb(0x31, 0x38, 0x44),
        panel: Color32::from_rgb(0x28, 0x2e, 0x38),
        panel_header: Color32::from_rgb(0x22, 0x27, 0x30),
        tab_active: Color32::from_rgb(0x4e, 0x5a, 0x6e),
        control: Color32::from_rgb(0x3d, 0x46, 0x56),
        control_hover: Color32::from_rgb(0x4c, 0x58, 0x6c),
        border: Color32::from_rgb(0x1c, 0x21, 0x28),
        text: Color32::from_rgb(0xdd, 0xe2, 0xe8),
        text_dim: Color32::from_rgb(0x9a, 0xa3, 0xb0),
        text_faint: Color32::from_rgb(0x6c, 0x75, 0x82),
        accent: Color32::from_rgb(0x3d, 0x8b, 0xfd),
        selection: Color32::from_rgb(0xff, 0xc8, 0x32),
        hover: Color32::from_rgb(0x6b, 0xad, 0xff),
        status_bar_bg: Color32::from_rgb(0x18, 0x1c, 0x22),
        status_bar_text: Color32::from_rgb(0xaa, 0xb2, 0xbd),
    };

    pub const LIGHT: AppTokens = AppTokens {
        chrome: Color32::from_rgb(0xe5, 0xe9, 0xf0),
        chrome_dark: Color32::from_rgb(0xd8, 0xde, 0xe9),
        panel: Color32::from_rgb(0xf2, 0xf4, 0xf8),
        panel_header: Color32::from_rgb(0xe8, 0xec, 0xf2),
        tab_active: Color32::from_rgb(0xd0, 0xd7, 0xe5),
        control: Color32::from_rgb(0xd8, 0xdf, 0xeb),
        control_hover: Color32::from_rgb(0xc8, 0xd2, 0xe2),
        border: Color32::from_rgb(0xc4, 0xcc, 0xd8),
        text: Color32::from_rgb(0x24, 0x29, 0x2f),
        text_dim: Color32::from_rgb(0x57, 0x60, 0x6a),
        text_faint: Color32::from_rgb(0x8c, 0x95, 0x9f),
        accent: Color32::from_rgb(0x09, 0x69, 0xda),
        selection: Color32::from_rgb(0xbf, 0x87, 0x00),
        hover: Color32::from_rgb(0x21, 0x8b, 0xff),
        status_bar_bg: Color32::from_rgb(0xe1, 0xe5, 0xee),
        status_bar_text: Color32::from_rgb(0x42, 0x4a, 0x53),
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

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PcbTokens {
    pub canvas: Color32,
    pub grid_minor: Color32,
    pub grid_major: Color32,
    pub axis_x: Color32,
    pub axis_y: Color32,
    pub text: Color32,
    pub selection: Color32,
    pub hover: Color32,

    // Board elements
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
}

impl PcbTokens {
    pub const CADCRAFT: PcbTokens = PcbTokens {
        canvas: Color32::from_rgb(0x1a, 0x1e, 0x24),
        grid_minor: Color32::from_rgb(0x22, 0x28, 0x32),
        grid_major: Color32::from_rgb(0x2b, 0x33, 0x40),
        axis_x: Color32::from_rgb(0x70, 0x30, 0x30),
        axis_y: Color32::from_rgb(0x30, 0x60, 0x35),
        text: Color32::from_rgb(0xdd, 0xe2, 0xe8),
        selection: Color32::from_rgb(0xff, 0xc8, 0x32), // Gold/Amber highlight for selected pin
        hover: Color32::from_rgb(0x6b, 0xad, 0xff),

        board_outline: Color32::from_rgb(0xb0, 0xb8, 0xc4),
        board_fill: Color32::from_rgba_premultiplied(0x1e, 0x26, 0x30, 0x80),
        part_body: Color32::from_rgba_premultiplied(0x38, 0x42, 0x52, 0x90),
        part_text: Color32::from_rgb(0xc0, 0xc6, 0xd0),
        pin_default: Color32::from_rgb(0xdc, 0x50, 0x40), // Reddish-coral for standard pins
        pin_unconnected: Color32::from_rgb(0x70, 0x78, 0x84), // Gray for unconnected pins
        pin_highlight_cur_side: Color32::from_rgb(0x32, 0xdd, 0x66), // Bright vivid emerald green
        pin_highlight_opp_side: Color32::from_rgb(0x1b, 0x7d, 0xa8), // Deep cyan/blue
        via_cur_side: Color32::from_rgb(0xe0, 0xa0, 0x30),
        via_opp_side: Color32::from_rgb(0x70, 0x55, 0x20),
        via_highlight: Color32::from_rgb(0x32, 0xdd, 0x66),
        net_web: Color32::from_rgba_premultiplied(0x32, 0xdd, 0x66, 0xcc),
    };

    pub const DARK: PcbTokens = PcbTokens {
        canvas: Color32::from_rgb(0x12, 0x14, 0x18),
        grid_minor: Color32::from_rgb(0x1b, 0x1f, 0x24),
        grid_major: Color32::from_rgb(0x24, 0x2b, 0x33),
        axis_x: Color32::from_rgb(0x8a, 0x2a, 0x2a),
        axis_y: Color32::from_rgb(0x2a, 0x7a, 0x35),
        text: Color32::from_rgb(0xf0, 0xf3, 0xf6),
        selection: Color32::from_rgb(0xff, 0xd0, 0x00),
        hover: Color32::from_rgb(0x58, 0xa6, 0xff),

        board_outline: Color32::from_rgb(0xc0, 0xc8, 0xd4),
        board_fill: Color32::from_rgba_premultiplied(0x18, 0x20, 0x28, 0x90),
        part_body: Color32::from_rgba_premultiplied(0x30, 0x3c, 0x4a, 0xa0),
        part_text: Color32::from_rgb(0xd5, 0xdc, 0xe5),
        pin_default: Color32::from_rgb(0xe0, 0x55, 0x45),
        pin_unconnected: Color32::from_rgb(0x65, 0x70, 0x80),
        pin_highlight_cur_side: Color32::from_rgb(0x2e, 0xe0, 0x60),
        pin_highlight_opp_side: Color32::from_rgb(0x20, 0x90, 0xc8),
        via_cur_side: Color32::from_rgb(0xeb, 0xaa, 0x30),
        via_opp_side: Color32::from_rgb(0x75, 0x58, 0x22),
        via_highlight: Color32::from_rgb(0x2e, 0xe0, 0x60),
        net_web: Color32::from_rgba_premultiplied(0x2e, 0xe0, 0x60, 0xd0),
    };

    pub const LIGHT: PcbTokens = PcbTokens {
        canvas: Color32::from_rgb(0xf6, 0xf8, 0xfa),
        grid_minor: Color32::from_rgb(0xe8, 0xeb, 0xf0),
        grid_major: Color32::from_rgb(0xd4, 0xd8, 0xe0),
        axis_x: Color32::from_rgb(0xd0, 0x50, 0x50),
        axis_y: Color32::from_rgb(0x40, 0x9a, 0x50),
        text: Color32::from_rgb(0x1f, 0x23, 0x28),
        selection: Color32::from_rgb(0xe3, 0x88, 0x00),
        hover: Color32::from_rgb(0x09, 0x69, 0xda),

        board_outline: Color32::from_rgb(0x35, 0x40, 0x50),
        board_fill: Color32::from_rgba_premultiplied(0xdf, 0xe7, 0xf0, 0x80),
        part_body: Color32::from_rgba_premultiplied(0xb0, 0xc0, 0xd4, 0x80),
        part_text: Color32::from_rgb(0x24, 0x29, 0x2f),
        pin_default: Color32::from_rgb(0xd1, 0x38, 0x2c),
        pin_unconnected: Color32::from_rgb(0x8c, 0x95, 0x9f),
        pin_highlight_cur_side: Color32::from_rgb(0x1a, 0x9e, 0x3d),
        pin_highlight_opp_side: Color32::from_rgb(0x09, 0x69, 0xda),
        via_cur_side: Color32::from_rgb(0xc0, 0x7a, 0x10),
        via_opp_side: Color32::from_rgb(0x80, 0x5a, 0x20),
        via_highlight: Color32::from_rgb(0x1a, 0x9e, 0x3d),
        net_web: Color32::from_rgba_premultiplied(0x1a, 0x9e, 0x3d, 0xcc),
    };

    pub const WARM: PcbTokens = PcbTokens {
        canvas: Color32::from_rgb(0x11, 0x11, 0x11), // charcoal black
        grid_minor: Color32::from_rgb(0x1c, 0x1c, 0x1c),
        grid_major: Color32::from_rgb(0x2a, 0x2a, 0x2a),
        axis_x: Color32::from_rgb(0x80, 0x20, 0x20),
        axis_y: Color32::from_rgb(0x20, 0x80, 0x20),
        text: Color32::from_rgb(0xee, 0xee, 0xee),
        selection: Color32::from_rgb(0xff, 0xd0, 0x00), // bright yellow
        hover: Color32::from_rgb(0xff, 0xe0, 0x55),

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
        net_web: Color32::from_rgba_premultiplied(0xff, 0xd0, 0x00, 0xb4),
    };

    pub const SWEET_COLD: PcbTokens = PcbTokens {
        canvas: Color32::from_rgb(0x0f, 0x14, 0x1d), // deep navy pastel canvas
        grid_minor: Color32::from_rgb(0x17, 0x1f, 0x2c),
        grid_major: Color32::from_rgb(0x22, 0x2d, 0x3d),
        axis_x: Color32::from_rgb(0x7a, 0x38, 0x38),
        axis_y: Color32::from_rgb(0x30, 0x6a, 0x78),
        text: Color32::from_rgb(0xde, 0xe7, 0xf3),
        selection: Color32::from_rgb(0xff, 0x9e, 0x3b), // agy warm pastel orange
        hover: Color32::from_rgb(0x79, 0xc0, 0xff),

        board_outline: Color32::from_rgb(0x79, 0xa8, 0xd8), // soft steel blue border
        board_fill: Color32::from_rgba_premultiplied(0x15, 0x20, 0x30, 0x90),
        part_body: Color32::from_rgba_premultiplied(0x2c, 0x3c, 0x54, 0x90),
        part_text: Color32::from_rgb(0xd0, 0xdf, 0xf0),
        pin_default: Color32::from_rgb(0x4d, 0x90, 0xcd), // pastel soft slate blue
        pin_unconnected: Color32::from_rgb(0x55, 0x64, 0x78),
        pin_highlight_cur_side: Color32::from_rgb(0xff, 0x9e, 0x3b), // agy warm pastel orange
        pin_highlight_opp_side: Color32::from_rgb(0x79, 0xc0, 0xff), // agy pastel cyan-blue
        via_cur_side: Color32::from_rgb(0xd9, 0x8a, 0x32), // soft warm amber
        via_opp_side: Color32::from_rgb(0x5a, 0x48, 0x2c),
        via_highlight: Color32::from_rgb(0xff, 0xb4, 0x54),
        net_web: Color32::from_rgba_premultiplied(0xff, 0x9e, 0x3b, 0xd0),
    };
}

/// Consolidated tokens for UI and PCB rendering
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tokens {
    pub app: AppTokens,
    pub pcb: PcbTokens,

    // App UI delegates as fields for ergonomics
    pub chrome: Color32,
    pub chrome_dark: Color32,
    pub panel: Color32,
    pub panel_header: Color32,
    pub tab_active: Color32,
    pub control: Color32,
    pub control_hover: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_faint: Color32,
    pub accent: Color32,
    pub selection: Color32,
    pub hover: Color32,
    pub status_bar_bg: Color32,
    pub status_bar_text: Color32,

    // PCB delegates as fields for ergonomics
    pub canvas: Color32,
    pub grid_minor: Color32,
    pub grid_major: Color32,
    pub axis_x: Color32,
    pub axis_y: Color32,
    pub part_text: Color32,
    pub board_outline: Color32,
    pub board_fill: Color32,
    pub part_body: Color32,
    pub pin_default: Color32,
    pub pin_unconnected: Color32,
    pub pin_highlight_cur_side: Color32,
    pub pin_highlight_opp_side: Color32,
    pub via_cur_side: Color32,
    pub via_opp_side: Color32,
    pub via_highlight: Color32,
    pub net_web: Color32,
}

impl Tokens {
    #[allow(dead_code)]
    pub fn new(app_theme: AppTheme, pcb_theme: PcbTheme) -> Self {
        Self::new_with_custom(app_theme, pcb_theme, None)
    }

    pub fn new_with_custom(
        app_theme: AppTheme,
        pcb_theme: PcbTheme,
        custom_pcb: Option<PcbTokens>,
    ) -> Self {
        let app = app_theme.tokens();
        let pcb = if pcb_theme == PcbTheme::Custom {
            custom_pcb.unwrap_or(PcbTokens::CADCRAFT)
        } else {
            pcb_theme.tokens()
        };
        Self {
            app,
            pcb,
            chrome: app.chrome,
            chrome_dark: app.chrome_dark,
            panel: app.panel,
            panel_header: app.panel_header,
            tab_active: app.tab_active,
            control: app.control,
            control_hover: app.control_hover,
            border: app.border,
            text: app.text,
            text_dim: app.text_dim,
            text_faint: app.text_faint,
            accent: app.accent,
            selection: app.selection,
            hover: app.hover,
            status_bar_bg: app.status_bar_bg,
            status_bar_text: app.status_bar_text,

            canvas: pcb.canvas,
            grid_minor: pcb.grid_minor,
            grid_major: pcb.grid_major,
            axis_x: pcb.axis_x,
            axis_y: pcb.axis_y,
            part_text: pcb.part_text,
            board_outline: pcb.board_outline,
            board_fill: pcb.board_fill,
            part_body: pcb.part_body,
            pin_default: pcb.pin_default,
            pin_unconnected: pcb.pin_unconnected,
            pin_highlight_cur_side: pcb.pin_highlight_cur_side,
            pin_highlight_opp_side: pcb.pin_highlight_opp_side,
            via_cur_side: pcb.via_cur_side,
            via_opp_side: pcb.via_opp_side,
            via_highlight: pcb.via_highlight,
            net_web: pcb.net_web,
        }
    }

    #[allow(dead_code)]
    pub fn egui_visuals(&self, is_light: bool) -> egui::Visuals {
        self.app.egui_visuals(is_light)
    }

    // Convenience delegates for app UI
    #[allow(dead_code)]
    pub fn chrome(&self) -> Color32 { self.app.chrome }
    #[allow(dead_code)]
    pub fn chrome_dark(&self) -> Color32 { self.app.chrome_dark }
    #[allow(dead_code)]
    pub fn panel(&self) -> Color32 { self.app.panel }
    #[allow(dead_code)]
    pub fn panel_header(&self) -> Color32 { self.app.panel_header }
    #[allow(dead_code)]
    pub fn tab_active(&self) -> Color32 { self.app.tab_active }
    #[allow(dead_code)]
    pub fn control(&self) -> Color32 { self.app.control }
    #[allow(dead_code)]
    pub fn control_hover(&self) -> Color32 { self.app.control_hover }
    #[allow(dead_code)]
    pub fn border(&self) -> Color32 { self.app.border }
    #[allow(dead_code)]
    pub fn text(&self) -> Color32 { self.app.text }
    #[allow(dead_code)]
    pub fn text_dim(&self) -> Color32 { self.app.text_dim }
    #[allow(dead_code)]
    pub fn text_faint(&self) -> Color32 { self.app.text_faint }
    #[allow(dead_code)]
    pub fn accent(&self) -> Color32 { self.app.accent }
    #[allow(dead_code)]
    pub fn selection(&self) -> Color32 { self.app.selection }
    #[allow(dead_code)]
    pub fn hover(&self) -> Color32 { self.app.hover }
    #[allow(dead_code)]
    pub fn status_bar_bg(&self) -> Color32 { self.app.status_bar_bg }
    #[allow(dead_code)]
    pub fn status_bar_text(&self) -> Color32 { self.app.status_bar_text }

    // Convenience delegates for PCB
    #[allow(dead_code)]
    pub fn canvas(&self) -> Color32 { self.pcb.canvas }
    #[allow(dead_code)]
    pub fn grid_minor(&self) -> Color32 { self.pcb.grid_minor }
    #[allow(dead_code)]
    pub fn grid_major(&self) -> Color32 { self.pcb.grid_major }
    #[allow(dead_code)]
    pub fn axis_x(&self) -> Color32 { self.pcb.axis_x }
    #[allow(dead_code)]
    pub fn axis_y(&self) -> Color32 { self.pcb.axis_y }
    #[allow(dead_code)]
    pub fn pcb_selection(&self) -> Color32 { self.pcb.selection }
    #[allow(dead_code)]
    pub fn pcb_hover(&self) -> Color32 { self.pcb.hover }
    #[allow(dead_code)]
    pub fn board_outline(&self) -> Color32 { self.pcb.board_outline }
    #[allow(dead_code)]
    pub fn board_fill(&self) -> Color32 { self.pcb.board_fill }
    #[allow(dead_code)]
    pub fn part_body(&self) -> Color32 { self.pcb.part_body }
    #[allow(dead_code)]
    pub fn part_text(&self) -> Color32 { self.pcb.part_text }
    #[allow(dead_code)]
    pub fn pin_default(&self) -> Color32 { self.pcb.pin_default }
    #[allow(dead_code)]
    pub fn pin_unconnected(&self) -> Color32 { self.pcb.pin_unconnected }
    #[allow(dead_code)]
    pub fn pin_highlight_cur_side(&self) -> Color32 { self.pcb.pin_highlight_cur_side }
    #[allow(dead_code)]
    pub fn pin_highlight_opp_side(&self) -> Color32 { self.pcb.pin_highlight_opp_side }
    #[allow(dead_code)]
    pub fn via_cur_side(&self) -> Color32 { self.pcb.via_cur_side }
    #[allow(dead_code)]
    pub fn via_opp_side(&self) -> Color32 { self.pcb.via_opp_side }
    #[allow(dead_code)]
    pub fn via_highlight(&self) -> Color32 { self.pcb.via_highlight }
    #[allow(dead_code)]
    pub fn net_web(&self) -> Color32 { self.pcb.net_web }

    pub const DARK: Tokens = Tokens {
        app: AppTokens::DARK,
        pcb: PcbTokens::CADCRAFT,
        chrome: AppTokens::DARK.chrome,
        chrome_dark: AppTokens::DARK.chrome_dark,
        panel: AppTokens::DARK.panel,
        panel_header: AppTokens::DARK.panel_header,
        tab_active: AppTokens::DARK.tab_active,
        control: AppTokens::DARK.control,
        control_hover: AppTokens::DARK.control_hover,
        border: AppTokens::DARK.border,
        text: AppTokens::DARK.text,
        text_dim: AppTokens::DARK.text_dim,
        text_faint: AppTokens::DARK.text_faint,
        accent: AppTokens::DARK.accent,
        selection: AppTokens::DARK.selection,
        hover: AppTokens::DARK.hover,
        status_bar_bg: AppTokens::DARK.status_bar_bg,
        status_bar_text: AppTokens::DARK.status_bar_text,

        canvas: PcbTokens::CADCRAFT.canvas,
        grid_minor: PcbTokens::CADCRAFT.grid_minor,
        grid_major: PcbTokens::CADCRAFT.grid_major,
        axis_x: PcbTokens::CADCRAFT.axis_x,
        axis_y: PcbTokens::CADCRAFT.axis_y,
        part_text: PcbTokens::CADCRAFT.part_text,
        board_outline: PcbTokens::CADCRAFT.board_outline,
        board_fill: PcbTokens::CADCRAFT.board_fill,
        part_body: PcbTokens::CADCRAFT.part_body,
        pin_default: PcbTokens::CADCRAFT.pin_default,
        pin_unconnected: PcbTokens::CADCRAFT.pin_unconnected,
        pin_highlight_cur_side: PcbTokens::CADCRAFT.pin_highlight_cur_side,
        pin_highlight_opp_side: PcbTokens::CADCRAFT.pin_highlight_opp_side,
        via_cur_side: PcbTokens::CADCRAFT.via_cur_side,
        via_opp_side: PcbTokens::CADCRAFT.via_opp_side,
        via_highlight: PcbTokens::CADCRAFT.via_highlight,
        net_web: PcbTokens::CADCRAFT.net_web,
    };
    #[allow(dead_code)]
    pub const LIGHT: Tokens = Tokens {
        app: AppTokens::LIGHT,
        pcb: PcbTokens::LIGHT,
        chrome: AppTokens::LIGHT.chrome,
        chrome_dark: AppTokens::LIGHT.chrome_dark,
        panel: AppTokens::LIGHT.panel,
        panel_header: AppTokens::LIGHT.panel_header,
        tab_active: AppTokens::LIGHT.tab_active,
        control: AppTokens::LIGHT.control,
        control_hover: AppTokens::LIGHT.control_hover,
        border: AppTokens::LIGHT.border,
        text: AppTokens::LIGHT.text,
        text_dim: AppTokens::LIGHT.text_dim,
        text_faint: AppTokens::LIGHT.text_faint,
        accent: AppTokens::LIGHT.accent,
        selection: AppTokens::LIGHT.selection,
        hover: AppTokens::LIGHT.hover,
        status_bar_bg: AppTokens::LIGHT.status_bar_bg,
        status_bar_text: AppTokens::LIGHT.status_bar_text,

        canvas: PcbTokens::LIGHT.canvas,
        grid_minor: PcbTokens::LIGHT.grid_minor,
        grid_major: PcbTokens::LIGHT.grid_major,
        axis_x: PcbTokens::LIGHT.axis_x,
        axis_y: PcbTokens::LIGHT.axis_y,
        part_text: PcbTokens::LIGHT.part_text,
        board_outline: PcbTokens::LIGHT.board_outline,
        board_fill: PcbTokens::LIGHT.board_fill,
        part_body: PcbTokens::LIGHT.part_body,
        pin_default: PcbTokens::LIGHT.pin_default,
        pin_unconnected: PcbTokens::LIGHT.pin_unconnected,
        pin_highlight_cur_side: PcbTokens::LIGHT.pin_highlight_cur_side,
        pin_highlight_opp_side: PcbTokens::LIGHT.pin_highlight_opp_side,
        via_cur_side: PcbTokens::LIGHT.via_cur_side,
        via_opp_side: PcbTokens::LIGHT.via_opp_side,
        via_highlight: PcbTokens::LIGHT.via_highlight,
        net_web: PcbTokens::LIGHT.net_web,
    };
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
        for app_theme in AppTheme::ALL {
            for pcb_theme in PcbTheme::ALL {
                let tokens = Tokens::new(app_theme, pcb_theme);
                assert!(tokens.pcb.canvas.r() > 0 || tokens.pcb.canvas.g() > 0 || tokens.pcb.canvas.b() > 0);
                assert!(tokens.app.accent.r() > 0 || tokens.app.accent.g() > 0 || tokens.app.accent.b() > 0);
                assert!(tokens.pcb.selection.r() > 0 || tokens.pcb.selection.g() > 0 || tokens.pcb.selection.b() > 0);
                assert!(tokens.pcb.pin_highlight_cur_side.r() > 0 || tokens.pcb.pin_highlight_cur_side.g() > 0 || tokens.pcb.pin_highlight_cur_side.b() > 0);
                assert!(tokens.pcb.board_outline.r() > 0 || tokens.pcb.board_outline.g() > 0 || tokens.pcb.board_outline.b() > 0);
            }
        }
    }

    #[test]
    fn test_theme_contrast_ratios() {
        for app_theme in AppTheme::ALL {
            let tokens = app_theme.tokens();
            let ratio = contrast_ratio(tokens.text, tokens.panel);
            assert!(
                ratio >= 4.5,
                "Theme {:?} failed WCAG AA text contrast: ratio was {:.2}:1, expected >= 4.5:1",
                app_theme,
                ratio
            );
        }
    }
}

