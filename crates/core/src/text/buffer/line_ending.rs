/// Line ending convention for a buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineEnding {
    /// Unix line ending (`\n`).
    #[default]
    Lf,
    /// Windows line ending (`\r\n`).
    CrLf,
}

impl LineEnding {
    /// Returns the string representation of this line ending.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
        }
    }

    /// Detects line ending from initial text.
    pub fn detect(text: &str) -> Self {
        if text.contains("\r\n") {
            Self::CrLf
        } else {
            Self::Lf
        }
    }
}

pub(super) fn normalize_crlf(s: &str) -> std::borrow::Cow<'_, str> {
    use std::borrow::Cow;

    let bytes = s.as_bytes();
    let bare_lfs = bytes
        .iter()
        .enumerate()
        .filter(|(index, byte)| **byte == b'\n' && (*index == 0 || bytes[*index - 1] != b'\r'))
        .count();
    if bare_lfs == 0 {
        return Cow::Borrowed(s);
    }

    let mut normalized = String::with_capacity(s.len() + bare_lfs);
    let mut copied_until = 0;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' && (index == 0 || bytes[index - 1] != b'\r') {
            normalized.push_str(&s[copied_until..index]);
            normalized.push('\r');
            copied_until = index;
        }
    }
    normalized.push_str(&s[copied_until..]);
    Cow::Owned(normalized)
}

#[cfg(test)]
mod tests {
    use super::{LineEnding, normalize_crlf};
    use std::borrow::Cow;

    #[test]
    fn normalization_borrows_when_no_bare_lf_exists() {
        for text in ["plain", "a\r\nb\r\n"] {
            assert!(matches!(normalize_crlf(text), Cow::Borrowed(_)));
        }
    }

    #[test]
    fn normalization_inserts_cr_only_before_bare_lf() {
        for (input, expected) in [
            ("\na\nb", "\r\na\r\nb"),
            ("a\r\nb\nc", "a\r\nb\r\nc"),
            ("Привет\n🌍", "Привет\r\n🌍"),
        ] {
            assert_eq!(normalize_crlf(input).as_ref(), expected);
        }
    }

    #[test]
    fn detection_keeps_crlf_for_mixed_newlines() {
        assert_eq!(LineEnding::detect("a\r\nb\n"), LineEnding::CrLf);
    }
}
