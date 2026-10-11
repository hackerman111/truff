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

pub(super) fn normalize_crlf(s: &str) -> String {
    let mut res = String::with_capacity(s.len() + 16);
    let mut prev_cr = false;
    for ch in s.chars() {
        if ch == '\n' {
            if !prev_cr {
                res.push('\r');
            }
            res.push('\n');
            prev_cr = false;
        } else {
            prev_cr = ch == '\r';
            res.push(ch);
        }
    }
    res
}
