use super::super::{AppResult, Application, global_config};
use crate::theme::Theme;

impl Application {
    pub(super) fn apply_theme_command(&mut self, argument: &str) -> AppResult<()> {
        if argument.is_empty() {
            let config = global_config::GlobalConfig::load();
            self.status = format!(
                "Active theme: {}. Available: nord, dark, light, or [themes.*] from config",
                config.theme
            );
            return Ok(());
        }
        let mut config = global_config::GlobalConfig::load();
        let theme = match argument {
            "nord" => Theme::nord(),
            "dark" | "obsidian" | "obsidian-dark" => Theme::obsidian_dark(),
            "light" => Theme::light(),
            custom_name => {
                if let Some(custom) = config.themes.get(custom_name) {
                    Theme::nord().apply_custom(custom)
                } else {
                    return Err(format!("Unknown theme: {argument}. Available: nord, dark, light, or themes defined in config.toml").into());
                }
            }
        };
        self.shell.set_theme(theme);
        config.theme = argument.to_string();
        let _ = config.save();
        self.status = format!("Theme applied: {argument}");
        Ok(())
    }
}
