use super::Shell;
use crate::overlays::{HelpItem, WhichKeyOverlay, key_label};
use truff_core::{
    commands::{CommandContext, CommandRegistry},
    vim::{Key, Mode},
};

impl Shell {
    fn registered_bindings(
        &self,
        context: CommandContext,
        mode: Mode,
    ) -> Vec<(truff_core::commands::CommandId, String)> {
        let mut bindings = Vec::new();
        let mut stack = vec![Vec::<Key>::new()];
        while let Some(prefix) = stack.pop() {
            for hint in self.keymap.hints(mode, &prefix, context) {
                let mut keys = prefix.clone();
                keys.push(hint.key);
                if hint.is_prefix {
                    stack.push(keys);
                } else if let truff_core::config::BindingLookup::Complete(id) =
                    self.keymap.lookup(mode, &keys, context)
                {
                    bindings.push((
                        id,
                        keys.into_iter()
                            .map(key_label)
                            .collect::<Vec<_>>()
                            .join(" "),
                    ));
                }
            }
        }
        bindings
    }

    pub(crate) fn binding_labels(
        &self,
        context: CommandContext,
        mode: Mode,
    ) -> std::collections::BTreeMap<truff_core::commands::CommandId, String> {
        self.registered_bindings(context, mode)
            .into_iter()
            .collect()
    }

    pub(crate) fn single_binding(&self, key: Key) -> Option<truff_core::commands::CommandId> {
        let context = if self.focus == super::Focus::Editor {
            CommandContext::Editor
        } else {
            CommandContext::Rows
        };
        match self.keymap.lookup(self.mode(), &[key], context) {
            truff_core::config::BindingLookup::Complete(id) => Some(id),
            _ => None,
        }
    }

    pub fn show_help(&mut self) {
        let context = if self.focus == super::Focus::Editor {
            CommandContext::Editor
        } else {
            CommandContext::Rows
        };
        let registry = CommandRegistry::default();
        self.which_key
            .show_help(WhichKeyOverlay::default_help_categories());
        // Native Vim motions remain the fallback; registered bindings use the active keymap.
        self.which_key.help.items.retain(|item| {
            if context == CommandContext::Rows {
                return matches!(item.keys.as_str(), "h" | "j" | "k" | "l" | "g" | "G");
            }
            item.category.starts_with("Навигация") || item.category.starts_with("Редактирование")
        });
        for (id, keys) in self.registered_bindings(context, Mode::Normal) {
            if let Ok(spec) = registry.get(&id) {
                self.which_key.help.items.retain(|item| item.keys != keys);
                self.which_key.help.items.push(HelpItem {
                    category: spec.group.to_string(),
                    keys,
                    description: format!("{} · {}", spec.title, spec.description),
                });
            }
        }
        self.which_key.help.filter();
    }
}
