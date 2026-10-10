use std::fmt;

use serde::{Deserialize, Serialize};

use crate::commands::{CommandContext, CommandId, CommandRegistry, RegistryError};
use crate::vim::{Key, Mode};

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

impl Default for KeymapConfig {
    fn default() -> Self {
        let bindings = vec![
            // Motions
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('h')],
                command: CommandId::from_static("motion.left"),
                description: "Move left".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('j')],
                command: CommandId::from_static("motion.down"),
                description: "Move down".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('k')],
                command: CommandId::from_static("motion.up"),
                description: "Move up".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('l')],
                command: CommandId::from_static("motion.right"),
                description: "Move right".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('w')],
                command: CommandId::from_static("motion.word_forward"),
                description: "Word forward".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('b')],
                command: CommandId::from_static("motion.word_backward"),
                description: "Word backward".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('e')],
                command: CommandId::from_static("motion.word_end"),
                description: "End of word".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('0')],
                command: CommandId::from_static("motion.line_start"),
                description: "Line start".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('^')],
                command: CommandId::from_static("motion.first_non_blank"),
                description: "First non-blank character".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('$')],
                command: CommandId::from_static("motion.line_end"),
                description: "Line end".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('g'), Key::Char('g')],
                command: CommandId::from_static("motion.first_line"),
                description: "First line of buffer".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('g'), Key::Char('j')],
                command: CommandId::from_static("motion.screen_down"),
                description: "Screen line down".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('g'), Key::Char('k')],
                command: CommandId::from_static("motion.screen_up"),
                description: "Screen line up".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('G')],
                command: CommandId::from_static("motion.last_line"),
                description: "Last line of buffer".to_string(),
            },
            // Edits & operators
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('d'), Key::Char('d')],
                command: CommandId::from_static("edit.delete_line"),
                description: "Delete line".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('c'), Key::Char('c')],
                command: CommandId::from_static("edit.change_line"),
                description: "Change line".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('y'), Key::Char('y')],
                command: CommandId::from_static("edit.yank_line"),
                description: "Yank line".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('x')],
                command: CommandId::from_static("edit.delete_char"),
                description: "Delete character".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('p')],
                command: CommandId::from_static("edit.paste_after"),
                description: "Paste after".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('P')],
                command: CommandId::from_static("edit.paste_before"),
                description: "Paste before".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('u')],
                command: CommandId::from_static("edit.undo"),
                description: "Undo".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Ctrl('r')],
                command: CommandId::from_static("edit.redo"),
                description: "Redo".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('.')],
                command: CommandId::from_static("edit.repeat"),
                description: "Repeat last edit".to_string(),
            },
        ];

        Self {
            bindings,
            limits: LimitsConfig::default(),
        }
    }
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
}

/// Compiled prefixes are the single source for key lookup and contextual hints.
#[derive(Debug, Clone)]
pub struct CompiledKeymap {
    prefixes: std::collections::BTreeMap<Mode, std::collections::BTreeMap<Vec<Key>, Vec<usize>>>,
    bindings: Vec<KeyBinding>,
    registry: CommandRegistry,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingLookup {
    Complete(CommandId),
    Prefix,
    Missing,
}
impl KeymapConfig {
    pub fn from_toml(source: &str) -> Result<Self, KeymapError> {
        let config: Self =
            toml::from_str(source).map_err(|e| KeymapError::InvalidToml(e.to_string()))?;
        config.validate()?;
        Ok(config)
    }
    pub fn compile(&self) -> Result<CompiledKeymap, KeymapError> {
        self.validate()?;
        let mut prefixes = std::collections::BTreeMap::<
            Mode,
            std::collections::BTreeMap<Vec<Key>, Vec<usize>>,
        >::new();
        for (idx, binding) in self.bindings.iter().enumerate() {
            for len in 0..=binding.keys.len() {
                prefixes
                    .entry(binding.mode)
                    .or_default()
                    .entry(binding.keys[..len].to_vec())
                    .or_default()
                    .push(idx);
            }
        }
        Ok(CompiledKeymap {
            prefixes,
            bindings: self.bindings.clone(),
            registry: CommandRegistry::default(),
        })
    }
}
impl CompiledKeymap {
    pub fn lookup(&self, mode: Mode, keys: &[Key], context: CommandContext) -> BindingLookup {
        let Some(bindings) = self
            .prefixes
            .get(&mode)
            .and_then(|prefixes| prefixes.get(keys))
        else {
            return BindingLookup::Missing;
        };
        for &idx in bindings {
            let binding = &self.bindings[idx];
            if self.registry.require(&binding.command, context).is_ok() {
                if binding.keys.len() == keys.len() {
                    return BindingLookup::Complete(binding.command.clone());
                }
                return BindingLookup::Prefix;
            }
        }
        BindingLookup::Missing
    }
    pub fn hints(&self, mode: Mode, prefix: &[Key], context: CommandContext) -> Vec<KeyHint> {
        let Some(bindings) = self
            .prefixes
            .get(&mode)
            .and_then(|prefixes| prefixes.get(prefix))
        else {
            return Vec::new();
        };
        let mut hints = std::collections::BTreeMap::new();
        for &idx in bindings {
            let binding = &self.bindings[idx];
            if binding.keys.len() > prefix.len()
                && self.registry.require(&binding.command, context).is_ok()
            {
                let key = binding.keys[prefix.len()];
                hints.entry(key).or_insert_with(|| KeyHint {
                    key,
                    description: binding.description.clone(),
                    is_prefix: binding.keys.len() > prefix.len() + 1,
                });
            }
        }
        hints.into_values().collect()
    }
}
