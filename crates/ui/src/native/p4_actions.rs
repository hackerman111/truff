use super::NativeApp;
use super::stream::StreamAction;
use super::toc::{TocAction, TocScope};
use super::wiki::WikiAction;

impl NativeApp {
    pub fn sync_toc(&mut self) {
        let active_path = self.application.active_relative_path();
        let facts = self.application.document_facts();
        let buffer = self.application.shell.editor_buffer();

        match self.toc.model.scope {
            TocScope::ActiveFile => {
                let caret_byte = self
                    .application
                    .shell
                    .editor_view()
                    .map(|v| v.head().get())
                    .unwrap_or(0);
                self.toc.model.rebuild_active_file(
                    buffer,
                    facts.as_deref(),
                    active_path.as_deref(),
                    caret_byte,
                );
            }
            TocScope::Folder | TocScope::Vault => {
                let files: Vec<String> = self
                    .application
                    .shell
                    .sidebar_file_paths()
                    .map(str::to_owned)
                    .collect();
                self.toc.model.rebuild_hierarchy(
                    self.toc.model.scope,
                    &files,
                    active_path.as_deref(),
                    buffer,
                    facts.as_deref(),
                );
            }
        }
    }

    pub fn handle_toc_action(&mut self, action: Option<TocAction>) {
        let Some(action) = action else { return };
        match action {
            TocAction::JumpHeading { line, path } => {
                if let Some(path) = path
                    && self.application.active_relative_path().as_deref() != Some(&path)
                    && let Err(e) = self.application.open_note(&path)
                {
                    self.application.status = e.to_string();
                    return;
                }
                self.application.jump_to_line(line);
                self.application
                    .shell
                    .set_focus(crate::focus::Focus::Editor);
            }
            TocAction::OpenFile { path } => {
                if let Err(e) = self.application.open_note(&path) {
                    self.application.status = e.to_string();
                }
                self.application
                    .shell
                    .set_focus(crate::focus::Focus::Editor);
            }
            TocAction::OpenAdjacent { line, path } => {
                if let Some(path) = path
                    && let Err(e) = self.application.open_note(&path)
                {
                    self.application.status = e.to_string();
                    return;
                }
                if let Some(line) = line {
                    self.application.jump_to_line(line);
                }
                self.application
                    .shell
                    .set_focus(crate::focus::Focus::Editor);
            }
            TocAction::TogglePin => {
                // Pin state is updated within FloatingToc
            }
            TocAction::Close => {
                self.toc.close();
                self.application
                    .shell
                    .set_focus(crate::focus::Focus::Editor);
            }
            TocAction::SwitchScope(_) => {
                self.sync_toc();
            }
        }
    }

    pub fn handle_wiki_action(&mut self, action: Option<WikiAction>) {
        let Some(action) = action else { return };
        match action {
            WikiAction::OpenNote { path, line } => {
                self.wiki.close();
                if let Err(e) = self.application.open_note(&path) {
                    self.application.status = e.to_string();
                    return;
                }
                if let Some(line) = line {
                    self.application.jump_to_line(line);
                }
                self.application
                    .shell
                    .set_focus(crate::focus::Focus::Editor);
            }
            WikiAction::CreateMasterNote { folder } => {
                let path = if folder.is_empty() {
                    "index.md".to_string()
                } else {
                    format!("{folder}/index.md")
                };
                if let Err(e) = self.application.create_note(&path) {
                    self.application.status = e.to_string();
                } else {
                    self.wiki.open_folder(&folder, &self.application);
                }
            }
            WikiAction::CreateNewNote { folder } => {
                self.dialogs.file.open = true;
                self.dialogs.file.path = if folder.is_empty() {
                    String::new()
                } else {
                    format!("{folder}/")
                };
                self.dialogs.file.error = None;
            }
            WikiAction::Close => {
                self.wiki.close();
                self.application
                    .shell
                    .set_focus(crate::focus::Focus::Editor);
            }
        }
    }

    pub fn handle_stream_action(&mut self, action: Option<StreamAction>) {
        let Some(action) = action else { return };
        match action {
            StreamAction::OpenNote { path } => {
                self.stream.close();
                if let Err(e) = self.application.open_note(&path) {
                    self.application.status = e.to_string();
                }
                self.application
                    .shell
                    .set_focus(crate::focus::Focus::Editor);
            }
            StreamAction::CreateNewNote => {
                let now = std::time::SystemTime::now();
                let dur = now
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let filename = format!("Stream-{}.md", dur);
                if let Err(e) = self.application.create_note(&filename) {
                    self.application.status = e.to_string();
                } else {
                    self.stream.open(&self.application);
                }
            }
            StreamAction::Close => {
                self.stream.close();
                self.application
                    .shell
                    .set_focus(crate::focus::Focus::Editor);
            }
            StreamAction::SwitchFilter(_) => {
                // Model state is already updated
            }
        }
    }
}
