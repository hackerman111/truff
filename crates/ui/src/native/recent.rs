use crate::app::global_config::GlobalConfig;
use std::path::Path;
use std::time::{Duration, Instant};

/// Shared by the welcome panel and sidebar; filesystem changes become visible
/// within one poll interval without parsing configuration on every paint.
#[derive(Default)]
pub(super) struct RecentVaults {
    config: GlobalConfig,
    refreshed: Option<Instant>,
}

impl RecentVaults {
    pub(super) const INTERVAL: Duration = Duration::from_millis(500);

    pub(super) fn refresh(&mut self) {
        if self
            .refreshed
            .is_some_and(|at| at.elapsed() < Self::INTERVAL)
        {
            return;
        }
        self.config = GlobalConfig::load();
        self.refreshed = Some(Instant::now());
    }

    pub(super) fn paths(&self) -> &[std::path::PathBuf] {
        &self.config.recent_vaults
    }

    pub(super) fn remove(&mut self, path: &Path) {
        // Merge with the current file so settings saved since the last poll survive.
        self.config = GlobalConfig::load();
        self.config.remove_recent_vault(path);
        let _ = self.config.save();
        self.refreshed = Some(Instant::now());
    }
}
