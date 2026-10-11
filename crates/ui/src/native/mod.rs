//! Native egui/glow adapter. Application owns domain state; this module owns events and paint.
mod editor;
mod events;
mod paint;

use crate::app::Application;
use crate::shell::Focus;
pub use editor::EditorWidget;
use editor::color;
use eframe::egui;
pub use paint::resolve_font_and_line_height;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct NativeApp {
    pub application: Application,
    pub editor: EditorWidget,
    hints_tick: Instant,
    pub show_create_vault_dialog: bool,
    pub vault_path_input: String,
    pub initial_note_name: String,
    pub create_starter_note: bool,
    pub create_vault_error: Option<String>,
}

impl NativeApp {
    pub fn new(mut application: Application, context: &egui::Context) -> Self {
        let wake_context = context.clone();
        application.set_storage_waker(Arc::new(move || wake_context.request_repaint()));
        Self {
            application,
            editor: EditorWidget::default(),
            hints_tick: Instant::now(),
            show_create_vault_dialog: false,
            vault_path_input: "~/TruffVault".to_string(),
            initial_note_name: "Welcome.md".to_string(),
            create_starter_note: true,
            create_vault_error: None,
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
        let mut visuals = ui.visuals().clone();
        visuals.override_text_color = Some(color(theme.text_primary));
        visuals.panel_fill = color(theme.bg_primary);
        visuals.window_fill = color(theme.bg_surface);
        visuals.widgets.noninteractive.bg_fill = color(theme.bg_primary);
        visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, color(theme.border));
        visuals.widgets.inactive.bg_fill = color(theme.bg_secondary);
        visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, color(theme.text_primary));
        visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(4);
        visuals.widgets.hovered.bg_fill = color(theme.selection);
        visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, color(theme.text_primary));
        visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(4);
        visuals.widgets.active.bg_fill = color(theme.selection);
        visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, color(theme.text_primary));
        visuals.widgets.active.corner_radius = egui::CornerRadius::same(4);
        visuals.selection.bg_fill = color(theme.selection);
        visuals.selection.stroke = egui::Stroke::new(1.0, color(theme.accent));
        *ui.visuals_mut() = visuals;

        egui::Panel::top("tabs")
            .frame(
                egui::Frame::new()
                    .fill(color(theme.bg_secondary))
                    .stroke(egui::Stroke::new(1.0, color(theme.border)))
                    .inner_margin(egui::Margin::symmetric(8, 2)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let active_view_id = self.application.shell.workspace().active_view();
                    let mut views: Vec<_> = self
                        .application
                        .shell
                        .workspace()
                        .views()
                        .values()
                        .collect();
                    views.sort_by_key(|v| v.id().get());

                    let mut switch_view = None;
                    let mut close_view = None;

                    if views.is_empty() {
                        ui.label(
                            egui::RichText::new("Truff")
                                .strong()
                                .color(color(theme.accent)),
                        );
                        ui.label(egui::RichText::new("·").color(color(theme.text_muted)));
                        ui.label(
                            egui::RichText::new("Open a vault with :vault <path>")
                                .color(color(theme.text_muted)),
                        );
                    } else {
                        for view in views {
                            let view_id = view.id();
                            let is_active = Some(view_id) == active_view_id;
                            let buffer_id = view.buffer_id();
                            let entry = self.application.shell.workspace().buffers().get(buffer_id);
                            let title = entry
                                .and_then(|e| e.path())
                                .and_then(|p| p.file_name())
                                .and_then(|n| n.to_str())
                                .unwrap_or("Scratch")
                                .to_string();
                            let is_dirty = self
                                .application
                                .shell
                                .workspace()
                                .buffers()
                                .get_buffer(buffer_id)
                                .is_some_and(|b| b.is_dirty());

                            let tab_bg = if is_active {
                                color(theme.bg_primary)
                            } else {
                                egui::Color32::TRANSPARENT
                            };
                            let text_color = if is_active {
                                color(theme.text_primary)
                            } else {
                                color(theme.text_muted)
                            };

                            let frame = egui::Frame::new()
                                .fill(tab_bg)
                                .stroke(if is_active {
                                    egui::Stroke::new(1.0, color(theme.border))
                                } else {
                                    egui::Stroke::NONE
                                })
                                .corner_radius(egui::CornerRadius {
                                    nw: 4,
                                    ne: 4,
                                    sw: 0,
                                    se: 0,
                                })
                                .inner_margin(egui::Margin::symmetric(10, 4));

                            frame.show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let glyph = if is_active { "📄 " } else { " " };
                                    let rich = egui::RichText::new(format!("{glyph}{title}"))
                                        .size(12.5)
                                        .color(text_color);
                                    let resp = ui.selectable_label(false, rich);
                                    if resp.clicked() {
                                        switch_view = Some(view_id);
                                    }
                                    if is_dirty {
                                        ui.label(
                                            egui::RichText::new("●")
                                                .size(10.0)
                                                .color(color(theme.accent)),
                                        );
                                    }
                                    if ui.small_button("×").on_hover_text("Close buffer").clicked()
                                    {
                                        close_view = Some(view_id);
                                    }
                                });
                            });
                        }
                    }

                    if let Some(id) = switch_view {
                        self.application
                            .shell
                            .workspace_mut()
                            .set_active_view(Some(id));
                        self.application.shell.reset_editor_input();
                    }
                    if let Some(id) = close_view {
                        let _ = self
                            .application
                            .dispatch(truff_core::commands::Action::CloseView(id));
                    }
                });
            });

        egui::Panel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(color(theme.bg_tertiary))
                    .stroke(egui::Stroke::new(1.0, color(theme.border)))
                    .inner_margin(egui::Margin::symmetric(12, 4)),
            )
            .show(ui, |ui| {
                if let Some(command) = self.application.shell.command() {
                    ui.label(
                        egui::RichText::new(format!(":{command}"))
                            .monospace()
                            .color(color(theme.accent))
                            .strong(),
                    );
                } else {
                    ui.horizontal(|ui| {
                        let mode = self.application.shell.mode();
                        let mode_str = mode.to_string();
                        let (pill_bg, pill_fg) = match mode {
                            truff_core::vim::Mode::Insert => {
                                (color(theme.accent), color(theme.bg_primary))
                            }
                            truff_core::vim::Mode::VisualChar
                            | truff_core::vim::Mode::VisualLine => {
                                (color(theme.callout_warning), color(theme.bg_primary))
                            }
                            _ => (color(theme.border), color(theme.text_primary)),
                        };
                        egui::Frame::new()
                            .fill(pill_bg)
                            .corner_radius(egui::CornerRadius::same(3))
                            .inner_margin(egui::Margin::symmetric(6, 2))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(mode_str)
                                        .size(11.0)
                                        .strong()
                                        .color(pill_fg),
                                );
                            });
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(format!("{:?}", self.application.shell.focus()))
                                .size(11.5)
                                .color(color(theme.text_muted)),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(&self.application.status)
                                .size(11.5)
                                .color(color(theme.text_muted)),
                        );

                        // Right-aligned cursor position and metadata
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new("Markdown · UTF-8")
                                    .size(11.5)
                                    .color(color(theme.text_muted)),
                            );
                            ui.add_space(8.0);
                            let cursor_str = if let Some(view) =
                                self.application.shell.editor_view()
                                && let Some(buffer) = self.application.shell.editor_buffer()
                            {
                                let head = view.head().get().min(buffer.byte_len());
                                let line = buffer.line_of_byte(head) + 1;
                                let line_start = buffer.byte_of_line(buffer.line_of_byte(head));
                                let col = head.saturating_sub(line_start) + 1;
                                format!("Ln {line}, Col {col}")
                            } else {
                                "Ln 1, Col 1".to_string()
                            };
                            ui.label(
                                egui::RichText::new(cursor_str)
                                    .monospace()
                                    .size(11.5)
                                    .color(color(theme.text_muted)),
                            );
                        });
                    });
                }
            });

        egui::Panel::left("vault")
            .frame(
                egui::Frame::new()
                    .fill(color(theme.bg_secondary))
                    .stroke(egui::Stroke::new(1.0, color(theme.border)))
                    .inner_margin(egui::Margin::symmetric(8, 8)),
            )
            .default_size(220.0)
            .min_size(140.0)
            .max_size(600.0)
            .resizable(true)
            .show_separator_line(true)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("VAULT")
                            .size(11.0)
                            .strong()
                            .color(color(theme.text_muted)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button("➕")
                            .on_hover_text("Создать новое хранилище")
                            .clicked()
                        {
                            self.show_create_vault_dialog = true;
                        }
                    });
                });
                ui.add_space(4.0);
                if self.application.vault_path().is_none() {
                    ui.add_space(8.0);
                    if ui.button("➕ Создать хранилище").clicked() {
                        self.show_create_vault_dialog = true;
                    }
                    let config = crate::app::global_config::GlobalConfig::load();
                    if !config.recent_vaults.is_empty() {
                        ui.add_space(8.0);
                        ui.label("Недавние:");
                        for path in &config.recent_vaults {
                            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("vault");
                            if ui
                                .button(format!("📁 {name}"))
                                .on_hover_text(path.display().to_string())
                                .clicked()
                                && let Err(e) = self.application.open_vault(path)
                            {
                                self.application.status = e.to_string();
                            }
                        }
                    }
                } else {
                    let model = self.application.shell.sidebar_model();
                    let count = model.visible_count();
                    let selected = model.selected_index();
                    let mut open = None;
                    let mut toggle_folder = None;
                    egui::ScrollArea::vertical().show_rows(ui, 28.0, count, |ui, rows| {
                        for index in rows {
                            if let Some(row) = model.get_visible_row(index) {
                                let indent = (row.depth as f32) * 14.0;
                                let is_selected = index == selected;
                                let glyph = if row.is_folder {
                                    if row.is_collapsed {
                                        "▸ 📁 "
                                    } else {
                                        "▾ 📁 "
                                    }
                                } else {
                                    "  📄 "
                                };
                                let label_text = format!("{glyph}{}", row.title);
                                let text_color = if is_selected || row.is_folder {
                                    color(theme.text_primary)
                                } else {
                                    color(theme.text_muted)
                                };
                                ui.horizontal(|ui| {
                                    ui.add_space(indent);
                                    let rich = egui::RichText::new(label_text)
                                        .color(text_color)
                                        .size(12.5);
                                    let resp = ui.selectable_label(is_selected, rich);
                                    if resp.clicked() {
                                        if row.is_folder {
                                            toggle_folder = Some(row.id);
                                        } else {
                                            open = row.data.clone();
                                        }
                                    }
                                });
                            }
                        }
                    });
                    if let Some(id) = toggle_folder {
                        self.application
                            .shell
                            .sidebar_model_mut()
                            .toggle_collapse(id);
                    }
                    if let Some(path) = open
                        && let Err(error) = self.application.open_note(&path)
                    {
                        self.application.status = error.to_string();
                    }
                }
            });

        if !self.application.backlinks.is_empty() {
            egui::Panel::right("references")
                .frame(
                    egui::Frame::new()
                        .fill(color(theme.bg_secondary))
                        .stroke(egui::Stroke::new(1.0, color(theme.border)))
                        .inner_margin(egui::Margin::symmetric(8, 8)),
                )
                .default_size(220.0)
                .min_size(140.0)
                .max_size(600.0)
                .resizable(true)
                .show_separator_line(true)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(
                        egui::RichText::new("BACKLINKS")
                            .size(11.0)
                            .strong()
                            .color(color(theme.text_muted)),
                    );
                    ui.add_space(4.0);
                    let mut open = None;
                    egui::ScrollArea::vertical().show_rows(
                        ui,
                        24.0,
                        self.application.backlinks.len(),
                        |ui, rows| {
                            for index in rows {
                                let is_selected = index == self.application.backlink_selection;
                                let rich = egui::RichText::new(
                                    &self.application.backlinks[index].source_path,
                                )
                                .size(13.5)
                                .color(if is_selected {
                                    color(theme.text_primary)
                                } else {
                                    color(theme.text_muted)
                                });
                                if ui.selectable_label(is_selected, rich).clicked() {
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
        }
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

            if self.application.vault_path().is_none()
                && self.application.shell.editor_text().is_empty()
                && self.application.shell.mode() == truff_core::vim::Mode::Normal
            {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.heading("Добро пожаловать в Truff v2");
                    ui.label("Персональная база знаний в формате Markdown с поддержкой Vim и вики-ссылок");
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 12.0;
                        if ui.button("➕  Создать новое хранилище").clicked() {
                            self.show_create_vault_dialog = true;
                        }
                        if ui.button("📂  Ввести путь к хранилищу").clicked() {
                            self.show_create_vault_dialog = true;
                        }
                    });
                    let config = crate::app::global_config::GlobalConfig::load();
                    if !config.recent_vaults.is_empty() {
                        ui.add_space(20.0);
                        ui.separator();
                        ui.add_space(8.0);
                        ui.strong("Сохранённые хранилища:");
                        ui.add_space(8.0);
                        for path in &config.recent_vaults {
                            let name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("vault");
                            let line = format!("📁 {name}  —  {}", path.display());
                            if ui.button(line).clicked()
                                && let Err(e) = self.application.open_vault(path)
                            {
                                self.application.status = e.to_string();
                            }
                        }
                    }
                });
            }
        });
        if self.show_create_vault_dialog {
            let mut close_dialog = false;
            let mut submit_creation = false;
            egui::Window::new("Создать или открыть хранилище")
                .collapsible(false)
                .resizable(false)
                .default_width(450.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(&context, |ui| {
                    ui.heading("Расположение хранилища");
                    ui.label("Укажите путь (поддерживаются ~, ./, ../ и абсолютные пути):");

                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.vault_path_input).desired_width(420.0),
                    );

                    if response.has_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Tab))
                        && let Some(completed) =
                            crate::app::path_completion::tab_complete(&self.vault_path_input)
                    {
                        self.vault_path_input = completed;
                    }
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        submit_creation = true;
                    }

                    let suggestions = crate::app::path_completion::complete_directory_path(
                        &self.vault_path_input,
                    );
                    if !suggestions.is_empty() {
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Подсказки:");
                            for suggestion in suggestions.iter().take(6) {
                                if ui.small_button(suggestion).clicked() {
                                    self.vault_path_input = suggestion.clone();
                                }
                            }
                        });
                    }

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label("Быстрый выбор:");
                        if ui.small_button("~/TruffVault").clicked() {
                            self.vault_path_input = "~/TruffVault".to_string();
                        }
                        if ui.small_button("./notes").clicked() {
                            self.vault_path_input = "./notes".to_string();
                        }
                        if ui.small_button("~/Documents/Notes").clicked() {
                            self.vault_path_input = "~/Documents/Notes".to_string();
                        }
                    });

                    ui.add_space(10.0);
                    ui.checkbox(&mut self.create_starter_note, "Создать стартовую заметку");
                    if self.create_starter_note {
                        ui.horizontal(|ui| {
                            ui.label("Имя файла:");
                            ui.text_edit_singleline(&mut self.initial_note_name);
                        });
                    }

                    if let Some(err) = &self.create_vault_error {
                        ui.add_space(6.0);
                        ui.colored_label(egui::Color32::from_rgb(255, 100, 100), err);
                    }

                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button("✔ Создать и открыть").clicked() {
                            submit_creation = true;
                        }
                        if ui.button("Отмена").clicked() {
                            self.create_vault_error = None;
                            close_dialog = true;
                        }
                    });
                });

            if submit_creation {
                let note = if self.create_starter_note && !self.initial_note_name.trim().is_empty()
                {
                    Some(self.initial_note_name.as_str())
                } else {
                    None
                };
                let path = std::path::Path::new(&self.vault_path_input);
                match self.application.create_and_open_vault(path, note) {
                    Ok(()) => {
                        self.create_vault_error = None;
                        close_dialog = true;
                    }
                    Err(err) => {
                        self.create_vault_error = Some(err.to_string());
                    }
                }
            }

            if close_dialog {
                self.show_create_vault_dialog = false;
            }
        }
        self.show_which_key_overlay(&context);
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

    fn show_which_key_overlay(&mut self, context: &egui::Context) {
        if !self.application.shell.which_key().is_visible() {
            return;
        }

        let theme = self.application.shell.theme().clone();
        let overlay = self.application.shell.which_key().clone();

        egui::Window::new("which_key_overlay")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -28.0])
            .frame(
                egui::Frame::new()
                    .fill(color(theme.bg_secondary))
                    .stroke(egui::Stroke::new(1.0, color(theme.border)))
                    .corner_radius(egui::CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(14, 10)),
            )
            .show(context, |ui| {
                ui.horizontal(|ui| {
                    let title = if overlay.title().is_empty() {
                        "Which-Key"
                    } else {
                        overlay.title()
                    };
                    ui.label(
                        egui::RichText::new(format!("⌨  {title}"))
                            .strong()
                            .color(color(theme.accent)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new("[Esc] закрыть")
                                .color(color(theme.text_muted))
                                .size(11.0),
                        );
                    });
                });
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                match overlay.mode() {
                    crate::overlays::WhichKeyMode::Prefix => {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(12.0, 6.0);
                            for hint in overlay.hints() {
                                ui.horizontal(|ui| {
                                    egui::Frame::new()
                                        .fill(color(theme.bg_tertiary))
                                        .stroke(egui::Stroke::new(1.0, color(theme.border)))
                                        .corner_radius(egui::CornerRadius::same(4))
                                        .inner_margin(egui::Margin::symmetric(6, 2))
                                        .show(ui, |ui| {
                                            ui.label(
                                                egui::RichText::new(hint.key.to_string())
                                                    .monospace()
                                                    .strong()
                                                    .color(color(theme.accent)),
                                            );
                                        });
                                    ui.label(
                                        egui::RichText::new("→").color(color(theme.text_muted)),
                                    );
                                    ui.label(
                                        egui::RichText::new(&hint.description)
                                            .color(color(theme.text_primary)),
                                    );
                                });
                            }
                        });
                    }
                    crate::overlays::WhichKeyMode::Help => {
                        egui::ScrollArea::vertical()
                            .max_height(260.0)
                            .show(ui, |ui| {
                                ui.horizontal_top(|ui| {
                                    ui.spacing_mut().item_spacing.x = 20.0;
                                    for category in overlay.categories() {
                                        ui.vertical(|ui| {
                                            ui.label(
                                                egui::RichText::new(&category.name)
                                                    .strong()
                                                    .color(color(theme.accent)),
                                            );
                                            ui.add_space(4.0);
                                            for hint in &category.hints {
                                                ui.horizontal(|ui| {
                                                    egui::Frame::new()
                                                        .fill(color(theme.bg_tertiary))
                                                        .stroke(egui::Stroke::new(
                                                            1.0,
                                                            color(theme.border),
                                                        ))
                                                        .corner_radius(egui::CornerRadius::same(4))
                                                        .inner_margin(egui::Margin::symmetric(5, 1))
                                                        .show(ui, |ui| {
                                                            ui.label(
                                                                egui::RichText::new(
                                                                    hint.key.to_string(),
                                                                )
                                                                .monospace()
                                                                .size(11.0)
                                                                .color(color(theme.accent)),
                                                            );
                                                        });
                                                    ui.label(
                                                        egui::RichText::new(&hint.description)
                                                            .size(12.0)
                                                            .color(color(theme.text_primary)),
                                                    );
                                                });
                                            }
                                        });
                                    }
                                });
                            });
                    }
                }
            });
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
