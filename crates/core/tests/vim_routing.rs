use truff_core::vim::{InputContext, InputResult, Key, Mode, VimState};
use truff_core::workspace::{View, ViewId};
use truff_core::{Buffer, BytePos};
fn run(vim: &mut VimState, b: &mut Buffer, v: &mut View, keys: &[Key]) -> InputResult {
    let mut result = InputResult::Consumed;
    for &key in keys {
        result = vim.feed_key(
            key,
            InputContext::Editor {
                view_id: v.id(),
                buffer: b,
                view: v,
            },
        );
    }
    result
}
fn chars(s: &str) -> Vec<Key> {
    s.chars().map(Key::Char).collect()
}
#[test]
fn visual_operators_are_inclusive_and_undoable() {
    for (text, keys, expected) in [("👨‍👩‍👧‍👦ab", "vld", "b"), ("a\r\nb\r\nc", "Vjd", "c")]
    {
        let mut b = Buffer::from_str(text);
        let mut v = View::new(ViewId::next(), b.id());
        let mut vim = VimState::new();
        run(&mut vim, &mut b, &mut v, &chars(keys));
        assert_eq!(b.as_str(), expected);
        assert_eq!(vim.mode, Mode::Normal);
        run(&mut vim, &mut b, &mut v, &chars("u"));
        assert_eq!(b.as_str(), text);
    }
}
#[test]
fn insert_and_change_are_single_undo_groups() {
    let mut b = Buffer::from_str("one two");
    let mut v = View::new(ViewId::next(), b.id());
    let mut vim = VimState::new();
    run(&mut vim, &mut b, &mut v, &chars("ciwabc"));
    run(
        &mut vim,
        &mut b,
        &mut v,
        &[Key::Backspace, Key::Tab, Key::Enter, Key::Esc],
    );
    run(&mut vim, &mut b, &mut v, &chars("u"));
    assert_eq!(b.as_str(), "one two");
}
#[test]
fn append_at_eof_and_open_crlf() {
    for (text, keys, expected) in [
        ("x", "az", "xz"),
        ("x\r\ny", "Oz", "z\r\nx\r\ny"),
        ("x", "oz", "x\nz"),
    ] {
        let mut b = Buffer::from_str(text);
        let mut v = View::new(ViewId::next(), b.id());
        let mut vim = VimState::new();
        run(&mut vim, &mut b, &mut v, &chars(keys));
        run(&mut vim, &mut b, &mut v, &[Key::Esc]);
        assert_eq!(b.as_str(), expected);
        run(&mut vim, &mut b, &mut v, &chars("u"));
        assert_eq!(b.as_str(), text);
    }
}
#[test]
fn dot_keeps_count_and_find_target() {
    let mut b = Buffer::from_str("a b c d e f");
    let mut v = View::new(ViewId::next(), b.id());
    let mut vim = VimState::new();
    run(&mut vim, &mut b, &mut v, &chars("2dw."));
    assert_eq!(b.as_str(), "e f");
    let mut b = Buffer::from_str("abXcdXef");
    let mut v = View::new(ViewId::next(), b.id());
    let mut vim = VimState::new();
    run(&mut vim, &mut b, &mut v, &chars("dfX."));
    assert_eq!(b.as_str(), "ef");
}
#[test]
fn literal_unicode_search_wraps_and_remembers_direction() {
    let mut b = Buffer::from_str("α a.b α aXb α");
    let mut v = View::new(ViewId::next(), b.id());
    let mut vim = VimState::new();
    run(&mut vim, &mut b, &mut v, &chars("/a.b"));
    run(&mut vim, &mut b, &mut v, &[Key::Enter]);
    assert_eq!(v.head().get(), 3);
    run(&mut vim, &mut b, &mut v, &chars("?α"));
    run(&mut vim, &mut b, &mut v, &[Key::Enter]);
    assert_eq!(v.head(), BytePos::ZERO);
    run(&mut vim, &mut b, &mut v, &chars("n"));
    assert_eq!(v.head().get(), 14);
}

#[test]
fn canonical_commit_is_one_event_and_repeats_in_dot_and_macros() {
    let mut b = Buffer::from_str("one two");
    let mut v = View::new(ViewId::next(), b.id());
    let mut vim = VimState::new();
    run(&mut vim, &mut b, &mut v, &chars("qaciw"));
    let revision = b.revision();
    let result = vim.commit_text(
        "日本👨‍👩‍👧‍👦",
        InputContext::Editor {
            view_id: v.id(),
            buffer: &mut b,
            view: &mut v,
        },
    );
    assert_eq!(result, InputResult::Consumed);
    assert_eq!(b.revision(), revision + 1);
    run(&mut vim, &mut b, &mut v, &[Key::Esc]);
    run(&mut vim, &mut b, &mut v, &chars("q"));
    assert_eq!(vim.macro_length('a'), 3);
    run(&mut vim, &mut b, &mut v, &chars("u"));
    assert_eq!(b.as_str(), "one two");
    for _ in 0..20 {
        run(&mut vim, &mut b, &mut v, &chars("@a"));
        assert_eq!(b.as_str(), "日本👨‍👩‍👧‍👦 two");
        run(&mut vim, &mut b, &mut v, &chars("u"));
        assert_eq!(b.as_str(), "one two");
    }
    run(&mut vim, &mut b, &mut v, &chars("."));
    assert_eq!(b.as_str(), "日本👨‍👩‍👧‍👦 two");
}
#[test]
fn macro_limits_count_commands_and_reset_each_run() {
    let mut b = Buffer::from_str("abc");
    let mut v = View::new(ViewId::next(), b.id());
    let mut vim = VimState::new();
    run(&mut vim, &mut b, &mut v, &chars("qalq"));
    for _ in 0..3 {
        assert_eq!(
            run(&mut vim, &mut b, &mut v, &chars("10000@a")),
            InputResult::Consumed
        );
    }
    assert!(matches!(
        run(&mut vim, &mut b, &mut v, &chars("10001@a")),
        InputResult::Error(_)
    ));
    run(&mut vim, &mut b, &mut v, &chars("qb"));
    for _ in 0..1000 {
        assert_eq!(
            run(&mut vim, &mut b, &mut v, &chars("l")),
            InputResult::Consumed
        );
    }
    assert!(matches!(
        run(&mut vim, &mut b, &mut v, &chars("l")),
        InputResult::Error(_)
    ));
    assert_eq!(vim.macro_length('b'), 0);
}
#[test]
fn count_overflow_is_explicit_and_parser_recovers() {
    let mut b = Buffer::from_str("abc");
    let mut v = View::new(ViewId::next(), b.id());
    let mut vim = VimState::new();
    let mut keys = chars(&usize::MAX.to_string());
    keys.push(Key::Char('9'));
    assert!(matches!(
        run(&mut vim, &mut b, &mut v, &keys),
        InputResult::Error(_)
    ));
    run(&mut vim, &mut b, &mut v, &chars("x"));
    assert_eq!(b.as_str(), "bc");
}
#[test]
fn indent_and_dedent_cover_all_selected_lines() {
    let mut b = Buffer::from_str("a\r\nb\r\nc");
    let mut v = View::new(ViewId::next(), b.id());
    let mut vim = VimState::new();
    run(&mut vim, &mut b, &mut v, &chars("Vj>"));
    assert_eq!(b.as_str(), "    a\r\n    b\r\nc");
    run(&mut vim, &mut b, &mut v, &chars("u"));
    assert_eq!(b.as_str(), "a\r\nb\r\nc");
    run(&mut vim, &mut b, &mut v, &chars("2>>"));
    run(&mut vim, &mut b, &mut v, &chars("2<<"));
    assert_eq!(b.as_str(), "a\r\nb\r\nc");
}
#[test]
fn nested_brackets_and_syntax_objects_use_source_ranges() {
    use truff_core::vim::motion::{SyntaxObjectRange, TextObjectTarget};
    let mut b = Buffer::from_str("(a (日本) z)");
    let mut v = View::new(ViewId::next(), b.id());
    let mut vim = VimState::new();
    v.set_cursor(BytePos::new(4), BytePos::new(4));
    run(&mut vim, &mut b, &mut v, &chars("di("));
    assert_eq!(b.as_str(), "(a () z)");
    let mut b = Buffer::from_str("# title\nbody\n# next");
    let mut v = View::new(ViewId::next(), b.id());
    let mut vim = VimState::new();
    vim.set_syntax_objects(
        b.revision(),
        vec![SyntaxObjectRange {
            target: TextObjectTarget::MarkdownHeading,
            inner: 2..13,
            around: 0..13,
        }],
    );
    run(&mut vim, &mut b, &mut v, &chars("dah"));
    assert_eq!(b.as_str(), "# next");
}
#[test]
fn visual_row_hook_preserves_visual_anchor() {
    let mut b = Buffer::from_str("abcdefgh");
    let mut v = View::new(ViewId::next(), b.id());
    let mut vim = VimState::new();
    run(&mut vim, &mut b, &mut v, &chars("v"));
    assert_eq!(
        run(&mut vim, &mut b, &mut v, &chars("2gj")),
        InputResult::VisualRow {
            down: true,
            count: 2
        }
    );
    vim.apply_visual_row(&mut v, BytePos::new(6));
    assert_eq!(v.anchor(), BytePos::ZERO);
    assert_eq!(v.head(), BytePos::new(6));
}

#[test]
fn search_wrap_keeps_matches_overlapping_the_cursor() {
    use truff_core::vim::motion::literal_search;
    let buffer = Buffer::from_str("abc abc");
    assert_eq!(literal_search(&buffer, 1, "abc", false, 1), Some(0));
    assert_eq!(literal_search(&buffer, 4, "abc", true, 1), Some(0));
    assert_eq!(literal_search(&buffer, 4, "abc", false, 2), Some(4));
}
#[test]
fn failed_find_never_deletes_and_vertical_operators_are_linewise() {
    let mut buffer = Buffer::from_str("ab\r\ncd\r\nef");
    let mut view = View::new(ViewId::next(), buffer.id());
    let mut vim = VimState::new();
    run(&mut vim, &mut buffer, &mut view, &chars("dfX"));
    assert_eq!(buffer.as_str(), "ab\r\ncd\r\nef");
    run(&mut vim, &mut buffer, &mut view, &chars("dj"));
    assert_eq!(buffer.as_str(), "ef");
    run(&mut vim, &mut buffer, &mut view, &chars("u"));
    assert_eq!(buffer.as_str(), "ab\r\ncd\r\nef");
}
#[test]
fn linewise_paste_separates_unterminated_lines() {
    for (text, keys, expected) in [
        ("a\r\nb", "Gyyp", "a\r\nb\r\nb"),
        ("a\nb", "GyyggP", "b\na\nb"),
    ] {
        let mut buffer = Buffer::from_str(text);
        let mut view = View::new(ViewId::next(), buffer.id());
        let mut vim = VimState::new();
        run(&mut vim, &mut buffer, &mut view, &chars(keys));
        assert_eq!(buffer.as_str(), expected);
        assert!(buffer.is_char_boundary(view.head().get()));
    }
}
#[test]
fn compiled_keymap_validates_registry_and_contextual_hints() {
    use truff_core::commands::{CommandContext, CommandId, CommandRegistry};
    use truff_core::config::{BindingLookup, KeyBinding, KeymapConfig, KeymapError};
    let mut config = KeymapConfig::default();
    let registry = CommandRegistry::default();
    let map = config.compile().unwrap();
    for context in [CommandContext::Editor, CommandContext::Rows] {
        for hint in map.hints(Mode::Normal, &[], context) {
            match map.lookup(Mode::Normal, &[hint.key], context) {
                BindingLookup::Complete(id) => {
                    registry.require(&id, context).unwrap();
                }
                BindingLookup::Prefix => {
                    assert!(!map.hints(Mode::Normal, &[hint.key], context).is_empty())
                }
                BindingLookup::Missing => panic!("hint must resolve"),
            }
        }
    }
    let source = toml::to_string(&config).unwrap();
    assert_eq!(KeymapConfig::from_toml(&source).unwrap(), config);
    config.bindings.push(KeyBinding {
        mode: Mode::Normal,
        keys: vec![Key::Char('z')],
        command: CommandId::new("does.not.exist"),
        description: "invalid".into(),
    });
    assert!(matches!(config.compile(), Err(KeymapError::Registry(_))));
    config.bindings.pop();
    config.bindings.push(config.bindings[0].clone());
    assert!(matches!(
        config.compile(),
        Err(KeymapError::DuplicateBinding { .. })
    ));
}

#[test]
fn test_command_registry_human_readable_and_search() {
    use truff_core::commands::{CommandId, CommandRegistry};
    let registry = CommandRegistry::default();

    // Verify human-readable titles (not raw internal IDs)
    let save_cmd = registry
        .get(&CommandId::from_static("buffer.save"))
        .unwrap();
    assert_eq!(save_cmd.title, "Сохранить заметку");
    assert_eq!(save_cmd.default_key.as_deref(), Some(":w"));

    let files_cmd = registry
        .get(&CommandId::from_static("panel.files"))
        .unwrap();
    assert_eq!(files_cmd.title, "Файлы и папки");
    assert_eq!(files_cmd.default_key.as_deref(), Some("Space f"));

    // Verify search
    let results = registry.search("сохранить");
    assert!(!results.is_empty());
    assert!(results.iter().any(|s| s.id.as_str() == "buffer.save"));

    let panel_results = registry.search("папки");
    assert!(!panel_results.is_empty());
    assert!(panel_results.iter().any(|s| s.id.as_str() == "panel.files"));

    // Verify categories
    let categories = registry.by_category();
    assert!(categories.contains_key("Панели"));
    assert!(categories.contains_key("Файлы"));
    assert!(categories.contains_key("Редактирование"));
}
