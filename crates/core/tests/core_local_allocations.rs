use truff_core::markdown::style::remove_style;
use truff_core::{Buffer, BufferId, LineEnding, Transaction};

#[test]
fn crlf_edits_preserve_normalized_text_and_undo() {
    let mut buffer = Buffer::from_text(BufferId::new(101), "a\r\nb").unwrap();
    assert_eq!(buffer.line_ending(), LineEnding::CrLf);

    buffer.apply(Transaction::insert(4, "Ж\nx")).unwrap();
    assert_eq!(buffer.as_str(), "a\r\nbЖ\r\nx");
    buffer.undo().unwrap();
    assert_eq!(buffer.as_str(), "a\r\nb");
}

#[test]
fn crlf_normalization_counts_added_bytes_against_limit() {
    let mut buffer = Buffer::from_str("a\r\nb");
    buffer.set_max_file_bytes(6);

    let error = buffer.apply(Transaction::insert(4, "x\ny")).unwrap_err();
    assert!(matches!(
        error,
        truff_core::EditError::SizeLimitExceeded { .. }
    ));
    assert_eq!(buffer.as_str(), "a\r\nb");
}

#[test]
fn ambiguous_adjacent_color_spans_do_not_expand_removal_range() {
    let text = "<span style=\"color:#111111\">a</span><span style=\"color:#222222\">b</span>";
    let range = text.find(">a").unwrap() + 1..text.find("b</span>").unwrap() + 1;

    let tx = remove_style(text, range.clone()).unwrap();
    assert!(tx.edits.is_empty());
}

#[test]
fn malformed_short_color_tag_does_not_panic_during_removal() {
    let text = "<span style=\"color:</span>";
    assert!(remove_style(text, 0..text.len()).is_ok());
}
