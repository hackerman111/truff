//! UI-P4 integration tests: Floating TOC, Folder Wiki, and Note Stream.
use egui::Event;
use truff_core::vim::Key;
use truff_ui::app::Application;
use truff_ui::native::NativeApp;
use truff_ui::native::stream::{StreamAction, StreamFilter};
use truff_ui::native::toc::{FloatingToc, TocAction, TocItemKind, TocModel, TocScope};
use truff_ui::native::wiki::{FolderWiki, WikiAction, WikiTab};

struct Window {
    app: NativeApp,
    context: egui::Context,
}

impl Window {
    fn event(&mut self, event: Event) {
        self.frame(vec![event]);
    }

    fn redraw(&mut self) {
        self.frame(vec![]);
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200.0, 800.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| self.app.show(ui),
            )
            .drop_without_applying_deltas();
    }
}

fn window(application: Application) -> Window {
    let context = egui::Context::default();
    let app = NativeApp::new(application, &context);
    let mut window = Window { app, context };
    window.redraw();
    window
}

fn named(root: &mut Window, key: egui::Key) {
    root.event(Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
}

fn idle(root: &mut Window) {
    let start = std::time::Instant::now();
    while root.app.application.needs_poll() || root.app.application.file_operation_pending() {
        assert!(
            start.elapsed() < std::time::Duration::from_secs(15),
            "{}",
            root.app.application.status
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
        root.redraw();
    }
}

fn command(root: &mut Window, cmd: &str) {
    let _ = root.app.application.execute_command(cmd);
    root.redraw();
}

// ---------------------------------------------------------------------------
// 1. Floating TOC Unit & Flow Tests (§6.1)
// ---------------------------------------------------------------------------

#[test]
fn test_floating_toc_model_active_file_headings_and_caret_sync() {
    let mut app = Application::new();
    let sample_text = "# Project Truff\n\nIntroduction text.\n\n## Architecture\n\nSome text.\n\n### Core Layer\n\nCore details.\n\n### UI Layer\n\nUI details.\n\n## Conclusion\n\nFinal thoughts.\n";
    app.shell.set_editor_text(sample_text);

    let buffer = app.shell.editor_buffer().unwrap();
    let facts = truff_core::markdown::facts::MarkdownFacts::extract(&buffer.as_str()).unwrap();

    let mut model = TocModel::new();
    model.rebuild_active_file(Some(buffer), Some(&facts), Some("test.md"), 0);

    // Verify all headings are extracted
    assert_eq!(model.all_items.len(), 5);
    assert_eq!(model.all_items[0].title, "Project Truff");
    assert_eq!(model.all_items[0].level, 0); // H1
    assert_eq!(model.all_items[1].title, "Architecture");
    assert_eq!(model.all_items[1].level, 1); // H2
    assert_eq!(model.all_items[2].title, "Core Layer");
    assert_eq!(model.all_items[2].level, 2); // H3
    assert_eq!(model.all_items[3].title, "UI Layer");
    assert_eq!(model.all_items[3].level, 2); // H3
    assert_eq!(model.all_items[4].title, "Conclusion");
    assert_eq!(model.all_items[4].level, 1); // H2

    // Parent hierarchy checks
    assert_eq!(model.all_items[0].parent_idx, None);
    assert_eq!(model.all_items[1].parent_idx, Some(0));
    assert_eq!(model.all_items[2].parent_idx, Some(1));
    assert_eq!(model.all_items[3].parent_idx, Some(1));
    assert_eq!(model.all_items[4].parent_idx, Some(0));

    // Initially at byte 0 -> H1 "Project Truff" is active
    assert_eq!(model.active_item_index, Some(0));
    assert_eq!(model.sticky_breadcrumbs, vec!["Project Truff"]);

    // Move caret into "UI Layer"
    let ui_layer_offset = sample_text.find("UI details").unwrap();
    model.sync_caret(ui_layer_offset, Some(&facts));

    assert_eq!(model.active_item_index, Some(3));
    // Sticky parent breadcrumbs chain: H1 > H2 > H3
    assert_eq!(
        model.sticky_breadcrumbs,
        vec!["Project Truff", "Architecture", "UI Layer"]
    );

    // Move caret into "Conclusion"
    let conclusion_offset = sample_text.find("Final thoughts").unwrap();
    model.sync_caret(conclusion_offset, Some(&facts));
    assert_eq!(model.active_item_index, Some(4));
    assert_eq!(
        model.sticky_breadcrumbs,
        vec!["Project Truff", "Conclusion"]
    );
}

#[test]
fn test_floating_toc_collapse_expand_and_navigation() {
    let mut app = Application::new();
    let sample = "# H1\n## H2A\n### H3\n## H2B\n";
    app.shell.set_editor_text(sample);
    let buf = app.shell.editor_buffer().unwrap();
    let facts = truff_core::markdown::facts::MarkdownFacts::extract(&buf.as_str()).unwrap();

    let mut model = TocModel::new();
    model.rebuild_active_file(Some(buf), Some(&facts), Some("notes.md"), 0);

    assert_eq!(model.visible_indices.len(), 4);
    assert_eq!(model.selected_index, 0);

    // 'j' moves down to H2A (index 1)
    model.move_down(1);
    assert_eq!(model.selected_index, 1);
    assert_eq!(model.selected_item().unwrap().title, "H2A");

    // 'h' collapses H2A (which has child H3)
    model.collapse();
    assert!(model.all_items[1].is_collapsed);
    assert_eq!(model.visible_indices.len(), 3); // H3 is now hidden

    // 'l' expands H2A back
    model.expand();
    assert!(!model.all_items[1].is_collapsed);
    assert_eq!(model.visible_indices.len(), 4);

    // 'toggle_collapse'
    model.toggle_collapse();
    assert!(model.all_items[1].is_collapsed);
    assert_eq!(model.visible_indices.len(), 3);
}

#[test]
fn test_floating_toc_search_filtering() {
    let mut model = TocModel::new();
    model.all_items = vec![
        truff_ui::native::toc::TocItem::new_heading(1, 1, "Alpha".into(), 1, 0, None),
        truff_ui::native::toc::TocItem::new_heading(2, 2, "Beta Architecture".into(), 5, 20, None),
        truff_ui::native::toc::TocItem::new_heading(3, 2, "Gamma Details".into(), 10, 50, None),
    ];
    model.refresh_visible();
    assert_eq!(model.visible_indices.len(), 3);

    // Set search query
    model.search_query = "arch".into();
    model.refresh_visible();
    assert_eq!(model.visible_indices.len(), 1);
    assert_eq!(model.selected_item().unwrap().title, "Beta Architecture");

    model.search_query.clear();
    model.refresh_visible();
    assert_eq!(model.visible_indices.len(), 3);
}

#[test]
fn test_floating_toc_scope_switching_and_hierarchy() {
    let mut model = TocModel::new();
    assert_eq!(model.scope, TocScope::ActiveFile);

    model.scope = model.scope.next();
    assert_eq!(model.scope, TocScope::Folder);

    model.scope = model.scope.next();
    assert_eq!(model.scope, TocScope::Vault);

    let files = vec![
        "projects/backend/api.md".to_string(),
        "projects/backend/db.md".to_string(),
        "projects/frontend/app.md".to_string(),
    ];

    model.rebuild_hierarchy(
        TocScope::Vault,
        &files,
        Some("projects/backend/api.md"),
        None,
        None,
    );
    // Tree contains folders and notes
    assert!(
        model
            .all_items
            .iter()
            .any(|item| matches!(item.kind, TocItemKind::Folder { .. }))
    );
    assert!(
        model
            .all_items
            .iter()
            .any(|item| matches!(item.kind, TocItemKind::Note { .. }))
    );
}

#[test]
fn test_floating_toc_controller_pin_and_keys() {
    let mut toc = FloatingToc::new();
    assert!(!toc.is_open);
    assert!(!toc.is_pinned);

    toc.open();
    assert!(toc.is_open);

    // Key 'p' toggles pin
    let act = toc.feed_key(Key::Char('p'));
    assert_eq!(act, Some(TocAction::TogglePin));
    assert!(toc.is_pinned);

    // Key 'Tab' switches scope
    let act = toc.feed_key(Key::Tab);
    assert_eq!(act, Some(TocAction::SwitchScope(TocScope::Folder)));

    // Key 'Esc' closes unpinned TOC
    toc.set_pinned(false);
    let act = toc.feed_key(Key::Esc);
    assert_eq!(act, Some(TocAction::Close));
    assert!(!toc.is_open);
}

// ---------------------------------------------------------------------------
// 2. Folder Wiki Tests (§6.2)
// ---------------------------------------------------------------------------

#[test]
fn test_folder_wiki_master_note_detection_and_task_aggregation() {
    let tmp = std::env::temp_dir().join(format!("truff-wiki-test-{}", std::process::id()));
    let folder = tmp.join("Engineering");
    std::fs::create_dir_all(&folder).unwrap();

    // Create master note index.md
    std::fs::write(
        folder.join("index.md"),
        "# Engineering Hub\nWelcome to engineering docs.\n",
    )
    .unwrap();

    // Create a note with tasks
    std::fs::write(
        folder.join("sprint.md"),
        "# Current Sprint\n\n- [ ] Implement stage P4\n- [x] Complete stage P3\n- [ ] Write integration tests\n",
    )
    .unwrap();

    let mut app = Application::new();
    app.open_vault(&tmp).unwrap();

    let mut wiki = FolderWiki::new();
    wiki.open_folder("Engineering", &app);

    // Master note detection
    assert!(wiki.data.has_master_file);
    assert_eq!(wiki.data.master_file_name, "index.md");
    assert!(!wiki.data.master_preview.is_empty());
    assert!(wiki.data.master_preview[0].contains("Engineering Hub"));

    // Tasks extraction
    assert_eq!(wiki.data.tasks.len(), 3);
    assert_eq!(wiki.data.tasks[0].text, "Implement stage P4");
    assert!(!wiki.data.tasks[0].checked);
    assert_eq!(wiki.data.tasks[1].text, "Complete stage P3");
    assert!(wiki.data.tasks[1].checked);
    assert_eq!(wiki.data.tasks[2].text, "Write integration tests");
    assert!(!wiki.data.tasks[2].checked);

    // Notes metadata
    assert_eq!(wiki.data.notes.len(), 2);
    let sprint_note = wiki
        .data
        .notes
        .iter()
        .find(|n| n.name == "sprint.md")
        .unwrap();
    assert_eq!(sprint_note.task_count, 3);
    assert_eq!(sprint_note.pending_tasks, 2);

    // Jump from task to note line via Enter on Tasks tab
    wiki.data.active_tab = WikiTab::Tasks;
    wiki.data.selected_index = 0;
    let action = wiki.feed_key(Key::Enter);
    assert_eq!(
        action,
        Some(WikiAction::OpenNote {
            path: "Engineering/sprint.md".to_string(),
            line: Some(wiki.data.tasks[0].line),
        })
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_folder_wiki_tabs_and_controller_navigation() {
    let mut wiki = FolderWiki::new();
    wiki.data.folder_path = "Projects".into();
    wiki.is_open = true;

    assert_eq!(wiki.data.active_tab, WikiTab::Overview);

    // Tab key switches to next tab
    wiki.feed_key(Key::Tab);
    assert_eq!(wiki.data.active_tab, WikiTab::NotesTree);

    wiki.feed_key(Key::Tab);
    assert_eq!(wiki.data.active_tab, WikiTab::Tasks);

    wiki.feed_key(Key::Tab);
    assert_eq!(wiki.data.active_tab, WikiTab::MetadataTable);

    wiki.feed_key(Key::Tab);
    assert_eq!(wiki.data.active_tab, WikiTab::Backlinks);

    // Direct number keys
    wiki.feed_key(Key::Char('1'));
    assert_eq!(wiki.data.active_tab, WikiTab::Overview);

    wiki.feed_key(Key::Char('3'));
    assert_eq!(wiki.data.active_tab, WikiTab::Tasks);

    // Key 'a' creates a new note in folder
    let action = wiki.feed_key(Key::Char('a'));
    assert_eq!(
        action,
        Some(WikiAction::CreateNewNote {
            folder: "Projects".into()
        })
    );

    // Key 'Esc' closes wiki
    let action = wiki.feed_key(Key::Esc);
    assert_eq!(action, Some(WikiAction::Close));
    assert!(!wiki.is_open);
}

// ---------------------------------------------------------------------------
// 3. Note Stream Tests (§6.3)
// ---------------------------------------------------------------------------

#[test]
fn test_note_stream_virtualized_timeline_with_1000_notes() {
    let mut stream = truff_ui::native::stream::NoteStream::new();
    stream.is_open = true;

    // Populate model with 1000 virtual note cards without text concatenation
    for i in 0..1000 {
        let has_unfinished = i % 5 == 0;
        let is_revisit = i > 800;
        let is_today = i < 50;

        let badge = if has_unfinished {
            "2 незавершённых задач".to_string()
        } else if is_revisit {
            format!("Не открывалась {} дн.", i)
        } else if is_today {
            "Сегодня".to_string()
        } else {
            "Изменена недавно".to_string()
        };

        let date_str = if is_today {
            "Сегодня".to_string()
        } else {
            format!("2026-05-{:02}", (i % 28) + 1)
        };

        stream
            .model
            .all_cards
            .push(truff_ui::native::stream::StreamCard {
                id: i as u64,
                path: format!("journal/note_{:04}.md", i),
                title: format!("Daily Note #{:04}", i),
                date_str,
                reason_badge: badge,
                preview_snippet: vec![
                    format!("Summary of work session #{}.", i),
                    "- [ ] Review pull request".to_string(),
                ],
                line_count: 42,
                task_count: if has_unfinished { 2 } else { 0 },
                pending_tasks: if has_unfinished { 2 } else { 0 },
            });
    }

    assert_eq!(stream.model.all_cards.len(), 1000);

    // Test filter: All
    stream.model.filter = StreamFilter::All;
    assert_eq!(stream.model.filtered_cards().len(), 1000);

    // Test filter: Today
    stream.model.filter = StreamFilter::Today;
    let today_cards = stream.model.filtered_cards();
    assert_eq!(today_cards.len(), 50);

    // Test filter: Unfinished
    stream.model.filter = StreamFilter::Unfinished;
    let unfinished_cards = stream.model.filtered_cards();
    assert_eq!(unfinished_cards.len(), 200);

    // Test filter: Revisit
    stream.model.filter = StreamFilter::Revisit;
    let revisit_cards = stream.model.filtered_cards();
    // Cards from 801 to 999 not having unfinished tasks
    assert!(!revisit_cards.is_empty());

    // Test navigation: j/k, clamp
    stream.model.filter = StreamFilter::All;
    stream.model.selected_index = 0;
    stream.feed_key(Key::Char('j'));
    assert_eq!(stream.model.selected_index, 1);
    stream.model.move_down(1000);
    assert_eq!(stream.model.selected_index, 999);
    stream.model.move_up(1000);
    assert_eq!(stream.model.selected_index, 0);

    // Card selection to open via Enter
    let act = stream.feed_key(Key::Enter);
    assert_eq!(
        act,
        Some(StreamAction::OpenNote {
            path: "journal/note_0000.md".to_string()
        })
    );
}

#[test]
fn test_note_stream_controller_and_actions() {
    let mut stream = truff_ui::native::stream::NoteStream::new();
    stream.is_open = true;

    // Filter switching via Tab
    stream.feed_key(Key::Tab);
    assert_eq!(stream.model.filter, StreamFilter::Unfinished);

    stream.feed_key(Key::Tab);
    assert_eq!(stream.model.filter, StreamFilter::Related);

    stream.feed_key(Key::Tab);
    assert_eq!(stream.model.filter, StreamFilter::Revisit);

    // Direct filter numbers
    stream.feed_key(Key::Char('5'));
    assert_eq!(stream.model.filter, StreamFilter::All);

    stream.feed_key(Key::Char('1'));
    assert_eq!(stream.model.filter, StreamFilter::Today);

    // Create note action 'a'
    let act = stream.feed_key(Key::Char('a'));
    assert_eq!(act, Some(StreamAction::CreateNewNote));

    // Close action 'Esc'
    let act = stream.feed_key(Key::Esc);
    assert_eq!(act, Some(StreamAction::Close));
    assert!(!stream.is_open);
}

// ---------------------------------------------------------------------------
// 4. End-to-End Headless NativeApp Integration Tests
// ---------------------------------------------------------------------------

#[test]
fn test_gui_p4_commands_and_overlay_dispatch() {
    let tmp = std::env::temp_dir().join(format!("truff-gui-p4-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(
        tmp.join("readme.md"),
        "# Truff UI P4\n\nTesting P4 flows.\n",
    )
    .unwrap();

    let mut application = Application::new();
    application.open_vault(&tmp).unwrap();

    let mut win = window(application);
    idle(&mut win);

    // 1. Execute :toc command
    command(&mut win, "toc");
    assert!(win.app.toc.is_open);
    assert!(win.app.editor.is_input_blocked());

    named(&mut win, egui::Key::Escape);
    win.redraw();
    assert!(!win.app.toc.is_open);

    // 2. Execute :toc pin
    command(&mut win, "toc pin");
    assert!(win.app.toc.is_open);
    assert!(win.app.toc.is_pinned);

    // 3. Execute :wiki command
    command(&mut win, "wiki");
    assert!(win.app.wiki.is_open);

    named(&mut win, egui::Key::Escape);
    win.redraw();
    assert!(!win.app.wiki.is_open);

    // 4. Execute :stream command
    command(&mut win, "stream");
    assert!(win.app.stream.is_open);
    assert_eq!(win.app.stream.model.filter, StreamFilter::Today);

    // Execute :stream unfinished
    command(&mut win, "stream unfinished");
    assert!(win.app.stream.is_open);
    assert_eq!(win.app.stream.model.filter, StreamFilter::Unfinished);

    named(&mut win, egui::Key::Escape);
    win.redraw();
    assert!(!win.app.stream.is_open);

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_gui_advanced_features_workflows() {
    let tmp = std::env::temp_dir().join(format!("truff-gui-adv-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(
        tmp.join("task_board.kanban.md"),
        "# Board\n\n## Бэклог\n- [ ] Первая задача #todo\n- [x] Сделано #done\n\n## В процессе\n- [ ] Вторая задача #work <!-- color:crimson -->\n",
    )
    .unwrap();
    std::fs::write(
        tmp.join("notes.md"),
        "# Заметка\n\nТекст с тегом #zettelkasten и ссылкой [[task_board]].\n",
    )
    .unwrap();

    let mut application = Application::new();
    application.open_vault(&tmp).unwrap();

    let mut win = window(application);
    idle(&mut win);

    // 1. Test :graph command
    command(&mut win, "graph");
    assert!(win.app.application.graph_view_open);
    assert!(win.app.editor.is_input_blocked());

    named(&mut win, egui::Key::Escape);
    win.redraw();
    assert!(!win.app.application.graph_view_open);

    // 2. Test :database command
    command(&mut win, "database");
    assert!(win.app.application.database_view_open);
    assert!(win.app.editor.is_input_blocked());

    named(&mut win, egui::Key::Escape);
    win.redraw();
    assert!(!win.app.application.database_view_open);

    // 3. Test :tags command (opens right dock on tab 2)
    command(&mut win, "tags");
    assert!(win.app.application.right_dock_visible);
    win.redraw();

    // 4. Test opening a Kanban note and interacting via Vim motions
    command(&mut win, "open task_board.kanban.md");
    idle(&mut win);
    win.redraw();
    assert!(win.app.application.is_active_note_kanban());
    assert!(win.app.editor.is_input_blocked());

    // Vim motions: 'l' moves column right, 'h' moves column left
    win.app
        .kanban
        .feed_key(Key::Char('l'), &mut win.app.application);
    assert_eq!(win.app.kanban.selected_col, 1);
    win.app
        .kanban
        .feed_key(Key::Char('h'), &mut win.app.application);
    assert_eq!(win.app.kanban.selected_col, 0);

    // Vim motion: 'x' toggles checkbox of selected card
    win.app
        .kanban
        .feed_key(Key::Char('x'), &mut win.app.application);
    idle(&mut win);
    let buf_text = win.app.application.shell.editor_buffer().unwrap().as_str();
    assert!(buf_text.contains("- [x] Первая задача"));

    // Vim motion: 'H' / 'L' moves card across columns
    win.app
        .kanban
        .feed_key(Key::Char('L'), &mut win.app.application);
    idle(&mut win);
    assert_eq!(win.app.kanban.selected_col, 1);

    // Toggle back to markdown mode via :kanban
    command(&mut win, "kanban");
    assert!(!win.app.application.is_active_note_kanban());

    // 5. Test :template command
    command(&mut win, "open notes.md");
    idle(&mut win);
    command(&mut win, "template zk");
    idle(&mut win);
    let updated_notes = win.app.application.shell.editor_buffer().unwrap().as_str();
    assert!(updated_notes.contains("#permanent"));

    // 6. Test :wiki-sync command
    command(&mut win, "wiki-sync");
    idle(&mut win);
    let index_file = tmp.join("index.md");
    assert!(index_file.exists());
    let index_text = std::fs::read_to_string(&index_file).unwrap();
    assert!(index_text.contains("<!-- truff:wiki:start -->"));
    assert!(index_text.contains("<!-- truff:wiki:end -->"));

    let _ = std::fs::remove_dir_all(&tmp);
}
