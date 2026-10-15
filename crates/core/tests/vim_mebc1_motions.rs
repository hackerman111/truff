use truff_core::text::{Buffer, BytePos};
use truff_core::vim::motion::{Motion, eval_motion};
use truff_core::vim::parser::{Command, ParseResult, ScrollPosition, VimParser};
use truff_core::vim::{Key, Mode};

#[test]
fn test_mebc1_parser_zz_zt_zb() {
    let mut parser = VimParser::new();
    assert_eq!(
        parser.feed(Key::Char('z'), Mode::Normal),
        ParseResult::Incomplete
    );
    assert_eq!(
        parser.feed(Key::Char('z'), Mode::Normal),
        ParseResult::Complete(Command::ScrollCursor(ScrollPosition::Center))
    );

    let mut parser = VimParser::new();
    assert_eq!(
        parser.feed(Key::Char('z'), Mode::Normal),
        ParseResult::Incomplete
    );
    assert_eq!(
        parser.feed(Key::Char('t'), Mode::Normal),
        ParseResult::Complete(Command::ScrollCursor(ScrollPosition::Top))
    );

    let mut parser = VimParser::new();
    assert_eq!(
        parser.feed(Key::Char('z'), Mode::Normal),
        ParseResult::Incomplete
    );
    assert_eq!(
        parser.feed(Key::Char('b'), Mode::Normal),
        ParseResult::Complete(Command::ScrollCursor(ScrollPosition::Bottom))
    );
}

#[test]
fn test_mebc1_parser_star_and_hash() {
    let mut parser = VimParser::new();
    assert_eq!(
        parser.feed(Key::Char('*'), Mode::Normal),
        ParseResult::Complete(Command::SearchWordUnderCursor { forward: true })
    );

    let mut parser = VimParser::new();
    assert_eq!(
        parser.feed(Key::Char('#'), Mode::Normal),
        ParseResult::Complete(Command::SearchWordUnderCursor { forward: false })
    );
}

#[test]
fn test_mebc1_parser_percent_matching_bracket() {
    let mut parser = VimParser::new();
    assert_eq!(
        parser.feed(Key::Char('%'), Mode::Normal),
        ParseResult::Complete(Command::Motion {
            motion: Motion::MatchingBracket,
            count: 1,
        })
    );
}

#[test]
fn test_mebc1_parser_gf_follow_link() {
    let mut parser = VimParser::new();
    assert_eq!(
        parser.feed(Key::Char('g'), Mode::Normal),
        ParseResult::Incomplete
    );
    assert_eq!(
        parser.feed(Key::Char('f'), Mode::Normal),
        ParseResult::Complete(Command::FollowLink)
    );
}

#[test]
fn test_mebc1_eval_matching_bracket() {
    let text = "function(a, [b, {c}])";
    let buffer = Buffer::from_str(text);

    // From '(' at index 8 -> matching ')' at index 20
    let pos = BytePos::new(8);
    let target = eval_motion(&buffer, pos, &Motion::MatchingBracket, 1, None).0;
    assert_eq!(target, BytePos::new(20));

    // From ')' at index 20 -> matching '(' at index 8
    let target_back = eval_motion(&buffer, BytePos::new(20), &Motion::MatchingBracket, 1, None).0;
    assert_eq!(target_back, BytePos::new(8));

    // From '[' at index 12 -> matching ']' at index 19
    let target_bracket =
        eval_motion(&buffer, BytePos::new(12), &Motion::MatchingBracket, 1, None).0;
    assert_eq!(target_bracket, BytePos::new(19));

    // From '{' at index 16 -> matching '}' at index 18
    let target_brace = eval_motion(&buffer, BytePos::new(16), &Motion::MatchingBracket, 1, None).0;
    assert_eq!(target_brace, BytePos::new(18));
}
