//! Configurable keyboard shortcuts: the rebindable actions, their key bindings and the
//! keymap persisted in the preferences file.
//!
//! Bindings are stored as strings such as `"R"`, `"Shift+R"` or `"Command+Comma"`, using egui's
//! stable key names. Loading is lenient: anything unknown or unparsable falls back to the
//! action's default bindings instead of failing the preferences load.

use egui::{InputState, Key, Modifiers};
use serde::de::Deserializer;
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// A board action that can be bound to keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    RotateCw,
    RotateCcw,
    FlipSide,
    MirrorX,
    MirrorY,
    FitView,
    ZoomIn,
    ZoomOut,
    PanUp,
    PanDown,
    PanLeft,
    PanRight,
    TogglePreferences,
    Measure,
    NavBack,
    NavForward,
}

impl Action {
    /// Every action, in display order.
    pub const ALL: [Action; 16] = [
        Action::RotateCw,
        Action::RotateCcw,
        Action::FlipSide,
        Action::MirrorX,
        Action::MirrorY,
        Action::FitView,
        Action::ZoomIn,
        Action::ZoomOut,
        Action::PanUp,
        Action::PanDown,
        Action::PanLeft,
        Action::PanRight,
        Action::TogglePreferences,
        Action::Measure,
        Action::NavBack,
        Action::NavForward,
    ];

    /// Translation key of the action's label.
    pub fn i18n_key(self) -> &'static str {
        match self {
            Action::RotateCw => "shortcut.rotate_cw",
            Action::RotateCcw => "shortcut.rotate_ccw",
            Action::FlipSide => "shortcut.flip_side",
            Action::MirrorX => "shortcut.mirror_x",
            Action::MirrorY => "shortcut.mirror_y",
            Action::FitView => "shortcut.fit_view",
            Action::ZoomIn => "shortcut.zoom_in",
            Action::ZoomOut => "shortcut.zoom_out",
            Action::PanUp => "shortcut.pan_up",
            Action::PanDown => "shortcut.pan_down",
            Action::PanLeft => "shortcut.pan_left",
            Action::PanRight => "shortcut.pan_right",
            Action::TogglePreferences => "shortcut.toggle_preferences",
            Action::Measure => "shortcut.measure",
            Action::NavBack => "shortcut.nav_back",
            Action::NavForward => "shortcut.nav_forward",
        }
    }

    fn default_bindings(self) -> Vec<Binding> {
        let plain = |key| vec![Binding::new(key, Mods::NONE)];
        let shifted = |key| vec![Binding::new(key, Mods::SHIFT)];
        match self {
            Action::RotateCw => plain(Key::R),
            Action::RotateCcw => shifted(Key::R),
            Action::FlipSide => plain(Key::Space),
            Action::MirrorX => plain(Key::M),
            Action::MirrorY => shifted(Key::M),
            Action::FitView => plain(Key::F),
            Action::ZoomIn => vec![Binding::new(Key::Plus, Mods::NONE), Binding::new(Key::Equals, Mods::NONE)],
            Action::ZoomOut => plain(Key::Minus),
            Action::PanUp => plain(Key::ArrowUp),
            Action::PanDown => plain(Key::ArrowDown),
            Action::PanLeft => plain(Key::ArrowLeft),
            Action::PanRight => plain(Key::ArrowRight),
            Action::TogglePreferences => vec![Binding::new(Key::Comma, Mods::COMMAND)],
            Action::Measure => plain(Key::Q),
            Action::NavBack => vec![Binding::new(Key::ArrowLeft, Mods::ALT)],
            Action::NavForward => vec![Binding::new(Key::ArrowRight, Mods::ALT)],
        }
    }
}

/// Modifier state of a binding. `command` is the platform command modifier: Ctrl on
/// Linux/Windows, Cmd on macOS (egui's `Modifiers::command`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Mods {
    pub shift: bool,
    pub alt: bool,
    pub command: bool,
}

impl Mods {
    pub const NONE: Mods = Mods { shift: false, alt: false, command: false };
    pub const SHIFT: Mods = Mods { shift: true, alt: false, command: false };
    pub const COMMAND: Mods = Mods { shift: false, alt: false, command: true };
    pub const ALT: Mods = Mods { shift: false, alt: true, command: false };
}

/// One key plus the exact modifiers that must be held.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Binding {
    pub key: Key,
    pub modifiers: Mods,
}

impl Binding {
    pub const fn new(key: Key, modifiers: Mods) -> Self {
        Self { key, modifiers }
    }

    /// Whether this binding was pressed this frame (key-repeat included, like
    /// `InputState::key_pressed`), with exactly the bound modifiers.
    pub fn pressed(&self, input: &InputState) -> bool {
        input.events.iter().any(|event| {
            matches!(event, egui::Event::Key { key, pressed: true, modifiers, .. } if matches(self, *key, *modifiers))
        })
    }

    /// Human-readable form such as `"Shift+R"`, `"Ctrl+,"` (`"Cmd+,"` on macOS) or `"⏶"`.
    pub fn display(&self) -> String {
        let key = match self.key {
            Key::Minus => "-",
            key => key.symbol_or_name(),
        };
        let command = if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" };
        self.join(command, key)
    }

    /// Stable persisted form, e.g. `"Shift+R"` or `"Command+Comma"`.
    fn to_config(self) -> String {
        self.join("Command", self.key.name())
    }

    fn join(&self, command: &str, key: &str) -> String {
        let Mods { shift, alt, command: cmd } = self.modifiers;
        let mut parts: Vec<&str> = Vec::new();
        parts.extend(cmd.then_some(command));
        parts.extend(alt.then_some("Alt"));
        parts.extend(shift.then_some("Shift"));
        parts.push(key);
        parts.join("+")
    }

    /// Parses [`Binding::to_config`] output; `None` for anything unknown.
    fn from_config(text: &str) -> Option<Self> {
        let mut parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let key = Key::from_name(parts.pop()?)?;
        let mut modifiers = Mods::NONE;
        for part in parts {
            match part {
                "Shift" => modifiers.shift = true,
                "Alt" => modifiers.alt = true,
                "Command" => modifiers.command = true,
                _ => return None,
            }
        }
        Some(Self { key, modifiers })
    }
}

impl Binding {
    /// The binding a key press records when capturing a new shortcut. `None` for a press that
    /// cannot be bound: a Ctrl that is not the command modifier (macOS Control). Shift is dropped
    /// on [`shift_insensitive`] keys, whose glyph already implies it.
    pub fn from_press(key: Key, mods: Modifiers) -> Option<Self> {
        if mods.ctrl && !mods.command {
            return None;
        }
        let shift = mods.shift && !shift_insensitive(key);
        Some(Self::new(key, Mods { shift, alt: mods.alt, command: mods.command }))
    }
}

/// Result of feeding one key press to a shortcut capture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureOutcome {
    /// The press became the action's only binding.
    Assigned,
    /// The press is already bound to this other action; nothing changed.
    Conflict(Action),
    /// Esc: the capture ends without changes.
    Cancelled,
    /// The press cannot be bound (Enter, macOS Control); keep capturing.
    Ignored,
}

/// Keys whose glyph needs Shift on common keyboard layouts (e.g. `+` is Shift+`=` on US).
/// A binding to one of them without Shift ignores the Shift state, so the default zoom-in `+`
/// keeps working however the layout produces it.
fn shift_insensitive(key: Key) -> bool {
    matches!(
        key,
        Key::Plus | Key::Colon | Key::Questionmark | Key::Exclamationmark | Key::Pipe
            | Key::OpenCurlyBracket | Key::CloseCurlyBracket
    )
}

/// Whether a key event (`key` with `mods`) triggers `binding`.
///
/// Modifiers must match exactly, except for Shift on [`shift_insensitive`] keys bound without
/// Shift. A Ctrl that is not the command modifier (macOS Control) never matches.
pub fn matches(binding: &Binding, key: Key, mods: Modifiers) -> bool {
    let want = binding.modifiers;
    let extra_ctrl = mods.ctrl && !mods.command;
    let shift_ok = mods.shift == want.shift || (!want.shift && shift_insensitive(binding.key));
    binding.key == key && !extra_ctrl && shift_ok && mods.alt == want.alt && mods.command == want.command
}

/// The bindings of every action. Always holds at least one binding per action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keymap {
    bindings: BTreeMap<Action, Vec<Binding>>,
}

impl Default for Keymap {
    fn default() -> Self {
        Self { bindings: Action::ALL.iter().map(|&a| (a, a.default_bindings())).collect() }
    }
}

impl Keymap {
    pub fn bindings(&self, action: Action) -> &[Binding] {
        self.bindings.get(&action).map(Vec::as_slice).unwrap_or_default()
    }

    pub fn pressed(&self, action: Action, input: &InputState) -> bool {
        self.bindings(action).iter().any(|b| b.pressed(input))
    }

    /// Replaces the action's bindings with `binding`.
    pub fn set(&mut self, action: Action, binding: Binding) {
        self.bindings.insert(action, vec![binding]);
    }

    pub fn reset(&mut self, action: Action) {
        self.bindings.insert(action, action.default_bindings());
    }

    /// Restores the action's defaults unless one of them is now bound to another action, which
    /// is returned instead (nothing changes).
    pub fn try_reset(&mut self, action: Action) -> Result<(), Action> {
        let defaults = action.default_bindings();
        if let Some(other) = defaults.iter().find_map(|&binding| self.conflict(binding, action)) {
            return Err(other);
        }
        self.bindings.insert(action, defaults);
        Ok(())
    }

    /// Whether the action still has its default bindings.
    pub fn is_default(&self, action: Action) -> bool {
        self.bindings(action) == action.default_bindings().as_slice()
    }

    /// Feeds a captured key press for `action`. Esc cancels; Enter (fixed calibration key) and
    /// unbindable presses are ignored; a press bound to another action is refused.
    pub fn apply_capture(&mut self, action: Action, key: Key, mods: Modifiers) -> CaptureOutcome {
        if key == Key::Escape {
            return CaptureOutcome::Cancelled;
        }
        if key == Key::Enter {
            return CaptureOutcome::Ignored;
        }
        let Some(binding) = Binding::from_press(key, mods) else { return CaptureOutcome::Ignored };
        if let Some(other) = self.conflict(binding, action) {
            return CaptureOutcome::Conflict(other);
        }
        self.set(action, binding);
        CaptureOutcome::Assigned
    }

    pub fn reset_all(&mut self) {
        *self = Self::default();
    }

    /// The first action other than `except` already bound to `binding`.
    pub fn conflict(&self, binding: Binding, except: Action) -> Option<Action> {
        Action::ALL.into_iter().find(|&a| a != except && self.bindings(a).contains(&binding))
    }

    /// `label` followed by the action's shortcut in parentheses, e.g. `"Rotate 90° CW (R)"`.
    pub fn hint(&self, label: &str, action: Action) -> String {
        format!("{label} ({})", self.display(action))
    }

    /// All bindings of the action, joined by `" / "`.
    pub fn display(&self, action: Action) -> String {
        self.bindings(action).iter().map(Binding::display).collect::<Vec<_>>().join(" / ")
    }
}

impl Serialize for Keymap {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.bindings.len()))?;
        for (action, bindings) in &self.bindings {
            let list: Vec<String> = bindings.iter().map(|b| b.to_config()).collect();
            map.serialize_entry(action, &list)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Keymap {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Any JSON is accepted; each action keeps only its parsable bindings, else its defaults.
        let value = Value::deserialize(deserializer)?;
        let mut keymap = Keymap::default();
        let Some(entries) = value.as_object() else { return Ok(keymap) };
        for (name, list) in entries {
            let Ok(action) = serde_json::from_value::<Action>(Value::String(name.clone())) else { continue };
            let parsed: Vec<Binding> = list
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|item| item.as_str().and_then(Binding::from_config))
                .collect();
            if !parsed.is_empty() {
                keymap.bindings.insert(action, parsed);
            }
        }
        Ok(keymap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, RawInput};

    const CMD: &str = if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" };

    fn b(key: Key, modifiers: Mods) -> Binding {
        Binding::new(key, modifiers)
    }

    fn input_with(key: Key, modifiers: Modifiers) -> InputState {
        let raw = RawInput {
            events: vec![Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }],
            ..RawInput::default()
        };
        InputState::default().begin_pass(raw, false, 1.0, egui::InputOptions::default())
    }

    #[test]
    fn keymap_defaults_reproduce_todays_keys() {
        let km = Keymap::default();
        let expect = [
            (Action::RotateCw, vec![b(Key::R, Mods::NONE)]),
            (Action::RotateCcw, vec![b(Key::R, Mods::SHIFT)]),
            (Action::FlipSide, vec![b(Key::Space, Mods::NONE)]),
            (Action::MirrorX, vec![b(Key::M, Mods::NONE)]),
            (Action::MirrorY, vec![b(Key::M, Mods::SHIFT)]),
            (Action::FitView, vec![b(Key::F, Mods::NONE)]),
            (Action::ZoomIn, vec![b(Key::Plus, Mods::NONE), b(Key::Equals, Mods::NONE)]),
            (Action::ZoomOut, vec![b(Key::Minus, Mods::NONE)]),
            (Action::PanUp, vec![b(Key::ArrowUp, Mods::NONE)]),
            (Action::PanDown, vec![b(Key::ArrowDown, Mods::NONE)]),
            (Action::PanLeft, vec![b(Key::ArrowLeft, Mods::NONE)]),
            (Action::PanRight, vec![b(Key::ArrowRight, Mods::NONE)]),
            (Action::TogglePreferences, vec![b(Key::Comma, Mods::COMMAND)]),
            (Action::Measure, vec![b(Key::Q, Mods::NONE)]),
            (Action::NavBack, vec![b(Key::ArrowLeft, Mods::ALT)]),
            (Action::NavForward, vec![b(Key::ArrowRight, Mods::ALT)]),
        ];
        assert_eq!(expect.len(), Action::ALL.len());
        for (action, bindings) in expect {
            assert_eq!(km.bindings(action), bindings.as_slice(), "{action:?}");
        }
    }

    #[test]
    fn keymap_i18n_keys_use_snake_case_names() {
        assert_eq!(Action::RotateCw.i18n_key(), "shortcut.rotate_cw");
        assert_eq!(Action::TogglePreferences.i18n_key(), "shortcut.toggle_preferences");
        for action in Action::ALL {
            let name = serde_json::to_value(action).unwrap();
            assert_eq!(action.i18n_key(), format!("shortcut.{}", name.as_str().unwrap()));
        }
    }

    #[test]
    fn keymap_matching_is_exact_on_modifiers() {
        let r = b(Key::R, Mods::NONE);
        let shift_r = b(Key::R, Mods::SHIFT);
        assert!(matches(&r, Key::R, Modifiers::NONE));
        assert!(!matches(&r, Key::R, Modifiers::SHIFT));
        assert!(!matches(&r, Key::R, Modifiers::ALT));
        assert!(!matches(&r, Key::R, Modifiers::COMMAND));
        assert!(!matches(&r, Key::M, Modifiers::NONE));
        assert!(matches(&shift_r, Key::R, Modifiers::SHIFT));
        assert!(!matches(&shift_r, Key::R, Modifiers::NONE));
        // The command binding accepts the platform command modifier (Ctrl here, Cmd on macOS).
        let prefs = b(Key::Comma, Mods::COMMAND);
        assert!(matches(&prefs, Key::Comma, Modifiers::COMMAND));
        assert!(!matches(&prefs, Key::Comma, Modifiers::NONE));
        // A Ctrl that is not the command modifier (macOS Control) never matches.
        let mac_ctrl = Modifiers { ctrl: true, ..Modifiers::NONE };
        assert!(!matches(&r, Key::R, mac_ctrl));
    }

    #[test]
    fn keymap_shift_is_ignored_for_shifted_glyph_keys() {
        // `+` needs Shift on common layouts, so an unshifted Plus binding ignores Shift.
        let plus = b(Key::Plus, Mods::NONE);
        assert!(matches(&plus, Key::Plus, Modifiers::NONE));
        assert!(matches(&plus, Key::Plus, Modifiers::SHIFT));
        assert!(!matches(&plus, Key::Plus, Modifiers::COMMAND));
        // `=` is unshifted on common layouts: exact matching applies.
        let equals = b(Key::Equals, Mods::NONE);
        assert!(!matches(&equals, Key::Equals, Modifiers::SHIFT));
    }

    #[test]
    fn keymap_pressed_reads_input_events() {
        let km = Keymap::default();
        let shift_r = input_with(Key::R, Modifiers::SHIFT);
        assert!(km.pressed(Action::RotateCcw, &shift_r));
        assert!(!km.pressed(Action::RotateCw, &shift_r));
        let r = input_with(Key::R, Modifiers::NONE);
        assert!(km.pressed(Action::RotateCw, &r));
        assert!(!km.pressed(Action::RotateCcw, &r));
        assert!(km.pressed(Action::ZoomIn, &input_with(Key::Equals, Modifiers::NONE)));
        assert!(km.pressed(Action::ZoomIn, &input_with(Key::Plus, Modifiers::SHIFT)));
        assert!(km.pressed(Action::TogglePreferences, &input_with(Key::Comma, Modifiers::COMMAND)));
        assert!(!km.pressed(Action::FitView, &InputState::default()));
    }

    #[test]
    fn keymap_set_reset_and_conflicts() {
        let mut km = Keymap::default();
        let g = b(Key::G, Mods::NONE);
        km.set(Action::FitView, g);
        assert_eq!(km.bindings(Action::FitView), &[g]);
        km.set(Action::ZoomIn, b(Key::I, Mods::NONE));
        assert_eq!(km.bindings(Action::ZoomIn).len(), 1);

        assert_eq!(km.conflict(g, Action::RotateCw), Some(Action::FitView));
        assert_eq!(km.conflict(g, Action::FitView), None);
        assert_eq!(km.conflict(b(Key::R, Mods::SHIFT), Action::RotateCw), Some(Action::RotateCcw));
        assert_eq!(km.conflict(b(Key::F, Mods::NONE), Action::RotateCw), None);

        km.reset(Action::FitView);
        assert_eq!(km.bindings(Action::FitView), &[b(Key::F, Mods::NONE)]);
        assert_ne!(km, Keymap::default());
        km.reset_all();
        assert_eq!(km, Keymap::default());
    }

    #[test]
    fn keymap_display_strings() {
        let km = Keymap::default();
        assert_eq!(km.display(Action::RotateCw), "R");
        assert_eq!(km.display(Action::RotateCcw), "Shift+R");
        assert_eq!(km.display(Action::FlipSide), "Space");
        assert_eq!(km.display(Action::ZoomIn), "+ / =");
        assert_eq!(km.display(Action::ZoomOut), "-");
        assert_eq!(km.display(Action::PanUp), "⏶");
        assert_eq!(km.display(Action::PanLeft), "⏴");
        assert_eq!(km.display(Action::TogglePreferences), format!("{CMD}+,"));
        let all = Binding::new(Key::K, Mods { shift: true, alt: true, command: true });
        assert_eq!(all.display(), format!("{CMD}+Alt+Shift+K"));
    }

    #[test]
    fn keymap_hint_appends_the_live_binding() {
        let mut km = Keymap::default();
        assert_eq!(km.hint("Rotate", Action::RotateCw), "Rotate (R)");
        km.set(Action::RotateCw, b(Key::T, Mods::COMMAND));
        assert_eq!(km.hint("Rotate", Action::RotateCw), format!("Rotate ({CMD}+T)"));
        assert_eq!(km.hint("Zoom", Action::ZoomIn), "Zoom (+ / =)");
    }

    #[test]
    fn keymap_serializes_as_action_to_binding_list() {
        let mut km = Keymap::default();
        km.set(Action::RotateCw, b(Key::T, Mods { shift: false, alt: true, command: true }));
        let value = serde_json::to_value(&km).unwrap();
        assert_eq!(value["rotate_cw"], serde_json::json!(["Command+Alt+T"]));
        assert_eq!(value["zoom_in"], serde_json::json!(["Plus", "Equals"]));
        assert_eq!(value["toggle_preferences"], serde_json::json!(["Command+Comma"]));
        let back: Keymap = serde_json::from_value(value).unwrap();
        assert_eq!(back, km);
    }

    #[test]
    fn keymap_deserialize_is_lenient() {
        let value = serde_json::json!({
            "unknown_action": ["X"],
            "rotate_cw": ["NotAKey"],
            "fit_view": [],
            "mirror_x": ["Shift+Bogus+M", "Alt+K"],
            "zoom_out": "Minus",
            "flip_side": ["Shift+B"],
        });
        let km: Keymap = serde_json::from_value(value).unwrap();
        let def = Keymap::default();
        assert_eq!(km.bindings(Action::RotateCw), def.bindings(Action::RotateCw));
        assert_eq!(km.bindings(Action::FitView), def.bindings(Action::FitView));
        assert_eq!(km.bindings(Action::ZoomOut), def.bindings(Action::ZoomOut));
        assert_eq!(km.bindings(Action::MirrorX), &[b(Key::K, Mods { shift: false, alt: true, command: false })]);
        assert_eq!(km.bindings(Action::FlipSide), &[b(Key::B, Mods::SHIFT)]);
        assert_eq!(km.bindings(Action::PanUp), def.bindings(Action::PanUp));

        let not_a_map: Keymap = serde_json::from_value(serde_json::json!(42)).unwrap();
        assert_eq!(not_a_map, def);
    }

    #[test]
    fn capture_binding_from_press() {
        assert_eq!(Binding::from_press(Key::G, Modifiers::SHIFT), Some(b(Key::G, Mods::SHIFT)));
        let all = Modifiers { alt: true, shift: true, ..Modifiers::COMMAND };
        assert_eq!(Binding::from_press(Key::K, all), Some(b(Key::K, Mods { shift: true, alt: true, command: true })));
        // `+` arrives with Shift on many layouts: the glyph already implies it.
        assert_eq!(Binding::from_press(Key::Plus, Modifiers::SHIFT), Some(b(Key::Plus, Mods::NONE)));
        // A Control that is not the command modifier (macOS) cannot be represented.
        assert_eq!(Binding::from_press(Key::G, Modifiers { ctrl: true, ..Modifiers::NONE }), None);
    }

    #[test]
    fn capture_assigns_cancels_ignores_and_refuses_conflicts() {
        let mut km = Keymap::default();
        assert_eq!(km.apply_capture(Action::FitView, Key::G, Modifiers::COMMAND), CaptureOutcome::Assigned);
        assert_eq!(km.bindings(Action::FitView), &[b(Key::G, Mods::COMMAND)]);

        let before = km.clone();
        assert_eq!(km.apply_capture(Action::FitView, Key::R, Modifiers::SHIFT), CaptureOutcome::Conflict(Action::RotateCcw));
        assert_eq!(km.apply_capture(Action::FitView, Key::Escape, Modifiers::NONE), CaptureOutcome::Cancelled);
        assert_eq!(km.apply_capture(Action::FitView, Key::Enter, Modifiers::NONE), CaptureOutcome::Ignored);
        let mac_ctrl = Modifiers { ctrl: true, ..Modifiers::NONE };
        assert_eq!(km.apply_capture(Action::FitView, Key::H, mac_ctrl), CaptureOutcome::Ignored);
        assert_eq!(km, before);

        // Re-capturing an action's own key, or one of its alternates, is not a conflict.
        assert_eq!(km.apply_capture(Action::ZoomIn, Key::Equals, Modifiers::NONE), CaptureOutcome::Assigned);
        assert_eq!(km.bindings(Action::ZoomIn), &[b(Key::Equals, Mods::NONE)]);
    }

    #[test]
    fn capture_reset_and_default_state() {
        let mut km = Keymap::default();
        assert!(Action::ALL.iter().all(|&a| km.is_default(a)));
        km.set(Action::RotateCw, b(Key::G, Mods::NONE));
        assert!(!km.is_default(Action::RotateCw));
        assert!(km.is_default(Action::FitView));
        // R is free again; give it to Fit view, then resetting Rotate CW must be refused.
        km.set(Action::FitView, b(Key::R, Mods::NONE));
        assert_eq!(km.try_reset(Action::RotateCw), Err(Action::FitView));
        assert_eq!(km.bindings(Action::RotateCw), &[b(Key::G, Mods::NONE)]);
        assert_eq!(km.try_reset(Action::FitView), Ok(()));
        assert_eq!(km.try_reset(Action::RotateCw), Ok(()));
        assert_eq!(km, Keymap::default());
    }
}
