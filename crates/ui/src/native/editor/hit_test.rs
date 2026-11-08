//! Gutter fold controls, link hover feedback, and pointer selection/navigation hit-testing.

use egui::{Painter, Pos2, Rect, Response};
use truff_core::markdown::{LinkDestination, extract_link_destination};
use truff_core::text::BufferId;
use truff_core::workspace::View;

use super::paragraph::{Paragraph, TASK_PADDING_X};
use super::{EditorWidget, LINE_HEIGHT, color};
use crate::editor::{EditorInput, InlineStyle, VisualPos};
use crate::theme::Theme;

impl EditorWidget {
    pub(super) fn paint_fold_gutter_and_hit_test(
        &self,
        painter: &Painter,
        response: &Response,
        theme: &Theme,
        (buffer_id, line): (BufferId, usize),
        entry: &Paragraph,
        origin: Pos2,
    ) -> Option<EditorInput> {
        let mut clicked = None;
        let is_folded = self.is_heading_folded(buffer_id, line);
        let entry_size = entry.size();

        let chevron_text = if is_folded { "▶" } else { "▼" };
        let chevron_pos = egui::pos2(origin.x - 18.0, origin.y + 4.0);
        let chevron_rect = egui::Rect::from_min_size(
            egui::pos2(origin.x - 24.0, origin.y),
            egui::vec2(22.0, entry_size.y.max(LINE_HEIGHT)),
        );
        let is_hovered = response
            .hover_pos()
            .is_some_and(|p| chevron_rect.contains(p));
        if is_hovered {
            painter
                .ctx()
                .set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let chevron_color = if is_folded || is_hovered {
            color(theme.text_secondary)
        } else {
            color(theme.text_muted).gamma_multiply(0.6)
        };
        painter.text(
            chevron_pos,
            egui::Align2::LEFT_TOP,
            chevron_text,
            egui::FontId::proportional(11.0),
            chevron_color,
        );
        if response.clicked()
            && response
                .interact_pointer_pos()
                .is_some_and(|p| chevron_rect.contains(p))
        {
            clicked = Some(EditorInput::ToggleFold { heading_line: line });
        }

        if is_folded {
            let badge_pos = egui::pos2(origin.x + entry_size.x + 8.0, origin.y + 4.0);
            let badge_rect = egui::Rect::from_min_size(badge_pos, egui::vec2(24.0, 16.0));
            let badge_hovered = response.hover_pos().is_some_and(|p| badge_rect.contains(p));
            if badge_hovered {
                painter
                    .ctx()
                    .set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            let badge_bg = if badge_hovered {
                color(theme.bg_secondary)
            } else {
                color(theme.bg_secondary).gamma_multiply(0.7)
            };
            painter.rect_filled(badge_rect, 4.0, badge_bg);
            painter.text(
                badge_rect.center(),
                egui::Align2::CENTER_CENTER,
                "...",
                egui::FontId::proportional(11.0),
                color(theme.text_muted),
            );
            if response.clicked()
                && response
                    .interact_pointer_pos()
                    .is_some_and(|p| badge_rect.contains(p))
            {
                clicked = Some(EditorInput::ToggleFold { heading_line: line });
            }
        }
        clicked
    }

    pub(super) fn hit_test_line_pointer(
        &self,
        response: &Response,
        viewport_rect: Rect,
        view: &View,
        entry: &Paragraph,
        origin: Pos2,
        already_clicked: bool,
    ) -> Option<EditorInput> {
        let entry_h = entry.size().y.max(LINE_HEIGHT);
        let galley_origin = if entry.is_task() {
            egui::pos2(origin.x + TASK_PADDING_X, origin.y)
        } else {
            origin
        };

        if let Some(pointer_pos) = response.hover_pos()
            && pointer_pos.y >= origin.y
            && pointer_pos.y < origin.y + entry_h
        {
            let visual = visual_byte_at_pos(entry, pointer_pos - galley_origin);
            let source_offset = entry
                .display
                .visual_to_source(VisualPos::new(visual))
                .pos
                .get();
            let is_on_link = entry.display.styled_spans().iter().any(|span| {
                span.style == InlineStyle::Link
                    && source_offset >= span.content.start
                    && source_offset <= span.content.end
            });
            if is_on_link {
                response.ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        }

        if already_clicked || self.input_blocked || !(response.clicked() || response.dragged()) {
            return None;
        }
        let raw_pos = response.interact_pointer_pos()?;
        let position = if response.dragged() {
            viewport_rect.shrink(1.0).clamp(raw_pos)
        } else {
            raw_pos
        };
        if position.y < origin.y || position.y >= origin.y + entry_h {
            return None;
        }

        let visual = visual_byte_at_pos(entry, position - galley_origin);
        if response.clicked()
            && let Some(checkbox) = entry.display.task_at_visual(VisualPos::new(visual))
        {
            return Some(EditorInput::ToggleTask {
                checkbox: entry.range.start + checkbox,
            });
        }

        let source_offset = entry
            .display
            .visual_to_source(VisualPos::new(visual))
            .pos
            .get();
        if response.clicked() {
            let link_span = entry.display.styled_spans().iter().find(|span| {
                span.style == InlineStyle::Link
                    && source_offset >= span.content.start
                    && source_offset <= span.content.end
            });
            if let Some(span) = link_span
                && let Some(dest) =
                    extract_link_destination(entry.display.source_text(), &span.content)
            {
                return Some(match dest {
                    LinkDestination::External(url) => {
                        response.ctx.open_url(egui::OpenUrl::new_tab(url));
                        EditorInput::SetSelection {
                            anchor: entry.range.start + source_offset,
                            head: entry.range.start + source_offset,
                        }
                    }
                    LinkDestination::InternalHeading(heading) => {
                        EditorInput::FollowLink(format!("#{heading}"))
                    }
                    LinkDestination::Note { path, heading } => {
                        let target = match heading {
                            Some(h) => format!("{path}#{h}"),
                            None => path,
                        };
                        EditorInput::FollowLink(target)
                    }
                });
            }
        }

        let source = entry.range.start + source_offset;
        Some(EditorInput::SetSelection {
            anchor: if response.dragged() {
                view.anchor().get()
            } else {
                source
            },
            head: source,
        })
    }
}

fn visual_byte_at_pos(entry: &Paragraph, local_pos: egui::Vec2) -> usize {
    let index = entry.cursor_from_pos(local_pos);
    let vis_text = entry.display.visual_text();
    vis_text
        .char_indices()
        .nth(index.index.0)
        .map_or(vis_text.len(), |(offset, _)| offset)
}
