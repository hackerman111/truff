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

#[test]
fn test_table_rows_visually_aligned() {
    let row1 = "| Сценарий | Базовый кандидат | Критерий |";
    let row2 = "| Мало размеченных аудиочасов | wav2vec 2.0/HuBERT + CTC | WER на малой разметке и out-of-domain |";

    let col_widths = std::sync::Arc::new(vec![31, 26, 40]);
    let pres1 = LinePresentation {
        table_col_widths: Some(col_widths.clone()),
        ..LinePresentation::default()
    };
    let pres2 = LinePresentation {
        table_col_widths: Some(col_widths.clone()),
        ..LinePresentation::default()
    };

    let display1 = DisplayMap::from_owned_with_presentation(
        row1.to_string(),
        ProjectionMode::LivePreview,
        Some(pres1),
    );
    let display2 = DisplayMap::from_owned_with_presentation(
        row2.to_string(),
        ProjectionMode::LivePreview,
        Some(pres2),
    );

    let v1 = display1.visual_text();
    let v2 = display2.visual_text();

    let char_pipes = |s: &str| -> Vec<usize> {
        s.chars()
            .enumerate()
            .filter_map(|(idx, c)| if c == '|' { Some(idx) } else { None })
            .collect()
    };

    let pipes1 = char_pipes(v1);
    let pipes2 = char_pipes(v2);

    assert_eq!(
        pipes1, pipes2,
        "All column pipes must align at identical visual character columns across rows: v1={:?}, v2={:?}",
        v1, v2
    );

    // Coordinate mapping tests
    let visual_hit = display1.visual_to_source(truff_ui::editor::display::VisualPos(2));
    assert!(visual_hit.pos.get() >= 2 && visual_hit.pos.get() <= 18);

    // Source to visual roundtrip for the first pipe
    let vis_pipe0 =
        display1.source_to_visual(BytePos::new(0), truff_ui::editor::display::Affinity::Right);
    assert_eq!(vis_pipe0.get(), 0);
}
