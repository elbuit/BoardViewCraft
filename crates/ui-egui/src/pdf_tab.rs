//! The PDF tab: the schematic viewer (pages, zoom, search, hit highlights) and the text token
//! clicked for reverse cross-probing to the board.

use crate::i18n::{self, Language};
use crate::pdf::{PdfSchematicViewer, SchematicOccurrence};
use crate::theme::Tokens;
use egui::{Color32, Pos2, Rect, RichText, Ui};

/// What the PDF tab asks the app to do this frame.
#[derive(Default)]
pub struct PdfTabOutput {
    /// The "Open PDF" button was pressed (no document loaded).
    pub open_requested: bool,
    /// The close button was pressed: close the document.
    pub close_requested: bool,
    /// Text token under a click on the page, for reverse cross-probing.
    pub clicked_token: Option<SchematicOccurrence>,
}

/// Draws the PDF tab in `ui`. Without a document it offers an "Open PDF" button.
pub fn show_pdf_tab(ui: &mut Ui, tokens: &Tokens, lang: Language, viewer: &mut PdfSchematicViewer) -> PdfTabOutput {
    let mut out = PdfTabOutput::default();
    if !viewer.is_loaded() {
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() * 0.4).max(0.0));
            ui.label(RichText::new(i18n::tr(lang, "pdf.no_document")).color(tokens.text_dim()));
            ui.add_space(8.0);
            if ui.button(RichText::new(i18n::tr(lang, "pdf.open")).strong()).clicked() {
                out.open_requested = true;
            }
        });
        return out;
    }

    ui.horizontal(|ui| {
        ui.heading(RichText::new(i18n::tr(lang, "pdf.title")).strong().color(tokens.accent()));
        if let Some(fn_str) = &viewer.filename {
            ui.label(RichText::new(fn_str).small().color(tokens.text_dim()));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("✖").on_hover_text(i18n::tr(lang, "pdf.close")).clicked() {
                out.close_requested = true;
            }
        });
    });
    ui.separator();

    // Navigation bar
    ui.horizontal(|ui| {
        if ui.button("⏴").on_hover_text(i18n::tr(lang, "pdf.page_prev")).clicked() && viewer.current_page > 1 {
            viewer.current_page -= 1;
        }
        let page_counter_text = i18n::tr(lang, "pdf.page_counter")
            .replacen("{}", &viewer.current_page.to_string(), 1)
            .replacen("{}", &viewer.total_pages.to_string(), 1);
        ui.label(page_counter_text);
        if ui.button("⏵").on_hover_text(i18n::tr(lang, "pdf.page_next")).clicked() && viewer.current_page < viewer.total_pages {
            viewer.current_page += 1;
        }

        ui.separator();
        let zoom_pct = (viewer.zoom * 100.0).round() as i32;
        let zoom_label = format!("🔍 {zoom_pct}%");
        if ui.button(RichText::new(zoom_label).color(tokens.accent()).monospace())
            .on_hover_text("Click to reset zoom to 100%")
            .clicked()
        {
            viewer.zoom = 1.0;
            viewer.pan_offset = egui::Vec2::ZERO;
            ui.ctx().request_repaint();
        }

        if !viewer.search_results.is_empty() {
            ui.separator();
            let count = viewer.search_results.len();
            let current = viewer.selected_result_idx + 1;
            let hits_label = format!("{current}/{count} {}", i18n::tr(lang, "pdf.hits"));

            let mut match_to_select = None;
            egui::ComboBox::from_id_salt("pdf_match_list")
                .selected_text(RichText::new(hits_label).color(tokens.accent()))
                .show_ui(ui, |ui| {
                    for (i, hit) in viewer.search_results.iter().enumerate() {
                        let item_label = format!("#{}: {} (p. {})", i + 1, hit.text, hit.page_number);
                        let is_selected = i == viewer.selected_result_idx;
                        if ui.selectable_label(is_selected, item_label).clicked() {
                            match_to_select = Some(i);
                        }
                    }
                });

            if let Some(idx) = match_to_select {
                viewer.select_match(idx);
            }

            if ui.button(i18n::tr(lang, "pdf.hit_prev")).clicked() {
                viewer.prev_match();
            }
            if ui.button(i18n::tr(lang, "pdf.hit_next")).clicked() {
                viewer.next_match();
            }
        }
    });

    // Text search: Enter or the button jumps to the first hit; Enter again on the
    // same query steps to the next one.
    ui.horizontal(|ui| {
        let search_resp = ui.add(
            egui::TextEdit::singleline(&mut viewer.search_input)
                .hint_text(i18n::tr(lang, "pdf.search_hint"))
                .desired_width(180.0),
        );
        let entered = search_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if entered || ui.button(i18n::tr(lang, "pdf.search")).clicked() {
            let query = viewer.search_input.trim().to_string();
            if entered && !query.is_empty() && query == viewer.active_search && !viewer.search_results.is_empty() {
                viewer.next_match();
            } else {
                viewer.search(&query);
            }
            if entered {
                search_resp.request_focus();
            }
        }
        if !viewer.active_search.is_empty()
            && viewer.search_results.is_empty()
            && viewer.text_indexed()
        {
            ui.label(RichText::new(i18n::tr(lang, "pdf.no_hits")).color(tokens.text_dim()));
        }
    });
    ui.separator();

    // PDF Page viewer canvas area with interactive pan and zoom
    let (rect, resp) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 4.0, Color32::from_rgb(255, 255, 255));

    if resp.dragged() {
        viewer.pan_offset += resp.drag_delta();
    }

    // Mouse wheel zoom centered under cursor with gentle sensitivity
    if resp.hovered() {
        let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll_y != 0.0 {
            let old_zoom = viewer.zoom;
            // Gentle 1.07 factor per tick instead of aggressive 1.15
            let zoom_factor = if scroll_y > 0.0 { 1.07 } else { 1.0 / 1.07 };
            let new_zoom = (old_zoom * zoom_factor).clamp(0.2, 30.0);

            if let Some(hover_pos) = resp.hover_pos() {
                // Keep the document point under the mouse invariant:
                // hover_pos = rect.center() + pan_offset_new + (hover_pos - rect.center() - pan_offset_old) * (new_zoom / old_zoom)
                let cursor_rel = hover_pos - rect.center() - viewer.pan_offset;
                let scale_ratio = new_zoom / old_zoom;
                viewer.pan_offset = hover_pos - rect.center() - cursor_rel * scale_ratio;
            }

            viewer.zoom = new_zoom;
            ui.ctx().request_repaint();
        }

        // Keyboard navigation inside PDF panel when hovered, unless a text field
        // (e.g. the search box) has the keyboard
        let typing = ui.memory(|m| m.focused().is_some());
        let (page_up, page_down, arrow_left, arrow_right, plus_key, minus_key) = ui.input(|i| {
            if typing {
                return Default::default();
            }
            (
                i.key_pressed(egui::Key::PageUp),
                i.key_pressed(egui::Key::PageDown),
                i.key_pressed(egui::Key::ArrowLeft),
                i.key_pressed(egui::Key::ArrowRight),
                i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals),
                i.key_pressed(egui::Key::Minus),
            )
        });

        if (page_up || arrow_left) && viewer.current_page > 1 {
            viewer.current_page -= 1;
            ui.ctx().request_repaint();
        } else if (page_down || arrow_right) && viewer.current_page < viewer.total_pages {
            viewer.current_page += 1;
            ui.ctx().request_repaint();
        }

        if plus_key {
            viewer.zoom = (viewer.zoom * 1.15).clamp(0.2, 30.0);
            ui.ctx().request_repaint();
        } else if minus_key {
            viewer.zoom = (viewer.zoom / 1.15).clamp(0.2, 30.0);
            ui.ctx().request_repaint();
        }
    }

    // Render crisp rasterized PDF page (whole, or backdrop + visible tiles) from engine
    if viewer.doc_info.is_some() {
        let page_size_pts = viewer.page_size_pts(viewer.current_page);
        // Natural unzoomed size of the page in UI points
        let base_page_size = page_size_pts * crate::pdf::PT;
        let fit_scale = ((rect.width() * 0.95) / base_page_size.x.max(1.0))
            .min((rect.height() * 0.95) / base_page_size.y.max(1.0));
        let display_scale = fit_scale * viewer.zoom;
        let render_size = base_page_size * display_scale;

        // Center viewport on selected match if requested (e.g. cross-probing or next/prev match)
        if viewer.pending_center_on_match
            && let Some(hit) = viewer.search_results.get(viewer.selected_result_idx)
            && hit.page_number == viewer.current_page
        {
            let comp_pos = hit.bbox.center() * crate::pdf::PT * display_scale;
            viewer.pan_offset = egui::vec2(render_size.x * 0.5 - comp_pos.x, render_size.y * 0.5 - comp_pos.y);
            viewer.pending_center_on_match = false;
        }

        let render_pos = rect.min
            + (rect.size() - render_size) * 0.5
            + viewer.pan_offset;

        let img_rect = Rect::from_min_size(render_pos, render_size);
        let painted = viewer.paint_current_page(ui.ctx(), &painter, img_rect, rect);
        if !painted {
            let page_label = i18n::tr(lang, "pdf.loading_page")
                .replace("{}", &viewer.current_page.to_string());
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                page_label,
                egui::FontId::proportional(16.0),
                Color32::from_rgb(100, 100, 110),
            );
        }

        // PDF coordinate mapping function (PDF points -> screen points)
        let to_screen = |pdf_pt: Pos2| -> Pos2 {
            let p = pdf_pt.to_vec2() * crate::pdf::PT * display_scale;
            render_pos + p
        };

        // Cross-probing highlights
        for (i, hit) in viewer.search_results.iter().enumerate() {
            if hit.page_number == viewer.current_page {
                let top_left = to_screen(hit.bbox.min);
                let btm_right = to_screen(hit.bbox.max);
                let marker_rect = Rect::from_min_max(top_left, btm_right);

                let is_selected = i == viewer.selected_result_idx;
                let bg_color = if is_selected {
                    Color32::from_rgba_unmultiplied(255, 230, 0, 160)
                } else {
                    Color32::from_rgba_unmultiplied(100, 180, 255, 120)
                };

                painter.rect_filled(marker_rect, 2.0, bg_color);
                painter.rect_stroke(
                    marker_rect,
                    2.0,
                    egui::Stroke::new(2.0, Color32::from_rgb(220, 20, 40)),
                    egui::StrokeKind::Middle,
                );
            }
        }

        // Reverse cross-probing: the text token under a click goes back to the board.
        if resp.clicked() {
            let click_opt = resp.interact_pointer_pos().or_else(|| resp.hover_pos()).or_else(|| ui.input(|i| i.pointer.interact_pos()));
            if let Some(click_pos) = click_opt
                && rect.contains(click_pos)
            {
                // Inverse map: screen pos -> PDF point
                let rel_screen = click_pos - render_pos;
                let clicked_pdf_pt = Pos2::new(
                    rel_screen.x / (crate::pdf::PT * display_scale),
                    rel_screen.y / (crate::pdf::PT * display_scale),
                );
                out.clicked_token = viewer.find_token_at(viewer.current_page, clicked_pdf_pt);
            }
        }
    } else {
        let page_label = i18n::tr(lang, "pdf.loading_page")
            .replace("{}", &viewer.current_page.to_string());
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            page_label,
            egui::FontId::proportional(16.0),
            Color32::from_rgb(100, 100, 110),
        );
    }
    out
}
