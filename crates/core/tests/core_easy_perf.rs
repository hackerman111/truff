use std::path::PathBuf;

use truff_core::commands::CommandId;
use truff_core::config::{KeyBinding, KeymapConfig, KeymapError};
use truff_core::text::BytePos;
use truff_core::vim::{Key, Mode};
use truff_core::workspace::{StoreEvent, Workspace};

fn binding(mode: Mode, keys: &[char], command: &'static str) -> KeyBinding {
    KeyBinding {
        mode,
        keys: keys.iter().copied().map(Key::Char).collect(),
        command: CommandId::from_static(command),
        description: String::new(),
    }
}

fn config(bindings: Vec<KeyBinding>) -> KeymapConfig {
    KeymapConfig {
        bindings,
        limits: Default::default(),
    }
}

#[test]
fn keymap_prefix_conflicts_are_reported_in_either_input_order() {
    let short = binding(Mode::Normal, &['g'], "motion.left");
    let long = binding(Mode::Normal, &['g', 'g'], "motion.right");
    for bindings in [vec![short.clone(), long.clone()], vec![long, short]] {
        assert!(matches!(
            config(bindings).validate(),
            Err(KeymapError::ShadowedPrefix { .. })
        ));
    }
}

#[test]
fn keymap_prefixes_and_duplicates_are_scoped_to_the_mode() {
    let normal = binding(Mode::Normal, &['g'], "motion.left");
    let insert = binding(Mode::Insert, &['g', 'g'], "motion.right");
    assert!(config(vec![normal.clone(), insert]).validate().is_ok());

    assert!(matches!(
        config(vec![normal.clone(), normal]).validate(),
        Err(KeymapError::DuplicateBinding { .. })
    ));
}

#[test]
fn keymap_validation_rejects_invalid_commands_and_sequences() {
    let mut invalid_command = binding(Mode::Normal, &['z'], "command.does_not_exist");
    assert!(matches!(
        config(vec![invalid_command.clone()]).validate(),
        Err(KeymapError::Registry(_))
    ));

    invalid_command.command = CommandId::from_static("motion.left");
    invalid_command.keys.clear();
    assert!(matches!(
        config(vec![invalid_command]).validate(),
        Err(KeymapError::InvalidBinding(_))
    ));
}

#[test]
fn unicode_load_snaps_every_attached_view_to_buffer_graphemes() {
    let mut workspace = Workspace::new();
    let buffer_id = workspace
        .open_buffer_from_text(PathBuf::from("note.md"), "old text")
        .unwrap();
    let first = workspace.create_view(buffer_id).unwrap();
    let second = workspace.create_view(buffer_id).unwrap();
    workspace
        .get_view_mut(first)
        .unwrap()
        .set_cursor(BytePos::new(1), BytePos::new(4));
    workspace
        .get_view_mut(second)
        .unwrap()
        .set_cursor(BytePos::new(2), BytePos::new(7));
    let key = workspace.buffers().get_snapshot_key(buffer_id).unwrap();

    let effects = workspace
        .accept(StoreEvent::FileLoaded {
            key,
            content: "e\u{301}xy".into(),
            fingerprint: "fp".into(),
        })
        .unwrap();

    let first_view = workspace.get_view(first).unwrap();
    assert_eq!(first_view.anchor(), BytePos::new(0));
    assert_eq!(first_view.head(), BytePos::new(4));
    let second_view = workspace.get_view(second).unwrap();
    assert_eq!(second_view.anchor(), BytePos::new(0));
    assert_eq!(second_view.head(), BytePos::new(5));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        truff_core::commands::Effect::InvalidateView(id) if *id == first
    )));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        truff_core::commands::Effect::InvalidateView(id) if *id == second
    )));
}
