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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub last_vault: Option<PathBuf>,
    #[serde(default)]
    pub recent_vaults: Vec<PathBuf>,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub themes: BTreeMap<String, CustomThemeConfig>,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            last_vault: None,
            recent_vaults: Vec::new(),
            theme: default_theme(),
            themes: BTreeMap::new(),
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
        toml::from_str(&contents).unwrap_or_default()
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
        self.add_recent_vault(path_buf);
        let _ = self.save();
    }
}
