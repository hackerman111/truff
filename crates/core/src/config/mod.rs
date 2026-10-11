use std::fmt;

use serde::{Deserialize, Serialize};

use crate::commands::{CommandContext, CommandId, CommandRegistry, RegistryError};
use crate::vim::{Key, Mode};

mod compiled;
mod defaults;

pub use compiled::{BindingLookup, CompiledKeymap};

/// Limits and safety thresholds for editing, macros, and history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitsConfig {
    pub max_macro_commands: usize,
    pub max_macro_steps: usize,
    pub max_history_bytes: usize,
}

impl LimitsConfig {
    /// Default maximum commands allowed in a single macro definition.
    pub const DEFAULT_MAX_MACRO_COMMANDS: usize = 1000;
    /// Default maximum execution steps allowed across macro playback.
    pub const DEFAULT_MAX_MACRO_STEPS: usize = 10000;
    /// Default maximum history memory: 8 MiB.
    pub const DEFAULT_MAX_HISTORY_BYTES: usize = 8 * 1024 * 1024;
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_macro_commands: Self::DEFAULT_MAX_MACRO_COMMANDS,
            max_macro_steps: Self::DEFAULT_MAX_MACRO_STEPS,
            max_history_bytes: Self::DEFAULT_MAX_HISTORY_BYTES,
        }
    }
}

/// A contextual hint for WhichKey display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyHint {
    pub key: Key,
    pub description: String,
    pub is_prefix: bool,
}

/// Errors detected during keymap compilation or validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeymapError {
    ShadowedPrefix {
        prefix: Vec<Key>,
        conflicting_key: Key,
    },
    InvalidBinding(String),
    DuplicateBinding {
        mode: Mode,
        keys: Vec<Key>,
    },
    Registry(RegistryError),
    InvalidToml(String),
}

impl fmt::Display for KeymapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ShadowedPrefix {
                prefix,
                conflicting_key,
            } => {
                write!(
                    f,
                    "keymap prefix {:?} conflicts with complete binding {:?}",
                    prefix, conflicting_key
                )
            }
            Self::InvalidBinding(msg) => write!(f, "invalid binding: {msg}"),
            Self::DuplicateBinding { mode, keys } => {
                write!(f, "duplicate binding in {mode}: {keys:?}")
            }
            Self::Registry(e) => e.fmt(f),
            Self::InvalidToml(e) => write!(f, "invalid keymap TOML: {e}"),
        }
    }
}

impl std::error::Error for KeymapError {}

/// A single keymap rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyBinding {
    pub mode: Mode,
    pub keys: Vec<Key>,
    pub command: CommandId,
    pub description: String,
}

/// Configuration storing key bindings and providing prefix lookups for which-key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeymapConfig {
    pub bindings: Vec<KeyBinding>,
    pub limits: LimitsConfig,
}

impl KeymapConfig {
    /// Checks whether a key sequence is a complete binding or an exact prefix of an existing binding.
    pub fn has_binding_or_prefix(&self, mode: Mode, keys: &[Key]) -> bool {
        self.compile().is_ok_and(|map| {
            map.lookup(mode, keys, CommandContext::Editor) != BindingLookup::Missing
        })
    }

    /// Returns available key hints for which-key given an entered prefix.
    pub fn available_hints(&self, mode: Mode, prefix: &[Key]) -> Vec<KeyHint> {
        self.compile()
            .map(|map| map.hints(mode, prefix, CommandContext::Editor))
            .unwrap_or_default()
    }

    /// Validates the keymap to ensure no ambiguous collisions.
    pub fn validate(&self) -> Result<(), KeymapError> {
        if self.bindings.len() > 4096 {
            return Err(KeymapError::InvalidBinding(
                "more than 4096 bindings".into(),
            ));
        }
        let registry = CommandRegistry::default();
        for (i, a) in self.bindings.iter().enumerate() {
            if a.keys.len() > 32 {
                return Err(KeymapError::InvalidBinding(
                    "key sequence exceeds 32 keys".into(),
                ));
            }
            if a.keys.is_empty() {
                return Err(KeymapError::InvalidBinding("empty key sequence".into()));
            }
            registry.get(&a.command).map_err(KeymapError::Registry)?;
            for (j, b) in self.bindings.iter().enumerate() {
                if i != j && a.mode == b.mode {
                    if a.keys == b.keys {
                        return Err(KeymapError::DuplicateBinding {
                            mode: a.mode,
                            keys: a.keys.clone(),
                        });
                    }
                    // Check if a is a strict prefix of b
                    if a.keys.len() < b.keys.len() && b.keys.starts_with(&a.keys) {
                        return Err(KeymapError::ShadowedPrefix {
                            prefix: a.keys.clone(),
                            conflicting_key: b.keys[a.keys.len()],
                        });
                    }
                }
            }
        }
        Ok(())
    }

    pub fn from_toml(source: &str) -> Result<Self, KeymapError> {
        let config: Self =
            toml::from_str(source).map_err(|e| KeymapError::InvalidToml(e.to_string()))?;
        config.validate()?;
        Ok(config)
    }
}
