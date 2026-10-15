mod file;
mod folder;
mod operation;
mod settings;
mod vault;

pub use file::FileDialog;
pub use folder::FolderDialog;
pub use operation::OperationDialog;
pub use settings::SettingsDialog;
pub use vault::VaultDialog;

use crate::app::Application;
use crate::theme::Theme;
use eframe::egui;

#[derive(Default)]
pub struct CreationDialogs {
    pub vault: VaultDialog,
    pub file: FileDialog,
    pub folder: FolderDialog,
    pub settings: SettingsDialog,
    pub operation: OperationDialog,
}

impl CreationDialogs {
    pub fn is_any_open(&self) -> bool {
        self.operation.is_open()
            || self.vault.open
            || self.file.open
            || self.folder.open
            || self.settings.open
    }

    pub(super) fn show(
        &mut self,
        application: &mut Application,
        context: &egui::Context,
        theme: &Theme,
    ) {
        self.vault.show(application, context, theme);
        self.file.show(application, context, theme);
        self.folder.show(application, context, theme);
        self.settings.show(application, context, theme);
        self.operation.show(application, context, theme);
    }
}
