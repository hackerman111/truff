use std::collections::HashMap;
use std::mem::size_of;
use std::sync::Arc;

/// Kind of text selection stored in a register (Vim-style).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RegisterKind {
    /// Characterwise text.
    #[default]
    Characterwise,
    /// Linewise text (includes terminating newline).
    Linewise,
    /// Blockwise rectangular text.
    Blockwise,
}

/// A stored register entry containing text and selection shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Register {
    /// Text content.
    pub text: String,
    /// Selection shape.
    pub kind: RegisterKind,
}

impl Register {
    /// Creates a new register entry.
    pub fn new(text: impl Into<String>, kind: RegisterKind) -> Self {
        Self {
            text: text.into(),
            kind,
        }
    }

    /// Returns estimated memory consumption in bytes.
    pub fn size_in_bytes(&self) -> usize {
        size_of::<Self>() + self.text.capacity()
    }
}

/// Collection of named, unnamed, and clipboard registers.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Registers {
    unnamed: Option<Arc<Register>>,
    named: HashMap<char, Arc<Register>>,
}

impl Registers {
    /// Creates an empty register collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Retrieves a register by name (`None` or `Some('"')` accesses the unnamed register).
    pub fn get(&self, name: Option<char>) -> Option<&Register> {
        self.entry(name).map(Arc::as_ref)
    }

    fn entry(&self, name: Option<char>) -> Option<&Arc<Register>> {
        match name {
            None | Some('"') => self.unnamed.as_ref(),
            Some(c) => {
                let key = c.to_ascii_lowercase();
                self.named.get(&key)
            }
        }
    }

    pub(crate) fn get_shared(&self, name: Option<char>) -> Option<Arc<Register>> {
        self.entry(name).cloned()
    }

    /// Sets the value of a register.
    ///
    /// If `name` is uppercase (`'A'..='Z'`), appends to the corresponding lowercase register.
    /// In all cases, also updates the unnamed register.
    pub fn set(&mut self, name: Option<char>, value: Register) {
        let value = Arc::new(value);
        match name {
            None | Some('"') => {
                self.unnamed = Some(value);
            }
            Some(c) if c.is_ascii_uppercase() => {
                let key = c.to_ascii_lowercase();
                if let Some(existing) = self.named.get_mut(&key) {
                    let register = Arc::make_mut(existing);
                    register.text.push_str(&value.text);
                    register.kind = value.kind;
                    self.unnamed = Some(existing.clone());
                } else {
                    self.named.insert(key, value.clone());
                    self.unnamed = Some(value);
                }
            }
            Some(c) => {
                let key = c.to_ascii_lowercase();
                self.named.insert(key, value.clone());
                self.unnamed = Some(value);
            }
        }
    }

    /// Clears all stored registers.
    pub fn clear(&mut self) {
        self.unnamed = None;
        self.named.clear();
    }
}
