use truff_core::Buffer;
use truff_core::markdown::style::{
    CalloutKind, StyleError, apply_highlight, apply_text_color, remove_style, wrap_callout,
};

#[test]
fn color_replaces_wrapper() {
    let initial_text = "The quick brown fox jumps over the lazy dog";
    let mut buffer = Buffer::from_str(initial_text);

    // Apply red color to "brown" (bytes 10..15)
    let brown_range = 10..15;
    assert_eq!(&initial_text[brown_range.clone()], "brown");
    let tx1 = apply_text_color(initial_text, brown_range, "#ff0000").expect("apply initial color");
    buffer.apply(tx1).expect("buffer apply tx1");

    let colored_once = buffer.as_str();
    assert_eq!(
        colored_once,
        "The quick <span style=\"color:#ff0000\">brown</span> fox jumps over the lazy dog"
    );

    // Now re-color "brown" to blue "#0000ff".
    // Find where the word "brown" or the whole span is located in colored_once:
    let span_start = colored_once.find("<span").unwrap();
    let span_end = colored_once.find("</span>").unwrap() + "</span>".len();
    let brown_word_start = colored_once.find("brown").unwrap();
    let brown_word_end = brown_word_start + "brown".len();

    // 1. Re-coloring with selection covering the inner text:
    let tx2 = apply_text_color(&colored_once, brown_word_start..brown_word_end, "#0000ff")
        .expect("re-color inner word");
    buffer.apply(tx2).expect("buffer apply tx2");

    let colored_twice = buffer.as_str();
    assert_eq!(
        colored_twice,
        "The quick <span style=\"color:#0000ff\">brown</span> fox jumps over the lazy dog"
    );
    // Crucial requirement: MUST NOT nest spans!
    assert_eq!(colored_twice.matches("<span").count(), 1);
    assert_eq!(colored_twice.matches("</span>").count(), 1);
    assert!(!colored_twice.contains("#ff0000"));

    // 2. Re-coloring with selection covering the full span tag:
    let tx3 = apply_text_color(&colored_twice, span_start..span_end, "#00ff00")
        .expect("re-color full span");
    buffer.apply(tx3).expect("buffer apply tx3");

    let colored_thrice = buffer.as_str();
    assert_eq!(
        colored_thrice,
        "The quick <span style=\"color:#00ff00\">brown</span> fox jumps over the lazy dog"
    );
    assert_eq!(colored_thrice.matches("<span").count(), 1);
    assert_eq!(colored_thrice.matches("</span>").count(), 1);
}

#[test]
fn color_undo_exact() {
    let initial_text = "Markdown document with several words.\nSecond line with info.";
    let mut buffer = Buffer::from_str(initial_text);

    // Apply color
    let tx = apply_text_color(initial_text, 9..17, "#123456").expect("apply color");
    buffer.apply(tx).expect("apply tx");
    assert_ne!(buffer.as_str(), initial_text);

    // Undo should restore byte-identical text
    buffer.undo().expect("undo");
    assert_eq!(buffer.as_str(), initial_text);

    // Apply highlight and undo
    let tx_hi = apply_highlight(initial_text, 0..8, "#ffaa00").expect("apply highlight");
    buffer.apply(tx_hi).expect("apply tx_hi");
    assert_ne!(buffer.as_str(), initial_text);
    buffer.undo().expect("undo highlight");
    assert_eq!(buffer.as_str(), initial_text);

    // Wrap callout and undo
    let tx_co =
        wrap_callout(initial_text, 0..initial_text.len(), CalloutKind::Note).expect("wrap callout");
    buffer.apply(tx_co).expect("apply tx_co");
    assert_ne!(buffer.as_str(), initial_text);
    buffer.undo().expect("undo callout");
    assert_eq!(buffer.as_str(), initial_text);
}

#[test]
fn color_fenced_code_protected() {
    let text =
        "Text before code\n```rust\nfn main() {\n    println!(\"hello\");\n}\n```\nText after code";
    let mut buffer = Buffer::from_str(text);

    // Attempting to apply a color over a range that crosses the code fence boundary:
    let before_to_inside = 5..35;
    let res1 = apply_text_color(text, before_to_inside, "#ff0000");
    assert!(
        matches!(res1, Err(StyleError::IntersectsCodeFence { .. })),
        "Expected IntersectsCodeFence error when range crosses fence, got {:?}",
        res1
    );

    // Attempting to cross the closing fence:
    let inside_to_after = 35..text.len() - 5;
    let res2 = apply_text_color(text, inside_to_after, "#00ff00");
    assert!(
        matches!(res2, Err(StyleError::IntersectsCodeFence { .. })),
        "Expected IntersectsCodeFence error when range crosses closing fence, got {:?}",
        res2
    );

    // Attempting to split a wiki-link delimiter [[...]]
    let wiki_text = "See [[my_note|My Note]] for details";
    let split_open_bracket = 3..6; // crosses '[['
    let res3 = apply_text_color(wiki_text, split_open_bracket, "#0000ff");
    assert!(
        matches!(res3, Err(StyleError::IntersectsLinkDelimiter { .. })),
        "Expected IntersectsLinkDelimiter when slicing [[, got {:?}",
        res3
    );

    // Applying color safely outside the code block succeeds and leaves code block intact:
    let safe_tx = apply_text_color(text, 0..4, "#112233").expect("color outside fence");
    buffer.apply(safe_tx).expect("apply safe_tx");
    assert!(
        buffer
            .as_str()
            .contains("```rust\nfn main() {\n    println!(\"hello\");\n}\n```")
    );
}

#[test]
fn remove_style_clean() {
    let text = "Hello <span style=\"color:#ff0000\">colored</span> and <mark style=\"background-color:#ffff00\">highlighted</mark> text";
    let mut buffer = Buffer::from_str(text);

    // Remove color from the colored span
    let colored_start = text.find("<span").unwrap();
    let colored_end = text.find("</span>").unwrap() + "</span>".len();
    let tx1 = remove_style(text, colored_start..colored_end).expect("remove color span");
    buffer.apply(tx1).expect("apply tx1");

    assert_eq!(
        buffer.as_str(),
        "Hello colored and <mark style=\"background-color:#ffff00\">highlighted</mark> text"
    );

    // Remove highlight from the highlight mark
    let current_text = buffer.as_str();
    let mark_start = current_text.find("<mark").unwrap();
    let mark_end = current_text.find("</mark>").unwrap() + "</mark>".len();
    let tx2 = remove_style(&current_text, mark_start..mark_end).expect("remove highlight mark");
    buffer.apply(tx2).expect("apply tx2");

    assert_eq!(buffer.as_str(), "Hello colored and highlighted text");
}

#[test]
fn color_trailing_newline_trimmed() {
    let initial_text = "hello\nworld";
    let mut buffer = Buffer::from_str(initial_text);

    // Range covers "hello\n" (0..6)
    let range = 0..6;
    assert_eq!(&initial_text[range.clone()], "hello\n");
    let tx = apply_text_color(initial_text, range, "#ff0000").expect("apply color with newline");
    buffer.apply(tx).expect("apply tx");

    // The closing </span> must stay on the line, newline must not be wrapped inside <span>
    assert_eq!(
        buffer.as_str(),
        "<span style=\"color:#ff0000\">hello</span>\nworld"
    );

    // CRLF case: "hello\r\nworld"
    let crlf_text = "hello\r\nworld";
    let mut crlf_buffer = Buffer::from_str(crlf_text);
    let crlf_range = 0..7;
    assert_eq!(&crlf_text[crlf_range.clone()], "hello\r\n");
    let tx_crlf =
        apply_text_color(crlf_text, crlf_range, "#00ff00").expect("apply color with crlf");
    crlf_buffer.apply(tx_crlf).expect("apply tx_crlf");

    assert_eq!(
        crlf_buffer.as_str(),
        "<span style=\"color:#00ff00\">hello</span>\r\nworld"
    );
}
