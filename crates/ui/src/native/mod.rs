//! Native egui/glow adapter. Application owns domain state; this module owns events and paint.
mod backlinks;
mod content;
pub mod creation;
mod editor;
mod events;
mod hints;
pub mod icons;
mod paint;
mod sidebar;
mod status;
mod tabs;
pub mod widgets;

use crate::app::Application;
pub use editor::EditorWidget;
use eframe::egui;
pub use paint::resolve_font_and_line_height;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub fn configure_fonts(
    ctx: &egui::Context,
    typography: &crate::app::global_config::TypographyConfig,
) {
    let mut fonts = egui::FontDefinitions::default();
    if let Some(path) = &typography.custom_font_path
        && path.is_file()
        && let Ok(bytes) = std::fs::read(path)
    {
        fonts.font_data.insert(
            "custom_font".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .insert(0, "custom_font".to_owned());
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "custom_font".to_owned());
    }
    ctx.set_fonts(fonts);
}

pub struct NativeApp {
    pub application: Application,
    pub editor: EditorWidget,
    pub dialogs: creation::CreationDialogs,
    hints_tick: Instant,
    last_typography: crate::app::global_config::TypographyConfig,
}

impl NativeApp {
    pub fn new(mut application: Application, context: &egui::Context) -> Self {
        let wake_context = context.clone();
        application.set_storage_waker(Arc::new(move || wake_context.request_repaint()));
        configure_fonts(context, application.shell.typography());
        let last_typography = application.shell.typography().clone();
        Self {
            application,
            editor: EditorWidget::default(),
            hints_tick: Instant::now(),
            dialogs: creation::CreationDialogs::default(),
            last_typography,
        }
    }
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        if self.application.shell.typography() != &self.last_typography {
            configure_fonts(&context, self.application.shell.typography());
            self.last_typography = self.application.shell.typography().clone();
        }
        self.events(&context);
        self.application.poll();
        let now = Instant::now();
        self.application.shell.which_key_mut().tick(
            now.duration_since(self.hints_tick)
                .as_millis()
                .min(u64::MAX as u128) as u64,
        );
        self.hints_tick = now;
        let theme = self.application.shell.theme().clone();
        paint::apply_theme(ui, &theme);
        tabs::show(&mut self.application, ui, &theme);
        status::show(&self.application, ui, &theme);
        sidebar::show(&mut self.application, &mut self.dialogs, ui, &theme);
        backlinks::show(&mut self.application, ui, &theme);
        let open_vault = content::show(&mut self.application, &mut self.editor, ui, &theme);
        if open_vault {
            self.dialogs.vault.open = true;
        }
        self.dialogs.show(&mut self.application, &context, &theme);
        hints::show(&self.application.shell, &context);
        if let Some(id) = self.application.shell.workspace().active_view()
            && let Some(view) = self.application.shell.workspace_mut().get_view_mut(id)
        {
            view.set_scroll_top_line(self.editor.top_line());
        }
        if let Some(deadline) = self.application.recovery_deadline() {
            context.request_repaint_after(deadline.saturating_duration_since(Instant::now()));
        }
        if !self.application.shell.which_key().hints().is_empty()
            && !self.application.shell.which_key().is_visible()
        {
            context.request_repaint_after(Duration::from_millis(300));
        }
        if self.application.quit_requested
            || context.input(|input| input.viewport().close_requested())
        {
            match self.application.shutdown() {
                Ok(()) => context.send_viewport_cmd(egui::ViewportCommand::Close),
                Err(error) => {
                    self.application.status = error.to_string();
                    self.application.quit_requested = false;
                    context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                }
            }
        }
    }
}
impl eframe::App for NativeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        self.show(ui);
    }
    fn persist_egui_memory(&self) -> bool {
        true
    }
}
