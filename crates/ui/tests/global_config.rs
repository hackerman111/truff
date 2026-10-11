use std::path::PathBuf;
use truff_ui::app::global_config::GlobalConfig;

#[test]
fn test_default_config_is_empty() {
    let config = GlobalConfig::default();
    assert_eq!(config.last_vault, None);
    assert!(config.recent_vaults.is_empty());
    assert_eq!(config.theme, "nord");
}

#[test]
fn test_theme_config_defaults_to_nord_and_deserializes_custom() {
    let toml_str = r##"
        theme = "custom_nord"

        [themes.custom_nord]
        bg_primary = "#2e3440"
        bg_secondary = "#242933"
        accent = "#88c0d0"
    "##;
    let config: GlobalConfig = toml::from_str(toml_str).expect("parse config with themes");
    assert_eq!(config.theme, "custom_nord");
    assert!(config.themes.contains_key("custom_nord"));
    let theme_cfg = &config.themes["custom_nord"];
    assert_eq!(theme_cfg.bg_primary.as_deref(), Some("#2e3440"));
}

#[test]
fn test_record_vault_deduplicates_and_orders() {
    let mut config = GlobalConfig::default();
    let p1 = PathBuf::from("/tmp/vault1");
    let p2 = PathBuf::from("/tmp/vault2");

    config.add_recent_vault(p1.clone());
    assert_eq!(config.last_vault, Some(p1.clone()));
    assert_eq!(config.recent_vaults, vec![p1.clone()]);

    config.add_recent_vault(p2.clone());
    assert_eq!(config.last_vault, Some(p2.clone()));
    assert_eq!(config.recent_vaults, vec![p2.clone(), p1.clone()]);

    // Adding p1 again moves it to the front
    config.add_recent_vault(p1.clone());
    assert_eq!(config.last_vault, Some(p1.clone()));
    assert_eq!(config.recent_vaults, vec![p1.clone(), p2.clone()]);
}

#[test]
fn test_roundtrip_toml() {
    let mut config = GlobalConfig::default();
    config.add_recent_vault(PathBuf::from("/tmp/test_vault"));

    let toml_str = toml::to_string_pretty(&config).expect("serialization succeeds");
    let loaded: GlobalConfig = toml::from_str(&toml_str).expect("deserialization succeeds");

    assert_eq!(config, loaded);
}

static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn test_app_auto_opens_saved_vault() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let tmp = std::env::temp_dir().join(format!("truff-test-autoopen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let config_dir = tmp.join("config");
    let vault_dir = tmp.join("my_vault");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::create_dir_all(&vault_dir).unwrap();
    std::fs::write(vault_dir.join("note.md"), "hello auto open\n").unwrap();

    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &config_dir);
    }

    let mut config = GlobalConfig::default();
    config.record_vault(&vault_dir);

    let mut app = truff_ui::app::Application::new();
    assert_eq!(app.vault_path(), None);

    let opened = app.try_auto_open_saved_vault().unwrap();
    assert!(opened);
    assert_eq!(
        app.vault_path(),
        Some(vault_dir.canonicalize().unwrap().as_path())
    );

    // Test :vaults command
    app.execute_command("vaults").unwrap();
    assert!(app.status.contains("Saved vaults:"));

    let _ = app.shutdown();
    unsafe {
        std::env::remove_var("XDG_CONFIG_HOME");
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_create_and_open_vault() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let tmp = std::env::temp_dir().join(format!("truff-test-create-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let target_vault = tmp.join("new_created_vault");

    let mut app = truff_ui::app::Application::new();
    assert_eq!(app.vault_path(), None);

    app.create_and_open_vault(&target_vault, Some("Welcome.md"))
        .unwrap();
    assert_eq!(
        app.vault_path(),
        Some(target_vault.canonicalize().unwrap().as_path())
    );
    assert!(target_vault.join("Welcome.md").exists());

    let _ = app.shutdown();
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_prune_nonexistent_removes_dead_vaults() {
    let tmp = std::env::temp_dir().join(format!("truff-test-prune-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let existing = tmp.join("existing_vault");
    std::fs::create_dir_all(&existing).unwrap();
    let dead = tmp.join("dead_vault"); // never created

    let mut config = GlobalConfig::default();
    config.add_recent_vault(existing.clone());
    config.add_recent_vault(dead.clone());
    assert_eq!(config.recent_vaults.len(), 2);
    assert_eq!(config.last_vault, Some(dead.clone()));

    let changed = config.prune_nonexistent();
    assert!(changed);
    assert_eq!(config.recent_vaults, vec![existing.clone()]);
    assert_eq!(config.last_vault, Some(existing.clone()));

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_remove_recent_vault() {
    let mut config = GlobalConfig::default();
    let p1 = PathBuf::from("/tmp/vault_a");
    let p2 = PathBuf::from("/tmp/vault_b");
    config.add_recent_vault(p1.clone());
    config.add_recent_vault(p2.clone());

    config.remove_recent_vault(&p2);
    assert_eq!(config.recent_vaults, vec![p1.clone()]);
    assert_eq!(config.last_vault, Some(p1));
}

#[test]
fn typography_config_defaults_and_parsing() {
    let toml_str = r#"
        theme = "nord"
        [typography]
        font_family = "monospace"
        base_font_size = 18.0
        line_height_multiplier = 1.6
    "#;
    let config: truff_ui::app::global_config::GlobalConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.typography.font_family, "monospace");
    assert_eq!(config.typography.base_font_size, 18.0);
    assert_eq!(config.typography.line_height_multiplier, 1.6);
    assert_eq!(config.typography.custom_font_path, None);

    let default_config = truff_ui::app::global_config::GlobalConfig::default();
    assert_eq!(default_config.typography.font_family, "monospace");
    assert_eq!(default_config.typography.base_font_size, 15.0);
    assert_eq!(default_config.typography.line_height_multiplier, 1.5);
    assert_eq!(default_config.typography.custom_font_path, None);
}

#[test]
fn test_settings_and_config_commands() {
    let mut app = truff_ui::app::Application::new();
    assert!(app.execute_command("settings").is_ok());
    assert!(app.settings_requested);
    app.settings_requested = false;

    assert!(app.execute_command("config").is_ok());
    assert!(app.settings_requested);
    app.settings_requested = false;

    assert!(app.execute_command("settings size 19").is_ok());
    assert_eq!(app.shell.typography().base_font_size, 19.0);

    assert!(app.execute_command("settings reset").is_ok());
    assert_eq!(app.shell.typography().base_font_size, 15.0);
}

#[test]
fn execute_settings_command() {
    let mut app = truff_ui::app::Application::new();
    assert!(app.execute_command("settings").is_ok());
    assert!(app.execute_command("config").is_ok());
}
