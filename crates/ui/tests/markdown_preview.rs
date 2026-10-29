use egui::{Context, Event, Pos2, RawInput, Rect, vec2};
use truff_ui::{
    app::Application,
    native::NativeApp,
    navigator::{NavigatorMode, render::markdown},
    theme::Theme,
};

#[test]
fn test_markdown_preview_renders_formatted_blocks_without_panic() {
    let ctx = Context::default();
    let theme = Theme::default();

    let sample_md = r#"# Main Heading
This is a **bold** and *italic* paragraph with `inline code` and [a link](https://example.com) and [[WikiLink]].

## Sub Heading

- Bullet 1
- Bullet 2
  - Nested bullet

1. Numbered one
2. Numbered two

- [ ] Unfinished task
- [x] Completed task

> [!tip]
> This is a helpful callout tip!

```rust
fn main() {
    println!("Hello from code block");
}
```

---

| Column 1 | Column 2 |
| -------- | -------- |
| Value A  | Value B  |
"#;

    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
            ..Default::default()
        },
        |ui| {
            markdown::render(ui, sample_md, &theme);
        },
    );

    // Shapes should be rendered (text, frames, lines)
    assert!(
        !output.shapes.is_empty(),
        "markdown rendering must produce graphical shapes"
    );
    output.textures_delta.clear();
}

#[test]
fn test_file_preview_with_markdown_in_navigator() {
    let ctx = Context::default();
    let mut application = Application::new();

    let temp_dir = std::env::temp_dir().join(format!("truff-md-preview-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let note_path = temp_dir.join("formatted.md");
    std::fs::write(
        &note_path,
        "# Title\n\nParagraph with **bold** text.\n\n- [ ] Task\n",
    )
    .unwrap();

    application.open_vault(&temp_dir).unwrap();
    let mut app = NativeApp::new(application, &ctx);

    app.open_navigator(NavigatorMode::Files);
    assert!(app.navigator.preview_visible);

    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 700.0))),
            events: vec![Event::Text("formatted".into())],
            ..Default::default()
        },
        |ui| {
            app.show(ui);
        },
    );

    assert!(!output.shapes.is_empty());
    output.textures_delta.clear();
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_long_text_in_list_item_and_paragraph_wraps_within_available_width() {
    let ctx = Context::default();
    let theme = Theme::default();

    let long_item = "- багуля с кривой подсветкой в visual моде. Выделение символа должно выделять весь символ по его длине, а не фиксированной длине. Строка должна выделяться по самому высокому символу. Сейчас подсветка идет по длине обычного символа поэтому оглавление криво подсвечивается";
    let available_width = 300.0;

    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
            ..Default::default()
        },
        |ui| {
            ui.set_max_width(available_width);
            markdown::render(ui, long_item, &theme);
            assert!(
                ui.min_rect().width() <= available_width + 1.0,
                "Rendered width {} exceeded max width {}",
                ui.min_rect().width(),
                available_width
            );
        },
    );
    output.textures_delta.clear();
}

#[test]
fn test_file_preview_with_long_lines_does_not_expand_navigator_modal() {
    let ctx = Context::default();
    let mut application = Application::new();

    let temp_dir = std::env::temp_dir().join(format!("truff-long-md-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let note_path = temp_dir.join("long_notes.md");
    let content = r#"# Notes
- багуля с кривой подсветкой в visual моде. Выделение символа должно выделять весь символ по его длине, а не фиксированной длине. Строка должна выделяться по самому высокому символу. Сейчас подсветка идет по длине обычного символа поэтому оглавление криво подсвечивается
- при нажатии s при вводе названия папки активируется режим flash.nvim
- делать парсинг markdown.
- перенести полноценный vim-motions для всего и панели, чтобы избежать работы с мышкой. Для всего что связано с текстом vimmotions должен быть реализован на уровне хоткеев.
"#;
    std::fs::write(&note_path, content).unwrap();

    application.open_vault(&temp_dir).unwrap();
    let mut app = NativeApp::new(application, &ctx);

    app.open_navigator(NavigatorMode::Files);
    assert!(app.navigator.preview_visible);

    let screen_size = vec2(1000.0, 700.0);
    let expected_max_modal_width = Theme::default().geometry.navigator_size(screen_size).x;

    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen_size)),
            events: vec![Event::Text("long_notes".into())],
            ..Default::default()
        },
        |ui| {
            app.show(ui);
        },
    );
    output.textures_delta.clear();

    // Second frame to render with preview loaded
    let mut output2 = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen_size)),
            ..Default::default()
        },
        |ui| {
            app.show(ui);
        },
    );

    // Verify all text galleys are wrapped within expected modal width
    for shape in &output2.shapes {
        if let egui::Shape::Text(text) = &shape.shape {
            assert!(
                text.galley.rect.width() <= expected_max_modal_width,
                "Text row {:?} with width {} exceeded expected modal width {}",
                text.galley.job.text,
                text.galley.rect.width(),
                expected_max_modal_width
            );
        }
    }

    output2.textures_delta.clear();
    let _ = std::fs::remove_dir_all(&temp_dir);
}
