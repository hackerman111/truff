# Очистка корзины, удаление несохранённых файлов и улучшения Kanban Implementation Plan

> **For Antigravity:** REQUIRED SUB-SKILL: Load executing-plans to implement this plan task-by-task.

**Goal:** Реализовать возможность очистки корзины (`.truff/trash`), удаление несохранённых файлов через меню/корзину, исправление вызова Which-Key меню на пробел в Kanban, устранение затемнения самой панели Kanban при вызове модального оверлея, добавление возможности определять расположение файла доски в Kanban-панели и переименование стандартных (и любых) колонок Kanban.

**Architecture:** 
1. **Корзина и файлы:** Добавить операцию очистки содержимого директории корзины (`vault.trash_dir()`), зарегистрировать команду и кнопку в интерфейсе с подтверждением, а также снять блокировку `DirtyBufferBlocked` при перемещении в корзину для закрытия несохранённых буферов без ошибок.
2. **Space / Which-Key в Kanban:** В `KanbanBoardState` перестать перехватывать `Space` в обычном режиме для смены статуса (сохранив `x` / `X`), позволяя событию пробела подниматься до уровня Shell и открывать глобальное Which-Key меню действий.
3. **Модальный оверлей Kanban:** Заменить ручной `layer_painter` и `Window` на `egui::Modal`, гарантирующий, что фон затемняется под модальной панелью, а сама панель остаётся яркой и четкой.
4. **Расположение файла Kanban:** Добавить в заголовок оверлея отображение текущего пути файла доски, возможность редактировать/перемещать файл доски, а при создании новой доски — запрашивать путь к файлу вместо жёстко зашитого `new_board.kanban.md`.
5. **Переименование колонок:** Сделать заголовки колонок интерактивными (клик/дабл-клик по названию, кнопка редактирования `Icon::Edit` и контекстное меню), открывающими режим ввода нового названия для стандартных («План», «В работе», «Готово») и пользовательских колонок.

**Tech Stack:** Rust, egui 0.36.2, truff-core, truff-store, truff-ui.

---

### Task 1: Снятие блокировки на удаление несохранённых файлов в корзину
**Files:**
- `crates/store/src/operations/plan.rs`
- `crates/ui/src/native/creation/operation.rs`
- `crates/store/tests/operations.rs`

**Details:**
1. В `crates/store/src/operations/plan.rs:153`: для `FileOpKind::Trash` убрать проверку `if dirty_buffers.iter().any(|d| d == path) { return Err(FileOpError::DirtyBufferBlocked(path.clone())); }`.
2. В `crates/ui/src/native/creation/operation.rs:121`: скорректировать подпись в диалоге подтверждения корзины — убрать фразу «Несохранённые изменения блокируют операцию.» и заменить на сообщение о том, что файл будет перемещён в корзину, а несохранённые изменения будут сброшены.
3. Написать тест в `crates/store/tests/operations.rs`, проверяющий, что план `FileOpPlan::trash` успешно валидируется даже при наличии файла в `dirty_buffers`.

---

### Task 2: Реализация функционала очистки корзины (`empty_trash`)
**Files:**
- `crates/ui/src/app/commands/files.rs`
- `crates/ui/src/app/commands/mod.rs`
- `crates/core/src/commands/defaults.rs`
- `crates/ui/src/native/creation/operation.rs`
- `crates/ui/src/native/sidebar.rs`
- `crates/ui/tests/trash.rs` (новый тест или дополнение существующих)

**Details:**
1. В `crates/ui/src/app/commands/files.rs`: добавить метод `pub(crate) fn empty_trash(&mut self) -> AppResult<usize>`, который считывает файлы из `vault.trash_dir()`, удаляет их рекурсивно и возвращает количество удалённых элементов. Обновляет `self.status = "Корзина очищена".to_string()`.
2. В `crates/core/src/commands/defaults.rs`: зарегистрировать команду `trash.empty` («Очистить корзину», «Безвозвратно удалить все файлы и папки из корзины хранилища», контекст `[CommandContext::Global]`).
3. В `crates/ui/src/app/commands/mod.rs`: поддержать вызов `:empty_trash` и `:clear_trash`.
4. В `crates/ui/src/native/creation/operation.rs`: добавить вариант `Operation::EmptyTrash`, отображающий диалог подтверждения очистки корзины.
5. В `crates/ui/src/native/sidebar.rs`: в заголовке хранилища (рядом с кнопками настроек и создания) добавить кнопку с иконкой `Icon::Trash` и тултипом «Очистить корзину», вызывающую диалог `dialogs.operation.empty_trash()`.
6. Написать тест, создающий файл, удаляющий его в корзину и затем очищающий корзину.

---

### Task 3: Исправление вызова Which-Key меню на `Space` в Kanban
**Files:**
- `crates/ui/src/native/kanban/state.rs`
- `crates/ui/src/native/kanban/render.rs`
- `crates/ui/src/native/events/mod.rs`
- `crates/ui/tests/kanban_workflows.rs`

**Details:**
1. В `crates/ui/src/native/kanban/state.rs`: убрать `Key::Char(' ')` из перехвата смены статуса задачи в `KanbanInputMode::Normal`. Статус карточки циклически переключается клавишами `x` и `X` (а также кликом мыши).
2. В `crates/ui/src/native/kanban/render.rs`: обновить подсказку в футере с `("Space", "статус")` на `("x", "статус")`.
3. В `crates/ui/src/native/events/mod.rs`: убедиться, что при нажатии `Key::Char(' ')` в центральном Kanban-виде или оверлее (когда не происходит текстовый ввод) событие пробела не поглощается канбаном, а открывает Which-Key меню (`<Space> Навигатор`). В случае модального оверлея нажатие `Space` закрывает оверлей и активирует Which-Key меню.
4. Добавить интеграционный тест в `crates/ui/tests/kanban_workflows.rs`, подтверждающий, что в центральном режиме Kanban при нажатии `Space` открывается Which-Key меню и не происходит ложного переключения статуса карточки.

---

### Task 4: Устранение затемнения самой панели Kanban при вызове оверлея
**Files:**
- `crates/ui/src/native/kanban/render.rs`
- `crates/ui/src/native/kanban/controller.rs`

**Details:**
1. В `crates/ui/src/native/kanban/render.rs`: в `render_kanban_modal` удалить ручной вызов `ctx.layer_painter(backdrop_layer).rect_filled(...)` на слое `Order::Middle`, который перекрывал окно модальной панели и приводил к её затемнению.
2. Использовать `egui::Modal::new(egui::Id::new("kanban_modal_window")).frame(frame).show(ctx, |ui| { ... })` по аналогии с `universal_navigator` и диалогами создания.
3. Проверить, что фон вокруг модального окна затемняется стандартным полупрозрачным бэкдропом модала, а само содержимое доски рендерится чистыми и яркими цветами выбранной темы.

---

### Task 5: Определение расположения файла Kanban в панели
**Files:**
- `crates/ui/src/native/kanban/state.rs`
- `crates/ui/src/native/kanban/render.rs`
- `crates/ui/src/native/kanban/controller.rs`
- `crates/ui/tests/kanban_workflows.rs`

**Details:**
1. В `crates/ui/src/native/kanban/state.rs`: добавить вариант в `KanbanInputMode`: `BoardPathInput { buffer: String, is_new: bool }`.
2. В `crates/ui/src/native/kanban/state.rs`: добавить `KanbanIntent::SetBoardPath { old_path: Option<String>, new_path: String, is_new: bool }`.
3. В `crates/ui/src/native/kanban/render.rs`:
   - В шапке модального оверлея добавить строку отображения текущего пути файла доски (например, чип с иконкой `Icon::Folder` / `Icon::File` и надписью `Файл: <path>`) с кнопкой «Изменить расположение».
   - Кнопку `+` («Создать доску») перевести в режим ввода пути: открывается `BoardPathInput` с предзаполненным значением (например, `kanban_2.kanban.md`).
   - Отображать поле ввода пути с подсказкой и кнопками подтверждения/отмены.
4. В `crates/ui/src/native/kanban/controller.rs`: поддержать `KanbanIntent::SetBoardPath`:
   - Если `is_new`: создать файл доски по указанному относительному пути в хранилище (создавая родительские папки при необходимости), инициализировать шаблон и переключиться на новую доску.
   - Если перемещение существующей доски: переименовать/переместить файл на диске, обновить буфер и сделать его активным.
5. Написать тест в `crates/ui/tests/kanban_workflows.rs`, проверяющий создание доски в подпапке и смену пути.

---

### Task 6: Переименование стандартных столбцов в Kanban
**Files:**
- `crates/ui/src/native/kanban/render.rs`
- `crates/ui/src/native/kanban/state.rs`
- `crates/ui/tests/kanban_workflows.rs`

**Details:**
1. В `crates/ui/src/native/kanban/render.rs`:
   - Сделать название колонки кликабельным: клик мышью по заголовку колонки переводит её в режим редактирования `KanbanInputMode::ColumnInput { rename_col_idx: Some(col_idx), buffer: col.title.clone() }`.
   - В блок кнопок управления колонкой (рядом с кнопкой удаления корзины и добавления карточки) добавить кнопку `Icon::Edit` с тултипом «Переименовать колонку (R)».
   - Добавить контекстное меню при правом клике на заголовок колонки с пунктом «Переименовать колонку».
   - Убедиться, что при сохранении (Enter / кнопка «✓») транзакция `plan_rename_column` применяется для любых колонок, включая начальные стандартные («План», «В работе», «Готово»).
2. Написать тест в `crates/ui/tests/kanban_workflows.rs`, проверяющий переименование стандартной колонки «План» в «Бэклог» через `ColumnInput`.
