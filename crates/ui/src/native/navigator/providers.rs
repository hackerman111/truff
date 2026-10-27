use super::super::{NativeApp, outline};

impl NativeApp {
    pub(super) fn navigator_items(
        &mut self,
        mode: crate::navigator::NavigatorMode,
    ) -> (String, Vec<crate::navigator::NavigatorItem>) {
        use crate::navigator::{NavigatorItem, NavigatorMode};
        match mode {
            NavigatorMode::Files | NavigatorMode::Folders => {
                let vault_name = self
                    .application
                    .vault_path()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("Хранилище");
                let title = format!("Vault / {vault_name}");

                if let Some(path) = self.application.active_relative_path() {
                    self.application.shell.select_sidebar_file(&path);
                }
                let all_rows = self.application.shell.sidebar_model().all_items();
                let mut items = Vec::with_capacity(all_rows.len());
                let mut parents: Vec<&str> = Vec::with_capacity(8);
                for row in all_rows {
                    parents.truncate(row.depth);
                    if row.is_folder {
                        parents.push(&row.title);
                    }
                    if mode == NavigatorMode::Folders && !row.is_folder {
                        continue;
                    }
                    let path = row.data.clone().unwrap_or_else(|| parents.join("/"));
                    let mut item = NavigatorItem::new_file(
                        row.id.get(),
                        &row.title,
                        path,
                        row.is_folder,
                        row.depth,
                    );
                    item.is_collapsed = row.is_collapsed;
                    items.push(item);
                }
                (title, items)
            }
            NavigatorMode::Content | NavigatorMode::Mixed => {
                ("Поиск в текущем хранилище".into(), Vec::new())
            }
            NavigatorMode::Vaults => {
                self.recent_vaults.refresh();
                let items = self
                    .recent_vaults
                    .paths()
                    .iter()
                    .enumerate()
                    .map(|(index, path)| {
                        let mut item = NavigatorItem::new_file(
                            index as u64 + 1,
                            path.file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or("Vault"),
                            path.display().to_string(),
                            false,
                            0,
                        );
                        item.category = Some(std::borrow::Cow::Borrowed("Хранилища"));
                        item.payload =
                            crate::navigator::NavigatorItemPayload::Vault { path: path.clone() };
                        item
                    })
                    .collect();
                ("Недавние хранилища".into(), items)
            }
            NavigatorMode::Recent => {
                let items = self
                    .application
                    .recent_notes()
                    .enumerate()
                    .map(|(index, path)| {
                        NavigatorItem::new_file(
                            index as u64 + 1,
                            path.rsplit('/').next().unwrap_or(path),
                            path.to_owned(),
                            false,
                            0,
                        )
                    })
                    .collect();
                ("Заметки текущей сессии".into(), items)
            }
            NavigatorMode::Colors => {
                let mut items = Vec::new();
                let mut id = 1u64;

                // 1. Text colors (foreground)
                for (name, hex) in [
                    ("Красный", "#BF616A"),
                    ("Оранжевый", "#D08770"),
                    ("Жёлтый", "#EBCB8B"),
                    ("Зелёный", "#A3BE8C"),
                    ("Голубой", "#88C0D0"),
                    ("Синий", "#81A1C1"),
                    ("Фиолетовый", "#B48EAD"),
                ] {
                    let mut item = NavigatorItem::new_file(id, name, hex.into(), false, 0);
                    item.shortcut = Some(hex.into());
                    item.category = Some(std::borrow::Cow::Borrowed("Цвет текста"));
                    item.payload = crate::navigator::NavigatorItemPayload::Color { hex };
                    items.push(item);
                    id += 1;
                }

                // 2. Highlights (background <mark>)
                for (name, hex) in [
                    ("Жёлтый (маркер)", "#EBCB8B"),
                    ("Зелёный (маркер)", "#A3BE8C"),
                    ("Голубой (маркер)", "#88C0D0"),
                    ("Оранжевый (маркер)", "#D08770"),
                    ("Красный (маркер)", "#BF616A"),
                    ("Фиолетовый (маркер)", "#B48EAD"),
                ] {
                    let mut item = NavigatorItem::new_file(id, name, hex.into(), false, 0);
                    item.shortcut = Some(hex.into());
                    item.category = Some(std::borrow::Cow::Borrowed("Маркер (Highlight)"));
                    item.payload = crate::navigator::NavigatorItemPayload::Highlight { hex };
                    items.push(item);
                    id += 1;
                }

                // 3. Obsidian Callouts
                for (name, kind) in [
                    ("Callout: Note", truff_core::markdown::CalloutKind::Note),
                    ("Callout: Tip", truff_core::markdown::CalloutKind::Tip),
                    (
                        "Callout: Important",
                        truff_core::markdown::CalloutKind::Important,
                    ),
                    (
                        "Callout: Warning",
                        truff_core::markdown::CalloutKind::Warning,
                    ),
                ] {
                    let mut item = NavigatorItem::new_file(id, name, kind.tag().into(), false, 0);
                    item.category = Some(std::borrow::Cow::Borrowed("Callout (Блок)"));
                    item.payload = crate::navigator::NavigatorItemPayload::Callout { kind };
                    items.push(item);
                    id += 1;
                }

                // 4. Remove style
                let mut clear_item = NavigatorItem::new_file(
                    id,
                    "Очистить оформление (стиль)",
                    "remove_style".into(),
                    false,
                    0,
                );
                clear_item.category = Some(std::borrow::Cow::Borrowed("Очистка"));
                clear_item.payload = crate::navigator::NavigatorItemPayload::RemoveStyle;
                items.push(clear_item);

                ("Палитра стилей и цветов".into(), items)
            }
            NavigatorMode::Tasks => {
                let items = self
                    .application
                    .shell
                    .editor_buffer()
                    .map(|buffer| {
                        let snapshot = truff_core::markdown::MarkdownSnapshot::from_buffer(buffer);
                        snapshot
                            .tasks
                            .into_iter()
                            .enumerate()
                            .map(|(index, task)| {
                                NavigatorItem::new_task(
                                    index as u64 + 1,
                                    task.line_start + 1,
                                    task.checked,
                                    buffer.slice(task.content_range).to_string(),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                ("Задачи текущей заметки".into(), items)
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

                    let mut item = NavigatorItem::new_buffer(
                        idx as u64 + 1,
                        buffer_id,
                        buf_title,
                        buf_path.clone(),
                        is_dirty,
                        line_count,
                    );
                    if let Some(buffer) = entry.and_then(|entry| entry.buffer()) {
                        let end = buffer
                            .snap_to_grapheme_boundary(truff_core::BytePos::new(
                                buffer.byte_len().min(4096),
                            ))
                            .get();
                        item.preview = Some(buffer.slice(0..end).to_string().into_boxed_str());
                    }
                    if self.application.view_is_pinned(view.id()) {
                        item.shortcut = Some("Закреплён".into());
                    }
                    item.payload = crate::navigator::NavigatorItemPayload::View {
                        view_id: view.id(),
                        path: buf_path,
                    };
                    items.push(item);
                }
                (title, items)
            }
            NavigatorMode::Commands => {
                let title = "Палитра команд".to_string();
                let mut items = Vec::new();
                let registry = truff_core::commands::CommandRegistry::default();
                let context = if self.navigator_origin == crate::focus::Focus::Editor {
                    truff_core::commands::CommandContext::Editor
                } else {
                    truff_core::commands::CommandContext::Rows
                };
                let labels = self
                    .application
                    .shell
                    .binding_labels(context, truff_core::vim::Mode::Normal);
                for (idx, spec) in registry.specs().iter().enumerate() {
                    let mut item = NavigatorItem::new_command(
                        idx as u64 + 1,
                        spec.id.clone(),
                        spec.title.to_string(),
                        spec.description.to_string(),
                        spec.group.clone(),
                        labels.get(&spec.id).cloned(),
                    );
                    item.disabled_reason = registry
                        .require(&spec.id, context)
                        .err()
                        .map(|error| error.to_string().into_boxed_str());
                    if matches!(
                        spec.id.as_str(),
                        "style.highlight_color" | "style.wrap_callout" | "style.pane_accent"
                    ) {
                        item.disabled_reason = Some("Команда пока недоступна в интерфейсе".into());
                    }
                    items.push(item);
                }
                (title, items)
            }
            NavigatorMode::Outline => {
                let note_name = self
                    .application
                    .active_relative_path()
                    .unwrap_or_else(|| "Документ".to_string());
                let title = format!("Outline / {note_name}");
                let facts = self.application.document_facts();
                let items = self
                    .application
                    .shell
                    .editor_buffer()
                    .zip(facts.as_deref())
                    .map(|(buffer, facts)| outline::items(buffer, facts))
                    .unwrap_or_default();
                (title, items)
            }
            NavigatorMode::Links => {
                let title = "Ссылки и связи (Backlinks)".to_string();
                let mut items = Vec::new();
                if let Some(facts) = self.application.document_facts() {
                    for (index, link) in facts.links.iter().enumerate() {
                        let target = link.heading.as_ref().map_or_else(
                            || link.target.clone(),
                            |heading| format!("{}#{heading}", link.target),
                        );
                        let mut item = NavigatorItem::new_link(
                            1_000_000 + index as u64,
                            link.label.as_deref().unwrap_or(&target),
                            target.clone(),
                        );
                        if target.starts_with("http://")
                            || target.starts_with("https://")
                            || target.starts_with("mailto:")
                        {
                            item.disabled_reason =
                                Some("Внешние ссылки здесь пока недоступны".into());
                        }
                        items.push(item);
                    }
                }
                for (idx, bl) in self.application.backlinks.iter().enumerate() {
                    let mut item = NavigatorItem::new_file(
                        idx as u64 + 1,
                        &bl.source_path,
                        bl.source_path.clone(),
                        false,
                        0,
                    );
                    item.category = Some(std::borrow::Cow::Borrowed("Обратные"));
                    item.payload = crate::navigator::NavigatorItemPayload::Location {
                        path: bl.source_path.clone(),
                        byte: bl.range.start,
                    };
                    items.push(item);
                }
                (title, items)
            }
            NavigatorMode::Boards => {
                let title = "Файлы .kanban.md".to_string();
                let mut items = Vec::new();
                let files = self.application.shell.sidebar_file_paths();
                for (idx, f) in files.filter(|f| f.ends_with(".kanban.md")).enumerate() {
                    items.push(NavigatorItem::new_file(
                        idx as u64 + 1,
                        f,
                        f.to_owned(),
                        false,
                        0,
                    ));
                }
                (title, items)
            }
        }
    }
}
