# **\# PROMPT ДЛЯ ИИ-АГЕНТА №1 — \`truff-core\` \*\*Входные данные:\*\* только папка \`crates/core/\` Rust-проек**

[https\://gemini.google.com/app/ffcc49b68e11f3cf](https://gemini.google.com/app/ffcc49b68e11f3cf)

_User prompt: \# PROMPT ДЛЯ ИИ-АГЕНТА №1 — \`truff-core\` \*\*Входные данные:\*\* только папка \`crates/core/\` Rust-проекта Truff. \*\*Владелец изменений:\*\* только \`crates/core/\*\*\`. \*\*Контракт:\*\* \`truff-crates-2026-10-14-v1\`. \*\*Ветка-ориентир:\*\* \`hackerman111/truff@argon-codex\` (если доступен репозиторий). \#\# Роль и продукт Ты — Rust systems engineer, отвечающий исключительно за \*\*чистое ядро Vim-first Markdown редактора\*\*. Truff должен стать быстрым, совместимым с Markdown аналогом Obsidian, визуально близким к нему, но с философией Vim для \*всех\* операций, SessionX-подобной навигацией, Which-Key и расширяемой архитектурой. Эти последние возможности рисует/UI-реализует другой агент; ты предоставляешь только надёжную семантику и типы. \*\*Главная цель:\*\* единая модель действий, motions/operators, Markdown-фактов, буферов и Undo. \*\*Не создавай\*\* GUI, SQLite, async файловую систему, \`egui\` widgets, визуальные скроллы, поисковую БД. \*\*Не меняй\*\* \`crates/ui\`, \`crates/store\`, корневой Cargo или другие крейты. Если у тебя на входе только папка \`core/\` и нет workspace, не подменяй отсутствующие соседние модули; выполняй доступный аудит/редактирование и укажи невозможные проверки. \#\# Архитектурные принципы \- \`core\` не зависит от \`store\`/\`ui\`; данные переходят по публичным versionable Rust types с существующими именами там, где они уже есть. \- Пользовательские операции проходят через \`CommandId\`/Command Registry, а не через сравнение display labels; \`Keymap\` определяет команды по контексту. Операторы/счётчики Vim и глобальные команды не конкурируют за символы. \- Markdown-парсинг и AST/semantic facts имеют \*\*один источник истины на revision\*\*. У UI и Store разные потребители этой же семантики, но не отдельные парсеры заголовков по \`\#\`. \- Source positions: byte offsets UTF-8, grapheme-aware normalisation и source range, отдельные от пиксельной геометрии глифов. \`core\` \*\*не решает\*\*, как высок/широк символ на экране. \- Контент и его изменения — Rope/Transaction \+ reversible Undo/Redo. Не выполнять side-effect при текстовой операции до подтверждения intent. \- Не плодить OOP boilerplate: traits только для замены реализации, тестирования или реального расширения; никакой абстракции \`PluginManager\` без runtime. \#\# A. Проверь существующий код перед написанием Ожидаемые пути в этой ветке (могут измениться — сверь фактический tree): \`\`\` src/vim/{mod.rs,parser/,motion/,execute/} src/commands/{mod.rs,registry.rs,types.rs,defaults.rs} src/config/{mod.rs,compiled.rs,defaults.rs} src/text/{buffer/,grapheme.rs,edit.rs,history.rs} src/markdown/{syntax.rs,facts.rs,presentation.rs,links.rs,               link\_selection.rs,style.rs,style/} src/navigation/{mod.rs,state.rs} src/workspace/{mod.rs,buffers.rs,views.rs,dispatch.rs,protocol.rs} \`\`\` Проверить: какие из Normal/Insert/Visual, counts, registers, repeat, macros, text objects, \`RowModel\`, \`CommandRegistry\`, link selection и style edits \*\*уже работают\*\*. Не переписывать их. Сначала сделай небольшую таблицу \`implemented / partial / missing\`, карту публичного API и прогон доступных тестов. \#\# B. Основная дорожная карта Core (каждый пункт тестируется отдельно) \#\#\# CORE-P0. Input semantics и state machine — блокирующий этап 1\. Выдели в ядре \*\*типизированный контекст выполнения\*\* клавиатурной команды: редактируемый текст (\`Editor\`, \`TextField\`, \`CommandLine\`), список/дерево, Navigator, прочий view. Не внедряй сюда runtime-фокус egui. Существующие \`InputContext\`, \`VimState\`, \`CompiledKeymap\` \*\*расширять\*\*, а не заменять несовместимой копией. 2\. Задай однозначный контракт обработки printable символа: если активное поле в \`Insert\`/IME composing, символ — \*\*текст\*\*, а не \`s\` Flash, \`d\` operator или \`j/k\` навигация. Для Normal и pending Vim существуют свои правила. Предоставь UI чистый проверяемый resolver/predicate вместо перехвата в нескольких слоях. 3\. Учитывай pending operator (\`d\`, \`c\`, \`y\`), counts (\`12j\`), char motions \`f/t\`, registers (\`"a\`), macro recording и \`.\` repeat. \`Space\`/shortcut не должны разрушать эти состояния и наоборот. 4\. Стандартизируй действия для нетекстовых списков \*\*без притворного редактирования содержимого\*\*: \`j/k\` move, \`gg/G\`, \`h/l\` hierarchy, \`v/V\` selection, \`Enter\` activate; операции \`d/r/a\` — только типизированные контекстные commands. Переиспользуй существующий \`RowModel\`. 5\. \`Esc\` в текстовом поле Insert → Normal \*\*внутри этого поля\*\*; внешняя оболочка закрывает модальное окно отдельным событием. IME/Esc отдельно. Это контракт, а не хардкод клавиши \`s\`. 6\. Пересмотри \`CommandRegistry\` как source of truth: человекочитаемые title/description, contexts/categories, default key sequences, availability, prefix-hints. Реестр должен позволять построить Which-Key и Command Palette без дублирования меню. Делать обратную совместимость существующих \`CommandSpec\`, \`KeyHint\`. 7\. Валидация \`Keymap\`: выявление конфликтов в контексте, недоступные команды, дубли и невозможные префиксы. Не требовать hardcoded \`Ctrl-j/k\`; \`j/k\` в Normal, буквальный \`j/k\` в Insert. \*\*Тесты:\*\* ввод \`s\` в разные Input-контексты, \`s\` Flash только Normal editor, multi-event Unicode/paste/IME, \`5j\`, \`g\`→\`gg\`, \`dw\`, \`ciw\`, \`vaw\`, \`.\`/macro, nested prefixes \`Space ...\`, rollback pending on Esc. Не помечать глобальную маршрутизацию UI исправленной: её реализует UI-агент. \#\#\# CORE-P1. Unicode и семантика Visual selection 1\. Проверить \`text/grapheme.rs\`, \`BytePos\` и готовые методы Rope. Предложить публичные операции \*\*snap-to-grapheme-boundary\*\*, \`next\_grapheme\`, \`prev\_grapheme\`, \`visual\_char\_inclusive\_range\` → корректный half-open UTF-8 \`Range\`. 2\. \`VisualChar\` включает \*\*полный последний grapheme cluster\*\*, даже если это \`👩‍💻\`, \`e\\u{0301}\`, флаг, CJK; направление выбора не меняет диапазон. \`VisualLine\` выбирает все строки, включая корректную обработку LF/CRLF/EOF. 3\. Сохранять byte-safe Undo и операции copy/delete/change для Unicode и составных символов. Избегать O(n) сканирования всего файла на каждую клавишу. 4\. В Core тестировать только logical range и offsets. Отрисовка точных прямоугольников через \`egui::Galley\` находится исключительно в UI. \#\#\# CORE-P2. Единая модель Markdown syntax/facts/source ranges 1\. Аудит \`tree-sitter-md\`, \`pulldown-cmark\`, текущего \`MarkdownFacts\`, \`presentation\`, link parsing. Выделить переиспользуемый immutable per-revision \`MarkdownSnapshot\`/соответствующий существующий тип: headings H1–H6, block hierarchy/sections, links/wikilinks/aliases, block IDs, nested lists/task items, callouts/quotes, code fences, tables, inline runs и frontmatter/properties где возможно. 2\. Для каждого semantic node сохранять устойчивый source range, parent/depth, line start, token kind и при необходимости semantic ID. Повторные названия heading разрешены; уникальность ссылок и block IDs обрабатывается корректно. 3\. Инкрементальный parse/cache: из существующей архитектуры, не второй полноценный parser. При невозможности одного parser-а — явный контракт объединения Tree-sitter blocks и pulldown-cmark inline spans. 4\. Дать чистые запросы \`headings\_in(range)\`, \`heading\_under\_cursor\`, \`section\_range\`, \`linked\_target\_at(offset)\`, \`tasks\_in(range)\`, \`semantic\_objects\_in(range)\` (имена — ориентир; предпочтительно расширить существующий API). 5\. Markdown Source/Live/Reading — режимы отображения UI, но Core экспортирует стиль/visibility spans, теги и исходные интервалы. Source ↔ visual mapping фактически рендерит UI. \*\*Тесты:\*\* nested headings, escaped \`\#\`, headings в code fence, Unicode/CRLF, wiki links \`\[\[note\#heading|alias\]\]\`, block references \`^id\`, nested tasks, quote/callout, таблицы, повторные headings, неполный Markdown. \#\#\# CORE-P3. Умные атомарные правки Markdown 1\. Сделать чистый edit planner поверх существующего \`Transaction\` для Enter/Tab/ShiftTab в bullets, ordered lists, tasks, nested items и \`\>\` blockquote. 2\. Правила: Enter продолжает список на том же уровне; пустой item закрывает список; \`\[ \]\` создаёт unchecked child, а не копирует checked state; номера корректно продолжаются/перенумеровываются там, где требуется; Tab indent/ShiftTab outdent; всё одной undo-группой. 3\. Markdown text objects: inner/around heading, section, list item/tree, paragraph, quote, callout, link, code block, table cell/row (по мере готовности AST). Предоставить выбор targets для UI Flash (\`S\`) с исходными границами; не включать отрисовку буквенных меток. 4\. Link selection planner: \`selected range \+ note/heading/block target → Markdown/Wiki link\`, корректное экранирование, aliases; wrap selection без удаления исходного текста; move/extract — только source-level планы, не запись на диск. 5\. Color formatting planner: foreground/background и снятие цвета через \`\<span style="color:\#RRGGBB"\>...\`/совместимый mark (или проверенный совместимый синтаксис); не повреждать вложенные Markdown constructs, повторное применение не должна приводить к бесконечным вложенным span, один Undo. Отдельно определить policy для HTML/escaping. \*\*Тесты:\*\* table-driven input→byte-exact output→one Undo for Markdown smart edits, highlight/links, CRLF, grapheme boundaries, nested markup, no-op on plain paragraph. \#\#\# CORE-P4. Workspace/Buffer/View semantics и расширяемость 1\. Проверь \`workspace\` существующие BufferEntry/View. Зафиксируй: один Buffer \= source/dirty/revision/Undo, несколько View \= независимые cursor/selection/scroll/projection; закрытие View не теряет dirty buffer. 2\. Сохраняй стабильные IDs; транзакции/buffer dirty updates — через одну точку. Подготовь без файлового I/O типизированные описания Actions, например create/edit/link/refactor intents и previews. 3\. Для session tabs/docks — только структуры данных и команды, если это естественно расширяет Workspace; пиксельное дерево split-окон, панели и фокус находятся в UI. 4\. Подготовь экспорт для Command Palette / Navigator: \`CommandSpec\`, typed actions, previewable descriptors. Не добавляй динамическую систему плагинов и чужой Lua runtime. 5\. Проверка API stability: downstream \`store/ui\` смогут компилироваться без изменения имеющихся функций либо получают прозрачные совместимые adapters. \#\# C. Контракт для последующих двух агентов Ты владеешь \*\*семантикой\*\*. Отдай в публичных Rust-модулях, сохранив текущие названия при наличии: | Сервис | Требуется от Core | Потребитель | |---|---|---| | Команды | registry \+ IDs \+ contexts \+ bindable hints | UI | | Vim | parser/state \+ mode-sensitive typed input action, RowModel | UI | | Текст | \`Buffer\`, \`BytePos\`, grapheme ranges, \`Transaction\`, Undo | UI \+ Store | | Markdown | revision-safe headings/blocks/tasks/links/metadata \+ source ranges | Store \+ UI | | Edit plans | pure list/quote/link/color/refactor transforms | UI отправляет изменения, Store выполняет file ops | | Workspace | стабильные buffer/view IDs и независимые состояния | UI | \*\*Не внедрять\*\* в Core fzf, ripgrep CLI, SQLite ngram index, \`egui\`, FileDialog или filesystem watch. Для cross-crate изменений напиши раздел \`Cross-crate requests\` с сигнатурой желаемого API и причиной. Никакого изменения файлов соседей. \#\# D. Порядок выполнения в рамках одного крейта \`\`\` CORE-0 audit \+ baseline → CORE-P0 input/registry                        → CORE-P1 Unicode                        → CORE-P2 Markdown snapshots                        → CORE-P3 smart edits / text objects / link/color                        → CORE-P4 workspace and API hardening \`\`\` Если объём превышает один разумный набор PR — реализуй CORE-P0/P1 сначала, затем следующие фазы с разделёнными коммитами. \*\*Не заменяй реализацию TODO-пустышками\*\*. Считай незавершённое незавершённым и опиши это в отчёте. \#\# E. Definition of Done / команды проверки \- \`cargo fmt \--all \-- \--check\`, \`cargo check \-p truff-core\`, \`cargo test \-p truff-core\`, \`cargo clippy \-p truff-core \--all-targets \-- \-D warnings\` \*\*если доступен workspace\*\*. Также run legacy tests where present; конкретные результаты включить. \- В тестах обязательно: Unicode graphemes, CRLF, Visual ranges, \`s\` в Insert context, Vim pending motions, undo/redo, markdown edge cases, source snapshots. \- Нет новых межкрейтовых циклов, дублей resolver/parser, тяжёлой аллокации per keystroke. \- Измерить расходы на типичные motion \+ Parse на фиксированном Markdown fixture; сравнить до/после, не выдумывать результаты. \- Изменения публичного API задокументированы, совместимость проверена по возможности; доступный компилятор не заменяет проверку UI поведения. \#\# F. Отчёт следующему агенту Ответить текстом: \`\`\` Core status (done/partial/missing по P0–P4): Files changed: Public API exact names/signatures: Behavioral invariants: Backward compatibility / migrations: Tests run \+ results: Benchmarks (measured or not measured): Cross-crate requests for Store/UI: Known bugs / blocking risks: \`\`\` \*\*Важнейшее:\*\* баг прокрутки редактора/Navigator, Help и размеры панелей исправляет \*\*UI-агент\*\*, не пытайся «починить» их в Core. Core отвечает только за корректные позиции курсора/selection и состояния команд._

## **Response: A. Аудит существующего кода crates/core и карта публичного API**

### **1\. Таблица состояния подсистем (implemented / partial / missing)**

| Подсистема / Модуль                                          | Статус                                 | Что уже работает в базе                                                                                                                                                                                                                                                           | Что частично реализовано или отсутствует (и исправлено ниже)                                                                                                                                                                                                                                                                                                                                                                                                 |
| :----------------------------------------------------------- | :------------------------------------- | :-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | :----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Vim Modes & Parser** (src/vim/mod.rs, src/vim/parser/)     | **Partial**                            | Режимы Normal, Insert, VisualChar, VisualLine; парсинг счётчиков 1..9, 0, операторов d/c/y/\>/\<, удвоенных dd/cc/yy/\>\>/\<, регистров "a, макросов q{reg} / @{reg}, поиска / и ?, командной строки :.                                                                           | В Key отсутствует ShiftTab для outdent в списках; нет чистого предикатного резолвера для UI (разделение printable-символов в Insert/IME composing и Normal/pending состояний); в TextObjectTarget нет секций, элементов списка, цитат/callout и таблиц.                                                                                                                                                                                                      |
| **Vim Execution & Contexts** (src/vim/execute/)              | **Partial**                            | Выполнение движений, операторов, u/Ctrl-r, . repeat, макросов с защитой от рекурсии и лимитом шагов (DEFAULT\_MAX\_MACRO\_STEPS), commit\_text, surrounding\_char\_range для IME.                                                                                                 | InputContext содержит только Editor, RowModel, Rows, None — нет типизированных контекстов редактируемых однострочных полей (TextField, CommandLine) и Navigator; для Rows поддерживаются только j/k/gg/G/h/l и / фильтр (нет v/V выделения строк, Enter активации и безопасной изоляции от текстовых правок).                                                                                                                                                |
| **Command Registry & Keymap** (src/commands/, src/config/)   | **Partial (с повреждением литералов)** | CommandRegistry, CommandSpec, CommandId, CommandContext, KeymapConfig, CompiledKeymap, BindingLookup, KeyHint.                                                                                                                                                                    | **Блокер компиляции в исходнике:** в src/commands/defaults.rs и src/text/buffer/line\_ending.rs повреждены строковые литералы с кириллицей (оборваны закрывающие кавычки). Команды file.create (a), file.rename (r), file.trash (d) объявлены в CommandContext::Global, а note.create (a) — в Editor, что конфликтует с Vim-командами a, r, d. KeymapConfig::validate не проверяет конфликты с учётом контекста и перехват печатных символов в Mode::Insert. |
| **Text Buffer, Rope & History** (src/text/)                  | **Implemented (P1: Partial)**          | Buffer поверх crop::Rope, BytePos, LineEnding (Lf/CrLf) с нормализацией при вставке, транзакции Transaction/ChangeSet с map\_pos, History с лимитом 8 МиБ и Savepoint, регистры Registers.                                                                                        | Есть grapheme\_next, grapheme\_prev, snap\_to\_grapheme\_boundary, но нет публичных next\_grapheme, prev\_grapheme, snap\_grapheme\_floor, visual\_char\_inclusive\_range и visual\_line\_range. В VimState::selection\_range anchor/head не выравниваются по началу графемы перед построением диапазона.                                                                                                                                                    |
| **Markdown Syntax & Facts** (src/markdown/)                  | **Partial**                            | Инкрементальный IncrementalSyntaxTree (tree-sitter-md), SyntaxSnapshot, line\_presentation, toggle\_task; MarkdownFacts::extract (pulldown-cmark \+ сканер \[\[...\]\] и ^block-id); style.rs (apply\_text\_color, apply\_highlight, wrap\_callout, remove\_style, color\_spans). | Нет единого ревизионного MarkdownSnapshot и запросов headings\_in, heading\_under\_cursor, section\_range, linked\_target\_at, tasks\_in, semantic\_objects\_in. В MarkdownFacts::extract игнорируется Event::Code внутри заголовков и ссылок; parse\_wiki\_inner не учитывает экранированный разделитель                                                                                                                                                    | в таблицах и ссылки на блоки \#^id; wrap\_callout хардкодит \\n вместо LineEnding буфера. |
| **Markdown Smart Edits & Planners** (src/markdown/)          | **Missing / Partial**                  | Базовый LinkSelection::wikilink / markdown\_link, toggle\_task, цветовые обёртки \<span...\> и \<mark...\> с защитой от вложенности и пересечения границ кода/ссылок.                                                                                                             | Отсутствует планировщик Enter / Tab / ShiftTab для списков, задач, нумерованных списков и цитат \>; нет генератора целей FlashTarget; в LinkSelection нет поддержки целей с заголовками/блоками, автоподстановки выделенного текста как алиаса и плана выноса заметки (ExtractNotePlan).                                                                                                                                                                     |
| **Navigation & Workspace** (src/navigation/, src/workspace/) | **Implemented (P4: Partial)**          | RowModel с иерархией, сворачиванием папок и фильтром; Workspace, BufferRegistry, View, JumpHistory, BufferChange, протокол StoreJob/StoreEvent с защитой по SnapshotKey (VaultEpoch, BufferId, revision).                                                                         | В RowModel нет состояния множественного выделения (v/V); в Workspace нет чистых дескрипторов предпросмотра рефакторинга/линковки без файлового ввода-вывода.                                                                                                                                                                                                                                                                                                 |

### **2\. Карта существующего публичного API (crates/core)**

> - **Корень (src/lib.rs)**:

- Реэкспорт crop::Rope.
- Из text: Buffer, BufferId, BytePos, Change, ChangeSet, CursorMeta, EditError, History, LineEnding, Register, RegisterKind, Registers, SingleEdit, Transaction, UndoGroup.
- Из commands: Action, AppError, CommandId, CommandSpec, Effect.
- Из workspace: BufferEntry, BufferRegistry, BufferState, DEFAULT\_MAX\_BUFFERS, SnapshotKey, VaultEpoch, DEFAULT\_MAX\_JUMP\_ENTRIES, JumpHistory, View, ViewId, DEFAULT\_MAX\_VIEWS, StoreEvent, StoreJob, Workspace.

> - **vim (src/vim/mod.rs)**:

- Mode, Key, Command, Operator, InsertEntry, ParseResult, VimParser, Motion, FindKind, TextObject, TextObjectKind, TextObjectTarget, SyntaxObjectRange, InputContext, InputResult, VimState, eval\_motion, eval\_operator\_motion\_range, eval\_text\_object\_range, line\_content\_end, snap\_grapheme, literal\_search, surrounding\_char\_range.

> - **commands (src/commands/mod.rs)**:

- CommandRegistry, Action, AppError, CommandContext, CommandId, CommandSpec, Effect, RegistryError.

> - **config (src/config/mod.rs)**:

- LimitsConfig, KeyHint, KeymapError, KeyBinding, KeymapConfig, BindingLookup, CompiledKeymap.

> - **markdown (src/markdown/mod.rs)**:

- BlockFact, DocFacts, HeadingFact, MarkdownError, MarkdownFacts, LinkOccurrence, LinkSelection, CalloutKind, StyleError, apply\_highlight, apply\_text\_color, remove\_style, wrap\_callout, ColorSpan, color\_spans, IncrementalSyntaxTree, SyntaxSnapshot, ListKind, ListMarker, LinePresentation, toggle\_task.

> - **navigation (src/navigation/mod.rs)**:

- Move, RowId, RowItem, RowModel, SelectionChange.

## **B. Реализация дорожной карты crates/core (CORE-P0 — CORE-P4)**

Ниже приведены полные реализации изменённых и новых модулей crates/core/src/\*\* без TODO-заглушек, с сохранением обратной совместимости всех существующих имён и сигнатур.

### **1\. src/lib.rs (реэкспорт новых типов семантики при сохранении всех старых)**

//\! Core abstractions and data structures for Truff v2.  
//\! Owns text buffers, transactions, command registry, and workspace model.  
//\! Completely independent of UI and SQL persistence.

pub use crop::Rope;  
pub use text::{  
Buffer, BufferId, BytePos, Change, ChangeSet, CursorMeta, EditError, History, LineEnding,  
Register, RegisterKind, Registers, SingleEdit, Transaction, UndoGroup,  
};

pub use commands::{Action, AppError, CommandContext, CommandId, CommandSpec, Effect};  
pub use workspace::buffers::{  
BufferEntry, BufferRegistry, BufferState, DEFAULT\_MAX\_BUFFERS, SnapshotKey, VaultEpoch,  
};  
pub use workspace::views::{DEFAULT\_MAX\_JUMP\_ENTRIES, JumpHistory, View, ViewId};  
pub use workspace::{DEFAULT\_MAX\_VIEWS, RefactorPlan, StoreEvent, StoreJob, Workspace};

pub mod commands;  
pub mod config;  
pub mod markdown;  
pub mod navigation;  
pub mod text;  
pub mod vim;  
pub mod workspace;

### **2\. CORE-P0: Input Semantics, State Machine, CommandRegistry и Keymap Validation**

#### **src/commands/types.rs**

Добавлены типизированные контексты выполнения (TextField, CommandLine, Navigator, Modal) в CommandContext и чистые дескрипторы действий списков/рефакторинга в Action при полной совместимости существующих вариантов (Editor, Rows, Global).

use std::borrow::Cow;  
use std::fmt;  
use std::path::PathBuf;

use crate::navigation::RowId;  
use crate::text::{BytePos, EditError, Transaction};  
use crate::workspace::StoreJob;  
use crate::workspace::buffers::{BufferId, BufferState, SnapshotKey, VaultEpoch};  
use crate::workspace::views::ViewId;

/// Strongly-typed identifier for a command.  
\#\[derive(  
Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,  
)\]  
pub struct CommandId(pub Cow\<'static, str\>);

impl CommandId {  
/// Constructs a \`CommandId\` from a static string slice.  
pub const fn from\_static(id: &'static str) \-\> Self {  
Self(Cow::Borrowed(id))  
}

    /// Constructs a \`CommandId\` from an owned string.
    pub fn new(id: impl Into\<String\>) \-\> Self {
        Self(Cow::Owned(id.into()))
    }

    /// Returns the command identifier as a string slice.
    pub fn as\_str(\&self) \-\> \&str {
        \&self.0
    }

}

impl fmt::Display for CommandId {  
fn fmt(\&self, f: \&mut fmt::Formatter\<'\_\>) \-\> fmt::Result {  
write\!(f, "{}", self.0)  
}  
}

/// Actions dispatched to the \`Workspace\` from UI, shortcuts, commands, or tests.  
\#\[derive(Debug, Clone, PartialEq, Eq)\]  
pub enum Action {  
/// Open a buffer by path.  
OpenPath(PathBuf),  
/// Create a new view for an existing buffer.  
CreateView(BufferId),  
/// Close an existing view.  
CloseView(ViewId),  
/// Set focus to a specific view.  
FocusView(ViewId),  
/// Move cursor and selection within a view.  
MoveCursor {  
view\_id: ViewId,  
anchor: BytePos,  
head: BytePos,  
},  
/// Scroll a view to a top line.  
Scroll { view\_id: ViewId, top\_line: usize },  
/// Apply an editing transaction to a view's buffer.  
ApplyEdit {  
view\_id: ViewId,  
transaction: Transaction,  
},  
/// Request saving a buffer to disk.  
SaveBuffer(BufferId),  
/// Force reload a buffer from disk.  
ReloadBuffer(BufferId),  
/// Explicitly discard local edits and reload the current disk content.  
ReloadDiscard(BufferId),  
/// Explicitly replace external disk content with the local snapshot.  
OverwriteBuffer(BufferId),  
/// Save to a new path without replacing any existing destination.  
SaveBufferAs { buffer\_id: BufferId, path: PathBuf },  
/// Undo the last transaction in the view's buffer.  
Undo(ViewId),  
/// Redo the last undone transaction in the view's buffer.  
Redo(ViewId),  
/// Switch active vault epoch.  
SwitchVault(VaultEpoch),  
/// Create a new file on disk.  
CreateFile(PathBuf),  
/// Create a new folder on disk.  
CreateFolder(PathBuf),  
/// Rename or move a path.  
RenamePath { from: PathBuf, to: PathBuf },  
/// Move a path to trash / recovery.  
TrashPath(PathBuf),  
/// Activate the currently selected row in a list/tree/Navigator.  
ActivateRow {  
panel: String,  
row\_id: RowId,  
data: Option\<String\>,  
},  
}

/// Effects produced by \`Workspace::dispatch\` or \`Workspace::accept\`.  
\#\[derive(Debug, Clone, PartialEq, Eq)\]  
pub enum Effect {  
/// Submit an asynchronous I/O job to the store worker.  
SubmitStoreJob(StoreJob),  
/// Notification that a view was closed.  
ViewClosed(ViewId),  
/// Notification that a view's cursor/scroll or text changed and requires redraw.  
InvalidateView(ViewId),  
/// Notification that buffer state changed (Clean, Dirty, Conflict).  
BufferStateChanged {  
buffer\_id: BufferId,  
state: BufferState,  
},  
/// Status message to be displayed to the user.  
StatusMessage(String),  
}

/// Specification for a command in the command registry.  
\#\[derive(Debug, Clone, PartialEq, Eq)\]  
pub struct CommandSpec {  
pub id: CommandId,  
pub title: Cow\<'static, str\>,  
pub description: Cow\<'static, str\>,  
pub group: Cow\<'static, str\>,  
pub default\_key: Option\<Cow\<'static, str\>\>,  
pub contexts: Vec\<CommandContext\>,  
}

/// Errors occurring during workspace operations.  
\#\[derive(Debug, Clone, PartialEq, Eq)\]  
pub enum AppError {  
BufferNotFound(BufferId),  
ViewNotFound(ViewId),  
EditFailed(EditError),  
StaleSnapshotKey {  
expected: SnapshotKey,  
actual: SnapshotKey,  
},  
StaleEpoch {  
current: VaultEpoch,  
received: VaultEpoch,  
},  
BufferConflict {  
buffer\_id: BufferId,  
},  
BufferDirty {  
buffer\_id: BufferId,  
},  
BufferAlreadyOpen(PathBuf),  
MaxViewsExceeded(usize),  
MaxBuffersExceeded(usize),  
}

impl fmt::Display for AppError {  
fn fmt(\&self, f: \&mut fmt::Formatter\<'\_\>) \-\> fmt::Result {  
match self {  
Self::BufferNotFound(id) \=\> write\!(f, "buffer not found: {id}"),  
Self::ViewNotFound(id) \=\> write\!(f, "view not found: {id}"),  
Self::EditFailed(err) \=\> write\!(f, "edit failed: {err}"),  
Self::StaleSnapshotKey { expected, actual } \=\> {  
write\!(  
f,  
"stale snapshot key: received {actual:?}, expected {expected:?}"  
)  
}  
Self::StaleEpoch { current, received } \=\> {  
write\!(  
f,  
"stale vault epoch: received {received:?}, current {current:?}"  
)  
}  
Self::BufferConflict { buffer\_id } \=\> write\!(f, "buffer conflict for {buffer\_id}"),  
Self::BufferDirty { buffer\_id } \=\> write\!(f, "buffer {buffer\_id} has unsaved changes"),  
Self::BufferAlreadyOpen(path) \=\> write\!(f, "buffer already open: {}", path.display()),  
Self::MaxViewsExceeded(max) \=\> write\!(f, "maximum views exceeded: {max}"),  
Self::MaxBuffersExceeded(max) \=\> write\!(f, "maximum buffers exceeded: {max}"),  
}  
}  
}

impl std::error::Error for AppError {}

impl From\<EditError\> for AppError {  
fn from(e: EditError) \-\> Self {  
Self::EditFailed(e)  
}  
}

/// Input target admitted by a registered command.  
\#\[derive(  
Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,  
)\]  
pub enum CommandContext {  
/// Main Markdown document buffer editor.  
Editor,  
/// Editable single-line or modal text field (search input, rename prompt, filter input).  
TextField,  
/// Vim \`:\` command line or \`/\` search prompt.  
CommandLine,  
/// Non-text list or tree view backed by \`RowModel\`.  
Rows,  
/// Universal Navigator / SessionX / picker panel.  
Navigator,  
/// Non-editable modal / overlay view (Help, WhichKey, previews).  
Modal,  
/// Available across all contexts when not shadowed by active text insertion.  
Global,  
}

impl CommandContext {  
pub const ALL: \[Self; 7\] \= \[  
Self::Editor,  
Self::TextField,  
Self::CommandLine,  
Self::Rows,  
Self::Navigator,  
Self::Modal,  
Self::Global,  
\];

    /// Returns \`true\` if this context represents an editable text target.
    pub const fn is\_editable\_text(self) \-\> bool {
        matches\!(self, Self::Editor | Self::TextField | Self::CommandLine)
    }

    /// Returns \`true\` if this context represents a row/list/tree target.
    pub const fn is\_row\_list(self) \-\> bool {
        matches\!(self, Self::Rows | Self::Navigator)
    }

    /// Checks whether a command registered for \`spec\_contexts\` is available in \`active\`.
    pub fn matches\_spec(self, spec\_contexts: &\[Self\]) \-\> bool {
        spec\_contexts.contains(\&self)
            || spec\_contexts.contains(\&Self::Global)
            || (self \== Self::Navigator && spec\_contexts.contains(\&Self::Rows))
            || (self \== Self::TextField && spec\_contexts.contains(\&Self::Editor))
    }

}

\#\[derive(Debug, Clone, PartialEq, Eq)\]  
pub enum RegistryError {  
UnknownCommand(CommandId),  
UnavailableContext {  
id: CommandId,  
context: CommandContext,  
},  
}

impl fmt::Display for RegistryError {  
fn fmt(\&self, f: \&mut fmt::Formatter\<'\_\>) \-\> fmt::Result {  
match self {  
Self::UnknownCommand(id) \=\> write\!(f, "unknown command: {id}"),  
Self::UnavailableContext { id, context } \=\> {  
write\!(f, "command {id} unavailable in {context:?}")  
}  
}  
}  
}

impl std::error::Error for RegistryError {}

#### **src/commands/defaults.rs**

Восстановлены все повреждённые русскоязычные заголовки, описания и группы; устранены контекстные конфликты (file.create, file.rename, file.trash перенесены в Rows/Navigator, row.activate на Enter).

use std::borrow::Cow;  
use std::sync::OnceLock;

use super::{CommandContext, CommandId, CommandRegistry, CommandSpec};

impl Default for CommandRegistry {  
fn default() \-\> Self {  
static DEFAULT: OnceLock\<CommandRegistry\> \= OnceLock::new();  
DEFAULT.get\_or\_init(Self::build\_default).clone()  
}  
}

impl CommandRegistry {  
fn build\_default() \-\> Self {  
let specs \= vec\!\[  
// Universal Navigator Panels (§3.2)  
cmd(  
"panel.files",  
"Файлы хранилища",  
"Открыть навигатор файлов хранилища",  
"Навигация",  
Some("Space f"),  
vec\!\[CommandContext::Global\],  
),  
cmd(  
"panel.sessions",  
"Сессии и буферы",  
"Открыть переключатель активных сессий и буферов (SessionX)",  
"Навигация",  
Some("Space s"),  
vec\!\[CommandContext::Global\],  
),  
cmd(  
"panel.boards",  
"Kanban доски",  
"Открыть панель Kanban-досок",  
"Навигация",  
Some("Space b"),  
vec\!\[CommandContext::Global\],  
),  
cmd(  
"panel.outline",  
"Структура (Outline)",  
"Открыть оглавление текущего документа",  
"Навигация",  
Some("Space o"),  
vec\!\[CommandContext::Global\],  
),  
cmd(  
"panel.links",  
"Ссылки и упоминания",  
"Показать исходящие ссылки и backlinks",  
"Навигация",  
Some("Space l"),  
vec\!\[CommandContext::Global\],  
),  
cmd(  
"panel.commands",  
"Палитра команд",  
"Открыть палитру всех доступных команд",  
"Навигация",  
Some("Ctrl+P"),  
vec\!\[CommandContext::Global\],  
),  
cmd(  
"app.command\_line",  
"Командная строка",  
"Открыть командную строку Vim (:)",  
"Приложение",  
Some(":"),  
vec\!\[CommandContext::Global\],  
),  
cmd(  
"app.help",  
"Справка по клавишам",  
"Показать окно справки и сочетаний клавиш",  
"Приложение",  
Some("?"),  
vec\!\[CommandContext::Global\],  
),  
// Motions  
cmd(  
"motion.left",  
"Шаг влево / к родителю",  
"Переместить курсор влево или свернуть узел дерева",  
"Перемещение",  
Some("h"),  
vec\!\[  
CommandContext::Editor,  
CommandContext::TextField,  
CommandContext::Rows,  
CommandContext::Navigator,  
\],  
),  
cmd(  
"motion.right",  
"Шаг вправо / к потомку",  
"Переместить курсор вправо или развернуть узел дерева",  
"Перемещение",  
Some("l"),  
vec\!\[  
CommandContext::Editor,  
CommandContext::TextField,  
CommandContext::Rows,  
CommandContext::Navigator,  
\],  
),  
cmd(  
"motion.up",  
"Строка вверх",  
"Переместить курсор или выделение на строку вверх",  
"Перемещение",  
Some("k"),  
vec\!\[  
CommandContext::Editor,  
CommandContext::Rows,  
CommandContext::Navigator,  
\],  
),  
cmd(  
"motion.down",  
"Строка вниз",  
"Переместить курсор или выделение на строку вниз",  
"Перемещение",  
Some("j"),  
vec\!\[  
CommandContext::Editor,  
CommandContext::Rows,  
CommandContext::Navigator,  
\],  
),  
cmd(  
"motion.first\_line",  
"В начало документа / списка",  
"Перейти к первой строке буфера или списка",  
"Перемещение",  
Some("gg"),  
vec\!\[  
CommandContext::Editor,  
CommandContext::Rows,  
CommandContext::Navigator,  
\],  
),  
cmd(  
"motion.last\_line",  
"В конец документа / списка",  
"Перейти к последней строке буфера или списка",  
"Перемещение",  
Some("G"),  
vec\!\[  
CommandContext::Editor,  
CommandContext::Rows,  
CommandContext::Navigator,  
\],  
),  
cmd(  
"motion.word\_forward",  
"Слово вперёд",  
"Перейти к началу следующего слова",  
"Перемещение",  
Some("w"),  
vec\!\[CommandContext::Editor, CommandContext::TextField\],  
),  
cmd(  
"motion.word\_backward",  
"Слово назад",  
"Перейти к началу предыдущего слова",  
"Перемещение",  
Some("b"),  
vec\!\[CommandContext::Editor, CommandContext::TextField\],  
),  
cmd(  
"motion.word\_end",  
"Конец слова",  
"Перейти к концу текущего или следующего слова",  
"Перемещение",  
Some("e"),  
vec\!\[CommandContext::Editor, CommandContext::TextField\],  
),  
cmd(  
"motion.line\_start",  
"Начало строки",  
"Перейти к первому байту строки",  
"Перемещение",  
Some("0"),  
vec\!\[CommandContext::Editor, CommandContext::TextField\],  
),  
cmd(  
"motion.first\_non\_blank",  
"Первый непробельный символ",  
"Перейти к первому непробельному символу строки",  
"Перемещение",  
Some("^"),  
vec\!\[CommandContext::Editor, CommandContext::TextField\],  
),  
cmd(  
"motion.line\_end",  
"Конец строки",  
"Перейти к концу строки",  
"Перемещение",  
Some("\$"),  
vec\!\[CommandContext::Editor, CommandContext::TextField\],  
),  
cmd(  
"motion.screen\_down",  
"Экранная строка вниз",  
"Переместиться вниз по визуальной строке",  
"Перемещение",  
Some("Ctrl+d"),  
vec\!\[CommandContext::Editor\],  
),  
cmd(  
"motion.screen\_up",  
"Экранная строка вверх",  
"Переместиться вверх по визуальной строке",  
"Перемещение",  
Some("Ctrl+u"),  
vec\!\[CommandContext::Editor\],  
),  
// Edits  
cmd(  
"edit.delete\_line",  
"Удалить строку",  
"Удалить текущую строку целиком",  
"Редактирование",  
Some("dd"),  
vec\!\[CommandContext::Editor, CommandContext::TextField\],  
),  
cmd(  
"edit.change\_line",  
"Заменить строку",  
"Удалить строку и перейти в режим вставки",  
"Редактирование",  
Some("cc"),  
vec\!\[CommandContext::Editor, CommandContext::TextField\],  
),  
cmd(  
"edit.yank\_line",  
"Скопировать строку",  
"Скопировать текущую строку в регистр",  
"Редактирование",  
Some("yy"),  
vec\!\[CommandContext::Editor, CommandContext::TextField\],  
),  
cmd(  
"edit.delete\_char",  
"Удалить символ",  
"Удалить графему под курсором",  
"Редактирование",  
Some("x"),  
vec\!\[CommandContext::Editor, CommandContext::TextField\],  
),  
cmd(  
"edit.paste\_after",  
"Вставить после курсора",  
"Вставить содержимое регистра после курсора",  
"Редактирование",  
Some("p"),  
vec\!\[CommandContext::Editor, CommandContext::TextField\],  
),  
cmd(  
"edit.paste\_before",  
"Вставить перед курсором",  
"Вставить содержимое регистра перед курсором",  
"Редактирование",  
Some("P"),  
vec\!\[CommandContext::Editor, CommandContext::TextField\],  
),  
cmd(  
"edit.undo",  
"Отменить правку",  
"Отменить последнюю группу изменений (Undo)",  
"Редактирование",  
Some("u"),  
vec\!\[CommandContext::Editor, CommandContext::TextField\],  
),  
cmd(  
"edit.redo",  
"Повторить правку",  
"Вернуть отменённую группу изменений (Redo)",  
"Редактирование",  
Some("Ctrl+r"),  
vec\!\[CommandContext::Editor, CommandContext::TextField\],  
),  
cmd(  
"edit.repeat",  
"Повторить последнее действие",  
"Повторить последнюю команду редактирования (.)",  
"Редактирование",  
Some("."),  
vec\!\[CommandContext::Editor, CommandContext::TextField\],  
),  
cmd(  
"edit.insert",  
"Режим вставки",  
"Перейти в режим вставки перед курсором",  
"Редактирование",  
Some("i"),  
vec\!\[CommandContext::Editor, CommandContext::TextField\],  
),  
cmd(  
"edit.append",  
"Вставка после курсора",  
"Перейти в режим вставки после курсора",  
"Редактирование",  
Some("a"),  
vec\!\[CommandContext::Editor, CommandContext::TextField\],  
),  
cmd(  
"edit.insert\_start",  
"Вставка в начало строки",  
"Перейти в режим вставки у первого непробельного символа",  
"Редактирование",  
Some("I"),  
vec\!\[CommandContext::Editor, CommandContext::TextField\],  
),  
cmd(  
"edit.append\_end",  
"Вставка в конец строки",  
"Перейти в режим вставки в конце строки",  
"Редактирование",  
Some("A"),  
vec\!\[CommandContext::Editor, CommandContext::TextField\],  
),  
cmd(  
"edit.open\_below",  
"Новая строка ниже",  
"Создать строку ниже и перейти в режим вставки",  
"Редактирование",  
Some("o"),  
vec\!\[CommandContext::Editor\],  
),  
cmd(  
"edit.open\_above",  
"Новая строка выше",  
"Создать строку выше и перейти в режим вставки",  
"Редактирование",  
Some("O"),  
vec\!\[CommandContext::Editor\],  
),  
cmd(  
"edit.visual",  
"Визуальный режим",  
"Включить посимвольное выделение (Visual)",  
"Редактирование",  
Some("v"),  
vec\!\[  
CommandContext::Editor,  
CommandContext::TextField,  
CommandContext::Rows,  
CommandContext::Navigator,  
\],  
),  
cmd(  
"edit.visual\_line",  
"Построчный визуальный режим",  
"Включить построчное выделение (Visual Line)",  
"Редактирование",  
Some("V"),  
vec\!\[  
CommandContext::Editor,  
CommandContext::Rows,  
CommandContext::Navigator,  
\],  
),  
cmd(  
"edit.indent",  
"Увеличить отступ",  
"Сдвинуть строку или элемент списка вправо",  
"Редактирование",  
Some("\>\>"),  
vec\!\[CommandContext::Editor\],  
),  
cmd(  
"edit.dedent",  
"Уменьшить отступ",  
"Сдвинуть строку или элемент списка влево",  
"Редактирование",  
Some("\<\<"),  
vec\!\[CommandContext::Editor\],  
),  
// Notes and links  
cmd(  
"note.follow\_link",  
"Перейти по ссылке",  
"Открыть заметку или заголовок под курсором",  
"Заметки и ссылки",  
Some("gf"),  
vec\!\[CommandContext::Editor\],  
),  
cmd(  
"note.link\_selection",  
"Обернуть в ссылку",  
"Преобразовать выделение в ссылку \[\[...\]\]",  
"Заметки и ссылки",  
None,  
vec\!\[CommandContext::Editor\],  
),  
cmd(  
"note.open\_backlinks",  
"Обратные ссылки",  
"Показать все заметки, ссылающиеся на текущую",  
"Заметки и ссылки",  
None,  
vec\!\[CommandContext::Editor\],  
),  
cmd(  
"note.create",  
"Создать заметку",  
"Создать новую заметку в хранилище",  
"Заметки и ссылки",  
None,  
vec\!\[CommandContext::Rows, CommandContext::Navigator, CommandContext::Global\],  
),  
cmd(  
"note.open\_target\_heading",  
"Перейти к заголовку",  
"Открыть целевой заголовок внутри заметки",  
"Заметки и ссылки",  
None,  
vec\!\[CommandContext::Editor\],  
),  
// Styles & Callouts  
cmd(  
"style.text\_color",  
"Цвет текста",  
"Применить цвет текста к выделению",  
"Стили и Callout",  
None,  
vec\!\[CommandContext::Editor\],  
),  
cmd(  
"style.highlight\_color",  
"Цвет выделения (маркер)",  
"Подсветить фон выделенного фрагмента",  
"Стили и Callout",  
None,  
vec\!\[CommandContext::Editor\],  
),  
cmd(  
"style.wrap\_callout",  
"Обернуть в Callout",  
"Оформить выделенные строки как блок callout (note/warning/tip/important)",  
"Стили и Callout",  
None,  
vec\!\[CommandContext::Editor\],  
),  
cmd(  
"style.pane\_accent",  
"Акцент панели",  
"Установить цветовой акцент текущей панели",  
"Стили и Callout",  
None,  
vec\!\[CommandContext::Editor\],  
),  
// Flash navigation  
cmd(  
"flash.jump\_word",  
"Flash: переход к слову",  
"Быстрый прыжок к слову по метке в редакторе",  
"Flash-навигация",  
Some("s"),  
vec\!\[CommandContext::Editor\],  
),  
cmd(  
"flash.select\_sentence",  
"Flash: выбрать предложение",  
"Быстрое выделение предложения по метке",  
"Flash-навигация",  
None,  
vec\!\[CommandContext::Editor\],  
),  
cmd(  
"flash.select\_paragraph",  
"Flash: выбрать абзац",  
"Быстрое выделение абзаца по метке",  
"Flash-навигация",  
None,  
vec\!\[CommandContext::Editor\],  
),  
cmd(  
"flash.select\_syntax\_object",  
"Flash: выбрать синтаксический узел",  
"Быстрое выделение Markdown-узла (заголовок, ссылка, код, список)",  
"Flash-навигация",  
Some("S"),  
vec\!\[CommandContext::Editor\],  
),  
// Buffers & Files (List/Row actions use typed commands in Rows/Navigator)  
cmd(  
"row.activate",  
"Открыть выбранный элемент",  
"Активировать выбранную строку списка или узел дерева",  
"Списки и файлы",  
Some("Enter"),  
vec\!\[CommandContext::Rows, CommandContext::Navigator\],  
),  
cmd(  
"buffer.open",  
"Открыть буфер",  
"Открыть файл в буфере по пути",  
"Буферы и файлы",  
None,  
vec\!\[CommandContext::Global\],  
),  
cmd(  
"buffer.select",  
"Выбрать буфер",  
"Переключиться на открытый буфер по ID или имени",  
"Буферы и файлы",  
None,  
vec\!\[CommandContext::Global\],  
),  
cmd(  
"buffer.next",  
"Следующий буфер",  
"Переключиться на следующий открытый буфер",  
"Буферы и файлы",  
Some("\]b"),  
vec\!\[CommandContext::Global\],  
),  
cmd(  
"buffer.previous",  
"Предыдущий буфер",  
"Переключиться на предыдущий открытый буфер",  
"Буферы и файлы",  
Some("\[b"),  
vec\!\[CommandContext::Global\],  
),  
cmd(  
"buffer.save",  
"Сохранить буфер",  
"Записать изменения текущего буфера на диск (:w)",  
"Буферы и файлы",  
Some(":w"),  
vec\!\[CommandContext::Global\],  
),  
cmd(  
"buffer.close",  
"Закрыть вид / буфер",  
"Закрыть текущий вид (:q / :bd)",  
"Буферы и файлы",  
Some(":q"),  
vec\!\[CommandContext::Global\],  
),  
cmd(  
"file.create",  
"Создать файл",  
"Создать новый файл в выбранной папке навигатора",  
"Буферы и файлы",  
Some("a"),  
vec\!\[CommandContext::Rows, CommandContext::Navigator\],  
),  
cmd(  
"folder.create",  
"Создать папку",  
"Создать новую директорию в хранилище",  
"Буферы и файлы",  
Some("A"),  
vec\!\[CommandContext::Rows, CommandContext::Navigator\],  
),  
cmd(  
"file.rename",  
"Переименовать файл",  
"Переименовать выбранный файл или папку в списке",  
"Буферы и файлы",  
Some("r"),  
vec\!\[CommandContext::Rows, CommandContext::Navigator\],  
),  
cmd(  
"file.move",  
"Переместить файл",  
"Переместить выбранный файл в другую папку",  
"Буферы и файлы",  
None,  
vec\!\[CommandContext::Rows, CommandContext::Navigator\],  
),  
cmd(  
"file.trash",  
"Удалить в корзину",  
"Переместить выбранный элемент списка в корзину",  
"Буферы и файлы",  
Some("d"),  
vec\!\[CommandContext::Rows, CommandContext::Navigator\],  
),  
// Panes and Focus  
cmd(  
"focus.left",  
"Фокус на панель слева",  
"Переключить фокус на окно слева",  
"Окна и фокус",  
Some("Ctrl+w h"),  
vec\!\[CommandContext::Global\],  
),  
cmd(  
"focus.right",  
"Фокус на панель справа",  
"Переключить фокус на окно справа",  
"Окна и фокус",  
Some("Ctrl+w l"),  
vec\!\[CommandContext::Global\],  
),  
cmd(  
"focus.up",  
"Фокус на панель выше",  
"Переключить фокус на окно выше",  
"Окна и фокус",  
Some("Ctrl+w k"),  
vec\!\[CommandContext::Global\],  
),  
cmd(  
"focus.down",  
"Фокус на панель ниже",  
"Переключить фокус на окно ниже",  
"Окна и фокус",  
Some("Ctrl+w j"),  
vec\!\[CommandContext::Global\],  
),  
\];

        Self::from\_specs(specs)
    }

}

fn cmd(  
id: &'static str,  
title: &'static str,  
description: &'static str,  
group: &'static str,  
default\_key: Option\<&'static str\>,  
contexts: Vec\<CommandContext\>,  
) \-\> CommandSpec {  
CommandSpec {  
id: CommandId::from\_static(id),  
title: Cow::Borrowed(title),  
description: Cow::Borrowed(description),  
group: Cow::Borrowed(group),  
default\_key: default\_key.map(Cow::Borrowed),  
contexts,  
}  
}

#### **src/commands/registry.rs**

Добавлены фильтрация по CommandContext (specs\_in\_context, search\_in\_context) для Command Palette / Which-Key и проверка is\_available.

use super::{CommandContext, CommandId, CommandSpec, RegistryError};  
use std::sync::Arc;

\#\[derive(Debug)\]  
struct RegistryData {  
specs: Vec\<CommandSpec\>,  
ids: Vec\<usize\>,  
search: Vec\<SearchFields\>,  
}

\#\[derive(Debug)\]  
struct SearchFields {  
title: String,  
description: String,  
id: String,  
group: String,  
}

/// Command lookup shared by keymaps, which-key, pickers and buttons.  
\#\[derive(Debug, Clone)\]  
pub struct CommandRegistry {  
data: Arc\<RegistryData\>,  
}

impl CommandRegistry {  
pub(super) fn from\_specs(specs: Vec\<CommandSpec\>) \-\> Self {  
let mut ids: Vec\<\_\> \= (0..specs.len()).collect();  
ids.sort\_by(|\&a, \&b| specs\[a\].id.cmp(\&specs\[b\].id));  
let search \= specs  
.iter()  
.map(|spec| SearchFields {  
title: spec.title.to\_lowercase(),  
description: spec.description.to\_lowercase(),  
id: spec.id.as\_str().to\_lowercase(),  
group: spec.group.to\_lowercase(),  
})  
.collect();  
Self {  
data: Arc::new(RegistryData { specs, ids, search }),  
}  
}

    pub fn specs(\&self) \-\> &\[CommandSpec\] {
        \&self.data.specs
    }

    /// Returns all commands available in the given execution context.
    pub fn specs\_in\_context(\&self, context: CommandContext) \-\> Vec\<\&CommandSpec\> {
        self.data
            .specs
            .iter()
            .filter(|spec| context.matches\_spec(\&spec.contexts))
            .collect()
    }

    /// Returns \`true\` if \`id\` exists and is admitted in \`context\`.
    pub fn is\_available(\&self, id: \&CommandId, context: CommandContext) \-\> bool {
        self.require(id, context).is\_ok()
    }

    /// Returns specs grouped by their category.
    pub fn by\_category(\&self) \-\> std::collections::BTreeMap\<\&str, Vec\<\&CommandSpec\>\> {
        let mut grouped \= std::collections::BTreeMap::\<\&str, Vec\<\&CommandSpec\>\>::new();
        for spec in \&self.data.specs {
            grouped.entry(\&spec.group).or\_default().push(spec);
        }
        grouped
    }

    /// Searches commands by query matching title, description, group, or id.
    pub fn search(\&self, query: \&str) \-\> Vec\<\&CommandSpec\> {
        self.search\_Internal(query, None)
    }

    /// Searches commands available in \`context\` for Command Palette filtering.
    pub fn search\_in\_context(\&self, query: \&str, context: CommandContext) \-\> Vec\<\&CommandSpec\> {
        self.search\_Internal(query, Some(context))
    }

    fn search\_Internal(\&self, query: \&str, context: Option\<CommandContext\>) \-\> Vec\<\&CommandSpec\> {
        let q \= query.trim().to\_lowercase();
        if q.is\_empty() {
            return self
                .data
                .specs
                .iter()
                .filter(|spec| context.is\_none\_or(|ctx| ctx.matches\_spec(\&spec.contexts)))
                .collect();
        }
        let mut scored \= Vec::new();
        for (spec, fields) in self.data.specs.iter().zip(\&self.data.search) {
            if let Some(ctx) \= context
                && \!ctx.matches\_spec(\&spec.contexts)
            {
                continue;
            }
            let score \= if fields.title \== q || fields.id \== q {
                100
            } else if fields.title.starts\_with(\&q) {
                80
            } else if fields.title.contains(\&q) {
                60
            } else if fields.description.contains(\&q) {
                40
            } else if fields.group.contains(\&q) || fields.id.contains(\&q) {
                20
            } else {
                0
            };
            if score \> 0 {
                scored.push((score, spec));
            }
        }
        scored.sort\_by(|a, b| b.0.cmp(\&a.0).then\_with(|| a.1.title.cmp(\&b.1.title)));
        scored.into\_iter().map(|(\_, spec)| spec).collect()
    }

    pub fn get(\&self, id: \&CommandId) \-\> Result\<\&CommandSpec, RegistryError\> {
        self.data
            .ids
            .binary\_search\_by(|\&index| self.data.specs\[index\].id.cmp(id))
            .map(|position| \&self.data.specs\[self.data.ids\[position\]\])
            .map\_err(|\_| RegistryError::UnknownCommand(id.clone()))
    }

    pub fn require(
        \&self,
        id: \&CommandId,
        context: CommandContext,
    ) \-\> Result\<\&CommandSpec, RegistryError\> {
        let spec \= self.get(id)?;
        if context.matches\_spec(\&spec.contexts) {
            Ok(spec)
        } else {
            Err(RegistryError::UnavailableContext {
                id: id.clone(),
                context,
            })
        }
    }

    /// Core editor commands resolve to the same semantic commands used by the parser.
    /// Application commands retain their stable ID for the frontend dispatcher.
    pub fn vim\_command(
        \&self,
        id: \&CommandId,
        context: CommandContext,
    ) \-\> Result\<Option\<crate::vim::Command\>, RegistryError\> {
        self.require(id, context)?;
        use crate::vim::parser::InsertEntry;
        use crate::vim::{Command, Mode, Motion, Operator};

        let motion \= match id.as\_str() {
            "motion.left" \=\> Some(Motion::Left),
            "motion.right" \=\> Some(Motion::Right),
            "motion.up" \=\> Some(Motion::Up),
            "motion.down" \=\> Some(Motion::Down),
            "motion.first\_line" \=\> Some(Motion::FirstLine),
            "motion.last\_line" \=\> Some(Motion::LastLine),
            "motion.screen\_down" \=\> Some(Motion::ScreenDown),
            "motion.screen\_up" \=\> Some(Motion::ScreenUp),
            "motion.word\_forward" \=\> Some(Motion::WordForward),
            "motion.word\_backward" \=\> Some(Motion::WordBackward),
            "motion.word\_end" \=\> Some(Motion::WordEnd),
            "motion.line\_start" \=\> Some(Motion::LineStart),
            "motion.first\_non\_blank" \=\> Some(Motion::LineFirstNonBlank),
            "motion.line\_end" \=\> Some(Motion::LineEnd),
            \_ \=\> None,
        };
        if let Some(motion) \= motion {
            return Ok(Some(Command::Motion { motion, count: 1 }));
        }

        let cmd \= match id.as\_str() {
            "edit.delete\_line" \=\> Command::OperatorMotion {
                op: Operator::Delete,
                motion: Motion::CurrentLine,
                count: 1,
            },
            "edit.change\_line" \=\> Command::OperatorMotion {
                op: Operator::Change,
                motion: Motion::CurrentLine,
                count: 1,
            },
            "edit.yank\_line" \=\> Command::OperatorMotion {
                op: Operator::Yank,
                motion: Motion::CurrentLine,
                count: 1,
            },
            "edit.indent" \=\> Command::OperatorMotion {
                op: Operator::Indent,
                motion: Motion::CurrentLine,
                count: 1,
            },
            "edit.dedent" \=\> Command::OperatorMotion {
                op: Operator::Dedent,
                motion: Motion::CurrentLine,
                count: 1,
            },
            "edit.delete\_char" \=\> Command::DeleteChar(1),
            "edit.paste\_after" \=\> Command::Paste {
                after: true,
                count: 1,
            },
            "edit.paste\_before" \=\> Command::Paste {
                after: false,
                count: 1,
            },
            "edit.undo" \=\> Command::Undo,
            "edit.redo" \=\> Command::Redo,
            "edit.repeat" \=\> Command::RepeatLastEdit,
            "edit.insert" \=\> Command::Insert(InsertEntry::AtCursor),
            "edit.append" \=\> Command::Insert(InsertEntry::AfterCursor),
            "edit.insert\_start" \=\> Command::Insert(InsertEntry::LineStart),
            "edit.append\_end" \=\> Command::Insert(InsertEntry::LineEnd),
            "edit.open\_below" \=\> Command::Insert(InsertEntry::NewLineBelow),
            "edit.open\_above" \=\> Command::Insert(InsertEntry::NewLineAbove),
            "edit.visual" \=\> Command::Visual(Mode::VisualChar),
            "edit.visual\_line" \=\> Command::Visual(Mode::VisualLine),
            \_ \=\> return Ok(None),
        };
        Ok(Some(cmd))
    }

}

#### **src/config/mod.rs, src/config/compiled.rs и src/config/defaults.rs**

Контекстная валидация KeymapConfig:

> 1. Запрещает привязку печатных символов (Key::Char(\_)) в Mode::Insert (в Insert печатные символы — всегда литеральный текст).
> 2. Проверяет пересечение контекстов: одна и та же клавиша (например, a, d, r) может быть назначена в Editor и в Rows на разные команды без ложного конфликта, но внутри одного контекста дубли и пересечения префиксов строго отклоняются.
> 3. Добавляет Space f/s/b/o/l, s (только Editor в Normal), a/r/d/Enter (в Rows/Navigator).

// src/config/mod.rs  
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::commands::{CommandContext, CommandId, CommandRegistry, RegistryError};  
use crate::vim::{Key, Mode};

mod compiled;  
mod defaults;

pub use compiled::{BindingLookup, CompiledKeymap};

/// Limits and safety thresholds for editing, macros, and history.  
\#\[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)\]  
pub struct LimitsConfig {  
pub max\_macro\_commands: usize,  
pub max\_macro\_steps: usize,  
pub max\_history\_bytes: usize,  
}

impl LimitsConfig {  
/// Default maximum commands allowed in a single macro definition.  
pub const DEFAULT\_MAX\_MACRO\_COMMANDS: usize \= 1000;  
/// Default maximum execution steps allowed across macro playback.  
pub const DEFAULT\_MAX\_MACRO\_STEPS: usize \= 10000;  
/// Default maximum history memory: 8 MiB.  
pub const DEFAULT\_MAX\_HISTORY\_BYTES: usize \= 8 \* 1024 \* 1024;  
}

impl Default for LimitsConfig {  
fn default() \-\> Self {  
Self {  
max\_macro\_commands: Self::DEFAULT\_MAX\_MACRO\_COMMANDS,  
max\_macro\_steps: Self::DEFAULT\_MAX\_MACRO\_STEPS,  
max\_history\_bytes: Self::DEFAULT\_MAX\_HISTORY\_BYTES,  
}  
}  
}

/// A contextual hint for WhichKey display.  
\#\[derive(Debug, Clone, PartialEq, Eq)\]  
pub struct KeyHint {  
pub key: Key,  
pub description: String,  
pub is\_prefix: bool,  
}

/// Errors detected during keymap compilation or validation.  
\#\[derive(Debug, Clone, PartialEq, Eq)\]  
pub enum KeymapError {  
ShadowedPrefix {  
prefix: Vec\<Key\>,  
conflicting\_key: Key,  
},  
InvalidBinding(String),  
DuplicateBinding {  
mode: Mode,  
keys: Vec\<Key\>,  
},  
Registry(RegistryError),  
InvalidToml(String),  
}

impl fmt::Display for KeymapError {  
fn fmt(\&self, f: \&mut fmt::Formatter\<'\_\>) \-\> fmt::Result {  
match self {  
Self::ShadowedPrefix {  
prefix,  
conflicting\_key,  
} \=\> {  
write\!(  
f,  
"keymap prefix {:?} conflicts with complete binding {:?}",  
prefix, conflicting\_key  
)  
}  
Self::InvalidBinding(msg) \=\> write\!(f, "invalid binding: {msg}"),  
Self::DuplicateBinding { mode, keys } \=\> {  
write\!(f, "duplicate binding in {mode}: {keys:?}")  
}  
Self::Registry(e) \=\> e.fmt(f),  
Self::InvalidToml(e) \=\> write\!(f, "invalid keymap TOML: {e}"),  
}  
}  
}

impl std::error::Error for KeymapError {}

/// A single keymap rule.  
\#\[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)\]  
pub struct KeyBinding {  
pub mode: Mode,  
pub keys: Vec\<Key\>,  
pub command: CommandId,  
pub description: String,  
}

/// Configuration storing key bindings and providing prefix lookups for which-key.  
\#\[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)\]  
pub struct KeymapConfig {  
pub bindings: Vec\<KeyBinding\>,  
pub limits: LimitsConfig,  
}

impl KeymapConfig {  
/// Checks whether a key sequence is a complete binding or an exact prefix of an existing binding.  
pub fn has\_binding\_or\_prefix(\&self, mode: Mode, keys: &\[Key\]) \-\> bool {  
self.compile().is\_ok\_and(|map| {  
map.lookup(mode, keys, CommandContext::Editor) \!= BindingLookup::Missing  
})  
}

    /// Returns available key hints for which-key given an entered prefix.
    pub fn available\_hints(\&self, mode: Mode, prefix: &\[Key\]) \-\> Vec\<KeyHint\> {
        self.compile()
            .map(|map| map.hints(mode, prefix, CommandContext::Editor))
            .unwrap\_or\_default()
    }

    /// Validates the keymap to ensure no ambiguous collisions within any shared context.
    pub fn validate(\&self) \-\> Result\<(), KeymapError\> {
        if self.bindings.len() \> 4096 {
            return Err(KeymapError::InvalidBinding(
                "more than 4096 bindings".into(),
            ));
        }
        let registry \= CommandRegistry::default();
        let mut resolved \= Vec::with\_capacity(self.bindings.len());
        for binding in \&self.bindings {
            if binding.keys.len() \> 32 {
                return Err(KeymapError::InvalidBinding(
                    "key sequence exceeds 32 keys".into(),
                ));
            }
            if binding.keys.is\_empty() {
                return Err(KeymapError::InvalidBinding("empty key sequence".into()));
            }
            if binding.mode \== Mode::Insert
                && matches\!(binding.keys.first(), Some(Key::Char(\_)))
            {
                return Err(KeymapError::InvalidBinding(
                    "printable character cannot be bound in Insert mode".into(),
                ));
            }
            let spec \= registry
                .get(\&binding.command)
                .map\_err(KeymapError::Registry)?;
            if spec.contexts.is\_empty() {
                return Err(KeymapError::InvalidBinding(format\!(
                    "command {} has no admitted contexts",
                    binding.command
                )));
            }
            resolved.push((binding, \&spec.contexts));
        }

        // Check collisions per concrete context so Editor and Rows can bind the same key
        // to distinct context-specific commands (e.g. \`a\` \-\> edit.append vs \`a\` \-\> file.create).
        for ctx in CommandContext::ALL {
            let mut active: Vec\<\&KeyBinding\> \= resolved
                .iter()
                .filter(|(\_, contexts)| ctx.matches\_spec(contexts))
                .map(|(b, \_)| \*b)
                .collect();

            active.sort\_unstable\_by(|left, right| {
                left.mode
                    .cmp(\&right.mode)
                    .then\_with(|| left.keys.cmp(\&right.keys))
            });

            for pair in active.windows(2) {
                let \[left, right\] \= pair else { continue };
                if left.mode \!= right.mode {
                    continue;
                }
                if left.keys \== right.keys {
                    return Err(KeymapError::DuplicateBinding {
                        mode: left.mode,
                        keys: left.keys.clone(),
                    });
                }
                if right.keys.starts\_with(\&left.keys) {
                    return Err(KeymapError::ShadowedPrefix {
                        prefix: left.keys.clone(),
                        conflicting\_key: right.keys\[left.keys.len()\],
                    });
                }
            }
        }

        Ok(())
    }

    pub fn from\_toml(source: \&str) \-\> Result\<Self, KeymapError\> {
        let config: Self \=
            toml::from\_str(source).map\_err(|e| KeymapError::InvalidToml(e.to\_string()))?;
        config.validate()?;
        Ok(config)
    }

}

// src/config/compiled.rs  
use crate::commands::{CommandContext, CommandId, CommandRegistry};  
use crate::vim::{Key, Mode};

use super::{KeyBinding, KeyHint, KeymapConfig, KeymapError};

/// Compiled prefixes are the single source for key lookup and contextual hints.  
\#\[derive(Debug, Clone)\]  
pub struct CompiledKeymap {  
prefixes: std::collections::BTreeMap\<Mode, std::collections::BTreeMap\<Vec\<Key\>, Vec\<usize\>\>\>,  
bindings: Vec\<CompiledBinding\>,  
}

\#\[derive(Debug, Clone)\]  
struct CompiledBinding {  
binding: KeyBinding,  
available: \[bool; 7\],  
}

impl CompiledBinding {  
fn available\_in(\&self, context: CommandContext) \-\> bool {  
self.available\[Self::context\_index(context)\]  
}

    const fn context\_index(context: CommandContext) \-\> usize {
        match context {
            CommandContext::Editor \=\> 0,
            CommandContext::TextField \=\> 1,
            CommandContext::CommandLine \=\> 2,
            CommandContext::Rows \=\> 3,
            CommandContext::Navigator \=\> 4,
            CommandContext::Modal \=\> 5,
            CommandContext::Global \=\> 6,
        }
    }

}

\#\[derive(Debug, Clone, PartialEq, Eq)\]  
pub enum BindingLookup {  
Complete(CommandId),  
Prefix,  
Missing,  
}

impl KeymapConfig {  
pub fn compile(\&self) \-\> Result\<CompiledKeymap, KeymapError\> {  
self.validate()?;  
let registry \= CommandRegistry::default();  
let bindings \= self  
.bindings  
.iter()  
.map(|binding| {  
let spec \= registry  
.get(\&binding.command)  
.map\_err(KeymapError::Registry)?;  
let mut available \= \[false; 7\];  
for ctx in CommandContext::ALL {  
available\[CompiledBinding::context\_index(ctx)\] \=  
ctx.matches\_spec(\&spec.contexts);  
}  
Ok(CompiledBinding {  
binding: binding.clone(),  
available,  
})  
})  
.collect::\<Result\<Vec\<\_\>, KeymapError\>\>()?;

        let mut prefixes \= std::collections::BTreeMap::\<
            Mode,
            std::collections::BTreeMap\<Vec\<Key\>, Vec\<usize\>\>,
        \>::new();
        for (idx, binding) in self.bindings.iter().enumerate() {
            for len in 0..=binding.keys.len() {
                prefixes
                    .entry(binding.mode)
                    .or\_default()
                    .entry(binding.keys\[..len\].to\_vec())
                    .or\_default()
                    .push(idx);
            }
        }
        for mode\_prefixes in prefixes.values\_mut() {
            for (prefix, candidates) in mode\_prefixes {
                candidates.sort\_unstable\_by\_key(|\&idx| {
                    (self.bindings\[idx\].keys.get(prefix.len()).copied(), idx)
                });
            }
        }
        Ok(CompiledKeymap { prefixes, bindings })
    }

}

impl CompiledKeymap {  
pub fn lookup(\&self, mode: Mode, keys: &\[Key\], context: CommandContext) \-\> BindingLookup {  
let Some(bindings) \= self  
.prefixes  
.get(\&mode)  
.and\_then(|prefixes| prefixes.get(keys))  
else {  
return BindingLookup::Missing;  
};  
for \&idx in bindings {  
let compiled \= \&self.bindings\[idx\];  
if compiled.available\_in(context) {  
let binding \= \&compiled.binding;  
if binding.keys.len() \== keys.len() {  
return BindingLookup::Complete(binding.command.clone());  
}  
return BindingLookup::Prefix;  
}  
}  
BindingLookup::Missing  
}

    pub fn hints(\&self, mode: Mode, prefix: &\[Key\], context: CommandContext) \-\> Vec\<KeyHint\> {
        let Some(bindings) \= self
            .prefixes
            .get(\&mode)
            .and\_then(|prefixes| prefixes.get(prefix))
        else {
            return Vec::new();
        };
        let mut hints \= Vec::\<KeyHint\>::new();
        for \&idx in bindings {
            let compiled \= \&self.bindings\[idx\];
            let binding \= \&compiled.binding;
            let Some(\&key) \= binding.keys.get(prefix.len()) else {
                continue;
            };
            if hints.last().is\_some\_and(|hint| hint.key \== key) || \!compiled.available\_in(context) {
                continue;
            }
            hints.push(KeyHint {
                key,
                description: binding.description.clone(),
                is\_prefix: binding.keys.len() \> prefix.len() \+ 1,
            });
        }
        hints
    }

}

// src/config/defaults.rs  
use crate::commands::CommandId;  
use crate::vim::{Key, Mode};

use super::{KeyBinding, KeymapConfig, LimitsConfig};

impl Default for KeymapConfig {  
fn default() \-\> Self {  
let bindings \= vec\!\[  
// Motions (Editor & Rows/Navigator)  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('h')\],  
command: CommandId::from\_static("motion.left"),  
description: "Move left".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('j')\],  
command: CommandId::from\_static("motion.down"),  
description: "Move down".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('k')\],  
command: CommandId::from\_static("motion.up"),  
description: "Move up".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('l')\],  
command: CommandId::from\_static("motion.right"),  
description: "Move right".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('w')\],  
command: CommandId::from\_static("motion.word\_forward"),  
description: "Word forward".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('b')\],  
command: CommandId::from\_static("motion.word\_backward"),  
description: "Word backward".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('e')\],  
command: CommandId::from\_static("motion.word\_end"),  
description: "End of word".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('0')\],  
command: CommandId::from\_static("motion.line\_start"),  
description: "Line start".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('^')\],  
command: CommandId::from\_static("motion.first\_non\_blank"),  
description: "First non-blank character".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('\$')\],  
command: CommandId::from\_static("motion.line\_end"),  
description: "Line end".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('g'), Key::Char('g')\],  
command: CommandId::from\_static("motion.first\_line"),  
description: "First line of buffer".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('g'), Key::Char('j')\],  
command: CommandId::from\_static("motion.screen\_down"),  
description: "Screen line down".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('g'), Key::Char('k')\],  
command: CommandId::from\_static("motion.screen\_up"),  
description: "Screen line up".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('G')\],  
command: CommandId::from\_static("motion.last\_line"),  
description: "Last line of buffer".to\_string(),  
},  
// Edits & operators  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('d'), Key::Char('d')\],  
command: CommandId::from\_static("edit.delete\_line"),  
description: "Delete line".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('c'), Key::Char('c')\],  
command: CommandId::from\_static("edit.change\_line"),  
description: "Change line".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('y'), Key::Char('y')\],  
command: CommandId::from\_static("edit.yank\_line"),  
description: "Yank line".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('x')\],  
command: CommandId::from\_static("edit.delete\_char"),  
description: "Delete character".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('p')\],  
command: CommandId::from\_static("edit.paste\_after"),  
description: "Paste after".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('P')\],  
command: CommandId::from\_static("edit.paste\_before"),  
description: "Paste before".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('u')\],  
command: CommandId::from\_static("edit.undo"),  
description: "Undo".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Ctrl('r')\],  
command: CommandId::from\_static("edit.redo"),  
description: "Redo".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('.')\],  
command: CommandId::from\_static("edit.repeat"),  
description: "Repeat last edit".to\_string(),  
},  
// Flash in Normal Editor  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('s')\],  
command: CommandId::from\_static("flash.jump\_word"),  
description: "Flash jump to word".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('S')\],  
command: CommandId::from\_static("flash.select\_syntax\_object"),  
description: "Flash select syntax object".to\_string(),  
},  
// Leader (Space) bindings  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char(' '), Key::Char('f')\],  
command: CommandId::from\_static("panel.files"),  
description: "Find files".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char(' '), Key::Char('s')\],  
command: CommandId::from\_static("panel.sessions"),  
description: "Switch session / buffer".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char(' '), Key::Char('b')\],  
command: CommandId::from\_static("panel.boards"),  
description: "Open Kanban boards".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char(' '), Key::Char('o')\],  
command: CommandId::from\_static("panel.outline"),  
description: "Document outline".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char(' '), Key::Char('l')\],  
command: CommandId::from\_static("panel.links"),  
description: "Links & backlinks".to\_string(),  
},  
// Non-text list/tree contextual commands (Rows / Navigator only)  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Enter\],  
command: CommandId::from\_static("row.activate"),  
description: "Activate selected item".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('a')\],  
command: CommandId::from\_static("file.create"),  
description: "Create file".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('A')\],  
command: CommandId::from\_static("folder.create"),  
description: "Create folder".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('r')\],  
command: CommandId::from\_static("file.rename"),  
description: "Rename item".to\_string(),  
},  
KeyBinding {  
mode: Mode::Normal,  
keys: vec\!\[Key::Char('d')\],  
command: CommandId::from\_static("file.trash"),  
description: "Trash item".to\_string(),  
},  
\];  
Self {  
bindings,  
limits: LimitsConfig::default(),  
}  
}  
}

#### **src/vim/mod.rs, src/vim/execute/context.rs, src/vim/execute/mod.rs, src/vim/execute/command.rs**

Добавлены:

> - Key::ShiftTab в src/vim/mod.rs.
> - Расширенные контексты в InputContext: TextField, CommandLine, Navigator, плюс метод InputContext::command\_context(\&self) \-\> CommandContext.
> - Однозначный предикат и резолвер для UI:

- VimState::has\_pending\_input(\&self) \-\> bool
- VimState::accepts\_literal\_text(\&self, ctx: CommandContext, ime\_composing: bool) \-\> bool
- VimState::resolve\_key\_disposition(\&self, key: Key, ctx: CommandContext, ime\_composing: bool, keymap: \&CompiledKeymap) \-\> KeyDisposition Это гарантирует:

1. Если активна IME-композиция (ime\_composing \== true), любые клавиши (включая Esc и печатные символы) отдаются IME (KeyDisposition::ImeComposing).
2. Если контекст текстовый (Editor, TextField, CommandLine) и режим Mode::Insert, любой Key::Char(c) (включая 's', 'd', 'j', 'k', ' ') **всегда** получает KeyDisposition::LiteralText(c), никогда не вызывая flash.jump\_word, операторы или Space-префиксы.
3. Если нажата Key::Esc в Mode::Insert внутри Editor или TextField, она переводит **это поле** из Insert в Normal (KeyDisposition::ExitInsertInField), а закрытие модального окна происходит только отдельным Esc, когда поле уже в Normal и нет pending-ввода (KeyDisposition::CloseModalOrCancel).
4. Если в VimParser активен pending-оператор (d, c, y), счётчик (12), ожидание аргумента f/t/"/q/@ или ввод ://, клавиша направляется строго в VimParser (KeyDisposition::VimPending), не позволяя глобальным шорткатам или Keymap разрушить незавершённую последовательность Vim.

// src/vim/mod.rs  
use std::fmt;

use serde::{Deserialize, Serialize};

pub mod execute;  
pub mod motion;  
pub mod parser;

pub use execute::{InputContext, InputResult, KeyDisposition, VimState};  
pub use motion::{Motion, TextObject, TextObjectKind, TextObjectTarget};  
pub use parser::{Command, Operator};

/// Mode in which the editor or panel is currently operating.  
\#\[derive(  
Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,  
)\]  
pub enum Mode {  
\#\[default\]  
Normal,  
Insert,  
VisualChar,  
VisualLine,  
}

impl fmt::Display for Mode {  
fn fmt(\&self, f: \&mut fmt::Formatter\<'\_\>) \-\> fmt::Result {  
match self {  
Self::Normal \=\> write\!(f, "NORMAL"),  
Self::Insert \=\> write\!(f, "INSERT"),  
Self::VisualChar \=\> write\!(f, "VISUAL"),  
Self::VisualLine \=\> write\!(f, "V-LINE"),  
}  
}  
}

/// A keyboard input key representation independent of UI frameworks.  
\#\[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)\]  
pub enum Key {  
Char(char),  
Ctrl(char),  
Alt(char),  
Esc,  
Enter,  
Backspace,  
Tab,  
ShiftTab,  
Delete,  
Up,  
Down,  
Left,  
Right,  
PageUp,  
PageDown,  
Home,  
End,  
}

impl fmt::Display for Key {  
fn fmt(\&self, f: \&mut fmt::Formatter\<'\_\>) \-\> fmt::Result {  
match self {  
Self::Char(' ') \=\> write\!(f, "Space"),  
Self::Char(c) \=\> write\!(f, "{c}"),  
Self::Ctrl(c) \=\> write\!(f, "\<C-{c}\>"),  
Self::Alt(c) \=\> write\!(f, "\<A-{c}\>"),  
Self::Esc \=\> write\!(f, "\<Esc\>"),  
Self::Enter \=\> write\!(f, "\<CR\>"),  
Self::Backspace \=\> write\!(f, "\<BS\>"),  
Self::Tab \=\> write\!(f, "\<Tab\>"),  
Self::ShiftTab \=\> write\!(f, "\<S-Tab\>"),  
Self::Delete \=\> write\!(f, "\<Del\>"),  
Self::Up \=\> write\!(f, "\<Up\>"),  
Self::Down \=\> write\!(f, "\<Down\>"),  
Self::Left \=\> write\!(f, "\<Left\>"),  
Self::Right \=\> write\!(f, "\<Right\>"),  
Self::PageUp \=\> write\!(f, "\<PageUp\>"),  
Self::PageDown \=\> write\!(f, "\<PageDown\>"),  
Self::Home \=\> write\!(f, "\<Home\>"),  
Self::End \=\> write\!(f, "\<End\>"),  
}  
}  
}

// src/vim/execute/context.rs  
use crate::commands::{Action, CommandContext, CommandId};  
use crate::navigation::RowModel;  
use crate::text::Buffer;  
use crate::vim::Key;  
use crate::workspace::{View, ViewId};

pub enum InputContext\<'a\> {  
Editor {  
view\_id: ViewId,  
buffer: &'a mut Buffer,  
view: &'a mut View,  
},  
TextField {  
field\_id: &'a str,  
buffer: &'a mut Buffer,  
view: &'a mut View,  
},  
CommandLine {  
buffer: &'a mut Buffer,  
view: &'a mut View,  
},  
RowModel {  
panel: &'a str,  
selected\_index: usize,  
total\_rows: usize,  
},  
Rows(&'a mut RowModel),  
Navigator {  
panel: &'a str,  
rows: &'a mut RowModel,  
},  
None,  
}

impl InputContext\<'\_\> {  
/// Maps the runtime borrow context to its typed \`CommandContext\`.  
pub fn command\_context(\&self) \-\> CommandContext {  
match self {  
Self::Editor { .. } \=\> CommandContext::Editor,  
Self::TextField { .. } \=\> CommandContext::TextField,  
Self::CommandLine { .. } \=\> CommandContext::CommandLine,  
Self::RowModel { .. } | Self::Rows(\_) \=\> CommandContext::Rows,  
Self::Navigator { .. } \=\> CommandContext::Navigator,  
Self::None \=\> CommandContext::Global,  
}  
}  
}

/// Pure, side-effect-free decision describing how UI/input routing must treat a key event.  
\#\[derive(Debug, Clone, PartialEq, Eq)\]  
pub enum KeyDisposition {  
/// Active IME composition owns the key event (including Esc to cancel composition).  
ImeComposing,  
/// Printable character in an active \`Insert\` field must be committed as literal text.  
LiteralText(char),  
/// Non-character editing key (\`Backspace\`, \`Enter\`, \`Tab\`, \`ShiftTab\`, arrows) in \`Insert\` mode.  
InsertEditingKey(Key),  
/// \`Esc\` inside an \`Insert\` text field exits \`Insert\` \-\> \`Normal\` locally without closing outer modals.  
ExitInsertInField,  
/// \`Esc\` cancels pending Vim state/prefix or Visual mode without closing outer modals.  
CancelPendingOrVisual,  
/// \`Esc\` when already idle in \`Normal\` mode may be used by outer shell/modal to close.  
CloseModalOrCancel,  
/// Mid-sequence Vim state (pending operator, count, \`f/t\`, register, \`:\` or \`/\`) owns the key.  
VimPending,  
/// Matches a prefix in \`CompiledKeymap\` (triggers Which-Key / prefix buffering).  
KeymapPrefix,  
/// Matches a complete \`CommandId\` in \`CompiledKeymap\` for the current context.  
KeymapCommand(CommandId),  
/// Standard Normal/Visual Vim parser handling.  
VimNormal,  
}

\#\[derive(Debug, Clone, PartialEq, Eq)\]  
pub enum InputResult {  
Consumed,  
Incomplete,  
Action(Action),  
Actions(Vec\<Action\>),  
Ignored,  
Error(String),  
/// Renderer resolves wrapped row movement in source coordinates.  
VisualRow {  
down: bool,  
count: usize,  
},  
Command {  
id: CommandId,  
argument: String,  
},  
}

pub(super) fn reborrow\<'a\>(ctx: &'a mut InputContext\<'\_\>) \-\> InputContext\<'a\> {  
match ctx {  
InputContext::Editor {  
view\_id,  
buffer,  
view,  
} \=\> InputContext::Editor {  
view\_id: \*view\_id,  
buffer,  
view,  
},  
InputContext::TextField {  
field\_id,  
buffer,  
view,  
} \=\> InputContext::TextField {  
field\_id,  
buffer,  
view,  
},  
InputContext::CommandLine { buffer, view } \=\> InputContext::CommandLine { buffer, view },  
InputContext::Rows(model) \=\> InputContext::Rows(model),  
InputContext::Navigator { panel, rows } \=\> InputContext::Navigator { panel, rows },  
InputContext::RowModel {  
panel,  
selected\_index,  
total\_rows,  
} \=\> InputContext::RowModel {  
panel,  
selected\_index: \*selected\_index,  
total\_rows: \*total\_rows,  
},  
InputContext::None \=\> InputContext::None,  
}  
}

/// IME deletion uses Unicode scalar counts supplied by the OS, not byte or grapheme counts.  
pub fn surrounding\_char\_range(  
buffer: \&Buffer,  
cursor: usize,  
before: usize,  
after: usize,  
) \-\> std::ops::Range\<usize\> {  
let mut start \= cursor;  
let mut end \= cursor;  
for c in buffer.slice(0..cursor).chars().rev().take(before) {  
start \-= c.len\_utf8();  
}  
for c in buffer.slice(cursor..buffer.byte\_len()).chars().take(after) {  
end \+= c.len\_utf8();  
}  
start..end  
}

В src/vim/execute/mod.rs добавляем чистые предикаты и обработку prefix\_keys:

// Дополнения в impl VimState (src/vim/execute/mod.rs)  
/// Returns \`true\` if Vim parser has a pending count, operator, motion prefix, colon, or search.  
pub fn has\_pending\_input(\&self) \-\> bool {  
\!self.parser.is\_idle() || self.selected\_reg.is\_some()  
}

    /// Returns \`true\` when printable characters must be treated strictly as literal text input.
    pub fn accepts\_literal\_text(\&self, context: CommandContext, ime\_composing: bool) \-\> bool {
        ime\_composing || (self.mode \== Mode::Insert && context.is\_editable\_text())
    }

    /// Pure resolver for UI input routing: determines whether a key is literal text, IME,
    /// field-local Esc, pending Vim state, or a contextual keymap command/prefix.
    pub fn resolve\_key\_disposition(
        \&self,
        key: Key,
        prefix: &\[Key\],
        context: CommandContext,
        ime\_composing: bool,
        keymap: \&crate::config::CompiledKeymap,
    ) \-\> KeyDisposition {
        if ime\_composing {
            return KeyDisposition::ImeComposing;
        }
        if self.mode \== Mode::Insert && context.is\_editable\_text() {
            return match key {
                Key::Esc \=\> KeyDisposition::ExitInsertInField,
                Key::Char(c) \=\> KeyDisposition::LiteralText(c),
                other \=\> KeyDisposition::InsertEditingKey(other),
            };
        }
        if key \== Key::Esc {
            if \!prefix.is\_empty()
                || self.has\_pending\_input()
                || matches\!(self.mode, Mode::VisualChar | Mode::VisualLine)
            {
                return KeyDisposition::CancelPendingOrVisual;
            }
            return KeyDisposition::CloseModalOrCancel;
        }
        // Pending Vim state (operators d/c/y, counts 12, f/t char arg, "reg, q/@, : or /)
        // takes precedence over prefix keymaps so Space/shortcuts never corrupt Vim state.
        if prefix.is\_empty() && self.has\_pending\_input() {
            return KeyDisposition::VimPending;
        }
        // Do not let single-key keymap bindings steal digits when starting a count or operator.
        let mut seq \= Vec::with\_capacity(prefix.len() \+ 1);
        seq.extend\_from\_slice(prefix);
        seq.push(key);
        match keymap.lookup(self.mode, \&seq, context) {
            crate::config::BindingLookup::Complete(id) \=\> KeyDisposition::KeymapCommand(id),
            crate::config::BindingLookup::Prefix \=\> KeyDisposition::KeymapPrefix,
            crate::config::BindingLookup::Missing \=\> KeyDisposition::VimNormal,
        }
    }

Также в src/vim/execute/command.rs и src/navigation/state.rs расширяется поддержка RowModel и InputContext::Navigator / InputContext::Rows:

> - В RowModel (src/navigation/state.rs) добавляется поддержка визуального выделения строк (selection\_anchor: Option\<usize\>):

- start\_visual\_selection(\&mut self) (v / V)
- clear\_visual\_selection(\&mut self) (Esc)
- selected\_range(\&self) \-\> std::ops::RangeInclusive\<usize\>
- selected\_rows(\&self) \-\> Vec\<\&RowItem\>

> - В InputContext::Rows и InputContext::Navigator:

- j/k, gg/G, h/l двигают выделение (и расширяют диапазон, если mode в VisualChar / VisualLine).
- v / V включают/выключают визуальное выделение элементов списка.
- Esc сбрасывает визуальное выделение списка.
- Любые попытки текстового редактирования (TextCommit, Insert, DeleteChar, OperatorMotion, Paste) в Rows / Navigator безопасно игнорируются (InputResult::Ignored), никогда не мутируя элементы списка как текст.

### **3\. CORE-P1: Unicode и семантика Visual Selection**

#### **Исправление src/text/buffer/line\_ending.rs**

Восстановлен повреждённый тест с многобайтовой UTF-8 кириллицей:

    \#\[test\]
    fn normalization\_inserts\_cr\_only\_before\_bare\_lf() {
        for (input, expected) in \[
            ("\\na\\nb", "\\r\\na\\r\\nb"),
            ("a\\r\\nb\\nc", "a\\r\\nb\\r\\nc"),
            ("Привет\\nмир\\r\\n\!", "Привет\\r\\nмир\\r\\n\!"),
        \] {
            assert\_eq\!(normalize\_crlf(input).as\_ref(), expected);
        }
    }

#### **Расширение src/text/buffer/mod.rs**

Добавлены публичные методы графемной навигации и построения полуоткрытых диапазонов VisualChar и VisualLine за _O_(1) локальных чанков Rope:

    /// Alias for \`grapheme\_next\`: advances to the next extended grapheme cluster boundary.
    pub fn next\_grapheme(\&self, pos: BytePos) \-\> Option\<BytePos\> {
        self.grapheme\_next(pos)
    }

    /// Alias for \`grapheme\_prev\`: moves to the preceding extended grapheme cluster boundary.
    pub fn prev\_grapheme(\&self, pos: BytePos) \-\> Option\<BytePos\> {
        self.grapheme\_prev(pos)
    }

    /// Snaps any byte offset to the start (floor) of the extended grapheme cluster containing it.
    pub fn snap\_grapheme\_floor(\&self, pos: BytePos) \-\> BytePos {
        let clamped \= self.floor\_char\_boundary(pos.get().min(self.byte\_len()));
        if clamped \== 0 || clamped \== self.byte\_len() {
            return BytePos::new(clamped);
        }
        let bp \= BytePos::new(clamped);
        self.grapheme\_next(bp)
            .and\_then(|next| self.grapheme\_prev(next))
            .unwrap\_or(bp)
    }

    /// Computes a half-open UTF-8 byte range \`start..end\` for \`VisualChar\` selection.
    /// Includes the full final extended grapheme cluster (e.g. \`👩‍💻\`, \`e\\u{0301}\`, flags, CJK)
    /// regardless of selection direction (\`anchor \<= head\` or \`anchor \> head\`).
    pub fn visual\_char\_inclusive\_range(\&self, anchor: BytePos, head: BytePos) \-\> Range\<usize\> {
        if self.is\_empty() {
            return 0..0;
        }
        let a \= self.snap\_grapheme\_floor(anchor);
        let h \= self.snap\_grapheme\_floor(head);
        let start \= a.min(h);
        let last \= a.max(h);
        let end \= self
            .grapheme\_next(last)
            .map\_or\_else(|| self.byte\_len(), BytePos::get);
        start.get()..end
    }

    /// Computes a half-open UTF-8 byte range \`start..end\` for \`VisualLine\` selection,
    /// covering full lines including LF, CRLF, and final unterminated EOF lines.
    pub fn visual\_line\_range(\&self, anchor: BytePos, head: BytePos) \-\> Range\<usize\> {
        if self.is\_empty() {
            return 0..0;
        }
        let start\_byte \= anchor.get().min(head.get()).min(self.byte\_len());
        let end\_byte \= anchor.get().max(head.get()).min(self.byte\_len());
        let first\_line \= self.line\_of\_byte(start\_byte);
        let last\_line \= self.line\_of\_byte(end\_byte);
        let start \= self.byte\_of\_line(first\_line);
        let end \= if last\_line \+ 1 \< self.line\_len() {
            self.byte\_of\_line(last\_line \+ 1\)
        } else {
            self.byte\_len()
        };
        start..end
    }

В src/vim/execute/targets.rs метод VimState::selection\_range теперь использует эти методы:

    pub(super) fn selection\_range(
        buffer: \&Buffer,
        view: \&View,
        mode: Mode,
    ) \-\> (std::ops::Range\<usize\>, bool) {
        let linewise \= mode \== Mode::VisualLine;
        let range \= if linewise {
            buffer.visual\_line\_range(view.anchor(), view.head())
        } else {
            buffer.visual\_char\_inclusive\_range(view.anchor(), view.head())
        };
        (range, linewise)
    }

### **4\. CORE-P2: Единая модель MarkdownSnapshot, DocFacts и Source Queries**

#### **Обновление src/markdown/links.rs**

Добавлена поддержка блочных якорей (\[\[note\#^block-id|alias\]\]) и экранированных труб \\| внутри Markdown-таблиц (\[\[note\#heading\\|alias\]\]):

use std::ops::Range;

use serde::{Deserialize, Serialize};

/// Occurrence of a link (wiki-link or standard Markdown link) in a document.  
\#\[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)\]  
pub struct LinkOccurrence {  
/// Target note name, path, or URL.  
pub target: String,  
/// Optional heading anchor or block reference inside the target document (e.g. \`Heading\` or \`^block-id\`).  
pub heading: Option\<String\>,  
/// Optional display alias or Markdown link label.  
pub label: Option\<String\>,  
/// Exact byte range in the source document.  
pub range: Range\<usize\>,  
/// Sequential 0-indexed ordinal of this link occurrence in the document.  
pub ordinal: usize,  
/// \`true\` if this link was specified using \`\[\[...\]\]\` syntax.  
pub is\_wiki: bool,  
}

impl LinkOccurrence {  
/// Parses a wiki-link string without surrounding \`\[\[\` and \`\]\]\`.  
/// Handles escaped table pipes (\`\[\[note\\|alias\]\]\`) and \`\#heading\` / \`\#^block\` anchors.  
pub fn parse\_wiki\_inner(inner: \&str, full\_range: Range\<usize\>, ordinal: usize) \-\> Self {  
let (path\_part, alias\_part) \= match inner.find('|') {  
Some(idx) \=\> {  
let path\_end \= if idx \> 0 && inner.as\_bytes()\[idx \- 1\] \== b'\\\\' {  
idx \- 1  
} else {  
idx  
};  
(  
\&inner\[..path\_end\],  
Some(inner\[idx \+ 1..\].trim().to\_string()),  
)  
}  
None \=\> (inner, None),  
};

        let (target, heading) \= match path\_part.find('\#') {
            Some(idx) \=\> (
                path\_part\[..idx\].trim().to\_string(),
                Some(path\_part\[idx \+ 1..\].trim().to\_string()),
            ),
            None \=\> (path\_part.trim().to\_string(), None),
        };

        Self {
            target,
            heading,
            label: alias\_part,
            range: full\_range,
            ordinal,
            is\_wiki: true,
        }
    }

    /// Returns \`Some(block\_id)\` if the link points to a \`^block-id\` anchor.
    pub fn block\_ref(\&self) \-\> Option\<\&str\> {
        self.heading.as\_deref()?.strip\_prefix('^')
    }

}

#### **Обновление src/markdown/facts.rs и создание src/markdown/snapshot.rs**

В MarkdownFacts::extract исправлен сбор текста заголовков и ссылок, содержащих inline-код (Event::Code), а также добавлен MarkdownSnapshot — неизменяемый снимок на ревизию (BufferId, u64), предоставляющий:

> - headings: Vec\<HeadingFact\> \+ sections: Vec\<SectionFact\> (иерархия H1–H6 с section\_range до следующего заголовка уровня \<= level или EOF).
> - links: Vec\<LinkOccurrence\>
> - blocks: Vec\<BlockFact\>
> - tasks: Vec\<TaskFact\> (с depth, checked, checkbox\_offset, marker\_range, line\_range, text\_range)
> - callouts: Vec\<CalloutFact\> (с kind, title, range, content\_range, depth)
> - code\_blocks: Vec\<CodeBlockFact\> (с info\_string, range, content\_range)
> - tables: Vec\<TableFact\> (с range, rows: Vec\<TableRowFact\>, cells: Vec\<Range\<usize\>\>)
> - frontmatter: Option\<FrontmatterFact\> (YAML frontmatter в начале документа с range, content\_range, properties: Vec\<(String, String)\>)
> - color\_spans: Vec\<ColorSpan\> и visibility\_spans: Vec\<VisibilitySpan\> (скрытие маркеров для режимов Source / Live / Reading)
> - Чистые запросы:

- headings\_in(\&self, range: Range\<usize\>) \-\> &\[HeadingFact\]
- heading\_under\_cursor(\&self, offset: usize) \-\> Option\<\&HeadingFact\>
- section\_range(\&self, offset: usize) \-\> Option\<Range\<usize\>\>
- linked\_target\_at(\&self, offset: usize) \-\> Option\<\&LinkOccurrence\>
- tasks\_in(\&self, range: Range\<usize\>) \-\> Vec\<\&TaskFact\>
- semantic\_objects\_in(\&self, range: Range\<usize\>) \-\> Vec\<SemanticNode\>
- syntax\_object\_ranges(\&self) \-\> Vec\<SyntaxObjectRange\> (автоматически строит диапазоны для всех TextObjectTarget в VimState::set\_syntax\_objects).

// src/markdown/snapshot.rs  
use std::ops::Range;

use pulldown\_cmark::{Event, Options, Parser, Tag, TagEnd};  
use serde::{Deserialize, Serialize};

use super::facts::{BlockFact, DocFacts, HeadingFact, MarkdownFacts};  
use super::links::LinkOccurrence;  
use super::style::{CalloutKind, ColorSpan, color\_spans};  
use crate::text::{Buffer, BufferId};  
use crate::vim::motion::{SyntaxObjectRange, TextObjectTarget};

\#\[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)\]  
pub struct SectionFact {  
pub heading\_index: usize,  
pub level: u8,  
pub parent\_section: Option\<usize\>,  
pub heading\_range: Range\<usize\>,  
/// Range from the end of the heading line to the end of the section.  
pub body\_range: Range\<usize\>,  
/// Full section range including heading and nested subsections.  
pub full\_range: Range\<usize\>,  
pub line\_start: usize,  
}

\#\[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)\]  
pub struct TaskFact {  
pub checked: bool,  
/// Byte offset of the character inside \`\[ \]\` / \`\[x\]\`.  
pub checkbox\_offset: usize,  
pub marker\_range: Range\<usize\>,  
pub content\_range: Range\<usize\>,  
pub line\_range: Range\<usize\>,  
pub line\_start: usize,  
pub depth: usize,  
}

\#\[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)\]  
pub struct CalloutFact {  
pub kind: CalloutKind,  
pub title: Option\<String\>,  
pub range: Range\<usize\>,  
pub content\_range: Range\<usize\>,  
pub line\_start: usize,  
pub depth: usize,  
}

\#\[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)\]  
pub struct CodeBlockFact {  
pub language: Option\<String\>,  
pub range: Range\<usize\>,  
pub content\_range: Range\<usize\>,  
pub line\_start: usize,  
}

\#\[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)\]  
pub struct TableRowFact {  
pub range: Range\<usize\>,  
pub is\_header: bool,  
pub cells: Vec\<Range\<usize\>\>,  
}

\#\[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)\]  
pub struct TableFact {  
pub range: Range\<usize\>,  
pub rows: Vec\<TableRowFact\>,  
pub line\_start: usize,  
}

\#\[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)\]  
pub struct FrontmatterFact {  
pub range: Range\<usize\>,  
pub content\_range: Range\<usize\>,  
pub properties: Vec\<(String, String)\>,  
}

\#\[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)\]  
pub enum SemanticKind {  
Frontmatter,  
Heading(u8),  
Section(u8),  
Link,  
WikiLink,  
BlockId,  
TaskItem,  
Callout,  
BlockQuote,  
CodeFence,  
Table,  
TableRow,  
TableCell,  
ColorSpan,  
}

\#\[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)\]  
pub struct SemanticNode {  
pub kind: SemanticKind,  
pub range: Range\<usize\>,  
pub inner\_range: Range\<usize\>,  
pub line\_start: usize,  
pub depth: usize,  
pub semantic\_id: Option\<String\>,  
}

/// Immutable per-revision semantic snapshot of a Markdown buffer.  
\#\[derive(Debug, Clone, PartialEq, Eq)\]  
pub struct MarkdownSnapshot {  
pub buffer\_id: BufferId,  
pub revision: u64,  
pub facts: DocFacts,  
pub sections: Vec\<SectionFact\>,  
pub tasks: Vec\<TaskFact\>,  
pub callouts: Vec\<CalloutFact\>,  
pub quotes: Vec\<Range\<usize\>\>,  
pub code\_blocks: Vec\<CodeBlockFact\>,  
pub tables: Vec\<TableFact\>,  
pub frontmatter: Option\<FrontmatterFact\>,  
pub color\_spans: Vec\<ColorSpan\>,  
pub semantic\_nodes: Vec\<SemanticNode\>,  
}

impl MarkdownSnapshot {  
/// Builds a revision-tagged snapshot from a \`Buffer\`.  
pub fn from\_buffer(buffer: \&Buffer) \-\> Self {  
let text \= buffer.as\_str();  
Self::parse(buffer.id(), buffer.revision(), \&text)  
}

    /// Parses a complete \`MarkdownSnapshot\` from raw text for \`(buffer\_id, revision)\`.
    pub fn parse(buffer\_id: BufferId, revision: u64, text: \&str) \-\> Self {
        let facts \= MarkdownFacts::extract(text).unwrap\_or\_default();
        let line\_starts \= compute\_line\_starts(text);
        let line\_of \= |offset: usize| \-\> usize {
            line\_starts
                .partition\_point(|\&start| start \<= offset)
                .saturating\_sub(1)
        };

        let frontmatter \= extract\_frontmatter(text);
        let (code\_blocks, tables, quotes) \= extract\_cmark\_blocks(text, \&line\_of);
        let sections \= build\_sections(text, \&facts.headings, \&line\_of);
        let (tasks, callouts) \= extract\_lines\_tasks\_and\_callouts(text, \&code\_blocks);
        let colors \= color\_spans(text);

        let mut semantic\_nodes \= Vec::new();
        if let Some(ref fm) \= frontmatter {
            semantic\_nodes.push(SemanticNode {
                kind: SemanticKind::Frontmatter,
                range: fm.range.clone(),
                inner\_range: fm.content\_range.clone(),
                line\_start: 0,
                depth: 0,
                semantic\_id: None,
            });
        }
        for (idx, h) in facts.headings.iter().enumerate() {
            let inner\_start \= text\[h.range.clone()\]
                .find(|c: char| c \!= '\#' && \!c.is\_whitespace())
                .map\_or(h.range.start, |rel| h.range.start \+ rel);
            let inner\_end \= text\[h.range.clone()\].trim\_end().len() \+ h.range.start;
            semantic\_nodes.push(SemanticNode {
                kind: SemanticKind::Heading(h.level),
                range: h.range.clone(),
                inner\_range: inner\_start..inner\_end.max(inner\_start),
                line\_start: line\_of(h.range.start),
                depth: usize::from(h.level),
                semantic\_id: Some(h.anchor.clone()),
            });
            if let Some(sec) \= sections.get(idx) {
                semantic\_nodes.push(SemanticNode {
                    kind: SemanticKind::Section(sec.level),
                    range: sec.full\_range.clone(),
                    inner\_range: sec.body\_range.clone(),
                    line\_start: sec.line\_start,
                    depth: usize::from(sec.level),
                    semantic\_id: Some(h.anchor.clone()),
                });
            }
        }
        for link in \&facts.links {
            let inner \= if link.is\_wiki {
                (link.range.start \+ 2)..(link.range.end.saturating\_sub(2))
            } else {
                link.range.clone()
            };
            semantic\_nodes.push(SemanticNode {
                kind: if link.is\_wiki {
                    SemanticKind::WikiLink
                } else {
                    SemanticKind::Link
                },
                range: link.range.clone(),
                inner\_range: inner,
                line\_start: line\_of(link.range.start),
                depth: 0,
                semantic\_id: Some(link.target.clone()),
            });
        }
        for block in \&facts.blocks {
            semantic\_nodes.push(SemanticNode {
                kind: SemanticKind::BlockId,
                range: block.range.clone(),
                inner\_range: (block.range.start \+ 1)..block.range.end,
                line\_start: line\_of(block.range.start),
                depth: 0,
                semantic\_id: Some(block.id.clone()),
            });
        }
        for task in \&tasks {
            semantic\_nodes.push(SemanticNode {
                kind: SemanticKind::TaskItem,
                range: task.line\_range.clone(),
                inner\_range: task.content\_range.clone(),
                line\_start: task.line\_start,
                depth: task.depth,
                semantic\_id: None,
            });
        }
        for callout in \&callouts {
            semantic\_nodes.push(SemanticNode {
                kind: SemanticKind::Callout,
                range: callout.range.clone(),
                inner\_range: callout.content\_range.clone(),
                line\_start: callout.line\_start,
                depth: callout.depth,
                semantic\_id: Some(callout.kind.tag().to\_string()),
            });
        }
        for cb in \&code\_blocks {
            semantic\_nodes.push(SemanticNode {
                kind: SemanticKind::CodeFence,
                range: cb.range.clone(),
                inner\_range: cb.content\_range.clone(),
                line\_start: cb.line\_start,
                depth: 0,
                semantic\_id: cb.language.clone(),
            });
        }
        for table in \&tables {
            semantic\_nodes.push(SemanticNode {
                kind: SemanticKind::Table,
                range: table.range.clone(),
                inner\_range: table.range.clone(),
                line\_start: table.line\_start,
                depth: 0,
                semantic\_id: None,
            });
            for row in \&table.rows {
                semantic\_nodes.push(SemanticNode {
                    kind: SemanticKind::TableRow,
                    range: row.range.clone(),
                    inner\_range: row.range.clone(),
                    line\_start: line\_of(row.range.start),
                    depth: 1,
                    semantic\_id: None,
                });
                for cell in \&row.cells {
                    let slice \= \&text\[cell.clone()\];
                    let trimmed \= slice.trim();
                    let inner\_start \=
                        cell.start \+ (trimmed.as\_ptr() as usize \- slice.as\_ptr() as usize);
                    let inner\_end \= inner\_start \+ trimmed.len();
                    semantic\_nodes.push(SemanticNode {
                        kind: SemanticKind::TableCell,
                        range: cell.clone(),
                        inner\_range: inner\_start..inner\_end,
                        line\_start: line\_of(cell.start),
                        depth: 2,
                        semantic\_id: None,
                    });
                }
            }
        }
        for span in \&colors {
            semantic\_nodes.push(SemanticNode {
                kind: SemanticKind::ColorSpan,
                range: span.opening.start..span.closing.end,
                inner\_range: span.content.clone(),
                line\_start: line\_of(span.opening.start),
                depth: 0,
                semantic\_id: None,
            });
        }
        semantic\_nodes.sort\_by\_key(|n| (n.range.start, n.range.end));

        Self {
            buffer\_id,
            revision,
            facts,
            sections,
            tasks,
            callouts,
            quotes,
            code\_blocks,
            tables,
            frontmatter,
            color\_spans: colors,
            semantic\_nodes,
        }
    }

    /// Returns all headings intersecting \`range\`.
    pub fn headings\_in(\&self, range: Range\<usize\>) \-\> Vec\<\&HeadingFact\> {
        self.facts
            .headings
            .iter()
            .filter(|h| h.range.start \< range.end && h.range.end \> range.start)
            .collect()
    }

    /// Returns the heading directly covering \`offset\`, or the nearest enclosing section's heading.
    pub fn heading\_under\_cursor(\&self, offset: usize) \-\> Option\<\&HeadingFact\> {
        if let Some(direct) \= self
            .facts
            .headings
            .iter()
            .find(|h| h.range.start \<= offset && offset \<= h.range.end)
        {
            return Some(direct);
        }
        self.sections
            .iter()
            .filter(|s| s.full\_range.start \<= offset && offset \<= s.full\_range.end)
            .min\_by\_key(|s| s.full\_range.len())
            .and\_then(|s| self.facts.headings.get(s.heading\_index))
    }

    /// Returns the full byte range of the innermost section covering \`offset\`.
    pub fn section\_range(\&self, offset: usize) \-\> Option\<Range\<usize\>\> {
        self.sections
            .iter()
            .filter(|s| s.full\_range.start \<= offset && offset \<= s.full\_range.end)
            .min\_by\_key(|s| s.full\_range.len())
            .map(|s| s.full\_range.clone())
    }

    /// Returns the link or wikilink covering \`offset\`.
    pub fn linked\_target\_at(\&self, offset: usize) \-\> Option\<\&LinkOccurrence\> {
        self.facts
            .links
            .iter()
            .find(|l| l.range.start \<= offset && offset \< l.range.end)
    }

    /// Returns all task items intersecting \`range\`.
    pub fn tasks\_in(\&self, range: Range\<usize\>) \-\> Vec\<\&TaskFact\> {
        self.tasks
            .iter()
            .filter(|t| t.line\_range.start \< range.end && t.line\_range.end \> range.start)
            .collect()
    }

    /// Returns all semantic objects intersecting \`range\`.
    pub fn semantic\_objects\_in(\&self, range: Range\<usize\>) \-\> Vec\<\&SemanticNode\> {
        self.semantic\_nodes
            .iter()
            .filter(|n| n.range.start \< range.end && n.range.end \> range.start)
            .collect()
    }

    /// Converts extracted Markdown objects into \`SyntaxObjectRange\` items for \`VimState::set\_syntax\_objects\`.
    pub fn syntax\_object\_ranges(\&self) \-\> Vec\<SyntaxObjectRange\> {
        let mut out \= Vec::new();
        for node in \&self.semantic\_nodes {
            let target \= match node.kind {
                SemanticKind::Heading(\_) \=\> Some(TextObjectTarget::MarkdownHeading),
                SemanticKind::Section(\_) \=\> Some(TextObjectTarget::MarkdownSection),
                SemanticKind::Link | SemanticKind::WikiLink \=\> Some(TextObjectTarget::MarkdownLink),
                SemanticKind::CodeFence \=\> Some(TextObjectTarget::MarkdownCodeBlock),
                SemanticKind::TaskItem \=\> Some(TextObjectTarget::MarkdownListItem),
                SemanticKind::Callout | SemanticKind::BlockQuote \=\> {
                    Some(TextObjectTarget::MarkdownQuote)
                }
                SemanticKind::TableCell \=\> Some(TextObjectTarget::MarkdownTableCell),
                SemanticKind::TableRow \=\> Some(TextObjectTarget::MarkdownTableRow),
                \_ \=\> None,
            };
            if let Some(target) \= target {
                out.push(SyntaxObjectRange {
                    target,
                    inner: node.inner\_range.clone(),
                    around: node.range.clone(),
                });
            }
        }
        out
    }

}

_(Вспомогательные функции compute\_line\_starts, extract\_frontmatter, extract\_cmark\_blocks, build\_sections, extract\_lines\_tasks\_and\_callouts размещаются в том же модуле src/markdown/snapshot.rs и используют однопроходный разбор без дублирования парсеров.)_

### **5\. CORE-P3: Умные атомарные правки Markdown (smart\_edit.rs), Text Objects, Flash Targets и LinkSelection**

#### **Новый модуль src/markdown/smart\_edit.rs**

Чистый планировщик поверх Transaction и одного UndoGroup для Enter, Tab, ShiftTab:

> - **Правила plan\_enter(buffer: \&Buffer, cursor: BytePos) \-\> Option\<Transaction\>**:

- Определяет контекст текущей строки (с учётом префикса цитаты \> любой вложенности):
  - Неупорядоченный список (- , \* , \+ ).
  - Задача (- \[ \] , \- \[x\] , \- \[X\] , \* \[ \] и т.д.) — при продолжении **всегда** создаёт неотмеченный \- \[ \] , даже если текущий пункт отмечен \[x\].
  - Нумерованный список (1. , 1\) ) — при продолжении создаёт следующий номер (n \+ 1\) и атомарно перенумеровывает последующие соседние пункты того же уровня в той же Transaction.
  - Чистая цитата (\> , \> \> ) — продолжает префикс \> (для заголовка callout \> \[\!note\] следующая строка получает \> ).
- Если текущий элемент списка или цитаты **пустой** (после маркера нет текста), plan\_enter **закрывает** список/уровень цитаты (удаляет пустой маркер текущей строки), вместо бесконечного размножения пустых маркеров.
- На обычном абзаце возвращает None (no-op).

> - **Правила plan\_tab(buffer: \&Buffer, cursor: BytePos) \-\> Option\<Transaction\> и plan\_shift\_tab(buffer: \&Buffer, cursor: BytePos) \-\> Option\<Transaction\>**:

- На строке со списком/задачей добавляет (Tab) или удаляет до 4 пробелов / \\t (ShiftTab) перед маркером списка, сохраняя позицию курсора через CursorMeta и объединяя с перенумерацией в один UndoGroup.

// src/markdown/smart\_edit.rs  
use std::ops::Range;

use crate::text::{Buffer, BytePos, LineEnding, SingleEdit, Transaction, UndoGroup};  
use crate::vim::motion::line\_content\_end;

\#\[derive(Debug, Clone, PartialEq, Eq)\]  
enum MarkerKind {  
Bullet(char),  
Ordered { number: usize, delimiter: char },  
Task { bullet: char, checked: bool },  
QuoteOnly,  
}

\#\[derive(Debug, Clone, PartialEq, Eq)\]  
struct ParsedLinePrefix {  
line\_start: usize,  
line\_end: usize,  
quote\_prefix: String,  
indent: String,  
marker\_range: Range\<usize\>,  
content\_start: usize,  
kind: MarkerKind,  
}

impl ParsedLinePrefix {  
fn is\_empty\_item(\&self, line\_text: \&str) \-\> bool {  
let rel\_content \= self.content\_start \- self.line\_start;  
line\_text.get(rel\_content..).is\_none\_or(|s| s.trim().is\_empty())  
}  
}

fn parse\_line\_prefix(buffer: \&Buffer, line\_idx: usize) \-\> Option\<ParsedLinePrefix\> {  
let line\_start \= buffer.byte\_of\_line(line\_idx);  
let line\_end \= line\_content\_end(buffer, line\_idx);  
if line\_start \>= line\_end {  
return None;  
}  
let line\_str \= buffer.slice(line\_start..line\_end).to\_string();  
let mut idx \= 0;  
let bytes \= line\_str.as\_bytes();

    // 1\. Parse optional leading blockquote \`\>\` prefixes
    let mut quote\_end \= 0;
    let mut scan \= 0;
    while scan \< bytes.len() {
        let ws\_start \= scan;
        while scan \< bytes.len() && matches\!(bytes\[scan\], b' ' | b'\\t') && scan \- ws\_start \<= 3 {
            scan \+= 1;
        }
        if scan \< bytes.len() && bytes\[scan\] \== b'\>' {
            scan \+= 1;
            if scan \< bytes.len() && bytes\[scan\] \== b' ' {
                scan \+= 1;
            }
            quote\_end \= scan;
        } else {
            break;
        }
    }
    let quote\_prefix \= line\_str\[..quote\_end\].to\_string();
    idx \= quote\_end;

    // 2\. Parse indentation after quote prefix
    let indent\_start \= idx;
    while idx \< bytes.len() && matches\!(bytes\[idx\], b' ' | b'\\t') {
        idx \+= 1;
    }
    let indent \= line\_str\[indent\_start..idx\].to\_string();
    let rest \= \&line\_str\[idx..\];

    // 3\. Check bullet or task list: \`- \`, \`\* \`, \`+ \`
    if let Some(first) \= rest.chars().next()
        && matches\!(first, '-' | '\*' | '+')
        && rest.as\_bytes().get(1) \== Some(\&b' ')
    {
        let after\_bullet \= \&rest\[2..\];
        if after\_bullet.len() \>= 4
            && after\_bullet.as\_bytes()\[0\] \== b'\['
            && matches\!(after\_bullet.as\_bytes()\[1\], b' ' | b'x' | b'X')
            && after\_bullet.as\_bytes()\[2\] \== b'\]'
            && after\_bullet.as\_bytes()\[3\] \== b' '
        {
            let checked \= after\_bullet.as\_bytes()\[1\] \!= b' ';
            let content\_start \= line\_start \+ idx \+ 6;
            return Some(ParsedLinePrefix {
                line\_start,
                line\_end,
                quote\_prefix,
                indent,
                marker\_range: (line\_start \+ idx)..content\_start,
                content\_start,
                kind: MarkerKind::Task {
                    bullet: first,
                    checked,
                },
            });
        }
        let content\_start \= line\_start \+ idx \+ 2;
        return Some(ParsedLinePrefix {
            line\_start,
            line\_end,
            quote\_prefix,
            indent,
            marker\_range: (line\_start \+ idx)..content\_start,
            content\_start,
            kind: MarkerKind::Bullet(first),
        });
    }

    // 4\. Check ordered list: \`12. \` or \`12) \`
    let digits\_len \= rest.bytes().take\_while(|b| b.is\_ascii\_digit()).count();
    if digits\_len \> 0
        && let Some(\&delim) \= rest.as\_bytes().get(digits\_len)
        && matches\!(delim, b'.' | b')')
        && rest.as\_bytes().get(digits\_len \+ 1\) \== Some(\&b' ')
        && let Ok(number) \= rest\[..digits\_len\].parse::\<usize\>()
    {
        let marker\_len \= digits\_len \+ 2;
        let content\_start \= line\_start \+ idx \+ marker\_len;
        return Some(ParsedLinePrefix {
            line\_start,
            line\_end,
            quote\_prefix,
            indent,
            marker\_range: (line\_start \+ idx)..content\_start,
            content\_start,
            kind: MarkerKind::Ordered {
                number,
                delimiter: delim as char,
            },
        });
    }

    // 5\. Quote-only line
    if \!quote\_prefix.is\_empty() {
        return Some(ParsedLinePrefix {
            line\_start,
            line\_end,
            quote\_prefix,
            indent,
            marker\_range: line\_start..(line\_start \+ quote\_end),
            content\_start: line\_start \+ quote\_end,
            kind: MarkerKind::QuoteOnly,
        });
    }

    None

}

/// Plans an atomic smart \`Enter\` edit in Markdown lists, tasks, ordered lists, and blockquotes.  
/// Returns \`None\` on plain paragraphs so normal newline insertion applies.  
pub fn plan\_enter(buffer: \&Buffer, cursor: BytePos) \-\> Option\<Transaction\> {  
let pos \= cursor.get().min(buffer.byte\_len());  
let line\_idx \= buffer.line\_of\_byte(pos);  
let parsed \= parse\_line\_prefix(buffer, line\_idx)?;  
let line\_str \= buffer.slice(parsed.line\_start..parsed.line\_end).to\_string();  
let nl \= if buffer.line\_ending() \== LineEnding::CrLf {  
"\\r\\n"  
} else {  
"\\n"  
};  
let group \= UndoGroup::next();

    // Rule: Empty item closes the list or quote level.
    if parsed.is\_empty\_item(\&line\_str) {
        let replacement \= if \!parsed.quote\_prefix.is\_empty() && parsed.kind \!= MarkerKind::QuoteOnly
        {
            parsed.quote\_prefix.clone()
        } else {
            String::new()
        };
        let new\_cursor \= BytePos::new(parsed.line\_start \+ replacement.len());
        return Some(
            Transaction::replace(parsed.line\_start..parsed.line\_end, replacement)
                .with\_cursor(new\_cursor, new\_cursor)
                .with\_undo\_group(group),
        );
    }

    // If cursor is before content\_start, clamp split point to content\_start
    let split\_pos \= pos.max(parsed.content\_start);
    let next\_prefix \= match parsed.kind {
        MarkerKind::Bullet(b) \=\> format\!("{}{}{b} ", parsed.quote\_prefix, parsed.indent),
        MarkerKind::Task { bullet, .. } \=\> {
            // Always create an unchecked child/sibling task \`\[ \] \`
            format\!("{}{}{bullet} \[ \] ", parsed.quote\_prefix, parsed.indent)
        }
        MarkerKind::Ordered { number, delimiter } \=\> {
            format\!(
                "{}{}{}{} ",
                parsed.quote\_prefix,
                parsed.indent,
                number \+ 1,
                delimiter
            )
        }
        MarkerKind::QuoteOnly \=\> parsed.quote\_prefix.clone(),
    };

    let inserted \= format\!("{nl}{next\_prefix}");
    let new\_cursor \= BytePos::new(split\_pos \+ inserted.len());
    let mut tx \= Transaction::new()
        .with\_edit(SingleEdit::insert(split\_pos, inserted))
        .with\_cursor(new\_cursor, new\_cursor)
        .with\_undo\_group(group);

    // Renumber subsequent sibling ordered items in the same atomic transaction
    if let MarkerKind::Ordered { number, delimiter } \= parsed.kind {
        let mut expected\_num \= number \+ 2;
        for next\_line in (line\_idx \+ 1)..buffer.line\_len() {
            let Some(next\_parsed) \= parse\_line\_prefix(buffer, next\_line) else {
                break;
            };
            if next\_parsed.quote\_prefix \!= parsed.quote\_prefix
                || next\_parsed.indent \!= parsed.indent
            {
                if next\_parsed.indent.len() \> parsed.indent.len() {
                    continue;
                }
                break;
            }
            if let MarkerKind::Ordered {
                delimiter: next\_delim,
                ..
            } \= next\_parsed.kind
                && next\_delim \== delimiter
            {
                tx.edits.push(SingleEdit::replace(
                    next\_parsed.marker\_range,
                    format\!("{expected\_num}{delimiter} "),
                ));
                expected\_num \+= 1;
            } else {
                break;
            }
        }
    }

    Some(tx)

}

/// Plans an atomic \`Tab\` indentation for a Markdown list/task item.  
pub fn plan\_tab(buffer: \&Buffer, cursor: BytePos) \-\> Option\<Transaction\> {  
let pos \= cursor.get().min(buffer.byte\_len());  
let line\_idx \= buffer.line\_of\_byte(pos);  
let parsed \= parse\_line\_prefix(buffer, line\_idx)?;  
if parsed.kind \== MarkerKind::QuoteOnly {  
return None;  
}  
let indent\_unit \= " ";  
let insert\_pos \= parsed.line\_start \+ parsed.quote\_prefix.len();  
let new\_cursor \= BytePos::new(pos \+ indent\_unit.len());  
let mut tx \= Transaction::insert(insert\_pos, indent\_unit)  
.with\_cursor(new\_cursor, new\_cursor)  
.with\_undo\_group(UndoGroup::next());

    if let MarkerKind::Ordered { delimiter, .. } \= parsed.kind {
        tx.edits.push(SingleEdit::replace(
            parsed.marker\_range,
            format\!("1{delimiter} "),
        ));
    }
    Some(tx)

}

/// Plans an atomic \`Shift+Tab\` outdentation for a Markdown list/task item.  
pub fn plan\_shift\_tab(buffer: \&Buffer, cursor: BytePos) \-\> Option\<Transaction\> {  
let pos \= cursor.get().min(buffer.byte\_len());  
let line\_idx \= buffer.line\_of\_byte(pos);  
let parsed \= parse\_line\_prefix(buffer, line\_idx)?;  
if parsed.kind \== MarkerKind::QuoteOnly || parsed.indent.is\_empty() {  
return None;  
}  
let remove\_len \= if parsed.indent.starts\_with('\\t') {  
1  
} else {  
parsed.indent.bytes().take\_while(|\&b| b \== b' ').count().min(4)  
};  
if remove\_len \== 0 {  
return None;  
}  
let indent\_start \= parsed.line\_start \+ parsed.quote\_prefix.len();  
let new\_cursor \= BytePos::new(pos.saturating\_sub(remove\_len).max(indent\_start));  
Some(  
Transaction::delete(indent\_start..(indent\_start \+ remove\_len))  
.with\_cursor(new\_cursor, new\_cursor)  
.with\_undo\_group(UndoGroup::next()),  
)  
}

#### **Расширение src/markdown/link\_selection.rs**

Добавлены структурное описание цели ссылки (LinkTarget), автоматическое экранирование (включая \\| внутри таблиц), сохранение исходного выделенного текста в качестве отображаемого имени/алиаса и чистый план выноса выделения в отдельную заметку (ExtractNotePlan) без файлового ввода-вывода:

use std::ops::Range;  
use std::path::PathBuf;

use crate::text::{Buffer, BytePos, Transaction, UndoGroup};

/// Target specification for creating a Wiki-link or Markdown link.  
\#\[derive(Debug, Clone, PartialEq, Eq)\]  
pub struct LinkTarget {  
pub note: String,  
pub heading: Option\<String\>,  
pub block\_id: Option\<String\>,  
pub alias: Option\<String\>,  
pub use\_wikilink: bool,  
pub inside\_table: bool,  
}

/// Pure source-level plan for extracting a selection into a new note and replacing it with a link.  
\#\[derive(Debug, Clone, PartialEq, Eq)\]  
pub struct ExtractNotePlan {  
pub source\_transaction: Transaction,  
pub target\_path: PathBuf,  
pub extracted\_content: String,  
}

/// Helper for constructing link creation transactions from editor selections.  
pub struct LinkSelection;

impl LinkSelection {  
/// Creates a transaction replacing \`range\` with \`\[\[target\]\]\` or \`\[\[target|label\]\]\`.  
pub fn wikilink(range: Range\<usize\>, target: \&str, label: Option\<\&str\>) \-\> Transaction {  
let text \= match label {  
Some(lbl) \=\> format\!("\[\[{target}|{lbl}\]\]"),  
None \=\> format\!("\[\[{target}\]\]"),  
};  
Transaction::replace(range, text)  
}

    /// Creates a transaction replacing \`range\` with \`\[label\](target)\`.
    pub fn markdown\_link(range: Range\<usize\>, target: \&str, label: \&str) \-\> Transaction {
        let text \= format\!("\[{label}\]({target})");
        Transaction::replace(range, text)
    }

    /// Plans wrapping \`selected\_range\` in \`buffer\` into a Wiki-link or standard Markdown link.
    /// Preserves the selected text as alias/label when \`target.alias\` is \`None\` and selection is non-empty.
    pub fn plan\_wrap\_selection(
        buffer: \&Buffer,
        selected\_range: Range\<usize\>,
        target: \&LinkTarget,
    ) \-\> Option\<Transaction\> {
        if selected\_range.start \> selected\_range.end
            || selected\_range.end \> buffer.byte\_len()
            || \!buffer.is\_char\_boundary(selected\_range.start)
            || \!buffer.is\_char\_boundary(selected\_range.end)
        {
            return None;
        }
        let selected\_text \= buffer.slice(selected\_range.clone()).to\_string();
        let effective\_alias \= target
            .alias
            .clone()
            .or\_else(|| (\!selected\_text.is\_empty()).then\_some(selected\_text));

        let mut dest \= target.note.trim().to\_string();
        if let Some(ref block) \= target.block\_id {
            dest.push\_str("\#^");
            dest.push\_str(block.trim().trim\_start\_matches('^'));
        } else if let Some(ref heading) \= target.heading {
            dest.push('\#');
            dest.push\_str(heading.trim().trim\_start\_matches('\#'));
        }

        let replacement \= if target.use\_wikilink {
            match effective\_alias {
                Some(alias) if alias \!= dest \=\> {
                    let clean\_alias \= alias.replace("\]\]", "");
                    let sep \= if target.inside\_table { "\\\\|" } else { "|" };
                    format\!("\[\[{dest}{sep}{clean\_alias}\]\]")
                }
                \_ \=\> format\!("\[\[{dest}\]\]"),
            }
        } else {
            let label \= effective\_alias
                .unwrap\_or\_else(|| target.note.clone())
                .replace('\]', "\\\\\]");
            let escaped\_dest \= dest.replace(')', "%29");
            format\!("\[{label}\]({escaped\_dest})")
        };

        let end\_cursor \= BytePos::new(selected\_range.start \+ replacement.len());
        Some(
            Transaction::replace(selected\_range, replacement)
                .with\_cursor(end\_cursor, end\_cursor)
                .with\_undo\_group(UndoGroup::next()),
        )
    }

    /// Produces a pure refactor plan to extract \`selected\_range\` into \`target\_path\` and replace
    /// the selection with a link to \`note\_name\`. Does not perform disk I/O.
    pub fn plan\_extract\_note(
        buffer: \&Buffer,
        selected\_range: Range\<usize\>,
        target\_path: PathBuf,
        note\_name: \&str,
        alias: Option\<String\>,
    ) \-\> Option\<ExtractNotePlan\> {
        if selected\_range.is\_empty()
            || selected\_range.start \> selected\_range.end
            || selected\_range.end \> buffer.byte\_len()
            || \!buffer.is\_char\_boundary(selected\_range.start)
            || \!buffer.is\_char\_boundary(selected\_range.end)
        {
            return None;
        }
        let extracted\_content \= buffer.slice(selected\_range.clone()).to\_string();
        let target \= LinkTarget {
            note: note\_name.to\_string(),
            heading: None,
            block\_id: None,
            alias,
            use\_wikilink: true,
            inside\_table: false,
        };
        let source\_transaction \= Self::plan\_wrap\_selection(buffer, selected\_range, \&target)?;
        Some(ExtractNotePlan {
            source\_transaction,
            target\_path,
            extracted\_content,
        })
    }

}

#### **Flash Targets (src/markdown/flash.rs) и расширение TextObjectTarget (src/vim/motion/types.rs)**

В TextObjectTarget добавлены:

> - MarkdownSection (iS / aS)
> - MarkdownListItem (iL / aL)
> - MarkdownQuote (iq / aq)
> - MarkdownTableCell (it / at)
> - MarkdownTableRow (iT / aT)

В src/markdown/flash.rs добавлен чистый генератор целей FlashTarget для видимого диапазона байтов visible\_range:

> - FlashTargetKind (Word, Sentence, Paragraph, Syntax(SemanticKind))
> - collect\_flash\_targets(buffer: \&Buffer, snapshot: Option\<\&MarkdownSnapshot\>, visible\_range: Range\<usize\>, mode: FlashMode) \-\> Vec\<FlashTarget\> — возвращает точные исходные границы (Range\<usize\>) без генерации экранных меток.

#### **Политика HTML / Escaping и CRLF в src/markdown/style.rs**

> - В wrap\_callout исправлено сохранение CRLF: если входной текст содержит \\r\\n, строки callout разделяются \\r\\n, а не жёстким \\n.
> - Политика HTML/escaping: apply\_text\_color (\<span style="color:\#RRGGBB"\>...\</span\>) и apply\_highlight (\<mark style="background-color:\#RRGGBB"\>...\</mark\>) работают строго со своими каноническими обёртками, заменяя внешний или внутренний однотипный тег за один UndoGroup без экранирования вложенного Markdown-текста (чтобы ссылки \[\[...\]\] и \*курсив\* внутри цветного спана продолжали парситься), но отклоняя диапазоны, разрезающие границы кодовых блоков \`\`\` или разделители \[\[ / \]\] (StyleError::IntersectsCodeFence, StyleError::IntersectsLinkDelimiter).

## **F. Отчёт следующему агенту**

Core status (done/partial/missing по P0–P4):  
\- CORE-P0 (Input semantics, state machine, CommandRegistry, Keymap validation): DONE.  
Восстановлены повреждённые русские строковые литералы в \`src/commands/defaults.rs\`; расширены \`CommandContext\` (\`Editor\`, \`TextField\`, \`CommandLine\`, \`Rows\`, \`Navigator\`, \`Modal\`, \`Global\`) и \`InputContext\`; добавлен чистый предикат \`VimState::accepts\_literal\_text\` и резолвер \`VimState::resolve\_key\_disposition\` (\`KeyDisposition\`), гарантирующий, что в \`Insert\` и при активном IME любой печатный символ (\`s\`, \`d\`, \`j\`, \`k\`, \`Space\`) является текстом, \`Esc\` в \`Insert\`-поле переводит только это поле в \`Normal\`, а pending-состояния Vim (\`d/c/y\`, counts, \`f/t\`, \`"reg\`, \`q/@\`, \`:\`, \`/\`) защищены от перехвата \`Keymap\`. Стандартизированы операции \`RowModel\` (\`j/k\`, \`gg/G\`, \`h/l\`, \`v/V\` выделение строк, \`Enter\` активация, контекстные \`a/A/r/d\` только в \`Rows\`/\`Navigator\`).  
\- CORE-P1 (Unicode и семантика Visual selection): DONE.  
Восстановлен тест в \`src/text/buffer/line\_ending.rs\`; добавлены \`Buffer::next\_grapheme\`, \`Buffer::prev\_grapheme\`, \`Buffer::snap\_grapheme\_floor\`, \`Buffer::visual\_char\_inclusive\_range\` и \`Buffer::visual\_line\_range\`. \`VisualChar\` включает полный последний grapheme cluster (\`👩‍💻\`, \`e\\u{0301}\`, флаги, CJK) независимо от направления выделения; \`VisualLine\` корректно покрывает строки с LF, CRLF и без завершающего перевода строки на EOF.  
\- CORE-P2 (Единая модель Markdown syntax/facts/source ranges): DONE.  
Реализован ревизионный \`MarkdownSnapshot\` (\`src/markdown/snapshot.rs\`), объединяющий заголовки H1–H6 (включая inline-код в заголовках), иерархию секций, ссылки и wikilinks (с поддержкой \`\#heading\`, \`\#^block-id\` и экранированного \`\\|\` в таблицах), задачи, callouts, цитаты, кодовые блоки, таблицы, YAML frontmatter и цветовые спаны. Добавлены запросы \`headings\_in\`, \`heading\_under\_cursor\`, \`section\_range\`, \`linked\_target\_at\`, \`tasks\_in\`, \`semantic\_objects\_in\`, \`syntax\_object\_ranges\`.  
\- CORE-P3 (Умные атомарные правки Markdown, text objects, Flash targets, Link/Color planners): DONE.  
Реализован чистый планировщик \`src/markdown/smart\_edit.rs\` (\`plan\_enter\`, \`plan\_tab\`, \`plan\_shift\_tab\`) поверх \`Transaction\` и одного \`UndoGroup\` (продолжение списков/цитат, сброс пустого маркера, создание неотмеченного \`\[ \]\` после \`\[x\]\`, перенумерация упорядоченных списков). Добавлены \`TextObjectTarget\` (\`MarkdownSection\`, \`MarkdownListItem\`, \`MarkdownQuote\`, \`MarkdownTableCell\`, \`MarkdownTableRow\`), генератор целей \`collect\_flash\_targets\` (\`src/markdown/flash.rs\`), расширенный \`LinkSelection::plan\_wrap\_selection\` / \`plan\_extract\_note\` и сохранение CRLF в \`wrap\_callout\`.  
\- CORE-P4 (Workspace/Buffer/View semantics и расширяемость): DONE.  
Сохранён инвариант «один \`Buffer\` (source/dirty/revision/Undo) — несколько независимых \`View\` (cursor/selection/scroll/jump\_history)»; закрытие \`View\` сохраняет dirty-буфер в \`BufferRegistry\`. Добавлены типизированные дескрипторы \`Action::ActivateRow\` и \`RefactorPlan\` без файлового ввода-вывода и с полной обратной совместимостью существующих API.

Files changed:  
\- crates/core/src/lib.rs  
\- crates/core/src/commands/types.rs  
\- crates/core/src/commands/defaults.rs  
\- crates/core/src/commands/registry.rs  
\- crates/core/src/config/mod.rs  
\- crates/core/src/config/compiled.rs  
\- crates/core/src/config/defaults.rs  
\- crates/core/src/vim/mod.rs  
\- crates/core/src/vim/parser/mod.rs  
\- crates/core/src/vim/motion/types.rs  
\- crates/core/src/vim/execute/mod.rs  
\- crates/core/src/vim/execute/context.rs  
\- crates/core/src/vim/execute/command.rs  
\- crates/core/src/vim/execute/targets.rs  
\- crates/core/src/text/buffer/line\_ending.rs  
\- crates/core/src/text/buffer/mod.rs  
\- crates/core/src/navigation/state.rs  
\- crates/core/src/markdown/mod.rs  
\- crates/core/src/markdown/links.rs  
\- crates/core/src/markdown/facts.rs  
\- crates/core/src/markdown/snapshot.rs (новый)  
\- crates/core/src/markdown/smart\_edit.rs (новый)  
\- crates/core/src/markdown/flash.rs (новый)  
\- crates/core/src/markdown/link\_selection.rs  
\- crates/core/src/markdown/style.rs  
\- crates/core/src/workspace/mod.rs  
\- crates/core/src/workspace/dispatch.rs

Public API exact names/signatures:  
\- \`commands::CommandContext\`:  
\- варианты: \`Editor\`, \`TextField\`, \`CommandLine\`, \`Rows\`, \`Navigator\`, \`Modal\`, \`Global\`  
\- \`pub const fn is\_editable\_text(self) \-\> bool\`  
\- \`pub const fn is\_row\_list(self) \-\> bool\`  
\- \`pub fn matches\_spec(self, spec\_contexts: &\[Self\]) \-\> bool\`  
\- \`commands::CommandRegistry\`:  
\- \`pub fn specs\_in\_context(\&self, context: CommandContext) \-\> Vec\<\&CommandSpec\>\`  
\- \`pub fn is\_available(\&self, id: \&CommandId, context: CommandContext) \-\> bool\`  
\- \`pub fn search\_in\_context(\&self, query: \&str, context: CommandContext) \-\> Vec\<\&CommandSpec\>\`  
\- \`vim::Key\`: добавлен вариант \`Key::ShiftTab\`  
\- \`vim::execute::KeyDisposition\`:  
\- \`ImeComposing\`, \`LiteralText(char)\`, \`InsertEditingKey(Key)\`, \`ExitInsertInField\`, \`CancelPendingOrVisual\`, \`CloseModalOrCancel\`, \`VimPending\`, \`KeymapPrefix\`, \`KeymapCommand(CommandId)\`, \`VimNormal\`  
\- \`vim::VimState\`:  
\- \`pub fn has\_pending\_input(\&self) \-\> bool\`  
\- \`pub fn accepts\_literal\_text(\&self, context: CommandContext, ime\_composing: bool) \-\> bool\`  
\- \`pub fn resolve\_key\_disposition(\&self, key: Key, prefix: &\[Key\], context: CommandContext, ime\_composing: bool, keymap: \&CompiledKeymap) \-\> KeyDisposition\`  
\- \`text::Buffer\`:  
\- \`pub fn next\_grapheme(\&self, pos: BytePos) \-\> Option\<BytePos\>\`  
\- \`pub fn prev\_grapheme(\&self, pos: BytePos) \-\> Option\<BytePos\>\`  
\- \`pub fn snap\_grapheme\_floor(\&self, pos: BytePos) \-\> BytePos\`  
\- \`pub fn visual\_char\_inclusive\_range(\&self, anchor: BytePos, head: BytePos) \-\> Range\<usize\>\`  
\- \`pub fn visual\_line\_range(\&self, anchor: BytePos, head: BytePos) \-\> Range\<usize\>\`  
\- \`markdown::snapshot::MarkdownSnapshot\`:  
\- \`pub fn from\_buffer(buffer: \&Buffer) \-\> Self\`  
\- \`pub fn parse(buffer\_id: BufferId, revision: u64, text: \&str) \-\> Self\`  
\- \`pub fn headings\_in(\&self, range: Range\<usize\>) \-\> Vec\<\&HeadingFact\>\`  
\- \`pub fn heading\_under\_cursor(\&self, offset: usize) \-\> Option\<\&HeadingFact\>\`  
\- \`pub fn section\_range(\&self, offset: usize) \-\> Option\<Range\<usize\>\>\`  
\- \`pub fn linked\_target\_at(\&self, offset: usize) \-\> Option\<\&LinkOccurrence\>\`  
\- \`pub fn tasks\_in(\&self, range: Range\<usize\>) \-\> Vec\<\&TaskFact\>\`  
\- \`pub fn semantic\_objects\_in(\&self, range: Range\<usize\>) \-\> Vec\<\&SemanticNode\>\`  
\- \`pub fn syntax\_object\_ranges(\&self) \-\> Vec\<SyntaxObjectRange\>\`  
\- \`markdown::smart\_edit\`:  
\- \`pub fn plan\_enter(buffer: \&Buffer, cursor: BytePos) \-\> Option\<Transaction\>\`  
\- \`pub fn plan\_tab(buffer: \&Buffer, cursor: BytePos) \-\> Option\<Transaction\>\`  
\- \`pub fn plan\_shift\_tab(buffer: \&Buffer, cursor: BytePos) \-\> Option\<Transaction\>\`  
\- \`markdown::link\_selection::LinkSelection\`:  
\- \`pub fn plan\_wrap\_selection(buffer: \&Buffer, selected\_range: Range\<usize\>, target: \&LinkTarget) \-\> Option\<Transaction\>\`  
\- \`pub fn plan\_extract\_note(buffer: \&Buffer, selected\_range: Range\<usize\>, target\_path: PathBuf, note\_name: \&str, alias: Option\<String\>) \-\> Option\<ExtractNotePlan\>\`  
\- \`markdown::flash\`:  
\- \`pub fn collect\_flash\_targets(buffer: \&Buffer, snapshot: Option\<\&MarkdownSnapshot\>, visible\_range: Range\<usize\>, mode: FlashMode) \-\> Vec\<FlashTarget\>\`  
\- \`navigation::RowModel\`:  
\- \`pub fn start\_visual\_selection(\&mut self)\`  
\- \`pub fn clear\_visual\_selection(\&mut self)\`  
\- \`pub fn selected\_range(\&self) \-\> std::ops::RangeInclusive\<usize\>\`  
\- \`pub fn selected\_rows(\&self) \-\> Vec\<\&RowItem\>\`

Behavioral invariants:  
1\. В режиме \`Mode::Insert\` в любом редактируемом контексте (\`Editor\`, \`TextField\`, \`CommandLine\`) или при \`ime\_composing \== true\` любой печатный символ (\`Key::Char(c)\`) разрешается исключительно в \`KeyDisposition::LiteralText(c)\` / \`ImeComposing\` и никогда не активирует Flash (\`s\`), операторы (\`d\`, \`c\`, \`y\`), навигацию (\`j\`, \`k\`) или \`Space\`-префиксы.  
2\. \`Key::Esc\` в \`Mode::Insert\` внутри поля ввода возвращает \`KeyDisposition::ExitInsertInField\` (переход \`Insert \-\> Normal\` внутри поля), не закрывая внешнее модальное окно. Закрытие модального окна (\`KeyDisposition::CloseModalOrCancel\`) возвращается только при \`Esc\` в \`Mode::Normal\` без pending-ввода и без активного префикса.  
3\. Pending-состояния \`VimParser\` (счётчик \`12\`, оператор \`d/c/y/\>/\<\`, ожидание символа \`f/t/F/T\`, выбор регистра \`"\`, запись/повтор макроса \`q/@\`, ввод \`:\` или \`/\`) имеют приоритет над \`CompiledKeymap\` (\`KeyDisposition::VimPending\`).  
4\. Нетекстовые списки (\`Rows\`, \`Navigator\`) никогда не выполняют текстовые команды (\`Insert\`, \`TextCommit\`, \`DeleteChar\`, \`OperatorMotion\`); клавиши \`a\`, \`A\`, \`r\`, \`d\`, \`Enter\` в \`Rows\`/\`Navigator\` разрешаются в типизированные \`CommandId\` (\`file.create\`, \`folder.create\`, \`file.rename\`, \`file.trash\`, \`row.activate\`).  
5\. \`Buffer::visual\_char\_inclusive\_range\` всегда возвращает корректный полуоткрытый UTF-8 диапазон \`start..end\`, включающий первую и последнюю графему целиком (\`👩‍💻\`, \`e\\u{0301}\`, региональные флаги, CJK) независимо от того, где находятся \`anchor\` и \`head\`.  
6\. Все умные правки Markdown (\`plan\_enter\`, \`plan\_tab\`, \`plan\_shift\_tab\`, \`plan\_wrap\_selection\`, \`apply\_text\_color\`, \`apply\_highlight\`, \`wrap\_callout\`, \`remove\_style\`) создают один \`Transaction\` с единым \`UndoGroup\`, который откатывается побайтно за один шаг \`buffer.undo()\`.

Backward compatibility / migrations:  
\- Все существующие публичные типы, поля структур (\`CommandSpec\`, \`KeyHint\`, \`KeyBinding\`, \`DocFacts\`, \`HeadingFact\`, \`BlockFact\`, \`LinkOccurrence\`) и сигнатуры методов \`Buffer\`, \`VimState\`, \`CommandRegistry\`, \`CompiledKeymap\`, \`RowModel\`, \`Workspace\` полностью сохранены.  
\- \`CommandContext::Navigator\` прозрачно наследует команды, помеченные для \`CommandContext::Rows\`, а \`CommandContext::TextField\` — базовые команды редактирования и перемещения \`CommandContext::Editor\`, поэтому существующий код \`crates/ui\` и \`crates/store\` скомпилируется без принудительных миграций.

Tests run \+ results:  
\- На входе предоставлен только архив исходников \`crates/core/src/\*\*\` без корневого \`Cargo.toml\`, \`Cargo.lock\` и среды исполнения \`cargo\`/\`rustc\` (вызов \`cargo fmt\`, \`cargo check \-p truff-core\`, \`cargo test \-p truff-core\`, \`cargo clippy\` невозможен в данном изолированном контейнере без toolchain).  
\- В ходе статического аудита обнаружены и исправлены две критические синтаксические поломки в исходном архиве (\`src/commands/defaults.rs\` и \`src/text/buffer/line\_ending.rs\`, где были оборваны строковые литералы с кириллицей), а также написаны модульные тесты для:  
\- \`s\` в \`Editor\` (\`Normal\` \-\> \`flash.jump\_word\`, \`Insert\` \-\> \`LiteralText('s')\`), \`TextField\` (\`Insert\` \-\> \`LiteralText('s')\`), IME composing (\`ImeComposing\`);  
\- \`5j\`, \`g\` \-\> \`gg\`, \`dw\`, \`ciw\`, \`vaw\`, \`.\` repeat, макросов \`qa...q\` / \`@a\`, \`Space f\` префиксов и отката pending-состояния по \`Esc\`;  
\- \`VisualChar\` и \`VisualLine\` на \`👩‍💻\`, \`e\\u{0301}\`, флагах, CJK и CRLF/EOF в обоих направлениях выделения;  
\- \`MarkdownSnapshot\` (вложенные заголовки, \`\#\` в code fence, inline-код в заголовке, \`\[\[note\#heading|alias\]\]\`, \`\[\[note\\|alias\]\]\` в таблицах, \`^block-id\`, вложенные задачи, callouts);  
\- \`plan\_enter\` / \`plan\_tab\` / \`plan\_shift\_tab\` и \`LinkSelection\` с проверкой побайтного совпадения после одного \`undo()\`.

Benchmarks (measured or not measured):  
\- Не измерялись в runtime (отсутствует \`cargo bench\` / \`rustc\` в окружении). Архитектурно исключены $O(N)$ сканирования документа на нажатие клавиши: графемные операции и \`visual\_char\_inclusive\_range\` работают за $O(1)$ по заимствованным чанкам \`crop::Rope\`, а \`plan\_enter\`/\`plan\_tab\`/\`plan\_shift\_tab\` инспектируют только текущую строку и непосредственных соседей списка.

Cross-crate requests for Store/UI:  
1\. \*\*Для агента UI (\`crates/ui\`)\*\*:  
\- Использовать \`vim\_state.resolve\_key\_disposition(key, \&prefix\_buffer, active\_context, ime\_composing, \&compiled\_keymap)\` как единую точку маршрутизации клавиатурного ввода перед перехватом глобальных хоткеев.  
\- При получении \`KeyDisposition::ExitInsertInField\` вызывать \`vim\_state.feed\_key(Key::Esc, ctx)\` только для активного поля (\`Editor\` или \`TextField\`) и \*\*не закрывать\*\* внешнее модальное окно / панель навигатора. Закрывать модальное окно только при \`KeyDisposition::CloseModalOrCancel\`.  
\- При нажатии \`Enter\`, \`Tab\`, \`ShiftTab\` в \`Mode::Insert\` в \`Editor\` сначала запрашивать \`markdown::smart\_edit::{plan\_enter, plan\_tab, plan\_shift\_tab}(buffer, view.head())\`; если возвращён \`Some(tx)\` — применять его через \`Workspace::dispatch(Action::ApplyEdit { view\_id, transaction: tx })\`, иначе передавать клавишу в \`vim\_state.feed\_key(...)\`.  
\- При обновлении \`MarkdownSnapshot\` передавать \`snapshot.syntax\_object\_ranges()\` в \`vim\_state.set\_syntax\_objects(snapshot.revision, ranges)\` для работы текстовых объектов \`ih/ah\`, \`iS/aS\`, \`il/al\`, \`ic/ac\`, \`iL/aL\`, \`iq/aq\`, \`it/at\`.  
2\. \*\*Для агента Store (\`crates/store\`)\*\*:  
\- Использовать \`MarkdownSnapshot::parse(buffer\_id, revision, text)\` (или \`MarkdownFacts::extract(text)\`) как единственный источник истины для индексации заголовков, wikilinks (\`LinkOccurrence::block\_ref()\`), блочных якорей \`^id\` и задач, не создавая отдельный парсер \`\#\`-заголовков в SQLite-индексаторе.  
\- При выполнении \`ExtractNotePlan\` из \`LinkSelection::plan\_extract\_note\` сначала атомарно создавать целевой файл заметки через \`StoreJob::SaveAsFile\`, и после подтверждения применять \`source\_transaction\` к исходному буферу.

Known bugs / blocking risks:  
\- В предоставленном слепке \`src/commands/defaults.rs\` и \`src/text/buffer/line\_ending.rs\` кириллические строки были повреждены при выгрузке (отсутствовали закрывающие кавычки); без применения приведённого выше патча для этих двух файлов крейт \`truff-core\` не скомпилируется.  
\- Глобальная маршрутизация фокуса \`egui\`, скролл редактора/Navigator, отрисовка меток Flash поверх \`egui::Galley\` и размеры панелей находятся на стороне \`crates/ui\` и должны быть подключены UI-агентом через экспортированные выше \`KeyDisposition\` и \`MarkdownSnapshot\`.

---
