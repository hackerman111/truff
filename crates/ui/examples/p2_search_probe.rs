//! Request-to-accepted-page and CPU frame timing through the production Navigator adapter.
use std::{
    fs,
    time::{Duration, Instant},
};
use truff_ui::{app::Application, native::NativeApp, navigator::NavigatorMode};

fn frame(app: &mut NativeApp, context: &egui::Context) -> f64 {
    let start = Instant::now();
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
    start.elapsed().as_secs_f64() * 1000.0
}

fn percentile(samples: &mut [f64], percentile: usize) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[(samples.len() * percentile / 100).min(samples.len() - 1)]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("notes,query_p50_ms,query_p95_ms,frame_p95_ms,max_page_items");
    for count in [100, 1000, 10_000] {
        let root =
            std::env::temp_dir().join(format!("truff-search-probe-{}-{count}", std::process::id()));
        fs::create_dir(&root)?;
        for index in 0..count {
            fs::write(
                root.join(format!("note-{index:05}.md")),
                format!("# Note {index}\n\nbodymarker{} ordinary text\n", index % 10),
            )?;
        }
        let mut application = Application::new();
        application.open_vault(&root)?;
        let deadline = Instant::now() + Duration::from_secs(180);
        while application.needs_poll() {
            application.poll();
            if Instant::now() > deadline {
                return Err(format!("Catalog timeout: {}", application.status).into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let context = egui::Context::default();
        let mut app = NativeApp::new(application, &context);
        app.open_navigator(NavigatorMode::Content);
        let mut queries = Vec::new();
        let mut frames = Vec::new();
        let mut max_items = 0;
        for query in 0..35 {
            let marker = query % 10;
            let text = format!("bodymarker{marker}");
            app.navigator.set_query(&text);
            let start = Instant::now();
            while app.navigator.items.is_empty() {
                let elapsed = frame(&mut app, &context);
                if query >= 5 {
                    frames.push(elapsed);
                }
                if start.elapsed() > Duration::from_secs(10) {
                    return Err(format!("Search timeout: {}", app.navigator.context_title).into());
                }
                if app.navigator.items.is_empty() {
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
            if app.navigator.items.len() != (count / 10).min(100)
                || app.navigator.items.iter().any(|item| {
                    !item
                        .preview
                        .as_deref()
                        .is_some_and(|preview| preview.contains(&text))
                })
            {
                return Err("Incorrect or stale search page".into());
            }
            max_items = max_items.max(app.navigator.items.len());
            if query >= 5 {
                queries.push(start.elapsed().as_secs_f64() * 1000.0);
            }
        }
        println!(
            "{count},{:.3},{:.3},{:.3},{max_items}",
            percentile(&mut queries, 50),
            percentile(&mut queries, 95),
            percentile(&mut frames, 95),
        );
        app.application.shutdown()?;
        drop(app);
        fs::remove_dir_all(root)?;
    }
    Ok(())
}
