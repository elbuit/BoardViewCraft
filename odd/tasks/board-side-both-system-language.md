# Feature: board-side-both-system-language

## Objective
1. Replace the TOP/BOTTOM toolbar toggle with a dropdown offering Top, Bottom and Both (both board sides drawn at the same time).
2. Detect the system language at startup the same way pdfcraft does, with an "Auto (system)" choice in Preferences.

## Problem / Why
User request 2026-11-01. Today the side button only flips Top/Bottom, and the UI language defaults to English regardless of the OS locale.

## Scope
- `bvc_core::Side::Both` already exists; `visible_on(Both)` shows every element and picking already filters with it. Both renders unmirrored (like Top), with the user's mirror flags still applied.
- Toolbar: ComboBox/menu with Top / Bottom / Both (translated, new key `toolbar.view_both`), hover text translated. Space key keeps flipping Top <-> Bottom; from Both it goes to Top (unchanged). Status label already prints BOTH.
- Photo overlay in Both: keeps the existing rule (top photo slot).
- Language: preference becomes "auto or explicit language" (pdfcraft model: `from_pref` falls back to system language). Port pdfcraft's `candidates`, `lang_from_tag`, `detect_system_lang` (LC_ALL / LC_MESSAGES / LANG; macOS `defaults read -g AppleLanguages`; Windows `sys-locale 0.3` as a `cfg(windows)` dependency). Default is Auto. Old `preferences.json` files must still load.
- Branch `feat/board-side-both-system-language` stacked on `feat/pdf-deep-zoom-perf` (73e0380, not yet merged to main).

## Tasks
- [x] T1 — Side dropdown Top/Bottom/Both (route: delegated writer; lib.rs + canvas tests + 6 i18n catalogs)
- [x] T2 — System language detection with Auto preference (route: same delegated writer; i18n.rs + preferences.rs + Cargo.toml + catalogs)

## Acceptance criteria
- Choosing Both shows parts, pins and vias of both sides, unmirrored; hit-testing finds elements of either side.
- With no saved language (or Auto), the UI follows LC_ALL/LC_MESSAGES/LANG (e.g. `LANG=es_ES.UTF-8` -> Spanish); an explicit choice wins.
- `cargo test -p bvc-ui-egui` and `cargo test -p bvc-core` pass; no new clippy warnings versus base; `cargo build --release` succeeds.

## Delivery strategy
ask-on-risk. Forecast ~350 authored lines.

## Progress
- Branch created from `73e0380`. Exploration done (side model, overlay, i18n, pdfcraft detection).
- T1 done: toolbar toggle replaced by a ComboBox (`SIDE_CHOICES` Top/Bottom/Both, `side_label`), new keys `toolbar.view_both` and `toolbar.view_side_hover` in all 6 catalogs (key count 148 -> 150). Tests: canvas round-trip loop now covers `Side::Both`, new `test_both_side_is_unmirrored_like_top`, new `side_selector_tests`. RED: lib tests failed to compile (missing `SIDE_CHOICES`/`side_label`). GREEN: `cargo test -p bvc-ui-egui --release` 105 passed / 2 ignored; bvc-core 44 passed; clippy warning count 147 before and after. Commit: `c3c5d0b`.
- T2 done: `i18n::LanguagePref { Auto (default), Explicit(Language) }` serialized as `"auto"` or the variant name (codes accepted, unknown -> Auto); `resolve()` falls back to cached `system_lang()` (English under `cfg(test)`). Ported pdfcraft `candidates`/`lang_from_tag`/`detect_system_lang` (LC_ALL/LC_MESSAGES/LANG, macOS `defaults`, Windows `sys-locale 0.3` as `cfg(windows)` dep)/`language_override`/`first_supported`; added `Language::from_code`. `Preferences::ui_language()` used by lib.rs/canvas.rs; picker gets `prefs.language_auto` (6 catalogs, 151 keys). Preferences file version bumped to 2; v1 files load, v1 `English` migrates to Auto, other languages kept. RED: 44 compile errors (missing `LanguagePref`, `lang_from_tag`, `from_code`, ...). GREEN: `cargo test -p bvc-ui-egui --release` 115 passed / 2 ignored; bvc-core 44 passed; clippy warning count 147 before and after; `cargo build --release` ok. Commit: T2 commit `feat(i18n): detect system language with Auto preference` (hash in the follow-up docs commit / git log).
