use eframe::egui;
use std::time::Instant;
use truff_ui::{app::Application, editor::EditorInput, native::NativeApp};

fn frame(app: &mut NativeApp, context: &egui::Context) {
    context
        .run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1100.0, 700.0),
                )),
                ..Default::default()
            },
            |ui| app.show(ui),
        )
        .drop_without_applying_deltas();
}
fn main() {
    for count in [1000, 30_000] {
        let context = egui::Context::default();
        let mut application = Application::new();
        application
            .shell
            .set_editor_text(&"- parent\n  - child\n    - [ ] task 字🦀\n\n".repeat(count / 4));
        let start = Instant::now();
        let mut app = NativeApp::new(application, &context);
        frame(&mut app, &context);
        println!(
            "{count} lines startup {:.2} ms",
            start.elapsed().as_secs_f64() * 1000.0
        );
        app.application
            .shell
            .apply_input(EditorInput::SetSelection { anchor: 3, head: 3 })
            .unwrap();
        let start = Instant::now();
        for _ in 0..30 {
            app.application
                .shell
                .apply_input(EditorInput::InsertText("x".into()))
                .unwrap();
            frame(&mut app, &context);
        }
        println!(
            "{count} lines edit {:.2} ms/frame",
            start.elapsed().as_secs_f64() * 1000.0 / 30.0
        );
        let builds = app.editor.layout_rebuild_count();
        let start = Instant::now();
        for _ in 0..100 {
            frame(&mut app, &context);
        }
        println!(
            "{count} lines idle {:.3} ms/frame, builds {} cached {}",
            start.elapsed().as_secs_f64() * 10.0,
            app.editor.layout_rebuild_count() - builds,
            app.editor.cached_paragraph_count()
        );
    }
}
