use crate::app::global_config::TypographyConfig;
use crate::editor::{Affinity, DisplayMap, InlineStyle};
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
        let size = (base * 1.6).round();
        (FontId::new(size, family), (size * 1.3).round())
    } else if source_trim.starts_with("## ") {
        let size = (base * 1.35).round();
        (FontId::new(size, family), (size * 1.35).round())
    } else if source_trim.starts_with("### ") || source_trim.starts_with("#### ") {
        let size = (base * 1.15).round();
        (FontId::new(size, family), (size * 1.4).round())
    } else if source_trim.starts_with("##### ") || source_trim.starts_with("###### ") {
        let size = base;
        (FontId::new(size, family), (size * 1.4).round())
    } else if source_trim.starts_with("```") || source_trim.starts_with("    ") {
        let size = base;
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
) -> (Arc<Galley>, f32) {
    let (font, line_height) = resolve_font_and_line_height(display.source_text(), typography);
    let indent = display.list_content_start().map_or(0.0, |start| {
        painter
            .layout_no_wrap(
                display.visual_text()[..start.get()].to_owned(),
                font.clone(),
                foreground,
            )
            .size()
            .x
    });

    let mut job = egui::text::LayoutJob::simple(
        display.visual_text().to_owned(),
        font.clone(),
        foreground,
        width,
    );
    job.sections[0].format.line_height = Some(line_height);
    let base = job.sections[0].format.clone();

    let visual_len = display.visual_text().len();
    let mut mapped_styles: Vec<(std::ops::Range<usize>, &InlineStyle)> = Vec::new();
    for span in display.styled_spans() {
        let start = display
            .source_to_visual(BytePos::new(span.content.start), Affinity::Right)
            .get();
        let end = display
            .source_to_visual(BytePos::new(span.content.end), Affinity::Left)
            .get();
        if start < end && end <= visual_len {
            mapped_styles.push((start..end, &span.style));
        }
    }

    let mut boundaries = vec![0, visual_len];
    for (range, _) in &mapped_styles {
        boundaries.push(range.start);
        boundaries.push(range.end);
    }
    boundaries.sort_unstable();
    boundaries.dedup();

    job.sections.clear();
    for window in boundaries.windows(2) {
        let start = window[0];
        let end = window[1];
        if start >= end {
            continue;
        }
        let mut format = base.clone();
        for (range, style) in &mapped_styles {
            if range.start <= start && range.end >= end {
                match style {
                    InlineStyle::Bold => {
                        format.color = Color32::from_rgba_unmultiplied(
                            (format.color.r() as u16 + 50).min(255) as u8,
                            (format.color.g() as u16 + 50).min(255) as u8,
                            (format.color.b() as u16 + 50).min(255) as u8,
                            format.color.a(),
                        );
                    }
                    InlineStyle::Italic => {
                        format.italics = true;
                    }
                    InlineStyle::Strikethrough => {
                        format.strikethrough = egui::Stroke::new(1.0, format.color);
                    }
                    InlineStyle::Code => {
                        format.font_id =
                            FontId::new((font.size * 0.95).max(9.0), egui::FontFamily::Monospace);
                        format.background = Color32::from_rgba_unmultiplied(128, 128, 128, 45);
                    }
                    InlineStyle::Link => {
                        format.color = Color32::from_rgb(136, 192, 208);
                        format.underline = egui::Stroke::new(
                            1.0,
                            Color32::from_rgba_unmultiplied(136, 192, 208, 180),
                        );
                    }
                    InlineStyle::TextColor([r, g, b, a]) => {
                        format.color = Color32::from_rgba_unmultiplied(*r, *g, *b, *a);
                    }
                    InlineStyle::Highlight([r, g, b, a]) => {
                        let alpha = (*a as u16 * 75 / 255) as u8;
                        format.background = Color32::from_rgba_unmultiplied(*r, *g, *b, alpha);
                    }
                }
            }
        }
        job.sections.push(egui::text::LayoutSection {
            leading_space: 0.0,
            byte_range: start.into()..end.into(),
            format,
        });
    }
    if job.sections.is_empty() {
        job.sections.push(egui::text::LayoutSection {
            leading_space: 0.0,
            byte_range: 0.into()..0.into(),
            format: base,
        });
    }
    // A negative first-row indent reserves a hanging margin for subsequent rows.
    job.sections[0].leading_space = -indent;
    job.wrap.max_width = (width - indent).max(1.0);
    (painter.layout_job(job), indent)
}
