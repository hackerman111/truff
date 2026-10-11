use truff_core::config::{KeymapConfig, LimitsConfig};
use truff_core::text::{Buffer, BytePos};
use truff_core::vim::{InputContext, InputResult, Key, Mode, VimState};
use truff_core::workspace::views::{View, ViewId};

fn setup_editor(text: &str) -> (Buffer, View) {
    let buffer = Buffer::from_str(text);
    let view = View::new(ViewId::new(1), buffer.id());
    (buffer, view)
}

fn feed_keys(vim: &mut VimState, buffer: &mut Buffer, view: &mut View, keys: &[Key]) {
    for &k in keys {
        let ctx = InputContext::Editor {
            view_id: view.id(),
            buffer,
            view,
        };
        let _ = vim.feed_key(k, ctx);
    }
}

#[test]
fn table_driven_edits() {
    struct TestCase {
        name: &'static str,
        initial_text: &'static str,
        initial_cursor: usize,
        keys: Vec<Key>,
        expected_text: &'static str,
        expected_cursor: usize,
    }

    let cases = vec![
        TestCase {
            name: "2dw deletes two words",
            initial_text: "one two three four",
            initial_cursor: 0,
            keys: vec![Key::Char('2'), Key::Char('d'), Key::Char('w')],
            expected_text: "three four",
            expected_cursor: 0,
        },
        TestCase {
            name: "d2w deletes two words",
            initial_text: "one two three four",
            initial_cursor: 0,
            keys: vec![Key::Char('d'), Key::Char('2'), Key::Char('w')],
            expected_text: "three four",
            expected_cursor: 0,
        },
        TestCase {
            name: "ciw changes inner word",
            initial_text: "hello world here",
            initial_cursor: 7, // in 'world'
            keys: vec![
                Key::Char('c'),
                Key::Char('i'),
                Key::Char('w'),
                Key::Char('r'),
                Key::Char('u'),
                Key::Char('s'),
                Key::Char('t'),
                Key::Esc,
            ],
            expected_text: "hello rust here",
            expected_cursor: 9, // cursor on last inserted char 't' after Esc
        },
        TestCase {
            name: "dd deletes current line",
            initial_text: "first line\nsecond line\nthird line\n",
            initial_cursor: 0,
            keys: vec![Key::Char('d'), Key::Char('d')],
            expected_text: "second line\nthird line\n",
            expected_cursor: 0,
        },
        TestCase {
            name: "yy and p pastes line below",
            initial_text: "first line\nsecond line\n",
            initial_cursor: 0,
            keys: vec![Key::Char('y'), Key::Char('y'), Key::Char('p')],
            expected_text: "first line\nfirst line\nsecond line\n",
            expected_cursor: 11, // start of pasted line
        },
        TestCase {
            name: "yy and P pastes line above",
            initial_text: "first line\nsecond line\n",
            initial_cursor: 11, // at 'second line'
            keys: vec![Key::Char('y'), Key::Char('y'), Key::Char('P')],
            expected_text: "first line\nsecond line\nsecond line\n",
            expected_cursor: 11,
        },
        TestCase {
            name: "undo reverts change",
            initial_text: "hello world",
            initial_cursor: 0,
            keys: vec![Key::Char('d'), Key::Char('w'), Key::Char('u')],
            expected_text: "hello world",
            expected_cursor: 0,
        },
        TestCase {
            name: "dot repeat duplicates last edit",
            initial_text: "one two three four",
            initial_cursor: 0,
            keys: vec![Key::Char('d'), Key::Char('w'), Key::Char('.')],
            expected_text: "three four",
            expected_cursor: 0,
        },
    ];

    for case in cases {
        let (mut buffer, mut view) = setup_editor(case.initial_text);
        view.set_cursor(
            BytePos::new(case.initial_cursor),
            BytePos::new(case.initial_cursor),
        );
        let mut vim = VimState::new();

        feed_keys(&mut vim, &mut buffer, &mut view, &case.keys);

        let actual_text = buffer.as_str();
        assert_eq!(
            actual_text, case.expected_text,
            "Failed test '{}': text mismatch",
            case.name
        );
        assert_eq!(
            view.head().get(),
            case.expected_cursor,
            "Failed test '{}': cursor mismatch",
            case.name
        );
    }
}

#[test]
fn emoji_delete() {
    // Emoji sequence with ZWJ: family: 👨‍👩‍👧‍👦
    // Man (4 bytes) + ZWJ (3) + Woman (4) + ZWJ (3) + Girl (4) + ZWJ (3) + Boy (4) = 25 bytes.
    let text = "👨‍👩‍👧‍👦 after";
    let (mut buffer, mut view) = setup_editor(text);
    view.set_cursor(BytePos::ZERO, BytePos::ZERO);
    let mut vim = VimState::new();

    // In normal mode, 'x' deletes one grapheme cluster
    feed_keys(&mut vim, &mut buffer, &mut view, &[Key::Char('x')]);

    assert_eq!(
        buffer.as_str(),
        " after",
        "x on composite emoji must delete exactly one whole grapheme"
    );
    assert_eq!(view.head().get(), 0);
}

#[test]
fn macro_bounded() {
    let (mut buffer, mut view) = setup_editor("line 1\nline 2\nline 3\n");
    let mut vim = VimState::new();

    // 1. Recursive macro rejection: record @a inside a
    // qa -> @a -> q
    feed_keys(
        &mut vim,
        &mut buffer,
        &mut view,
        &[
            Key::Char('q'),
            Key::Char('a'),
            Key::Char('@'),
            Key::Char('a'),
            Key::Char('q'),
        ],
    );

    // Replay @a: must return an error / reject recursive execution
    let ctx = InputContext::Editor {
        view_id: view.id(),
        buffer: &mut buffer,
        view: &mut view,
    };
    let res = vim.feed_key(Key::Char('@'), ctx);
    assert_eq!(res, InputResult::Incomplete);
    let ctx = InputContext::Editor {
        view_id: view.id(),
        buffer: &mut buffer,
        view: &mut view,
    };
    let res2 = vim.feed_key(Key::Char('a'), ctx);
    match res2 {
        InputResult::Error(msg) => {
            assert!(
                msg.to_lowercase().contains("recurs") || msg.to_lowercase().contains("loop"),
                "Expected recursion rejection, got: {msg}"
            );
        }
        other => panic!("Expected InputResult::Error for recursive macro, got: {other:?}"),
    }

    // 2. Macro exceeding max commands limit (e.g. 1000)
    let mut vim2 = VimState::new();
    let ctx = InputContext::Editor {
        view_id: view.id(),
        buffer: &mut buffer,
        view: &mut view,
    };
    let _ = vim2.feed_key(Key::Char('q'), ctx);
    let ctx = InputContext::Editor {
        view_id: view.id(),
        buffer: &mut buffer,
        view: &mut view,
    };
    let _ = vim2.feed_key(Key::Char('b'), ctx);

    // Feed 1005 keys into macro recording
    for _ in 0..1005 {
        let ctx = InputContext::Editor {
            view_id: view.id(),
            buffer: &mut buffer,
            view: &mut view,
        };
        let _ = vim2.feed_key(Key::Char('l'), ctx);
    }
    // Stop recording
    let ctx = InputContext::Editor {
        view_id: view.id(),
        buffer: &mut buffer,
        view: &mut view,
    };
    let _stop_res = vim2.feed_key(Key::Char('q'), ctx);
    // Either stops with error or records up to bounded limit
    assert!(
        vim2.macro_length('b') <= LimitsConfig::DEFAULT_MAX_MACRO_COMMANDS,
        "Macro command count must be bounded by DEFAULT_MAX_MACRO_COMMANDS"
    );
}

#[test]
fn keymap_hints_match() {
    let keymap = KeymapConfig::default();
    let hints = keymap.available_hints(Mode::Normal, &[Key::Char('g')]);

    assert!(!hints.is_empty(), "Prefix 'g' should provide hints");
    // Verify that every hint matches a valid command or continuation
    for hint in &hints {
        assert!(!hint.description.is_empty());
        assert!(keymap.has_binding_or_prefix(Mode::Normal, &[Key::Char('g'), hint.key]));
    }

    // Keymap validation passes on clean config
    assert!(keymap.validate().is_ok());
}

#[test]
fn colon_commands_protect_dirty_buffers() {
    let (mut buffer, mut view) = setup_editor("clean text");
    // Make buffer dirty
    buffer
        .apply(truff_core::text::Transaction::insert(0, "dirty "))
        .unwrap();
    assert!(buffer.is_dirty());

    let mut vim = VimState::new();
    // Feed :q<Enter>
    feed_keys(
        &mut vim,
        &mut buffer,
        &mut view,
        &[Key::Char(':'), Key::Char('q'), Key::Enter],
    );

    // Should NOT close or lose dirty buffer, must report error
    assert_eq!(
        vim.last_error(),
        Some("No write since last change (use ! to override)")
    );
}

#[test]
fn visual_mode_delete_lines_with_uppercase_x() {
    let (mut buffer, mut view) = setup_editor("line 1\nline 2\nline 3\n");
    let mut vim = VimState::new();

    // In VisualChar mode: select some characters on line 2, then press 'X'
    // Should delete the whole line 2 (linewise)
    feed_keys(
        &mut vim,
        &mut buffer,
        &mut view,
        &[
            Key::Char('j'), // move to line 2
            Key::Char('v'), // enter VisualChar
            Key::Char('w'), // select a word
            Key::Char('X'), // delete line(s)
        ],
    );
    assert_eq!(buffer.as_str(), "line 1\nline 3\n");
    assert_eq!(vim.mode, Mode::Normal);

    // In VisualLine mode: select line 1, press 'X'
    feed_keys(
        &mut vim,
        &mut buffer,
        &mut view,
        &[
            Key::Char('k'), // move to line 1
            Key::Char('V'), // enter VisualLine
            Key::Char('X'), // delete line
        ],
    );
    assert_eq!(buffer.as_str(), "line 3\n");
    assert_eq!(vim.mode, Mode::Normal);
}

#[test]
fn visual_mode_delete_selection_with_lowercase_x() {
    let (mut buffer, mut view) = setup_editor("hello world\n");
    let mut vim = VimState::new();

    // Select "hello " in VisualChar, then press 'x'
    feed_keys(
        &mut vim,
        &mut buffer,
        &mut view,
        &[Key::Char('v'), Key::Char('e'), Key::Char('x')],
    );
    assert_eq!(buffer.as_str(), " world\n");
    assert_eq!(vim.mode, Mode::Normal);
}

#[test]
fn test_visual_line_yank_multibyte_without_trailing_newline() {
    let text = "АБВГДЕЁЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЫЬЭЮЯабвгдеёжзийклмнопрстуфхцчшщъыьэюя";
    let (mut buffer, mut view) = setup_editor(text);
    let mut vim = VimState::new();

    feed_keys(
        &mut vim,
        &mut buffer,
        &mut view,
        &[
            Key::Char('V'), // enter VisualLine
            Key::Char('y'), // yank line
            Key::Char('p'), // paste below
        ],
    );
    assert_eq!(vim.mode, Mode::Normal);
    assert_eq!(buffer.as_str(), format!("{text}\n{text}"));
}

#[test]
fn test_visual_line_indent_multibyte_without_trailing_newline() {
    let text = "абвгдеёжзийклмнопрстуфхцчшщъыьэюя";
    let (mut buffer, mut view) = setup_editor(text);
    let mut vim = VimState::new();

    feed_keys(
        &mut vim,
        &mut buffer,
        &mut view,
        &[
            Key::Char('V'), // enter VisualLine
            Key::Char('>'), // indent line
        ],
    );
    assert_eq!(vim.mode, Mode::Normal);
    assert_eq!(buffer.as_str(), format!("    {text}"));
}
