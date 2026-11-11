//! User preferences and settings dialog.

use crate::board_tabs::{BoardTab, BothLayout, PaneContent};
use crate::i18n::{self, Language, LanguagePref};
use crate::keymap::{Action, CaptureOutcome, Keymap};
use crate::theme::{AppTheme, PcbTheme, PcbTokens, Tokens};
use bvc_core::Side;
use egui::{Align2, Color32, Context, RichText, Vec2, Window};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_BYTES: u64 = 64 * 1024;
/// Maximum number of remembered board -> schematic PDF associations (oldest dropped first),
/// keeping the preferences file well below `MAX_BYTES`.
pub const MAX_BOARD_SCHEMATICS: usize = 32;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn default_true() -> bool {
    true
}

fn default_cross_layer_transparency() -> f32 {
    0.60
}

fn default_split_ratio() -> f32 {
    0.5
}

fn default_divisions() -> u8 {
    1
}

fn default_pane_first() -> PaneContent {
    PaneContent::Top
}

fn default_pane_second() -> PaneContent {
    PaneContent::Pdf
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ViaDisplayMode {
    #[default]
    Web,
    Radial,
}

impl ViaDisplayMode {
    pub const ALL: [ViaDisplayMode; 2] = [
        ViaDisplayMode::Radial,
        ViaDisplayMode::Web,
    ];

    pub fn display_name(&self, lang: Language) -> &'static str {
        match self {
            ViaDisplayMode::Radial => i18n::tr(lang, "prefs.via_mode_radial"),
            ViaDisplayMode::Web => i18n::tr(lang, "prefs.via_mode_web"),
        }
    }
}

/// The schematic PDF last opened while a board file was loaded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardSchematic {
    pub board: PathBuf,
    pub pdf: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CustomPcbTheme {
    pub name: String,
    pub tokens: PcbTokens,
    #[serde(default)]
    pub net_lines_dashed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preferences {
    /// Auto follows the system locale; resolve it with [`Preferences::ui_language`].
    #[serde(default)]
    pub language: LanguagePref,
    #[serde(default)]
    pub app_theme: AppTheme,
    #[serde(default)]
    pub pcb_theme: PcbTheme,
    #[serde(default)]
    pub custom_pcb_tokens: Option<PcbTokens>,
    #[serde(default)]
    pub custom_pcb_theme_name: Option<String>,
    #[serde(default)]
    pub custom_pcb_themes: Vec<CustomPcbTheme>,
    #[serde(default)]
    pub theme_overrides: std::collections::BTreeMap<String, PcbTokens>,
    #[serde(default)]
    pub theme_dashed_overrides: std::collections::BTreeMap<String, bool>,
    pub show_net_web: bool,
    #[serde(default)]
    pub via_display_mode: ViaDisplayMode,
    #[serde(default)]
    pub net_lines_dashed: bool,
    #[serde(default = "default_cross_layer_transparency")]
    pub cross_layer_transparency: f32,
    pub show_part_labels: bool,
    pub show_pin_annotations: bool,
    pub high_density_lod: bool,
    pub show_opposite_elements: bool,
    #[serde(default)]
    pub show_vias: bool,
    #[serde(default)]
    pub show_unnamed_vias: bool,
    #[serde(default = "default_true")]
    pub show_pins: bool,
    #[serde(default)]
    pub recent_files: Vec<PathBuf>,
    /// Each board's schematic PDF, most recently used first (at most `MAX_BOARD_SCHEMATICS`).
    #[serde(default)]
    pub board_schematics: Vec<BoardSchematic>,
    /// Central-area tab shown last (Top, Bottom, Both, PDF or Split).
    #[serde(default)]
    pub board_tab: BoardTab,
    /// Arrangement of the two sides in the Both tab.
    #[serde(default)]
    pub both_layout: BothLayout,
    /// Split view orientation: true for horizontal (board left, PDF right), false for vertical.
    #[serde(default = "default_true")]
    pub split_horizontal: bool,
    /// Split view ratio for board pane (between 0.15 and 0.85).
    #[serde(default = "default_split_ratio")]
    pub split_ratio: f32,
    /// Side of the PCB shown in the split view (Top or Bottom).
    #[serde(default)]
    pub split_side: Side,
    /// Number of divisions in the central view: 1 or 2.
    #[serde(default = "default_divisions")]
    pub view_divisions: u8,
    /// Content of the first division pane (left or top).
    #[serde(default = "default_pane_first")]
    pub pane_first: PaneContent,
    /// Content of the second division pane (right or bottom).
    #[serde(default = "default_pane_second")]
    pub pane_second: PaneContent,
    /// Whether zoom between PCB canvases (Top/Bottom) is locked/synchronized.
    #[serde(default)]
    pub lock_zoom: bool,
    /// Side panel docked on the left (true) or on the right (false) of the window.
    #[serde(default = "default_true")]
    pub side_panel_left: bool,
    /// Keyboard shortcuts; missing or invalid entries fall back to the default keys.
    #[serde(default)]
    pub keymap: Keymap,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            language: LanguagePref::Auto,
            app_theme: AppTheme::Dark,
            pcb_theme: PcbTheme::BlueCarrot,
            custom_pcb_tokens: None,
            custom_pcb_theme_name: None,
            custom_pcb_themes: Vec::new(),
            theme_overrides: std::collections::BTreeMap::new(),
            theme_dashed_overrides: std::collections::BTreeMap::new(),
            show_net_web: true,
            via_display_mode: ViaDisplayMode::Web,
            net_lines_dashed: false,
            cross_layer_transparency: 0.60,
            show_part_labels: true,
            show_pin_annotations: true,
            high_density_lod: true,
            show_opposite_elements: false,
            show_vias: false,
            show_unnamed_vias: false,
            show_pins: true,
            recent_files: Vec::new(),
            board_schematics: Vec::new(),
            board_tab: BoardTab::Top,
            both_layout: BothLayout::Auto,
            split_horizontal: true,
            split_ratio: 0.5,
            split_side: Side::Top,
            view_divisions: 1,
            pane_first: PaneContent::Top,
            pane_second: PaneContent::Pdf,
            lock_zoom: false,
            side_panel_left: true,
            keymap: Keymap::default(),
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

/// Version written by [`Preferences::save_atomic_to`]. Version 1 forced English as the default
/// language; version 2 adds Auto (system language).
const PREFERENCES_VERSION: u64 = 2;

impl Preferences {
    /// The language the UI uses this frame (Auto resolved to the system language).
    pub fn ui_language(&self) -> Language {
        self.language.resolve()
    }

    pub fn active_theme_display_name(&self) -> String {
        match self.pcb_theme {
            PcbTheme::Custom => {
                if let Some(ref name) = self.custom_pcb_theme_name {
                    name.clone()
                } else if let Some(first) = self.custom_pcb_themes.first() {
                    first.name.clone()
                } else {
                    PcbTheme::BlueCarrot.display_name().to_string()
                }
            }
            built_in => {
                let name = built_in.display_name();
                if self.theme_overrides.contains_key(name) || self.theme_dashed_overrides.contains_key(name) {
                    format!("{} *", name)
                } else {
                    name.to_string()
                }
            }
        }
    }

    pub fn active_pcb_tokens(&self) -> PcbTokens {
        match self.pcb_theme {
            PcbTheme::Custom => {
                if let Some(ref name) = self.custom_pcb_theme_name {
                    if let Some(custom) = self.custom_pcb_themes.iter().find(|t| &t.name == name) {
                        return custom.tokens;
                    }
                }
                if let Some(tokens) = self.custom_pcb_tokens {
                    return tokens;
                }
                if let Some(first) = self.custom_pcb_themes.first() {
                    return first.tokens;
                }
                PcbTokens::DARK
            }
            built_in => {
                let name = built_in.display_name();
                if let Some(&overridden) = self.theme_overrides.get(name) {
                    overridden
                } else {
                    built_in.tokens()
                }
            }
        }
    }

    pub fn set_active_pcb_tokens(&mut self, tokens: PcbTokens) {
        match self.pcb_theme {
            PcbTheme::Custom => {
                self.custom_pcb_tokens = Some(tokens);
                if let Some(ref name) = self.custom_pcb_theme_name {
                    if let Some(custom) = self.custom_pcb_themes.iter_mut().find(|t| &t.name == name) {
                        custom.tokens = tokens;
                        return;
                    }
                }
                if let Some(first) = self.custom_pcb_themes.first_mut() {
                    first.tokens = tokens;
                }
            }
            built_in => {
                self.theme_overrides.insert(built_in.display_name().to_string(), tokens);
            }
        }
    }

    pub fn active_pcb_net_lines_dashed(&self) -> bool {
        match self.pcb_theme {
            PcbTheme::Custom => {
                if let Some(ref name) = self.custom_pcb_theme_name {
                    if let Some(custom) = self.custom_pcb_themes.iter().find(|t| &t.name == name) {
                        return custom.net_lines_dashed;
                    }
                }
                self.net_lines_dashed
            }
            built_in => {
                let name = built_in.display_name();
                if let Some(&dashed) = self.theme_dashed_overrides.get(name) {
                    dashed
                } else {
                    self.net_lines_dashed
                }
            }
        }
    }

    pub fn set_active_pcb_net_lines_dashed(&mut self, dashed: bool) {
        self.net_lines_dashed = dashed;
        match self.pcb_theme {
            PcbTheme::Custom => {
                if let Some(ref name) = self.custom_pcb_theme_name {
                    if let Some(custom) = self.custom_pcb_themes.iter_mut().find(|t| &t.name == name) {
                        custom.net_lines_dashed = dashed;
                    }
                }
            }
            built_in => {
                self.theme_dashed_overrides.insert(built_in.display_name().to_string(), dashed);
            }
        }
    }

    pub fn is_builtin_theme_modified(&self) -> bool {
        if self.pcb_theme == PcbTheme::Custom {
            false
        } else {
            let name = self.pcb_theme.display_name();
            self.theme_overrides.contains_key(name) || self.theme_dashed_overrides.contains_key(name)
        }
    }

    pub fn reset_active_built_in_theme(&mut self) {
        if self.pcb_theme != PcbTheme::Custom {
            let name = self.pcb_theme.display_name();
            self.theme_overrides.remove(name);
            self.theme_dashed_overrides.remove(name);
            self.net_lines_dashed = false;
        }
    }

    pub fn select_builtin_theme(&mut self, theme: PcbTheme) {
        self.pcb_theme = theme;
        self.net_lines_dashed = self.active_pcb_net_lines_dashed();
    }

    pub fn select_custom_theme(&mut self, name: &str) {
        if let Some(theme) = self.custom_pcb_themes.iter().find(|t| t.name == name) {
            self.pcb_theme = PcbTheme::Custom;
            self.custom_pcb_theme_name = Some(name.to_string());
            self.custom_pcb_tokens = Some(theme.tokens);
            self.net_lines_dashed = theme.net_lines_dashed;
        }
    }

    pub fn delete_custom_theme(&mut self, name: &str) {
        self.custom_pcb_themes.retain(|t| t.name != name);
        if self.custom_pcb_theme_name.as_deref() == Some(name) {
            self.pcb_theme = PcbTheme::BlueCarrot;
            self.custom_pcb_theme_name = None;
            self.custom_pcb_tokens = None;
            self.net_lines_dashed = self.active_pcb_net_lines_dashed();
        }
    }

    pub fn create_new_theme_from_current(&mut self, name: String) {
        let current_tokens = self.active_pcb_tokens();
        let current_dashed = self.active_pcb_net_lines_dashed();
        self.custom_pcb_themes.push(CustomPcbTheme {
            name: name.clone(),
            tokens: current_tokens,
            net_lines_dashed: current_dashed,
        });
        self.pcb_theme = PcbTheme::Custom;
        self.custom_pcb_theme_name = Some(name);
        self.custom_pcb_tokens = Some(current_tokens);
        self.net_lines_dashed = current_dashed;
    }

    pub fn load() -> Self {
        Self::load_from(&default_config_path())
    }

    pub fn add_recent_file(&mut self, path: PathBuf) {
        self.recent_files.retain(|p| p != &path);
        self.recent_files.insert(0, path);
        if self.recent_files.len() > 10 {
            self.recent_files.truncate(10);
        }
    }

    /// Remembers `pdf` as the schematic of `board` (most recent first, bounded). Returns whether
    /// the preferences changed.
    pub fn remember_board_schematic(&mut self, board: PathBuf, pdf: PathBuf) -> bool {
        let entry = BoardSchematic { board, pdf };
        if self.board_schematics.first() == Some(&entry) {
            return false;
        }
        self.board_schematics.retain(|e| e.board != entry.board);
        self.board_schematics.insert(0, entry);
        self.board_schematics.truncate(MAX_BOARD_SCHEMATICS);
        true
    }

    /// The schematic PDF remembered for `board`, if any.
    pub fn board_schematic(&self, board: &Path) -> Option<&Path> {
        self.board_schematics.iter().find(|e| e.board == board).map(|e| e.pdf.as_path())
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
        let version = value.get("version").and_then(Value::as_u64);
        if !matches!(version, Some(1..=PREFERENCES_VERSION)) {
            return Err(format!("{}: unsupported preferences version", path.display()));
        }
        let prefs_val = value
            .get("preferences")
            .ok_or_else(|| format!("{}: missing preferences object", path.display()))?;
        let mut prefs: Preferences = serde_json::from_value(prefs_val.clone())
            .map_err(|error| format!("{}: invalid preferences payload: {error}", path.display()))?;
        // Version 1 had no Auto and saved the forced English default: follow the system instead.
        if version == Some(1) && prefs.language == LanguagePref::Explicit(Language::English) {
            prefs.language = LanguagePref::Auto;
        }
        Ok(Some(prefs))
    }

    pub fn save_atomic(&self) -> Result<(), String> {
        self.save_atomic_to(&default_config_path())
    }

    pub fn save_atomic_to(&self, path: &Path) -> Result<(), String> {
        let json_value = serde_json::json!({
            "version": PREFERENCES_VERSION,
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

/// State of the Keyboard shortcuts section, kept in egui temporary memory between frames.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ShortcutCapture {
    /// Action waiting for its new key press.
    capturing: Option<Action>,
    /// Row action whose last capture or reset was refused, and the action already using the key.
    conflict: Option<(Action, Action)>,
}

impl ShortcutCapture {
    fn start(action: Action) -> Self {
        Self { capturing: Some(action), conflict: None }
    }

    /// Applies one key press; returns the next state and whether the keymap changed.
    fn press(self, keymap: &mut Keymap, key: egui::Key, mods: egui::Modifiers) -> (Self, bool) {
        let Some(action) = self.capturing else { return (self, false) };
        match keymap.apply_capture(action, key, mods) {
            CaptureOutcome::Assigned => (Self::default(), true),
            CaptureOutcome::Cancelled => (Self::default(), false),
            CaptureOutcome::Conflict(other) => (Self { capturing: None, conflict: Some((action, other)) }, false),
            CaptureOutcome::Ignored => (self, false),
        }
    }
}

fn shortcut_capture_id() -> egui::Id {
    egui::Id::unique("prefs_shortcut_capture")
}

fn shortcut_capture(ctx: &Context) -> ShortcutCapture {
    ctx.data(|d| d.get_temp(shortcut_capture_id())).unwrap_or_default()
}

fn set_shortcut_capture(ctx: &Context, state: ShortcutCapture) {
    ctx.data_mut(|d| d.insert_temp(shortcut_capture_id(), state));
}

fn new_theme_dialog_id() -> egui::Id {
    egui::Id::unique("prefs_new_theme_dialog_state")
}

fn get_new_theme_dialog(ctx: &Context) -> Option<String> {
    ctx.data(|d| d.get_temp(new_theme_dialog_id()))
}

fn set_new_theme_dialog(ctx: &Context, name: Option<String>) {
    ctx.data_mut(|d| {
        if let Some(n) = name {
            d.insert_temp(new_theme_dialog_id(), n);
        } else {
            d.remove_temp::<String>(new_theme_dialog_id());
        }
    });
}

/// Feeds this frame's key presses to an active shortcut capture of the Preferences window.
///
/// Call it once per frame before any hotkey handling: while capturing, every key press is
/// consumed so neither the board shortcuts, the Preferences toggle nor the window's Esc-to-close
/// react to it. Returns whether `keymap` changed.
pub fn capture_shortcut_keys(ctx: &Context, keymap: &mut Keymap) -> bool {
    let mut state = shortcut_capture(ctx);
    if state.capturing.is_none() {
        return false;
    }
    let presses: Vec<(egui::Key, egui::Modifiers)> = ctx.input_mut(|i| {
        let mut presses = Vec::new();
        i.events.retain(|event| match event {
            egui::Event::Key { key, pressed, repeat, modifiers, .. } => {
                if *pressed && !*repeat {
                    presses.push((*key, *modifiers));
                }
                false
            }
            _ => true,
        });
        presses
    });
    let mut changed = false;
    for (key, mods) in presses {
        let (next, assigned) = state.press(keymap, key, mods);
        state = next;
        changed |= assigned;
    }
    set_shortcut_capture(ctx, state);
    changed
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

    // Check Escape key to close. While a shortcut is being captured, `capture_shortcut_keys`
    // has already consumed this frame's Esc (it cancels the capture instead).
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        *open = false;
        set_shortcut_capture(ctx, ShortcutCapture::default());
        set_new_theme_dialog(ctx, None);
        return false;
    }

    let mut is_open = *open;
    let mut close_requested = false;

    Window::new(RichText::new("Preferences").strong().color(tokens.accent()))
        .open(&mut is_open)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .resizable(true)
        .min_size(Vec2::new(420.0, 320.0))
        .default_size(Vec2::new(460.0, 520.0))
        .show(ctx, |ui| {
            let lang = prefs.ui_language();
            ui.add_space(4.0);
            // The body scrolls; the Close row below stays visible.
            let footer = ui.spacing().interact_size.y + 24.0;
            egui::ScrollArea::vertical()
                .max_height((ui.available_height() - footer).max(80.0))
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    // Language Selector: Auto (system) first, then each language by its own name.
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(i18n::tr(lang, "prefs.language")).strong().color(tokens.text()));
                        let auto_label = i18n::tr(lang, "prefs.language_auto");
                        let selected = match prefs.language {
                            LanguagePref::Auto => auto_label,
                            LanguagePref::Explicit(l) => l.display_name(),
                        };
                        egui::ComboBox::from_id_salt("language_combo")
                            .selected_text(selected)
                            .show_ui(ui, |ui| {
                                if ui.selectable_value(&mut prefs.language, LanguagePref::Auto, auto_label).clicked() {
                                    changed = true;
                                }
                                for l in Language::ALL {
                                    if ui
                                        .selectable_value(&mut prefs.language, LanguagePref::Explicit(l), l.display_name())
                                        .clicked()
                                    {
                                        changed = true;
                                    }
                                }
                            });
                    });

                    ui.add_space(8.0);
                    ui.heading(RichText::new(i18n::tr(lang, "prefs.appearance")).color(tokens.text()));
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label(RichText::new(i18n::tr(lang, "prefs.side_panel_position")).strong().color(tokens.text()));
                        if ui
                            .selectable_value(&mut prefs.side_panel_left, true, i18n::tr(lang, "prefs.side_panel_left"))
                            .clicked()
                        {
                            changed = true;
                        }
                        if ui
                            .selectable_value(&mut prefs.side_panel_left, false, i18n::tr(lang, "prefs.side_panel_right"))
                            .clicked()
                        {
                            changed = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label(RichText::new(i18n::tr(lang, "prefs.app_theme")).strong().color(tokens.text()));
                        egui::ComboBox::from_id_salt("app_theme_combo")
                            .selected_text(prefs.app_theme.display_name())
                            .show_ui(ui, |ui| {
                                for theme in AppTheme::ALL {
                                    if ui
                                        .selectable_value(&mut prefs.app_theme, theme, theme.display_name())
                                        .clicked()
                                    {
                                        changed = true;
                                    }
                                }
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.label(RichText::new(i18n::tr(lang, "prefs.pcb_theme")).strong().color(tokens.text()));
                        let active_name = prefs.active_theme_display_name();
                        egui::ComboBox::from_id_salt("pcb_theme_combo")
                            .selected_text(&active_name)
                            .show_ui(ui, |ui| {
                                for theme in PcbTheme::BUILTIN {
                                    let is_selected = prefs.pcb_theme == theme;
                                    let label = if prefs.theme_overrides.contains_key(theme.display_name())
                                        || prefs.theme_dashed_overrides.contains_key(theme.display_name())
                                    {
                                        format!("{} *", theme.display_name())
                                    } else {
                                        theme.display_name().to_string()
                                    };
                                    if ui.selectable_label(is_selected, label).clicked() {
                                        prefs.select_builtin_theme(theme);
                                        changed = true;
                                    }
                                }
                                if !prefs.custom_pcb_themes.is_empty() {
                                    ui.separator();
                                    let mut selected_custom_theme = None;
                                    for custom in &prefs.custom_pcb_themes {
                                        let is_selected = prefs.pcb_theme == PcbTheme::Custom
                                            && prefs.custom_pcb_theme_name.as_deref() == Some(&custom.name);
                                        if ui.selectable_label(is_selected, &custom.name).clicked() {
                                            selected_custom_theme = Some(custom.name.clone());
                                        }
                                    }
                                    if let Some(name) = selected_custom_theme {
                                        prefs.select_custom_theme(&name);
                                        changed = true;
                                    }
                                }
                            });

                        if ui.button(format!("+ {}", i18n::tr(lang, "prefs.new_theme"))).clicked() {
                            let default_name = format!("{} (Copy)", prefs.active_theme_display_name().trim_end_matches(" *"));
                            set_new_theme_dialog(ctx, Some(default_name));
                            ctx.request_repaint();
                        }
                    });

                    if let Some(mut theme_name_buf) = get_new_theme_dialog(ctx) {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(i18n::tr(lang, "prefs.new_theme_name")).color(tokens.text()));
                            let text_res = ui.text_edit_singleline(&mut theme_name_buf);
                            if text_res.changed() {
                                set_new_theme_dialog(ctx, Some(theme_name_buf.clone()));
                            }
                            let enter_pressed = text_res.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter));
                            if ui.button(i18n::tr(lang, "prefs.create_theme")).clicked() || enter_pressed {
                                let name_to_add = theme_name_buf.trim().to_string();
                                if !name_to_add.is_empty() {
                                    prefs.create_new_theme_from_current(name_to_add);
                                    changed = true;
                                }
                                set_new_theme_dialog(ctx, None);
                            }
                            if ui.button(i18n::tr(lang, "prefs.cancel")).clicked() {
                                set_new_theme_dialog(ctx, None);
                            }
                        });
                    }

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(i18n::tr(lang, "prefs.theme_options")).strong().color(tokens.accent()));
                        if prefs.is_builtin_theme_modified() {
                            if ui.button(i18n::tr(lang, "prefs.reset_theme")).clicked() {
                                prefs.reset_active_built_in_theme();
                                changed = true;
                            }
                        }
                        if prefs.pcb_theme == PcbTheme::Custom {
                            if let Some(name) = prefs.custom_pcb_theme_name.clone() {
                                if ui.button(i18n::tr(lang, "prefs.delete_theme")).clicked() {
                                    prefs.delete_custom_theme(&name);
                                    changed = true;
                                }
                            }
                        }
                    });

                    // Theme dashed line option
                    let mut current_dashed = prefs.active_pcb_net_lines_dashed();
                    if ui
                        .checkbox(
                            &mut current_dashed,
                            RichText::new(i18n::tr(lang, "prefs.net_lines_dashed")).color(tokens.text()),
                        )
                        .changed()
                    {
                        prefs.set_active_pcb_net_lines_dashed(current_dashed);
                        changed = true;
                    }

                    // Theme Colors Grid
                    let mut active_tokens = prefs.active_pcb_tokens();
                    let mut tokens_modified = false;
                    egui::Grid::new("active_pcb_colors_grid")
                        .num_columns(2)
                        .spacing([12.0, 4.0])
                        .show(ui, |ui| {
                            ui.label(i18n::tr(lang, "prefs.color_canvas"));
                            let mut c = active_tokens.canvas.to_array();
                            if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                                active_tokens.canvas = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                                tokens_modified = true;
                            }
                            ui.end_row();

                            ui.label(i18n::tr(lang, "prefs.color_board_outline"));
                            let mut c = active_tokens.board_outline.to_array();
                            if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                                active_tokens.board_outline = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                                tokens_modified = true;
                            }
                            ui.end_row();

                            ui.label(i18n::tr(lang, "prefs.color_board_fill"));
                            let mut c = active_tokens.board_fill.to_array();
                            if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                                active_tokens.board_fill = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                                tokens_modified = true;
                            }
                            ui.end_row();

                            ui.label(i18n::tr(lang, "prefs.color_part_body"));
                            let mut c = active_tokens.part_body.to_array();
                            if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                                active_tokens.part_body = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                                tokens_modified = true;
                            }
                            ui.end_row();

                            ui.label(i18n::tr(lang, "prefs.color_part_body_line"));
                            let mut c = active_tokens.part_body_line.to_array();
                            if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                                active_tokens.part_body_line = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                                tokens_modified = true;
                            }
                            ui.end_row();

                            ui.label(i18n::tr(lang, "prefs.color_part_text"));
                            let mut c = active_tokens.part_text.to_array();
                            if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                                active_tokens.part_text = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                                tokens_modified = true;
                            }
                            ui.end_row();

                            ui.label(i18n::tr(lang, "prefs.color_pin_default"));
                            let mut c = active_tokens.pin_default.to_array();
                            if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                                active_tokens.pin_default = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                                tokens_modified = true;
                            }
                            ui.end_row();

                            ui.label(i18n::tr(lang, "prefs.color_pin_unconnected"));
                            let mut c = active_tokens.pin_unconnected.to_array();
                            if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                                active_tokens.pin_unconnected = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                                tokens_modified = true;
                            }
                            ui.end_row();

                            ui.label(i18n::tr(lang, "prefs.color_pin_cur_side"));
                            let mut c = active_tokens.pin_highlight_cur_side.to_array();
                            if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                                active_tokens.pin_highlight_cur_side = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                                tokens_modified = true;
                            }
                            ui.end_row();

                            ui.label(i18n::tr(lang, "prefs.color_pin_opp_side"));
                            let mut c = active_tokens.pin_highlight_opp_side.to_array();
                            if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                                active_tokens.pin_highlight_opp_side = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                                tokens_modified = true;
                            }
                            ui.end_row();

                            ui.label(i18n::tr(lang, "prefs.color_via_cur_side"));
                            let mut c = active_tokens.via_cur_side.to_array();
                            if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                                active_tokens.via_cur_side = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                                tokens_modified = true;
                            }
                            ui.end_row();

                            ui.label(i18n::tr(lang, "prefs.color_via_opp_side"));
                            let mut c = active_tokens.via_opp_side.to_array();
                            if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                                active_tokens.via_opp_side = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                                tokens_modified = true;
                            }
                            ui.end_row();

                            ui.label(i18n::tr(lang, "prefs.color_via_highlight"));
                            let mut c = active_tokens.via_highlight.to_array();
                            if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                                active_tokens.via_highlight = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                                tokens_modified = true;
                            }
                            ui.end_row();

                            ui.label(i18n::tr(lang, "prefs.color_net_web"));
                            let mut c = active_tokens.net_web.to_array();
                            if ui.color_edit_button_srgba_unmultiplied(&mut c).changed() {
                                active_tokens.net_web = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                                tokens_modified = true;
                            }
                            ui.end_row();
                        });

                    if tokens_modified {
                        prefs.set_active_pcb_tokens(active_tokens);
                        changed = true;
                    }

                    ui.add_space(12.0);
                    ui.heading(RichText::new(i18n::tr(lang, "prefs.visualization")).color(tokens.text()));
                    ui.separator();

                    if ui
                        .checkbox(
                            &mut prefs.show_net_web,
                            RichText::new(i18n::tr(lang, "prefs.show_net_web")).color(tokens.text()),
                        )
                        .on_hover_text("Draw connecting spiderweb lines between pins and vias on the selected net.")
                        .changed()
                    {
                        changed = true;
                    }

                    if prefs.show_net_web {
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.label(RichText::new(i18n::tr(lang, "prefs.via_display_mode")).color(tokens.text()));
                            egui::ComboBox::from_id_salt("via_display_mode_combo")
                                .selected_text(prefs.via_display_mode.display_name(lang))
                                .show_ui(ui, |ui| {
                                    for mode in ViaDisplayMode::ALL {
                                        if ui
                                            .selectable_value(&mut prefs.via_display_mode, mode, mode.display_name(lang))
                                            .clicked()
                                        {
                                            changed = true;
                                        }
                                    }
                                });
                        });

                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            if ui
                                .checkbox(
                                    &mut prefs.net_lines_dashed,
                                    RichText::new(i18n::tr(lang, "prefs.net_lines_dashed")).color(tokens.text()),
                                )
                                .on_hover_text("Render net flight lines with dashed styling to reduce visual clutter.")
                                .changed()
                            {
                                changed = true;
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.label(RichText::new(i18n::tr(lang, "prefs.cross_layer_transparency")).color(tokens.text()));
                            let mut pct = (prefs.cross_layer_transparency * 100.0).round() as u32;
                            if ui.add(egui::Slider::new(&mut pct, 0..=100).suffix("%")).changed() {
                                prefs.cross_layer_transparency = pct as f32 / 100.0;
                                changed = true;
                            }
                        });
                    }

                    if ui
                        .checkbox(
                            &mut prefs.show_part_labels,
                            RichText::new(i18n::tr(lang, "prefs.show_part_labels")).color(tokens.text()),
                        )
                        .on_hover_text("Render part name labels on components when zoomed in.")
                        .changed()
                    {
                        changed = true;
                    }

                    if ui
                        .checkbox(
                            &mut prefs.show_pin_annotations,
                            RichText::new(i18n::tr(lang, "prefs.show_pin_annotations")).color(tokens.text()),
                        )
                        .on_hover_text("Render pin numbers and descriptions when zooming into components.")
                        .changed()
                    {
                        changed = true;
                    }

                    if ui
                        .checkbox(
                            &mut prefs.high_density_lod,
                            RichText::new(i18n::tr(lang, "prefs.adaptive_lod")).color(tokens.text()),
                        )
                        .on_hover_text("Simplify circles to quads/points on dense nets and high zoom outs for 60+ FPS performance.")
                        .changed()
                    {
                        changed = true;
                    }

                    if ui
                        .checkbox(
                            &mut prefs.show_opposite_elements,
                            RichText::new(i18n::tr(lang, "prefs.show_opposite")).color(tokens.text()),
                        )
                        .on_hover_text("When enabled, dimmed pins and vias on the reverse side of the board are visible.")
                        .changed()
                    {
                        changed = true;
                    }

                    if ui
                        .checkbox(
                            &mut prefs.show_pins,
                            RichText::new(i18n::tr(lang, "prefs.show_pins")).color(tokens.text()),
                        )
                        .on_hover_text("Render component pins and pads on the board.")
                        .changed()
                    {
                        changed = true;
                    }

                    if ui
                        .checkbox(
                            &mut prefs.show_vias,
                            RichText::new(i18n::tr(lang, "prefs.show_vias")).color(tokens.text()),
                        )
                        .on_hover_text("Render PCB vias and test probe nails on the board.")
                        .changed()
                    {
                        changed = true;
                    }

                    if ui
                        .checkbox(
                            &mut prefs.show_unnamed_vias,
                            RichText::new(i18n::tr(lang, "prefs.show_unnamed_vias")).color(tokens.text()),
                        )
                        .on_hover_text("When disabled, vias and test pads with empty net names are hidden to reduce clutter.")
                        .changed()
                    {
                        changed = true;
                    }

                    ui.add_space(12.0);
                    egui::CollapsingHeader::new(
                        RichText::new(i18n::tr(lang, "prefs.shortcuts")).heading().color(tokens.text()),
                    )
                    .id_salt("prefs_shortcuts")
                    .show(ui, |ui| {
                        if show_shortcuts_section(ui, &mut prefs.keymap, lang, tokens) {
                            changed = true;
                        }
                    });
                });

            ui.separator();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(i18n::tr(lang, "prefs.close")).clicked() {
                    close_requested = true;
                }
            });
        });

    if !is_open || close_requested {
        *open = false;
        set_shortcut_capture(ctx, ShortcutCapture::default());
        set_new_theme_dialog(ctx, None);
    }

    changed
}

/// Keyboard shortcuts section: one row per action with its binding (click to capture a new
/// one) and a reset button, the conflict message of the refused row, and "Reset all".
/// Returns whether the keymap changed.
fn show_shortcuts_section(ui: &mut egui::Ui, keymap: &mut Keymap, lang: Language, tokens: &Tokens) -> bool {
    let mut state = shortcut_capture(ui.ctx());
    let mut changed = false;
    let error = ui.visuals().error_fg_color;
    egui::Grid::new("shortcuts_grid").num_columns(3).spacing([12.0, 4.0]).show(ui, |ui| {
        for action in Action::ALL {
            ui.label(RichText::new(i18n::tr(lang, action.i18n_key())).color(tokens.text()));
            let capturing = state.capturing == Some(action);
            let text = if capturing { i18n::tr(lang, "prefs.shortcut_press_key").to_string() } else { keymap.display(action) };
            let button = ui
                .add(egui::Button::new(text).selected(capturing))
                .on_hover_text(i18n::tr(lang, "prefs.shortcut_capture_hover"));
            if button.clicked() {
                state = if capturing { ShortcutCapture::default() } else { ShortcutCapture::start(action) };
                // A focused button would also "click" on Space/Enter, the keys being captured.
                button.surrender_focus();
            }
            let reset = egui::Button::new(i18n::tr(lang, "prefs.shortcut_reset"));
            if ui.add_enabled(!keymap.is_default(action), reset).clicked() {
                state = match keymap.try_reset(action) {
                    Ok(()) => {
                        changed = true;
                        ShortcutCapture::default()
                    }
                    Err(other) => ShortcutCapture { capturing: None, conflict: Some((action, other)) },
                };
            }
            ui.end_row();
            if let Some((_, other)) = state.conflict.filter(|&(row, _)| row == action) {
                ui.label("");
                let message = i18n::tr(lang, "prefs.shortcut_conflict").replace("{action}", i18n::tr(lang, other.i18n_key()));
                ui.label(RichText::new(message).color(error));
                ui.end_row();
            }
        }
    });
    ui.add_space(6.0);
    let all_default = Action::ALL.iter().all(|&action| keymap.is_default(action));
    if ui.add_enabled(!all_default, egui::Button::new(i18n::tr(lang, "prefs.shortcut_reset_all"))).clicked() {
        keymap.reset_all();
        state = ShortcutCapture::default();
        changed = true;
    }
    set_shortcut_capture(ui.ctx(), state);
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preferences_serde() {
        let prefs = Preferences {
            app_theme: AppTheme::Light,
            pcb_theme: PcbTheme::Custom,
            custom_pcb_tokens: Some(PcbTokens::DARK),
            custom_pcb_theme_name: Some("My Custom PCB".to_string()),
            language: LanguagePref::Explicit(Language::Catalan),
            show_net_web: false,
            via_display_mode: ViaDisplayMode::Radial,
            net_lines_dashed: true,
            cross_layer_transparency: 0.60,
            show_part_labels: true,
            show_pin_annotations: false,
            high_density_lod: true,
            show_opposite_elements: false,
            show_vias: true,
            show_unnamed_vias: false,
            show_pins: true,
            recent_files: vec![PathBuf::from("/path/to/board.cad")],
            board_schematics: vec![BoardSchematic {
                board: PathBuf::from("/path/to/board.cad"),
                pdf: PathBuf::from("/path/to/board.pdf"),
            }],
            board_tab: BoardTab::Pdf,
            both_layout: BothLayout::Vertical,
            split_horizontal: false,
            split_ratio: 0.6,
            split_side: Side::Bottom,
            view_divisions: 2,
            pane_first: PaneContent::Both,
            pane_second: PaneContent::Pdf,
            lock_zoom: true,
            side_panel_left: false,
            keymap: Keymap::default(),
            ..Preferences::default()
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
        prefs.language = LanguagePref::Explicit(Language::Spanish);
        prefs.show_net_web = false;
        prefs.via_display_mode = ViaDisplayMode::Radial;
        prefs.app_theme = AppTheme::Light;
        prefs.pcb_theme = PcbTheme::Custom;
        prefs.custom_pcb_tokens = Some(PcbTokens::DARK);

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

    #[test]
    fn board_schematics_default_when_missing_from_saved_preferences() {
        let mut value = serde_json::to_value(Preferences::default()).unwrap();
        value.as_object_mut().unwrap().remove("board_schematics");
        let prefs: Preferences = serde_json::from_value(value).expect("old preferences still load");
        assert!(prefs.board_schematics.is_empty());
    }

    #[test]
    fn keymap_defaults_when_missing_and_round_trips_rebinds() {
        use crate::keymap::{Action, Binding, Mods};
        let mut value = serde_json::to_value(Preferences::default()).unwrap();
        value.as_object_mut().unwrap().remove("keymap");
        let prefs: Preferences = serde_json::from_value(value).expect("old preferences still load");
        assert_eq!(prefs.keymap, Keymap::default());

        let mut saved = Preferences::default();
        saved.keymap.set(Action::FitView, Binding::new(egui::Key::G, Mods::SHIFT));
        let loaded: Preferences = serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
        assert_eq!(loaded.keymap.bindings(Action::FitView), &[Binding::new(egui::Key::G, Mods::SHIFT)]);
        assert_eq!(loaded, saved);
    }

    #[test]
    fn board_tab_and_layout_default_and_round_trip() {
        let mut value = serde_json::to_value(Preferences::default()).unwrap();
        let object = value.as_object_mut().unwrap();
        object.remove("board_tab");
        object.remove("both_layout");
        object.remove("split_side");
        object.remove("view_divisions");
        object.remove("pane_first");
        object.remove("pane_second");
        object.remove("lock_zoom");
        let prefs: Preferences = serde_json::from_value(value).expect("old preferences still load");
        assert_eq!((prefs.board_tab, prefs.both_layout, prefs.split_side), (BoardTab::Top, BothLayout::Auto, Side::Top));
        assert_eq!((prefs.view_divisions, prefs.pane_first, prefs.pane_second, prefs.lock_zoom), (1, PaneContent::Top, PaneContent::Pdf, false));

        let saved = Preferences {
            board_tab: BoardTab::Both,
            both_layout: BothLayout::Horizontal,
            split_side: Side::Bottom,
            view_divisions: 2,
            pane_first: PaneContent::Both,
            pane_second: PaneContent::Bottom,
            lock_zoom: true,
            ..Preferences::default()
        };
        let loaded: Preferences = serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
        assert_eq!((loaded.board_tab, loaded.both_layout, loaded.split_side), (BoardTab::Both, BothLayout::Horizontal, Side::Bottom));
        assert_eq!((loaded.view_divisions, loaded.pane_first, loaded.pane_second, loaded.lock_zoom), (2, PaneContent::Both, PaneContent::Bottom, true));
    }

    #[test]
    fn board_schematic_insert_lookup_and_replace() {
        let mut prefs = Preferences::default();
        let a = PathBuf::from("/boards/a.cad");
        assert_eq!(prefs.board_schematic(&a), None);
        assert!(prefs.remember_board_schematic(a.clone(), PathBuf::from("/boards/a.pdf")));
        assert!(prefs.remember_board_schematic(PathBuf::from("/boards/b.cad"), PathBuf::from("/b.pdf")));
        assert_eq!(prefs.board_schematic(&a), Some(Path::new("/boards/a.pdf")));
        // Reopening the same pair at the front changes nothing.
        assert!(!prefs.remember_board_schematic(PathBuf::from("/boards/b.cad"), PathBuf::from("/b.pdf")));
        // A new PDF for a board replaces its entry and moves it to the front.
        assert!(prefs.remember_board_schematic(a.clone(), PathBuf::from("/other/a_rev2.pdf")));
        assert_eq!(prefs.board_schematic(&a), Some(Path::new("/other/a_rev2.pdf")));
        assert_eq!(prefs.board_schematics.len(), 2);
        assert_eq!(prefs.board_schematics[0].board, a);
    }

    #[test]
    fn board_schematics_evict_least_recent_beyond_cap() {
        let mut prefs = Preferences::default();
        for i in 0..MAX_BOARD_SCHEMATICS + 5 {
            prefs.remember_board_schematic(PathBuf::from(format!("/b{i}.cad")), PathBuf::from(format!("/b{i}.pdf")));
        }
        assert_eq!(prefs.board_schematics.len(), MAX_BOARD_SCHEMATICS);
        assert_eq!(prefs.board_schematic(Path::new("/b0.cad")), None, "oldest evicted");
        assert_eq!(prefs.board_schematic(Path::new("/b4.cad")), None);
        assert!(prefs.board_schematic(Path::new("/b5.cad")).is_some());
        let newest = format!("/b{}.cad", MAX_BOARD_SCHEMATICS + 4);
        assert_eq!(prefs.board_schematics[0].board, PathBuf::from(newest));
    }

    #[test]
    fn test_add_recent_file_dedup_and_cap() {
        let mut prefs = Preferences::default();
        for i in 1..=15 {
            prefs.add_recent_file(PathBuf::from(format!("/board_{i}.cad")));
        }
        assert_eq!(prefs.recent_files.len(), 10);
        assert_eq!(prefs.recent_files[0], PathBuf::from("/board_15.cad"));

        // Re-adding existing pushes to top
        prefs.add_recent_file(PathBuf::from("/board_10.cad"));
        assert_eq!(prefs.recent_files.len(), 10);
        assert_eq!(prefs.recent_files[0], PathBuf::from("/board_10.cad"));
    }

    #[test]
    fn test_default_language_is_auto() {
        assert_eq!(Preferences::default().language, LanguagePref::Auto);
    }

    fn load_json(json: &serde_json::Value) -> Preferences {
        let dir = std::env::temp_dir().join(format!(
            "bvc_test_prefs_lang_{}_{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("preferences.json");
        fs::write(&file, serde_json::to_vec(json).unwrap()).unwrap();
        let prefs = Preferences::read_file(&file).expect("valid preferences").expect("file exists");
        let _ = fs::remove_dir_all(&dir);
        prefs
    }

    fn v1_payload(language: &str) -> serde_json::Value {
        let mut value = serde_json::to_value(Preferences::default()).unwrap();
        value["language"] = serde_json::Value::String(language.to_string());
        serde_json::json!({ "version": 1, "preferences": value })
    }

    #[test]
    fn test_v1_english_migrates_to_auto() {
        assert_eq!(load_json(&v1_payload("English")).language, LanguagePref::Auto);
    }

    #[test]
    fn test_v1_explicit_language_is_kept() {
        assert_eq!(load_json(&v1_payload("Spanish")).language, LanguagePref::Explicit(Language::Spanish));
    }

    #[test]
    fn test_v2_explicit_english_is_kept_and_saved_as_v2() {
        let temp_dir = std::env::temp_dir().join(format!("bvc_test_prefs_v2_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        let file = temp_dir.join("preferences.json");
        let prefs = Preferences { language: LanguagePref::Explicit(Language::English), ..Preferences::default() };
        prefs.save_atomic_to(&file).unwrap();
        let saved: serde_json::Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        assert_eq!(saved["version"], 2);
        assert_eq!(Preferences::load_from(&file), prefs);
        let auto = Preferences::default();
        auto.save_atomic_to(&file).unwrap();
        assert_eq!(Preferences::load_from(&file).language, LanguagePref::Auto);
        let _ = fs::remove_dir_all(&temp_dir);
    }

    fn press_event(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
    }

    /// Runs one frame with `events` (each press followed by its release, so the next frame's
    /// press is not a repeat), feeding them to [`capture_shortcut_keys`]; returns whether the
    /// keymap changed, and whether the Preferences toggle and Esc were still visible afterwards.
    fn capture_frame(
        ctx: &Context,
        keymap: &mut Keymap,
        mut events: Vec<egui::Event>,
    ) -> (bool, bool, bool) {
        let releases: Vec<egui::Event> = events
            .iter()
            .filter_map(|event| match event {
                egui::Event::Key { key, modifiers, .. } => {
                    Some(egui::Event::Key { key: *key, physical_key: None, pressed: false, repeat: false, modifiers: *modifiers })
                }
                _ => None,
            })
            .collect();
        events.extend(releases);
        let raw = egui::RawInput { events, ..Default::default() };
        let mut out = (false, false, false);
        let mut output = ctx.run_ui(raw, |ui| {
            let changed = capture_shortcut_keys(ui.ctx(), keymap);
            let toggle = ui.input(|i| keymap.pressed(Action::TogglePreferences, i));
            let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
            out = (changed, toggle, esc);
        });
        output.textures_delta.clear();
        out
    }

    #[test]
    fn shortcut_capture_state_transitions() {
        use egui::{Key, Modifiers};
        let mut km = Keymap::default();
        let capturing = ShortcutCapture::start(Action::FitView);
        assert_eq!(capturing.press(&mut km, Key::Enter, Modifiers::NONE), (capturing, false));
        let (state, changed) = capturing.press(&mut km, Key::R, Modifiers::NONE);
        assert_eq!(state, ShortcutCapture { capturing: None, conflict: Some((Action::FitView, Action::RotateCw)) });
        assert!(!changed);
        // A new capture clears the conflict message; Esc cancels.
        let capturing = ShortcutCapture::start(Action::FitView);
        assert_eq!(capturing.press(&mut km, Key::Escape, Modifiers::NONE), (ShortcutCapture::default(), false));
        assert_eq!(capturing.press(&mut km, Key::G, Modifiers::NONE), (ShortcutCapture::default(), true));
        assert_eq!(km.display(Action::FitView), "G");
        // Without an active capture presses are ignored.
        assert_eq!(ShortcutCapture::default().press(&mut km, Key::H, Modifiers::NONE), (ShortcutCapture::default(), false));
    }

    #[test]
    fn capture_consumes_presses_so_hotkeys_and_esc_do_not_fire() {
        use egui::{Key, Modifiers};
        let ctx = Context::default();
        let mut km = Keymap::default();

        // Not capturing: nothing is consumed.
        let toggle = press_event(Key::Comma, Modifiers::COMMAND);
        assert_eq!(capture_frame(&ctx, &mut km, vec![toggle.clone()]), (false, true, false));

        // Capturing Rotate CW: Ctrl/Cmd+, conflicts and must not toggle Preferences.
        set_shortcut_capture(&ctx, ShortcutCapture::start(Action::RotateCw));
        assert_eq!(capture_frame(&ctx, &mut km, vec![toggle]), (false, false, false));
        assert_eq!(shortcut_capture(&ctx).conflict, Some((Action::RotateCw, Action::TogglePreferences)));

        // Esc cancels the capture and is not seen by the window's Esc-to-close.
        set_shortcut_capture(&ctx, ShortcutCapture::start(Action::RotateCw));
        assert_eq!(capture_frame(&ctx, &mut km, vec![press_event(Key::Escape, Modifiers::NONE)]), (false, false, false));
        assert_eq!(shortcut_capture(&ctx), ShortcutCapture::default());

        // A free key is assigned.
        set_shortcut_capture(&ctx, ShortcutCapture::start(Action::RotateCw));
        assert_eq!(capture_frame(&ctx, &mut km, vec![press_event(Key::T, Modifiers::NONE)]).0, true);
        assert_eq!(km.display(Action::RotateCw), "T");
        assert_eq!(shortcut_capture(&ctx), ShortcutCapture::default());
    }

    #[test]
    fn test_modify_builtin_theme_and_reset() {
        let mut prefs = Preferences::default();
        assert_eq!(prefs.pcb_theme, PcbTheme::BlueCarrot);
        assert!(!prefs.is_builtin_theme_modified());

        // Modify canvas color and dashed lines on Blue Carrot
        let mut tokens = prefs.active_pcb_tokens();
        tokens.canvas = Color32::from_rgb(0x12, 0x34, 0x56);
        prefs.set_active_pcb_tokens(tokens);
        prefs.set_active_pcb_net_lines_dashed(true);

        assert!(prefs.is_builtin_theme_modified());
        assert_eq!(prefs.active_pcb_tokens().canvas, Color32::from_rgb(0x12, 0x34, 0x56));
        assert!(prefs.active_pcb_net_lines_dashed());
        assert!(prefs.active_theme_display_name().contains('*'));

        // Reset
        prefs.reset_active_built_in_theme();
        assert!(!prefs.is_builtin_theme_modified());
        assert_eq!(prefs.active_pcb_tokens().canvas, PcbTokens::BLUE_CARROT.canvas);
        assert!(!prefs.active_pcb_net_lines_dashed());
        assert!(!prefs.active_theme_display_name().contains('*'));
    }

    #[test]
    fn test_create_new_theme_from_current_and_delete() {
        let mut prefs = Preferences::default();
        prefs.set_active_pcb_net_lines_dashed(true);

        // Create new theme
        prefs.create_new_theme_from_current("Studio Emerald".to_string());
        assert_eq!(prefs.pcb_theme, PcbTheme::Custom);
        assert_eq!(prefs.custom_pcb_theme_name.as_deref(), Some("Studio Emerald"));
        assert_eq!(prefs.custom_pcb_themes.len(), 1);
        assert_eq!(prefs.custom_pcb_themes[0].name, "Studio Emerald");
        assert!(prefs.active_pcb_net_lines_dashed());

        // Modify token in custom theme
        let mut tokens = prefs.active_pcb_tokens();
        tokens.via_opp_side = Color32::from_rgba_unmultiplied(0x11, 0x22, 0x33, 128);
        prefs.set_active_pcb_tokens(tokens);
        assert_eq!(prefs.active_pcb_tokens().via_opp_side, Color32::from_rgba_unmultiplied(0x11, 0x22, 0x33, 128));

        // Delete custom theme
        prefs.delete_custom_theme("Studio Emerald");
        assert_eq!(prefs.pcb_theme, PcbTheme::BlueCarrot);
        assert!(prefs.custom_pcb_themes.is_empty());
    }
}
