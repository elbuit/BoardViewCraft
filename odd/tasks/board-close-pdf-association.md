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
- [ ] T2 — Per-board PDF association and auto-close/auto-open (route: same writer)

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
- Not verified: the menu entry in the running GUI (no interactive screen session).
