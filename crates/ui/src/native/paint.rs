use super::editor::color;
use crate::app::global_config::TypographyConfig;
use crate::editor::{Affinity, DisplayMap};
use eframe::egui::{self, Color32, FontId, Galley, Painter};
use std::sync::Arc;
use truff_core::text::BytePos;

/// Resolves font family, size and line height according to Truff design specification.
pub fn resolve_font_and_line_height(source: &str, typography: &TypographyConfig) -> (FontId, f32) {
    let family = match typography.font_family.as_str() {
        "proportional" => egui::FontFamily::Proportional,
        _ => egui::FontFamily::Monospace,
    };
    let base = typography.base_font_size;
    let source_trim = source.trim_start();
    if source_trim.starts_with("# ") {
        let size = (base * 1.8).round();
        (FontId::new(size, family), (size * 1.3).round())
    } else if source_trim.starts_with("## ") {
        let size = (base * 1.45).round();
        (FontId::new(size, family), (size * 1.35).round())
    } else if source_trim.starts_with("### ") {
        let size = (base * 1.25).round();
        (FontId::new(size, family), (size * 1.4).round())
    } else if source_trim.starts_with("#### ") {
        let size = (base * 1.1).round();
        (FontId::new(size, family), (size * 1.45).round())
    } else if source_trim.starts_with("##### ") {
        let size = (base * 1.0).round();
        (FontId::new(size, family), (size * 1.5).round())
    } else if source_trim.starts_with("###### ") {
        let size = (base * 0.95).round();
        (FontId::new(size, family), (size * 1.5).round())
    } else if source_trim.starts_with("```") || source_trim.starts_with("    ") {
        let size = (base * 0.9).round();
        (
            FontId::new(size, egui::FontFamily::Monospace),
            (size * 1.5).round(),
        )
    } else {
        let size = base;
        (
            FontId::new(size, family),
            (size * typography.line_height_multiplier).round(),
        )
    }
}

pub(super) fn shape(
    painter: &Painter,
    display: &DisplayMap,
    foreground: Color32,
    width: f32,
    typography: &TypographyConfig,
) -> Arc<Galley> {
    let (font, line_height) = resolve_font_and_line_height(display.source_text(), typography);

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
