use truff_core::vim::Key;
use truff_core::{Buffer, BytePos};
use truff_ui::app::Application;
use truff_ui::overlays::LinkCandidate;

#[test]
fn test_link_completion_trigger_detection() {
    let buffer = Buffer::from_str("Here is a link to [[");
    let cursor = 20; // right after "[["
    let trigger = truff_ui::overlays::LinkCompletionOverlay::detect_trigger(&buffer, cursor);
    assert_eq!(
        trigger,
        Some((18, String::new())),
        "detect_trigger should detect '[[' at byte 18 with empty query"
    );

    let buffer2 = Buffer::from_str("Here is [[Note#Section");
    let cursor2 = 22;
    let trigger2 = truff_ui::overlays::LinkCompletionOverlay::detect_trigger(&buffer2, cursor2);
    assert_eq!(
        trigger2,
        Some((8, "Note#Section".to_string())),
        "detect_trigger should extract query 'Note#Section'"
    );

    let buffer3 = Buffer::from_str("Here is [[Closed Link]] plain text");
    let cursor3 = buffer3.byte_len();
    let trigger3 = truff_ui::overlays::LinkCompletionOverlay::detect_trigger(&buffer3, cursor3);
    assert_eq!(
        trigger3, None,
        "detect_trigger must return None when all brackets are closed"
    );
}

#[test]
fn test_heading_completion_candidates() {
    let mut app = Application::new();
    let doc = "# Main Title\n\n## First Heading\n\n## Second Heading\n\nContent";
    app.shell.set_editor_text(doc);

    // Refresh link completion for query "#" (headings in current note)
    app.refresh_link_completions(0, "#");
    assert!(app.link_completion.active);
    assert!(!app.link_completion.candidates.is_empty());

    let displays: Vec<_> = app
        .link_completion
        .candidates
        .iter()
        .map(|c| c.display.as_str())
        .collect();
    assert!(displays.iter().any(|d| d.contains("Main Title")));
    assert!(displays.iter().any(|d| d.contains("First Heading")));
    assert!(displays.iter().any(|d| d.contains("Second Heading")));
}

#[test]
fn test_link_completion_apply_replaces_trigger_with_wikilink() {
    let mut app = Application::new();
    app.shell.set_editor_text("See [[");
    // Place cursor right after "[["
    let cursor = 6;
    if let Some(id) = app.shell.workspace().active_view() {
        app.shell
            .workspace_mut()
            .get_view_mut(id)
            .unwrap()
            .set_cursor(BytePos::new(cursor), BytePos::new(cursor));
    }

    app.link_completion.active = true;
    app.link_completion.trigger_byte = 4;
    app.link_completion.query = String::new();
    app.link_completion.candidates = vec![LinkCandidate {
        display: "Architecture.md".into(),
        target: "Architecture".into(),
        description: "Vault note".into(),
    }];
    app.link_completion.selected = 0;

    // Apply completion
    app.apply_link_completion().expect("apply link completion");

    assert_eq!(app.shell.editor_text(), "See [[Architecture]]");
    assert!(!app.link_completion.active);

    // Single undo restores state before completion
    app.shell.feed_key(Key::Char('u'));
    assert_eq!(app.shell.editor_text(), "See [[");
}

#[test]
fn test_visual_mode_link_selection_wraps_target_and_alias() {
    let mut app = Application::new();
    app.shell
        .set_editor_text("We need to discuss memory layout tomorrow.");

    // Select "memory layout" (bytes 19..32) in VisualChar mode
    if let Some(id) = app.shell.workspace().active_view() {
        // v at byte 19, move to byte 31
        app.shell
            .workspace_mut()
            .get_view_mut(id)
            .unwrap()
            .set_cursor(BytePos::new(19), BytePos::new(31));
    }
    app.shell.feed_key(Key::Char('v'));

    // Execute link creation with target "Architecture#Memory"
    app.link_selection("Architecture#Memory")
        .expect("link selection");

    assert_eq!(
        app.shell.editor_text(),
        "We need to discuss [[Architecture#Memory|memory layout]] tomorrow."
    );

    // Single undo restores original text
    app.shell.feed_key(Key::Char('u'));
    assert_eq!(
        app.shell.editor_text(),
        "We need to discuss memory layout tomorrow."
    );
}

#[test]
fn test_visual_mode_link_selection_omits_alias_when_target_matches() {
    let mut app = Application::new();
    app.shell.set_editor_text("Read Architecture carefully.");

    // Select "Architecture" (bytes 5..17)
    if let Some(id) = app.shell.workspace().active_view() {
        app.shell
            .workspace_mut()
            .get_view_mut(id)
            .unwrap()
            .set_cursor(BytePos::new(5), BytePos::new(16));
    }
    app.shell.feed_key(Key::Char('v'));

    // Execute link creation with target "Architecture"
    app.link_selection("Architecture").expect("link selection");

    // Target matches selected text -> should be [[Architecture]], not [[Architecture|Architecture]] (§5.2)
    assert_eq!(app.shell.editor_text(), "Read [[Architecture]] carefully.");
}
