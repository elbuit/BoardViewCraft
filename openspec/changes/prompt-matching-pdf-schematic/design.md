# Technical Design: Co-located PDF Schematic Auto-Prompt

## Context & Problem
Engineers working on circuit board repair frequently store the boardview file (e.g. `MS-V373.cad`) alongside its schematic documentation (`MS-V373.pdf`). When opening the boardview in BoardViewCraft, having the application automatically discover the matching schematic and offer to load it avoids repetitive manual navigation through file dialogs.

## Architecture & Data Flow

```
User Action (Open Board / Drag & Drop / CLI / Recent)
         │
         ▼
BoardViewApp::open_file_from_path(board_path)
         │
         ├── 1. Load board bytes into parser & spatial index
         ├── 2. Add to recent files & save preferences
         ▼
find_matching_schematic_pdf(board_path)
         │
         ├─► Checks: sibling exists? (.pdf or .PDF)
         ├─► Checks: already opened in pdf_viewer?
         │
         ▼ (If found & not open)
Set self.pending_schematic_prompt = Some(pdf_path)
         │
         ▼
Next Frame: ui(&mut self, ui, frame)
         │
         ▼
render_schematic_prompt_dialog(ui, ctx)
         │
         ├── [Open] (or Enter) ──► self.pdf_viewer.open_file(&path); clear prompt
         └── [Cancel] (or Esc) ──► clear prompt
```

## Detailed Changes

### 1. Sibling PDF Detection Helper
A pure, testable helper function in `crates/ui-egui/src/lib.rs`:
```rust
pub fn find_matching_schematic_pdf(board_path: &std::path::Path) -> Option<std::path::PathBuf> {
    let parent = board_path.parent()?;
    let stem = board_path.file_stem()?.to_str()?;

    // 1. Direct candidate checks
    for ext in &["pdf", "PDF"] {
        let candidate = parent.join(format!("{stem}.{ext}"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    // 2. Directory scan fallback (case-insensitive for mixed extensions on Linux)
    if let Ok(entries) = std::fs::read_dir(parent) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let (Some(entry_stem), Some(entry_ext)) = (path.file_stem(), path.extension()) {
                    if entry_stem.to_string_lossy().eq_ignore_ascii_case(stem)
                        && entry_ext.to_string_lossy().eq_ignore_ascii_case("pdf")
                    {
                        return Some(path);
                    }
                }
            }
        }
    }

    None
}
```

### 2. State & UI Rendering in `BoardViewApp`
In `BoardViewApp`:
- Field: `pending_schematic_prompt: Option<std::path::PathBuf>`
- When `pending_schematic_prompt` is `Some(ref pdf_path)`:
  - Render an `egui::Window` or modal overlay with `pivot(Align2::CENTER_CENTER)` and fixed position at screen center.
  - Show icon, file name, localized question, and two action buttons (`Open Schematic` & `Cancel`).
  - Capture `Key::Enter` (open) and `Key::Escape` (dismiss).

### 3. Internationalization (i18n)
Add the following 4 keys across all 6 TSV catalogs (`en`, `es`, `ca`, `fr`, `de`, `it`):
- `pdf.prompt_found_title`: Dialog title
- `pdf.prompt_found_msg`: "Matching schematic found for this board. Would you like to open it?"
- `pdf.prompt_open`: "Open Schematic"
- `pdf.prompt_cancel`: "Cancel"
