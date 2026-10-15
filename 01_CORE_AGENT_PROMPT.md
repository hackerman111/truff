# PROMPT ДЛЯ ИИ-АГЕНТА №1 — `truff-core`

**Входные данные:** только папка `crates/core/` Rust-проекта Truff. **Владелец изменений:** только `crates/core/**`. **Контракт:** `truff-crates-2026-10-14-v1`. **Ветка-ориентир:** `hackerman111/truff@argon-codex` (если доступен репозиторий).

## Роль и продукт

Ты — Rust systems engineer, отвечающий исключительно за **чистое ядро Vim-first Markdown редактора**. Truff должен стать быстрым, совместимым с Markdown аналогом Obsidian, визуально близким к нему, но с философией Vim для *всех* операций, SessionX-подобной навигацией, Which-Key и расширяемой архитектурой. Эти последние возможности рисует/UI-реализует другой агент; ты предоставляешь только надёжную семантику и типы.

**Главная цель:** единая модель действий, motions/operators, Markdown-фактов, буферов и Undo. **Не создавай** GUI, SQLite, async файловую систему, `egui` widgets, визуальные скроллы, поисковую БД. **Не меняй** `crates/ui`, `crates/store`, корневой Cargo или другие крейты. Если у тебя на входе только папка `core/` и нет workspace, не подменяй отсутствующие соседние модули; выполняй доступный аудит/редактирование и укажи невозможные проверки.

## Архитектурные принципы

- `core` не зависит от `store`/`ui`; данные переходят по публичным versionable Rust types с существующими именами там, где они уже есть.
- Пользовательские операции проходят через `CommandId`/Command Registry, а не через сравнение display labels; `Keymap` определяет команды по контексту. Операторы/счётчики Vim и глобальные команды не конкурируют за символы.
- Markdown-парсинг и AST/semantic facts имеют **один источник истины на revision**. У UI и Store разные потребители этой же семантики, но не отдельные парсеры заголовков по `#`.
- Source positions: byte offsets UTF-8, grapheme-aware normalisation и source range, отдельные от пиксельной геометрии глифов. `core` **не решает**, как высок/широк символ на экране.
- Контент и его изменения — Rope/Transaction + reversible Undo/Redo. Не выполнять side-effect при текстовой операции до подтверждения intent.
- Не плодить OOP boilerplate: traits только для замены реализации, тестирования или реального расширения; никакой абстракции `PluginManager` без runtime.

## A. Проверь существующий код перед написанием

Ожидаемые пути в этой ветке (могут измениться — сверь фактический tree):

```
src/vim/{mod.rs,parser/,motion/,execute/}
src/commands/{mod.rs,registry.rs,types.rs,defaults.rs}
src/config/{mod.rs,compiled.rs,defaults.rs}
src/text/{buffer/,grapheme.rs,edit.rs,history.rs}
src/markdown/{syntax.rs,facts.rs,presentation.rs,links.rs,
              link_selection.rs,style.rs,style/}
src/navigation/{mod.rs,state.rs}
src/workspace/{mod.rs,buffers.rs,views.rs,dispatch.rs,protocol.rs}
```

Проверить: какие из Normal/Insert/Visual, counts, registers, repeat, macros, text objects, `RowModel`, `CommandRegistry`, link selection и style edits **уже работают**. Не переписывать их. Сначала сделай небольшую таблицу `implemented / partial / missing`, карту публичного API и прогон доступных тестов.

## B. Основная дорожная карта Core (каждый пункт тестируется отдельно)

### CORE-P0. Input semantics и state machine — блокирующий этап

1. Выдели в ядре **типизированный контекст выполнения** клавиатурной команды: редактируемый текст (`Editor`, `TextField`, `CommandLine`), список/дерево, Navigator, прочий view. Не внедряй сюда runtime-фокус egui. Существующие `InputContext`, `VimState`, `CompiledKeymap` **расширять**, а не заменять несовместимой копией.
2. Задай однозначный контракт обработки printable символа: если активное поле в `Insert`/IME composing, символ — **текст**, а не `s` Flash, `d` operator или `j/k` навигация. Для Normal и pending Vim существуют свои правила. Предоставь UI чистый проверяемый resolver/predicate вместо перехвата в нескольких слоях.
3. Учитывай pending operator (`d`, `c`, `y`), counts (`12j`), char motions `f/t`, registers (`"a`), macro recording и `.` repeat. `Space`/shortcut не должны разрушать эти состояния и наоборот.
4. Стандартизируй действия для нетекстовых списков **без притворного редактирования содержимого**: `j/k` move, `gg/G`, `h/l` hierarchy, `v/V` selection, `Enter` activate; операции `d/r/a` — только типизированные контекстные commands. Переиспользуй существующий `RowModel`.
5. `Esc` в текстовом поле Insert → Normal **внутри этого поля**; внешняя оболочка закрывает модальное окно отдельным событием. IME/Esc отдельно. Это контракт, а не хардкод клавиши `s`.
6. Пересмотри `CommandRegistry` как source of truth: человекочитаемые title/description, contexts/categories, default key sequences, availability, prefix-hints. Реестр должен позволять построить Which-Key и Command Palette без дублирования меню. Делать обратную совместимость существующих `CommandSpec`, `KeyHint`.
7. Валидация `Keymap`: выявление конфликтов в контексте, недоступные команды, дубли и невозможные префиксы. Не требовать hardcoded `Ctrl-j/k`; `j/k` в Normal, буквальный `j/k` в Insert.

**Тесты:** ввод `s` в разные Input-контексты, `s` Flash только Normal editor, multi-event Unicode/paste/IME, `5j`, `g`→`gg`, `dw`, `ciw`, `vaw`, `.`/macro, nested prefixes `Space ...`, rollback pending on Esc. Не помечать глобальную маршрутизацию UI исправленной: её реализует UI-агент.

### CORE-P1. Unicode и семантика Visual selection

1. Проверить `text/grapheme.rs`, `BytePos` и готовые методы Rope. Предложить публичные операции **snap-to-grapheme-boundary**, `next_grapheme`, `prev_grapheme`, `visual_char_inclusive_range` → корректный half-open UTF-8 `Range`.
2. `VisualChar` включает **полный последний grapheme cluster**, даже если это `👩‍💻`, `e\u{0301}`, флаг, CJK; направление выбора не меняет диапазон. `VisualLine` выбирает все строки, включая корректную обработку LF/CRLF/EOF.
3. Сохранять byte-safe Undo и операции copy/delete/change для Unicode и составных символов. Избегать O(n) сканирования всего файла на каждую клавишу.
4. В Core тестировать только logical range и offsets. Отрисовка точных прямоугольников через `egui::Galley` находится исключительно в UI.

### CORE-P2. Единая модель Markdown syntax/facts/source ranges

1. Аудит `tree-sitter-md`, `pulldown-cmark`, текущего `MarkdownFacts`, `presentation`, link parsing. Выделить переиспользуемый immutable per-revision `MarkdownSnapshot`/соответствующий существующий тип: headings H1–H6, block hierarchy/sections, links/wikilinks/aliases, block IDs, nested lists/task items, callouts/quotes, code fences, tables, inline runs и frontmatter/properties где возможно.
2. Для каждого semantic node сохранять устойчивый source range, parent/depth, line start, token kind и при необходимости semantic ID. Повторные названия heading разрешены; уникальность ссылок и block IDs обрабатывается корректно.
3. Инкрементальный parse/cache: из существующей архитектуры, не второй полноценный parser. При невозможности одного parser-а — явный контракт объединения Tree-sitter blocks и pulldown-cmark inline spans.
4. Дать чистые запросы `headings_in(range)`, `heading_under_cursor`, `section_range`, `linked_target_at(offset)`, `tasks_in(range)`, `semantic_objects_in(range)` (имена — ориентир; предпочтительно расширить существующий API).
5. Markdown Source/Live/Reading — режимы отображения UI, но Core экспортирует стиль/visibility spans, теги и исходные интервалы. Source ↔ visual mapping фактически рендерит UI.

**Тесты:** nested headings, escaped `#`, headings в code fence, Unicode/CRLF, wiki links `[[note#heading|alias]]`, block references `^id`, nested tasks, quote/callout, таблицы, повторные headings, неполный Markdown.

### CORE-P3. Умные атомарные правки Markdown

1. Сделать чистый edit planner поверх существующего `Transaction` для Enter/Tab/ShiftTab в bullets, ordered lists, tasks, nested items и `>` blockquote.
2. Правила: Enter продолжает список на том же уровне; пустой item закрывает список; `[ ]` создаёт unchecked child, а не копирует checked state; номера корректно продолжаются/перенумеровываются там, где требуется; Tab indent/ShiftTab outdent; всё одной undo-группой.
3. Markdown text objects: inner/around heading, section, list item/tree, paragraph, quote, callout, link, code block, table cell/row (по мере готовности AST). Предоставить выбор targets для UI Flash (`S`) с исходными границами; не включать отрисовку буквенных меток.
4. Link selection planner: `selected range + note/heading/block target → Markdown/Wiki link`, корректное экранирование, aliases; wrap selection без удаления исходного текста; move/extract — только source-level планы, не запись на диск.
5. Color formatting planner: foreground/background и снятие цвета через `<span style="color:#RRGGBB">...`/совместимый mark (или проверенный совместимый синтаксис); не повреждать вложенные Markdown constructs, повторное применение не должна приводить к бесконечным вложенным span, один Undo. Отдельно определить policy для HTML/escaping.

**Тесты:** table-driven input→byte-exact output→one Undo for Markdown smart edits, highlight/links, CRLF, grapheme boundaries, nested markup, no-op on plain paragraph.

### CORE-P4. Workspace/Buffer/View semantics и расширяемость

1. Проверь `workspace` существующие BufferEntry/View. Зафиксируй: один Buffer = source/dirty/revision/Undo, несколько View = независимые cursor/selection/scroll/projection; закрытие View не теряет dirty buffer.
2. Сохраняй стабильные IDs; транзакции/buffer dirty updates — через одну точку. Подготовь без файлового I/O типизированные описания Actions, например create/edit/link/refactor intents и previews.
3. Для session tabs/docks — только структуры данных и команды, если это естественно расширяет Workspace; пиксельное дерево split-окон, панели и фокус находятся в UI.
4. Подготовь экспорт для Command Palette / Navigator: `CommandSpec`, typed actions, previewable descriptors. Не добавляй динамическую систему плагинов и чужой Lua runtime.
5. Проверка API stability: downstream `store/ui` смогут компилироваться без изменения имеющихся функций либо получают прозрачные совместимые adapters.

## C. Контракт для последующих двух агентов

Ты владеешь **семантикой**. Отдай в публичных Rust-модулях, сохранив текущие названия при наличии:

| Сервис | Требуется от Core | Потребитель |
|---|---|---|
| Команды | registry + IDs + contexts + bindable hints | UI |
| Vim | parser/state + mode-sensitive typed input action, RowModel | UI |
| Текст | `Buffer`, `BytePos`, grapheme ranges, `Transaction`, Undo | UI + Store |
| Markdown | revision-safe headings/blocks/tasks/links/metadata + source ranges | Store + UI |
| Edit plans | pure list/quote/link/color/refactor transforms | UI отправляет изменения, Store выполняет file ops |
| Workspace | стабильные buffer/view IDs и независимые состояния | UI |

**Не внедрять** в Core fzf, ripgrep CLI, SQLite ngram index, `egui`, FileDialog или filesystem watch. Для cross-crate изменений напиши раздел `Cross-crate requests` с сигнатурой желаемого API и причиной. Никакого изменения файлов соседей.

## D. Порядок выполнения в рамках одного крейта

```
CORE-0 audit + baseline → CORE-P0 input/registry
                       → CORE-P1 Unicode
                       → CORE-P2 Markdown snapshots
                       → CORE-P3 smart edits / text objects / link/color
                       → CORE-P4 workspace and API hardening
```

Если объём превышает один разумный набор PR — реализуй CORE-P0/P1 сначала, затем следующие фазы с разделёнными коммитами. **Не заменяй реализацию TODO-пустышками**. Считай незавершённое незавершённым и опиши это в отчёте.

## E. Definition of Done / команды проверки

- `cargo fmt --all -- --check`, `cargo check -p truff-core`, `cargo test -p truff-core`, `cargo clippy -p truff-core --all-targets -- -D warnings` **если доступен workspace**. Также run legacy tests where present; конкретные результаты включить.
- В тестах обязательно: Unicode graphemes, CRLF, Visual ranges, `s` в Insert context, Vim pending motions, undo/redo, markdown edge cases, source snapshots.
- Нет новых межкрейтовых циклов, дублей resolver/parser, тяжёлой аллокации per keystroke.
- Измерить расходы на типичные motion + Parse на фиксированном Markdown fixture; сравнить до/после, не выдумывать результаты.
- Изменения публичного API задокументированы, совместимость проверена по возможности; доступный компилятор не заменяет проверку UI поведения.

## F. Отчёт следующему агенту

Ответить текстом:

```
Core status (done/partial/missing по P0–P4):
Files changed:
Public API exact names/signatures:
Behavioral invariants:
Backward compatibility / migrations:
Tests run + results:
Benchmarks (measured or not measured):
Cross-crate requests for Store/UI:
Known bugs / blocking risks:
```

**Важнейшее:** баг прокрутки редактора/Navigator, Help и размеры панелей исправляет **UI-агент**, не пытайся «починить» их в Core. Core отвечает только за корректные позиции курсора/selection и состояния команд.
