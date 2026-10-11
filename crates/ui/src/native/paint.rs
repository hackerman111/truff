use super::editor::color;
use crate::editor::{Affinity, DisplayMap};
use eframe::egui::{self, Color32, FontId, Galley, Painter};
use std::sync::Arc;
use truff_core::text::BytePos;

/// Resolves font family, size and line height according to Truff design specification.
pub fn resolve_font_and_line_height(source: &str) -> (FontId, f32) {
    let source_trim = source.trim_start();
    if source_trim.starts_with("# ") {
        (FontId::proportional(28.0), 36.0)
    } else if source_trim.starts_with("## ") {
        (FontId::proportional(22.0), 30.0)
    } else if source_trim.starts_with("### ") {
        (FontId::proportional(18.0), 26.0)
    } else if source_trim.starts_with("#### ") {
        (FontId::proportional(16.0), 24.0)
    } else if source_trim.starts_with("##### ") {
        (FontId::proportional(15.0), 22.0)
    } else if source_trim.starts_with("###### ") {
        (FontId::proportional(14.0), 20.0)
    } else if source_trim.starts_with("```") || source_trim.starts_with("    ") {
        (FontId::monospace(13.5), 20.0)
    } else {
        (FontId::proportional(15.5), 24.0)
    }
}

pub(super) fn shape(
    painter: &Painter,
    display: &DisplayMap,
    foreground: Color32,
    width: f32,
) -> Arc<Galley> {
    let (font, line_height) = resolve_font_and_line_height(display.source_text());

    let mut job =
        egui::text::LayoutJob::simple(display.visual_text().to_owned(), font, foreground, width);
    job.sections[0].format.line_height = Some(line_height);
    let base = job.sections[0].format.clone();
    let mut ranges = display
        .color_spans()
        .iter()
        .filter_map(|span| {
            let start = display
                .source_to_visual(BytePos::new(span.content.start), Affinity::Right)
                .get();
            let end = display
                .source_to_visual(BytePos::new(span.content.end), Affinity::Left)
                .get();
            (start < end).then_some((start..end, span.rgba))
        })
        .collect::<Vec<_>>();
    ranges.sort_by_key(|(range, _)| range.start);
    job.sections.clear();
    let mut offset = 0;
    for (range, [r, g, b, a]) in ranges {
        let start = range.start.max(offset);
        let end = range.end.min(job.text.len());
        if start >= end {
            continue;
        }
        if offset < start {
            job.sections.push(egui::text::LayoutSection {
                leading_space: 0.0,
                byte_range: offset.into()..start.into(),
                format: base.clone(),
            });
        }
        let mut format = base.clone();
        format.color = Color32::from_rgba_unmultiplied(r, g, b, a);
        job.sections.push(egui::text::LayoutSection {
            leading_space: 0.0,
            byte_range: start.into()..end.into(),
            format,
        });
        offset = end;
    }
    if offset < job.text.len() || job.sections.is_empty() {
        job.sections.push(egui::text::LayoutSection {
            leading_space: 0.0,
            byte_range: offset.into()..job.text.len().into(),
            format: base,
        });
    }
    painter.layout_job(job)
}

pub(super) fn apply_theme(ui: &mut egui::Ui, theme: &crate::theme::Theme) {
    let mut visuals = ui.visuals().clone();
    visuals.override_text_color = Some(color(theme.text_primary));
    visuals.panel_fill = color(theme.bg_primary);
    visuals.window_fill = color(theme.bg_surface);
    visuals.widgets.noninteractive.bg_fill = color(theme.bg_primary);
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, color(theme.border));
    visuals.widgets.inactive.bg_fill = color(theme.bg_secondary);
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, color(theme.text_primary));
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(4);
    visuals.widgets.hovered.bg_fill = color(theme.selection);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, color(theme.text_primary));
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(4);
    visuals.widgets.active.bg_fill = color(theme.selection);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, color(theme.text_primary));
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(4);
    visuals.selection.bg_fill = color(theme.selection);
    visuals.selection.stroke = egui::Stroke::new(1.0, color(theme.accent));
    *ui.visuals_mut() = visuals;
}
