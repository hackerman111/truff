use truff_core::Buffer;
use truff_core::vim::{InputContext, InputResult, Key, VimState};
use truff_core::workspace::{View, ViewId};

fn run(vim: &mut VimState, buffer: &mut Buffer, view: &mut View, keys: &[Key]) -> InputResult {
    let mut result = InputResult::Consumed;
    for &key in keys {
        result = vim.feed_key(
            key,
            InputContext::Editor {
                view_id: view.id(),
                buffer,
                view,
            },
        );
    }
    result
}

fn chars(text: &str) -> Vec<Key> {
    text.chars().map(Key::Char).collect()
}

#[test]
fn linewise_paste_before_and_after_final_line_preserves_newline_shape() {
    for (text, keys, expected) in [
        ("a\nb", "GyyP", "a\nb\nb"),
        ("a\nb", "Gyyp", "a\nb\nb"),
        ("a\nb\n", "GyyP", "a\nb\nb\n"),
        ("a\nb\n", "Gyyp", "a\nb\nb\n"),
        ("a\r\nb", "GyyP", "a\r\nb\r\nb"),
        ("a\r\nb", "Gyyp", "a\r\nb\r\nb"),
        ("a\r\nb\r\n", "GyyP", "a\r\nb\r\nb\r\n"),
        ("a\r\nb\r\n", "Gyyp", "a\r\nb\r\nb\r\n"),
    ] {
        let mut buffer = Buffer::from_str(text);
        let mut view = View::new(ViewId::next(), buffer.id());
        let mut vim = VimState::new();
        run(&mut vim, &mut buffer, &mut view, &chars(keys));
        assert_eq!(buffer.as_str(), expected, "keys {keys:?} on {text:?}");
    }
}

#[test]
fn linewise_repeat_count_includes_interline_separators() {
    let mut buffer = Buffer::from_str("a\nb");
    let mut view = View::new(ViewId::next(), buffer.id());
    let mut vim = VimState::new();
    run(&mut vim, &mut buffer, &mut view, &chars("Gyy3p"));
    assert_eq!(buffer.as_str(), "a\nb\nb\nb\nb");
}

#[test]
fn linewise_size_limit_counts_prefix_suffix_and_separators() {
    for (limit, keys) in [(4, "GyyP"), (4, "Gyyp"), (7, "Gyy3p")] {
        let mut buffer = Buffer::from_str("a\nb");
        buffer.set_max_file_bytes(limit);
        let mut view = View::new(ViewId::next(), buffer.id());
        let mut vim = VimState::new();
        let result = run(&mut vim, &mut buffer, &mut view, &chars(keys));
        assert_eq!(
            result,
            InputResult::Error("Paste exceeds buffer size limit".into())
        );
        assert_eq!(buffer.as_str(), "a\nb");
    }
}

#[test]
fn count_one_matches_unprefixed_paste() {
    for keys in ["Gyyp", "Gyy1p"] {
        let mut buffer = Buffer::from_str("a\nb");
        let mut view = View::new(ViewId::next(), buffer.id());
        let mut vim = VimState::new();
        run(&mut vim, &mut buffer, &mut view, &chars(keys));
        assert_eq!(buffer.as_str(), "a\nb\nb");
    }
}
