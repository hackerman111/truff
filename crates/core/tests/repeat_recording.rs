use truff_core::text::{Buffer, BytePos};
use truff_core::vim::parser::InsertEntry;
use truff_core::vim::{Command, InputContext, InputResult, Key, Motion, Operator, VimState};
use truff_core::workspace::views::{View, ViewId};

fn setup(text: &str, cursor: usize) -> (Buffer, View) {
    let buffer = Buffer::from_str(text);
    let mut view = View::new(ViewId::new(1), buffer.id());
    view.set_cursor(BytePos::new(cursor), BytePos::new(cursor));
    (buffer, view)
}

fn run(vim: &mut VimState, buffer: &mut Buffer, view: &mut View, command: Command) -> InputResult {
    let view_id = view.id();
    vim.run_command(
        command,
        InputContext::Editor {
            view_id,
            buffer,
            view,
        },
    )
}

#[test]
fn insert_mode_delete_is_saved_for_dot_after_it_leaves_insert() {
    let (mut buffer, mut view) = setup("first\nsecond\n", 0);
    let mut vim = VimState::new();

    assert_eq!(
        run(
            &mut vim,
            &mut buffer,
            &mut view,
            Command::Insert(InsertEntry::AtCursor),
        ),
        InputResult::Consumed
    );
    assert_eq!(
        run(
            &mut vim,
            &mut buffer,
            &mut view,
            Command::OperatorMotion {
                op: Operator::Delete,
                motion: Motion::CurrentLine,
                count: 1,
            },
        ),
        InputResult::Consumed
    );
    assert_eq!(buffer.as_str(), "second\n");

    assert_eq!(
        run(&mut vim, &mut buffer, &mut view, Command::RepeatLastEdit,),
        InputResult::Consumed
    );
    assert_eq!(buffer.as_str(), "");
}

#[test]
fn committed_insert_survives_escape_and_repeats_with_dot() {
    let (mut buffer, mut view) = setup("a", 1);
    let mut vim = VimState::new();

    assert_eq!(
        run(
            &mut vim,
            &mut buffer,
            &mut view,
            Command::Insert(InsertEntry::AtCursor),
        ),
        InputResult::Consumed
    );
    assert_eq!(
        run(
            &mut vim,
            &mut buffer,
            &mut view,
            Command::TextCommit("x".into()),
        ),
        InputResult::Consumed
    );
    assert_eq!(
        run(
            &mut vim,
            &mut buffer,
            &mut view,
            Command::InsertKey(Key::Esc),
        ),
        InputResult::Consumed
    );
    assert_eq!(buffer.as_str(), "ax");

    assert_eq!(
        run(&mut vim, &mut buffer, &mut view, Command::RepeatLastEdit,),
        InputResult::Consumed
    );
    assert_eq!(buffer.as_str(), "axx");
}
