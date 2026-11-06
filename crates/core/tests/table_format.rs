use truff_core::markdown::table::{
    compute_column_pixel_widths, find_table_range_at, format_table_str, format_table_transaction,
};
use truff_core::text::Buffer;

#[test]
fn test_format_table_unaligned_cells() {
    let unaligned = "\
| Header 1 | H2 |
|---|---|
| Value 1 | Short |
| Very long value indeed | V2 |
";

    let expected = "\
| Header 1               | H2    |
|------------------------|-------|
| Value 1                | Short |
| Very long value indeed | V2    |
";

    let formatted = format_table_str(unaligned).expect("should format table");
    assert_eq!(formatted, expected);
}

#[test]
fn test_format_table_preserves_alignments() {
    let raw = "\
| Left | Center | Right |
|:---|:---:|---:|
| 1 | 2 | 3 |
";

    let expected = "\
| Left | Center | Right |
|:-----|:------:|------:|
| 1    |   2    |     3 |
";

    let formatted = format_table_str(raw).expect("should format alignments");
    assert_eq!(formatted, expected);
}

#[test]
fn test_format_table_with_escaped_pipes() {
    let raw = "\
| Col 1 | Col 2 |
|---|---|
| Text with \\| pipe | Next |
";

    let formatted = format_table_str(raw).expect("should handle escaped pipe");
    assert!(formatted.contains("Text with \\| pipe"));
    assert_eq!(formatted.lines().count(), 3);
}

#[test]
fn test_format_table_russian_unicode() {
    let raw = "\
| Сценарий | Кандидат |
|---|---|
| Мало аудиочасов | CTC |
| Много данных | Whisper |
";

    let expected = "\
| Сценарий        | Кандидат |
|-----------------|----------|
| Мало аудиочасов | CTC      |
| Много данных    | Whisper  |
";

    let formatted = format_table_str(raw).expect("should format unicode table");
    assert_eq!(formatted, expected);
}

#[test]
fn test_compute_column_pixel_widths_proportional_wrap() {
    let chars = [22, 86, 74];
    let available_w = 750.0;
    let char_w = 8.5;

    let widths = compute_column_pixel_widths(&chars, available_w, char_w);
    assert_eq!(widths.len(), 3);
    let total: f32 = widths.iter().sum();
    assert!(
        (total - available_w).abs() < 1.0,
        "Total must equal available width: total={}, available={}",
        total,
        available_w
    );
    assert!(widths[0] < widths[1]);
    assert!(widths[0] < widths[2]);
    assert!(widths[1] > widths[2]);
    for &w in &widths {
        assert!(w >= 50.0);
    }
}

#[test]
fn test_find_table_range_and_transaction() {
    let doc = "\
# Title

Some text before.

| A | B |
|---|---|
| 1 | Very long value |

Some text after.
";

    let mut buffer = Buffer::from_str(doc);
    let table_pos = doc.find("| A").unwrap();

    let range = find_table_range_at(&buffer, table_pos).expect("table range found");
    let table_str = buffer.slice(range.clone()).to_string();
    assert!(table_str.starts_with("| A"));
    assert!(table_str.trim_end().ends_with("| Very long value |"));

    let tx = format_table_transaction(&buffer, table_pos).expect("transaction created");
    buffer.apply(tx).unwrap();

    let result = buffer.as_str();
    assert!(result.contains("| A | B               |"));
    assert!(result.contains("|---|-----------------|"));
    assert!(result.contains("| 1 | Very long value |"));
}
