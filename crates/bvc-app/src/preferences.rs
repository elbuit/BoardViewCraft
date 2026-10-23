//! User preferences and settings dialog.

use crate::i18n::{self, Language};
use crate::theme::{ThemePreset, Tokens};
use egui::{Align2, Color32, Context, RichText, Vec2, Window};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_BYTES: u64 = 64 * 1024;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preferences {
    pub language: Language,
    pub theme: ThemePreset,
    pub custom_tokens: Option<Tokens>,
    pub show_net_web: bool,
    pub show_part_labels: bool,
    pub show_pin_annotations: bool,
    pub high_density_lod: bool,
    pub show_opposite_elements: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            language: Language::English,
            theme: ThemePreset::CadcraftDark,
            custom_tokens: None,
            show_net_web: true,
            show_part_labels: true,
            show_pin_annotations: true,
            high_density_lod: true,
            show_opposite_elements: false,
        }
    }
}

pub fn config_dir_from(platform: &str, get: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    if let Some(root) = get("BOARDVIEWCRAFT_CONFIG_DIR") {
        return (!root.is_empty()).then(|| PathBuf::from(root));
    }
    let nonempty = |key| get(key).filter(|value| !value.is_empty()).map(PathBuf::from);
    match platform {
        "macos" => nonempty("HOME").map(|home| home.join("Library/Application Support/BoardViewCraft")),
        "windows" => nonempty("APPDATA").map(|root| root.join("BoardViewCraft")),
        _ => nonempty("XDG_CONFIG_HOME")
            .map(|root| root.join("boardviewcraft"))
            .or_else(|| nonempty("HOME").map(|home| home.join(".config/boardviewcraft"))),
    }
}

pub fn default_config_path() -> PathBuf {
    config_dir_from(std::env::consts::OS, |key| std::env::var_os(key))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("preferences.json")
}

impl Preferences {
    pub fn load() -> Self {
        Self::load_from(&default_config_path())
    }

    pub fn load_from(path: &Path) -> Self {
        match Self::read_file(path) {
            Ok(Some(prefs)) => prefs,
            Ok(None) => Self::default(),
            Err(err) => {
                eprintln!("Warning: Failed to load preferences from {}: {err}", path.display());
                Self::default()
            }
        }
    }

    fn read_file(path: &Path) -> Result<Option<Self>, String> {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("{}: {error}", path.display())),
        };
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(format!("{}: preferences file exceeds 64 KiB", path.display()));
        }
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|error| format!("{}: invalid preferences JSON: {error}", path.display()))?;
        if value.get("version").and_then(Value::as_u64) != Some(1) {
            return Err(format!("{}: unsupported preferences version", path.display()));
        }
        let prefs_val = value
            .get("preferences")
            .ok_or_else(|| format!("{}: missing preferences object", path.display()))?;
        let prefs: Preferences = serde_json::from_value(prefs_val.clone())
            .map_err(|error| format!("{}: invalid preferences payload: {error}", path.display()))?;
        Ok(Some(prefs))
    }

    pub fn save_atomic(&self) -> Result<(), String> {
        self.save_atomic_to(&default_config_path())
    }

    pub fn save_atomic_to(&self, path: &Path) -> Result<(), String> {
        let json_value = serde_json::json!({
            "version": 1,
            "preferences": self,
        });
        let bytes = serde_json::to_vec_pretty(&json_value).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err("preferences exceeds 64 KiB".into());
        }
        let parent = path.parent().ok_or("preferences path has no parent")?;
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(".preferences-{}-{sequence}.tmp", std::process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|error| format!("{}: {error}", temporary.display()))?;
        let result = (|| {
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, path)
        })();
        if let Err(error) = result {
            let _ = fs::remove_file(&temporary);
            return Err(format!("{}: could not save preferences: {error}", path.display()));
        }
        Ok(())
    }
}

/// Displays the preferences window dialog.
/// Returns `true` if any preference was changed this frame.
pub fn show_preferences_window(
    ctx: &Context,
    open: &mut bool,
    prefs: &mut Preferences,
    tokens: &Tokens,
) -> bool {
    if !*open {
        return false;
    }

    let mut changed = false;

    // Check Escape key to close
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        *open = false;
        return false;
    }

    let mut is_open = *open;
    let mut close_requested = false;

    Window::new(RichText::new("Preferences").strong().color(tokens.accent))
        .open(&mut is_open)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .resizable(true)
        .min_size(Vec2::new(420.0, 320.0))
        .default_size(Vec2::new(440.0, 360.0))
        .show(ctx, |ui| {
            let lang = prefs.language;
            ui.add_space(4.0);

            // Language Selector
            ui.horizontal(|ui| {
                ui.label(RichText::new(i18n::tr(lang, "prefs.language")).strong().color(tokens.text));
                egui::ComboBox::from_id_salt("language_combo")
                    .selected_text(prefs.language.display_name())
                    .show_ui(ui, |ui| {
                        for l in Language::ALL {
                            if ui.selectable_value(&mut prefs.language, l, l.display_name()).clicked() {
                                changed = true;
                            }
                        }
                    });
            });

            ui.add_space(8.0);
            ui.heading(RichText::new(i18n::tr(lang, "prefs.appearance")).color(tokens.text));
            ui.separator();

            ui.horizontal(|ui| {
                ui.label(RichText::new(i18n::tr(lang, "prefs.color_scheme")).strong().color(tokens.text));
                egui::ComboBox::from_id_salt("theme_preset_combo")
                    .selected_text(prefs.theme.display_name())
                    .show_ui(ui, |ui| {
                        for preset in ThemePreset::ALL {
                            if ui
                                .selectable_value(&mut prefs.theme, preset, preset.display_name())
                                .clicked()
                            {
                                changed = true;
                                if preset == ThemePreset::Custom && prefs.custom_tokens.is_none() {
                                    prefs.custom_tokens = Some(*tokens);
                                }
                            }
                        }
                    });
            });

            // If Custom theme selected, expose color pickers
            if prefs.theme == ThemePreset::Custom {
                let custom = prefs.custom_tokens.get_or_insert_with(|| *tokens);
                ui.add_space(4.0);
                ui.label(RichText::new(i18n::tr(lang, "prefs.custom_colors")).strong().color(tokens.accent));
                egui::Grid::new("custom_colors_grid").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
                    ui.label("Canvas Background:");
                    let mut c = custom.canvas.to_array();
                    if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                        custom.canvas = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                        changed = true;
                    }
                    ui.end_row();

                    ui.label("Pin Default Color:");
                    let mut c = custom.pin_default.to_array();
                    if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                        custom.pin_default = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                        changed = true;
                    }
                    ui.end_row();

                    ui.label("Pin Highlight (Active Side):");
                    let mut c = custom.pin_highlight_cur_side.to_array();
                    if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                        custom.pin_highlight_cur_side = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                        changed = true;
                    }
                    ui.end_row();

                    ui.label("Via Color:");
                    let mut c = custom.via_cur_side.to_array();
                    if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                        custom.via_cur_side = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                        changed = true;
                    }
                    ui.end_row();

                    ui.label("Net Web Lines:");
                    let mut c = custom.net_web.to_array();
                    if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                        custom.net_web = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                        changed = true;
                    }
                    ui.end_row();

                    ui.label("Board Outline:");
                    let mut c = custom.board_outline.to_array();
                    if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                        custom.board_outline = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                        changed = true;
                    }
                    ui.end_row();
                });
            }

            ui.add_space(12.0);
            ui.heading(RichText::new(i18n::tr(lang, "prefs.visualization")).color(tokens.text));
            ui.separator();

            if ui
                .checkbox(
                    &mut prefs.show_net_web,
                    RichText::new(i18n::tr(lang, "prefs.show_net_web")).color(tokens.text),
                )
                .on_hover_text("Draw connecting spiderweb lines between pins and vias on the selected net.")
                .changed()
            {
                changed = true;
            }

            if ui
                .checkbox(
                    &mut prefs.show_part_labels,
                    RichText::new(i18n::tr(lang, "prefs.show_part_labels")).color(tokens.text),
                )
                .on_hover_text("Render part name labels on components when zoomed in.")
                .changed()
            {
                changed = true;
            }

            if ui
                .checkbox(
                    &mut prefs.show_pin_annotations,
                    RichText::new(i18n::tr(lang, "prefs.show_pin_annotations")).color(tokens.text),
                )
                .on_hover_text("Render pin numbers and descriptions when zooming into components.")
                .changed()
            {
                changed = true;
            }

            if ui
                .checkbox(
                    &mut prefs.high_density_lod,
                    RichText::new(i18n::tr(lang, "prefs.adaptive_lod")).color(tokens.text),
                )
                .on_hover_text("Simplify circles to quads/points on dense nets and high zoom outs for 60+ FPS performance.")
                .changed()
            {
                changed = true;
            }

            if ui
                .checkbox(
                    &mut prefs.show_opposite_elements,
                    RichText::new(i18n::tr(lang, "prefs.show_opposite")).color(tokens.text),
                )
                .on_hover_text("When enabled, dimmed pins and vias on the reverse side of the board are visible.")
                .changed()
            {
                changed = true;
            }

            ui.add_space(16.0);
            ui.separator();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(i18n::tr(lang, "prefs.close")).clicked() {
                    close_requested = true;
                }
            });
        });

    if !is_open || close_requested {
        *open = false;
    }

    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preferences_serde() {
        let prefs = Preferences {
            theme: ThemePreset::OpenBoardViewClassic,
            custom_tokens: Some(Tokens::DARK),
            language: Language::Catalan,
            show_net_web: false,
            show_part_labels: true,
            show_pin_annotations: false,
            high_density_lod: true,
            show_opposite_elements: false,
        };

        let json = serde_json::to_string(&prefs).expect("Serialize to JSON");
        let deserialized: Preferences = serde_json::from_str(&json).expect("Deserialize from JSON");

        assert_eq!(prefs, deserialized);
    }

    #[test]
    fn test_config_dir_resolution() {
        let get = |key: &str| match key {
            "BOARDVIEWCRAFT_CONFIG_DIR" => Some(OsString::from("/custom/config/dir")),
            "HOME" | "APPDATA" | "XDG_CONFIG_HOME" => Some(OsString::from("/unrelated")),
            _ => None,
        };
        for platform in ["macos", "windows", "linux"] {
            assert_eq!(
                config_dir_from(platform, get),
                Some(PathBuf::from("/custom/config/dir"))
            );
        }

        // Test Linux XDG and HOME
        let get_linux_xdg = |key: &str| match key {
            "XDG_CONFIG_HOME" => Some(OsString::from("/home/user/.config_xdg")),
            _ => None,
        };
        assert_eq!(
            config_dir_from("linux", get_linux_xdg),
            Some(PathBuf::from("/home/user/.config_xdg/boardviewcraft"))
        );

        let get_linux_home = |key: &str| match key {
            "HOME" => Some(OsString::from("/home/user")),
            _ => None,
        };
        assert_eq!(
            config_dir_from("linux", get_linux_home),
            Some(PathBuf::from("/home/user/.config/boardviewcraft"))
        );

        // Test macOS
        let get_macos = |key: &str| match key {
            "HOME" => Some(OsString::from("/Users/alice")),
            _ => None,
        };
        assert_eq!(
            config_dir_from("macos", get_macos),
            Some(PathBuf::from("/Users/alice/Library/Application Support/BoardViewCraft"))
        );

        // Test Windows
        let get_windows = |key: &str| match key {
            "APPDATA" => Some(OsString::from("/appdata")),
            _ => None,
        };
        assert_eq!(
            config_dir_from("windows", get_windows),
            Some(PathBuf::from("/appdata/BoardViewCraft"))
        );
    }

    #[test]
    fn test_atomic_save_and_load_roundtrip() {
        let temp_dir = std::env::temp_dir().join(format!("bvc_test_prefs_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        let config_file = temp_dir.join("preferences.json");

        let mut prefs = Preferences::default();
        prefs.language = Language::Spanish;
        prefs.show_net_web = false;
        prefs.theme = ThemePreset::HighContrast;

        // Save
        prefs.save_atomic_to(&config_file).expect("Atomic save should succeed");

        // Verify file exists and load
        assert!(config_file.exists());
        let loaded = Preferences::load_from(&config_file);
        assert_eq!(prefs, loaded);

        // Test corrupted file fallback
        fs::write(&config_file, b"corrupted json content!").unwrap();
        let fallback = Preferences::load_from(&config_file);
        assert_eq!(fallback, Preferences::default());

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
