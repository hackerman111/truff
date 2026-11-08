use super::editor::color;
use super::{icons, widgets};
use crate::app::Application;
use crate::app::global_config::GlobalConfig;
use crate::theme::Theme;
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

    /// Renders clickable recent-vault rows with a remove action button.
    pub(super) fn show_rows(
        &mut self,
        application: &mut Application,
        ui: &mut egui::Ui,
        theme: &Theme,
        show_full_path: bool,
    ) {
        let mut remove_path = None;
        for path in &self.config.recent_vaults {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("vault");
            let path_str = path.display().to_string();

            ui.horizontal(|ui| {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                icons::render_icon(
                    ui.painter(),
                    icons::Icon::Folder,
                    rect,
                    color(theme.accent),
                    1.2,
                );

                let mut label = egui::RichText::new(name).color(color(theme.text_primary));
                if show_full_path {
                    label = label.strong();
                }
                let resp = ui.selectable_label(false, label).on_hover_text(&path_str);
                if resp.clicked()
                    && let Err(e) = application.open_vault(path)
                {
                    application.status = e.to_string();
                }

                if show_full_path {
                    ui.label(
                        egui::RichText::new(&path_str)
                            .size(11.0)
                            .color(color(theme.text_muted)),
                    );
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::icon_button(ui, icons::Icon::Close, "Удалить из недавних").clicked()
                    {
                        remove_path = Some(path.clone());
                    }
                });
            });
            ui.add_space(2.0);
        }
        if let Some(to_remove) = remove_path {
            self.remove(&to_remove);
        }
    }
}
