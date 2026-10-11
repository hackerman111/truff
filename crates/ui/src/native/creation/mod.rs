mod file;
mod folder;
mod vault;

pub use file::FileDialog;
pub use folder::FolderDialog;
pub use vault::VaultDialog;

use crate::app::Application;
use crate::theme::Theme;
use eframe::egui;

#[derive(Default)]
pub struct CreationDialogs {
    pub vault: VaultDialog,
    pub file: FileDialog,
    pub folder: FolderDialog,
}

impl CreationDialogs {
    pub fn is_any_open(&self) -> bool {
        self.vault.open || self.file.open || self.folder.open
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
    }
}
