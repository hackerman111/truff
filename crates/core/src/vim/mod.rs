use std::fmt;

use serde::{Deserialize, Serialize};

pub mod execute;
pub mod motion;
pub mod parser;

pub use execute::{InputContext, InputResult, KeyDisposition, VimState};
pub use motion::{Motion, TextObject, TextObjectKind, TextObjectTarget};
pub use parser::{Command, Operator, ScrollPosition};

/// Mode in which the editor or panel is currently operating.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
    VisualChar,
    VisualLine,
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Normal => write!(f, "NORMAL"),
            Self::Insert => write!(f, "INSERT"),
            Self::VisualChar => write!(f, "VISUAL"),
            Self::VisualLine => write!(f, "V-LINE"),
        }
    }
}

/// A keyboard input key representation independent of UI frameworks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Key {
    Char(char),
    Ctrl(char),
    Alt(char),
    Esc,
    Enter,
    ShiftEnter,
    CtrlEnter,
    Backspace,
    Tab,
    ShiftTab,
    Delete,
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Home,
    End,
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Char(c) => write!(f, "{c}"),
            Self::Ctrl(c) => write!(f, "<C-{c}>"),
            Self::Alt(c) => write!(f, "<A-{c}>"),
            Self::Esc => write!(f, "<Esc>"),
            Self::Enter => write!(f, "<CR>"),
            Self::ShiftEnter => write!(f, "<S-CR>"),
            Self::CtrlEnter => write!(f, "<C-CR>"),
            Self::Backspace => write!(f, "<BS>"),
            Self::Tab => write!(f, "<Tab>"),
            Self::ShiftTab => write!(f, "<S-Tab>"),
            Self::Delete => write!(f, "<Del>"),
            Self::Up => write!(f, "<Up>"),
            Self::Down => write!(f, "<Down>"),
            Self::Left => write!(f, "<Left>"),
            Self::Right => write!(f, "<Right>"),
            Self::PageUp => write!(f, "<PageUp>"),
            Self::PageDown => write!(f, "<PageDown>"),
            Self::Home => write!(f, "<Home>"),
            Self::End => write!(f, "<End>"),
        }
    }
}
