use std::collections::BTreeMap;

use truff_core::commands::{CommandContext, CommandId, CommandRegistry};
use truff_core::config::{BindingLookup, KeyBinding, KeyHint, KeymapConfig, LimitsConfig};
use truff_core::vim::{Key, Mode};

fn binding(mode: Mode, keys: &[char], command: &'static str, description: &str) -> KeyBinding {
    KeyBinding {
        mode,
        keys: keys.iter().copied().map(Key::Char).collect(),
        command: CommandId::from_static(command),
        description: description.into(),
    }
}

fn custom_config() -> KeymapConfig {
    KeymapConfig {
        bindings: vec![
            binding(
                Mode::Normal,
                &['a', 'b', 'x'],
                "motion.left",
                "first shared prefix",
            ),
            binding(
                Mode::Normal,
                &['a', 'b', 'y'],
                "motion.right",
                "second shared prefix",
            ),
            binding(Mode::Normal, &['a', 'c', 'z'], "motion.up", "third prefix"),
            binding(
                Mode::Normal,
                &['q', 'r', 's'],
                "edit.delete_line",
                "editor only",
            ),
            binding(
                Mode::Normal,
                &['q', 'r', 't'],
                "motion.left",
                "rows after unavailable binding",
            ),
            binding(Mode::Insert, &['i', 'b', 'x'], "motion.left", "insert"),
            binding(Mode::VisualChar, &['v', 'b', 'x'], "motion.right", "visual"),
            binding(Mode::Normal, &['z'], "app.help", "global command"),
            binding(
                Mode::VisualLine,
                &['l', 'b', 'x'],
                "motion.up",
                "line visual",
            ),
        ],
        limits: LimitsConfig::default(),
    }
}

fn reference_hints(
    config: &KeymapConfig,
    mode: Mode,
    prefix: &[Key],
    context: CommandContext,
) -> Vec<KeyHint> {
    let registry = CommandRegistry::default();
    let mut hints = BTreeMap::new();
    for binding in &config.bindings {
        if binding.mode == mode
            && binding.keys.starts_with(prefix)
            && binding.keys.len() > prefix.len()
            && registry.require(&binding.command, context).is_ok()
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

fn reference_lookup(
    config: &KeymapConfig,
    mode: Mode,
    keys: &[Key],
    context: CommandContext,
) -> BindingLookup {
    let registry = CommandRegistry::default();
    for binding in &config.bindings {
        if binding.mode == mode
            && binding.keys.starts_with(keys)
            && registry.require(&binding.command, context).is_ok()
        {
            if binding.keys.len() == keys.len() {
                return BindingLookup::Complete(binding.command.clone());
            }
            return BindingLookup::Prefix;
        }
    }
    BindingLookup::Missing
}

#[test]
fn hints_match_btree_reference_for_modes_contexts_and_prefixes() {
    let mut reversed = custom_config();
    reversed.bindings.reverse();
    let configs = [KeymapConfig::default(), custom_config(), reversed];
    let modes = [
        Mode::Normal,
        Mode::Insert,
        Mode::VisualChar,
        Mode::VisualLine,
    ];
    let contexts = [
        CommandContext::Editor,
        CommandContext::Rows,
        CommandContext::Global,
    ];
    for config in &configs {
        let map = config.compile().unwrap();
        let mut prefixes = vec![vec![], vec![Key::Char('~')]];
        for binding in &config.bindings {
            for len in 0..=binding.keys.len() {
                prefixes.push(binding.keys[..len].to_vec());
            }
        }
        for mode in modes {
            for context in contexts {
                for prefix in &prefixes {
                    assert_eq!(
                        map.hints(mode, prefix, context),
                        reference_hints(config, mode, prefix, context),
                        "mode={mode:?}, context={context:?}, prefix={prefix:?}",
                    );
                    assert_eq!(
                        map.lookup(mode, prefix, context),
                        reference_lookup(config, mode, prefix, context),
                        "lookup mode={mode:?}, context={context:?}, prefix={prefix:?}",
                    );
                }
            }
        }
    }
}

#[test]
fn duplicate_next_keys_keep_first_binding_and_sort_by_key() {
    let config = custom_config();
    let hints =
        config
            .compile()
            .unwrap()
            .hints(Mode::Normal, &[Key::Char('a')], CommandContext::Editor);
    assert_eq!(
        hints.iter().map(|hint| hint.key).collect::<Vec<_>>(),
        vec![Key::Char('b'), Key::Char('c')]
    );
    assert_eq!(hints[0].description, "first shared prefix");
    assert!(hints[0].is_prefix);

    let map = config.compile().unwrap();
    assert_eq!(
        map.hints(Mode::Normal, &[Key::Char('q')], CommandContext::Rows),
        vec![KeyHint {
            key: Key::Char('r'),
            description: "rows after unavailable binding".into(),
            is_prefix: true,
        }]
    );
    assert_eq!(
        map.lookup(
            Mode::Normal,
            &[Key::Char('q'), Key::Char('r'), Key::Char('t')],
            CommandContext::Rows,
        ),
        BindingLookup::Complete(CommandId::from_static("motion.left"))
    );
}
