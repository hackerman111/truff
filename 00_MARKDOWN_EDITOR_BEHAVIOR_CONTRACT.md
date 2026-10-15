# Truff — Markdown Editor Behavior Contract (MEBC-1)
**Цель:** единый, тестируемый контракт поведения Markdown-редактора в Truff.  
**Репозиторий:** `hackerman111/truff`, ветка `argon-codex` (сверить SHA перед работой).  
**Статус:** specification / implementation backlog, **не** утверждение, что функции уже реализованы.  
**Зона ответственности:** логика редактора, рендеринг, события и интеграция с уже существующим `truff-core`; не Kanban/Graph/Vault UI.

## 0. Принципы, которые запрещено нарушать

1. **Markdown-файл является источником истины.** Markdown хранится как обычный текст в `Buffer`/Rope. Визуальные режимы не изменяют исходник; ни один режим не создаёт собственную копию документа для редактирования.
2. **Vim-first, без мыши:** всё редактирование, переходы, ссылки, выделение, форматирование, прокрутка и смена режима доступны по клавиатуре. Мышь — дополнительный путь к той же команде.
3. **Один поток команд:** события ОС → проверка активного фокуса/IME/диалога → режим ввода → Vim/CommandRegistry → транзакция → ChangeSet → индекс/рендер. Никаких параллельных обработчиков `s`, `/`, `j`, `k` в нескольких местах.
4. **Редактирование — атомарные транзакции.** Одна семантическая операция = одна единица undo; redo восстанавливает ровно те же байты. Внешняя кодировка строк LF/CRLF сохраняется.
5. **Source offset ↔ visual coordinate:** все курсоры, диапазоны ссылок, выделения, правки и диагностика хранятся в байтовых диапазонах исходника; `DisplayMap` отвечает за визуальную проекцию. Нельзя смешивать byte, Unicode scalar, grapheme и колонку глифов.
6. **Большие свободные поверхности:** центральный Markdown-редактор занимает максимум доступной площади; читаемая колонка на широком мониторе, но без узкого принудительного окна. Связанные селекторы и Help полноразмерные/широкие, единая тема Obsidian-like.
7. **Сохранность:** автоматическое форматирование не переписывает чужие строки, не сортирует Markdown и не нормализует весь файл.

## 1. Модель состояния

Предлагаемый контракт (не требование вводить именно такую структуру):

```text
EditorState {
  buffer_id, buffer_revision, view_id,
  vim_mode: Normal | Insert | VisualChar | VisualLine | OperatorPending | Command,
  projection: Source | LivePreview | Reading,
  anchor: SourceBytePos, head: SourceBytePos,
  scroll_anchor: (source_block, local_pixel_offset),
  preferred_visual_column,
  ime_state, completion_state, pending_operator,
  focused_surface: Editor | TextField | Navigator | Help | Dialog | Other
}
```

**Главный инвариант:** в любой момент активна одна поверхность — получатель клавиатуры. `Insert` в поле поиска/имени папки **не** является `Insert` центрального редактора. Возврат из Navigator/Help восстанавливает фокус и нужный Vim-режим, не перемещая caret неожиданно.

**Разрешение конфликта клавиш:**
1. Открытый диалог или IME обрабатывает зарезервированные события.
2. Сфокусированное поле ввода обрабатывает текст; символы не идут в Flash/Vim.
3. Навигатор/Help получает свои клавиши.
4. Только сфокусированный редактор получает Vim-мotions.
5. Команды приложения выполняются через registry; подсказки Which-Key получаются из него же.

Запрет: при вводе имени файла/папки `s` не активирует Flash. Запрет: при открытом поиске `j/k` не двигают курсор в документе.

## 2. Режимы редактирования (нормативное поведение)

| Состояние | Ввод символов | `Esc` | `j/k/h/l` | `Enter` |
|---|---|---|---|---|
| Editor Normal | Vim-команды, не печатать | снять префикс/выделение | motions | контекстная команда, НЕ автоматическое изменение текста |
| Editor Insert | печатать | завершить Insert и IME | печатать буквы | smart newline |
| Editor VisualChar | расширение выделения | Normal, снятие выделения | расширять range | только явно назначенная команда |
| Editor VisualLine | диапазон логических строк | Normal | расширять по строкам | только явно назначенная команда |
| Operator-pending | аргумент motion/text-object | отменить оператор | область операции | не подменять Enter |
| Dialog/Search Insert | печатать, не Vim | закрыть/перейти в Normal поля | печатать | подтвердить |
| Navigator/Help Normal | контекстные команды | закрыть/вернуться | переместить выбор/скролл | активировать пункт |
| Reading | не редактировать | закрыть режим/вернуться | прокручивать/перемещать фокус | перейти по выбранной ссылке |

Должны работать базовые Vim-механизмы: `i/a/I/A/o/O`, `h/j/k/l`, `w/b/e`, `0/$`, `gg/G`, `f/t/F/T`, `;/,`, `%` там, где применимо, `d/c/y`, `p/P`, `u`, `<C-r>`, `.`, counts, registers, `v/V`, поиск `/ ? n N`, `* #`, `zz/zt/zb`, `<C-d>/<C-u>` без требования `<C-j>/<C-k>`. Поведение должно соответствовать уже поддерживаемому ядру; отсутствующее — отдельно договорённый backlog `core`, а не дублирование в UI.

**Строки с visual wrapping:** `j/k` перемещаются по логическим строкам Vim по умолчанию; `gj/gk` по визуальным строкам (если core поддерживает, иначе UI даёт контракт-запрос). При вертикальном движении сохранять `preferred_visual_column`, зажимая на короткой строке. Свёрнутые разделы обрабатываются как видимые строки, но source остаётся неизменным.

## 3. Smart Markdown editing — точные преобразования

**Преусловие:** редактор сфокусирован, `Insert`, нет незавершённой IME-композиции; caret стоит в указанном месте (`|`); операция происходит в рамках одной транзакции. Семантика определяется AST Markdown, а не только регулярным выражением. По умолчанию smart behavior включено и отключаемо в настройках.

### 3.1. `Enter` — список / задача / нумерация

| Исходник до Enter | Результат (`|` — caret) |
|---|---|
| `- one|` | `- one\n- |` |
| `* one|` | `* one\n* |` |
| `+ one|` | `+ one\n+ |` |
| `  - nested|` | `  - nested\n  - |` |
| `- [ ] todo|` | `- [ ] todo\n- [ ] |` |
| `- [x] done|` | `- [x] done\n- [ ] |` |
| `1. first|` | `1. first\n2. |` |
| `7) later|` | `7) later\n8) |` |
| `> quote|` | `> quote\n> |` |
| `- |` | `|` на следующей обычной строке — выход из списка |
| `> |` | `|` на следующей обычной строке — выход из цитаты |
| `plain|` | `plain\n|` |
| ```` ```rust\nlet a = 1;|\n``` ```` | обычный `\n`, без списка или автозакрытия fence |
| `---\ntitle: x|` в YAML | обычный `\n`, без логики списков (кроме YAML-специфичной, по отдельному запросу) |

Для **вложенного пустого пункта** однократный Enter снимает текущий маркер и поднимает вложенность на один уровень; повторный Enter на оставшемся пустом уровне завершает список. Вложенные списки с mixed tabs/spaces и разным bullet style требуют сохранения исходного синтаксиса родителя.

Enter **посреди строки** делит содержание в точке caret и вставляет соответствующий маркер в новую строку: `- ab|cd` → `- ab\n- |cd`. Проверять, что выделенный текст при Enter заменяется корректно.

Список в blockquote продолжает обе составляющие: `> - item|` → `> - item\n> - |`. Не порождать `-` в fenced code, inline code, math fence, HTML block. Не считать строку `-` внутри таблицы пунктом списка.

**Numbering:** автоматически увеличивается число **только у нового пункта**, не производится массовая перенумерация документа. Вариант «перенумеровать весь список» — отдельная явная команда.

### 3.2. `Shift+Enter`, `Tab`, `Shift+Tab`, `Backspace`

- `Shift+Enter` — **буквальный перевод строки без smart continuation**; не порождает новый list/task/quote marker. Должен поддерживаться драйвером событий. Не вставлять два пробела или `<br>` неявно.
- `Tab` на пункте списка — увеличить уровень вложенности текущего пункта **с дочерним поддеревом** (одно undo); `Shift+Tab` — уменьшить. Отступ определяется настройкой (default: 2 пробела), но стиль исходника сохранять, когда это возможно.
- `Tab` в fenced code вставляет заданный отступ. В обычном тексте Tab подчиняется конфигу insert/spaces. Если открыт completion, `Tab` принимает кандидат **только по явной настройке**; иначе выполняет обычный Tab.
- `Backspace` сразу после пустого маркера удаляет marker/снимает уровень вложенности, не склеивая с предыдущим пунктом неожиданно; в середине обычного слова стирает предыдущий полный grapheme.
- `Delete` удаляет следующий полный grapheme; парные маркеры не стирать автоматически без отдельной настройки.
- `Ctrl+Enter` в пункте задачи — переключить `[ ]` ↔ `[x]`; если нет task item, не менять текст. Для задач можно выбрать эту клавишу как настройку по умолчанию, подчинённую общему registry.

**Повтор `.`, макросы и undo:** SmartEnter, smart indent, toggle task воспроизводятся через семантический action, а не симуляцию отдельных синтетических нажатий. Одна операция, одно undo.

### 3.3. Автопары и операции форматирования

- MVP: команды выделения `bold`, `italic`, `strike`, `inline code`, `highlight`, `quote`, `heading`, `link`, `color`, `callout`.
- На выделении обрамление сохраняет текст и преобразует ровно выбранный source range; повторное применение не создаёт бесконечного вложения. Без выделения разрешается вставка пары `**|**` и аналогов **только если включён autopair**.
- Не обрамлять фрагменты внутри fenced code, не ломать `[[wikilink]]` или Markdown link delimiters, не изменять неожиданные соседи диапазона.
- Текстовые цвета: `<span style="color:#RRGGBB">...</span>`, фон: `<mark style="background-color:#RRGGBB">...</mark>`; палитра позволяет клавиатурой выбрать цвет, изменить и удалить стиль. Рендер обоих цветов обязан работать в Live Preview/Reading, Source показывает Markdown.
- Имеющиеся методы `truff_core::markdown::style` **переиспользовать**, не копировать их в UI.

## 4. Режимы отображения и source ↔ view

### 4.1. Source
Видны все символы Markdown, без скрытых маркеров. Подсветка синтаксиса и типографика возможны, но source offsets должны совпадать с содержимым файла.

### 4.2. Live Preview
Markdown-маркеры скрываются либо стилизуются **только визуально**. Когда caret/выделение пересекает объект, **раскрывается его исходный синтаксис**, не меняя Rope и scroll-anchor. Одна и та же позиция caret должна соответствовать тому же source byte до/после раскрытия. Внутри одного редактора можно мгновенно переключать `Source` ↔ `LivePreview` с сохранением caret, selection, scroll.

### 4.3. Reading
Документ не редактируется. Полный визуальный Markdown: заголовки, ссылки/wiki-links/alias, списки, задачи, цитаты, callouts, таблицы, code fences, inline code, strike, highlight, HR, footnotes, frontmatter hiding, images и embeds (другие файлы — с fallback), math и Mermaid (последние фазы). Любой неподдержанный блок отображается безопасным текстовым fallback, не пропадает.

**Матрица целевого парсинга:**

| Элемент | Source | Live Preview | Reading |
|---|---|---|---|
| H1–H6 | raw `#` + текст | пропорциональный заголовок, показать `#` в фокусе | заголовок |
| Bold/italic/strike/highlight | видны маркеры | визуальный стиль + reveal при фокусе | визуальный стиль |
| Списки/tasks | raw маркеры | отступ/маркер/checkbox | готовый список |
| Wiki links/aliases/anchors | raw `[[...]]` | label + reveal | активная ссылка |
| Tables | pipe source | аккуратная сетка с редактируемым source | таблица |
| Fenced code | raw fences | кодовая панель с language и copy | кодовая панель |
| Callouts | `> [!kind]` | единая семантическая панель | панель |
| LaTeX/Mermaid | raw source | отложенный render + fallback | render + fallback |
| Images/embeds | raw source | preview с ограничениями по памяти | контент или fallback |

### 4.4. Визуальные гарантии
- Одна масштабная типографическая система Obsidian-like: H1–H6, body, lists, code, captions; line height и отступы из tokens, без случайных px в каждом виджете.
- Большая центральная область; optional readable-line-width 760–900 logical pixels при широком окне, но адаптивно при узком.
- Текст выделяется аккуратным полупрозрачным слоем **под** glyphs; caret и Flash поверх него. Цветовые span/mark не должны ухудшать контраст выделения.
- Размер строки берётся из реального galley/line layout, не константы символа. Для переносов — несколько rect по визуальным строкам.

## 5. Ссылки и знания — операции редактора

1. Ввод `[[` вызывает completion файлов **текущего хранилища**, `[[Note#` предлагает заголовки; `[[Note^` — блоки; `|` — alias. Если store API ещё не готов, использовать inject-able локального `CompletionProvider`, пустая выдача не ломает редактирование.
2. В Visual выделить `text`, вызвать `link.create`, выбрать заметку/заголовок из крупной панели → заменить выделение на `[[Target|text]]` (или `[[Target]]`, когда совпадает); undo восстанавливает байты.
3. Link target resolution — задача store; UI не гадает по пути и не встраивает индекс. Сломанные ссылки показывают состояние unresolved, но текст остаётся редактируемым.
4. Normal `gf`/явная команда follow-link открывает ссылку под caret, в том числе heading/block; `Esc` возвращает на прежнее место. Ctrl/Cmd+click — дополнительный mouse shortcut.
5. Link hover preview и backlinks вызывают тот же resolver/provider; никакой второй системы поиска ссылок только для редактора.
6. Автоматические backlinks **по выделению**: выделить source fragment → команда `link.attach_selection` → Picker с note/heading targets → подтвердить → атомарная ссылка/вставка с сохранением выделенного текста. Идеи n-gram ranking внедряются в store позже.

## 6. Выделение, Flash и Unicode (P0)

- `VisualChar`: начало/конец диапазона лежат на границе **полного grapheme cluster**. Правая граница включаемого Vim-символа преобразуется в полуоткрытый source range: `[start, grapheme_next(head))`.
- `VisualLine`: логические строки выделены от первого байта первой строки до байта после последней; визуальная заливка до края content area — политика line-wise, не character-wise.
- `VisualBlock`: **не выдавать за реализованное**, пока нет отдельной модели колонок/soft-wrap/Unicode; вынести в future.
- Ширина каждого selection rect = фактические x-boundaries shaped glyphs для соответствующего visual row, а не `char_count * fixed_width`. Вертикальные границы учитывают реальный ascent/descent/row height (включая H1, CJK, emoji). При `👨‍👩‍👧‍👦`, combining `é`, CJK, ligatures, tabs выделение не делит grapheme.
- Для softwrap строятся rect отдельно по визуальным строкам. Hidden marker `**` в LivePreview не должен выделяться видимым прямоугольником вместо текста.
- Клавиша `s` запускает Flash **только в Editor Normal**. Введённая `s` в Insert, filename, folder, search, command prompt, IME остаётся текстом.
- Flash labels привязываются к source ranges видимых объектов; при выборе метки caret попадает в допустимую границу source. `S` — отдельный режим AST object ranges (после готовности parser).
- Selection и Flash не должны переносить scroll внезапно; переход к невидимому target сначала изменяет scroll, затем перерисовывает.

## 7. Прокрутка, viewport и навигация (P0)

- При `j/k`, `gg/G`, поиске, `n/N`, `gf` и выборе заголовка caret остаётся внутри **видимой** области. Не использовать первоначальную оценку высоты строки вместо реально измеренной после layout.
- Курсор на последнем логическом и визуальном ряду всегда достижим; плавная прокрутка колеса/тачпада до конца документа; scrollbar корректен.
- **Явной ручной прокрутке не должен противоречить caret-follow**: колесо и `<C-d>/<C-u>` меняют viewport; автодоводка применяется при следующем реальном изменении caret/active target, не каждый кадр.
- `zz`, `zt`, `zb` устанавливают позицию caret относительно области редактора (центр / верх / низ); не перемещают source caret.
- Высоты больших заголовков, списков, softwrap, раскрываемого syntax и callouts индексируются инкрементально; если изменилась высота выше viewport, scroll-anchor компенсирует дельту.
- Экран в 800+ строк, длинные строки, быстрый переход к концу, изменение размера окна, font-size и переключение режимов обязаны работать без прыжка в начало.
- Если виртуализированная строка ещё не измерена, сначала приблизительный bring-into-view; после измерения гарантированно корректировать viewport. **Отсутствие caret в render window не отменяет его автоподводку.**

## 8. Автодополнение и подсказки

- Источники: файлы/заголовки, Markdown snippets, существующие слова текущего файла, команды, опционально словарь. Ранжирование deterministic, memory-bounded, без LLM и сетевых запросов.
- `[[` и `#` внутри wikilink активируют ссылочную подсказку, обычный текст — слова/орфографию по настройке.
- Popup не крадёт editor state: выбор Enter/Tab (настраивается), `Esc` закрывает popup без выхода из Insert; текстовый ввод не попадает в Flash.
- Which-Key — **для команд**, а не для каждого вводимого символа. Должен генерироваться из существующего registry с учётом current focus/mode.
- Spellcheck подчёркивает ошибки, не переписывает слова сам; словарь/язык конфигурируемы, исходный Markdown код/URL/fenced code не проверять.

## 9. Матрица приёмки: необходимые сценарии

**P0 — блокеры**:
- [ ] `s` в title/folder/search/Insert печатается; в Normal вызывает Flash ровно один раз.
- [ ] 1000 строк, `G`, 200×`j`, `gg`: caret всегда на экране, scroll достигает низа.
- [ ] Колесо прокручивает без обратного «прыжка» к caret на следующем кадре.
- [ ] `zz`, `zt`, `zb`, `<C-d>/<C-u>` дают правильную геометрию на многострочных H1 и softwrap.
- [ ] VisualChar на `W`, `Ш`, `🧑🏽‍💻`, `e\u0301`, tabs, ligatures не рисует «одинаковую ширину»; выделение доходит до фактической границы glyph.
- [ ] VisualLine на заголовках и абзацах учитывает большую высоту и не обрезает нижнюю часть.
- [ ] Source↔LivePreview не меняет байты, cursor, selection, undo.
- [ ] IME preedit не меняет Rope; commit вставляет точно один раз.

**P1 — smart editing**:
- [ ] Все строки таблицы §3.1, включая пустой/вложенный список и quote+list.
- [ ] `Shift+Enter` не создаёт маркер; `Tab/Shift+Tab` сохраняют вложенные subtrees.
- [ ] Task toggle имеет одно undo; `.`, macros, CRLF сохраняются.
- [ ] Fenced code, YAML, inline code не получают list continuation.
- [ ] Enter в середине пункта и при активном selection детерминированен.

**P2 — Markdown & links**:
- [ ] Все элементы матрицы §4.3 хотя бы fallback, без потери исходника.
- [ ] `[[` completion, `#heading`, alias, broken-link, `gf`, create-link-from-visual.
- [ ] Bold/italic/color/highlight/callout сохраняют исходные байты при undo, без бесконечных nested tags.
- [ ] Скрытие/раскрытие маркера не даёт source/visual drift.
- [ ] Нажатие checkbox мышью и `Ctrl+Enter` клавиатурой дают одинаковый transaction.

**P3 — resilience/performance**:
- [ ] 10k+ логических строк: нет O(document size) перерисовки на один символ, layout видимой части с overscan.
- [ ] Длинная строка >4096 bytes: корректны hit-testing, caret, горизонтальная прокрутка и выделение; не скрывать содержимое без возможности достичь.
- [ ] Ошибочный/незакрытый Markdown, invalid link, CRLF, пустой файл, Unicode, paste, undo/redo и recovery без panic.
- [ ] Память viewport/cache ограничена; есть performance сценарий со счётчиками re-layout, frame latency и scroll stability.
- [ ] При каждом PR: `cargo fmt --check`, `cargo test -p truff-ui`, `cargo test -p truff-core`, `cargo clippy -p truff-ui --all-targets -- -D warnings` (если установлен общий baseline — соблюдать).

## 10. Поэтапная реализация и ownership

| Этап | Что | Владелец |
|---|---|---|
| ME-0 | Аудит и тестовые фикстуры, freeze наблюдаемого поведения | `ui/tests`, без изменения crates |
| ME-1 | Input routing / caret / selection / scroll / Unicode (P0) | `src/editor` + `src/native/editor`; интеграция `native/events` отдельной задачей |
| ME-2 | Smart newline / indent / task / source transforms | `core` реализует семантические операции; `ui/src/editor` — адаптер; UI не дублирует parser |
| ME-3 | Source/LivePreview/Reading + Markdown style + mapping | `src/editor/display` и `src/native/editor` |
| ME-4 | Links/completion/selection actions | core/store provide + UI adapters |
| ME-5 | Advanced table/code/callouts/math/Mermaid/media + spellcheck | изолированные renderer/providers |
| ME-6 | Stress, perf, resilience, UX polishing | `ui/tests` и `src/perf_scenarios` |

**Строгий порядок:** ME-0 → ME-1 → ME-2 → ME-3 → ME-4 → ME-5 → ME-6. Никакого нового «редактора целиком» рядом с существующим.

### Контракты между крейтами

- `truff-core`: `Buffer`, `BytePos`, `Transaction`, `ChangeSet`, `IncrementalSyntaxTree`, `SyntaxSnapshot`, Markdown transformations, Vim, workspace. Не импортирует egui и UI.
- `truff-store`: read/query provider файлов/заголовков/ссылок; ничего не рисует; не решает, как выглядит popup.
- `truff-ui`: source/display mapping, rendering, IME, focus, command routing, styling controls, scroll, completion presentation. **Не имплементирует второй Markdown AST и не пишет в SQLite напрямую из editor widget.**
- Межпапочные требования передавать как файл `INTEGRATION_REQUESTS.md` с именем нужного API, входами, выходами и тестом. Не делать скрытых cross-folder правок.

### Definition of Done
Каждая задача считается завершённой, только если есть (1) unit/GUI regression tests, (2) сравнительный baseline, (3) нет изменений вне разрешённой папки либо они отдельно согласованы, (4) указан реальный список изменённых файлов, (5) нет TODO вместо заявленного поведения.

## 11. Первые места для проверки (состояние `argon-codex`, 15.10.2026)

- `crates/ui/src/editor/input.rs`: `EditorInput` и `InputAdapter` уже существуют; `insert_str` выполняет универсальную замену, а не AST-aware `Enter`.
- `crates/ui/src/editor/display/{mapping,projection,markers}.rs`: отображение Source/LivePreview и скрытие маркеров уже есть; проверить корректность source ↔ view в Unicode.
- `crates/ui/src/native/editor/viewport.rs`: scrolling и высоты виртуализированных строк уже есть, но reported bug — переходы вниз/прокрутка.
- `crates/ui/src/native/editor/paragraph/paint.rs`: расчёт glyph-rect и Visual selection; источник reported bug высоких/широких глифов.
- `crates/ui/src/native/events.rs` + `src/shell/routing.rs`: маршрутизация Flash по `s`, IME, focus; не переписывать дважды.
- `crates/core/src/markdown/{syntax,presentation,style,link_selection}.rs`: переиспользовать текущие AST, task toggle, цвет, callout и links.
- `crates/ui/tests/{editor_input,display,gui,native,vim_routing}.rs`: существующие тесты расширять, не заменять.

**Reference:** [GitHub branch argon-codex](https://github.com/hackerman111/truff/tree/argon-codex). Этот документ — контракт поведения. Его небольшие task prompts лежат рядом.
