use truff_core::commands::{CommandContext, CommandId, CommandRegistry};

#[test]
fn test_format_table_command_registered() {
    let registry = CommandRegistry::default();
    let cmd_id = CommandId::new("markdown.format_table");

    let spec = registry
        .get(&cmd_id)
        .expect("markdown.format_table command should exist");
    assert_eq!(spec.title, "Форматировать таблицу");
    assert!(spec.contexts.contains(&CommandContext::Editor));
    assert!(registry.is_available(&cmd_id, CommandContext::Editor));
}
