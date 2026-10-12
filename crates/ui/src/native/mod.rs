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
    pub navigator: crate::navigator::NavigatorState,
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
            navigator: crate::navigator::NavigatorState::default(),
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
        if self.application.settings_requested {
            self.application.settings_requested = false;
            self.dialogs.settings.open = true;
            let config = crate::app::global_config::GlobalConfig::load();
            self.dialogs
                .settings
                .init_from_app(self.application.shell.typography(), &config.theme);
        }
        self.dialogs.show(&mut self.application, &context, &theme);
        let nav_action = crate::navigator::show(&mut self.navigator, &context, &theme);
        self.handle_navigator_action(nav_action);
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

    pub fn open_navigator(&mut self, mode: crate::navigator::NavigatorMode) {
        use crate::navigator::{NavigatorItem, NavigatorMode};

        self.application
            .shell
            .push_focus(crate::focus::Focus::Navigator);

        let (context_title, items) = match mode {
            NavigatorMode::Files => {
                let vault_name = self
                    .application
                    .vault_path()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("Хранилище");
                let title = format!("Vault / {vault_name}");

                let mut items = Vec::new();
                let files = self.application.shell.sidebar_files();
                for (idx, path_str) in files.iter().enumerate() {
                    let path = std::path::Path::new(path_str);
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(path_str);
                    items.push(NavigatorItem::new_file(
                        idx as u64 + 1,
                        name.to_string(),
                        path_str.clone(),
                        false,
                        0,
                    ));
                }
                (title, items)
            }
            NavigatorMode::Sessions => {
                let title = "Сессии и вкладки".to_string();
                let mut items = Vec::new();
                let mut views: Vec<_> = self
                    .application
                    .shell
                    .workspace()
                    .views()
                    .values()
                    .collect();
                views.sort_by_key(|v| v.id().get());

                for (idx, view) in views.iter().enumerate() {
                    let buffer_id = view.buffer_id();
                    let entry = self.application.shell.workspace().buffers().get(buffer_id);
                    let buf_path = entry
                        .and_then(|e| e.path())
                        .map(|p| p.display().to_string());
                    let buf_title = entry
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
                    let line_count = self
                        .application
                        .shell
                        .workspace()
                        .buffers()
                        .get_buffer(buffer_id)
                        .map_or(0, |b| b.line_len());

                    items.push(NavigatorItem::new_buffer(
                        idx as u64 + 1,
                        buffer_id,
                        buf_title,
                        buf_path,
                        is_dirty,
                        line_count,
                    ));
                }
                (title, items)
            }
            NavigatorMode::Commands => {
                let title = "Палитра команд".to_string();
                let mut items = Vec::new();
                let registry = truff_core::commands::CommandRegistry::default();
                for (idx, spec) in registry.specs().iter().enumerate() {
                    items.push(NavigatorItem::new_command(
                        idx as u64 + 1,
                        spec.id.clone(),
                        spec.title.to_string(),
                        spec.description.to_string(),
                        spec.group.to_string(),
                        spec.default_key.as_ref().map(|k| k.to_string()),
                    ));
                }
                (title, items)
            }
            NavigatorMode::Outline => {
                let note_name = self
                    .application
                    .active_relative_path()
                    .unwrap_or_else(|| "Документ".to_string());
                let title = format!("Outline / {note_name}");
                let mut items = Vec::new();
                if let Some(buf) = self.application.shell.editor_buffer() {
                    let text = buf.as_str();
                    for (line_idx, line) in text.lines().enumerate() {
                        let trimmed = line.trim_start();
                        if trimmed.starts_with('#') {
                            let level = trimmed.chars().take_while(|&c| c == '#').count();
                            let heading_text = trimmed[level..].trim().to_string();
                            if !heading_text.is_empty() && level <= 6 {
                                items.push(NavigatorItem::new_heading(
                                    line_idx as u64 + 1,
                                    line_idx + 1,
                                    level,
                                    heading_text,
                                ));
                            }
                        }
                    }
                }
                (title, items)
            }
            NavigatorMode::Links => {
                let title = "Ссылки и связи (Backlinks)".to_string();
                let mut items = Vec::new();
                for (idx, bl) in self.application.backlinks.iter().enumerate() {
                    items.push(NavigatorItem::new_file(
                        idx as u64 + 1,
                        bl.source_path.clone(),
                        bl.source_path.clone(),
                        false,
                        0,
                    ));
                }
                (title, items)
            }
            NavigatorMode::Boards => {
                let title = "Kanban доски".to_string();
                let mut items = Vec::new();
                let files = self.application.shell.sidebar_files();
                for (idx, f) in files
                    .iter()
                    .filter(|f| f.ends_with(".kanban.md") || f.contains("board"))
                    .enumerate()
                {
                    items.push(NavigatorItem::new_file(
                        idx as u64 + 1,
                        f.clone(),
                        f.clone(),
                        false,
                        0,
                    ));
                }
                (title, items)
            }
        };

        self.navigator.open_mode(mode, context_title, items);
    }

    pub fn handle_navigator_action(&mut self, action: crate::navigator::NavigatorAction) {
        use crate::navigator::NavigatorAction;
        match action {
            NavigatorAction::None => {}
            NavigatorAction::Close => {
                self.navigator.close();
                self.application.shell.pop_focus();
            }
            NavigatorAction::OpenFile { path } => {
                self.navigator.close();
                self.application.shell.pop_focus();
                if let Err(e) = self.application.open_note(&path) {
                    self.application.status = e.to_string();
                }
            }
            NavigatorAction::CreateFile { parent_dir } => {
                self.navigator.close();
                self.application.shell.pop_focus();
                self.dialogs.file.open = true;
                self.dialogs.file.path = parent_dir.map_or(String::new(), |d| format!("{d}/"));
                self.dialogs.file.error = None;
            }
            NavigatorAction::CreateFolder { parent_dir } => {
                self.navigator.close();
                self.application.shell.pop_focus();
                self.dialogs.folder.open = true;
                self.dialogs.folder.path = parent_dir.map_or(String::new(), |d| format!("{d}/"));
                self.dialogs.folder.error = None;
            }
            NavigatorAction::RenameFile { path } => {
                self.navigator.close();
                self.application.shell.pop_focus();
                self.application.status = format!("Переименование: {path}");
            }
            NavigatorAction::DeleteFile { path } => {
                self.navigator.close();
                self.application.shell.pop_focus();
                self.application
                    .handle_shell_action(crate::shell::ShellAction::SidebarDeleteFile(path));
            }
            NavigatorAction::SwitchBuffer { buffer_id } => {
                self.navigator.close();
                self.application.shell.pop_focus();
                if let Some(view) = self
                    .application
                    .shell
                    .workspace()
                    .views()
                    .values()
                    .find(|v| v.buffer_id() == buffer_id)
                {
                    let view_id = view.id();
                    self.application
                        .shell
                        .workspace_mut()
                        .set_active_view(Some(view_id));
                    self.application.shell.reset_editor_input();
                }
            }
            NavigatorAction::CloseBuffer { buffer_id } => {
                if let Some(view) = self
                    .application
                    .shell
                    .workspace()
                    .views()
                    .values()
                    .find(|v| v.buffer_id() == buffer_id)
                {
                    let view_id = view.id();
                    let _ = self
                        .application
                        .dispatch(truff_core::commands::Action::CloseView(view_id));
                    if self.navigator.is_open()
                        && self.navigator.mode == crate::navigator::NavigatorMode::Sessions
                    {
                        self.open_navigator(crate::navigator::NavigatorMode::Sessions);
                    }
                }
            }
            NavigatorAction::ExecuteCommand { command_id } => {
                self.navigator.close();
                self.application.shell.pop_focus();
                if let Err(e) = self.application.execute_command(command_id.as_str()) {
                    self.application.status = e.to_string();
                }
            }
            NavigatorAction::JumpHeading { line } => {
                self.navigator.close();
                self.application.shell.pop_focus();
                if let Some(view_id) = self.application.shell.workspace().active_view()
                    && let Some((buffer, view)) = self
                        .application
                        .shell
                        .workspace_mut()
                        .editor_parts_mut(view_id)
                {
                    let line_idx = line.saturating_sub(1);
                    if line_idx < buffer.line_len() {
                        let byte_offset = buffer.byte_of_line(line_idx);
                        let byte_pos = truff_core::text::BytePos::new(byte_offset);
                        view.set_cursor(byte_pos, byte_pos);
                    }
                }
            }
            NavigatorAction::OpenLink { target } => {
                self.navigator.close();
                self.application.shell.pop_focus();
                if let Err(e) = self.application.open_note(&target) {
                    self.application.status = e.to_string();
                }
            }
            NavigatorAction::SelectCustom { id, data } => {
                self.navigator.close();
                self.application.shell.pop_focus();
                self.application.status = format!("{id}: {data}");
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
