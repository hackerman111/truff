//! Native egui/glow adapter. Application owns domain state; this module owns events and paint.
mod editor;
mod events;
mod paint;

use crate::app::Application;
use crate::shell::Focus;
pub use editor::EditorWidget;
use editor::color;
use eframe::egui;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct NativeApp {
    pub application: Application,
    pub editor: EditorWidget,
    hints_tick: Instant,
}

impl NativeApp {
    pub fn new(mut application: Application, context: &egui::Context) -> Self {
        let wake_context = context.clone();
        application.set_storage_waker(Arc::new(move || wake_context.request_repaint()));
        Self {
            application,
            editor: EditorWidget::default(),
            hints_tick: Instant::now(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
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
        ui.visuals_mut().override_text_color = Some(color(theme.text_primary));
        ui.visuals_mut().panel_fill = color(theme.bg_primary);
        ui.visuals_mut().widgets.inactive.bg_fill = color(theme.bg_secondary);
        ui.visuals_mut().selection.bg_fill = color(theme.selection);
        egui::Panel::top("title").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Truff");
                ui.separator();
                ui.label(
                    self.application
                        .active_relative_path()
                        .unwrap_or_else(|| "Open a vault with :vault <path>".into()),
                );
                if self
                    .application
                    .shell
                    .editor_buffer()
                    .is_some_and(|buffer| buffer.is_dirty())
                {
                    ui.label("●");
                }
                if self.application.shell.which_key().is_visible() {
                    for hint in self.application.shell.which_key().formatted_lines() {
                        ui.label(hint);
                    }
                }
            });
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            if let Some(command) = self.application.shell.command() {
                ui.label(format!(":{command}"));
            } else {
                ui.horizontal(|ui| {
                    ui.strong(self.application.shell.mode().to_string());
                    ui.label(format!("{:?}", self.application.shell.focus()));
                    ui.label(&self.application.status);
                });
            }
        });
        egui::Panel::left("vault")
            .default_size(200.0)
            .min_size(120.0)
            .max_size(600.0)
            .resizable(true)
            .show_separator_line(true)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.heading("Vault");
                let model = self.application.shell.sidebar_model();
                let count = model.visible_count();
                let selected = model.selected_index();
                let mut open = None;
                egui::ScrollArea::vertical().show_rows(ui, 24.0, count, |ui, rows| {
                    for index in rows {
                        if let Some(row) = model.get_visible_row(index)
                            && ui.selectable_label(index == selected, &row.title).clicked()
                        {
                            open = row.data.clone();
                        }
                    }
                });
                if let Some(path) = open
                    && let Err(error) = self.application.open_note(&path)
                {
                    self.application.status = error.to_string();
                }
            });
        egui::Panel::right("references")
            .default_size(220.0)
            .min_size(120.0)
            .max_size(600.0)
            .resizable(true)
            .show_separator_line(true)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.heading("Backlinks");
                let mut open = None;
                egui::ScrollArea::vertical().show_rows(
                    ui,
                    24.0,
                    self.application.backlinks.len(),
                    |ui, rows| {
                        for index in rows {
                            if ui
                                .selectable_label(
                                    index == self.application.backlink_selection,
                                    &self.application.backlinks[index].source_path,
                                )
                                .clicked()
                            {
                                open = Some(index);
                            }
                        }
                    },
                );
                if let Some(index) = open {
                    self.application.backlink_selection = index;
                    self.application.shell.set_focus(Focus::RightPanel);
                    if let Err(error) = self.application.open_backlink() {
                        self.application.status = error.to_string();
                    }
                }
            });
        egui::CentralPanel::default().show(ui, |ui| {
            let shell = &self.application.shell;
            let input = self.editor.show(
                ui,
                shell.editor_buffer(),
                shell.editor_view(),
                shell.mode(),
                self.application.projection(),
                &theme,
                shell.ime_preedit(),
                shell.flash().targets(),
            );
            if let Some(input) = input {
                self.application.shell.set_focus(Focus::Editor);
                self.input(input);
            }
        });
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
