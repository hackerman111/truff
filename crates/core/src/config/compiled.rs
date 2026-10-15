use crate::commands::{CommandContext, CommandId, CommandRegistry};
use crate::vim::{Key, Mode};

use super::{KeyBinding, KeyHint, KeymapConfig, KeymapError};

/// Compiled prefixes are the single source for key lookup and contextual hints.
#[derive(Debug, Clone)]
pub struct CompiledKeymap {
    prefixes: std::collections::BTreeMap<Mode, std::collections::BTreeMap<Vec<Key>, Vec<usize>>>,
    bindings: Vec<CompiledBinding>,
}

#[derive(Debug, Clone)]
struct CompiledBinding {
    binding: KeyBinding,
    available: [bool; 7],
}

impl CompiledBinding {
    fn available_in(&self, context: CommandContext) -> bool {
        self.available[Self::context_index(context)]
    }

    const fn context_index(context: CommandContext) -> usize {
        match context {
            CommandContext::Editor => 0,
            CommandContext::TextField => 1,
            CommandContext::CommandLine => 2,
            CommandContext::Rows => 3,
            CommandContext::Navigator => 4,
            CommandContext::Modal => 5,
            CommandContext::Global => 6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingLookup {
    Complete(CommandId),
    Prefix,
    Missing,
}

impl KeymapConfig {
    pub fn compile(&self) -> Result<CompiledKeymap, KeymapError> {
        self.validate()?;
        let registry = CommandRegistry::default();
        // Contexts cannot change inside a compiled keymap. Resolve them once
        // instead of looking up every candidate command during key input.
        let bindings = self
            .bindings
            .iter()
            .map(|binding| {
                let spec = registry
                    .get(&binding.command)
                    .map_err(KeymapError::Registry)?;
                let mut available = [false; 7];
                for ctx in CommandContext::ALL {
                    available[CompiledBinding::context_index(ctx)] =
                        ctx.matches_spec(&spec.contexts);
                }
                Ok(CompiledBinding {
                    binding: binding.clone(),
                    available,
                })
            })
            .collect::<Result<Vec<_>, KeymapError>>()?;
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
        for mode_prefixes in prefixes.values_mut() {
            for (prefix, candidates) in mode_prefixes {
                // Validation excludes complete/prefix collisions, so this
                // order preserves lookup. Original indices break ties so hints
                // still select the first eligible binding for each next key.
                candidates.sort_unstable_by_key(|&idx| {
                    (self.bindings[idx].keys.get(prefix.len()).copied(), idx)
                });
            }
        }
        Ok(CompiledKeymap { prefixes, bindings })
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
            let compiled = &self.bindings[idx];
            if compiled.available_in(context) {
                let binding = &compiled.binding;
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
        let mut hints = Vec::<KeyHint>::new();
        for &idx in bindings {
            let compiled = &self.bindings[idx];
            let binding = &compiled.binding;
            let Some(&key) = binding.keys.get(prefix.len()) else {
                continue;
            };
            if hints.last().is_some_and(|hint| hint.key == key) || !compiled.available_in(context) {
                continue;
            }
            hints.push(KeyHint {
                key,
                description: binding.description.clone(),
                is_prefix: binding.keys.len() > prefix.len() + 1,
            });
        }
        hints
    }
}
