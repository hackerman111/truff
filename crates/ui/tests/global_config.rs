use std::path::PathBuf;
use truff_ui::app::global_config::GlobalConfig;

#[test]
fn test_default_config_is_empty() {
    let config = GlobalConfig::default();
    assert_eq!(config.last_vault, None);
    assert!(config.recent_vaults.is_empty());
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

#[test]
fn test_app_auto_opens_saved_vault() {
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
    let _ = std::fs::remove_dir_all(&tmp);
}
