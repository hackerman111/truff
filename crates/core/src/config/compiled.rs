use crate::commands::{CommandContext, CommandId, CommandRegistry};
use crate::vim::{Key, Mode};

use super::{KeyBinding, KeyHint, KeymapConfig, KeymapError};

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
