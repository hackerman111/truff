//! CPU heap measurements, excluding the native window, GPU and allocator-retained pages.
#[path = "support/counting_allocator.rs"]
mod counting_allocator;

use counting_allocator::{CountingAllocator, measure};
use truff_core::text::{Buffer, BytePos};
use truff_ui::{
    app::Application,
    editor::{DisplayMap, ProjectionMode},
    native::NativeApp,
    navigator::{NavigatorItem, NavigatorMode, NavigatorState},
    shell::Shell,
};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn files() -> Vec<String> {
    (0..10_000)
        .map(|idx| {
            format!(
                "folder_{:02}/sub_{:02}/note_{idx:05}.md",
                idx % 100,
                idx % 10
            )
        })
        .collect()
}

fn frame(context: &egui::Context, app: &mut NativeApp) {
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1100.0, 700.0),
            )),
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();
    std::hint::black_box(output);
}

fn main() {
    println!("scenario,live_delta_bytes,peak_delta_bytes,allocations,ns");
    for _ in 0..5 {
        let mut shell = Shell::new();
        measure("sidebar_10000", || shell.set_sidebar_files(files()));

        let mut navigator = NavigatorState::new();
        measure("navigator_open_10000", || {
            let items = files()
                .into_iter()
                .enumerate()
                .map(|(idx, path)| {
                    let name = path.rsplit('/').next().unwrap().to_owned();
                    NavigatorItem::new_file(idx as u64, &name, path, false, 0)
                })
                .collect();
            navigator.open_mode(NavigatorMode::Files, "benchmark", items);
        });
        measure("navigator_close_10000", || navigator.close());

        let text = "plain text with 字🦀\n".repeat(100);
        let maps = measure("plain_display_1000", || {
            (0..1000)
                .map(|_| DisplayMap::new(&text, ProjectionMode::LivePreview))
                .collect::<Vec<_>>()
        });
        drop(maps);

        let context = egui::Context::default();
        let mut application = Application::new();
        application
            .shell
            .set_editor_text(&"plain **bold** and 字🦀\n".repeat(30_000));
        let mut app = NativeApp::new(application, &context);
        measure("native_first_frame", || frame(&context, &mut app));
        println!("cached_paragraphs,{}", app.editor.cached_paragraph_count());
        measure("native_idle_100", || {
            for _ in 0..100 {
                frame(&context, &mut app);
            }
        });
    }
    let text = format!("{}a\u{301}🦀", "x".repeat(8 * 1024 * 1024));
    let buffer = Buffer::from_str(&text);
    let offset = text.len() - 5;
    for _ in 0..5 {
        let copied = measure("copied_cursor_snap_8mib", || {
            let copy = buffer.as_str();
            std::hint::black_box(truff_ui::editor::snap_to_grapheme_boundary(&copy, offset))
        });
        let borrowed = measure("rope_cursor_snap_8mib", || {
            std::hint::black_box(buffer.snap_to_grapheme_boundary(BytePos::new(offset)))
        });
        assert_eq!(copied, borrowed.get());
    }
}
