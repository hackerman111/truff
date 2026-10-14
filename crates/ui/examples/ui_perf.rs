use std::hint::black_box;
use std::time::Instant;

use eframe::egui::{self, Event};
use truff_ui::{
    app::Application,
    native::NativeApp,
    navigator::{NavigatorItem, NavigatorState},
    overlays::FlashKind,
};

const ITERATIONS: usize = 100;
const SCREEN: egui::Vec2 = egui::vec2(1100.0, 700.0);

#[derive(Clone, Copy)]
enum Scenario {
    Idle,
    InsertCharacter,
    InsertNewline,
    CursorDown,
}

impl Scenario {
    fn name(self) -> &'static str {
        match self {
            Self::Idle => "idle_frame",
            Self::InsertCharacter => "insert_character_frame",
            Self::InsertNewline => "insert_newline_frame",
            Self::CursorDown => "cursor_down_frame",
        }
    }

    fn event(self) -> Option<Event> {
        match self {
            Self::Idle => None,
            Self::InsertCharacter => Some(Event::Text("x".into())),
            Self::InsertNewline => Some(Event::Text("\n".into())),
            Self::CursorDown => Some(Event::Text("j".into())),
        }
    }
}

fn frame(context: &egui::Context, app: &mut NativeApp, events: Vec<Event>) {
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)),
            events,
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();
    black_box(output);
}

fn run(scenario: Scenario) {
    let context = egui::Context::default();
    let mut application = Application::new();
    application
        .shell
        .set_editor_text(&"plain **bold** and 字🦀\n".repeat(30_000));
    let mut app = NativeApp::new(application, &context);

    if matches!(
        scenario,
        Scenario::InsertCharacter | Scenario::InsertNewline
    ) {
        frame(&context, &mut app, vec![Event::Text("i".into())]);
    }
    for _ in 0..2 {
        frame(&context, &mut app, scenario.event().into_iter().collect());
    }

    let builds_before = black_box(app.editor.layout_rebuild_count());
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        frame(&context, &mut app, scenario.event().into_iter().collect());
    }
    let ns_per_iter = start.elapsed().as_nanos() as f64 / ITERATIONS as f64;
    let layout_builds = black_box(app.editor.layout_rebuild_count()) - builds_before;
    let retained = black_box(app.editor.cached_paragraph_count());
    println!(
        "{},{ITERATIONS},{ns_per_iter:.0},{layout_builds},{retained}",
        scenario.name()
    );
}

fn run_navigator_filter() {
    let mut navigator = NavigatorState::new();
    navigator.items = (0..10_000)
        .map(|idx| {
            let title = format!("note-{idx:05}.md");
            NavigatorItem::new_file(
                idx as u64,
                &title,
                format!("folder-{idx:05}/{title}"),
                false,
                0,
            )
        })
        .collect();
    let queries = ["missing", "note-000", "folder-00042", "note-09999"];
    for _ in 0..2 {
        for query in queries {
            navigator.set_query(query);
        }
    }

    let start = Instant::now();
    let mut matches = 0;
    for _ in 0..ITERATIONS {
        for query in queries {
            navigator.set_query(query);
            matches = black_box(navigator.filtered_indices.len());
        }
    }
    let ns_per_iter = start.elapsed().as_nanos() as f64 / ITERATIONS as f64;
    println!("navigator_filter_10000,{ITERATIONS},{ns_per_iter:.0},0,{matches}");
}

fn run_flash_word_windows() {
    let text = (0..800)
        .map(|line| format!("строка-{line:04} слово🦀 пример текста и слова\n"))
        .collect::<String>();
    let mut ranges = Vec::new();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let end = offset + line.len();
        ranges.push(offset..end);
        offset = end;
    }

    let mut application = Application::new();
    application.shell.set_editor_text(&text);
    for _ in 0..2 {
        application
            .shell
            .start_flash_ranges(&ranges, FlashKind::Word)
            .expect("benchmark ranges match the editor text");
    }
    let start = Instant::now();
    let mut targets = 0;
    for _ in 0..ITERATIONS {
        application
            .shell
            .start_flash_ranges(&ranges, FlashKind::Word)
            .expect("benchmark ranges match the editor text");
        targets = black_box(application.shell.flash().targets().len());
    }
    let ns_per_iter = start.elapsed().as_nanos() as f64 / ITERATIONS as f64;
    println!("flash_word_windows,{ITERATIONS},{ns_per_iter:.0},0,{targets}");
}

fn main() {
    println!("scenario,iterations,ns_per_iter,layout_builds,retained");
    for scenario in [
        Scenario::Idle,
        Scenario::InsertCharacter,
        Scenario::InsertNewline,
        Scenario::CursorDown,
    ] {
        run(scenario);
    }
    run_navigator_filter();
    run_flash_word_windows();
}
