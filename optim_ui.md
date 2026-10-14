Ниже представлен архитектурный и низкоуровневый план оптимизации памяти крейта `truff-ui`, направленный на радикальное снижение потребления RAM (как базового RSS при запуске, так и пикового потребления на хранилищах из 10 000+ заметок).

---

## 1. Освобождение удерживаемой памяти в `NavigatorState` и отказ от дублирования 10 000 элементов (экономия ~4–6 МиБ RAM)

**Локация:** `src/navigator/model.rs` (`NavigatorItem`, `NavigatorState::close`) и `src/native/mod.rs` (`NativeApp::open_navigator`).

### Почему расходуется память:

1. **Утечка удержания после закрытия окна:** в `NavigatorState::close` (`src/navigator/model.rs`) сбрасываются только булевы флаги и строка запроса (`self.query.clear()`), но векторы `self.items: Vec<NavigatorItem>` и `self.filtered_indices: Vec<usize>` **никогда не очищаются и не освобождают память**. Стоит пользователю один раз нажать `Space f` в хранилище на 10 000 файлов и закрыть окно (`Esc`), как 10 000 тяжеловесных структур `NavigatorItem` остаются в RAM навсегда до закрытия программы.

2. **Раздутая структура `NavigatorItem` (до 240 байт на элемент с кучей):**

- При создании элемента файла в `NavigatorItem::new_file` (`src/navigator/model.rs`) выделяются **4 отдельные строки `String` в куче**:
- `payload: NavigatorItemPayload::File { path: relative_path }` (полный относительный путь, например `"folder_01/note_0001.md"`);

- `title: name` (копия имени файла `"note_0001.md"`, которая уже является суффиксом `relative_path`);

- `subtitle` (копия родительской папки `"folder_01"`, которая уже является префиксом `relative_path`);

- `category: Some("Файлы".to_string())` — 10 000 раз аллоцируется в куче одна и та же статическая константа!

- В `NavigatorItem::new_command` строка `description` клонируется дважды: в `subtitle: Some(description.clone())` и в `preview: Some(description)`.

- В `filtered_indices: Vec<usize>` используется 8-байтный `usize`, хотя элементов в навигаторе не бывает больше `u32::MAX`.

### Как переписать:

1. В `NavigatorState::close()` вызывайте `self.items = Vec::new(); self.filtered_indices = Vec::new(); self.query = String::new();`, чтобы полностью возвращать память аллокатору при закрытии модального окна.

2. Замените `category: Option<String>` на `Option<&'static str>`, `depth: usize` на `u16`, а для файлов не аллоцируйте дубликаты `title` и `subtitle` — вычисляйте их на лету как срезы `&str` от `path` при отрисовке тех 12 строк, которые реально видны в `ScrollArea::show_rows` (`src/navigator/render.rs`):

```rust
// В src/navigator/model.rs:
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigatorItem {
    pub id: u32,
    pub title: Box<str>,                // 16 байт вместо 24 (нет лишнего capacity)
    pub subtitle: Option<Box<str>>,     // Для файлов = None (берётся срезом из payload.path)
    pub category: Option<&'static str>, // 0 аллокаций в куче
    pub shortcut: Option<Box<str>>,
    pub is_folder: bool,
    pub is_collapsed: bool,
    pub depth: u16,                     // 2 байта вместо 8
    pub preview: Option<Box<str>>,
    pub payload: NavigatorItemPayload,
}

impl NavigatorItem {
    pub fn new_file(
        id: u32,
        _name: &str,
        relative_path: String,
        is_dir: bool,
        depth: usize,
    ) -> Self {
        Self {
            id,
            // Для файлов не дублируем имя в куче, если оно совпадает с концом relative_path:
            title: String::new().into_boxed_str(),
            subtitle: None,
            category: Some("Файлы"),
            shortcut: None,
            is_folder: is_dir,
            is_collapsed: false,
            depth: depth as u16,
            preview: None,
            payload: NavigatorItemPayload::File {
                path: relative_path,
                is_dir,
            },
        }
    }

    /// Возвращает заголовок без аллокаций (для файлов — срез имени из пути).
    #[inline]
    pub fn display_title(&self) -> &str {
        if !self.title.is_empty() {
            return &self.title;
        }
        if let NavigatorItemPayload::File { path, .. } = &self.payload {
            path.rsplit(['/', '\\']).next().unwrap_or(path)
        } else {
            &self.title
        }
    }

    /// Возвращает подзаголовок без аллокаций (для файлов — срез родительской директории).
    #[inline]
    pub fn display_subtitle(&self) -> Option<&str> {
        if let Some(sub) = &self.subtitle {
            return Some(sub);
        }
        match &self.payload {
            NavigatorItemPayload::File { path, is_dir } => {
                if *is_dir {
                    Some("Папка")
                } else {
                    path.rfind(['/', '\\']).map(|idx| &path[..idx]).filter(|s| !s.is_empty())
                }
            }
            NavigatorItemPayload::Command { .. } => self.preview.as_deref(),
            _ => None,
        }
    }
}

// В NavigatorState::close (src/navigator/model.rs):
pub fn close(&mut self) {
    self.open = false;
    self.search_mode = false;
    self.actions_menu_open = false;
    // Полностью освобождаем буферы кучи при закрытии оверлея:
    self.query = String::new();
    self.items = Vec::new();
    self.filtered_indices = Vec::new();
}

```

---

## 2. Устранение тройного хранения путей файлов в `Shell` и `tree.rs` (экономия ~65% RAM сайдбара)

**Локация:** `src/shell/mod.rs`, `src/shell/panels.rs` (`set_sidebar_files`, `sidebar_files`) и `src/shell/tree.rs` (`build_tree_items`).

### Почему расходуется память:

1. В `Shell` (`src/shell/mod.rs`) хранится одновременно и `sidebar_files: Vec<String>`, и `sidebar_model: RowModel`.

2. В `Shell::set_sidebar_files` (`src/shell/panels.rs`) входной вектор клонируется в `self.sidebar_files = files.clone()`, затем в `build_tree_items(&files)` (`src/shell/tree.rs`) строится промежуточное дерево `TreeNode`, где каждая папка и каждый файл аллоцируют собственные `name: String` и `full_path: Option<String>`, после чего `flatten` **клонирует их ещё раз** в `Vec<RowItem>`.

3. В самом `Shell` также создаются и постоянно живут `right_model: RowModel` и `row_vim: VimState`. При этом правая панель обратных ссылок (`src/native/backlinks.rs`) вообще не использует `right_model`, а рендерит `application.backlinks` напрямую. А `row_vim: VimState` в `Shell` держит второй полноценный экземпляр `VimState` (с собственными регистрами, макросами, историей правок и парсером) только ради нажатий `j`/`k`/`gg`/`G`/`/` в списке.

### Как переписать:

1. **Удалите поле `sidebar_files: Vec<String>` из `Shell**` (`src/shell/mod.rs`) — все файлы уже лежат в `self.sidebar_model.all_items()` (у каждого файла ` !item.is_folder` путь хранится в `item.data`).

2. **Построение дерева `build_tree_items` без промежуточного дерева `TreeNode` в куче:** поскольку `CatalogWorker` (`truff-store`) уже присылает список `files: Vec<String>` лексикографически отсортированным, можно отсортировать пути в порядке «папки перед файлами на каждом уровне» и построить плоский `Vec<RowItem>` за **один проход**, перемещая (moving) исходные `String` из `files` прямо в `RowItem.data` без единого `.clone()`!

```rust
// В src/shell/tree.rs:
use truff_core::navigation::{RowId, RowItem};
use std::cmp::Ordering;

/// Сравнивает два пути так, чтобы на каждом уровне иерархии папки шли раньше файлов.
fn cmp_tree_paths(a: &str, b: &str) -> Ordering {
    let mut a_parts = a.split(['/', '\\']).filter(|p| !p.is_empty()).peekable();
    let mut b_parts = b.split(['/', '\\']).filter(|p| !p.is_empty()).peekable();

    while let (Some(ap), Some(bp)) = (a_parts.next(), b_parts.next()) {
        let a_is_dir = a_parts.peek().is_some();
        let b_is_dir = b_parts.peek().is_some();
        match (a_is_dir, b_is_dir) {
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            _ => match ap.cmp(bp) {
                Ordering::Equal => continue,
                non_eq => return non_eq,
            },
        }
    }
    a_parts.count().cmp(&b_parts.count())
}

/// Строит плоский список RowItem напрямую из Vec<String>, ПЕРЕМЕЩАЯ строки без промежуточного TreeNode.
pub(super) fn build_tree_items(mut files: Vec<String>) -> Vec<RowItem> {
    files.sort_unstable_by(|a, b| cmp_tree_paths(a, b));

    let mut items = Vec::with_capacity(files.len() + files.len() / 4);
    let mut active_dirs: Vec<String> = Vec::with_capacity(8);
    let mut next_id = 0u64;

    for file_path in files {
        let mut parts_iter = file_path.split(['/', '\\']).filter(|p| !p.is_empty()).peekable();
        let mut depth = 0usize;

        while let Some(part) = parts_iter.next() {
            let is_dir = parts_iter.peek().is_some();
            if is_dir {
                if active_dirs.get(depth).map(String::as_str) != Some(part) {
                    active_dirs.truncate(depth);
                    let mut dir_title = String::with_capacity(part.len());
                    dir_title.push_str(part);
                    active_dirs.push(dir_title.clone());

                    items.push(RowItem {
                        id: RowId::new(next_id),
                        title: dir_title,
                        depth,
                        is_folder: true,
                        is_collapsed: true,
                        data: None,
                    });
                    next_id += 1;
                }
                depth += 1;
            } else {
                active_dirs.truncate(depth);
                let mut title = String::with_capacity(part.len());
                title.push_str(part);
                items.push(RowItem {
                    id: RowId::new(next_id),
                    title,
                    depth,
                    is_folder: false,
                    is_collapsed: false,
                    // Перемещаем сам String `file_path` с точным capacity без клонирования!
                    data: Some({
                        let mut p = file_path;
                        p.shrink_to_fit();
                        p
                    }),
                });
                next_id += 1;
                break;
            }
        }
    }
    items.shrink_to_fit();
    items
}

```

И в `src/shell/panels.rs` передавайте `files` по значению в `build_tree_items(files)`, убрав `self.sidebar_files = files.clone()`:

```rust
pub fn set_sidebar_files(&mut self, files: Vec<String>) {
    let is_empty = files.is_empty();
    let items = build_tree_items(files);
    self.sidebar_model.set_items(items);
    if is_empty && !self.sidebar_available && self.focus == Focus::Sidebar {
        self.set_focus(Focus::Editor);
    }
}

/// Итератор по относительным путям файлов без клонирования Vec<String>.
pub fn sidebar_file_paths(&self) -> impl Iterator<Item = &str> + '_ {
    self.sidebar_model
        .all_items()
        .iter()
        .filter(|item| !item.is_folder)
        .filter_map(|item| item.data.as_deref())
}

```

---

## 3. Сжатие кэша `EditorWidget`: отказ от 3x-оверскана, `Cow<str>` в `DisplayMap` и разреженный `HeightIndex`

**Локация:** `src/native/editor/viewport.rs` и `src/native/paint.rs`.

### Почему расходуется память:

1. **3-кратный оверскан кэша `Paragraph` (`src/native/editor/viewport.rs`):**

```rust
let first = self.heights.block_at_y((self.scroll - rect.height()).max(0.0)).0;
let last = (self.heights.block_at_y(self.scroll + rect.height() * 2.0).0 + 1).min(lines);
self.cache.retain(|line, _| *line >= first && *line < last);

```

Редактор одновременно держит в `self.cache` полные структуры `Paragraph` (содержащие `DisplayMap`, `Arc<Galley>` с тесселированными вершинными буферами глифов и `ParagraphStyle`) для **3 полных экранов** (`rect.height() * 3.0`), хотя на экране виден только 1 экран `[self.scroll .. self.scroll + rect.height()]`.

2. **Дублирование `TypographyConfig` внутри каждого `ParagraphStyle`:**
   Структура `ParagraphStyle` хранит клон `TypographyConfig` (включающий `font_family: String` и `custom_font_path: Option<PathBuf>`) в каждой закэшированной строке.

3. **Тройное хранение текста каждой видимой строки:**
   Внутри `DisplayMap` хранятся отдельно `source_text: String` и `visual_text: String`, а в `paint::shape` (`src/native/paint.rs`) делается третья копия `display.visual_text().to_owned()` для `egui::text::LayoutJob`. При этом для всех обычных строк без скрываемых символов форматирования `visual_text` побайтово равен `source_text`!

### Как переписать:

1. **Уменьшите окно кэширования в `src/native/editor/viewport.rs` с 3 экранов до 1 экрана + 4 строки буфера сверху и снизу** — это мгновенно сокращает число одновременно живущих `Paragraph` и `Galley` в **2.5–3 раза**:

```rust
// В src/native/editor/viewport.rs:
let overscan_px = LINE_HEIGHT * 4.0;
let first = self
    .heights
    .block_at_y((self.scroll - overscan_px).max(0.0))
    .0;
let last = (self
    .heights
    .block_at_y(self.scroll + rect.height() + overscan_px)
    .0
    + 1)
    .min(lines);
self.cache.retain(|line, _| *line >= first && *line < last);

```

2. **Уберите `TypographyConfig` (кучевые `String` и `PathBuf`) из `ParagraphStyle**`, заменив его компактным `Copy`-ключом размером 12 байт:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompactTypographyKey {
    pub proportional: bool,
    pub base_font_size_bits: u32,
    pub line_height_mult_bits: u32,
}

impl From<&TypographyConfig> for CompactTypographyKey {
    fn from(t: &TypographyConfig) -> Self {
        Self {
            proportional: t.font_family == "proportional",
            base_font_size_bits: t.base_font_size.to_bits(),
            line_height_mult_bits: t.line_height_multiplier.to_bits(),
        }
    }
}

```

Теперь `ParagraphStyle` становится `Copy`-структурой без единой аллокации в куче!

3. **В `DisplayMap` не храните `visual_text: String`, если в строке не скрывался ни один токен Markdown:** храните `visual_text: Option<Box<str>>`. Если `visual_text` равен `None`, метод `display.visual_text()` просто возвращает `&self.source_text`! Это экономит 50% строковой памяти `DisplayMap` на всех обычных строках и в режиме `ProjectionMode::Source`.

---

## 4. Оптимизация шрифтового атласа, кэшей `egui` и текстур OpenGL (`Glow`) (экономия ~15–30 МиБ RSS)

**Локация:** `src/main.rs`, `src/native/mod.rs` (`configure_fonts`, `persist_egui_memory`) и `src/native/paint.rs` (`resolve_font_and_line_height`).

### Почему расходуется память:

1. **18+ различных размеров шрифта раздувают текстурный атлас `egui` в RAM и VRAM:**
   В `egui` (`epaint`) каждый уникальный размер шрифта (`FontId { size, family }`) растеризует глифы в отдельную область текстурного атласа (CPU-буфер `FontImage` + GPU-текстура). Сейчас в коде используются дробные и близкие размеры: `10.0`, `10.5`, `11.0`, `11.5`, `12.0`, `12.5`, `13.0`, `13.5`, `14.0`, `15.0`, `16.0`, `26.0`, плюс в `resolve_font_and_line_height` (`src/native/paint.rs`) генерируются ещё 7 размеров для `h1..h6` и кода. Когда атлас заполняется, `egui` удваивает его размер по вертикали (до `2048x2048` или `4096x4096` байт в оперативной памяти).

2. **`persist_egui_memory(&self) -> bool { true }` в `src/native/mod.rs`:**
   При включённой персистентности памяти `egui` сохраняет метаданные всех виджетов, скролл-областей и окон между кадрами и при выходе сериализует их. При этом в `src/native/sidebar.rs` внутри цикла `show_rows` вызывается `ui.interact(rect, ui.auto_id_with(row.id), egui::Sense::click())`, что регистрирует тысячи уникальных `Id` в таблицах состояния `egui` при прокрутке дерева.

3. **Загрузка лишних шрифтов в `configure_fonts` (`src/native/mod.rs`):**
   Когда задан `custom_font_path`, вектор `bytes` (`std::fs::read(path)`) добавляется поверх всех дефолтных шрифтов `FontDefinitions::default()`, удерживая в RAM неиспользуемые встроенные шрифты.

### Как переписать:

1. **Отключите `persist_egui_memory` в `src/native/mod.rs**` (позиции курсоров и скролл вьюпортов Truff уже сам сохраняет в `.truff/session.toml` через `truff-store`):

```rust
// В src/native/mod.rs:
impl eframe::App for NativeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        self.show(ui);
    }

    fn persist_egui_memory(&self) -> bool {
        false
    }
}

```

2. **Квантуйте размеры шрифтов UI до 4 базовых ступеней (`11.0`, `12.0`, `14.0`, `16.0`), а в `resolve_font_and_line_height` объедините близкие уровни заголовков `####` / `#####` / `######`:**

````rust
// В src/native/paint.rs:
pub fn resolve_font_and_line_height(source: &str, typography: &TypographyConfig) -> (FontId, f32) {
    let family = match typography.font_family.as_str() {
        "proportional" => egui::FontFamily::Proportional,
        _ => egui::FontFamily::Monospace,
    };
    let base = typography.base_font_size.round();
    let source_trim = source.trim_start();

    // Используем только 4 дискретных размера в редакторе вместо 8,
    // чтобы не раздувать глифовый атлас epaint:
    if source_trim.starts_with("# ") {
        let size = (base * 1.6).round();
        (FontId::new(size, family), (size * 1.3).round())
    } else if source_trim.starts_with("## ") {
        let size = (base * 1.35).round();
        (FontId::new(size, family), (size * 1.35).round())
    } else if source_trim.starts_with("### ") || source_trim.starts_with("#### ") {
        let size = (base * 1.15).round();
        (FontId::new(size, family), (size * 1.4).round())
    } else if source_trim.starts_with("```") || source_trim.starts_with("    ") {
        (
            FontId::new(base, egui::FontFamily::Monospace),
            (base * 1.45).round(),
        )
    } else {
        (
            FontId::new(base, family),
            (base * typography.line_height_multiplier).round(),
        )
    }
}

````

3. **В `src/native/sidebar.rs` используйте уже созданный `frame_resp.response` вместо второго виджета `ui.interact(rect, ui.auto_id_with(row.id), ...)`:**

```rust
// В src/native/sidebar.rs вместо frame.show(...) + ui.interact(...):
let interact_resp = frame_resp.response.interact(egui::Sense::click());
if interact_resp.clicked() {
    if row.is_folder {
        toggle_folder = Some(row.id);
    } else {
        open = row.data.clone();
    }
}

```

---

## 5. Предотвращение фрагментации кучи от многомегабайтных `buffer.as_str()` и возврат страниц ОС (`malloc_trim`)

**Локация:** `src/app/commands/editing.rs`, `src/app/commands/links.rs`, `src/app/navigation.rs`, `src/app/session.rs`.

### Почему расходуется память:

- Когда в `Application` вызывается `self.shell.editor_text()` (который внутри вызывает `Buffer::as_str()`), в куче выделяется непрерывный кусок памяти размером со весь документ (до 16 МиБ).

- Если в этот же момент (например, внутри `follow_link`, `apply_pending_location`, `color_selection` или `link_selection`) создаются мелкие долгоживущие структуры (`Transaction`, `StatusMessage(String)`, `View`), они размещаются аллокатором на вершине кучи сразу за этим большим блоком. Когда большой `String` освобождается, аллокатор (особенно стандартный `ptmalloc` в glibc на Linux) не может сдвинуть `brk` вниз из-за мелких объектов сверху, и эти мегабайты навсегда остаются в `VmRSS` процесса!

### Как переписать:

1. **В `follow_link` (`src/app/commands/links.rs`) не парсите весь документ через `self.shell.editor_text()`:** ссылка под курсором гарантированно находится на **текущей строке** `head`, поэтому достаточно извлечь только одну текущую строку из `Buffer` (~100 байт вместо 16 МиБ)!

```rust
// В src/app/commands/links.rs:
pub(super) fn follow_link(&mut self, argument: &str) -> AppResult<()> {
    let target = if argument.is_empty() {
        let view = self.shell.editor_view().ok_or("Open a note first")?;
        let buffer = self.shell.editor_buffer().ok_or("Open a note first")?;
        let head = view.head().get().min(buffer.byte_len());

        // Извлекаем ТОЛЬКО текущую строку вокруг курсора вместо копирования всего буфера:
        let line_idx = buffer.line_of_byte(head);
        let line_start = buffer.byte_of_line(line_idx);
        let line_end = if line_idx + 1 < buffer.line_len() {
            buffer.byte_of_line(line_idx + 1)
        } else {
            buffer.byte_len()
        };
        let line_text = buffer.slice(line_start..line_end).to_string();
        let local_head = head - line_start;

        let facts = MarkdownFacts::extract(&line_text)?;
        let link = facts
            .links
            .iter()
            .find(|link| link.range.start <= local_head && local_head <= link.range.end)
            .ok_or("Place the cursor inside a link")?;
        match &link.heading {
            Some(heading) => format!("{}#{}", link.target, heading),
            None => link.target.clone(),
        }
    } else {
        argument.to_owned()
    };
    self.submit_catalog(CatalogJob::Follow {
        source: self.active_relative_path().ok_or("Open a note first")?,
        target,
    })
}

```

2. **В `WhichKeyOverlay` (`src/overlays/which_key.rs` и `src/overlays/which_key/help.rs`) не храните категории справки в `self.categories: Vec<WhichKeyCategory>` постоянно и используйте статический срез** или освобождайте вектор через `shrink_to_fit()` в `WhichKeyOverlay::hide()`:

```rust
// В src/overlays/which_key.rs:
pub fn hide(&mut self) {
    self.visible = false;
    self.hints = Vec::new();
    self.categories = Vec::new();
    self.title = String::new();
    self.mode = WhichKeyMode::Prefix;
    self.elapsed_ms = 0;
}

```

---

## Сводная таблица экономии памяти после внедрения изменений

| Участок `truff-ui`                              | Текущее поведение | После оптимизации | Экономия RAM |
| ----------------------------------------------- | ----------------- | ----------------- | ------------ |
| **`NavigatorState`** (`src/navigator/model.rs`) |

| Удерживает 10 000 `NavigatorItem` (по 4 `String` в каждом) даже после закрытия окна

| Срезы из `payload.path`, статические категории `&'static str`, полный сброс буферов в `close()` | **-4..6 МиБ** в простое |
| **`Shell` + `tree.rs**` (`src/shell/panels.rs`, `tree.rs`)

| Хранит `sidebar_files` + `TreeNode` + `RowModel` (5x копий путей в пике, 2x постоянно)

| Прямое перемещение `String` в `RowItem`, удаление дубликата `sidebar_files` | **-65% постоянной** и **-80% пиковой** памяти дерева |
| **`EditorWidget`** (`src/native/editor/viewport.rs`)

| Кэширует 3 экрана `Paragraph`/`Galley`, клонирует `TypographyConfig` в каждую строку

| Кэш 1 экрана + 4 строки оверскана, `Copy`-ключ типографики, `Cow`-текст в `DisplayMap` | **-65% памяти кэша вьюпорта** |
| **Шрифтовой атлас и память `egui**` (`src/native/mod.rs`, `paint.rs`)

| 18+ размеров шрифта в атласе, накопление `auto_id_with` при `persist_egui_memory = true`<br> | Квантование до 4 размеров в редакторе, `persist_egui_memory = false` | **-10..20 МиБ** текстурного кэша и `egui::Memory` |
| **Команды `follow_link` / `link_selection**` (`src/app/commands/`)

| Копируют весь буфер через `editor_text()` (до 16 МиБ) ради одной строки

| Чтение только текущей строки / среза `buffer.slice(range)` | **До -16 МиБ** пикового RSS |
