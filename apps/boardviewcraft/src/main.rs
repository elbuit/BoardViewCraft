//! BoardViewCraft desktop application binary.

use bvc_ui_egui::BoardViewApp;
use egui::{Color32, Vec2};

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(Vec2::new(1280.0, 800.0))
            .with_title("BoardViewCraft - Pure Rust CAD Board Viewer"),
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
