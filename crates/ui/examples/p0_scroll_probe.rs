//! CPU-side production egui frames; excludes GPU presentation and native input latency.
use egui::Event;
use std::time::Instant;
use truff_ui::{
    app::Application,
    native::NativeApp,
    navigator::{self, NavigatorItem, NavigatorMode, NavigatorState},
    theme::Theme,
};

fn input(events: Vec<Event>) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1100.0, 700.0),
        )),
        events,
        ..Default::default()
    }
}

fn report(name: &str, count: usize, samples: &mut [f64], retained: usize) {
    samples.sort_by(f64::total_cmp);
    println!(
        "{name},{count},{:.3},{:.3},{retained}",
        samples[samples.len() / 2],
        samples[(samples.len() * 95 / 100).min(samples.len() - 1)]
    );
}

fn main() {
    println!("scenario,items,p50_ms,p95_ms,max_retained");
    for count in [100, 1000, 10_000] {
        let ctx = egui::Context::default();
        let mut state = NavigatorState::new();
        state.open_mode(
            NavigatorMode::Files,
            "test",
            (0..count)
                .map(|i| {
                    NavigatorItem::new_file(i, &format!("note-{i}"), format!("note-{i}"), false, 0)
                })
                .collect(),
        );
        let mut samples = Vec::with_capacity(500);
        let mut retained = 0;
        for frame in 0..505 {
            if frame % 100 == 0 {
                state.move_top();
            }
            // Native printable input supplies both physical Key and authoritative Text.
            let events = vec![
                Event::Key {
                    key: egui::Key::J,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
                Event::Text("j".into()),
                Event::Key {
                    key: egui::Key::J,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ];
            let start = Instant::now();
            let mut output = ctx.run_ui(input(events), |_| {
                navigator::show(&mut state, &ctx, &Theme::nord());
            });
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            retained = retained.max(output.shapes.iter().filter(|shape| matches!(&shape.shape, egui::Shape::Text(t) if t.galley.job.text.starts_with("note-") && matches!(t.galley.job.sections[0].format.font_id.size,12.0 | 13.0 | 14.0) && t.pos.x + t.galley.size().x / 2.0 < 550.0)).count());
            output.textures_delta.clear();
            if frame >= 5 {
                samples.push(elapsed);
            }
        }
        report("navigator", count as usize, &mut samples, retained);
    }
    let ctx = egui::Context::default();
    let mut application = Application::new();
    let source: String = (0..2000)
        .map(|i| {
            if i % 10 == 0 {
                format!("# Heading {i}\n")
            } else {
                format!("{}\n", "word Ж 界 👩‍💻 ".repeat(8))
            }
        })
        .collect();
    application.shell.set_editor_text(&source);
    let mut app = NativeApp::new(application, &ctx);
    let mut samples = Vec::with_capacity(500);
    let mut retained = 0;
    for frame in 0..505 {
        let text = if frame % 100 == 0 { "gg" } else { "j" };
        let start = Instant::now();
        let mut output = ctx.run_ui(input(vec![Event::Text(text.into())]), |ui| app.show(ui));
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        output.textures_delta.clear();
        retained = retained.max(app.editor.cached_paragraph_count());
        if frame >= 5 {
            samples.push(elapsed);
        }
    }
    report("editor", 2000, &mut samples, retained);
}
