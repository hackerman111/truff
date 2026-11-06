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

    assert!(
        !v1.contains('|'),
        "Table pipes should not appear as raw ASCII '|' in visual text: v1={:?}",
        v1
    );
    assert!(
        !v2.contains('|'),
        "Table pipes should not appear as raw ASCII '|' in visual text: v2={:?}",
        v2
    );

    let source_pipes1 = truff_core::markdown::table::find_table_pipe_indices(row1);
    let source_pipes2 = truff_core::markdown::table::find_table_pipe_indices(row2);

    let pipe_cols1: Vec<usize> = source_pipes1
        .iter()
        .map(|&p| {
            let vis_byte = display1
                .source_to_visual(BytePos::new(p), truff_ui::editor::display::Affinity::Right)
                .get();
            v1[..vis_byte.min(v1.len())].chars().count()
        })
        .collect();

    let pipe_cols2: Vec<usize> = source_pipes2
        .iter()
        .map(|&p| {
            let vis_byte = display2
                .source_to_visual(BytePos::new(p), truff_ui::editor::display::Affinity::Right)
                .get();
            v2[..vis_byte.min(v2.len())].chars().count()
        })
        .collect();

    assert_eq!(
        pipe_cols1, pipe_cols2,
        "All column dividers must align at identical visual character columns across rows: v1={:?}, v2={:?}",
        v1, v2
    );

    // Coordinate mapping tests
    let visual_hit = display1.visual_to_source(truff_ui::editor::display::VisualPos(2));
    assert!(visual_hit.pos.get() >= 2 && visual_hit.pos.get() <= 18);

    // Source to visual roundtrip for the first pipe
    let vis_pipe0 =
        display1.source_to_visual(BytePos::new(0), truff_ui::editor::display::Affinity::Left);
    assert_eq!(vis_pipe0.get(), 0);
}

#[test]
fn test_table_with_math_and_bold_alignment() {
    let rows = [
        "| Представление | Как получают | Когда применяют |",
        "|---|---|---|",
        "| **Waveform** | Сырые отсчёты $x[n]$ | Waveform-frontends, wav2vec 2.0 |",
        "| **STFT** | Оконная DFT по участкам сигнала | Базовый временно-частотный анализ |",
        "| **Spectrogram** | $|X(t,f)|$ либо $|X(t,f)|^2$ | Визуализация и дальнейшая фильтрация |",
        "| **Log-Mel** | $S_{t,m}=\\log(\\epsilon+\\sum_f H_m(f)|X(t,f)|^2)$ | Стандартный feature input многих ASR |",
        "| **MFCC** | DCT от log-mel-энергий | Классические speech/HMM pipelines |",
        "| **Learned features** | Обучаемый CNN и/или SSL-энкодер | Pretrained wav2vec 2.0 / HuBERT / WavLM |",
    ];

    let widths =
        truff_core::markdown::table::compute_table_col_widths(rows.iter().copied()).unwrap();
    let arc_widths = std::sync::Arc::new(widths);

    let mut expected_pipes = None;

    for (idx, &row) in rows.iter().enumerate() {
        if idx == 1 {
            continue; // delimiter row
        }
        let pres = LinePresentation {
            table_header: idx == 0,
            table_col_widths: Some(arc_widths.clone()),
            ..LinePresentation::default()
        };
        let display = DisplayMap::from_owned_with_presentation(
            row.to_string(),
            ProjectionMode::LivePreview,
            Some(pres),
        );
        let visual = display.visual_text();
        println!("Row {}: {:?}", idx, visual);
        let source_pipes = truff_core::markdown::table::find_table_pipe_indices(row);
        let pipes: Vec<usize> = source_pipes
            .iter()
            .map(|&p| {
                let vis_byte = display
                    .source_to_visual(BytePos::new(p), truff_ui::editor::display::Affinity::Right)
                    .get();
                visual[..vis_byte.min(visual.len())].chars().count()
            })
            .collect();

        assert_eq!(
            pipes.len(),
            4,
            "Row {} must have exactly 4 delimiter pipes, got {}: visual={:?}",
            idx,
            pipes.len(),
            visual
        );

        let cols = display.table_pipe_visual_cols();
        assert_eq!(cols.len(), 4);
        assert_eq!(
            cols,
            vec![0, 19, 57, 99],
            "Pipe visual columns must match expected fixed columns for row {}",
            idx
        );

        if let Some(ref exp) = expected_pipes {
            assert_eq!(
                &pipes, exp,
                "Row {} pipes must align with row 0: visual={:?}",
                idx, visual
            );
        } else {
            expected_pipes = Some(pipes);
        }
    }
}
