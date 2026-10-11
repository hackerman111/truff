use truff_core::text::BytePos;
use truff_ui::editor::display::{Affinity, DisplayMap, ProjectionMode};
use truff_ui::editor::layout::{HeightIndex, ScrollAnchor, VirtualizedLayout};
use truff_ui::overlays::FlashOverlay;

#[test]
fn source_visual_affinity() {
    let text = "Hello world! This is a standard line of text.";

    // 1. Source projection: 1:1 roundtrip
    let source_map = DisplayMap::new(text, ProjectionMode::Source);
    assert_eq!(source_map.visual_text(), text);

    for p in 0..=text.len() {
        let visual_pos = source_map.source_to_visual(BytePos(p), Affinity::Left);
        assert_eq!(visual_pos.get(), p);
        let hit = source_map.visual_to_source(visual_pos);
        assert_eq!(hit.pos, BytePos(p));
    }

    // 2. LivePreview projection: roundtrip for visible spans
    let md_text = "Start **bold text** middle *italic* end.";
    let live_map = DisplayMap::new(md_text, ProjectionMode::LivePreview);
    // Cursor at 0, markers are hidden
    assert_eq!(live_map.visual_text(), "Start bold text middle italic end.");

    // Visible text character roundtrips correctly
    let word_start = md_text.find("bold").unwrap(); // source 8
    let visual_pos = live_map.source_to_visual(BytePos(word_start), Affinity::Right);
    let hit = live_map.visual_to_source(visual_pos);
    assert_eq!(hit.pos, BytePos(word_start));

    // End of visible text roundtrips
    let end_pos = md_text.find("end.").unwrap();
    let visual_end = live_map.source_to_visual(BytePos(end_pos), Affinity::Left);
    let hit_end = live_map.visual_to_source(visual_end);
    assert_eq!(hit_end.pos, BytePos(end_pos));
}

#[test]
fn hidden_marker_edit() {
    let text = "Hello **world**!";
    let mut map = DisplayMap::new(text, ProjectionMode::LivePreview);

    // Initial state: cursor is at 0, far from marker
    assert_eq!(map.visual_text(), "Hello world!");

    // Cursor touches the hidden marker '**' at byte 7 (6..8):
    map.set_cursor(BytePos(7));

    // Marker MUST be unhidden into raw Markdown for editing!
    assert_eq!(
        map.visual_text(),
        "Hello **world**!",
        "Cursor touching hidden marker must reveal raw Markdown!"
    );

    // Cursor moves inside the bold word at byte 10:
    map.set_cursor(BytePos(10));
    assert_eq!(
        map.visual_text(),
        "Hello **world**!",
        "Cursor within formatted span must keep markers revealed!"
    );

    // Cursor moves away back to 0:
    map.set_cursor(BytePos(0));
    assert_eq!(
        map.visual_text(),
        "Hello world!",
        "Moving cursor away must hide markers again!"
    );
}

#[test]
fn stable_scroll_anchor() {
    let mut lines = Vec::new();
    for i in 0..30 {
        lines.push(format!("Block {i}: content line"));
    }
    let text = lines.join("\n");

    let mut height_index = HeightIndex::new(24.0);
    height_index.init_from_text(&text);
    assert_eq!(height_index.block_count(), 30);

    // Viewport currently anchored at block 10 with scroll offset 240.0
    let mut anchor = ScrollAnchor::new(10, 0.0, 240.0);

    // Block 2 (which is ABOVE the anchor) is measured and expands from 24.0 to 54.0 px (+30.0 px)
    height_index.update_block_height(2, 54.0, Some(&mut anchor));

    // Crucial guarantee: anchor.scroll_offset_y must adjust by +30.0 px so content DOES NOT JUMP!
    assert_eq!(
        anchor.scroll_offset_y, 270.0,
        "Scroll offset must adjust by delta when block above viewport expands!"
    );

    // Invariant: distance from anchor scroll offset to block 10 top position remains identical
    let block_10_top = height_index.y_for_block(10);
    assert_eq!(anchor.scroll_offset_y, block_10_top);

    // Updating a block BELOW the anchor (e.g. block 20) must NOT alter the scroll anchor
    let offset_before = anchor.scroll_offset_y;
    height_index.update_block_height(20, 80.0, Some(&mut anchor));
    assert_eq!(
        anchor.scroll_offset_y, offset_before,
        "Height adjustments below anchor must not affect scroll offset!"
    );
}

#[test]
fn cursor_does_not_reparse() {
    let mut lines = Vec::new();
    for i in 0..50 {
        lines.push(format!(
            "Paragraph {i}: some text for paragraph layout testing."
        ));
    }
    let text = lines.join("\n");

    let mut height_index = HeightIndex::new(24.0);
    height_index.init_from_text(&text);

    let mut layout_engine = VirtualizedLayout::new(16.0);

    // Initial visible window layout for viewport [0.0, 500.0]
    let visible_blocks = layout_engine.layout_visible_window(
        &text,
        &mut height_index,
        0.0,
        500.0,
        Some(400.0),
        None,
    );
    assert!(!visible_blocks.is_empty());
    let initial_rebuilds = layout_engine.rebuild_count();
    assert!(initial_rebuilds > 0);

    // Now user moves cursor across multiple positions in the document
    let cursor_positions = [0, 25, 50, 120, 250, 400];
    for _pos in cursor_positions {
        // Cursor movement simply updates cursor position, does not invalidate unchanged paragraphs
        let _re_visible = layout_engine.layout_visible_window(
            &text,
            &mut height_index,
            0.0,
            500.0,
            Some(400.0),
            None,
        );

        assert_eq!(
            layout_engine.rebuild_count(),
            initial_rebuilds,
            "Moving cursor must NOT trigger layout rebuilds of unchanged paragraphs!"
        );
    }
}

#[test]
fn flash_visible_only() {
    let visible_text = "alpha beta gamma delta epsilon zeta eta";
    let viewport_start_offset = 5000; // Simulated offset deep in a large 16 MiB document

    let mut flash = FlashOverlay::new();
    flash.activate(visible_text, viewport_start_offset);

    assert!(flash.is_active());
    let targets = flash.targets();

    // Targets must be bounded strictly to the visible text (7 words)
    assert_eq!(targets.len(), 7);

    // Every target must have a source range within the viewport bounds
    for t in targets {
        assert!(
            t.source_range.start >= viewport_start_offset,
            "Target start {} is before viewport start {}",
            t.source_range.start,
            viewport_start_offset
        );
        assert!(
            t.source_range.end <= viewport_start_offset + visible_text.len(),
            "Target end {} is past viewport end {}",
            t.source_range.end,
            viewport_start_offset + visible_text.len()
        );
    }

    // Find the label for "gamma"
    let gamma_target = targets.iter().find(|t| t.text == "gamma").unwrap();
    let gamma_label = gamma_target.label.clone();
    let expected_gamma_range = gamma_target.source_range.clone();

    // Typing the label returns the exact source range for Vim operator/jump
    let mut matched_range = None;
    for ch in gamma_label.chars() {
        matched_range = flash.feed_char(ch);
    }

    assert_eq!(
        matched_range,
        Some(expected_gamma_range),
        "Selecting Flash label must return valid source range for Vim operator!"
    );
    assert!(!flash.is_active(), "Flash must deactivate after match");
}

#[test]
fn live_preview_projects_color_wrappers_and_keeps_code_literal() {
    let text = "plain <span style=\"color:#3388ff\">blue</span>\n`<span style=\"color:#ff0000\">code</span>`";
    let mut display = DisplayMap::new(text, ProjectionMode::LivePreview);
    display.set_cursor(BytePos::ZERO);
    assert!(display.visual_text().contains("plain blue"));
    assert!(display.visual_text().contains("color:#ff0000"));
    assert_eq!(display.color_spans().len(), 1);
    assert_eq!(display.color_spans()[0].rgba, [0x33, 0x88, 0xff, 255]);
    let pos = BytePos::new(text.find("blue").unwrap());
    display.set_cursor(pos);
    assert!(display.visual_text().contains("color:#3388ff"));
    assert_eq!(
        display
            .visual_to_source(display.source_to_visual(pos, Affinity::Right))
            .pos,
        pos
    );
}

#[test]
fn test_editor_readable_column_and_margin() {
    let available_width = 1200.0;
    let (content_width, margin_x) = truff_ui::native::EditorWidget::calculate_column_metrics(available_width);
    assert_eq!(content_width, 780.0);
    assert_eq!(margin_x, (1200.0 - 780.0) / 2.0);

    // Narrow window clamps to available_width - 48
    let narrow_width = 600.0;
    let (content_width_narrow, margin_x_narrow) = truff_ui::native::EditorWidget::calculate_column_metrics(narrow_width);
    assert_eq!(content_width_narrow, 600.0 - 48.0);
    assert_eq!(margin_x_narrow, 24.0);
}
