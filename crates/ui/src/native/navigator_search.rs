use super::{
    NativeApp,
    search::{SearchKey, SearchTransport},
};
use crate::navigator::{NavigatorItem, NavigatorMode};
use eframe::egui;
use std::{borrow::Cow, sync::Arc};

impl NativeApp {
    pub(super) fn poll_navigator_search(&mut self, context: &egui::Context) {
        if !self.navigator.is_open()
            || !matches!(
                self.navigator.mode,
                NavigatorMode::Content | NavigatorMode::Mixed
            )
        {
            if let Some(search) = &mut self.search {
                search.cancel();
            }
            return;
        }
        let Some(index) = self.application.search_index_path() else {
            self.navigator.items.clear();
            self.navigator.recompute_filter();
            if let Some(search) = &mut self.search {
                search.cancel();
            }
            self.navigator.context_title = "Ожидаем индекс хранилища".into();
            context.request_repaint_after(std::time::Duration::from_millis(50));
            return;
        };
        if self.search.is_none() {
            let context = context.clone();
            match SearchTransport::start(Arc::new(move || context.request_repaint())) {
                Ok(search) => self.search = Some(search),
                Err(error) => {
                    self.application.status = error.to_string();
                    return;
                }
            }
        }
        let key = SearchKey {
            index,
            epoch: self.application.shell.workspace().epoch(),
            mode: self.navigator.mode,
            text: self.navigator.query.clone(),
            page: self.navigator.search_page,
            catalog_revision: self.application.catalog_revision(),
        };
        let search = self.search.as_mut().expect("initialized search transport");
        if search.request(key) {
            self.navigator.items.clear();
            self.navigator.recompute_filter();
        }
        if let Some(reply) = search.poll() {
            match reply.result {
                Ok(page) => {
                    let mut items = Vec::new();
                    if self.navigator.mode == NavigatorMode::Mixed {
                        let mut files = crate::navigator::NavigatorState::new();
                        files.open_mode(
                            NavigatorMode::Files,
                            "",
                            self.application
                                .shell
                                .sidebar_file_paths()
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
                                .collect(),
                        );
                        files.set_query(&self.navigator.query);
                        items.extend(
                            files
                                .filtered_indices
                                .iter()
                                .take(100)
                                .map(|&index| files.items[index].clone()),
                        );
                    }
                    for (index, result) in page.results.into_iter().enumerate() {
                        let mut item = NavigatorItem::new_file(
                            1_000_000 + index as u64,
                            &result.title,
                            result.path,
                            false,
                            0,
                        );
                        item.category = Some(Cow::Borrowed("Содержимое"));
                        item.preview = Some(
                            result
                                .snippet
                                .replace("<b>", "")
                                .replace("</b>", "")
                                .into_boxed_str(),
                        );
                        items.push(item);
                    }
                    self.navigator.results_query = Some(reply.key.text);
                    self.navigator.search_total = page.total_matches;
                    self.navigator.items = items;
                    self.navigator.recompute_filter();
                    self.navigator.context_title = format!(
                        "{} совпадений · страница {}/{} · [ / ]",
                        page.total_matches,
                        self.navigator.search_page + 1,
                        page.total_matches.div_ceil(100).max(1)
                    );
                }
                Err(error) => {
                    self.navigator.context_title = error.to_string();
                }
            }
        }
        if search.busy {
            self.navigator.context_title = "Поиск…".into();
            context.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }
}
