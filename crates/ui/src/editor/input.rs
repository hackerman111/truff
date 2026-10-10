//! Input events and operations for the editor probe.

/// Actions that can be performed on the editor buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorInput {
    /// Active IME preedit text. Should NOT modify the persistent rope buffer.
    ImePreedit {
        text: String,
        cursor: Option<(usize, usize)>,
    },
    /// IME text commit. Must be inserted into the buffer exactly once.
    ImeCommit(String),
    /// Plain text insertion (e.g. keyboard typing).
    InsertText(String),
    /// Backspace (delete previous grapheme or selection).
    DeleteBackward,
    /// Delete (delete next grapheme or selection).
    DeleteForward,
    /// Move cursor left by one grapheme cluster.
    MoveCursorLeft { extend_selection: bool },
    /// Move cursor right by one grapheme cluster.
    MoveCursorRight { extend_selection: bool },
    /// Set explicit selection range (anchor, head).
    SetSelection { anchor: usize, head: usize },
    /// Select the entire text.
    SelectAll,
    /// Paste text from clipboard.
    Paste(String),
}
