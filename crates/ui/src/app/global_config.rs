//! Global user configuration persisting last-opened and recent vaults across sessions.

use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::theme::{CustomThemeConfig, Theme};
use std::collections::BTreeMap;

pub const MAX_RECENT_VAULTS: usize = 20;

fn default_theme() -> String {
    "nord".to_string()
}

fn default_font_family() -> String {
    "monospace".to_string()
}

fn default_base_font_size() -> f32 {
    15.0
}

fn default_line_height_multiplier() -> f32 {
    1.5
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypographyConfig {
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default = "default_base_font_size")]
    pub base_font_size: f32,
    #[serde(default = "default_line_height_multiplier")]
    pub line_height_multiplier: f32,
    #[serde(default)]
    pub custom_font_path: Option<PathBuf>,
}

impl Default for TypographyConfig {
    fn default() -> Self {
        Self {
            font_family: default_font_family(),
            base_font_size: default_base_font_size(),
            line_height_multiplier: default_line_height_multiplier(),
            custom_font_path: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub last_vault: Option<PathBuf>,
    #[serde(default)]
    pub recent_vaults: Vec<PathBuf>,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub themes: BTreeMap<String, CustomThemeConfig>,
    #[serde(default)]
    pub typography: TypographyConfig,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            last_vault: None,
            recent_vaults: Vec::new(),
            theme: default_theme(),
            themes: BTreeMap::new(),
            typography: TypographyConfig::default(),
        }
    }
}

impl GlobalConfig {
    /// Resolves the configured active Theme.
    pub fn resolve_theme(&self) -> Theme {
        match self.theme.as_str() {
            "nord" => Theme::nord(),
            "dark" | "obsidian-dark" => Theme::obsidian_dark(),
            "light" => Theme::light(),
            custom_name => {
                if let Some(custom) = self.themes.get(custom_name) {
                    Theme::nord().apply_custom(custom)
                } else {
                    Theme::nord()
                }
            }
        }
    }

    /// Returns the canonical path to `config.toml` in `$XDG_CONFIG_HOME/truff` or `~/.config/truff`.
    pub fn config_path() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("truff").join("config.toml"))
    }

    /// Loads the global configuration, returning default if missing or corrupted.
    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };
        if !path.exists() {
            return Self::default();
        }
        let Ok(contents) = fs::read_to_string(&path) else {
            return Self::default();
        };
        let mut config: Self = toml::from_str(&contents).unwrap_or_default();
        if config.prune_nonexistent() {
            let _ = config.save();
        }
        config
    }

    /// Prunes non-existent directory paths from recent vaults and last_vault.
    pub fn prune_nonexistent(&mut self) -> bool {
        let prev_len = self.recent_vaults.len();
        self.recent_vaults.retain(|p| p.is_dir());
        let last_vault_removed = self.last_vault.as_ref().is_some_and(|p| !p.is_dir());
        if last_vault_removed {
            self.last_vault = self.recent_vaults.first().cloned();
        }
        prev_len != self.recent_vaults.len() || last_vault_removed
    }

    /// Removes a specific vault path from recent vaults.
    pub fn remove_recent_vault(&mut self, path: &Path) {
        self.recent_vaults.retain(|p| p != path);
        if self.last_vault.as_deref() == Some(path) {
            self.last_vault = self.recent_vaults.first().cloned();
        }
    }

    /// Saves the global configuration atomically to `config.toml`.
    pub fn save(&self) -> io::Result<()> {
        let Some(path) = Self::config_path() else {
            return Ok(());
        };
        let Some(parent) = path.parent() else {
            return Ok(());
        };
        fs::create_dir_all(parent)?;

        let toml_str = toml::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let pid = std::process::id();
        let tmp_path = parent.join(format!(".config.toml.{pid}.tmp"));

        let write_res = (|| -> io::Result<()> {
            let mut file = File::options()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&tmp_path)?;
            file.write_all(toml_str.as_bytes())?;
            file.sync_data()?;
            drop(file);
            fs::rename(&tmp_path, &path)?;
            Ok(())
        })();

        if let Err(e) = write_res {
            let _ = fs::remove_file(&tmp_path);
            return Err(e);
        }

        Ok(())
    }

    /// Adds a vault to recent vaults and updates last_vault.
    pub fn add_recent_vault(&mut self, path: PathBuf) {
        self.last_vault = Some(path.clone());
        self.recent_vaults.retain(|p| p != &path);
        self.recent_vaults.insert(0, path);
        self.recent_vaults.truncate(MAX_RECENT_VAULTS);
    }

    /// Records a vault opening and persists the global configuration.
    pub fn record_vault(&mut self, path: &Path) {
        let path_buf = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if !path_buf.is_dir() {
            return;
        }
        let is_temp = path_buf.starts_with(std::env::temp_dir());
        let has_custom_config = std::env::var_os("XDG_CONFIG_HOME").is_some();
        if is_temp && !has_custom_config {
            return;
        }
        self.add_recent_vault(path_buf);
        let _ = self.save();
    }
}
