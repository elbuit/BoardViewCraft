# Feature: configurable-shortcuts

## Objective
COM-01: let the user rebind the frequent board keyboard shortcuts from Preferences, persisted in `preferences.json`.

## Problem / Why
User request 2026-11-01. Every shortcut is a hardcoded `i.key_pressed(egui::Key::X)` check (`crates/ui-egui/src/lib.rs:614-692`, `crates/ui-egui/src/canvas.rs:793-842`, `crates/ui-egui/src/pdf_tab.rs:164-195`). It is the only high-priority requirement left (`Especificacion_Pendientes_Alta_Prioridad.md`).

## Scope
Rebindable actions (defaults = today's keys):
- Rotate CW (R), Rotate CCW (Shift+R), Flip side (Space), Mirror X (M), Mirror Y (Shift+M), Fit view (F)
- Board zoom in (+ / =), zoom out (-), pan up/down/left/right (arrows)
- Toggle Preferences (Ctrl/Cmd+,)

Out of scope (stay fixed): Esc/Enter contextual behaviour, modal dialogs (Esc/Enter), PDF tab page/zoom keys, mouse chords (Ctrl+click, Alt drag).

## Constraints
- Same gating as today: board actions are ignored while a text field has focus or Preferences is open; pan/zoom only on the active canvas.
- Exact modifier matching (R must not fire on Shift+R and vice versa); Ctrl and Cmd treated as the platform command modifier.
- Old `preferences.json` files load unchanged: new field `#[serde(default)]`, no version bump. Unknown/invalid bindings fall back to defaults per action.
- One action may have a primary binding plus the existing alternate (zoom in `+` and `=`); a rebind replaces the action's bindings with the captured one.
- Conflicts: assigning a key already used by another action is refused with a visible message (no silent double binding).
- Hint texts ("(R)", "(F)", "(Space…)") are rendered from the live keymap, not baked into the 6 TSV catalogs.
- The key-capture widget must keep Esc from closing the Preferences window while capturing (Esc cancels the capture).
- Artifacts in English; i18n strings added to all 6 catalogs (en, es, ca, fr, de, it).

## Tasks
- [x] T1 — Keymap model: `crates/ui-egui/src/keymap.rs` (`Action` enum, `Binding {key, modifiers}`, `Keymap` with defaults, exact matching, conflict lookup, display text, serde with per-action fallback) + `Preferences.keymap` field + unit tests (RED first). Route: delegated writer.
- [ ] T2 — Wire `lib.rs` global hotkeys and `canvas.rs` pan/zoom through the keymap; render hint suffixes from the keymap and strip them from the TSVs; tests for dispatch. Route: delegated writer.
- [ ] T3 — Preferences UI "Keyboard shortcuts" section (scrollable): per-action row with current binding, click-to-capture, Esc cancels, conflict message, reset-one and reset-all; i18n keys in 6 catalogs + count test; README shortcut table note. Route: delegated writer.

## Acceptance criteria
- Defaults reproduce today's behaviour exactly.
- A rebound action fires with the new key, not with the old one; the change survives restart.
- Older preferences files load with default bindings.
- `cargo test -p bvc-ui-egui` passes; `cargo build --release` ok; clippy warning count does not grow.

## Delivery strategy
ask-on-risk. Forecast ~700 authored changed lines (T1 ~250, T2 ~200, T3 ~250) — above the ~400 budget, chain strategy `stacked-to-main` (user choice 2026-11-01). Planned slices: PR1 = T1, PR2 = T2 + T3 (re-slice if PR2 exceeds ~400).

## Progress
- Branch `feat/configurable-shortcuts` from `1ace614` (feat/app-icon).
- Clippy baseline (`cargo clippy -p bvc-ui-egui`): 139 warnings.
- Exploration done (read-only explorer): shortcut inventory, Preferences persistence (JSON v2, 64 KiB cap), Preferences window layout, i18n count test (158 keys, i18n.rs:270/273).

- T1 done (route: delegated writer, `## Allowed edit surfaces` keymap.rs, lib.rs mod line, preferences.rs). RED: 10 new tests (9 in `keymap::tests`, 1 in `preferences::tests`) compiled against `todo!()` stubs and all failed. GREEN: `cargo test -p bvc-ui-egui keymap` 10 passed; `cargo test -p bvc-ui-egui` 147 passed, 2 ignored; clippy 139 warnings (unchanged). keymap.rs is ~443 lines (~200 code, ~240 tests), above the ~250 forecast because of test coverage. Shift rule: a binding without Shift on a shifted-glyph key (`Plus`, `:`, `?`, `!`, `|`, `{`, `}`) ignores Shift, so `+` zooms however the layout produces it; all other modifiers match exactly, and a macOS Control that is not the command modifier never matches. Bindings persist as strings (`"Shift+R"`, `"Command+Comma"`) using egui key names. Commit: (recorded below).

## Next step
T2 (delegated writer): wire lib.rs hotkeys and canvas.rs pan/zoom through `Preferences.keymap`.
