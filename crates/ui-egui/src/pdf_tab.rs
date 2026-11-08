//! The PDF tab: the schematic viewer (pages, zoom, search, hit highlights) and the text token
//! clicked for reverse cross-probing to the board.

use crate::i18n::{self, Language};
use crate::pdf::{PdfSchematicViewer, SchematicOccurrence};
use crate::theme::Tokens;
use egui::{Color32, Pos2, Rect, RichText, Stroke, Ui};

/// What the PDF tab asks the app to do this frame.
#[derive(Default)]
pub struct PdfTabOutput {
    /// The "Open PDF" button was pressed (no document loaded).
    pub open_requested: bool,
    /// The close button was pressed: close the document.
    pub close_requested: bool,
    /// Text token under a click on the page, for reverse cross-probing.
    pub clicked_token: Option<SchematicOccurrence>,
    /// Request to navigate to the PCB tab focusing a token/component.
    pub navigate_to_pcb: Option<String>,
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

            if ui.button("⏴").on_hover_text(i18n::tr(lang, "pdf.hit_prev")).clicked() {
                viewer.prev_match();
            }

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

            if ui.button("⏵").on_hover_text(i18n::tr(lang, "pdf.hit_next")).clicked() {
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

    // Mouse wheel: Ctrl + Wheel for zoom, bare Wheel for page turn (PDF-only behavior)
    if resp.hovered() {
        let zoom_delta = ui.input(|i| i.zoom_delta());
        let ctrl_pressed = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
        let smooth_scroll_y = ui.input(|i| i.smooth_scroll_delta.y);

        // In egui, when Ctrl is held with mouse wheel, egui translates the wheel
        // directly into `i.zoom_delta()` and zeroes out `smooth_scroll_delta`.
        if ctrl_pressed && zoom_delta != 1.0 {
            let old_zoom = viewer.zoom;
            let new_zoom = (old_zoom * zoom_delta).clamp(0.2, 30.0);
            if let Some(hover_pos) = resp.hover_pos() {
                let cursor_rel = hover_pos - rect.center() - viewer.pan_offset;
                let scale_ratio = new_zoom / old_zoom;
                viewer.pan_offset = hover_pos - rect.center() - cursor_rel * scale_ratio;
            }
            viewer.zoom = new_zoom;
            ui.ctx().request_repaint();
        } else if ctrl_pressed && smooth_scroll_y != 0.0 {
            // Fallback for platforms/backends where smooth_scroll_delta is not zeroed
            let factor = if smooth_scroll_y > 0.0 { 1.10 } else { 1.0 / 1.10 };
            let old_zoom = viewer.zoom;
            let new_zoom = (old_zoom * factor).clamp(0.2, 30.0);
            if let Some(hover_pos) = resp.hover_pos() {
                let cursor_rel = hover_pos - rect.center() - viewer.pan_offset;
                let scale_ratio = new_zoom / old_zoom;
                viewer.pan_offset = hover_pos - rect.center() - cursor_rel * scale_ratio;
            }
            viewer.zoom = new_zoom;
            ui.ctx().request_repaint();
        } else if !ctrl_pressed && smooth_scroll_y != 0.0 {
            // 2. Normal Wheel turns pages in the PDF with threshold accumulator
            // Use egui's temp data to accumulate wheel deltas so one notch turns exactly 1 page
            let accum_id = egui::Id::unique("pdf_wheel_accum");
            let mut accum: f32 = ui.data(|d| d.get_temp(accum_id)).unwrap_or(0.0);
            accum += smooth_scroll_y;

            // Typical mouse wheel tick in egui smooth_scroll_delta is ~20 to 50
            const WHEEL_THRESHOLD: f32 = 40.0;
            if accum <= -WHEEL_THRESHOLD {
                if viewer.current_page < viewer.total_pages {
                    viewer.go_to_page(viewer.current_page + 1);
                    ui.ctx().request_repaint();
                }
                accum = 0.0;
            } else if accum >= WHEEL_THRESHOLD {
                if viewer.current_page > 1 {
                    viewer.go_to_page(viewer.current_page - 1);
                    ui.ctx().request_repaint();
                }
                accum = 0.0;
            }
            ui.data_mut(|d| d.insert_temp(accum_id, accum));
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

        // Clicking on the page clears context menu and selection
        if resp.clicked() {
            viewer.context_menu_pos = None;
            viewer.selection_rect = None;
            viewer.selected_text = None;
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

        // Context menu for copying selection, selecting all, or navigating to PCB
        if let Some(menu_pos) = viewer.context_menu_pos {
            let mut close_menu = false;
            let menu_id = egui::Id::unique("pdf_selection_context_menu");
            let click_pdf_pt = to_pdf(menu_pos);
            let hover_token = viewer.find_token_at(viewer.current_page, click_pdf_pt);
            let target_text = hover_token.as_ref().map(|t| t.text.clone()).or_else(|| viewer.selected_text.clone());

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
                            ui.set_min_width(170.0);

                            if let Some(ref text) = target_text {
                                let label = format!("{} ({})", i18n::tr(lang, "context_menu.show_in_pcb"), text.trim());
                                if ui.button(RichText::new(label).color(tokens.accent()).strong()).clicked() {
                                    out.navigate_to_pcb = Some(text.trim().to_string());
                                    close_menu = true;
                                }
                                ui.separator();
                            }

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

    // Interactive lateral page scrollbar / scrubber
    if viewer.doc_info.is_some() && viewer.total_pages > 1 {
        let bar_width = 14.0;
        let track_rect = Rect::from_min_max(
            Pos2::new(rect.max.x - bar_width - 3.0, rect.min.y + 6.0),
            Pos2::new(rect.max.x - 3.0, rect.max.y - 6.0),
        );

        let total = viewer.total_pages as f32;
        let thumb_height = (track_rect.height() / total).clamp(20.0, 60.0);
        let available_travel = (track_rect.height() - thumb_height).max(0.0);

        let page_fraction = if viewer.total_pages > 1 {
            ((viewer.current_page - 1) as f32) / (total - 1.0)
        } else {
            0.0
        };

        let thumb_top = track_rect.min.y + page_fraction * available_travel;
        let thumb_rect = Rect::from_min_max(
            Pos2::new(track_rect.min.x, thumb_top),
            Pos2::new(track_rect.max.x, thumb_top + thumb_height),
        );

        let scrollbar_id = egui::Id::unique("pdf_page_scrollbar");
        let bar_resp = ui.interact(track_rect, scrollbar_id, egui::Sense::click_and_drag());

        let mut hovered_or_dragged_page: Option<usize> = None;

        if bar_resp.dragged() || bar_resp.clicked() {
            if let Some(pos) = bar_resp.interact_pointer_pos().or_else(|| bar_resp.hover_pos()) {
                let frac = ((pos.y - track_rect.min.y - thumb_height * 0.5) / available_travel.max(1.0)).clamp(0.0, 1.0);
                let target_page = ((frac * (total - 1.0)).round() as usize + 1).clamp(1, viewer.total_pages);
                if target_page != viewer.current_page {
                    viewer.go_to_page(target_page);
                    ui.ctx().request_repaint();
                }
                hovered_or_dragged_page = Some(target_page);
            }
        } else if bar_resp.hovered() {
            if let Some(pos) = bar_resp.hover_pos() {
                let frac = ((pos.y - track_rect.min.y - thumb_height * 0.5) / available_travel.max(1.0)).clamp(0.0, 1.0);
                let target_page = ((frac * (total - 1.0)).round() as usize + 1).clamp(1, viewer.total_pages);
                hovered_or_dragged_page = Some(target_page);
            }
        }

        // Render scrollbar track and thumb
        let is_hovered = bar_resp.hovered() || bar_resp.dragged();
        let track_bg = if is_hovered {
            Color32::from_rgba_unmultiplied(20, 20, 25, 120)
        } else {
            Color32::from_rgba_unmultiplied(20, 20, 25, 40)
        };
        let thumb_color = if bar_resp.dragged() {
            Color32::from_rgba_unmultiplied(50, 120, 220, 240)
        } else if is_hovered {
            Color32::from_rgba_unmultiplied(70, 130, 200, 200)
        } else {
            Color32::from_rgba_unmultiplied(100, 110, 130, 150)
        };

        painter.rect_filled(track_rect, 4.0, track_bg);
        painter.rect_filled(thumb_rect, 4.0, thumb_color);

        // Show floating page badge next to thumb when hovering or dragging
        if is_hovered {
            let page_to_show = hovered_or_dragged_page.unwrap_or(viewer.current_page);
            let badge_text = format!("{page_to_show} / {}", viewer.total_pages);
            let font = egui::FontId::proportional(13.0);
            let badge_pos = Pos2::new(track_rect.min.x - 10.0, thumb_top + thumb_height * 0.5);

            // Compute background pill for text
            let galley = painter.layout_no_wrap(badge_text.clone(), font.clone(), Color32::WHITE);
            let badge_rect = Rect::from_center_size(
                Pos2::new(badge_pos.x - galley.size().x * 0.5, badge_pos.y),
                galley.size() + egui::vec2(16.0, 8.0),
            );

            painter.rect_filled(badge_rect, 4.0, Color32::from_rgb(30, 32, 40));
            painter.rect_stroke(
                badge_rect,
                4.0,
                Stroke::new(1.0, Color32::from_rgb(80, 130, 220)),
                egui::StrokeKind::Middle,
            );
            painter.text(
                badge_rect.center(),
                egui::Align2::CENTER_CENTER,
                badge_text,
                font,
                Color32::WHITE,
            );
        }
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

