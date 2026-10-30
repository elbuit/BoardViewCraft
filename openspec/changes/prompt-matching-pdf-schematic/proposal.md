# Proposal: Prompt Matching PDF Schematic on Boardview Open

## Intent

When diagnosing hardware and repairing PCB motherboards, engineers commonly store the boardview file (e.g. `MS-V373 REV 7.1.cad`) and its corresponding schematic document (e.g. `MS-V373 REV 7.1.pdf` or `MS-V373 REV 7.1.PDF`) in the same folder with matching base names.
Currently, after opening a boardview file, the user must manually navigate to the Actions menu or drag-and-drop the schematic PDF to initialize schematic cross-probing.

This change introduces automated discovery of co-located schematic PDFs matching the opened boardview filename (case-insensitive extension check: `.pdf`, `.PDF`) and displays a non-blocking modal prompt or toast asking the user if they want to open the matching schematic.

## Scope

### In Scope
- Automatic detection of sibling `<basename>.pdf` or `<basename>.PDF` in the same directory upon opening any boardview file (via Open File dialog, Drag & Drop, CLI argument, or Open Recent).
- State tracking for pending schematic confirmation in `BoardViewApp` (`pending_schematic_prompt: Option<PathBuf>`).
- UI prompt/dialog asking whether the user wants to open the matching schematic, with "Open" and "Dismiss" actions.
- Localized strings for the prompt across all supported languages (`en`, `es`, `ca`, `fr`, `de`, `it`).
- Automated tests verifying matching path detection (case sensitivity, non-existent siblings, different stems).

### Out of Scope
- Fuzzy search across subdirectories or unrelated filenames.
- Automatic opening without user confirmation (users may want to view only the boardview without rasterizing large PDF schematics in background).

## Capabilities

### New Capabilities
- `pdf-schematic-auto-prompt`: Detection and confirmation dialog for co-located matching PDF schematics when opening boardview files.

### Modified Capabilities
- None

## Approach

1. **Path Inspection Helper**:
   - In `BoardViewApp::open_file_from_path(path: &Path)`:
     - Check parent directory `path.parent()`.
     - Get base stem `path.file_stem()`.
     - Check sibling candidates: `<stem>.pdf`, `<stem>.PDF`, and case-insensitive matching in directory entries.
     - If a matching PDF exists and `pdf_viewer.path` is not already loaded with that file, set `self.pending_schematic_prompt = Some(pdf_path)`.

2. **Modal / Window Prompt in `ui-egui`**:
   - Render a centered, lightweight modal prompt dialog when `pending_schematic_prompt.is_some()`.
   - Message: "Matching schematic PDF found: <filename>. Would you like to open it?"
   - Buttons: "Open Schematic" (`Enter`), "Cancel" (`Esc`).
   - If accepted: invoke `self.pdf_viewer.open_file(&pdf_path)` and clear the pending prompt.

3. **i18n Localization**:
   - Add catalog keys:
     - `pdf.prompt_found_title`
     - `pdf.prompt_found_msg`
     - `pdf.prompt_open`
     - `pdf.prompt_dismiss`
   - Supply full translations for `en`, `es`, `ca`, `fr`, `de`, `it`.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/ui-egui/src/lib.rs` | Modified | Add `pending_schematic_prompt`, check sibling on `open_file_from_path`, render prompt modal |
| `crates/ui-egui/src/i18n.rs` | Modified | Update tests for the new keys |
| `crates/ui-egui/src/i18n/*.tsv` | Modified | Add prompt translation keys to en, es, ca, fr, de, it |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| User finds modal intrusive when loading boards repeatedly | Low | Keep it lightweight with clear keyboard triggers (`Enter` to open, `Esc` to dismiss) and do not prompt if already loaded. |
| Filesystems with strict case sensitivity on Linux vs Windows | Low | Check exact stem + `.pdf` and `.PDF`, plus scan directory entries ignoring case if direct check misses. |

## Rollback Plan

Revert the commit touching `crates/ui-egui/src/lib.rs` and the i18n TSV files. The feature is self-contained within application state and does not alter file storage or core data structures.

## Dependencies

- None (uses standard library `std::fs` / `std::path` and existing `rfd` / `pdf_viewer`).

## Success Criteria

- [ ] When opening `foo.cad` where `foo.pdf` exists in the same folder, a prompt asks to open the schematic.
- [ ] Both lowercase `.pdf` and uppercase `.PDF` extensions are recognized.
- [ ] Clicking "Open" loads the schematic into `pdf_viewer` with cross-probing active.
- [ ] Clicking "Cancel" or pressing `Esc` dismisses the prompt without opening the PDF.
- [ ] If no sibling PDF exists, no prompt is shown.
- [ ] All translations (`en`, `es`, `ca`, `fr`, `de`, `it`) pass automated coverage test.
