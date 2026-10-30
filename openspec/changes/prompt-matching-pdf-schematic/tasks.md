# Tasks: Prompt Matching PDF Schematic on Boardview Open

## Phase 1: Implementation
- [ ] 1.1 Implement `find_matching_schematic_pdf(board_path: &Path) -> Option<PathBuf>` in `crates/ui-egui/src/lib.rs` supporting `.pdf`, `.PDF`, and case-insensitive scanning.
- [ ] 1.2 Add `pending_schematic_prompt: Option<PathBuf>` field to `BoardViewApp`.
- [ ] 1.3 Trigger discovery in `open_file_from_path` and set `pending_schematic_prompt` if found and not already open in `pdf_viewer`.
- [ ] 1.4 Render modal confirmation dialog in `BoardViewApp::ui` with `Enter` (open) and `Esc` (dismiss).

## Phase 2: Internationalization
- [ ] 2.1 Add keys `pdf.prompt_found_title`, `pdf.prompt_found_msg`, `pdf.prompt_open`, and `pdf.prompt_cancel` to `en.tsv`, `es.tsv`, `ca.tsv`, `fr.tsv`, `de.tsv`, `it.tsv`.
- [ ] 2.2 Update total key count test in `crates/ui-egui/src/i18n.rs` from 128 to 132.

## Phase 3: Verification
- [ ] 3.1 Unit test `find_matching_schematic_pdf` with synthetic directory layouts (`.pdf`, `.PDF`, different stems, nonexistent paths).
- [ ] 3.2 Run `cargo test --workspace` and verify all tests pass.
