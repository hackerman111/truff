use masonry::core::{ErasedAction, NewWidget, WidgetId};
use masonry::dpi::LogicalSize;
use masonry::theme::default_property_set;
use masonry_winit::app::{AppDriver, DriverCtx, NewWindow, WindowId};
use masonry_winit::winit::window::Window;
use truff_ui::editor::{EditorProbe, EditorWidget};

struct TruffDriver;

impl AppDriver for TruffDriver {
    fn on_action(
        &mut self,
        _window_id: WindowId,
        _ctx: &mut DriverCtx<'_, '_>,
        _widget_id: WidgetId,
        _action: ErasedAction,
    ) {
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args
        .iter()
        .any(|arg| arg == "--probe" || arg == "--headless")
    {
        println!("Truff v2 Editor Probe (headless benchmark)");
        let mut probe = EditorProbe::new(
            "Truff v2 editor probe initial text\nTesting Parley & Crop\nПривет мир!",
        );
        probe.relayout(Some(800.0));
        let hit = probe.hit_test(100.0, 10.0);
        println!(
            "Probe layout height: {}, hit test offset: {hit}",
            probe.hit_test(10.0, 10.0)
        );
        return;
    }

    let sample_text = "Welcome to Truff v2!\n\nMinimal retained editor probe using crop::Rope and Parley.\nПоддержка Unicode, кириллицы, emoji 🦀 и IME.\n";
    let editor = EditorWidget::new(sample_text);

    let window_size = LogicalSize::new(900.0, 600.0);
    let window_attributes = Window::default_attributes()
        .with_title("Truff v2 — Editor Probe")
        .with_resizable(true)
        .with_min_inner_size(window_size);

    let driver = TruffDriver;
    let event_loop = masonry_winit::app::EventLoop::with_user_event()
        .build()
        .expect("Failed to create event loop");

    masonry_winit::app::run_with(
        event_loop,
        vec![NewWindow::new(
            window_attributes,
            NewWidget::new(editor).erased(),
        )],
        driver,
        default_property_set(),
    )
    .expect("Failed to run Masonry application");
}
