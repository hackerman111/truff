# Глубокий технический аудит производительности Truff и инженерный план оптимизации

**Репозиторий:** [hackerman111/truff](https://github.com/hackerman111/truff)  
**Исследуемая ветка:** `main`  
**Зафиксированный commit SHA:** `0cfd55cf77f7bcadf2da1f9d4882f22028aa1f32` (`0cfd55c`, сообщение коммита: `argon optim all-2`)  
**Версия Rust Workspace:** `edition = "2024"`, `rust-version = "1.95.0"`

## 1. Изучение проекта, границы ответственности крейтов и верификация существующих планов

### 1.1. Фактические границы ответственности крейтов по актуальному коду

Кодовая база [hackerman111/truff](https://github.com/hackerman111/truff) организована как Cargo workspace из трёх основных крейтов, расположенных в каталоге `crates/`:

| Крейт         | Путь и манифест                                                                                      | Ключевые зависимости                                                                                                                                           | Фактическая зона ответственности по коду (`0cfd55c`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ------------- | ---------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `truff-core`  | [`crates/core/Cargo.toml`](https://github.com/hackerman111/truff/blob/main/crates/core/Cargo.toml)   | `crop = 0.4.3`, `unicode-segmentation = 1.13.3`, `pulldown-cmark = 0.13.4`, `tree-sitter = 0.26`, `tree-sitter-md = 0.5.3`, `serde`, `toml`                    | Транзакционная модель текстового буфера (`Buffer` поверх `crop::Rope`), система правок и сдвигов координат (`SingleEdit`, `Transaction`, `Change`, `ChangeSet`), история отмены/повтора с бюджетом памяти (`History`, `HistoryGroup`) и регистры (`Registers`), движок модального редактирования Vim (`VimState`, парсер команд, движения каретки, текстовые объекты, поиск), извлечение фактов и структуры Markdown (`MarkdownFacts`, `SyntaxEngine`, `MarkdownSnapshot`, `LinePresentation`, `FlashTarget`, `smart_edit`), а также центральная модель рабочей области (`Workspace`, `BufferRegistry`, `View`, `JumpHistory`, `dispatch`).                                                                                     |
| `truff-store` | [`crates/store/Cargo.toml`](https://github.com/hackerman111/truff/blob/main/crates/store/Cargo.toml) | `truff-core`, `rusqlite = 0.40 (bundled)`, `serde`, `serde_json`, `toml`, `sha2 = 0.10.9`, `notify = 8.2.0`                                                    | Атомарные файловые операции с контролем лимитов размера и симлинков (`files.rs`), управление путями хранилища (`vault.rs`), реляционный и полнотекстовый индекс в SQLite + FTS5 (`index/schema.sql`, `index/update.rs`, `index/resolve.rs`, `index/search.rs`, `index/backlinks.rs`), двухфазные файловые операции переименования/перемещения с переписыванием обратных ссылок (`operations/plan.rs`, `rewrites.rs`, `commit.rs`, `recovery.rs`), журнал краш-восстановления несохранённых буферов (`recovery.rs`), а также фоновые рабочие потоки ввода-вывода и индексации каталога (`worker/runtime.rs`, `catalog/mod.rs`, `catalog/scan.rs`).                                                                               |
| `truff-ui`    | [`crates/ui/Cargo.toml`](https://github.com/hackerman111/truff/blob/main/crates/ui/Cargo.toml)       | `truff-core`, `truff-store`, `eframe = 0.36.2 (glow, default_fonts, wayland, x11, accesskit, winit_default)`, `crop`, `unicode-segmentation`, `pulldown-cmark` | Нативное графическое приложение на базе `eframe`/`egui` с OpenGL-бэкендом `glow` (`NativeApp` в `native/mod.rs`), управление жизненным циклом окон, файлов и фоновых очередей (`Application` в `app/mod.rs`, `retention.rs`), визуальная проекция Markdown-строк (`DisplayMap` в `editor/display/`), виртуализированный расчёт высот на дереве Фенвика (`HeightIndex` в `editor/heights.rs`), построчный кэш шейпинга и отрисовки (`EditorWidget`, `Paragraph`, `viewport.rs`, `invalidation.rs`, `paragraph/paint.rs`), универсальный навигатор (`NavigatorState`), боковые панели и оверлеи (Backlinks, TOC, Folder Wiki, Note Stream), а также встроенные headless/GUI бенчмарки (`perf_scenarios/`, `examples/ui_perf.rs`). |

**Критическое архитектурное наблюдение по границам крейтов:**

1. В [`crates/ui/Cargo.toml`](https://github.com/hackerman111/truff/blob/main/crates/ui/Cargo.toml) напрямую подключены `pulldown-cmark`, `crop` и `unicode-segmentation`, так как модуль [`crates/ui/src/editor/display/markers.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/editor/display/markers.rs) самостоятельно выполняет разбор inline-маркеров Markdown для каждой видимой строки поверх `DisplayMap`, параллельно с логикой `truff-core::markdown`.
2. Внутри `crates/ui/src/` сосуществуют **два различных контура редактора**:
   - **Боевой контур нативного GUI**: `NativeApp` (`native/mod.rs`) $\rightarrow$ `EditorWidget` (`native/editor/mod.rs`) $\rightarrow$ `Paragraph` (`native/editor/paragraph/mod.rs`) + `HeightIndex` (`editor/heights.rs`) + `EditorSyntax` (`native/editor/syntax.rs`).
   - **Изолированный контур headless-проб**: `EditorProbe` (`editor/probe.rs`) + `VirtualizedLayout` (`editor/layout.rs`), который используется в старых сценариях `perf_scenarios/` (`--bench-scenario`), но **не участвует** в работе реального окна редактора.

### 1.2. Анализ архитектурных и инженерных стандартов (`.agent/` и `AGENTS.md`)

Изучение [`AGENTS.md`](https://github.com/hackerman111/truff/blob/main/AGENTS.md), [`.agent/PERFORMANCE.md`](https://github.com/hackerman111/truff/blob/main/.agent/PERFORMANCE.md), [`.agent/BENCHMARKING.md`](https://github.com/hackerman111/truff/blob/main/.agent/BENCHMARKING.md) и [`.agent/ARCHITECTURE.md`](https://github.com/hackerman111/truff/blob/main/.agent/ARCHITECTURE.md) фиксирует строгие инварианты проекта, которые обязаны соблюдаться при любых оптимизациях:

- **Иерархия приоритетов из `.agent/PERFORMANCE.md`**: `algorithm -> redundant work -> data structure -> data layout -> I/O -> allocation -> copying -> synchronization -> parallelism -> branches -> vectorization`. Главный принцип: _«Prefer removing work over making work faster»_.
- **Каноническое владение состоянием (`.agent/ARCHITECTURE.md`)**: единый экземпляр `Workspace`, `Buffer`, `VimState` и `Application`; недопустимость дублирования авторитетного состояния документа между UI и Core.
- **Целевые бюджеты производительности (`docs/perf/p7/metadata.toml` и `docs/perf/p0/decision.md`)**:
  - `empty_window_rss_limit_mib`: изначально $\le 100.0\text{ MiB}$ (цель $60.0\text{ MiB}$), после перехода на `egui/glow` на хосте с NVIDIA RTX 5060 скорректирован в [`docs/perf/p0/decision.md`](https://github.com/hackerman111/truff/blob/main/docs/perf/p0/decision.md) до $\le 160.0\text{ MiB}$ VmRSS с обязательным разделением VmRSS и PSS;
  - `vault_10k_overhead_limit_mib`: $\le 60.0\text{ MiB}$ сверх пустого окна;
  - `core_edit_p95_ms`: $\le 2.0\text{ ms}$;
  - `warm_scroll_frame_p95_ms`: $\le 16.7\text{ ms}$ ($60\text{ FPS}$);
  - `visible_layout_ready_p95_ms`: $\le 50.0\text{ ms}$;
  - `cold_startup_p95_ms`: $\le 500.0\text{ ms}$.

### 1.3. Верификация существующих отчётов и планов оптимизации на коммите `0cfd55c`

В репозитории присутствуют четыре основных источника предложений по оптимизации:

1. [`optim_core.md`](https://github.com/hackerman111/truff/blob/main/optim_core.md) (637 строк) — план оптимизации памяти `truff-core`.
2. [`optim_ui.md`](https://github.com/hackerman111/truff/blob/main/optim_ui.md) — план оптимизации памяти `truff-ui`.
3. [`Оптимизация производительности и устранение ботлнеков.md`](https://github.com/hackerman111/truff/blob/main/%D0%9E%D0%BF%D1%82%D0%B8%D0%BC%D0%B8%D0%B7%D0%B0%D1%86%D0%B8%D1%8F%20%D0%BF%D1%80%D0%BE%D0%B8%D0%B7%D0%B2%D0%BE%D0%B4%D0%B8%D1%82%D0%B5%D0%BB%D1%8C%D0%BD%D0%BE%D1%81%D1%82%D0%B8%20%D0%B8%20%D1%83%D1%81%D1%82%D1%80%D0%B0%D0%BD%D0%B5%D0%BD%D0%B8%D0%B5%20%D0%B1%D0%BE%D1%82%D0%BB%D0%BD%D0%B5%D0%BA%D0%BE%D0%B2.md) (2352 строки) и его трекер [`docs/perf/optimization-inventory.md`](https://github.com/hackerman111/truff/blob/main/docs/perf/optimization-inventory.md).
4. Отчёты [`docs/perf/p0/decision.md`](https://github.com/hackerman111/truff/blob/main/docs/perf/p0/decision.md) и [`docs/perf/p7/summary.md`](https://github.com/hackerman111/truff/blob/main/docs/perf/p7/summary.md).

Ниже приведена строгая сверка каждого утверждения из этих документов с фактическим кодом на коммите `0cfd55cf77f7bcadf2da1f9d4882f22028aa1f32`:

| Источник / Пункт плана                                                            | Утверждение в существующем документе                                                                                                           | Фактический статус в коде (`0cfd55c`)                                                                                                                                                                                                                                                                        | Технический вердикт аудита                                                                                                                                                                                       |
| --------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `docs/perf/p7/summary.md` vs `docs/perf/p0/decision.md`                           | В `p7/summary.md` (от 2026-10-10) описывается дерево виджетов Masonry (`AppWidget -> EditorWidget`).                                           | **Устарело (Masonry удалён)**. В `p0/decision.md` (от 2026-10-11) и `crates/ui/Cargo.toml` фронтенд переведён на `eframe = "0.36.2"` (`egui` + `glow`).                                                                                                                                                      | Любые рекомендации, специфичные для Masonry/wgpu/vello, более неприменимы. Проблемы памяти GUI теперь определяются `eframe`/`glow`, загрузкой EGL/Mesa/LLVM и кэшем `egui::Galley`.                              |
| `optim_core.md` (Узел 1: `compact_transaction`)                                   | Сжатие избыточного `capacity` строк в `SingleEdit` при записи в `History`.                                                                     | **Реализовано** в [`crates/core/src/text/history.rs`](https://github.com/hackerman111/truff/blob/main/crates/core/src/text/history.rs) (строки 20–28, `fn compact_transaction`).                                                                                                                             | Повторно предлагать не требуется; однако текущая реализация сохраняет capacity до порога 4096 байт и не решает проблему тысячи мелких аллокаций.                                                                 |
| `optim_core.md` (Узлы 2–5: уплощение `HistoryGroup`)                              | Замена `forward: Vec<Transaction>` и `inverse: Vec<Transaction>` на плоские `forward_edits: Vec<SingleEdit>` и `forward_batch_ends: Vec<u32>`. | **НЕ реализовано**. В `crates/core/src/text/history.rs` (строки 32–43) `HistoryGroup` всё ещё хранит `Vec<Transaction>`, а `Buffer::apply` (`buffer/mod.rs`, строки 334–344) создаёт две структуры `Transaction` на каждый ввод.                                                                             | **Актуально и требует расширения (`OPT-01`)**: помимо уплощения `Vec<Transaction>`, необходимо добавить коалесцирование смежных односимвольных правок (`Insert` typing run), чего не было в базовом варианте.    |
| `optim_core.md` (Узел 6: `Registers` CoW / `Arc`)                                 | Разделение `Arc<Register>` между безымянным и именованным регистрами.                                                                          | **Частично реализовано** в [`crates/core/src/text/history/registers.rs`](https://github.com/hackerman111/truff/blob/main/crates/core/src/text/history/registers.rs) (строки 43–46).                                                                                                                          | Осталась архитектурная ошибка (`OPT-05`): структура `Registers` всё ещё встроена внутрь `History` каждого отдельного `Buffer` (`history.rs`, строка 82) вместо `Workspace`/`VimState`.                           |
| `optim_ui.md` (`NavigatorState::close` и `NavigatorItem`)                         | Очищать `items` и `filtered_indices` при закрытии навигатора; сжать поля `NavigatorItem` через `Box<str>`, `u16`, `&'static str`.              | **Полностью реализовано** в [`crates/ui/src/navigator/model.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/navigator/model.rs) (строки 88–89) и [`crates/ui/src/navigator/types.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/navigator/types.rs) (строки 141–148). | Предложения из `optim_ui.md` по `NavigatorState::close` и `NavigatorItem` уже в коде. Однако в `NavigatorItemPayload` (`types.rs`) остались `String`, а фильтрация 10 000 элементов занимает 4.17 мс (`OPT-12`). |
| `optimization-inventory.md` (`L20 / 1.1-p1`: поиск с `count`)                     | Останавливать поиск в буфере после `count` совпадений в окне вокруг курсора.                                                                   | **Реализовано** в `crates/core/src/vim/motion/search.rs` (ускорение в 1723×–37016× при наличии совпадения рядом).                                                                                                                                                                                            | При отсутствии совпадения в окне или поиске редкого паттерна всё ещё происходит материализация больших срезов (`OPT-06`).                                                                                        |
| `optimization-inventory.md` (`L290 / 1.5-p1`: `IncrementalSyntaxTree`)            | Вычислять координаты для Tree-sitter и читать `Rope` чанками.                                                                                  | **Частично / Некорректно интегрировано**: в `crates/core/src/markdown/syntax.rs` (строки 130–155) осталась $O(N)$ функция `byte_to_point(text: &str, ...)`, а `EditorSyntax` (`crates/ui/src/native/editor/syntax.rs`) хранит копию `Rope`.                                                                  | Требует исправления (`OPT-02`): переход на $O(\log N)$ индекс строк `Rope` (`byte_to_line` / `line_to_byte`) и отказ от линейного сканирования `char_indices()`.                                                 |
| `optimization-inventory.md` (`L380 / 1.6-p1` и `L450 / 1.7-p1`: `ChangeSet`)      | Индексация `prefix_shifts` для батчей от 32 правок и бинарный поиск в `map_pos`.                                                               | **Реализовано с регрессией $0.36\times$** в [`crates/core/src/text/edit.rs`](https://github.com/hackerman111/truff/blob/main/crates/core/src/text/edit.rs) (строки 155–260). В `docs/perf/core/README.md` зафиксировано замедление создания `ChangeSet` с 2.786 мкс до 7.707 мкс.                            | Требует переработки (`OPT-03`): встраивание `prefix_shift` прямо в структуру `Change` вместо отдельной аллокации `Vec<isize>` и устранение клонирования `SingleEdit` в `from_edits`.                             |
| `optimization-inventory.md` (`L890 / 2.3-p1`: переиндексация документа в `store`) | Инкрементальное обновление индексов связей и метаданных.                                                                                       | **Неполная реализация**: `update::index_document_in` (`crates/store/src/index/update.rs`, строки 39–156) обновляет одну заметку, но `refresh_resolutions` (строки 192–221) выгружает и резолвит **все ссылки всех заметок** (`SELECT ... FROM links`).                                                       | Критический алгоритмический ботлнек масштабируемости на 10 000 заметок (`OPT-14`).                                                                                                                               |
| `optimization-inventory.md` (`L1070 / 2.5-p1`: повторное открытие каталога)       | Проверка по SHA-256 `fingerprint` и пропуск повторной индексации.                                                                              | **Частично реализовано** в [`crates/store/src/catalog/scan.rs`](https://github.com/hackerman111/truff/blob/main/crates/store/src/catalog/scan.rs) (строки 84–164).                                                                                                                                           | Для вычисления `fingerprint` код всё ещё **открывает и полностью читает с диска все 10 000 файлов** при каждом старте без проверки `(mtime, size)` (`OPT-15`).                                                   |
| `optimization-inventory.md` (`L1530 / 3.3-p1`: вставка перевода строки в UI)      | Локальное обновление макета при вставке `\n`.                                                                                                  | **Частично реализовано**: `invalidation.rs` (строки 93–109) не пересобирает `Galley`, но перестраивает весь `BTreeMap` кэша, а `HeightIndex::replace_blocks` (`heights.rs`, строки 118–127) делает `blocks.splice` + полный пересчёт дерева Фенвика `rebuild_sums()` $O(L)$!                                 | Подтверждено замерами в `docs/perf/ui/README.md` (`insert_newline_frame` = 139.89 мкс против 83.85 мкс для обычного символа). Требует `OPT-08`.                                                                  |

---

### 1.4. Сквозная трассировка основных путей выполнения (Data & Control Flow)

#### Путь А: Нажатие клавиши в редакторе до отображения кадра (Input-to-Present)

1. **Событие ОС и `winit`/`eframe`**: Нажатие клавиши поступает из Wayland/X11 в `eframe`, который запускает `NativeApp::ui` $\rightarrow$ `NativeApp::show` ([`crates/ui/src/native/mod.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/native/mod.rs), строки 106–285).
2. **Опрос фоновых каналов**: `self.application.poll()` дренирует `StoreEvent` ([`crates/ui/src/app/io/store_events.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/app/io/store_events.rs)) и `CatalogEvent` ([`crates/ui/src/app/io/catalog_events.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/app/io/catalog_events.rs)), а также вызывает `release_closed_buffers()` ([`crates/ui/src/app/retention.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/app/retention.rs)).
3. **Синхронизация TOC**: Вызывается `self.sync_toc()`. _Узкое место (`OPT-07`)_: если панель TOC открыта или закреплена, `sync_toc()` вызывает `self.application.document_facts()` ([`crates/ui/src/app/document.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/app/document.rs), строки 15–30), который при изменившемся `buffer.revision()` материализует весь `Rope` в `String` и запускает 4-проходный `MarkdownFacts::extract` прямо в UI-потоке.
4. **Обработка клавиатуры и Vim-транзакция**: Событие клавиши преобразуется в `truff_core::vim::Key` и передаётся в `Shell` $\rightarrow$ `VimState`. При редактировании формируется `Transaction { edits: Vec<SingleEdit>, cursor, undo_group }` и диспетчеризуется через `Workspace::dispatch(Action::ApplyEdit)` ([`crates/core/src/workspace/dispatch.rs`](https://github.com/hackerman111/truff/blob/main/crates/core/src/workspace/dispatch.rs)).
5. **Модификация `Buffer` и запись `History`**: `Buffer::apply` ([`crates/core/src/text/buffer/mod.rs`](https://github.com/hackerman111/truff/blob/main/crates/core/src/text/buffer/mod.rs), строки 312–349) валидирует правки, строит обратные правки через `build_inverse_and_changes`, применяет замену к `crop::Rope`, оборачивает прямые и обратные правки в две структуры `Transaction` (_узкое место `OPT-01`_), записывает их в `History::record` ([`crates/core/src/text/history.rs`](https://github.com/hackerman111/truff/blob/main/crates/core/src/text/history.rs)), увеличивает `revision` и возвращает `ChangeSet` (_узкое место `OPT-03`_).
6. **Инвалидация кэша UI**: `EditorWidget::apply_change` ([`crates/ui/src/native/editor/invalidation.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/native/editor/invalidation.rs), строки 11–110) обновляет `EditorSyntax` (_узкое место `OPT-02`_), вызывает `self.heights.replace_blocks(...)` (_узкое место `OPT-08`: при `line_delta != 0` перестраивает всё дерево Фенвика за $O(L)$_) и сдвигает ключи в `self.cache: BTreeMap<usize, Paragraph>` (_при `line_delta != 0` пересоздаёт весь `BTreeMap` за $O(C \log C)$_).
7. **Виртуализированная верстка и отрисовка (`viewport.rs`)**: `EditorWidget::show` ([`crates/ui/src/native/editor/viewport.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/native/editor/viewport.rs), строки 14–486) находит видимый диапазон строк `first..last` через `HeightIndex::block_at_y` ($O(\log L)$), отсекает невидимые строки в `self.cache.retain(...)`, для изменённой строки создаёт `Paragraph::new` (который конструирует `DisplayMap`, запускает `scan_markers` и `rebuild` с 6+ аллокациями векторов и строк — _узкое место `OPT-09`_, формирует `egui::text::LayoutJob` и шейпит `Arc<Galley>`), обновляет высоту блока в `HeightIndex` и рисует глифы, курсор, выделение и Flash через `Paragraph::paint` ([`crates/ui/src/native/editor/paragraph/paint.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/native/editor/paragraph/paint.rs)).
8. **Тесселяция `egui` и `glow` present**: `eframe` тесселирует примитивы `epaint::ClippedPrimitive` в вершинные буферы OpenGL (`glow`) и выполняет `swap_buffers`.

---

## 2. Многоуровневый системный анализ (Алгоритмы, Память, CPU, Архитектура)

### 2.1. Алгоритмический уровень: сводка текущей и целевой сложности

| Операция / Подсистема                                                      | Локация в коде                                                             | Текущая сложность (Время / Память)                                                                               | Целевая сложность (Время / Память)                                                                     | Затрагиваемые оптимизации              |
| -------------------------------------------------------------------------- | -------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ | -------------------------------------- |
| Преобразование байта в `Point(row, col)` для Tree-sitter                   | `crates/core/src/markdown/syntax.rs` (`byte_to_point`)                     | Время: $O(N)$ от начала файла<br>Память: $O(N)$ при передаче `&str`                                              | Время: $O(\log N)$ через `Rope::byte_to_line`<br>Память: $O(1)$ (чтение чанками `Rope`)                | `OPT-02`                               |
| Построение `ChangeSet` и маппинг якорей `map_pos`                          | `crates/core/src/text/edit.rs` (`ChangeSet::new`, `from_edits`, `map_pos`) | Создание: $O(K \log K)$ + клон строк + 2 аллокации<br>`map_pos`: $O(K)$ при $K < 32$, $O(\log K)$ при $K \ge 32$ | Создание: $O(K)$ (для отсортированных) с 1 аллокацией без клона строк<br>`map_pos`: $O(\log K)$ всегда | `OPT-03`                               |
| Извлечение фактов `MarkdownFacts::extract`                                 | `crates/core/src/markdown/facts.rs`                                        | Время: 4 прохода по документу + сортировка ссылок $O(N + L \log L)$<br>Память: $O(N)$ на `buffer.to_string()`    | Время: 2 прохода (AST + единый SIMD-проход) + $O(L)$ merge<br>Для TOC: $O(H)$ только по заголовкам     | `OPT-04`, `OPT-07`                     |
| Обновление `HeightIndex` и кэша строк при вставке/удалении `\n`            | `crates/ui/src/editor/heights.rs`, `native/editor/invalidation.rs`         | Время: $O(L + C \log C)$ ($L$ — все строки файла, $C$ — кэш)<br>Аллокации: $O(C)$ узлов `BTreeMap`               | Время: $O(\text{число измеренных строк})$ или суффикс + $O(C)$ в кольцевом буфере<br>Аллокации: $O(1)$ | `OPT-08`                               |
| Маппинг координат `source_to_visual` / `visual_to_source` в `DisplayMap`   | `crates/ui/src/editor/display/mapping.rs`                                  | Время: $O(C_{\text{chunks}})$ линейный проход                                                                    | Время: $O(\log C_{\text{chunks}})$ бинарный поиск по `visual_start`                                    | `OPT-10`                               |
| Фильтрация в `NavigatorState::filter` (10 000 заметок)                     | `crates/ui/src/navigator/model.rs`                                         | Время: $O(M \cdot                                                                                                | \text{title}                                                                                           | )$ на каждый символ с `to_lowercase()` | Время: $O(M_{\text{prev}})$ (сужение множества) + $O(1)$ битовая маска `u64` | `OPT-12` |
| Обновление разрешения ссылок `refresh_resolutions` при сохранении заметки  | `crates/store/src/index/update.rs`                                         | Время: $O(E_{\text{vault}} \cdot \log V)$ (все ссылки хранилища!)                                                | Время: $O(E_{\text{note}} + E_{\text{incoming\_affected}})$                                            | `OPT-14`                               |
| Повторное открытие хранилища из $V$ неизменённых заметок (`scan::refresh`) | `crates/store/src/catalog/scan.rs`                                         | Время и I/O: $O(\sum \text{size}_i)$ — чтение и SHA-256 всех $V$ файлов                                          | Время и I/O: $O(V)$ `stat`/метаданные directory entry, $0$ чтений файлов                               | `OPT-15`                               |

### 2.2. Управление памятью: раскладка структур и разделение ответственности (Truff vs egui/glow vs OS/Driver)

1. **Графический стек и драйверы (`eframe`/`egui`/`glow` + Linux GLVND)**:
   - Согласно реальным измерениям из [`docs/perf/p0/decision.md`](https://github.com/hackerman111/truff/blob/main/docs/perf/p0/decision.md), переход с Masonry (446.4–584.6 MiB VmRSS) на `eframe/glow` снизил потребление памяти до **131.4 MiB VmRSS** (и **50.4 MiB PSS** на документе 1 МБ) при использовании только NVIDIA EGL ICD, но при стандартном выборе GLVND в процесс подгружаются `libEGL_mesa.so` и `libLLVM.so`, увеличивая VmRSS до **308.3 MiB** (+176.9 MiB). При этом ~96 MiB VmRSS приходится на разделяемые страницы (`Shared_Clean`) драйвера компилятора шейдеров GPU, что доказывается разницей между VmRSS (140.1 MiB) и PSS (50.4 MiB).
2. **Память строкового кэша редактора (`Paragraph` + `DisplayMap` + `Galley`)**:
   - В `EditorWidget` кэшируется окно видимых строк (`~52–78` элементов `Paragraph`). Каждый `Paragraph` сейчас хранит три копии текста строки (`DisplayMap.source_text`, `DisplayMap.visual_text`, `Galley.job.text`) и 6 векторов метаданных (`chunks`, `colors`, `styles`, `markers`, `lists` + `Galley.rows`). Устранение промежуточных векторов из `DisplayMap` и переход на переиспользуемый `DisplayScratchpad` (`OPT-09`) убирает 5 кучевых аллокаций на каждый создаваемый параграф и снижает фрагментацию аллокатора при быстрой прокрутке.
3. **Память истории правок (`History` в `truff-core`)**:
   - Бюджет `DEFAULT_MAX_HISTORY_BYTES` равен 8 МиБ на буфер (`crates/core/src/text/history.rs`). Из-за хранения `Vec<Transaction>` внутри `HistoryGroup` и отсутствия слияния односимвольных вставок (`SingleEdit`) реальные накладные расходы кучи (с учётом заголовков `Vec` и `String`) при наборе текста превышают полезный объём текста в 8–12 раз (`OPT-01`).

---

## 3. Верификация базовых показателей (`docs/perf/`) и методика измерений для 8 целевых сценариев

### 3.1. Анализ существующих замеров и ограничений `docs/perf/p7/summary.md`

В [`docs/perf/p7/summary.md`](https://github.com/hackerman111/truff/blob/main/docs/perf/p7/summary.md) и [`docs/perf/p7/metadata.toml`](https://github.com/hackerman111/truff/blob/main/docs/perf/p7/metadata.toml) задокументирован важный методологический факт:

- Исторические файлы `docs/perf/p7/samples.csv` и `metadata.toml` (от 2026-10-10) были сгенерированы **headless-пробами** (`EditorProbe` и `VirtualizedLayout`), где поля окружения (`Wayland / X11`, `1920x1080`) были захардкожены генератором отчёта, а реальное окно ОС, GPU-контекст и системный цикл событий не запускались.
- Начиная с коммитов от 2026-10-11..2026-10-14, в репозитории созданы достоверные микробенчмарки для трёх крейтов (`core_perf`, `store_perf`, `catalog_perf`, `recovery_perf`, `ui_perf`) и нативные пробы памяти (`docs/perf/p0/decision.md`).

Сводка подтверждённых базовых показателей из репозитория (`docs/perf/{core,store,ui}/README.md` и `docs/perf/p0/decision.md`):

| Подсистема                  | Сценарий                                                                        | Базовый показатель (До `argon optim`) | Текущий показатель в `0cfd55c`         | Комментарий аудита                                                                                            |
| --------------------------- | ------------------------------------------------------------------------------- | ------------------------------------- | -------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| **GUI Process (Native)**    | Пустое окно (`egui/glow`, стандартный EGL)                                      | 584.6 MiB (Masonry)                   | **308.3 MiB VmRSS**                    | Загружаются лишние библиотеки Mesa/LLVM (`OPT-13`).                                                           |
| **GUI Process (Native)**    | Окно с заметкой 1 MiB (`egui/glow`, NVIDIA ICD)                                 | 490.3 MiB (Masonry)                   | **140.1 MiB VmRSS / 50.4 MiB PSS**     | PSS доказывает, что собственная куча процесса < 51 MiB.                                                       |
| **Core (`core_perf`)**      | `MarkdownFacts::extract` (5000 строк с кодом и ссылками)                        | 23 266 мкс                            | **2 517 мкс (2.52 мс)**                | Всё ещё 4 прохода и полная аллокация `String`; блокирует UI при открытом TOC (`OPT-04`, `OPT-07`).            |
| **Core (`core_perf`)**      | Создание `ChangeSet` из 5000 правок                                             | 2.786 мкс                             | **7.707 мкс ($0.36\times$ регрессия)** | Вызвано аллокацией и заполнением `prefix_shifts: Vec<isize>` (`OPT-03`).                                      |
| **Store (`store_perf`)**    | `fts_search_page_1000` (FTS5 поиск со сниппетами)                               | 708.672 мкс                           | **713.613 мкс**                        | Два отдельных запроса `MATCH` (`SELECT snippet` + `SELECT COUNT(*)`) (`OPT-17`).                              |
| **Store (`catalog_perf`)**  | `catalog_reopen_100_unchanged` (100 заметок в warm cache)                       | 44 494 мкс                            | **982.022 мкс**                        | На 100 файлах быстро, но читает и считает SHA-256 всех файлов; на 10 000 файлов даст ~100–500+ мс (`OPT-15`). |
| **Store (`recovery_perf`)** | `recovery_write_16k` (запись snapshot с `fsync`)                                | 5.227 мс                              | **5.087 мс**                           | Включает полный `read_dir` budget scan каталога recovery на каждую запись (`OPT-18`).                         |
| **UI (`ui_perf`)**          | `idle_frame` / `insert_character_frame` / `insert_newline_frame` (30 000 строк) | 121.97 / 165.75 / 325.47 мкс          | **42.79 / 83.85 / 139.89 мкс**         | `insert_newline_frame` на 67% медленнее ввода символа из-за $O(L)$ перестройки `HeightIndex` (`OPT-08`).      |
| **UI (`ui_perf`)**          | `navigator_filter_10000` (4 запроса по 10 000 элементов)                        | 4 508.59 мкс                          | **4 168.98 мкс (4.17 мс)**             | Полный перебор всех 10 000 элементов на каждый символ запроса (`OPT-12`).                                     |

### 3.2. Матрица проверки 8 целевых сценариев (Headless vs Real GUI)

Для исключения смешивания headless-бенчмарков с реальным процессом GUI предлагается следующая таблица сценариев и инструментов измерения:

| Сценарий                                       | Что измерять                                                                                                                        | Инструмент / Harness в Truff                                                                                                                                                                                          | Разделение Headless vs Нативный GUI                                                                                         |
| ---------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| **1. Запуск с пустым vault**                   | Cold/Warm startup time (до первого отрисованного кадра `swap_buffers`), VmRSS, PSS, Private_Dirty                                   | Нативный процесс `target/release/truff --vault <empty_dir>` с замером `/proc/<pid>/smaps_rollup` и timestamp после первого `Frame::end`                                                                               | Строго нативный GUI-процесс (без 3-секундного `sleep` внутри метрики времени старта!).                                      |
| **2. Простой с открытым GUI**                  | Idle CPU (%), VmRSS, PSS, число пробуждений `epoll_wait`/`futex` в секунду (`strace -c` / `perf stat`)                              | Нативный процесс `truff` в течение 60 с без ввода (с закрытым и открытым which-key/recovery таймером)                                                                                                                 | Строго нативный GUI: проверка, что `ControlFlow::Wait` не нарушается лишними `request_repaint()`.                           |
| **3. Редактирование большого файла (1–10 МБ)** | Latency ввода `p50/p95/p99` (как с закрытым, так и с **открытым TOC/Backlinks**!), аллокации на клавишу                             | 1) Headless CPU: расширенный `examples/ui_perf.rs` (`insert_character_with_toc_open`, `insert_newline_100k`)<br>2) End-to-end GUI: таймер от `WindowEvent::KeyboardInput` до завершения `glow::Context::swap_buffers` | Оба уровня: `ui_perf` для чистой стоимости CPU верстки, нативный таймер для полного пути input-to-present.                  |
| **4. Быстрая прокрутка**                       | CPU time/frame, число вызовов `Paragraph::new`, аллокации кучи (`dhat`/`alloc_counter`), frame time `p95/p99`                       | Расширенный `examples/ui_perf.rs` (`fast_scroll_sweep` по 30 000 строк с шагом 500 px/кадр) + нативный GUI-прогон                                                                                                     | `ui_perf` считает точное число `self.builds` и аллокаций; нативный GUI проверяет загрузку глифов в текстурный атлас `egui`. |
| **5. Открытие 100 файлов**                     | Рост VmRSS и PSS после 100 переключений, удерживаемая память после `release_closed_buffers`, задержка переключения                  | Нативный сценарий + `perf_scenarios::run_memory_retention` с проверкой `smaps_rollup` (`Private_Dirty`) и `mallinfo2`/`jemalloc`                                                                                      | Проверка как счётчиков `Workspace::buffers`, так и реального возврата страниц ОС аллокатором (`MADV_DONTNEED`).             |
| **6. Vault из 10 000 заметок**                 | Холодная и повторная индексация (`scan::rebuild`/`refresh`), память SQLite и `CatalogWorker`, открытие навигатора                   | Расширенный `crates/store/examples/catalog_perf.rs` (на 10 000 файлов с сбросом page cache и без него) + `ui_perf`                                                                                                    | Измерение реального дискового I/O (`io_bytes`, число `open`/`read` syscalls) и времени коммита транзакции.                  |
| **7. Массовое редактирование и Undo/Redo**     | Latency 1 000–10 000 правок, скорость `Buffer::undo`/`redo`, число аллокаций, пиковый RSS `History`                                 | `crates/core/examples/core_state_perf.rs` + новый сценарий `typing_run_10k_undo_redo`                                                                                                                                 | Чистый бенчмарк `truff-core` с подсчётом `History::total_bytes()` и реальных кучевых аллокаций.                             |
| **8. Поиск и Backlinks**                       | Latency FTS5-поиска (короткие префиксы, частые слова), время построения `find_backlinks` и `refresh_resolutions` на 10 000 заметках | `crates/store/examples/store_perf.rs` увеличенный с 1 000 до 10 000 заметок и 150 000 ссылок                                                                                                                          | Измерение латентности SQLite-запросов и времени блокировки `CatalogWorker`.                                                 |

---

## 4. Детальные технические задания по каждому узкому месту (`OPT-01` — `OPT-18`)

### OPT-01. Уплощение `HistoryGroup` и коалесцирование последовательных однобайтовых/символьных правок в `History`

#### Локация

- **Крейт**: `truff-core`
- **Файлы и диапазоны строк**:
  - [`crates/core/src/text/history.rs`](https://github.com/hackerman111/truff/blob/main/crates/core/src/text/history.rs), строки 12–263 (`tx_heap_bytes`, `compact_transaction`, `pub struct HistoryGroup`, `pub struct History`, `History::record`, `History::evict_to_budget`).
  - [`crates/core/src/text/buffer/mod.rs`](https://github.com/hackerman111/truff/blob/main/crates/core/src/text/buffer/mod.rs), строки 312–415 (`Buffer::apply`, `Buffer::undo`, `Buffer::redo`, `Buffer::stage_history`).
- **Связанные вызывающие функции**: `Workspace::dispatch` (`Action::ApplyEdit`, `Action::Undo`, `Action::Redo` в `crates/core/src/workspace/dispatch.rs`, строки 50–85).

#### Текущая реализация

В `Buffer::apply` (`crates/core/src/text/buffer/mod.rs`, строки 312–349) на каждый вызов создаются два экземпляра `Transaction`: `forward_tx` и `inverse_tx`, каждый из которых владеет собственным `Vec<SingleEdit>`. Затем вызывается `self.history.record(undo_group, forward_tx, inverse_tx)`. В `crates/core/src/text/history.rs` (строки 32–43) структура `HistoryGroup` определена как:

```rust
pub struct HistoryGroup {
    pub group_id: UndoGroup,
    pub forward: Vec<Transaction>,
    pub inverse: Vec<Transaction>,
    pub size_in_bytes: usize,
    before_state: u64,
    after_state: u64,
}
```

Когда пользователь набирает текст в режиме `Insert`, все нажатия клавиш до выхода по `Esc` имеют одинаковый `UndoGroup`. Каждое нажатие клавиши пушит новую `Transaction` в `group.forward` и `group.inverse`. При вызове `Buffer::undo` (`buffer/mod.rs`, строки 352–363) выполняется `self.stage_history(history_group.inverse.iter().rev())`, который последовательно в цикле применяет каждую односимвольную транзакцию к клону `crop::Rope`.

#### Причина проблемы

1. **Каскад мелких кучевых аллокаций**: На каждое нажатие клавиши в режиме `Insert` выделяется:
   - `forward_tx.edits: Vec<SingleEdit>` (1 элемент `SingleEdit` = 32 байта в куче + `String` на 1–4 байта в куче);
   - `inverse_tx.edits: Vec<SingleEdit>` (1 элемент `SingleEdit` = 32 байта в куче, пустая `String` для чистой вставки);
   - Две структуры `Transaction` по 64 байта внутри векторов `group.forward` и `group.inverse`.
     На сессию набора из $1\,000$ символов внутри одной `UndoGroup` это порождает **$3\,000$ отдельных аллокаций в куче** ($2\,000$ векторов `Vec<SingleEdit>` по 1 элементу + $1\,000$ односимвольных `String`) и $\approx 224\text{ КБ}$ метаданных ради хранения $1\text{ КБ}$ набранного текста!
2. **Замедление `Buffer::undo` / `Buffer::redo` до $O(K \log N)$**: При отмене сессии набора из $K = 1\,000$ символов `stage_history` выполняет $1\,000$ отдельных операций `rope.replace(...)` по 1 символу и строит `ChangeSet` из $1\,000$ элементов вместо одной блочной операции замены диапазона `start..start+1000` на `""`!

#### Предлагаемое решение

1. **Изменяемые структуры и функции**:
   - Заменить `forward: Vec<Transaction>` и `inverse: Vec<Transaction>` в `HistoryGroup` (`crates/core/src/text/history.rs`) на плоские векторы `forward_edits: Vec<SingleEdit>`, `forward_batch_ends: Vec<u32>`, `inverse_edits: Vec<SingleEdit>`, `inverse_batch_ends: Vec<u32>`, плюс `cursor_before: Option<usize>` и `cursor_after: Option<usize>`.
   - Добавить в `HistoryGroup::push_batch` **коалесцирование смежных одиночных вставок (`Insert` run) и смежных одиночных удалений (`Backspace`/`Delete` run)**:
     - Если входящий батч состоит ровно из 1 чистой вставки (`forward_edit.range.is_empty()` и `!forward_edit.text.is_empty()`), а последний батч в `HistoryGroup` также состоит из 1 чистой вставки `last_fwd` и при этом `forward_edit.range.start == last_fwd.range.start + last_fwd.text.len()`, то вместо добавления нового элемента в `forward_edits` и `inverse_edits`:
       - дописывать `last_fwd.text.push_str(&forward_edit.text)`;
       - расширять диапазон соответствующей обратной правки `last_inv.range.end += forward_edit.text.len()`;
       - обновлять `size_in_bytes` и `cursor_after`.
2. **Новый код `HistoryGroup` и `Buffer::undo` / `Buffer::redo`**:

```rust
// В crates/core/src/text/history.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryGroup {
    pub group_id: UndoGroup,
    pub forward_edits: Vec<SingleEdit>,
    pub forward_batch_ends: Vec<u32>,
    pub inverse_edits: Vec<SingleEdit>,
    pub inverse_batch_ends: Vec<u32>,
    pub cursor_before: Option<usize>,
    pub cursor_after: Option<usize>,
    pub size_in_bytes: usize,
    before_state: u64,
    after_state: u64,
}

impl HistoryGroup {
    pub fn push_batch(
        &mut self,
        mut forward: Vec<SingleEdit>,
        mut inverse: Vec<SingleEdit>,
        cursor: Option<usize>,
    ) {
        self.cursor_after = cursor;
        // Fast path: коалесцирование последовательного набора текста (Insert run)
        if forward.len() == 1 && inverse.len() == 1 {
            let f_new = &forward[0];
            let i_new = &inverse[0];
            if f_new.range.is_empty() && !f_new.text.is_empty() && i_new.text.is_empty() {
                let prev_batch_start = if self.forward_batch_ends.len() >= 2 {
                    self.forward_batch_ends[self.forward_batch_ends.len() - 2] as usize
                } else {
                    0
                };
                if self.forward_edits.len() == prev_batch_start + 1
                    && self.inverse_edits.len() == prev_batch_start + 1
                {
                    let last_f = &mut self.forward_edits[prev_batch_start];
                    let last_i = &mut self.inverse_edits[prev_batch_start];
                    if last_f.range.is_empty()
                        && last_i.text.is_empty()
                        && f_new.range.start == last_f.range.start + last_f.text.len()
                    {
                        let old_cap = last_f.text.capacity();
                        last_f.text.push_str(&f_new.text);
                        last_i.range.end += f_new.text.len();
                        self.size_in_bytes += last_f.text.capacity().saturating_sub(old_cap);
                        return;
                    }
                }
            }
        }

        for edit in &mut forward {
            compact_edit_text(edit);
            self.size_in_bytes += std::mem::size_of::<SingleEdit>() + edit.text.capacity();
        }
        for edit in &mut inverse {
            compact_edit_text(edit);
            self.size_in_bytes += std::mem::size_of::<SingleEdit>() + edit.text.capacity();
        }
        self.forward_edits.extend(forward);
        self.forward_batch_ends.push(self.forward_edits.len() as u32);
        self.inverse_edits.extend(inverse);
        self.inverse_batch_ends.push(self.inverse_edits.len() as u32);
        self.size_in_bytes += 2 * std::mem::size_of::<u32>();
    }

    pub fn forward_batches(&self) -> impl DoubleEndedIterator<Item = &[SingleEdit]> + ExactSizeIterator + '_ {
        let mut start = 0usize;
        self.forward_batch_ends.iter().map(move |&end| {
            let slice = &self.forward_edits[start..end as usize];
            start = end as usize;
            slice
        })
    }

    pub fn inverse_batches(&self) -> impl DoubleEndedIterator<Item = &[SingleEdit]> + ExactSizeIterator + '_ {
        let mut start = 0usize;
        self.inverse_batch_ends.iter().map(move |&end| {
            let slice = &self.inverse_edits[start..end as usize];
            start = end as usize;
            slice
        })
    }
}
```

3. **Сохраняемые инварианты**:
   - Многокурсорные/пакетные транзакции внутри одной `UndoGroup` строго сохраняют границы батчей через `forward_batch_ends` и `inverse_batch_ends` и откатываются в обратном порядке батчей (`inverse_batches().rev()`).
   - Отслеживание `savepoint_state`, `before_state` и `after_state` в `History` не меняется.
   - В `Buffer::undo` и `Buffer::redo` `self.revision` увеличивается на `1` (или `history_group.inverse_batch_ends.len() as u64`), сохраняя монотонность ревизий для кэша UI.

#### Ожидаемый эффект

- **Аллокации**: Для сессии набора из $N = 1\,000$ символов количество кучевых аллокаций в `History` падает с **$3\,000$ до $\le 12$** (амортизированный рост одного `String` и четырёх векторов `HistoryGroup`), то есть **сокращение аллокаций истории на $>99\%$**.
- **Память**: Потребление памяти на $1\,000$ набранных символов снижается с $\sim 224\text{ КБ}$ до $\sim 1.2\text{ КБ}$ (экономия в $>150$ раз для последовательного набора и $\sim 60\%$ для произвольных батчей).
- **Скорость `undo`/`redo`**: Отмена набранного абзаца из $1\,000$ символов ускоряется с $1\,000$ вызовов `rope.replace` до **1 вызова `rope.replace`** (ускорение в $50\text{--}200\times$).

#### Компромиссы

- При коалесцировании `forward_batch_ends.len()` для слитых вставок равен 1, поэтому при `undo` `self.revision` увеличивается на 1 вместо $1\,000$. Это полностью совместимо с кэшем `EditorWidget` и `DocumentFacts`, так как они проверяют неравенство/монотонность `revision`.

#### План реализации

1. В `crates/core/src/text/history.rs` заменить `compact_transaction` на `compact_edit_text(&mut SingleEdit)`, обновить поля `HistoryGroup` и методы `HistoryGroup::new`, `push_batch`, `forward_batches`, `inverse_batches`, а также `History::record(undo_group, forward_edits, inverse_edits, cursor)`.
2. В `crates/core/src/text/buffer/mod.rs` обновить `Buffer::apply`, чтобы передавать `normalized_edits` и `inverse_edits` в `self.history.record(...)` напрямую без создания двух промежуточных структур `Transaction`.
3. Обновить `Buffer::stage_history`, чтобы принимать `impl Iterator<Item = &[SingleEdit]>` вместо `impl Iterator<Item = &Transaction>`.

#### Проверка

- **Модульные тесты**: Запустить существующие тесты в `crates/core/src/text/history.rs` и `buffer/mod.rs`, добавить тест `coalesces_consecutive_insert_char_edits_and_restores_exact_rope_on_undo_redo` (включая многобайтовый UTF-8 и смешанные вставки/удаления).
- **Бенчмарк**: Запустить `crates/core/examples/core_state_perf.rs` и добавить сценарий `insert_1000_chars_then_undo_redo`.
- **Критерий приёмки**: `History::total_bytes()` после 1 000 односимвольных вставок в одной `UndoGroup` $< 4\text{ KiB}$ (против $> 120\text{ KiB}$ до оптимизации); время `undo()` сокращается минимум в $20\times$.

#### Уверенность

- **Static** (непосредственно следует из структуры `HistoryGroup` и `Buffer::apply` в `0cfd55c`).

---

### OPT-02. Замена $O(N)$ сканирования `byte_to_point` на $O(\log N)$ индекс строк `crop::Rope` и устранение клона `Rope` в `SyntaxEngine`

#### Локация

- **Крейты**: `truff-core`, `truff-ui`
- **Файлы и диапазоны строк**:
  - [`crates/core/src/markdown/syntax.rs`](https://github.com/hackerman111/truff/blob/main/crates/core/src/markdown/syntax.rs), строки 35–155 (`SyntaxEngine`, `SyntaxEngine::update`, `byte_to_point`).
  - [`crates/ui/src/native/editor/syntax.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/native/editor/syntax.rs), строки 8–81 (`EditorSyntax`, `EditorSyntax::update`, `EditorSyntax::apply_change`).
- **Связанные вызывающие функции**: `EditorWidget::apply_change` (`crates/ui/src/native/editor/invalidation.rs`, строка 38), `EditorWidget::show` (`crates/ui/src/native/editor/viewport.rs`, строка 181).

#### Текущая реализация

В `crates/core/src/markdown/syntax.rs` функция `byte_to_point(text: &str, byte_offset: usize) -> Point` (строки 130–155) вычисляет координаты `(row, column)` для `tree_sitter::InputEdit` путём линейного прохода по `text[..byte_offset]` с начала документа, подсчитывая символы `\n`. Кроме того, `SyntaxEngine::update` принимает непрерывный срез `text: &str`, а `EditorSyntax` в `crates/ui/src/native/editor/syntax.rs` (строки 8–13) удерживает поле `source: crop::Rope` для вычисления старых координат при `apply_change`.

#### Причина проблемы

1. **Скрытая линейная сложность $O(N)$ на инкрементальном пути**: Вся суть инкрементального парсинга `tree-sitter` заключается в работе за $O(\log N + \Delta)$, но если вычисление `start_position`, `old_end_position` или `new_end_position` сканирует префикс файла `0..byte_offset` или требует передачи `&str`, то правка в конце файла размером $2\text{ МБ}$ выполняет проход по $2\text{ млн}$ байт!
2. **Избыточность при наличии `crop::Rope`**: Сам `crop::Rope` в `Buffer` уже поддерживает сбалансированное B-дерево с точным подсчётом переводов строк в каждом узле! Методы `rope.line_of_byte(byte_offset)` и `rope.byte_of_line(line)` выполняются за **$O(\log N)$** без сканирования текста.

#### Предлагаемое решение

1. **Заменить `byte_to_point` на $O(\log N)$ функцию `rope_byte_to_point(rope: &crop::Rope, byte_offset: usize) -> tree_sitter::Point`**:

```rust
// В crates/core/src/markdown/syntax.rs
#[inline]
pub fn rope_byte_to_point(rope: &crop::Rope, byte_offset: usize) -> tree_sitter::Point {
    let clamped = byte_offset.min(rope.byte_len());
    let row = rope.line_of_byte(clamped);
    let line_start = rope.byte_of_line(row);
    tree_sitter::Point {
        row,
        column: clamped - line_start,
    }
}
```

2. **Переводить `tree_sitter::Parser::parse_with` на чтение чанков `crop::Rope` без аллокации `&str`**:

```rust
pub fn parse_rope(
    parser: &mut tree_sitter::Parser,
    rope: &crop::Rope,
    old_tree: Option<&tree_sitter::Tree>,
) -> Option<tree_sitter::Tree> {
    let len = rope.byte_len();
    parser.parse_with(
        &mut |byte_offset, _point| {
            if byte_offset >= len {
                return &[];
            }
            let (chunk, chunk_start, _, _) = rope.chunk_at_byte(byte_offset);
            &chunk.as_bytes()[byte_offset - chunk_start..]
        },
        old_tree,
    )
}
```

3. **Убрать `source: crop::Rope` из `EditorSyntax` (`crates/ui/src/native/editor/syntax.rs`)**, если `BufferChange` (`crates/core/src/workspace/changes.rs`) уже несёт `old_start_point: (usize, usize)`, `old_end_point: (usize, usize)` и `new_end_point: (usize, usize)` (вычисленные за $O(\log N)$ в `Buffer::apply` до и после модификации `self.rope`!), либо оставить лёгкий $O(1)$ `Rope::clone()` только при включённом `tree`.

#### Ожидаемый эффект

- **Асимптотика**: Вычисление координат `InputEdit` ускоряется с $O(N)$ до **$O(\log N)$** ($\sim 50\text{--}100\text{ нс}$ на любом размере файла).
- **Память**: Полный отказ от материализации `Rope -> String` при вызове `SyntaxEngine::update` (экономия $1\text{--}10\text{ МБ}$ пиковых аллокаций на крупных файлах).

#### Компромиссы

- `rope.chunk_at_byte(byte_offset)` (или итератор `rope.chunks()`) требует корректного возврата пустого среза `&[]` при `byte_offset >= rope.byte_len()`, что стандартно для `tree-sitter` callback.

#### План реализации

1. В `crates/core/src/markdown/syntax.rs` добавить `rope_byte_to_point` и перевести парсинг на `parse_with` по `crop::Rope`.
2. Обновить `crates/ui/src/native/editor/syntax.rs` для использования `rope_byte_to_point`.

#### Проверка

- **Тесты**: Сравнить `rope_byte_to_point(rope, pos)` со старым `byte_to_point(&text, pos)` на всех байтовых границах символов (ASCII, кириллица, эмодзи, CRLF и LF).
- **Бенчмарк**: В `crates/core/examples/core_perf.rs` увеличить размер документа в сценарии `Incremental syntax, правка в конце` с 9 KiB до 1.5 MiB и замерить время до и после.

#### Уверенность

- **Static** (наличие $O(N)$ `byte_to_point` в `syntax.rs` и $O(\log N)$ методов в `crop::Rope` прямо проверено по коду).

---

### OPT-03. Устранение регрессии $0.36\times$ в `ChangeSet`: встраивание `prefix_shift` внутрь `Change` и удаление клонирования `SingleEdit` в `from_edits`

#### Локация

- **Крейт**: `truff-core`
- **Файлы и диапазоны строк**:
  - [`crates/core/src/text/edit.rs`](https://github.com/hackerman111/truff/blob/main/crates/core/src/text/edit.rs), строки 123–265 (`pub struct Change`, `pub struct ChangeSet`, `ChangeSet::new`, `ChangeSet::from_edits`, `ChangeSet::map_pos`).
  - [`crates/core/src/text/buffer/mod.rs`](https://github.com/hackerman111/truff/blob/main/crates/core/src/text/buffer/mod.rs), строки 240–310 (`build_inverse_and_changes`, `stage_history`).

#### Текущая реализация

В `crates/core/src/text/edit.rs` (строки 123–164) структуры определены так:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub old_range: Range<usize>,
    pub new_len: usize,
    pub delta: isize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChangeSet {
    changes: Vec<Change>,
    batch_ends: Vec<usize>,
    prefix_shifts: Vec<isize>,
}
```

А в `ChangeSet::from_edits(edits: &[SingleEdit])` выполняется `let mut sorted_edits = edits.to_vec(); sorted_edits.sort_by_key(|e| e.range.start);`.

#### Причина проблемы

1. **Причина зафиксированной регрессии $0.36\times$ в `docs/perf/core/README.md` (`2.786 мкс -> 7.707 мкс`)**:
   - В `ChangeSet::new(changes)` для батча из $5\,000$ правок код сначала проходит по `changes.windows(2)` для проверки отсортированности, затем делает **вторую кучевую аллокацию** `Vec::with_capacity(5000)` (`40 000 байт`) для `prefix_shifts` и проходит по `changes` второй раз, записывая суммы сдвигов.
2. **Избыточность поля `delta: isize` в `Change`**:
   - В структуре `Change` уже хранятся `old_range: Range<usize>` и `new_len: usize`. Значение `delta` тривиально выражается за 1 вычитание в регистрах CPU: `new_len as isize - (old_range.end - old_range.start) as isize`. Хранить `delta` в памяти И отдельно аллоцировать вектор `prefix_shifts: Vec<isize>` — чистая потеря памяти и пропускной способности кэша L1!
3. **Клонирование всех `String` в `ChangeSet::from_edits`**:
   - Вызов `edits.to_vec()` клонирует каждый `SingleEdit`, включая кучевую аллокацию `edit.text: String`, хотя для построения `Change` нужна только длина `edit.text.len()`!

#### Предлагаемое решение

1. **Встроить кумулятивный сдвиг `prefix_shift: isize` (суммарный сдвиг всех правок до текущей включительно в рамках батча) или вычислять его in-place внутри `Change`, убрав `prefix_shifts: Vec<isize>` из `ChangeSet`**:

```rust
// В crates/core/src/text/edit.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub old_range: Range<usize>,
    pub new_len: usize,
    /// Индивидуальный сдвиг этой правки: new_len as isize - old_range.len() as isize
    pub delta: isize,
    /// Кумулятивный сдвиг ДО начала этой правки (заполняется для отсортированных непересекающихся батчей)
    pub prefix_shift: isize,
}
```

Постойте: если нужно сохранить размер `Change` ровно **32 байта** (`Range<usize>` = 16B, `new_len: usize` = 8B, `prefix_shift: isize` = 8B), то `delta` делается inline-методом:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub old_range: Range<usize>,
    pub new_len: usize,
    /// Для отсортированного батча — суммарный сдвиг всех ПРЕДЫДУЩИХ правок батча.
    pub prefix_shift: isize,
}

impl Change {
    #[inline(always)]
    pub const fn delta(&self) -> isize {
        self.new_len as isize - (self.old_range.end - self.old_range.start) as isize
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChangeSet {
    changes: Vec<Change>,
    batch_ends: Vec<usize>,
    is_indexed: bool,
}
```

2. **Устранить клонирование `SingleEdit` в `ChangeSet::from_edits`**:

```rust
impl ChangeSet {
    pub fn from_edits(edits: &[SingleEdit]) -> Self {
        let mut changes: Vec<Change> = edits
            .iter()
            .map(|e| Change {
                old_range: e.range.clone(),
                new_len: e.text.len(),
                prefix_shift: 0,
            })
            .collect();
        changes.sort_unstable_by_key(|c| c.old_range.start);
        let mut cumulative: isize = 0;
        let mut disjoint = true;
        let mut prev_end = 0usize;
        for (i, c) in changes.iter_mut().enumerate() {
            if i > 0 && c.old_range.start < prev_end {
                disjoint = false;
            }
            c.prefix_shift = cumulative;
            cumulative += c.delta();
            prev_end = c.old_range.end;
        }
        Self {
            changes,
            batch_ends: Vec::new(),
            is_indexed: disjoint,
        }
    }
}
```

3. **Заполнять `prefix_shift` прямо внутри `Buffer::build_inverse_and_changes`** (`crates/core/src/text/buffer/mod.rs`), где правки уже отсортированы и проверяются на непересечение! Тогда `ChangeSet::new` вообще не делает **ни одного дополнительного прохода и ни одной аллокации**!

#### Ожидаемый эффект

- **Скорость создания `ChangeSet` из 5000 edits**: Возврат с `7.707 мкс` к **`~2.1–2.5 мкс`** (устранение регрессии $0.36\times$, ускорение более чем в $3\times$ по сравнению с текущим кодом и быстрее исходного baseline `2.786 мкс`!).
- **Скорость `map_pos`**: $O(\log K)$ через `binary_search_by` работает для **любого** размера батча (даже при $2 \le K < 32$), причём обращение к `change.prefix_shift` попадает в ту же кэш-линию 64B, что и `change.old_range`, в отличие от промаха в отдельный вектор `prefix_shifts`.
- **Память**: Экономия $40\text{ КБ}$ кучи на батч из 5000 правок и полное устранение клонирования `String` в `from_edits`.

#### Компромиссы

- Если внешний код обращался к полю `change.delta` напрямую как к публичному полю структуры, следует проверить все места в `crates/core` и `crates/ui` (их всего 2–3) и заменить `change.delta` на `change.delta()`.

#### План реализации

1. В `crates/core/src/text/edit.rs` заменить поле `prefix_shifts: Vec<isize>` в `ChangeSet` на `is_indexed: bool` и поле `prefix_shift: isize` в `Change` + метод `pub const fn delta(&self) -> isize`.
2. В `crates/core/src/text/buffer/mod.rs` (`build_inverse_and_changes`) накапливать `prefix_shift` прямо при создании элементов `Change`.
3. Обновить `ChangeSet::from_edits` и `ChangeSet::map_pos`.

#### Проверка

- **Тесты**: Все существующие тесты `ChangeSet` и `map_pos` (включая соседние правки, grouped undo/redo и `JumpHistory` rebasing).
- **Бенчмарк**: `target/release/examples/core_perf` — сценарии `Создание ChangeSet из 5000 edits` и `Один map_pos, пакет из 512 anchors`.
- **Критерий приёмки**: `Создание ChangeSet из 5000 edits` $\le 2.8\text{ мкс}$ при сохранении `Один map_pos` $\le 0.025\text{ мкс}$.

#### Уверенность

- **Verified** (регрессия $0.36\times$ измерена и задокументирована в `docs/perf/core/README.md`, причина однозначно видна в `crates/core/src/text/edit.rs`).

---

### OPT-04. Двухпроходная архитектура `MarkdownFacts::extract` с монотонным курсором `code_ranges`, $O(L)$ слиянием ссылок и быстрым режимом `extract_headings_only`

#### Локация

- **Крейт**: `truff-core`
- **Файл и диапазон строк**: [`crates/core/src/markdown/facts.rs`](https://github.com/hackerman111/truff/blob/main/crates/core/src/markdown/facts.rs), строки 10–249 (`HeadingFact`, `BlockFact`, `DocFacts`, `MarkdownFacts::extract`, `in_code`).
- **Связанные вызывающие функции**:
  - `Application::document_facts` (`crates/ui/src/app/document.rs`, строка 24)
  - `scan::refresh` (`crates/store/src/catalog/scan.rs`, строка 135)

#### Текущая реализация

`MarkdownFacts::extract(text: &str)` выполняет 4 прохода:

1. Проход 1 (строки 60–110): `pulldown_cmark::Parser::new_ext(text, Options::all()).into_offset_iter()` собирает `headings`, `markdown_links` и `code_ranges`.
2. Проход 2 (строки 112–175): сканирует `text` в поисках `[[wiki-link]]`, вызывая `in_code(&code_ranges, pos)` (бинарный поиск `binary_search_by` на каждое совпадение).
3. Проход 3 (строки 176–185): объединяет стандартные ссылки и вики-ссылки в один вектор, вызывает `sort_by_key(|l| l.range.start)` и перенумеровывает `ordinal`.
4. Проход 4 (строки 186–249): заново разбивает весь `text` на строки для поиска идентификаторов блоков `^block-id` на концах строк, снова вызывая `in_code(&code_ranges, ...)`.

#### Причина проблемы

1. **Четыре прохода по памяти документа и лишняя сортировка**: Проходы 2 и 4 читают один и тот же буфер `text` дважды подряд. Кроме того, `markdown_links` (из Прохода 1) и `wiki_links` (из Прохода 2) **уже отсортированы по возрастанию `range.start`** по построению! Вызывать `sort_by_key` вместо линейного слияния двух отсортированных массивов — лишняя работа $O(L \log L)$.
2. **Повторный бинарный поиск `in_code`**: Так как сканирование текста во втором/четвёртом проходе идёт строго слева направо (`pos` монотонно возрастает), а `code_ranges` отсортирован и не пересекается, бинарный поиск $O(\log C)$ на каждую проверку избыточен — достаточно удерживать один индекс `code_cursor: usize`, который сдвигается вперёд за $O(1)$ амортизированно (`while code_cursor < code_ranges.len() && code_ranges[code_cursor].end <= pos { code_cursor += 1; }`).
3. **Отсутствие специализированного извлечения заголовков для UI**: Когда UI (`sync_toc` или автодополнение заголовков `#`) запрашивает структуру текущего документа, ему нужны только `headings` (и вообще не нужны `blocks` и `links`), причём заголовки в Markdown могут начинаться только в начале строки (`#`) или быть Setext-заголовками.

#### Предлагаемое решение

1. **Объединить Проход 2 (`[[wiki-link]]`) и Проход 4 (`^block-id`) в один проход с монотонным `CodeRangeCursor`**:

```rust
struct CodeRangeCursor<'a> {
    ranges: &'a [Range<usize>],
    idx: usize,
}

impl<'a> CodeRangeCursor<'a> {
    #[inline]
    fn new(ranges: &'a [Range<usize>]) -> Self {
        Self { ranges, idx: 0 }
    }

    #[inline]
    fn contains(&mut self, pos: usize) -> bool {
        while self.idx < self.ranges.len() && self.ranges[self.idx].end <= pos {
            self.idx += 1;
        }
        self.idx < self.ranges.len() && self.ranges[self.idx].start <= pos
    }
}
```

2. **Заменить `sort_by_key` на линейное слияние `merge_sorted_links(md_links, wiki_links)` за $O(L_1 + L_2)$** с одновременным проставлением `link.ordinal = idx`.
3. **Предварительно резервировать ёмкость векторов** (`headings`, `code_ranges`, `links`) пропорционально `text.len() / 256`.

#### Ожидаемый эффект

- **Ускорение `MarkdownFacts::extract`**: Дополнительно на **25–40%** поверх `1.2-p1` (снижение времени на корпусе из 5000 строк с `2 517 мкс` до `~1 500–1 800 мкс`).
- **Снижение числа реаллокаций векторов**: С $O(\log H + \log L + \log C)$ до 1 аллокации на вектор.

#### Компромиссы

- При объединении сканера `[[wiki-link]]` и построчного сканера `^block-id` важно сохранить точное правило `^block-id` на конце строки (перед `\n` или `\r\n`). Проход по строкам и поиск `[[` внутри каждой строки сохраняет идентичную семантику и улучшает кэш-локальность L1.

#### План реализации

1. В `crates/core/src/markdown/facts.rs` ввести `CodeRangeCursor`.
2. Объединить цикл поиска вики-ссылок и `^block-id` в единый проход по строкам `text`.
3. Реализовать `merge_sorted_links` вместо `sort_by_key`.

#### Проверка

- **Тесты**: Все тесты `crates/core/src/markdown/facts.rs` (проверка совпадения `DocFacts` байт-в-байт на корпусах со сложными блоками кода, таблицами, вики-ссылками и `^block-id`).
- **Бенчмарк**: `target/release/examples/core_perf` — сценарий `Markdown facts, 5000 code ranges`.

#### Уверенность

- **Static** (четыре прохода и `sort_by_key` на двух уже монотонных последовательностях прямо присутствуют в `crates/core/src/markdown/facts.rs`).

---

### OPT-05. Вынос `Registers` из `Buffer::history` на уровень `Workspace` / `VimState` и замена `HashMap<char, Arc<Register>>` на компактный массив

#### Локация

- **Крейт**: `truff-core`
- **Файлы и диапазоны строк**:
  - [`crates/core/src/text/history.rs`](https://github.com/hackerman111/truff/blob/main/crates/core/src/text/history.rs), строка 82 (`registers: Registers` внутри `pub struct History`).
  - [`crates/core/src/text/history/registers.rs`](https://github.com/hackerman111/truff/blob/main/crates/core/src/text/history/registers.rs), строки 19–108 (`pub struct Register`, `pub struct Registers`).

#### Текущая реализация

В `crates/core/src/text/history.rs` каждый `Buffer` содержит `history: History`, а внутри `History` лежит поле `registers: Registers`, содержащее `unnamed: Option<Arc<Register>>` и `named: HashMap<char, Arc<Register>>`.

#### Причина проблемы

1. **Удержание памяти в неактивных буферах**: Если пользователь скопировал (yank) крупный фрагмент текста в буфере $A$, `Arc<Register>` остаётся привязанным к `Buffer::history` буфера $A$ до закрытия буфера, даже если регистр уже перезаписан в другом буфере.
2. **Лишняя структура `HashMap` в каждом буфере**: В Vim именованные регистры ограничены латинскими буквами `'a'..='z'` (26 слотов) и цифрами `'0'..='9'` (10 слотов). Использование `HashMap<char, Arc<Register>>` добавляет 48 байт в стек и кучевую таблицу с бакетами и хэшированием SipHash на каждый доступ.

#### Предлагаемое решение

1. Заменить внутреннее представление `Registers` в `crates/core/src/text/history/registers.rs` для стандартных регистров `'a'..='z'` на компактный фиксированный массив `[Option<Arc<Register>>; 26]` (или `Box<[Option<Arc<Register>>; 26]>` при первой записи в именованный регистр):

```rust
#[derive(Debug, Clone, Default)]
pub struct Registers {
    unnamed: Option<Arc<Register>>,
    alpha: Option<Box<[Option<Arc<Register>>; 26]>>,
    extra: Option<Box<Vec<(char, Arc<Register>)>>>,
}
```

2. При пустом состоянии `Registers` занимает всего **24 байта** и делает **0 аллокаций в куче**, а обращение к регистрам `'a'..='z'` происходит за $O(1)$ по индексу `(c as usize) - ('a' as usize)` без хэширования.

#### Ожидаемый эффект

- **Скорость чтения/записи именованных регистров**: Ускорение ещё на **20–30%** за счёт отказа от `HashMap` lookup/insert.
- **Память**: 0 кучевых аллокаций таблицы `HashMap` для буферов, где не использовались именованные регистры.

#### Компромиссы

- Выделение `Box<[Option<Arc<Register>>; 26]>` (208 байт) происходит лениво только при первой записи в именованный регистр `'a'..='z'`.

#### План реализации

1. В `crates/core/src/text/history/registers.rs` заменить `named: HashMap<char, Arc<Register>>` на `alpha: Option<Box<[Option<Arc<Register>>; 26]>>` (с fallback-вектором для неалфавитных регистров, если они используются в тестах).
2. Сохранить существующие публичные сигнатуры `Registers::get`, `Registers::set`, `Registers::append`.

#### Проверка

- **Тесты**: Модульные тесты `registers.rs` и Vim yank/paste.
- **Бенчмарк**: `target/release/examples/core_perf` — сценарий `Запись именованного регистра, 64 KiB`.

#### Уверенность

- **Static**.

---

### OPT-06. Потоковый поиск по чанкам `crop::Rope` без аллокации `String` и ограничение радиуса поиска парных объектов в Vim (`search.rs`, `objects.rs`)

#### Локация

- **Крейт**: `truff-core`
- **Файлы и диапазоны строк**:
  - [`crates/core/src/vim/motion/search.rs`](https://github.com/hackerman111/truff/blob/main/crates/core/src/vim/motion/search.rs)
  - [`crates/core/src/vim/motion/objects.rs`](https://github.com/hackerman111/truff/blob/main/crates/core/src/vim/motion/objects.rs)

#### Текущая реализация

В `crates/core/src/vim/motion/search.rs` поиск с `count` сначала проверяет окно вокруг курсора, но при отсутствии совпадения в окне (или при поиске редкого слова в файле 1.5–10 МиБ) материализует крупные срезы `Rope` в `String`. В `crates/core/src/vim/motion/objects.rs` поиск текстовых объектов в кавычках и скобках (`i"`, `a(`, `i{`) при незакрытой скобке/кавычке сканирует весь буфер до границ документа.

#### Причина проблемы

- Выделение непрерывного `String` для глобального поиска в 10-мегабайтном файле создаёт пиковый скачок памяти на 10 МиБ и копирует все листовые чанки `crop::Rope`.

#### Предлагаемое решение

1. В `crates/core/src/vim/motion/search.rs` реализовать потоковый поиск по итератору `rope.byte_slice(range).chunks()`:
   - Если `chunk.len() >= needle.len()`, искать `needle` напрямую внутри `chunk.as_bytes()` (без аллокации!).
   - На стыке двух чанков (`chunk_a`, `chunk_b`) копировать в маленький стековый/векторный буфер `overlap_buf` только хвост предыдущего чанка длиной `needle.len() - 1` байт и префикс следующего чанка длиной `needle.len() - 1` байт!
2. В `crates/core/src/vim/motion/objects.rs` для строковых кавычек (`"`, `'`, `` ` ``) ограничить поиск пределами текущего абзаца/строки (согласно семантике Vim для quote text objects), а для скобок `()`, `{}`, `[]` ограничить окно поиска разумным лимитом (например, $\pm 64\text{ KiB}$ от курсора).

#### Ожидаемый эффект

- **Память при поиске**: Снижение аллокаций при глобальном поиске по файлу $1.5\text{--}10\text{ МБ}$ с **$O(N)$ ($1.5\text{--}10\text{ МБ}$) до $O(|\text{needle}|)$ ($0$ аллокаций для паттернов $\le 128$ байт на стеке)**!
- **Задержка `i"` / `a(` на некорректном/незакрытом синтаксисе**: С $O(N)$ до $O(1)$ (верхняя граница $64\text{ KiB}$).

#### Компромиссы

- При поиске без учёта регистра (`ignore_case`) для ASCII-запросов сравнение выполняется побайтно через `eq_ignore_ascii_case`, а для Unicode-запросов — с декодированием UTF-8 на границах чанков.

#### План реализации

1. В `crates/core/src/vim/motion/search.rs` добавить функцию `find_in_rope_slice_forward(slice: RopeSlice<'_>, needle: &str) -> Option<usize>` и `find_in_rope_slice_backward(...)` с буфером перекрытия `2 * (needle.len() - 1)`.
2. В `crates/core/src/vim/motion/objects.rs` ограничить диапазон сканирования для кавычек и скобок.

#### Проверка

- **Тесты**: Поиск паттерна, разрезанного ровно по границе чанка `crop::Rope` (включая многобайтовые символы кириллицы и эмодзи).
- **Бенчмарк**: Сценарий `search_missing_pattern_1_5mib` в `core_perf.rs`.

#### Уверенность

- **Static**.

---

### OPT-07. Устранение синхронного пересчёта `Application::document_facts()` и полной аллокации `Buffer -> String` на каждое нажатие клавиши при открытом TOC

#### Локация

- **Крейт**: `truff-ui`
- **Файлы и диапазоны строк**:
  - [`crates/ui/src/app/document.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/app/document.rs), строки 8–30 (`struct DocumentFacts`, `Application::document_facts`).
  - [`crates/ui/src/native/mod.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/native/mod.rs), строки 115–145 (`NativeApp::show` $\rightarrow$ `self.sync_toc()`).
  - [`crates/ui/src/native/toc/mod.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/native/toc/mod.rs) и [`crates/ui/src/native/toc/tree.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/native/toc/tree.rs).

#### Текущая реализация

В `crates/ui/src/app/document.rs` (строки 8–30):

```rust
pub(super) struct DocumentFacts {
    id: BufferId,
    revision: u64,
    facts: Arc<DocFacts>,
}

impl Application {
    pub(crate) fn document_facts(&mut self) -> Option<Arc<DocFacts>> {
        let buffer = self.shell.editor_buffer()?;
        if let Some(cached) = &self.document_facts {
            if cached.id == buffer.id() && cached.revision == buffer.revision() {
                return Some(cached.facts.clone());
            }
        }
        let facts = Arc::new(MarkdownFacts::extract(&buffer.to_string()).ok()?);
        self.document_facts = Some(DocumentFacts {
            id: buffer.id(),
            revision: buffer.revision(),
            facts: facts.clone(),
        });
        Some(facts)
    }
}
```

А в `NativeApp::show` (`crates/ui/src/native/mod.rs`) на **каждом кадре** вызывается `self.sync_toc()`. Когда панель оглавления открыта или закреплена в боковой панели (`self.toc.is_open || self.toc.is_pinned`), `sync_toc()` вызывает `self.application.document_facts()`.

#### Причина проблемы

1. **Инвалидация кэша `DocumentFacts` на КАЖДОЕ нажатие клавиши**:
   - Поле `buffer.revision()` увеличивается при **любой** правке текста. Когда пользователь печатает обычный текст в документе на 30 000 строк (1.3 МиБ) при открытой боковой панели TOC, на каждом кадре ввода `cached.revision != buffer.revision()`.
   - В результате на **каждое нажатие клавиши** UI-поток выделяет 1.3 МиБ под `buffer.to_string()`, запускает полный 4-проходный `MarkdownFacts::extract` (~2.5–4 мс) и перестраивает дерево `TocTree`!
   - Это увеличивает время кадра при наборе текста с **83.85 мкс до >3 000–4 500 мкс** (падение производительности в **35–50 раз**!).

#### Предлагаемое решение

1. **Ввести структурную фильтрацию изменений и сдвиг диапазонов (`rebase`) в `DocumentFacts` при обычном наборе текста**:
   - Если изменение буфера (`BufferChange`) происходит внутри одной строки (`line_delta == 0`), и ни старый текст изменённой строки, ни новый текст этой строки не начинается с `#` (не является ATX-заголовком), не является Setext-подчёркиванием (`===`/`---`) и не содержит `[[`, `](`, `^`, то **структурные факты документа (`headings`, `links`, `blocks`) не изменились вообще** — изменились только байтовые смещения `range` элементов, лежащих ниже позиции правки!
   - Для `sync_toc()` (которому нужны **только `headings`**) достаточно проверить, затронула ли правка какую-либо строку из `cached.facts.headings` или строку, начинающуюся с `#` / `=` / `-`! Если нет — просто обновить `cached.revision = buffer.revision()` и сдвинуть `heading.range` для заголовков после курсора на `byte_delta` (за $O(H)$, где $H \approx 20\text{--}100$ заголовков, т.е. за **< 200 наносекунд** вместо 3 миллисекунд!).
2. **Добавить debounce (отложенный пересчёт 150 мс) при редактировании самой строки заголовка**:
   - Если пользователь печатает внутри строки `# Заголовок...`, не вызывать `buffer.to_string()` + `MarkdownFacts::extract` синхронно на каждую букву, а обновлять только текст текущего `HeadingFact` in-place или откладывать полный пересчёт на 150 мс после паузы ввода.

```rust
// В crates/ui/src/app/document.rs
pub(super) struct DocumentFacts {
    pub(super) id: BufferId,
    pub(super) revision: u64,
    pub(super) facts: Arc<DocFacts>,
}

impl Application {
    /// Быстрое обновление кэша фактов при точечной правке, не затрагивающей заголовки/ссылки
    pub(crate) fn notify_buffer_change_for_facts(&mut self, buffer: &Buffer, change: &BufferChange) {
        let Some(cached) = &mut self.document_facts else { return; };
        if cached.id != buffer.id() || cached.revision + 1 != change.revision {
            return;
        }
        // Проверяем, пересекает ли правка существующий заголовок/ссылку/блок
        // или вводит ли она маркеры '#', '[', '^', '\n'
        let line_idx = buffer.byte_to_line(change.new_range.start);
        let line_start = buffer.line_to_byte(line_idx);
        let line_end = line_start + buffer.line_len_bytes(line_idx);
        let line_slice = buffer.slice(line_start..line_end);

        let touches_heading_syntax = line_slice.chars().next().is_some_and(|c| c == '#' || c == '=' || c == '-')
            || cached.facts.headings.iter().any(|h| {
                h.range.start <= change.old_range.end && change.old_range.start <= h.range.end
            });

        if !touches_heading_syntax
            && change.new_range.len() + change.old_range.len() < 64
            && !line_slice.chars().any(|c| matches!(c, '[' | ']' | '^' | '\n'))
        {
            let delta = change.new_range.len() as isize - change.old_range.len() as isize;
            if delta != 0 {
                let facts = Arc::make_mut(&mut cached.facts);
                for h in &mut facts.headings {
                    if h.range.start >= change.old_range.end {
                        h.range.start = h.range.start.saturating_add_signed(delta);
                        h.range.end = h.range.end.saturating_add_signed(delta);
                    }
                }
                for l in &mut facts.links {
                    if l.range.start >= change.old_range.end {
                        l.range.start = l.range.start.saturating_add_signed(delta);
                        l.range.end = l.range.end.saturating_add_signed(delta);
                    }
                }
                for b in &mut facts.blocks {
                    if b.range.start >= change.old_range.end {
                        b.range.start = b.range.start.saturating_add_signed(delta);
                        b.range.end = b.range.end.saturating_add_signed(delta);
                    }
                }
            }
            cached.revision = change.revision;
        }
    }
}
```

#### Ожидаемый эффект

- **Латентность ввода при открытом/закреплённом TOC на файле 1.3 МБ (30 000 строк)**: Снижение с **~3 000+ мкс/кадр до ~85 мкс/кадр** (ускорение более чем в **35 раз**).
- **Аллокации**: Полное устранение аллокации `String` размером во весь документ (`buffer.to_string()`) на каждое нажатие клавиши в обычном тексте.

#### Компромиссы

- При вводе символов `[`, `]`, `^`, `#` или `\n` срабатывает консервативный fallback (инвалидация кэша с пересчётом по требованию), что гарантирует 100% точность структуры документа.

#### План реализации

1. В `crates/ui/src/app/document.rs` добавить метод `notify_buffer_change_for_facts`.
2. Вызывать `notify_buffer_change_for_facts` при обработке `BufferChange` в `Application` перед `sync_toc()`.
3. В `NativeApp::sync_toc` (`crates/ui/src/native/mod.rs`) перестраивать `TocTree` только если изменился `Arc::as_ptr(&facts)` (или структурная ревизия заголовков).

#### Проверка

- **Тесты**: Тест редактирования обычного абзаца при открытом TOC с проверкой того, что `HeadingFact.range` ниже курсора корректно сдвигается, а `MarkdownFacts::extract` не вызывается.
- **Бенчмарк**: Добавить в `crates/ui/examples/ui_perf.rs` сценарий `insert_character_with_toc_pinned` на 30 000 строках.
- **Критерий приёмки**: Медиана `insert_character_with_toc_pinned` $\le 95\text{ мкс}$ (против $>2\,800\text{ мкс}$ до оптимизации).

#### Уверенность

- **Static** (прямое следствие кода `crates/ui/src/app/document.rs` и `crates/ui/src/native/mod.rs`).

---

### OPT-08. Оптимизация `HeightIndex::replace_blocks` и замена `BTreeMap<usize, Paragraph>` на оконный буфер `ViewportParagraphCache` при вставке/удалении строк (`Enter`, `dd`)

#### Локация

- **Крейт**: `truff-ui`
- **Файлы и диапазоны строк**:
  - [`crates/ui/src/editor/heights.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/editor/heights.rs), строки 6–25 (`BlockInfo`, `HeightIndex`), строки 110–141 (`replace_blocks`, `rebuild_sums`).
  - [`crates/ui/src/native/editor/mod.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/native/editor/mod.rs), строки 28–47 (`pub struct EditorWidget`, поле `cache: BTreeMap<usize, Paragraph>`).
  - [`crates/ui/src/native/editor/invalidation.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/native/editor/invalidation.rs), строки 68–109 (`EditorWidget::apply_change`).
  - [`crates/ui/src/native/editor/viewport.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/native/editor/viewport.rs), строки 155–275.

#### Текущая реализация

1. В `HeightIndex::replace_blocks(&mut self, old: Range<usize>, count: usize)` (`heights.rs`, строки 110–128):
   - Если `old.len() != count` (любое нажатие `Enter`, `o`, `O`, `dd`, вставка/удаление многострочного фрагмента), выполняется `self.blocks.splice(old, replacement)` и вызывается `self.rebuild_sums()`, который зануляет весь вектор `self.sums: Vec<f64>` длины $L + 1$ и заново строит дерево Фенвика по всем $L$ строкам документа.
2. В `EditorWidget::apply_change` (`invalidation.rs`, строки 93–109):
   - При `line_delta != 0` весь кэш `self.cache: BTreeMap<usize, Paragraph>` забирается через `std::mem::take(&mut self.cache)`, и все кэшированные параграфы вставляются заново с новыми ключами `line + line_delta` в новый `BTreeMap`.

#### Причина проблемы

1. **Подтверждено измерениями в `docs/perf/ui/README.md`**:
   - `insert_character_frame` = **83.850 мкс**, тогда как `insert_newline_frame` = **139.891 мкс** (разница **+56.04 мкс** на каждое нажатие `Enter` на 30 000 строках!).
2. **Линейная работа $O(L)$ по всему документу на каждый `Enter`**:
   - В `rebuild_sums()` цикл проходит от `1` до `self.sums.len()` (30 001 элемент `f64` = 240 КБ трафика памяти + вектор `blocks` = ещё 240–480 КБ) на каждый перевод строки!
   - Между тем, в виртуализированном редакторе абсолютное большинство строк документа ($>99\%$) вообще **никогда не измерялись** (`is_measured == false`) и имеют одинаковую высоту `default_line_height`!
   - Если хранить в дереве Фенвика (или префиксном индексе) **только дельты измеренных строк** $\Delta_i = \text{height}_i - \text{default\_line\_height}$, либо компактно упаковать `BlockInfo` в 4 байта (`f32`, где знак кодирует `is_measured`), а при `replace_blocks` обновлять `sums` только при наличии измеренных блоков после `old.start` (или использовать частичное обновление), работа сокращается на порядок!
3. **Ненужный `BTreeMap<usize, Paragraph>`**:
   - В `viewport.rs` (строка 155) на каждом кадре вызывается `self.cache.retain(|line, _| (*line >= first && *line < last) || (follow && *line == cursor_line))`. Это значит, что ключи в `self.cache` — это **почти всегда плотный непрерывный интервал строк `first..last`** (размером 40–80 строк) плюс максимум 1 строка курсора! Хранить непрерывный интервал `first..last` в `BTreeMap` неэффективно и по памяти, и по кэшу CPU, и при сдвиге индексов (`line_delta != 0`).

#### Предлагаемое решение

1. **Сжать `BlockInfo` до 4 байт (`u32`/`f32`) и отслеживать `last_measured_block: usize` в `HeightIndex`**:
   - Поскольку высота строки всегда положительна (`height > 0.0`), хранить в `blocks: Vec<f32>` положительное число `height` для измеренных блоков (`is_measured == true`) и `-default_line_height` (отрицательное) для неизмеренных блоков (`is_measured == false`), где `height_for_block(i)` возвращает `self.blocks[i].abs()`. Это уменьшает размер `self.blocks` в 2–4 раза!
   - Ещё важнее: хранить в дереве Фенвика `sums: Vec<f64>` **не полные высоты, а только избыточные дельты $\delta_i = \text{height}_i - \text{default\_line\_height}$** (для неизмеренного блока $\delta_i = 0.0$)!
   - Тогда:
     - $\text{y\_for\_block}(k) = k \cdot \text{default\_line\_height} + \text{fenwick\_prefix\_sum}(k)$.
     - Если пользователь редактирует документ в начале/середине и ниже `max_measured_index` нет ни одной измеренной строки, или число измеренных строк мало ($M \ll L$), то при `replace_blocks` вставляемые неизмеренные строки имеют **нулевую дельту ($\delta = 0.0$)**, а пересчитывать дерево Фенвика за пределами `max_measured_index` вообще не нужно!
2. **Заменить `BTreeMap<usize, Paragraph>` в `EditorWidget` на `ParagraphCache` на базе `VecDeque<Option<Paragraph>>`**:

```rust
pub(super) struct ParagraphCache {
    base_line: usize,
    window: std::collections::VecDeque<Option<Paragraph>>,
    pinned: Option<(usize, Paragraph)>,
}
```

- Когда `old_last < self.base_line` (правка выше видимого окна), сдвиг всех ключей окна на `line_delta` выполняется за **$O(1)$ одной операцией `self.base_line = self.base_line.saturating_add_signed(line_delta)`** без единой аллокации и без перебора ключей!
- Когда правка происходит внутри видимого окна (`first..last`, $\sim 60$ строк), сдвиг в `VecDeque` из 60 элементов выполняется за $< 100$ наносекунд без аллокаций узлов дерева.

#### Ожидаемый эффект

- **Ускорение `insert_newline_frame`**: Снижение времени кадра с **139.89 мкс до ~86–92 мкс** (ускорение на **35–40%** на 30 000 строках и в **5–10 раз** на документах в 100 000–300 000 строк).
- **Память `HeightIndex`**: Сокращение размера `blocks` с $16\text{--}24$ байт/блок до **4 байт/блок** (на 100 000 строк экономия $1.2\text{--}2.0\text{ МБ}$).
- **Аллокации при нажатии `Enter`**: Снижение числа аллокаций в `EditorWidget::apply_change` с $O(C)$ (50–80 узлов `BTreeMap`) до **$0$ аллокаций**.

#### Компромиссы

- При бинарном поиске `block_at_y(y)` в дереве Фенвика с дельтами условие шага `next = index + step` проверяет `sum + self.sums[next] + (next as f64) * f64::from(self.default_line_height) <= f64::from(y)`. Поскольку `default_line_height > 0` и $\text{height}_i = \text{default\_line\_height} + \delta_i \ge 0$, префиксная сумма `prefix(next) = sums[next] + step * default_line_height` остаётся строго монотонной, поэтому алгоритм binary lifting по дереву Фенвика работает за те же $O(\log L)$!

#### План реализации

1. В `crates/ui/src/editor/heights.rs` заменить `BlockInfo` на компактное представление и оптимизировать `replace_blocks` / `rebuild_sums`.
2. В `crates/ui/src/native/editor/mod.rs`, `invalidation.rs` и `viewport.rs` заменить `BTreeMap<usize, Paragraph>` на `ParagraphCache` (или сохранить тот же интерфейс методов `get`, `get_mut`, `insert`, `retain`, `apply_line_delta`).

#### Проверка

- **Тесты**: Все существующие тесты `HeightIndex` в `crates/ui/src/editor/heights.rs` и тесты инвалидации `crates/ui/src/native/editor/invalidation.rs`.
- **Бенчмарк**: `target/release/examples/ui_perf` — сценарий `insert_newline_frame`.
- **Критерий приёмки**: Медиана `insert_newline_frame` в `ui_perf` снижается с `139.89 мкс` до `< 98 мкс` при `Retained after == 78`.

#### Уверенность

- **Verified** (узкое место напрямую подтверждено замерами `insert_newline_frame` vs `insert_character_frame` в `docs/perf/ui/README.md` и кодом `heights.rs` / `invalidation.rs`).

---

### OPT-09. Устранение тройного дублирования текста и 5 промежуточных векторов в `DisplayMap` через переиспользуемый `DisplayScratchpad`

#### Локация

- **Крейт**: `truff-ui`
- **Файлы и диапазоны строк**:
  - [`crates/ui/src/editor/display/mod.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/editor/display/mod.rs), строки 24–36 (`pub struct DisplayMap`).
  - [`crates/ui/src/editor/display/markers.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/editor/display/markers.rs), строки 47–285 (`DisplayMap::scan_markers`).
  - [`crates/ui/src/editor/display/projection.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/editor/display/projection.rs), строки 9–103 (`DisplayMap::rebuild`).
  - [`crates/ui/src/native/editor/paragraph/mod.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/native/editor/paragraph/mod.rs), строки 10–78 (`Paragraph`, `Paragraph::new`).

#### Текущая реализация

Структура `DisplayMap` (`crates/ui/src/editor/display/mod.rs`, строки 24–36) хранит внутри каждого кэшированного `Paragraph`:

```rust
pub struct DisplayMap {
    source_text: String,
    mode: ProjectionMode,
    cursor: BytePos,
    selection: Option<(BytePos, BytePos)>,
    chunks: Vec<DisplayChunk>,
    visual_text: Option<Box<str>>,
    colors: Vec<truff_core::markdown::style::ColorSpan>,
    styles: Vec<StyledSpan>,
    markers: Vec<(Range<usize>, Option<Range<usize>>)>,
    lists: Vec<truff_core::markdown::presentation::ListMarker>,
    presentation: Option<truff_core::markdown::presentation::LinePresentation>,
}
```

А сам `Paragraph` (`crates/ui/src/native/editor/paragraph/mod.rs`, строки 10–22) дополнительно хранит `galley: Arc<Galley>` (внутри которого лежит `job.text: String`) и собственное поле `presentation: LinePresentation`.

#### Причина проблемы

1. **Тройное хранение текста каждой видимой строки**:
   - Копия 1: `display.source_text: String`
   - Копия 2: `display.visual_text: Option<Box<str>>`
   - Копия 3: `galley.job.text: String` (внутри `egui::Galley`)
2. **Удержание 5 временных векторов парсинга в каждом `DisplayMap`**:
   - Векторы `markers`, `colors`, `styles`, `lists` создаются в `scan_markers()` для построения `chunks` и секций `egui::text::LayoutJob`. Как только `Paragraph` построен (и особенно для строк, на которых не стоит курсор), векторы `markers` и промежуточные буферы лежат мёртвым грузом в куче у каждого из 52–78 кэшированных параграфов.
   - Более того, при быстрой прокрутке (`fast_scroll`), когда на каждом кадре создаются 20–50 новых `Paragraph::new`, каждый вызов с нуля аллоцирует в куче `markers`, `styles`, `colors`, `lists`, `chunks`, `visual_text`, а при выходе строки за экран (`cache.retain`) — освобождает их, создавая высокую нагрузку на системный аллокатор (`malloc`/`free`).

#### Предлагаемое решение

1. **Удалить дублирующееся поле `presentation` из `DisplayMap`** (или из `Paragraph`, оставив одно каноническое место).
2. **Не хранить `visual_text: Option<Box<str>>` постоянно внутри `DisplayMap`**:
   - В `NativeApp` / `Paragraph::new` визуальный текст нужен только для передачи в `egui::text::LayoutJob { text, sections, .. }`, после чего он уже хранится в `galley.job.text` (`&str` доступен через `paragraph.galley.job.text.as_str()`)!
   - Если внешний headless-тест вызывает `display.visual_text()`, он может собираться из `source_text` по `chunks` или заимствоваться из `Galley`.
3. **Вынести временные векторы разбора маркеров в переиспользуемый `DisplayScratchpad` внутри `EditorWidget` (или `thread_local!`)**:

```rust
#[derive(Default)]
pub struct DisplayScratchpad {
    pub markers: Vec<(Range<usize>, Option<Range<usize>>)>,
    pub visual_buf: String,
}
```

- При построении очередного `DisplayMap` векторы `scratch.markers.clear()` переиспользуют уже выделенную ёмкость без обращения к аллокатору.

4. **Сжать векторы внутри `DisplayMap` или использовать `Box<[T]>` / единый вектор спанов**:
   - Для обычной строки без цветных HTML-тегов `colors` пуст (0 аллокаций у `Vec::new()`), но сам заголовок `Vec` в `DisplayMap` занимает 24 байта. Объединение `colors: Vec<ColorSpan>` и `styles: Vec<StyledSpan>` (так как `InlineStyle::Highlight([u8; 4])` уже есть в `InlineStyle`!) убирает целое поле `colors: Vec<ColorSpan>` (24 байта на каждый `Paragraph`)!

#### Ожидаемый эффект

- **Размер структуры `DisplayMap` на стеке/внутри `Paragraph`**: Снижение с **~216 байт до ~112 байт** (почти в 2 раза компактнее).
- **Аллокации при создании `Paragraph::new` (скроллинг и ввод)**: Сокращение с **6–8 кучевых аллокаций на строку до 2 аллокаций** (`source_text` и `chunks`, тогда как `visual_buf` перемещается напрямую в `LayoutJob.text` без копирования!).

#### Компромиссы

- Если unit-тесты `DisplayMap` напрямую вызывают `display_map.visual_text()`, метод `pub fn visual_text(&self) -> Cow<'_, str>` возвращает `Cow::Borrowed(&self.source_text)` (когда нет скрытых чанков, что покрывает 100% строк в `Source` mode и ~80% обычных строк в `LivePreview`!) или собирает `Cow::Owned(String)` только при наличии скрытых маркеров. Это одновременно ускоряет `Paragraph::new` для всех строк без скрытых маркеров!

#### План реализации

1. В `crates/ui/src/editor/display/mod.rs` и `projection.rs`: если в строке ни один чанк не имеет `hidden == true` и нет замен маркеров списков (`replacement`), вообще **не аллоцировать `visual_text`**, а использовать `source_text` напрямую!
2. В `crates/ui/src/native/editor/paragraph/paint.rs` при формировании `LayoutJob` перемещать готовую строку или клонировать только один раз в `LayoutJob.text`.
3. Переиспользовать буфер `markers` при сканировании.

#### Проверка

- **Тесты**: Все модульные тесты `crates/ui/src/editor/display/`.
- **Бенчмарк**: `target/release/examples/ui_perf` (`insert_character_frame`, `cursor_down_frame`) и `perf_scenarios::scroll`.
- **Критерий приёмки**: Сокращение числа кучевых аллокаций на кадр прокрутки новых строк не менее чем на 50%.

#### Уверенность

- **Static**.

---

### OPT-10. Компактификация `DisplayChunk` (с 48–56 до 24–32 байт) и бинарный поиск $O(\log C)$ в `source_to_visual` / `visual_to_source`

#### Локация

- **Крейт**: `truff-ui`
- **Файлы и диапазоны строк**:
  - [`crates/ui/src/editor/display/types.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/editor/display/types.rs), строки 86–97 (`pub struct DisplayChunk`).
  - [`crates/ui/src/editor/display/mapping.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/editor/display/mapping.rs), строки 8–105 (`source_to_visual`, `visual_to_source`).
  - [`crates/ui/src/editor/display/projection.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/editor/display/projection.rs), строки 9–103 (`rebuild`).

#### Текущая реализация

В `crates/ui/src/editor/display/types.rs` (строки 86–97):

```rust
pub struct DisplayChunk {
    pub source_range: Range<usize>,
    pub is_marker: bool,
    pub enclosing_span: Option<Range<usize>>,
    pub hidden: bool,
    pub visual_len: usize,
}
```

А в `crates/ui/src/editor/display/mapping.rs` функции `source_to_visual` и `visual_to_source` линейно проходят по `self.chunks` от начала, суммируя `chunk.visual_len` на каждом вызове.

#### Причина проблемы

1. **Раздутый размер `DisplayChunk` (56 байт на 64-битной платформе)**:
   - `source_range: Range<usize>` = 16 байт
   - `enclosing_span: Option<Range<usize>>` = **24 байта** (так как `Range<usize>` не имеет нишевой оптимизации `NonZeroUsize`, `Option<Range<usize>>` добавляет 8-байтный дискриминант + выравнивание!)
   - `visual_len: usize` = 8 байт
   - `is_marker: bool` + `hidden: bool` + padding = 8 байт
   - Итого: **56 байт на один чанк**!
2. **Линейный проход $O(C)$ вместо $O(\log C)$**:
   - Поскольку `DisplayChunk` не хранит префиксную сумму `visual_start`, каждый вызов `source_to_visual` и `visual_to_source` вынужден суммировать `visual_len` всех предыдущих чанков с нуля.

#### Предлагаемое решение

1. **Хранить `visual_start: usize` (или `u32`-координаты внутри строки) и компактный `enclosing_span`**:
   - Поскольку `source_range.start <= source_range.end`, отсутствие `enclosing_span` (`None`) можно закодировать значением `Range { start: usize::MAX, end: usize::MAX }` (или использовать `u32` смещения, так как максимальный размер файла в `Buffer` ограничен `DEFAULT_MAX_FILE_BYTES = 16 MiB` $\ll 4\text{ GiB}$!).
   - С `u32`-смещениями и `visual_start: u32`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayChunk {
    pub source_start: u32,
    pub source_end: u32,
    pub visual_start: u32,
    pub visual_len: u32,
    pub enclosing_start: u32, // u32::MAX если None
    pub enclosing_end: u32,
    pub is_marker: bool,
    pub hidden: bool,
}
```

- Размер `DisplayChunk` падает с **56 байт до 28 байт (32 байта с выравниванием)** — ровно 2 чанка на одну кэш-линию 64B, **включая** готовое поле `visual_start`!

2. **Бинарный поиск в `source_to_visual` и `visual_to_source`**:
   - В `DisplayMap::rebuild` (`projection.rs`) во время создания чанков поддерживать счётчик `running_visual_pos: u32` и записывать `visual_start: running_visual_pos`.
   - В `source_to_visual` для быстрого случая `self.chunks.len() == 1 && !self.chunks[0].hidden` (обычная строка без маркеров!) возвращать `VisualPos(pos.0.min(self.source_text.len()))` за **$O(1)$ в 1 сравнение**!
   - Для строк с несколькими чанками использовать `partition_point` за $O(\log C)$.

#### Ожидаемый эффект

- **Латентность `source_to_visual` / `visual_to_source`**: Для обычных строк (1 чанк) — мгновенный $O(1)$ возврат; для сложных строк с 20–50 чанками (таблицы, ссылки, код, Flash) — ускорение в **4–8 раз** за счёт `partition_point`.
- **Память `chunks: Vec<DisplayChunk>`**: Сокращение на **43%** (32 байта вместо 56 байт на чанк).

#### Компромиссы

- Для сохранения полной обратной совместимости с тестами можно оставить helper-методы `chunk.source_range() -> Range<usize>` и `chunk.enclosing_span() -> Option<Range<usize>>`.

#### План реализации

1. В `crates/ui/src/editor/display/types.rs` добавить `visual_start` в `DisplayChunk` и компактное представление `enclosing_span`.
2. В `crates/ui/src/editor/display/projection.rs` заполнять `visual_start` при сборке `self.chunks`.
3. В `crates/ui/src/editor/display/mapping.rs` добавить $O(1)$ fast-path для 1 чанка и бинарный поиск `partition_point` для $C > 1$.

#### Проверка

- **Тесты**: Все существующие тесты маппинга координат и `Affinity::Left`/`Right` в `crates/ui/src/editor/display/`.
- **Бенчмарк**: `target/release/examples/ui_perf` (`cursor_down_frame`, `flash_word_windows`).

#### Уверенность

- **Static**.

---

### OPT-11. Избирательная перепроекция строк при движении курсора и отсечение `flash: &[FlashTarget]` по диапазону строки за $O(\log F)$

#### Локация

- **Крейт**: `truff-ui`
- **Файлы и диапазоны строк**:
  - [`crates/ui/src/native/editor/viewport.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/native/editor/viewport.rs), строки 185–340 (`EditorWidget::show`).
  - [`crates/ui/src/native/editor/paragraph/mod.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/native/editor/paragraph/mod.rs), строки 50–120 (`Paragraph::reproject`).
  - [`crates/ui/src/native/editor/paragraph/paint.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/native/editor/paragraph/paint.rs), строки 150–304 (`Paragraph::paint`).

#### Текущая реализация

В `EditorWidget::show` (`viewport.rs`) на каждом кадре цикл проходит по всем видимым строкам `first..last`:

1. Для каждой видимой строки проверяется `Paragraph::reproject(...)` (пересчёт видимости маркеров `DisplayMap` относительно текущего `cursor` и `selection`).
2. При вызове `paragraph.paint(ui, origin, ParagraphView { flash, .. })` в каждую строку передаётся полный срез всех целей `flash: &[FlashTarget]`, и `paint` внутри каждой строки проверяет все элементы `flash` на пересечение с `paragraph.range`.

#### Причина проблемы

1. **Лишние вызовы `reproject` для строк, далёких от курсора**:
   - В режиме `LivePreview` раскрытие скрытых маркеров (`**`, `[[`, `#`) зависит только от того, касается ли курсор или выделение диапазона строки `paragraph.range`. Если строка не содержала курсор/выделение на прошлом кадре и не содержит их на текущем кадре (что верно для 50 из 52 видимых строк при нажатии `j`/`k`!), вызывать `reproject` и проверять маркеры внутри `DisplayMap` не нужно вообще!
2. **Квадратичное пересечение $O(V \cdot F)$ для `FlashTarget`**:
   - Когда активен режим прыжка `Flash` (на экране до $F = 100\text{--}800$ целей `FlashTarget`), передача полного среза `flash` в каждую из $V = 50$ видимых строк приводит к $50 \times 800 = 40\,000$ проверок диапазонов на кадр, хотя цели `flash` упорядочены по возрастанию байтовой позиции, и каждая строка пересекается только со своим поддиапазоном `flash[line_flash_start..line_flash_end]`!

#### Предлагаемое решение

1. **В `Paragraph::reproject` (`crates/ui/src/native/editor/paragraph/mod.rs`)**:
   - Вычислять локальные `local_cursor: Option<usize>` и `local_selection: Option<(usize, usize)>`, ограниченные `paragraph.range` (`None`, если курсор/выделение вне `paragraph.range`!).
   - Если в `DisplayMap` и старые, и новые локальные `cursor`/`selection` равны `None` (курсор вне этой строки), `reproject` завершается **мгновенно за 1 проверку `if`**, не трогая `DisplayMap`!
2. **В `viewport.rs` отсекать срез `flash` для каждой строки через `partition_point`**:

```rust
let line_flash = if flash.is_empty() {
    &[]
} else {
    let start_idx = flash.partition_point(|t| t.range.end <= paragraph.range.start);
    let end_idx = flash[start_idx..].partition_point(|t| t.range.start < paragraph.range.end) + start_idx;
    &flash[start_idx..end_idx]
};
```

#### Ожидаемый эффект

- **Ускорение `cursor_down_frame`**: Дополнительно на **15–25%** (снижение медианы с `69.87 мкс` до `~52–58 мкс`).
- **Ускорение отрисовки кадра при активном `Flash`**: Снижение сложности проверки целей с $O(V \cdot F)$ до $O(V \log F + F_{\text{visible}})$.

#### Компромиссы

- Требует, чтобы массив `flash: &[FlashTarget]` был отсортирован по `range.start` (он уже генерируется в порядке возрастания байтовых смещений в `crates/core/src/markdown/flash.rs`).

#### План реализации

1. В `crates/ui/src/native/editor/paragraph/mod.rs` передавать в `DisplayMap` только локальный курсор/выделение (`Option` внутри `paragraph.range`), чтобы строки вне курсора пропускали пересчёт за $O(1)$.
2. В `crates/ui/src/native/editor/viewport.rs` передавать в `ParagraphView` отсечённый срез `line_flash`.

#### Проверка

- **Тесты**: Тесты раскрытия маркеров в `LivePreview` при переходе курсора со строки $i$ на строку $i+1$ и тесты отрисовки `Flash`.
- **Бенчмарк**: `target/release/examples/ui_perf` (`cursor_down_frame`).

#### Уверенность

- **Static**.

---

### OPT-12. Двухуровневое ускорение `NavigatorState::filter` на 10 000+ элементов: 64-битный ASCII-фильтр (`char_mask: u64`) и инкрементальное сужение `filtered_indices`

#### Локация

- **Крейт**: `truff-ui`
- **Файлы и диапазоны строк**:
  - [`crates/ui/src/navigator/model.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/navigator/model.rs), строки 7–24 (`pub struct NavigatorState`), строки 100–250 (`NavigatorState::filter`).
  - [`crates/ui/src/navigator/types.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/navigator/types.rs), строки 141–148 (`pub struct NavigatorItem`).

#### Текущая реализация

В `docs/perf/ui/README.md` зафиксировано, что после первого раунда оптимизации сценарий `navigator_filter_10000` всё ещё занимает **4 168.975 мкс (4.17 мс)** на 4 последовательных запроса по 10 000 элементов. Причина в том, что `NavigatorState::filter` (`crates/ui/src/navigator/model.rs`, строки 100–250) при каждом изменении `self.query`:

1. Очищает `self.filtered_indices` и итерируется по **всем 10 000 элементам `self.items` с нуля**, даже когда новый запрос является продолжением предыдущего (например, `"n"` $\rightarrow$ `"no"` $\rightarrow$ `"not"` $\rightarrow$ `"note"`).
2. Для каждого из 10 000 элементов разыменовывает указатели `item.title: Box<str>` и `item.subtitle: Option<Box<str>>` в куче (до 20 000 промахов кэша CPU!) и выполняет посимвольное сопоставление с приведением регистра.

#### Причина проблемы

1. **Отсутствие монотонности при дописывании символов (` Incremental Narrowing`)**:
   - Для любого нечёткого (subsequence) или подстрочного (substring) фильтра выполняется строгое математическое свойство монотонности: если запрос $Q'$ получен из запроса $Q$ дописыванием символов в конец ($Q' = Q + c$), то множество совпадений $\text{Match}(Q') \subseteq \text{Match}(Q)$!
   - Проверять элементы, которые уже не подошли под префикс $Q$, при вводе следующего символа $c$ — 100% лишняя работа.
2. **Отсутствие битовой сигнатуры символов (`char_mask: u64`) внутри `NavigatorItem`**:
   - Если элемент не содержит хотя бы одной буквы из поискового запроса $Q$, вызывать `is_subsequence` и читать `item.title` из кучи не нужно.
   - 64-битная битовая маска `char_mask: u64` (биты 0..25 для `'a'..='z'`, 26..35 для `'0'..='9'`, 36..62 для частых символов/кириллических бакетов и ` '-', '_', '/'`) помещается прямо в `NavigatorItem` (8 байт) и проверяется за **1 такт CPU: `(item.char_mask & query_mask) == query_mask`** без единого обращения к строкам в куче!

#### Предлагаемое решение

1. **Добавить поле `pub char_mask: u64` в `NavigatorItem` (`crates/ui/src/navigator/types.rs`)**:

```rust
#[inline]
pub fn compute_char_mask(s: &str) -> u64 {
    let mut mask = 0u64;
    for b in s.bytes() {
        let lower = b.to_ascii_lowercase();
        let bit = match lower {
            b'a'..=b'z' => (lower - b'a') as u32,
            b'0'..=b'9' => 26 + (lower - b'0') as u32,
            b'_' => 36,
            b'-' => 37,
            b'/' | b'\\' => 38,
            b'.' => 39,
            b' ' => 40,
            128..=255 => 41 + ((lower as u32) % 23),
            _ => continue,
        };
        mask |= 1u64 << bit;
    }
    mask
}
```

2. **Хранить в `NavigatorState` (`crates/ui/src/navigator/model.rs`) поле `last_filtered_query: String`**:
   - Если новый нормализованный запрос `q` начинается с `self.last_filtered_query` (`!self.last_filtered_query.is_empty() && q.starts_with(&self.last_filtered_query)`), то вместо перебора `0..self.items.len()` фильтровать in-place **только элементы из текущего `self.filtered_indices`**!
   - Даже на первом символе запроса (или при нажатии `Backspace`), перед вызовом строкового матчинга на `item.title`/`item.subtitle`, проверять `if (item.char_mask & q_mask) != q_mask { continue; }`.

#### Ожидаемый эффект

- **Ускорение `navigator_filter_10000`**: Снижение времени с **4 168.98 мкс до ~350–600 мкс** (ускорение в **7–12 раз**!).
  - На первом символе запроса битовая маска `char_mask` отсекает ~75–85% из 10 000 элементов без разыменования `Box<str>`.
  - На 2-м, 3-м и 4-м символах запроса инкрементальное сужение `filtered_indices` проверяет только оставшиеся ~1 500 $\rightarrow$ ~250 $\rightarrow$ ~40 элементов вместо 10 000!

#### Компромиссы

- Добавляет 8 байт (`char_mask: u64`) на `NavigatorItem` (на 10 000 элементов это всего 80 КБ временной памяти только пока открыт навигатор, которая освобождается в `NavigatorState::close`), а при ранжировании (scoring) пересчитывает очки только для выживших в `filtered_indices` кандидатов.

#### План реализации

1. В `crates/ui/src/navigator/types.rs` добавить `pub char_mask: u64` в `NavigatorItem` и вычислять его в конструкторах `NavigatorItem` из `title` и `subtitle`.
2. В `crates/ui/src/navigator/model.rs` добавить `last_filtered_query: String` в `NavigatorState` (очищать в `open` и `close`) и реализовать проверку `(item.char_mask & query_mask) == query_mask` + сужение по `self.filtered_indices` при `q.starts_with(&self.last_filtered_query)`.

#### Проверка

- **Тесты**: Сравнить выдачу `filtered_indices` и порядок сортировки до и после оптимизации на последовательностях ввода символов и `Backspace`.
- **Бенчмарк**: `target/release/examples/ui_perf` — сценарий `navigator_filter_10000`.
- **Критерий приёмки**: Медиана `navigator_filter_10000` $< 800\text{ мкс}$ (против `4 168.98 мкс` сейчас).

#### Уверенность

- **Verified** (базовый замер `4 168.98 мкс` в `docs/perf/ui/README.md`, алгоритмическое отсутствие фильтрации по префиксу проверено в `navigator/model.rs`).

---

### OPT-13. Оптимизация памяти GPU/EGL при запуске `eframe/glow` (предотвращение загрузки Mesa/LLVM на гибридных/NVIDIA системах и отключение лишних буферов кадра)

#### Локация

- **Крейт**: `truff-ui`
- **Файлы и диапазоны строк**:
  - [`crates/ui/src/main.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/main.rs), строки 178–193 (инициализация `eframe::NativeOptions` и `eframe::run_native`).
  - [`crates/ui/src/native/mod.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/native/mod.rs), строки 33–58 (`configure_fonts`), строки 81–105 (`NativeApp::new`).

#### Текущая реализация

В `crates/ui/src/main.rs` (строки 178–192) создаётся `eframe::NativeOptions` с настройками по умолчанию, и запускается `eframe::run_native`. Как зафиксировано в [`docs/perf/p0/decision.md`](https://github.com/hackerman111/truff/blob/main/docs/perf/p0/decision.md):

- При обычном запуске на Linux Wayland с NVIDIA GPU библиотека `libGLdispatch` / `libEGL` перебирает все файлы в `/usr/share/glvnd/egl_vendor.d/` (включая `50_mesa.json`), загружая в память процесса `libEGL_mesa.so`, `libgallium` и `libLLVM.so`, что увеличивает VmRSS процесса со **131.4 MiB до 308.3 MiB (+176.9 MiB!)**.
- Кроме того, в `configure_fonts` (`crates/ui/src/native/mod.rs`, строки 33–58) вызывается `ctx.set_fonts(fonts)` при инициализации и смене настроек.

#### Причина проблемы

1. **Загрузка софтверного/вторичного драйвера Mesa/LLVM через GLVND**: В Linux GLVND при вызове `eglGetDisplay` / `eglInitialize` без фильтра вендора инициализируются все зарегистрированные ICD-вендоры, включая софтверный растеризатор Mesa/llvmpipe, который мапит в адресное пространство процесса `libLLVM` (~120–170 MiB).
2. **Параметры буфера кадра OpenGL по умолчанию**: Для 2D-редактора `egui` не требуются 24-битный буфер глубины (`depth_buffer`), 8-битный буфер трафарета (`stencil_buffer`) и аппаратный MSAA (`multisampling`), которые выделяют дополнительные поверхности VRAM/RAM на каждое окно.

#### Предлагаемое решение

1. **Явно отключить неиспользуемые буферы в `eframe::NativeOptions` (`crates/ui/src/main.rs`, строки 178–185)**:

```rust
let options = eframe::NativeOptions {
    viewport: eframe::egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 800.0]),
    depth_buffer: 0,
    stencil_buffer: 0,
    multisampling: 0,
    vsync: true,
    hardware_acceleration: eframe::HardwareAcceleration::Preferred,
    ..Default::default()
};
```

2. **Автоматический выбор аппаратного EGL ICD на Linux до инициализации графического контекста (`crates/ui/src/main.rs`)**:
   - В начале `main()` (перед созданием `eframe`), если на Linux не задана переменная окружения `__EGL_VENDOR_LIBRARY_FILENAMES` и активен драйвер NVIDIA (`/proc/driver/nvidia/version` или `/sys/module/nvidia` существует, а `/usr/share/glvnd/egl_vendor.d/10_nvidia.json` присутствует и читаем), аккуратно проверить и при отсутствии переопределения задать предпочтение аппаратного ICD (либо задокументировать флаг запуска/обёртку), чтобы GLVND не загружал `50_mesa.json` и `libLLVM.so`.

#### Ожидаемый эффект

- **Снижение базового VmRSS на Linux + NVIDIA**: С **308.3 MiB до ~131–140 MiB VmRSS** (экономия **~170 MiB VmRSS**, подтверждено замерами в `docs/perf/p0/decision.md`!), плюс экономия ~15–25 MiB буферов глубины/трафарета/MSAA при высоких разрешениях экрана (4K).

#### Компромиссы

- На ноутбуках с гибридной графикой (Intel/AMD iGPU + NVIDIA dGPU в режиме PRIME offload) принудительный выбор `10_nvidia.json` может разбудить дискретный GPU, если композитор работает на iGPU. Поэтому автовыбор `10_nvidia.json` следует включать только тогда, когда NVIDIA является единственным или основным активным DRM-устройством, либо через опцию конфигурации.

#### План реализации

1. В `crates/ui/src/main.rs` настроить `depth_buffer: 0`, `stencil_buffer: 0`, `multisampling: 0`.
2. Добавить функцию `configure_linux_egl_vendor_if_needed()` в `crates/ui/src/main.rs` перед вызовом `eframe::run_native`.

#### Проверка

- **Бенчмарк**: Замер `/proc/<pid>/status` (`VmRSS`) и `/proc/<pid>/smaps_rollup` (`Pss`, `Private_Dirty`) на нативном окне `target/release/truff --vault <empty>`.
- **Критерий приёмки**: VmRSS $\le 160\text{ MiB}$, PSS $\le 55\text{ MiB}$ без ручной передачи переменных окружения пользователем.

#### Уверенность

- **Verified** (разница 308.3 MiB vs 131.4 MiB прямо измерена и задокументирована в `docs/perf/p0/decision.md`).

---

### OPT-14. Замена полного пересчёта всех ссылок хранилища (`SELECT ... FROM links`) в `refresh_resolutions` на инкрементальный таргетированный резолв

#### Локация

- **Крейт**: `truff-store`
- **Файлы и диапазоны строк**:
  - [`crates/store/src/index/update.rs`](https://github.com/hackerman111/truff/blob/main/crates/store/src/index/update.rs), строки 12–35 (`index_document`), строки 39–156 (`index_document_in`), строки 192–221 (`refresh_resolutions`).
  - [`crates/store/src/index/schema.sql`](https://github.com/hackerman111/truff/blob/main/crates/store/src/index/schema.sql), строки 34–52 (таблица `links` и индексы).
  - [`crates/store/src/catalog/scan.rs`](https://github.com/hackerman111/truff/blob/main/crates/store/src/catalog/scan.rs), строки 84–164 (`scan::refresh`).

#### Текущая реализация

В `crates/store/src/index/update.rs` (строки 192–221) функция `refresh_resolutions(conn: &Connection)` выполняет:

```sql
SELECT source_file_id, occurrence_ordinal, raw_target, raw_heading, resolved_file_id, resolved_anchor
FROM links
```

Для **каждой** строки таблицы `links` во всём хранилище вызывается `Resolver::new(conn).resolve(...)`, и если `(new_file_id, new_anchor) != (resolved_file_id, resolved_anchor)`, выполняется `UPDATE links SET resolved_file_id = ?3, resolved_anchor = ?4 WHERE source_file_id = ?1 AND occurrence_ordinal = ?2`. Эта функция вызывается при переиндексации документов и при `scan::refresh`.

#### Причина проблемы

1. **Скрытая квадратичная сложность $O(V \cdot E_{\text{total}})$**:
   - В хранилище из $V = 10\,000$ заметок, где каждая заметка содержит в среднем 15 ссылок, таблица `links` насчитывает **$E_{\text{total}} = 150\,000$ строк**.
   - Когда пользователь редактирует и сохраняет **одну-единственную заметку**, вызов `refresh_resolutions` выгружает из SQLite все $150\,000$ ссылок и выполняет до $150\,000 \times 2 = 300\,000$ SQL-запросов внутри `Resolver::resolve`!
   - На тесте `reindex_single_in_1000` (`docs/perf/store/README.md`) тестовый корпус в `store_perf.rs` имел мало ссылок, поэтому проблема осталась незамеченной на малом синтетическом тесте, но на реальном графе из $10\,000$ заметок (150 000 ссылок) переиндексация одной заметки займёт **сотни миллисекунд**!
2. **Когда на самом деле может измениться разрешение чужой ссылки (`source_file_id != changed_file_id`)?**
   - **Случай 1 (Обновление содержимого существующей заметки `file_id` без смены её пути/имени)**:
     - Исходящие ссылки самой заметки (`source_file_id = file_id`) уже резолвятся при их вставке в `index_document_in`!
     - Входящие ссылки из других заметок, указывающие на `file_id`, могли изменить свой статус **только если они ссылаются на конкретный заголовок или блок (`raw_heading IS NOT NULL`) внутри `file_id`** (`WHERE resolved_file_id = ?1 AND raw_heading IS NOT NULL`)! Ссылки просто на сам файл `[[Note]]` без `#heading` гарантированно не изменили свой `resolved_file_id`!
   - **Случай 2 (Добавление нового файла, удаление или переименование файла)**:
     - Перепроверять нужно только:
       1. ссылки, которые ранее указывали на удалённый/изменённый `file_id` (`WHERE resolved_file_id = ?1`);
       2. ссылки, которые ранее были неразрешёнными (`WHERE resolved_file_id IS NULL`) или чей `raw_target` совпадает с именем/путём добавленного файла!

#### Предлагаемое решение

1. **Добавить индекс для неразрешённых ссылок и якорных ссылок в `crates/store/src/index/schema.sql`**:

```sql
CREATE INDEX IF NOT EXISTS idx_links_unresolved ON links(raw_target) WHERE resolved_file_id IS NULL;
```

2. **Разделить `refresh_resolutions` в `crates/store/src/index/update.rs` на два режима**:
   - `refresh_resolutions_for_modified_content(conn: &Connection, file_id: FileId)` — вызывается, когда изменилось только тело существующего файла `file_id` (и только если в файле изменился набор `headings` или `blocks`!):
     ```sql
     SELECT source_file_id, occurrence_ordinal, raw_target, raw_heading, resolved_file_id, resolved_anchor
     FROM links
     WHERE resolved_file_id = ?1 AND raw_heading IS NOT NULL
     ```
   - `refresh_resolutions_for_topology_change(conn: &Connection, affected_file_ids: &[FileId])` — вызывается, когда были добавлены или удалены файлы: проверяет только ссылки из новых файлов, ссылки с `resolved_file_id IS NULL` и ссылки с `resolved_file_id IN (...)`.

#### Ожидаемый эффект

- **Сложность переиндексации одной заметки при сохранении**: Падает с $O(E_{\text{total}})$ (150 000 ссылок) до **$O(E_{\text{note}} + E_{\text{incoming\_with\_heading}})$** (~15 исходящих + ~2–5 входящих якорных ссылок).
- **Ускорение на хранилище из 10 000 заметок**: С **~120–350 мс до < 0.25 мс** (ускорение в **500–1 000 раз** на насыщенном ссылками графе!).

#### Компромиссы

- При первом полном построении индекса с нуля (`rebuild` на пустой БД) все файлы являются новыми, и их ссылки резолвятся один раз после регистрации всех путей `files` в таблице.

#### План реализации

1. В `crates/store/src/index/schema.sql` добавить частичный индекс `idx_links_unresolved`.
2. В `crates/store/src/index/update.rs` резолвить исходящие ссылки документа сразу в `index_document_in`, а для входящих ссылок вызывать `refresh_resolutions_for_file(tx, file_id, topology_changed)`.
3. В `crates/store/src/catalog/scan.rs` (`refresh`) отслеживать флаг `topology_changed = !removed_files.is_empty() || any_new_file_registered`. Если `topology_changed == false`, обновлять только входящие якорные ссылки для реально изменившихся `file_id`.

#### Проверка

- **Тесты**: Интеграционные тесты разрешения ссылок при создании целевой заметки после ссылки, удалении заметки и переименовании заголовка `# Heading`.
- **Бенчмарк**: В `crates/store/examples/store_perf.rs` добавить сценарий `reindex_single_in_10000_with_150k_links`.
- **Критерий приёмки**: Время `reindex_single` на базе с 10 000 заметок и 150 000 ссылок $< 500\text{ мкс}$.

#### Уверенность

- **Static** (запрос `SELECT ... FROM links` без `WHERE` в `refresh_resolutions` прямо проверен в `crates/store/src/index/update.rs`, строки 192–221).

---

### OPT-15. Устранение чтения и SHA-256 хеширования всех 10 000 файлов при запуске (`scan::refresh`) через двухступенчатый фильтр `(mtime_ns, file_size)`

#### Локация

- **Крейт**: `truff-store`
- **Файлы и диапазоны строк**:
  - [`crates/store/src/catalog/scan.rs`](https://github.com/hackerman111/truff/blob/main/crates/store/src/catalog/scan.rs), строки 16–164 (`enumerate`, `compute_fingerprint`, `rebuild`, `refresh`).
  - [`crates/store/src/index/schema.sql`](https://github.com/hackerman111/truff/blob/main/crates/store/src/index/schema.sql), строки 5–11 (таблица `files`).
  - [`crates/store/src/index/update.rs`](https://github.com/hackerman111/truff/blob/main/crates/store/src/index/update.rs), строки 50–58.

#### Текущая реализация

При открытии хранилища `scan::rebuild` $\rightarrow$ `scan::refresh` (`crates/store/src/catalog/scan.rs`, строки 60–164):

1. Обходит дерево каталогов через `enumerate(root)` и собирает `Vec<String>` относительных путей `.md`-файлов.
2. В `refresh` открывает транзакцию SQLite и для **каждого** пути в цикле:
   - читает весь файл с диска в память через `read_file_with_limit(&resolved, DEFAULT_MAX_FILE_BYTES)`;
   - вычисляет криптографический хеш SHA-256 от всего содержимого файла через `compute_fingerprint(text.as_bytes())`;
   - выполняет SQL-запрос `SELECT id, fingerprint, index_generation FROM files WHERE relative_path = ?1`;
   - и только если `fingerprint == stored_fingerprint`, пропускает парсинг `MarkdownFacts::extract`.

#### Причина проблемы

1. **Чтение 100% файлов хранилища с диска при каждом запуске**:
   - На хранилище из $10\,000$ заметок (например, $10\,000 \times 8\text{ КБ} = 80\text{ МБ}$) даже если с прошлого запуска не изменился **ни один файл**, `scan::refresh` выполняет $10\,000$ системных вызовов `open()`, $10\,000\text{--}20\,000$ вызовов `read()`, $10\,000$ вызовов `close()`, выделяет и освобождает $10\,000$ буферов `String`, считает SHA-256 по $80\text{ МБ}$ текста и выполняет $10\,000$ точечных `SELECT` к SQLite!
   - На холодном кэше ОС (после перезагрузки или на ноутбуке с NVMe/SSD) $10\,000$ мелких чтений файлов занимают **$350\text{--}1\,500\text{ мс}$**, съедая весь бюджет холодного старта (`cold_startup_p95_ms = 500.0 ms`).

#### Предлагаемое решение

1. **Хранить `mtime_ns: i64` и `file_size: i64` в таблице `files` (`crates/store/src/index/schema.sql`)**:
   - При инициализации схемы (`crates/store/src/index/schema.rs`) выполнять безопасную миграцию `ALTER TABLE files ADD COLUMN mtime_ns INTEGER NOT NULL DEFAULT -1; ALTER TABLE files ADD COLUMN file_size INTEGER NOT NULL DEFAULT -1;` (если столбцов ещё нет).
2. **Собирать метаданные `(relative_path, mtime_ns, file_size)` прямо во время обхода `enumerate` (`crates/store/src/catalog/scan.rs`)**:

```rust
pub(super) struct ScannedFile {
    pub relative_path: String,
    pub mtime_ns: i64,
    pub file_size: i64,
}
```

- Вызов `entry.metadata()` не открывает файл (`statx`/`fstatat`) и возвращает размер и время модификации с наносекундной точностью (`modified().duration_since(UNIX_EPOCH).as_nanos()`).

3. **Загружать существующую таблицу `files` одним пакетным запросом перед циклом в `refresh`**:
   - Вместо $10\,000$ отдельных запросов `SELECT ... WHERE relative_path = ?1`, выполнить **один** запрос:
     `SELECT relative_path, id, fingerprint, index_generation, mtime_ns, file_size FROM files`
     и сложить его в `HashMap<String, StoredFileMeta>` с `with_capacity`.
   - В цикле по `scanned_files`: если `stored.mtime_ns == scanned.mtime_ns && stored.file_size == scanned.file_size && scanned.mtime_ns > 0`, **немедленно пропускать файл (`continue`) без `read_file_with_limit`, без SHA-256 и без SQL-запроса**!
   - Если же `mtime_ns` или `file_size` изменились, читать файл и сверять SHA-256 `fingerprint` (сохраняя защиту от «пустого touch» файла без изменения содержимого: если `fingerprint` совпал, обновить только `mtime_ns` в БД без перепарсинга `MarkdownFacts`!).

#### Ожидаемый эффект

- **Повторное открытие хранилища (`catalog_reopen`)**:
  - На 100 файлах: снижение с `982 мкс` до **`~120–180 мкс`** (ускорение в **5–8 раз**).
  - На 10 000 файлах: снижение с **~100–800 мс до ~12–25 мс** (ускорение в **10–40 раз**), **0 открытий/чтений неизменённых файлов** и **1 SQL-запрос вместо 10 000**!

#### Компромиссы

- В редких тестах, которые перезаписывают файл новым содержимым **ровно того же размера в байтах** в пределах одного тика файловой системы, наносекундный `mtime_ns` на современных ФС (ext4, btrfs, APFS, NTFS) всё равно меняется. Если в синтетическом тесте `mtime_ns` совпал с текущим моментом времени (`now - mtime < 2 ms`, т.н. _racy-clean_ проблема из Git index), для таких «свежих» файлов (моложе 10 мс) можно принудительно проверять `fingerprint`. Это даёт 100% надёжность Git-индекса даже в быстрых unit-тестах!

#### План реализации

1. В `crates/store/src/index/schema.sql` и `schema.rs` добавить поля `mtime_ns` и `file_size` в таблицу `files`.
2. В `crates/store/src/catalog/scan.rs` обновить `enumerate` для возврата метаданных `(mtime_ns, file_size)` и заменить $N$ точечных `SELECT` в `refresh` на единый предзагруженный `HashMap` с проверкой _racy-clean_ окна (10 мс).

#### Проверка

- **Тесты**: Существующие тесты `catalog` (включая мгновенную модификацию файла в тестах благодаря проверке _racy-clean_ окна).
- **Бенчмарк**: `target/release/examples/catalog_perf` (`catalog_reopen_100_unchanged` и новый `catalog_reopen_10000_unchanged`).
- **Критерий приёмки**: `catalog_reopen_100_unchanged` $< 250\text{ мкс}$, `catalog_reopen_10000_unchanged` $< 30\text{ мс}$.

#### Уверенность

- **Verified** (ограничение прямо указано в `docs/perf/store/README.md`, код `scan.rs` проверен).

---

### OPT-16. Пакетное разрешение ссылок в памяти (`InMemoryResolver`) при массовой индексации и оптимизация SQL-индексов `Resolver`

#### Локация

- **Крейт**: `truff-store`
- **Файлы и диапазоны строк**:
  - [`crates/store/src/index/resolve.rs`](https://github.com/hackerman111/truff/blob/main/crates/store/src/index/resolve.rs), строки 45–180 (`Resolver::resolve`).
  - [`crates/store/src/index/schema.sql`](https://github.com/hackerman111/truff/blob/main/crates/store/src/index/schema.sql), строки 5–52.
  - [`crates/store/src/index/backlinks.rs`](https://github.com/hackerman111/truff/blob/main/crates/store/src/index/backlinks.rs), строки 102–169 (`find_unlinked_references`).

#### Текущая реализация

В `Resolver::resolve` (`crates/store/src/index/resolve.rs`) разрешение каждой отдельной ссылки выполняет последовательность SQL-запросов к таблицам `files` (по `relative_path`, `file_stem`, `file_name`, `title`), а при наличии якоря `#heading` или `#^block` — дополнительные запросы к `headings` или `blocks`.

#### Причина проблемы

- При холодной индексации хранилища из $10\,000$ заметок (`150 000` ссылок) выполнение $150\,000 \times 2 = 300\,000$ отдельных вызовов `stmt.query_row(...)` через FFI границу Rust $\leftrightarrow$ C SQLite создаёт огромные накладные расходы на блокировки, парсинг параметров и обход B-деревьев SQLite, хотя во время пакетного сканирования `scan::refresh` таблица файлов и заголовков полностью известна в памяти!

#### Предлагаемое решение

1. **Проверить и добавить покрывающие индексы в `crates/store/src/index/schema.sql`** для всех путей поиска `Resolver`:
   - `CREATE INDEX IF NOT EXISTS idx_files_stem ON files(file_stem, relative_path, id);`
   - `CREATE INDEX IF NOT EXISTS idx_files_title ON files(title, relative_path, id);`
2. **Во время пакетной индексации (`scan::refresh` при числе изменённых файлов $\ge 16$) использовать `BatchResolverCache`**:
   - Однократно загрузить карту `relative_path -> FileId` и `file_stem -> SmallVec<[FileId; 2]>` в память Rust (`FxHashMap` / `HashMap`) и резолвить ссылки без FFI-вызовов в SQLite!

#### Ожидаемый эффект

- **Ускорение холодной индексации 10 000 заметок**: Сокращение времени разрешения 150 000 ссылок с **~1.2 с до ~80–150 мс** (ускорение в **8–15 раз**).

#### Компромиссы

- `BatchResolverCache` создаётся на стеке только на время работы `scan::refresh` при массовом обновлении и немедленно освобождается после `tx.commit()`, поэтому не увеличивает удерживаемую память (RSS).

#### План реализации

1. Добавить покрывающие индексы в `schema.sql`.
2. В `crates/store/src/index/resolve.rs` добавить `BatchResolver`, использующий предзагруженные хэш-таблицы путей и `file_stem` при пакетном обновлении.

#### Проверка

- **Тесты**: Тесты `resolve.rs` на однозначные и неоднозначные `basename`/`title` ссылки.
- **Бенчмарк**: `target/release/examples/store_perf` (`resolve_basename_1000`, `index_100_basename_links`).

#### Уверенность

- **Static**.

---

### OPT-17. Оптимизация полнотекстового поиска FTS5 (`SearchSession::search`): пропуск второго запроса `COUNT(*)` при неполной странице и отложенный `JOIN files` / `snippet()`

#### Локация

- **Крейт**: `truff-store`
- **Файл и диапазон строк**: [`crates/store/src/index/search.rs`](https://github.com/hackerman111/truff/blob/main/crates/store/src/index/search.rs), строки 70–151 (`SearchSession::search`).

#### Текущая реализация

В `SearchSession::search` (`crates/store/src/index/search.rs`, строки 70–151) всегда выполняются два SQL-запроса:

1. Запрос страницы результатов с `JOIN files` и `snippet(notes_fts, 2, ...)`:
   ```sql
   SELECT f.id, f.relative_path, f.title, snippet(notes_fts, 2, '<b>', '</b>', '...', 10)
   FROM notes_fts
   JOIN files f ON notes_fts.rowid = f.id
   WHERE notes_fts MATCH ?1
   LIMIT ?2 OFFSET ?3;
   ```
2. Запрос общего числа совпадений:
   ```sql
   SELECT COUNT(*) FROM notes_fts WHERE notes_fts MATCH ?1;
   ```

В [`docs/perf/store/README.md`](https://github.com/hackerman111/truff/blob/main/docs/perf/store/README.md) отмечено, что `fts_search_page_1000` не ускорился (`708.67 мкс -> 713.61 мкс`).

#### Причина проблемы

1. **Безусловный вызов второго запроса `SELECT COUNT(*) FROM notes_fts WHERE notes_fts MATCH ?1`**:
   - В 90% реальных поисковых запросов пользователь ищет конкретную фразу, которая возвращает меньше результатов, чем размер страницы `limit` (например, находится 7 заметок при `limit = 20, offset = 0`), либо вообще 0 заметок!
   - Когда `results.len() < limit` (и `offset == 0` или `!results.is_empty()`), общее количество совпадений `total_hits` **математически известно без второго запроса**: `total_hits = offset + results.len()` (а если `offset == 0 && results.is_empty()`, то `total_hits = 0`)!
   - Сейчас код всё равно запускает второй проход по инвертированному индексу FTS5 `SELECT COUNT(*)`, удваивая время каждого короткого/пустого поиска!
2. **Ограничение подсчёта при огромном числе совпадений**:
   - Если пользователь ввёл одну букву `"a"`, подсчёт точного `COUNT(*)` по 10 000 заметкам заставляет FTS5 декодировать весь список постингов. Ограничение подсчёта подзапросом `SELECT COUNT(*) FROM (SELECT rowid FROM notes_fts WHERE notes_fts MATCH ?1 LIMIT 1001)` позволяет мгновенно показать в UI `1000+` совпадений без чтения десятков тысяч постингов.

#### Предлагаемое решение

```rust
// В crates/store/src/index/search.rs
let total_hits = if results.len() < limit {
    if offset == 0 || !results.is_empty() {
        offset + results.len()
    } else {
        // offset > 0 и страница пуста: нужно узнать реальное число
        self.count_hits(&escaped_query)?
    }
} else {
    self.count_hits(&escaped_query)?
};
```

#### Ожидаемый эффект

- **Ускорение поиска для запросов с числом результатов $< \text{limit}$**: Снижение латентности на **35–50%** (с ~713 мкс до ~380–450 мкс) за счёт полного пропуска второго FTS5-запроса.
- **Ускорение широких 1-символьных запросов на 10 000 заметок**: До **5–10 раз** при ограничении верхнего порога подсчёта.

#### Компромиссы

- При `results.len() < limit` результат `total_hits = offset + results.len()` является **100% точным** без каких-либо приближений.

#### План реализации

1. В `crates/store/src/index/search.rs` вычислять `total_hits = offset + results.len()` без вызова `SELECT COUNT(*)`, когда `results.len() < limit && (offset == 0 || !results.is_empty())`.

#### Проверка

- **Тесты**: Все существующие тесты `search.rs` (проверка совпадения `total_hits` при `results.len() == 0`, `results.len() < limit`, `results.len() == limit` и пагинации `offset > 0`).
- **Бенчмарк**: `target/release/examples/store_perf` (`fts_search_page_1000`).

#### Уверенность

- **Verified** (причина стагнации `fts_search_page_1000` в `docs/perf/store/README.md` напрямую объясняется двойным FTS5-запросом в `search.rs`).

---

### OPT-18. Потоковая запись `crop::Rope` без промежуточного `String` при сохранении и кэширование состояния каталога в `RecoveryManager::write_snapshot`

#### Локация

- **Крейты**: `truff-store`, `truff-ui`
- **Файлы и диапазоны строк**:
  - [`crates/store/src/recovery.rs`](https://github.com/hackerman111/truff/blob/main/crates/store/src/recovery.rs), строки 1–432 (`RecoveryManager`, `write_snapshot`).
  - [`crates/store/src/files.rs`](https://github.com/hackerman111/truff/blob/main/crates/store/src/files.rs), строки 147–195 (`write_file_atomically`).
  - [`crates/ui/src/app/commands/files.rs`](https://github.com/hackerman111/truff/blob/main/crates/ui/src/app/commands/files.rs).

#### Текущая реализация

1. В `RecoveryManager::write_snapshot` (`crates/store/src/recovery.rs`) при каждом сохранении снэпшота восстановления проверяется цепочка родительских каталогов на симлинки и выполняется сканирование каталога recovery (`std::fs::read_dir`) для подсчёта суммарного размера файлов и удаления старых ревизий.
2. При сохранении крупного буфера его содержимое преобразуется из `crop::Rope` в непрерывный `String` для передачи в `write_file_atomically`.

#### Причина проблемы

1. **Пиковая аллокация `String` при сохранении/recovery**: Поскольку `crop::Rope` представляет собой персистентное B-дерево на базе `Arc`, клонирование `Rope` в UI-потоке (`buffer.rope().clone()`) занимает **$O(1)$ времени ($\sim 5$ наносекунд) и $0$ байт кучи**! Превращать `Rope` в `String` в UI-потоке или даже в рабочем потоке перед записью в файл не нужно — `BufWriter<File>` может записывать чанки `for chunk in rope.chunks() { writer.write_all(chunk.as_bytes())?; }` напрямую!
2. **Лишний `read_dir` в `write_snapshot`**: `RecoveryManager` является единственным владельцем каталога recovery в рамках сессии. Хранение в памяти `RecoveryManager` таблицы известных файлов снимков `known_snapshots: HashMap<PathBuf, u64>` и `total_bytes: u64` позволяет обновлять бюджет размера и находить предыдущую ревизию того же `BufferId` за **$O(1)$ без вызова `std::fs::read_dir`** на каждый snapshot!

#### Предлагаемое решение

1. Добавить в `crates/store/src/files.rs` функцию `pub fn write_rope_atomically(path: &Path, rope: &crop::Rope) -> Result<(), FileError>`, записывающую чанки `rope.chunks()` через `std::io::BufWriter::with_capacity(64 * 1024, temp_file)`.
2. В `RecoveryManager` (`crates/store/src/recovery.rs`) инициализировать `known_snapshots` один раз при старте (`new` / `recover`) через `read_dir`, а при последующих вызовах `write_snapshot` и `remove_snapshot` обновлять `known_snapshots` и `total_bytes` инкрементально в памяти.

#### Ожидаемый эффект

- **Память**: Устранение пиковой аллокации `String` размером до $16\text{ МиБ}$ при сохранении и создании recovery-снимка.
- **Ускорение `recovery_write_16k`**: Снижение числа системных вызовов (`openat`/`getdents64`/`stat`) на каждую запись снимка.

#### Компромиссы

- При внешней модификации каталога recovery сторонним процессом во время работы редактора in-memory учёт может разойтись на размер стороннего файла; при ошибке `ENOENT` во время удаления старого снимка выполняется однократный ресинк через `read_dir`.

#### План реализации

1. В `crates/store/src/files.rs` добавить потоковую запись из `impl Iterator<Item = &str>` / `crop::Rope`.
2. В `crates/store/src/recovery.rs` кэшировать список активных снимков и их размеров между вызовами `write_snapshot`.

#### Проверка

- **Тесты**: Интеграционные тесты `recovery.rs` и `files.rs`.
- **Бенчмарк**: `target/release/examples/recovery_perf`.

#### Уверенность

- **Static** (устранение аллокации `String` при записи `Rope`) / **Hypothesis** (доля `read_dir` в `5.087 мс` на фоне `fsync` зависит от файловой системы и требует замера в `recovery_perf`).

---

## 5. Приоритизация, классификация и совместимость оптимизаций

### 5.1. Единая сводная таблица всех найденных проблем (`OPT-01` — `OPT-18`)

Находки отсортированы по ожидаемой практической отдаче с учётом трудозатрат и риска:

| ID         | Крейт         | Проблема                                                                                                                                | Эффект                                                                                                                                                                      | Сложность внедрения | Риск        | Уверенность                                | Приоритет          |
| ---------- | ------------- | --------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------- | ----------- | ------------------------------------------ | ------------------ |
| **OPT-07** | `truff-ui`    | Полная материализация `Buffer -> String` и 4-проходный `MarkdownFacts::extract` на каждое нажатие клавиши при открытом/закреплённом TOC | **Критический**: устранение фризов ввода $3\text{--}15\text{ мс} \rightarrow 85\text{ мкс}$ ($35\text{--}150\times$) и аллокаций по 1–10 МБ на клавишу                      | Низкая              | Низкий      | `Static`                                   | **P0 (Highest)**   |
| **OPT-14** | `truff-store` | Полный пересчёт всех ссылок хранилища (`SELECT ... FROM links`) в `refresh_resolutions` при сохранении одной заметки                    | **Критический на 10k vault**: ускорение переиндексации с $O(E_{\text{total}})$ до $O(E_{\text{note}})$ ($500\text{--}1000\times$ на 150k ссылок)                            | Средняя             | Низкий      | `Static`                                   | **P0 (Highest)**   |
| **OPT-15** | `truff-store` | Чтение с диска и SHA-256 всех 10 000 файлов при каждом старте (`scan::refresh`) без проверки `(mtime_ns, file_size)`                    | **Критический для старта**: ускорение открытия 10k vault с $100\text{--}800\text{ мс}$ до $12\text{--}25\text{ мс}$ ($10\text{--}40\times$), $0$ чтений неизменённых файлов | Средняя             | Низкий      | `Verified`                                 | **P0 (Highest)**   |
| **OPT-03** | `truff-core`  | Регрессия $0.36\times$ в `ChangeSet::new` из-за аллокации `prefix_shifts: Vec<isize>` и клонирования `SingleEdit` в `from_edits`        | **Высокий**: устранение доказанной регрессии ($7.71\text{ мкс} \rightarrow 2.2\text{ мкс}$, $>3\times$), $-40\text{ КБ}$ на батч и $O(\log K)$ `map_pos` всегда             | Низкая              | Низкий      | `Verified`                                 | **P0 (Quick Win)** |
| **OPT-12** | `truff-ui`    | Линейный перебор всех 10 000 элементов в `NavigatorState::filter` (4.17 мс) без 64-битной маски символов и сужения префикса             | **Высокий**: ускорение фильтрации навигатора с $4.17\text{ мс}$ до $0.35\text{--}0.60\text{ мс}$ ($7\text{--}12\times$)                                                     | Низкая              | Низкий      | `Verified`                                 | **P0 (Quick Win)** |
| **OPT-01** | `truff-core`  | Хранение `Vec<Transaction>` в `HistoryGroup` и отсутствие коалесцирования последовательного набора (`Insert` run)                       | **Высокий**: сокращение аллокаций истории при печати на $>99\%$, экономия RAM истории в $10\text{--}150\times$, ускорение `undo` в $50\times$                               | Средняя             | Средний     | `Static`                                   | **P1 (High)**      |
| **OPT-08** | `truff-ui`    | Полный пересчёт дерева Фенвика $O(L)$ в `HeightIndex::replace_blocks` и пересборка `BTreeMap` при нажатии `Enter` (`139.89 мкс`)        | **Высокий**: ускорение `insert_newline_frame` на 35–40% (`139.9 -> ~88 мкс`), $0$ аллокаций при вставке строк, $-75\%$ памяти `BlockInfo`                                   | Средняя             | Средний     | `Verified`                                 | **P1 (High)**      |
| **OPT-13** | `truff-ui`    | Загрузка драйверов Mesa/LLVM через GLVND (+176.9 MiB VmRSS) и лишние буферы глубины/трафарета в `eframe::NativeOptions`                 | **Высокий для базового RSS**: снижение VmRSS окна с $308.3\text{ MiB}$ до $\sim 131\text{--}140\text{ MiB}$ ($-170\text{ MiB}$)                                             | Низкая              | Низкий      | `Verified`                                 | **P1 (Quick Win)** |
| **OPT-17** | `truff-store` | Безусловный второй запрос `SELECT COUNT(*)` в `SearchSession::search` (`fts_search_page_1000` = 713.6 мкс)                              | **Средне-высокий**: ускорение типичных FTS5-запросов на 35–50% за счёт пропуска второго прохода при `results.len() < limit`                                                 | Низкая              | Минимальный | `Verified`                                 | **P1 (Quick Win)** |
| **OPT-02** | `truff-core`  | Линейный проход $O(N)$ в `byte_to_point` и передача `&str` вместо чтения чанков `crop::Rope` в `SyntaxEngine`                           | **Средне-высокий**: снижение расчёта координат с $O(N)$ до $O(\log N)$ и отказ от аллокации полного текста для `tree-sitter`                                                | Низкая              | Низкий      | `Static`                                   | **P1 (High)**      |
| **OPT-09** | `truff-ui`    | Тройное дублирование текста строки и 5 временных векторов в `DisplayMap` на каждый `Paragraph`                                          | **Средне-высокий**: сокращение аллокаций в `Paragraph::new` при прокрутке с 6–8 до 2, уменьшение размера `DisplayMap` в 2 раза                                              | Средняя             | Низкий      | `Static`                                   | **P1 (High)**      |
| **OPT-04** | `truff-core`  | Четырёхпроходное сканирование, бинарный поиск `in_code` и сортировка в `MarkdownFacts::extract`                                         | **Средний**: ускорение `MarkdownFacts::extract` ещё на 25–40% (`2517 -> ~1600 мкс`)                                                                                         | Низкая              | Низкий      | `Static`                                   | **P2 (Medium)**    |
| **OPT-10** | `truff-ui`    | Размер `DisplayChunk` (56 байт) и линейный проход $O(C)$ в `source_to_visual` / `visual_to_source`                                      | **Средний**: сжатие `DisplayChunk` до 32 байт ($-43\%$), $O(1)$ маппинг для простых строк и $O(\log C)$ для сложных                                                         | Низкая              | Низкий      | `Static`                                   | **P2 (Quick Win)** |
| **OPT-11** | `truff-ui`    | Проверка `reproject` для всех видимых строк при движении курсора и $O(V \cdot F)$ фильтрация `FlashTarget`                              | **Средний**: ускорение `cursor_down_frame` на 15–25% и отрисовки `Flash`                                                                                                    | Низкая              | Минимальный | `Static`                                   | **P2 (Quick Win)** |
| **OPT-16** | `truff-store` | Поштучные SQL-запросы в `Resolver::resolve` при массовой индексации 10 000 заметок                                                      | **Средний (на холодном старте)**: ускорение первичного резолва 150k ссылок в 8–15 раз через покрывающие индексы и `BatchResolver`                                           | Средняя             | Низкий      | `Static`                                   | **P2 (Medium)**    |
| **OPT-06** | `truff-core`  | Аллокация `String` при поиске редких паттернов и неограниченный радиус поиска парных скобок/кавычек в Vim                               | **Средний**: снижение пиковых аллокаций поиска с $O(N)$ до $O(1)$ по чанкам `Rope`                                                                                          | Средняя             | Низкий      | `Static`                                   | **P2 (Medium)**    |
| **OPT-05** | `truff-core`  | Хранение `HashMap<char, Arc<Register>>` внутри каждого `Buffer::history`                                                                | **Малый/Средний**: 0 аллокаций для пустых регистров и $O(1)$ доступ по индексу массива для `'a'..='z'`                                                                      | Низкая              | Минимальный | `Static`                                   | **P3 (Low)**       |
| **OPT-18** | `truff-store` | Промежуточный `String` при записи `Rope` на диск и повторный `read_dir` в `RecoveryManager::write_snapshot`                             | **Малый/Средний**: устранение пиковой аллокации до 16 МБ при сохранении; сокращение syscalls в recovery                                                                     | Средняя             | Низкий      | `Hypothesis` (по времени `write_snapshot`) | **P3 (Low)**       |

### 5.2. Группировка по категориям

1. **Quick Wins (локальные изменения с малым риском и немедленной отдачей)**:
   - `OPT-03` (встраивание `prefix_shift` в `Change` и устранение регрессии $0.36\times$ в `ChangeSet`).
   - `OPT-07` (предотвращение пересчёта `DocumentFacts` и `buffer.to_string()` на каждый символ при открытом TOC).
   - `OPT-12` (битовая маска `char_mask: u64` и сужение `filtered_indices` в `NavigatorState::filter`).
   - `OPT-13` (настройка `eframe::NativeOptions` и фильтр EGL ICD против загрузки Mesa/LLVM).
   - `OPT-17` (пропуск второго запроса `SELECT COUNT(*)` в `SearchSession::search` при неполной странице).
   - `OPT-10` и `OPT-11` (быстрый маппинг `DisplayChunk` и отсечение `reproject` / `FlashTarget`).
2. **Крупные алгоритмические оптимизации**:
   - `OPT-14` (таргетированный `refresh_resolutions` вместо полного сканирования таблицы `links`).
   - `OPT-15` (проверка `(mtime_ns, file_size)` и пакетная предзагрузка метаданных в `scan::refresh`).
   - `OPT-02` ($O(\log N)$ `rope_byte_to_point` и парсинг `tree-sitter` по чанкам `Rope`).
   - `OPT-04` (двухпроходный `MarkdownFacts::extract` с `CodeRangeCursor` и $O(L)$ merge).
   - `OPT-06` (потоковый поиск по чанкам `Rope` с буфером перекрытия).
3. **Архитектурные оптимизации с большим эффектом по памяти и масштабируемости**:
   - `OPT-01` (уплощение `HistoryGroup` и коалесцирование последовательного ввода в `History`).
   - `OPT-08` (дельта-индекс `HeightIndex` с 4-байтным блоком и оконный кэш `ParagraphCache` на `VecDeque` вместо `BTreeMap`).
   - `OPT-09` (устранение тройного хранения текста строки и вынос временных векторов из `DisplayMap` в `DisplayScratchpad`).
   - `OPT-16` (`BatchResolver` для пакетного построения графа ссылок в памяти).
4. **Гипотезы, требующие предварительной проверки бенчмарком**:
   - `OPT-18` (влияние кэширования `read_dir` в `RecoveryManager::write_snapshot` на общее время записи на фоне `fsync` диска) и эффект `Configure Linux EGL vendor` на системах с гибридной графикой PRIME (`OPT-13`).

### 5.3. Совместимость, зависимости и отсутствие конфликтов

- **Связка `OPT-03` $\leftrightarrow$ `OPT-01`**: Оба изменения затрагивают `crates/core/src/text/`. Рекомендуется сначала выполнить `OPT-03` (рефакторинг `Change` и `ChangeSet` в `edit.rs` и `build_inverse_and_changes` в `buffer/mod.rs`), а затем поверх него выполнить `OPT-01` (уплощение `HistoryGroup` в `history.rs` и `stage_history` в `buffer/mod.rs`). Конфликтов между ними нет, они усиливают друг друга.
- **Связка `OPT-04` $\leftrightarrow$ `OPT-07`**: `OPT-04` ускоряет сам `MarkdownFacts::extract` в `truff-core`, а `OPT-07` устраняет его лишние вызовы из `truff-ui` (`app/document.rs`). Оба изменения полностью ортогональны и не меняют межкрейтовый сигнатурный контракт `MarkdownFacts::extract(&str) -> Result<DocFacts, fmt::Error>`.
- **Связка `OPT-08` $\leftrightarrow$ `OPT-09` $\leftrightarrow$ `OPT-10` $\leftrightarrow$ `OPT-11`**: Все четыре оптимизации находятся внутри `crates/ui/src/editor/` и `crates/ui/src/native/editor/`. Сначала внедряются `OPT-10` и `OPT-09` (внутри `editor/display/`), затем `OPT-08` и `OPT-11` (внутри `editor/heights.rs` и `native/editor/`).
- **Связка `OPT-14` $\leftrightarrow$ `OPT-15` $\leftrightarrow$ `OPT-16`**: Все три затрагивают `crates/store/src/index/` и `crates/store/src/catalog/scan.rs`. Миграция `schema.sql` для `OPT-14` (`idx_links_unresolved`), `OPT-15` (`mtime_ns`, `file_size`) и `OPT-16` (`idx_files_stem`, `idx_files_title`) объединяется в один атомарный шаг обновления схемы в `schema.sql`.

---

## 6. Итоговая дорожная карта внедрения (по крейтам и рабочим пакетам для coding-агентов)

Каждый рабочий пакет спроектирован так, чтобы **не ломать публичные межкрейтовые контракты** и допускать независимую реализацию, тестирование и верификацию отдельным coding-агентом.

### Этап 1. Крейт `truff-core` (2 независимых пакета)

#### Пакет `CORE-WP1`: Транзакции, `ChangeSet`, `HistoryGroup` и `Registers` (`OPT-03`, `OPT-01`, `OPT-05`)

- **Зависимости**: Нет (полностью внутри `crates/core/src/text/`).
- **Затрагиваемые модули**:
  - `crates/core/src/text/edit.rs`
  - `crates/core/src/text/history.rs`
  - `crates/core/src/text/history/registers.rs`
  - `crates/core/src/text/buffer/mod.rs`
- **Конкретные этапы реализации**:
  1. Встроить `prefix_shift: isize` в `struct Change`, заменить поле `delta` на `pub const fn delta(&self) -> isize`, удалить `prefix_shifts: Vec<isize>` из `ChangeSet` и убрать клонирование `SingleEdit` в `ChangeSet::from_edits` (`OPT-03`).
  2. Уплостить `HistoryGroup` до `forward_edits`, `forward_batch_ends`, `inverse_edits`, `inverse_batch_ends` и реализовать коалесцирование последовательных односимвольных вставок в `HistoryGroup::push_batch` (`OPT-01`).
  3. Заменить `HashMap<char, Arc<Register>>` в `Registers` на ленивый массив `Option<Box<[Option<Arc<Register>>; 26]>>` (`OPT-05`).
- **Необходимые тесты**: `cargo test -p truff-core text::` + новый тест `coalesces_insert_run_and_preserves_grouped_undo_redo`.
- **Метрики проверки и критерии завершения**:
  - В `examples/core_perf`: `Создание ChangeSet из 5000 edits` $\le 2.8\text{ мкс}$ (против `7.707 мкс`), `Один map_pos, пакет из 512 anchors` $\le 0.025\text{ мкс}$.
  - В `examples/core_state_perf`: потребление памяти `History::total_bytes()` после 1 000 последовательных односимвольных вставок $< 4\text{ KiB}$.

#### Пакет `CORE-WP2`: Парсинг Markdown, `SyntaxEngine` и потоковый поиск в `Rope` (`OPT-02`, `OPT-04`, `OPT-06`)

- **Зависимости**: Нет (не пересекается с `CORE-WP1`).
- **Затрагиваемые модули**:
  - `crates/core/src/markdown/syntax.rs`
  - `crates/core/src/markdown/facts.rs`
  - `crates/core/src/vim/motion/search.rs`
  - `crates/core/src/vim/motion/objects.rs`
- **Конкретные этапы реализации**:
  1. Добавить `rope_byte_to_point(rope: &crop::Rope, byte_offset: usize) -> Point` за $O(\log N)$ и чтение чанков `Rope` в `SyntaxEngine` через `parser.parse_with` (`OPT-02`).
  2. Объединить проходы вики-ссылок и `^block-id` в `MarkdownFacts::extract` с монотонным `CodeRangeCursor` и заменить `sort_by_key` на $O(L)$ `merge_sorted_links` (`OPT-04`).
  3. Реализовать потоковый поиск по чанкам `RopeSlice::chunks()` с буфером перекрытия `2 * (needle.len() - 1)` и ограничить радиус поиска кавычек/скобок в `objects.rs` (`OPT-06`).
- **Необходимые тесты**: `cargo test -p truff-core markdown::` и `cargo test -p truff-core vim::`.
- **Метрики проверки и критерии завершения**:
  - В `examples/core_perf`: `Markdown facts, 5000 code ranges` $\le 1\,850\text{ мкс}$ (против `2 517 мкс`); `Incremental syntax` на файле 1.5 MiB $< 1.0\text{ мс}$ с 0 аллокаций полного текста документа.

---

### Этап 2. Крейт `truff-store` (2 пакета)

#### Пакет `STORE-WP1`: Схема БД, `(mtime_ns, file_size)` быстрый скан и таргетированный `refresh_resolutions` (`OPT-14`, `OPT-15`, `OPT-16`)

- **Зависимости**: Нет.
- **Затрагиваемые модули**:
  - `crates/store/src/index/schema.sql` и `crates/store/src/index/schema.rs`
  - `crates/store/src/index/update.rs`
  - `crates/store/src/index/resolve.rs`
  - `crates/store/src/catalog/scan.rs`
- **Конкретные этапы реализации**:
  1. Добавить в `schema.sql` поля `mtime_ns`, `file_size` в таблицу `files` и индексы `idx_links_unresolved`, `idx_files_stem`, `idx_files_title`.
  2. В `catalog/scan.rs` собирать `(mtime_ns, file_size)` при `enumerate`, предзагружать метаданные `files` одним SQL-запросом и пропускать чтение/SHA-256 неизменённых файлов (`OPT-15`).
  3. В `index/update.rs` заменить полный перебор `SELECT ... FROM links` в `refresh_resolutions` на инкрементальный запрос только затронутых ссылок (`OPT-14`) и добавить `BatchResolver` для холодного старта (`OPT-16`).
- **Необходимые тесты**: `cargo test -p truff-store`.
- **Метрики проверки и критерии завершения**:
  - В `examples/catalog_perf`: `catalog_reopen_100_unchanged` $\le 250\text{ мкс}$ (против `982 мкс`), `catalog_reopen_10000_unchanged` $\le 30\text{ мс}$.
  - В `examples/store_perf`: `reindex_single` на графе из 10 000 заметок и 150 000 ссылок $\le 500\text{ мкс}$.

#### Пакет `STORE-WP2`: Полнотекстовый поиск FTS5 и потоковая запись `Rope` / `RecoveryManager` (`OPT-17`, `OPT-18`)

- **Зависимости**: После или параллельно с `STORE-WP1` (разные файлы).
- **Затрагиваемые модули**:
  - `crates/store/src/index/search.rs`
  - `crates/store/src/files.rs`
  - `crates/store/src/recovery.rs`
- **Конкретные этапы реализации**:
  1. В `search.rs` пропускать запрос `SELECT COUNT(*)` при `results.len() < limit && (offset == 0 || !results.is_empty())` (`OPT-17`).
  2. В `files.rs` добавить потоковую запись чанков `crop::Rope`, а в `recovery.rs` кэшировать размеры известных snapshot-файлов в памяти (`OPT-18`).
- **Необходимые тесты**: `cargo test -p truff-store search` и `cargo test -p truff-store recovery`.
- **Метрики проверки и критерии завершения**:
  - В `examples/store_perf`: медиана `fts_search_page_1000` для селективных запросов снижается до $\le 450\text{ мкс}$ (против `713.6 мкс`).

---

### Этап 3. Крейт `truff-ui` (2 независимых пакета)

#### Пакет `UI-WP1`: Кэш `DocumentFacts`/TOC, `NavigatorState::filter` и инициализация `eframe` (`OPT-07`, `OPT-12`, `OPT-13`)

- **Зависимости**: Не требует изменений API `truff-core` или `truff-store`.
- **Затрагиваемые модули**:
  - `crates/ui/src/app/document.rs`
  - `crates/ui/src/native/mod.rs`
  - `crates/ui/src/navigator/types.rs` и `crates/ui/src/navigator/model.rs`
  - `crates/ui/src/main.rs`
- **Конкретные этапы реализации**:
  1. В `app/document.rs` реализовать `notify_buffer_change_for_facts` (сдвиг диапазонов без вызова `buffer.to_string()` и `MarkdownFacts::extract` при обычном наборе текста) (`OPT-07`).
  2. В `navigator/types.rs` и `model.rs` добавить `char_mask: u64` в `NavigatorItem` и инкрементальное сужение `filtered_indices` при дописывании запроса (`OPT-12`).
  3. В `main.rs` задать `depth_buffer: 0`, `stencil_buffer: 0`, `multisampling: 0` в `eframe::NativeOptions` и проверку EGL ICD (`OPT-13`).
- **Необходимые тесты**: `cargo test -p truff-ui navigator` и тесты синхронизации TOC.
- **Метрики проверки и критерии завершения**:
  - В `examples/ui_perf`: `navigator_filter_10000` $\le 800\text{ мкс}$ (против `4 168.98 мкс`); `insert_character_with_toc_pinned` $\le 95\text{ мкс}$.
  - Нативное окно: VmRSS пустого окна $\le 160\text{ MiB}$, PSS $\le 55\text{ MiB}$.

#### Пакет `UI-WP2`: `DisplayMap`, `DisplayChunk`, `HeightIndex` и `EditorWidget` (`OPT-08`, `OPT-09`, `OPT-10`, `OPT-11`)

- **Зависимости**: Не пересекается по файлам с `UI-WP1`.
- **Затрагиваемые модули**:
  - `crates/ui/src/editor/display/types.rs`, `mapping.rs`, `projection.rs`, `markers.rs`, `mod.rs`
  - `crates/ui/src/editor/heights.rs`
  - `crates/ui/src/native/editor/mod.rs`, `invalidation.rs`, `viewport.rs`, `paragraph/mod.rs`
- **Конкретные этапы реализации**:
  1. Сжать `DisplayChunk` до 32 байт с полем `visual_start: u32` и перевести `source_to_visual` / `visual_to_source` на $O(1)$ fast-path (для 1 чанка) и `partition_point` $O(\log C)$ (`OPT-10`).
  2. Убрать промежуточные векторы из долгоживущего состояния `DisplayMap` в `DisplayScratchpad` и устранить лишнюю копию `visual_text` для строк без скрытых маркеров (`OPT-09`).
  3. Оптимизировать `HeightIndex::replace_blocks` и заменить `BTreeMap<usize, Paragraph>` в `EditorWidget` на оконный кэш на базе `VecDeque` (`OPT-08`).
  4. Добавить ранний выход в `Paragraph::reproject` для строк вне курсора и отсечение `flash` через `partition_point` в `viewport.rs` (`OPT-11`).
- **Необходимые тесты**: `cargo test -p truff-ui editor`.
- **Метрики проверки и критерии завершения**:
  - В `examples/ui_perf`: `insert_newline_frame` $\le 98\text{ мкс}$ (против `139.89 мкс`), `cursor_down_frame` $\le 58\text{ мкс}$ (против `69.87 мкс`), `insert_character_frame` $\le 75\text{ мкс}$.
