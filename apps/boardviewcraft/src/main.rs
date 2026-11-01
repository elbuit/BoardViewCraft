//! BoardViewCraft desktop application binary.

use bvc_ui_egui::BoardViewApp;
use egui::{Color32, Vec2};

/// Freedesktop app id: the hicolor icon name, the metainfo id and the Flatpak id.
const APP_ID: &str = "io.github.elbuit.BoardViewCraft";

/// The app icon (assets/app-icon/README.md). macOS gets the version on Apple's icon grid, with a
/// transparent margin; Windows and Linux get the full-bleed tile.
#[cfg(target_os = "macos")]
const APP_ICON_PNG: &[u8] = include_bytes!("../../../assets/app-icon/boardviewcraft-1024.png");
#[cfg(not(target_os = "macos"))]
const APP_ICON_PNG: &[u8] = include_bytes!(
    "../../../assets/app-icon/hicolor/256x256/apps/io.github.elbuit.BoardViewCraft.png"
);

/// Decodes the embedded app icon for the window (Dock, taskbar, Alt-Tab and launcher).
fn app_icon() -> Result<egui::IconData, String> {
    eframe::icon_data::from_png_bytes(APP_ICON_PNG).map_err(|e| e.to_string())
}

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size(Vec2::new(1280.0, 800.0))
        .with_title("BoardViewCraft - Pure Rust CAD Board Viewer")
        // Wayland app id / X11 WM_CLASS: matches the hicolor icon and metainfo id.
        .with_app_id(APP_ID);
    match app_icon() {
        Ok(icon) => viewport = viewport.with_icon(icon),
        Err(e) => eprintln!("app icon: {e}"),
    }
    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "BoardViewCraft",
        native_options,
        Box::new(|cc| {
            // Apply dark styling defaults
            let mut visuals = egui::Visuals::dark();
            visuals.panel_fill = Color32::from_rgb(0x28, 0x2e, 0x38);
            visuals.window_fill = Color32::from_rgb(0x28, 0x2e, 0x38);
            cc.egui_ctx.set_visuals(visuals);

            let mut app = BoardViewApp::new(cc);

            // If a file argument is provided on CLI, load it automatically on start
            if let Some(arg) = std::env::args().nth(1) {
                let path = std::path::Path::new(&arg);
                if let Ok(bytes) = std::fs::read(path) {
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("board.cad");
                    app.load_file(name, &bytes);
                } else {
                    app.set_status(format!("Failed to read file: {arg}"));
                }
            }

            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_app_icon_decodes_to_a_square_rgba_image() {
        let icon = app_icon().expect("embedded app icon is a valid PNG");
        let expected = if cfg!(target_os = "macos") { 1024 } else { 256 };
        assert_eq!((icon.width, icon.height), (expected, expected));
        assert_eq!(icon.rgba.len(), (expected * expected * 4) as usize);
        // The corners outside the rounded tile are transparent.
        assert_eq!(icon.rgba[3], 0, "top-left pixel alpha");
    }
}
