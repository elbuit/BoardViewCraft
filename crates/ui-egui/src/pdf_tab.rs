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
            viewer.go_to_page(viewer.current_page - 1);
        }

        let jump_edit = ui.add(
            egui::TextEdit::singleline(&mut viewer.page_jump_input)
                .desired_width(38.0)
                .align(egui::Align2::CENTER_CENTER)
                .hint_text(viewer.current_page.to_string()),
        );
        let entered_page = jump_edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if entered_page {
            if let Ok(target) = viewer.page_jump_input.trim().parse::<usize>() {
                viewer.go_to_page(target);
            } else {
                viewer.page_jump_input = viewer.current_page.to_string();
            }
        }
        ui.label(format!("/ {}", viewer.total_pages));

        if ui.button("⏵").on_hover_text(i18n::tr(lang, "pdf.page_next")).clicked() && viewer.current_page < viewer.total_pages {
            viewer.go_to_page(viewer.current_page + 1);
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

    // Pan with Middle or Secondary (Right) drag
    if resp.dragged_by(egui::PointerButton::Middle) || resp.dragged_by(egui::PointerButton::Secondary) {
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

        // Reverse cross-probing: the text token under a click goes back to the board (only when not selecting a rectangle).
        if resp.clicked() {
            viewer.context_menu_pos = None;
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
                viewer.selection_rect = None;
                viewer.selected_text = None;
            }
        }

        // Rectangular text selection with primary mouse drag
        let to_pdf = |screen_pt: Pos2| -> Pos2 {
            let rel = screen_pt - render_pos;
            Pos2::new(
                rel.x / (crate::pdf::PT * display_scale),
                rel.y / (crate::pdf::PT * display_scale),
            )
        };

        if resp.drag_started_by(egui::PointerButton::Primary) {
            viewer.context_menu_pos = None;
            if let Some(origin) = ui.input(|i| i.pointer.press_origin()) {
                let origin_pdf = to_pdf(origin);
                viewer.selection_rect = Some(Rect::from_two_pos(origin_pdf, origin_pdf));
                viewer.selected_text = None;
            }
        } else if resp.dragged_by(egui::PointerButton::Primary) {
            if let (Some(origin), Some(curr)) = (ui.input(|i| i.pointer.press_origin()), ui.input(|i| i.pointer.interact_pos())) {
                let origin_pdf = to_pdf(origin);
                let curr_pdf = to_pdf(curr);
                let sel_rect = Rect::from_two_pos(origin_pdf, curr_pdf);
                viewer.selection_rect = Some(sel_rect);
                let (text, _) = viewer.extract_text_in_rect(viewer.current_page, sel_rect);
                viewer.selected_text = if text.trim().is_empty() { None } else { Some(text) };
            }
        }

        // Secondary click (right click) opens the context menu
        if resp.secondary_clicked() {
            if let Some(mouse_pos) = ui.input(|i| i.pointer.interact_pos().or_else(|| i.pointer.hover_pos())) {
                viewer.context_menu_pos = Some(mouse_pos);
            }
        }

        // Draw active selection box and highlighted text glyph boxes
        if let Some(sel_rect) = viewer.selection_rect {
            let sel_screen = Rect::from_min_max(to_screen(sel_rect.min), to_screen(sel_rect.max));
            painter.rect_filled(sel_screen, 1.0, Color32::from_rgba_unmultiplied(60, 140, 255, 45));
            painter.rect_stroke(
                sel_screen,
                1.0,
                egui::Stroke::new(1.0, Color32::from_rgb(60, 140, 255)),
                egui::StrokeKind::Outside,
            );

            // Highlight glyph runs inside selection
            let (_, boxes) = viewer.extract_text_in_rect(viewer.current_page, sel_rect);
            for b in boxes {
                let glyph_screen = Rect::from_min_max(to_screen(Pos2::new(b[0], b[1])), to_screen(Pos2::new(b[2], b[3])));
                painter.rect_filled(glyph_screen, 1.0, Color32::from_rgba_unmultiplied(80, 160, 255, 90));
            }
        }

        // Context menu for copying selection or selecting all
        if let Some(menu_pos) = viewer.context_menu_pos {
            let mut close_menu = false;
            let menu_id = egui::Id::unique("pdf_selection_context_menu");
            let area_response = egui::Area::new(menu_id)
                .fixed_pos(menu_pos)
                .order(egui::Order::Foreground)
                .show(ui.ctx(), |ui| {
                    egui::Frame::menu(ui.style())
                        .fill(tokens.chrome)
                        .stroke(egui::Stroke::new(1.0, tokens.border))
                        .corner_radius(4.0)
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.set_min_width(160.0);
                            let has_text = viewer.selected_text.as_ref().is_some_and(|s| !s.trim().is_empty());
                            let copy_btn = egui::Button::new(
                                RichText::new(i18n::tr(lang, "pdf.copy_selection"))
                                    .color(if has_text { tokens.text } else { tokens.text_dim() }),
                            );
                            if ui.add_enabled(has_text, copy_btn).clicked() {
                                if let Some(ref text) = viewer.selected_text {
                                    ui.ctx().copy_text(text.clone());
                                }
                                close_menu = true;
                            }

                            if ui
                                .button(RichText::new(i18n::tr(lang, "pdf.select_all")).color(tokens.text))
                                .clicked()
                            {
                                let s = viewer.page_size_pts(viewer.current_page);
                                let full_rect = Rect::from_min_max(Pos2::ZERO, Pos2::new(s.x, s.y));
                                viewer.selection_rect = Some(full_rect);
                                let (text, _) = viewer.extract_text_in_rect(viewer.current_page, full_rect);
                                viewer.selected_text = if text.trim().is_empty() { None } else { Some(text) };
                                close_menu = true;
                            }

                            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                close_menu = true;
                            }
                        });
                });

            let clicked_outside = ui.input(|i| i.pointer.any_pressed())
                && ui.input(|i| i.pointer.interact_pos()).is_some_and(|p| !area_response.response.rect.contains(p));

            if close_menu || clicked_outside {
                viewer.context_menu_pos = None;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_go_to_page_clamps_and_syncs_buffer() {
        let mut viewer = PdfSchematicViewer::default();
        viewer.total_pages = 10;
        viewer.current_page = 1;

        viewer.go_to_page(5);
        assert_eq!(viewer.current_page, 5);
        assert_eq!(viewer.page_jump_input, "5");

        viewer.go_to_page(15);
        assert_eq!(viewer.current_page, 10);
        assert_eq!(viewer.page_jump_input, "10");

        viewer.go_to_page(0);
        assert_eq!(viewer.current_page, 1);
        assert_eq!(viewer.page_jump_input, "1");
    }

    #[test]
    fn test_extract_text_empty_when_no_text_layer() {
        let viewer = PdfSchematicViewer::default();
        let (text, boxes) = viewer.extract_text_in_rect(1, Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 100.0)));
        assert!(text.is_empty());
        assert!(boxes.is_empty());
    }
}

