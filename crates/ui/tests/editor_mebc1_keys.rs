use egui::{Rect, Vec2};
use truff_core::vim::{Key, Mode, ScrollPosition};
use truff_core::{Buffer, BytePos};
use truff_ui::app::global_config::TypographyConfig;
use truff_ui::editor::ProjectionMode;
use truff_ui::native::EditorWidget;
use truff_ui::shell::{Shell, ShellAction};
use truff_ui::theme::Theme;

fn run_widget(
    widget: &mut EditorWidget,
    buffer: &Buffer,
    view: &truff_core::workspace::View,
    viewport_size: Vec2,
) {
    let ctx = egui::Context::default();
    let raw_input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, viewport_size)),
        ..Default::default()
    };
    let theme = Theme::nord();
    let typography = TypographyConfig::default();
    let mut output = ctx.run_ui(raw_input, |ui| {
        widget.show(
            ui,
            Some(buffer),
            Some(view),
            Mode::Normal,
            ProjectionMode::Source,
            &theme,
            &typography,
            None,
            &[],
            None,
        );
    });
    output.textures_delta.clear();
}

#[test]
fn test_shift_enter_produces_literal_newline_in_insert_mode() {
    let mut shell = Shell::new();
    shell.set_editor_text("- list item");
    // Move to end of line and enter insert mode
    shell.feed_key(Key::Char('A'));
    assert_eq!(shell.mode(), Mode::Insert);

    // Shift+Enter MUST NOT continue bullet list (§3.2)
    shell.feed_key(Key::ShiftEnter);
    assert_eq!(
        shell.editor_text(),
        "- list item\n",
        "Shift+Enter should insert literal newline without bullet marker"
    );
}

#[test]
fn test_ctrl_enter_toggles_task_checkbox_under_cursor() {
    let mut shell = Shell::new();
    shell.set_editor_text("- [ ] task one\n- [x] task two");

    // Position cursor on "task one" (e.g. byte 8)
    if let Some(id) = shell.workspace().active_view() {
        let pos = BytePos::new(8);
        shell
            .workspace_mut()
            .get_view_mut(id)
            .unwrap()
            .set_cursor(pos, pos);
    }

    // Normal mode Ctrl+Enter toggles - [ ] to - [x]
    shell.feed_key(Key::CtrlEnter);
    assert_eq!(
        shell.editor_text(),
        "- [x] task one\n- [x] task two",
        "Ctrl+Enter should toggle - [ ] to - [x]"
    );

    // Ctrl+Enter again toggles back to - [ ]
    shell.feed_key(Key::CtrlEnter);
    assert_eq!(
        shell.editor_text(),
        "- [ ] task one\n- [x] task two",
        "Ctrl+Enter again should toggle - [x] to - [ ]"
    );

    // Move to second line (byte 20) and enter Insert mode
    if let Some(id) = shell.workspace().active_view() {
        let pos = BytePos::new(20);
        shell
            .workspace_mut()
            .get_view_mut(id)
            .unwrap()
            .set_cursor(pos, pos);
    }
    shell.feed_key(Key::Char('i'));
    assert_eq!(shell.mode(), Mode::Insert);

    // In Insert mode, Ctrl+Enter still toggles the task checkbox (§3.1)
    shell.feed_key(Key::CtrlEnter);
    assert_eq!(
        shell.editor_text(),
        "- [ ] task one\n- [ ] task two",
        "Ctrl+Enter in Insert mode should toggle task checkbox"
    );

    // Single undo restores state
    shell.feed_key(Key::Esc);
    shell.feed_key(Key::Char('u'));
    assert_eq!(
        shell.editor_text(),
        "- [ ] task one\n- [x] task two",
        "Undo should restore task state"
    );
}

#[test]
fn test_checkbox_click_and_ctrl_enter_generate_identical_transaction() {
    let buffer = Buffer::from_str("- [ ] task item\n");
    let cursor = BytePos::new(8);

    // 1. Transaction via plan_toggle_task (Ctrl+Enter)
    let ctrl_enter_tx = truff_core::markdown::plan_toggle_task(&buffer, cursor)
        .expect("plan_toggle_task for task line");

    // 2. Transaction via toggle_task (checkbox click at byte index 3)
    let click_tx =
        truff_core::markdown::toggle_task(&buffer, 3).expect("toggle_task for checkbox at 3");

    // Transactions must be identical
    assert_eq!(
        ctrl_enter_tx, click_tx,
        "Clicking checkbox and Ctrl+Enter must generate the exact same transaction"
    );
}

#[test]
fn test_scroll_cursor_zz_zt_zb_positioning() {
    let lines = (0..100)
        .map(|i| format!("Line {i:03}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut shell = Shell::new();
    shell.set_editor_text(&lines);
    let view_id = shell.workspace().active_view().unwrap();
    let line_50_byte = shell.editor_buffer().unwrap().byte_of_line(50);
    shell
        .workspace_mut()
        .get_view_mut(view_id)
        .unwrap()
        .set_cursor(BytePos::new(line_50_byte), BytePos::new(line_50_byte));

    let mut widget = EditorWidget::default();
    let viewport_size = Vec2::new(800.0, 600.0);

    let buffer = shell.editor_buffer().unwrap();
    let view = shell.workspace().get_view(view_id).unwrap();

    // First frame layout
    run_widget(&mut widget, buffer, view, viewport_size);

    // 1. zz -> Center
    widget.scroll_cursor(ScrollPosition::Center);
    run_widget(&mut widget, buffer, view, viewport_size);
    let scroll_center = widget.scroll_offset();
    let expected_center = EditorWidget::TOP_MARGIN + 50.0 * 24.0 - (600.0 - 24.0) / 2.0;
    assert!(
        (scroll_center - expected_center).abs() < 24.0,
        "zz should center line 50 in viewport: expected ~{expected_center}, got {scroll_center}"
    );
    assert_eq!(
        view.head().get(),
        line_50_byte,
        "Cursor byte pos must not change"
    );

    // 2. zt -> Top
    widget.scroll_cursor(ScrollPosition::Top);
    run_widget(&mut widget, buffer, view, viewport_size);
    let scroll_top = widget.scroll_offset();
    let expected_top = EditorWidget::TOP_MARGIN + 50.0 * 24.0;
    assert!(
        (scroll_top - expected_top).abs() < 24.0,
        "zt should align line 50 at top of viewport: expected ~{expected_top}, got {scroll_top}"
    );
    assert_eq!(
        view.head().get(),
        line_50_byte,
        "Cursor byte pos must not change"
    );

    // 3. zb -> Bottom
    widget.scroll_cursor(ScrollPosition::Bottom);
    run_widget(&mut widget, buffer, view, viewport_size);
    let scroll_bottom = widget.scroll_offset();
    let expected_bottom = EditorWidget::TOP_MARGIN + 50.0 * 24.0 - 600.0 + 24.0;
    assert!(
        (scroll_bottom - expected_bottom).abs() < 24.0,
        "zb should align line 50 at bottom of viewport: expected ~{expected_bottom}, got {scroll_bottom}"
    );
    assert_eq!(
        view.head().get(),
        line_50_byte,
        "Cursor byte pos must not change"
    );
}

#[test]
fn test_gf_on_link_emits_follow_link_action() {
    let mut shell = Shell::new();
    shell.set_editor_text("Check this out: [[Target Note]] for details.");

    // Position cursor inside [[Target Note]]
    if let Some(id) = shell.workspace().active_view() {
        let pos = BytePos::new(20);
        shell
            .workspace_mut()
            .get_view_mut(id)
            .unwrap()
            .set_cursor(pos, pos);
    }

    // Press gf
    shell.feed_key(Key::Char('g'));
    let action = shell.feed_key(Key::Char('f'));

    // gf must emit FollowLink or RegisteredCommand for note.follow_link
    assert!(
        matches!(
            action,
            ShellAction::RegisteredCommand { ref id, .. } if id.as_str() == "note.follow_link"
        ),
        "gf on a link should emit note.follow_link action, got {action:?}"
    );
}
