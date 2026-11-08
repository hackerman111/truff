mod tabs;

use super::model::{FolderWikiData, WikiTab};
use crate::native::editor::color;
use crate::native::{icons, widgets};
use crate::theme::Theme;

/// Action produced by interacting with the Folder Wiki dashboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WikiAction {
    OpenNote { path: String, line: Option<usize> },
    CreateMasterNote { folder: String },
    CreateNewNote { folder: String },
    Close,
}

/// Renders the Folder Wiki dashboard as a spacious, Obsidian Bases-like overlay.
pub fn render_folder_wiki(
    data: &mut FolderWikiData,
    ctx: &egui::Context,
    theme: &Theme,
) -> Option<WikiAction> {
    let mut action = None;
    let screen = ctx.content_rect();

    // Spacious responsive sizing: 82vw × 80vh
    let width = (screen.width() * 0.82).clamp(580.0, 1280.0);
    let height = (screen.height() * 0.80).clamp(440.0, 860.0);

    widgets::centered_modal_window(ctx, "folder_wiki_dashboard", width, height, theme, |ui| {
        // Top Header: Folder title, Stats pills, New note, Close
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;

            let (folder_rect, _) =
                ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
            icons::render_icon(
                ui.painter(),
                icons::Icon::Folder,
                folder_rect,
                color(theme.accent),
                1.4,
            );

            ui.label(
                egui::RichText::new(&data.folder_name)
                    .size(16.0)
                    .strong()
                    .color(color(theme.text_primary)),
            );

            if !data.folder_path.is_empty() {
                ui.label(
                    egui::RichText::new(format!("({})", data.folder_path))
                        .size(11.0)
                        .color(color(theme.text_muted)),
                );
            }

            let notes_cnt = data.notes.len();
            let pending_cnt = data.pending_tasks();
            let pill_text = format!("{notes_cnt} зам. · {pending_cnt} задач");
            ui.label(
                egui::RichText::new(pill_text)
                    .size(11.0)
                    .color(color(theme.accent)),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, icons::Icon::Close, "Закрыть (Esc)").clicked() {
                    action = Some(WikiAction::Close);
                }
                if widgets::primary_button_with_icon(ui, icons::Icon::Plus, "Новая заметка (a)")
                    .clicked()
                {
                    action = Some(WikiAction::CreateNewNote {
                        folder: data.folder_path.clone(),
                    });
                }
            });
        });

        ui.add_space(8.0);

        // Tab bar + live filter bar
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for tab in WikiTab::ALL {
                let is_active = data.active_tab == tab;
                let text = egui::RichText::new(tab.title())
                    .size(12.0)
                    .color(if is_active {
                        color(theme.accent)
                    } else {
                        color(theme.text_muted)
                    });
                if ui.selectable_label(is_active, text).clicked() {
                    data.active_tab = tab;
                    data.selected_index = 0;
                }
            }
        });

        ui.add_space(6.0);
        if widgets::search_filter_bar(
            ui,
            "wiki_search_input",
            &mut data.search_query,
            Some(&mut data.search_active),
            "Фильтр заметок, разделов и задач (/ для поиска)...",
            theme,
        ) {
            data.clamp_selection();
        }

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(8.0);

        match data.active_tab {
            WikiTab::Overview => tabs::render_overview(data, ui, theme, &mut action),
            WikiTab::NotesTree => tabs::render_notes_tree(data, ui, theme, &mut action),
            WikiTab::Tasks => tabs::render_tasks(data, ui, theme, &mut action),
            WikiTab::MetadataTable => tabs::render_metadata_table(data, ui, theme, &mut action),
            WikiTab::Backlinks => tabs::render_backlinks(data, ui, theme, &mut action),
        }

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(
                    "j/k: выбор  Enter: открыть  h/l: свернуть  /: поиск  Tab: вкладка  a: заметка  Esc: закрыть",
                )
                .size(11.0)
                .color(color(theme.text_muted)),
            );
        });
    });

    action
}
