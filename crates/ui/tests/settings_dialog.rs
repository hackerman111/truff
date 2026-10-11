use truff_ui::app::global_config::TypographyConfig;
use truff_ui::native::creation::SettingsDialog;

#[test]
fn test_settings_dialog_default() {
    let dialog = SettingsDialog::default();
    assert!(!dialog.open);
    assert_eq!(dialog.font_family, "monospace");
    assert_eq!(dialog.base_font_size, 15.0);
    assert_eq!(dialog.line_height_multiplier, 1.5);
    assert_eq!(dialog.custom_font_path, "");
    assert_eq!(dialog.active_theme, "nord");
    assert!(dialog.error.is_none());
}

#[test]
fn test_settings_dialog_init_from_app_and_config() {
    let mut dialog = SettingsDialog::default();
    let typo = TypographyConfig {
        font_family: "proportional".into(),
        base_font_size: 20.0,
        line_height_multiplier: 1.7,
        custom_font_path: None,
    };
    dialog.init_from_app(&typo, "light");
    assert_eq!(dialog.font_family, "proportional");
    assert_eq!(dialog.base_font_size, 20.0);
    assert_eq!(dialog.line_height_multiplier, 1.7);
    assert_eq!(dialog.custom_font_path, "");
    assert_eq!(dialog.active_theme, "light");

    let typo_custom = TypographyConfig {
        font_family: "custom".into(),
        base_font_size: 16.0,
        line_height_multiplier: 1.5,
        custom_font_path: Some(std::path::PathBuf::from("/tmp/font.ttf")),
    };
    dialog.init_from_config(&typo_custom, "dark");
    assert_eq!(dialog.font_family, "custom");
    assert_eq!(dialog.custom_font_path, "/tmp/font.ttf");
    assert_eq!(dialog.active_theme, "dark");
}
