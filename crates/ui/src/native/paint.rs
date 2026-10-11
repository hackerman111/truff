use crate::editor::{Affinity, DisplayMap};
use eframe::egui::{self, Color32, FontId, Galley, Painter};
use std::sync::Arc;
use truff_core::text::BytePos;

pub(super) fn shape(
    painter: &Painter,
    display: &DisplayMap,
    foreground: Color32,
    width: f32,
) -> Arc<Galley> {
    let source_trim = display.source_text().trim_start();
    let (font, line_height) = if source_trim.starts_with("# ") {
        (FontId::proportional(22.0), 32.0)
    } else if source_trim.starts_with("## ") {
        (FontId::proportional(18.5), 28.0)
    } else if source_trim.starts_with("### ") {
        (FontId::proportional(16.5), 25.0)
    } else if source_trim.starts_with("```") || source_trim.starts_with("    ") {
        (FontId::monospace(14.0), 22.0)
    } else {
        (FontId::proportional(15.0), 24.0)
    };

    let mut job = egui::text::LayoutJob::simple(
        display.visual_text().to_owned(),
        font,
        foreground,
        width,
    );
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
