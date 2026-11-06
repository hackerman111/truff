use truff_core::markdown::LinePresentation;
use truff_core::text::BytePos;
use truff_ui::editor::display::{DisplayMap, ProjectionMode};

#[test]
fn test_table_delimiter_hidden_when_unfocused() {
    let row = "|---|---|";
    let presentation = LinePresentation {
        table_delimiter: true,
        ..LinePresentation::default()
    };

    let mut display = DisplayMap::from_owned_with_presentation(
        row.to_string(),
        ProjectionMode::LivePreview,
        Some(presentation),
    );
    display.set_cursor(BytePos::new(usize::MAX)); // Cursor not on this line

    // In LivePreview without cursor, the delimiter row text should be hidden
    assert!(
        display.visual_text().is_empty(),
        "Delimiter text should be hidden when unfocused"
    );
}

#[test]
fn test_table_delimiter_revealed_when_focused() {
    let row = "|---|---|";
    let presentation = LinePresentation {
        table_delimiter: true,
        ..LinePresentation::default()
    };

    let mut display = DisplayMap::from_owned_with_presentation(
        row.to_string(),
        ProjectionMode::LivePreview,
        Some(presentation),
    );
    display.set_cursor(BytePos::new(2)); // Cursor inside the line

    // In LivePreview with cursor, delimiter row text is revealed
    assert_eq!(
        display.visual_text(),
        "|---|---|",
        "Delimiter text should be revealed when focused"
    );
}

#[test]
fn test_table_delimiter_unconditionally_hidden_in_reading_mode() {
    let row = "|---|---|";
    let presentation = LinePresentation {
        table_delimiter: true,
        ..LinePresentation::default()
    };

    let mut display = DisplayMap::from_owned_with_presentation(
        row.to_string(),
        ProjectionMode::Reading,
        Some(presentation),
    );
    display.set_cursor(BytePos::new(2)); // Even with cursor in Reading mode

    assert!(
        display.visual_text().is_empty(),
        "Delimiter text must be hidden in Reading mode"
    );
}
