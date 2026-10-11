//! CPU-side egui viewport benchmark; excludes platform window, GPU, and initial layout.
use eframe::egui::{self, Event};
use std::time::Instant;
use truff_ui::{app::Application, native::NativeApp};

fn main() {
    let mut samples = Vec::new();
    for _ in 0..5 {
        let context = egui::Context::default();
        let mut application = Application::new();
        application
            .shell
            .set_editor_text(&"plain **bold** and *italic* with 字🦀\n".repeat(30_000));
        let mut app = NativeApp::new(application, &context);
        let mut frames = |events| {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1100.0, 700.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| app.show(ui),
            );
            output.textures_delta.clear();
        };
        frames(vec![]);
        let start = Instant::now();
        for _ in 0..200 {
            frames(vec![Event::Text("j".into())]);
        }
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
        println!(
            "builds={} retained={}",
            app.editor.layout_rebuild_count(),
            app.editor.cached_paragraph_count()
        );
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "five runs of 200 native egui cursor frames, median_ms={:.3}, min_ms={:.3}, max_ms={:.3}",
        samples[2], samples[0], samples[4]
    );
}
