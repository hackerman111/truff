//! Native egui/glow adapter. Application owns domain state; this module owns events and paint.
mod backlinks;
mod command_bar;
mod content;
pub mod creation;
mod editor;
mod events;
mod file_commands;
mod hints;
pub mod icons;
mod navigator;
mod navigator_search;
mod outline;
mod p4_actions;
mod paint;
mod recent;
mod search;
mod sidebar;
mod status;
pub mod stream;
mod tabs;
pub mod toc;
pub mod widgets;
pub mod wiki;

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
    pub toc: toc::FloatingToc,
    pub wiki: wiki::FolderWiki,
    pub stream: stream::NoteStream,
    hints_tick: Instant,
    last_typography: crate::app::global_config::TypographyConfig,
    last_theme: Option<crate::theme::Theme>,
    recent_vaults: recent::RecentVaults,
    search: Option<search::SearchTransport>,
    navigator_origin: crate::focus::Focus,
    navigator_revision: u64,
    field_event_tail: Vec<egui::Event>,
    field_keys: Vec<(events::FieldOwner, truff_core::vim::Key, bool)>,
    navigator_selection: Option<crate::app::SelectionTarget>,
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
            toc: toc::FloatingToc::new(),
            wiki: wiki::FolderWiki::new(),
            stream: stream::NoteStream::new(),
            last_typography,
            last_theme: None,
            recent_vaults: recent::RecentVaults::default(),
            search: None,
            navigator_selection: None,
            navigator_origin: crate::focus::Focus::Editor,
            navigator_revision: 0,
            field_keys: Vec::new(),
            field_event_tail: Vec::new(),
        }
    }
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        if self.application.shell.typography() != &self.last_typography {
            configure_fonts(&context, self.application.shell.typography());
            self.editor.invalidate_fonts();
            self.last_typography = self.application.shell.typography().clone();
        }
        self.events(&context);
        self.application.poll();
        if self.navigator.is_open()
            && self.navigator_revision != self.application.catalog_revision()
            && !self.dialogs.is_any_open()
            && !matches!(
                self.navigator.mode,
                crate::navigator::NavigatorMode::Content | crate::navigator::NavigatorMode::Mixed
            )
        {
            self.refresh_navigator();
        }
        self.poll_navigator_search(&context);
        let now = Instant::now();
        self.application.shell.which_key_mut().tick(
            now.duration_since(self.hints_tick)
                .as_millis()
                .min(u64::MAX as u128) as u64,
        );
        self.hints_tick = now;
        let theme = self.application.shell.theme().clone();
        if self.last_theme.as_ref() != Some(&theme) {
            theme.apply(ui);
            self.last_theme = Some(theme.clone());
        }
        tabs::show(&mut self.application, ui, &theme);
        self.application.refresh_command_completions();
        status::show(&mut self.application, ui, &theme);
        if self.application.vault_path().is_none() && context.input(|i| i.focused) {
            self.recent_vaults.refresh();
            context.request_repaint_after(recent::RecentVaults::INTERVAL);
        }
        if let Some(arg) = self.application.toc_requested.take() {
            match arg.as_str() {
                "pin" => {
                    self.toc.set_pinned(true);
                    self.toc.open();
                }
                "unpin" => {
                    self.toc.set_pinned(false);
                    self.toc.open();
                }
                "file" => {
                    self.toc.model.scope = toc::TocScope::ActiveFile;
                    self.toc.open();
                }
                "folder" => {
                    self.toc.model.scope = toc::TocScope::Folder;
                    self.toc.open();
                }
                "vault" => {
                    self.toc.model.scope = toc::TocScope::Vault;
                    self.toc.open();
                }
                _ => {
                    self.toc.toggle_open();
                }
            }
            self.sync_toc();
        }
        if let Some(folder_arg) = self.application.wiki_requested.take() {
            let target_folder = if folder_arg.is_empty() {
                self.application
                    .active_relative_path()
                    .and_then(|p| {
                        std::path::Path::new(&p)
                            .parent()
                            .and_then(|d| d.to_str())
                            .map(str::to_owned)
                    })
                    .unwrap_or_default()
            } else {
                folder_arg
            };
            self.wiki.open_folder(&target_folder, &self.application);
        }
        if let Some(stream_arg) = self.application.stream_requested.take() {
            match stream_arg.as_str() {
                "today" => self.stream.model.filter = stream::StreamFilter::Today,
                "unfinished" => self.stream.model.filter = stream::StreamFilter::Unfinished,
                "related" => self.stream.model.filter = stream::StreamFilter::Related,
                "revisit" => self.stream.model.filter = stream::StreamFilter::Revisit,
                "all" => self.stream.model.filter = stream::StreamFilter::All,
                _ => {}
            }
            self.stream.open(&self.application);
        }
        for change in self.application.shell.take_buffer_changes() {
            self.application.notify_buffer_change_for_facts(&change);
            if let Some(buffer) = self
                .application
                .shell
                .workspace()
                .buffers()
                .get_buffer(change.buffer_id)
            {
                self.editor.apply_change(buffer, &change);
            }
        }
        self.sync_toc();

        if !self.application.focus_mode {
            sidebar::show(
                &mut self.application,
                &mut self.dialogs,
                &mut self.recent_vaults,
                ui,
                &theme,
            );
            let dock_toc_action = backlinks::show(&mut self.application, &mut self.toc, ui, &theme);
            self.handle_toc_action(dock_toc_action);
        }
        self.editor.input_blocked = self.application.shell.command().is_some()
            || self.dialogs.is_any_open()
            || self.navigator.is_open()
            || self.wiki.is_open()
            || self.stream.is_open()
            || (self.toc.is_open && !self.toc.is_pinned)
            || (self.application.shell.which_key().is_visible()
                && self.application.shell.which_key().mode()
                    == crate::overlays::WhichKeyMode::Help);
        self.editor.retain_views(self.application.shell.workspace());
        let open_vault = content::show(
            &mut self.application,
            &mut self.editor,
            &mut self.recent_vaults,
            ui,
            &theme,
        );
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
        let nav_action =
            crate::navigator::render::show_without_input(&mut self.navigator, &context, &theme);
        self.handle_navigator_action(nav_action);
        let toc_act = self.toc.show_overlay(&context, &theme);
        self.handle_toc_action(toc_act);
        let wiki_act = self.wiki.show(&context, &theme);
        self.handle_wiki_action(wiki_act);
        let stream_act = self.stream.show(&context, &theme);
        self.handle_stream_action(stream_act);
        self.dialogs.show(&mut self.application, &context, &theme);
        hints::show(&mut self.application.shell, &context);
        self.flush_field_keys(&context);
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
