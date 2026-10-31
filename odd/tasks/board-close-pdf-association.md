# Feature: board-close-pdf-association

## Objective
1. File > Close: close the current board (and its schematic PDF).
2. Each board has its own associated schematic PDF: opening another board closes the PDF that was open; the new board's PDF is opened from its remembered association, else the existing co-located PDF prompt applies.

## Problem / Why
User request 2026-10-31. Today opening another board keeps the previous board's PDF open; there is no Close entry and no per-board PDF memory (only the co-located-file prompt, `find_matching_schematic_pdf`, `pending_schematic_prompt` in `crates/ui-egui/src/lib.rs`).

## Scope
- File menu: "Close" entry (disabled without a board) that clears board, selection, overlays tied to the board, PDF viewer, filename/title, and resets the side panel.
- Association: when a PDF is opened while a board is loaded, persist `board path -> pdf path` in preferences (bounded, e.g. same 10-ish as recent files or a small map). On opening a board: close the current PDF; if an association exists and the PDF file still exists, open it automatically; otherwise fall back to the current co-located prompt.
- Strings in 6 catalogs; i18n key-count test update.
- Branch: same as `feat/trace-style-board-render` (stacked work, local only).

## Tasks
- [x] T1 — File > Close (route: delegated writer, after trace-style T4)
- [x] T2 — Per-board PDF association and auto-close/auto-open (route: same writer)

## Acceptance criteria
- Close leaves the app in the no-board state with no PDF panel.
- Opening board B after board A with PDF A open closes PDF A; reopening A later reopens PDF A automatically.
- Preferences remain backward compatible (serde default); tests for the association logic.

## Progress
### T1 evidence
- File menu: "Close Board" (`toolbar.close_board`, 6 catalogs, key-count test 147 -> 148), disabled without a board, calls `BoardViewApp::close_board`.
- `close_board` clears board, filename, pin/via/part/net selection, pending auto-fit, canvas mesh cache/hover/context menu, side panel overlay-editing/focus/stay-on-part and search cache (filter texts kept), both photo overlays and the alignment session (they are aligned to the closed board; decision beyond spec), closes the PDF (`PdfSchematicViewer::close` = reset to default, dropping the render pool) and the pending schematic prompt; status "Closed <name>.".
- Test-first exception: new entry point, RED would be a compile error; test `close_board_returns_to_the_no_board_state` added with it.
- `cargo test --release -p bvc-ui-egui`: 83 passed, 1 ignored.
- T1 commit: `b5930e8` feat(ui): add File > Close.
- Runtime: `close_board` exercised in the release GUI by the T2 smoke run below (board, path, PDF and prompt cleared, status "Closed MS-V375 REV 7.0.cad."); the menu click itself not exercised.
### T2 evidence
- `preferences.rs`: `BoardSchematic { board, pdf }`, `Preferences::board_schematics` (`#[serde(default)]`, most recent first, capped at `MAX_BOARD_SCHEMATICS` = 32), `remember_board_schematic` (dedup per board, returns whether changed) and `board_schematic` lookup. Tests: backward-compatible load without the field, insert/lookup/replace, eviction beyond the cap.
- `lib.rs`: new `board_path` (set only for boards opened from a file); `load_file` now returns whether it loaded and delegates to `adopt_board`, which closes the previous schematic PDF and pending prompt (only when the new board parsed: a failed open keeps the current board and its PDF; decision beyond spec). `open_file_from_path` (menu, recent, drag&drop) reopens `remembered_schematic_pdf` (association whose file still exists) without a prompt, else keeps the co-located prompt. All PDF-open sites (menu dialog, drag&drop, prompt accept) go through `open_schematic`, which records the association and saves preferences when it changed. Tests: `opening_a_board_closes_the_previous_schematic`, `schematic_is_remembered_only_for_a_board_file`, `remembered_schematic_requires_an_existing_pdf`.
- Test-first exception: new API (RED = compile errors); tests added with it.
- Runtime smoke (temporary env-gated hook, removed; release binary, `XDG_CONFIG_HOME` in the scratchpad so user preferences were untouched): open V375 -> prompt for V375 PDF, accepted -> PDF open; open V373 -> V375 PDF closed, prompt for V373 PDF; reopen V375 -> V375 PDF reopened automatically, no prompt; Close -> no board, no PDF; saved `board_schematics` = [V375 cad -> V375 pdf].
- Limitation: a board passed on the command line or dropped as bytes has no path, so no association is recorded for it (`apps/boardviewcraft/src/main.rs` uses `load_file`, outside the allowed surface).
- `cargo test --workspace --release`: all ok (bvc-core 22+4+18, ui-egui 89 passed, 1 ignored). `cargo clippy --workspace --release`: per-file warning counts identical to `d8db4f0`. `cargo build --release`: ok.
