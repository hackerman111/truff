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

    pub(super) fn apply_settings_command(&mut self, argument: &str) -> AppResult<()> {
        if argument.is_empty() || argument == "open" {
            self.settings_requested = true;
            self.status = "Настройки открыты".into();
            return Ok(());
        }

        if let Some(val) = argument
            .strip_prefix("size ")
            .or_else(|| argument.strip_prefix("font_size "))
        {
            let val = val.trim();
            if let Ok(size) = val.parse::<f32>() {
                if (10.0..=36.0).contains(&size) {
                    let mut config = global_config::GlobalConfig::load();
                    config.typography.base_font_size = size;
                    let _ = config.save();
                    self.shell.set_typography(config.typography);
                    self.status = format!("Базовый размер шрифта: {size}px");
                    return Ok(());
                } else {
                    return Err(format!(
                        "Некорректный размер шрифта: {size}. Допустимый диапазон: 10.0..=36.0"
                    )
                    .into());
                }
            } else {
                return Err(format!("Некорректное значение размера шрифта: {val}").into());
            }
        }

        if argument == "reset" {
            let mut config = global_config::GlobalConfig::load();
            config.typography = global_config::TypographyConfig::default();
            let _ = config.save();
            self.shell.set_typography(config.typography);
            self.status = "Типографика сброшена к умолчанию".into();
            return Ok(());
        }

        self.settings_requested = true;
        Ok(())
    }
}
