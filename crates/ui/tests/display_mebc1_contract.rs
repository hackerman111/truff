use truff_core::vim::Key;
use truff_core::{Buffer, BytePos};
use truff_ui::editor::display::{DisplayMap, ProjectionMode};
use truff_ui::shell::Shell;

#[test]
fn test_source_mode_shows_raw_markdown_unconditionally() {
    let raw = "# Title\n\n**bold** *italic* `code` [link](url)\n- [ ] task";
    let mut map = DisplayMap::new(raw, ProjectionMode::Source);

    // Cursor at start
    assert_eq!(map.visual_text(), raw);

    // Cursor inside bold
    let bold_pos = raw.find("bold").unwrap();
    map.set_cursor(BytePos::new(bold_pos));
    assert_eq!(
        map.visual_text(),
        raw,
        "Source mode must always display raw markdown"
    );

    // Cursor inside code
    let code_pos = raw.find("code").unwrap();
    map.set_cursor(BytePos::new(code_pos));
    assert_eq!(
        map.visual_text(),
        raw,
        "Source mode must always display raw markdown"
    );
}

#[test]
fn test_live_preview_mode_reveals_on_cursor_intersection() {
    let source = "Prefix **bold text** and *italic* suffix.";
    let mut map = DisplayMap::new(source, ProjectionMode::LivePreview);

    // 1. Cursor at 0 (outside): markers hidden
    map.set_cursor(BytePos::new(0));
    assert_eq!(map.visual_text(), "Prefix bold text and italic suffix.");

    // 2. Cursor inside "**bold text**" (e.g. on 'b' at byte 9): reveals **
    let b_pos = source.find("bold").unwrap();
    map.set_cursor(BytePos::new(b_pos));
    assert_eq!(
        map.visual_text(),
        "Prefix **bold text** and italic suffix.",
        "LivePreview must reveal markers when cursor touches the formatted span"
    );

    // 3. Cursor moves to "suffix" (byte 34): markers hidden again
    let s_pos = source.find("suffix").unwrap();
    map.set_cursor(BytePos::new(s_pos));
    assert_eq!(
        map.visual_text(),
        "Prefix bold text and italic suffix.",
        "LivePreview must re-hide markers when cursor moves away"
    );
}

#[test]
fn test_reading_mode_unconditionally_hides_markers() {
    let source = "Prefix **bold text** and *italic* suffix.";
    let mut map = DisplayMap::new(source, ProjectionMode::Reading);

    // In Reading mode, even if cursor is placed on "bold", markers MUST remain hidden (§4.3)
    let b_pos = source.find("bold").unwrap();
    map.set_cursor(BytePos::new(b_pos));
    assert_eq!(
        map.visual_text(),
        "Prefix bold text and italic suffix.",
        "Reading mode must unconditionally hide markers regardless of cursor position"
    );
}

#[test]
fn test_unicode_grapheme_cluster_preservation() {
    // Unicode test with Latin 'W', Cyrillic 'Ш', emoji '🧑🏽‍💻', combining accent 'e\u{0301}', and family emoji '👨‍👩‍👧‍👦'
    let text = "W Ш 🧑🏽‍💻 e\u{0301} 👨‍👩‍👧‍👦 end";
    let buffer = Buffer::from_str(text);

    // 1. Forward traversal via grapheme_next
    let mut pos = BytePos::ZERO;
    let mut clusters = Vec::new();
    while pos < BytePos::new(buffer.byte_len()) {
        let next = buffer
            .grapheme_next(pos)
            .unwrap_or(BytePos::new(buffer.byte_len()));
        clusters.push(buffer.slice(pos.get()..next.get()).to_string());
        pos = next;
    }

    assert_eq!(
        clusters,
        vec![
            "W",
            " ",
            "Ш",
            " ",
            "🧑🏽‍💻",
            " ",
            "e\u{0301}",
            " ",
            "👨‍👩‍👧‍👦",
            " ",
            "e",
            "n",
            "d"
        ]
    );

    // 2. Backward traversal via grapheme_prev from end of family emoji
    let family_start = text.find("👨‍👩‍👧‍👦").unwrap();
    let family_len = "👨‍👩‍👧‍👦".len();
    let family_end = BytePos::new(family_start + family_len);
    let prev = buffer.grapheme_prev(family_end).unwrap();
    assert_eq!(
        prev.get(),
        family_start,
        "grapheme_prev must jump across the entire 👨‍👩‍👧‍👦 cluster in one step"
    );

    // 3. Technologist emoji with modifier and ZWJ
    let tech_start = text.find("🧑🏽‍💻").unwrap();
    let tech_len = "🧑🏽‍💻".len();
    let tech_end = BytePos::new(tech_start + tech_len);
    let tech_prev = buffer.grapheme_prev(tech_end).unwrap();
    assert_eq!(
        tech_prev.get(),
        tech_start,
        "grapheme_prev must jump across 🧑🏽‍💻 cluster in one step"
    );
}

#[test]
fn test_single_step_undo_restores_exact_bytes_and_selection() {
    let mut shell = Shell::new();
    shell.set_editor_text("initial clean document");

    // 1. Append text in Insert mode
    shell.feed_key(Key::Char('A'));
    shell.commit_text(" appended").unwrap();
    shell.feed_key(Key::Esc);
    assert_eq!(shell.editor_text(), "initial clean document appended");

    // 2. Single 'u' in Normal mode restores exact original text
    shell.feed_key(Key::Char('u'));
    assert_eq!(
        shell.editor_text(),
        "initial clean document",
        "Single 'u' must restore exact bytes before the insert session"
    );

    // 3. Redo with Ctrl+r restores the edit
    shell.feed_key(Key::Ctrl('r'));
    assert_eq!(shell.editor_text(), "initial clean document appended");
}
