# PROMPT ДЛЯ ИИ-АГЕНТА №3 — `truff-ui`

**Вход:** только папка `crates/ui/` редактора Truff. **Изменять только `crates/ui/**`.** **Контракт:** `truff-crates-2026-10-14-v1`. **Ориентир:** `hackerman111/truff@argon-codex`.

## 0. Миссия: быстрый, Obsidian-like UI, полностью Vim-first

Ты — Rust GUI/UX engineer, отвечающий за **всё, что пользователь видит и чем управляет**. Truff — быстрый нативный Obsidian-аналог на egui/eframe с Vim-моделью для редактора, файлового дерева, текстовых полей, Navigator, буферов, связей, цветов и досок. Главная философия — **SessionX для всего приложения**: вызвать большую свободную панель, выбрать режим, fuzzy-фильтровать, увидеть предпросмотр, выполнить действие — без мыши. `which-key` всегда контекстен, `j/k` работают в Normal, обычный ввод не перехватывается global bindings.

**Три СРОЧНЫХ пользовательских бага, устрани ПЕРВЫМИ отдельным маленьким PR:**

1. **При движении выбора вниз (клавиши работают, selected item меняется) содержимое не прокручивается вслед за выбором.** Исправить и для Navigator, и для остальных виртуализированных списков; отдельно проверить редактор при движении caret/Visual вниз.
2. **Help/Which-Key плохо прокручивается.** Нужны свободное большое окно, настоящая scroll область, keyboard navigation, wheel/trackpad и корректный фокус.
3. **Панели слишком маленькие, тесные, похожи на сырые popup-прототипы.** Все главные навигационные/поисковые/help панели должны быть **крупными и просторными**, адаптивными, как часть единой дизайн-системы. Компактными должны оставаться лишь небольшие tooltip/inline-menu.

Также среди известных ошибок: `s` при вводе имени папки может запустить Flash; Visual selection и текст выделяются по неверным размерам некоторых глифов/строк; UI частично дублирует keymaps и парсинг outline. Исправить по фазам ниже.

**Жёсткая граница:** не редактируй `crates/core`, `crates/store`, root Cargo, документацию за пределами `crates/ui`. Используй существующие public API `core/store`; если следующей фиче нужен новый тип/метод в их крейте, описывай в `Cross-crate requests`. Не копируй внутрь UI сложный search engine, Markdown parser, recovery или Vim parser. Если на входе лишь папка UI, путь `../core`/`../store` отсутствует: честно фиксируй невозможные cargo tests и работай с доступным исходником/контрактами.

## 1. Обязательная разведка (не путать планы с реализованным кодом)

Ожидаемые пути (проверь существование, не переименовывай ради красоты):

```
src/native/{mod.rs,events.rs,hints.rs,outline.rs,sidebar.rs,
            backlinks.rs,content.rs,tabs.rs,widgets.rs,status.rs}
src/native/editor/{viewport.rs,navigation.rs,paragraph/paint.rs}
src/navigator/{mod.rs,model.rs,render.rs,actions.rs}
src/shell/{routing.rs,panels.rs,editor_input.rs,tree.rs,mod.rs}
src/editor/{input.rs,display/{mapping.rs,markers.rs,projection.rs},heights.rs}
src/overlays/{flash.rs,which_key.rs,which_key/help.rs}
src/{focus.rs,theme.rs,rows.rs}
src/app/{mod.rs,commands/,global_config.rs,io/,session.rs}
src/perf_scenarios/{scroll.rs,editing.rs,memory.rs,metrics.rs}
```

**Наблюдения по исходникам целевой ветки (обязательно перепроверь):**

- `navigator/render.rs`: есть `ScrollArea::vertical().show_rows(...)`, но не видно надёжного `ensure selected row visible` после перемещения через `j/k/G/gg`. Панель содержит жёсткие `ui.set_height(480.0)` и `total_height = 360.0`. Это **сильный кандидат** причины «выбор есть, вниз не пролистывается», но диагноз подтвердить тестом.
- `native/editor/viewport.rs`: высоты абзацев предварительно оцениваются `HeightIndex`, курсор сравнивается с пределами scroll до измерения реальных `Galley`, а индекс и cache частично пересоздаются при revision. Возможны ошибки Scroll-follow на soft-wrap/разновысоких строках. **Проверить, а не объявлять доказанной причиной**.
- `native/hints.rs`: `egui::Window` фиксированно у нижней границы, `resizable(false)`, Help `ScrollArea::vertical().max_height(320.0)`. Требуется отдельная крупная конфигурация Help, responsive и keyboard navigation.
- `native/events.rs`: ручной перехват `Key::Char('s')` для Flash и отдельная обработка `j/k` backlinks; `shell/routing.rs` обрабатывает `Space`/`Ctrl-w` дополнительно. Нужен единый вход для команд в активном контексте.
- `native/outline.rs` делает построчную обработку заголовков вместо фактов Core; избегать второго Markdown parser в UI.
- `theme.rs` имеет семантические цвета; `native/creation/settings.rs` и другие диалоги могут иметь локальные стили, которые следует заменить токенами.

**Этап UI-0:** создай в своём крейте короткий audit/results документ или отчёт: действующие key contexts, компоненты с независимым scroll, таблица hardcoded размеров/цветов, screenshots текущих окон и воспроизводимые шаги для трёх багов. Замерь baseline, если среда позволяет.

## 2. UI-P0 — исправление прокрутки и Help ДО других фич

### UI-P0.1. Navigator: selected row всегда видна

**Проблема:** keyboard `j/k`, `gg/G`, PageDown/PageUp, counts, поиск и фильтр меняют `selected_index`, но список визуально остаётся на предыдущих строках.

**Сделать:**

1. Разделить `selected_index`, `visible_range`, `scroll_offset` и `scroll_request/scroll_target` как разные состояния. У virtualized list должна быть типизированная операция `EnsureVisible { item_index, alignment }`.
2. После **всех** клавиатурных изменений selection или смены запроса послать scroll request на новый item, с верхним/нижним запасом 1–2 строки; учесть реальную высоту viewport. `gg` показывает начало, `G` показывает конец, PageDown/Up двигают по фактической высоте, а не магическим 10 строкам.
3. При `show_rows` нельзя надеяться на `scroll_to_me()` на **неотрисованной** выбранной строке; целевой offset вычисляется вне range виртуального рендеринга или задаётся через поддерживаемый egui 0.36.2 `ScrollArea` state API. Не перезаписывать offset каждый кадр, если selection не менялась.
4. На кликах/колесе сохранить естественную ручную прокрутку. Клавиатурный `selected_index` не должен насильно возвращать скролл во время wheel input, пока выбор не изменился. Актуализировать состояние только по чёткому user intention; мягкий default, без излишней анимации.
5. Изменения filtering/generation могут уменьшать количество строк: clamp selection + scroll. `Space` preview toggle не должен сбрасывать позицию списка; смена режима сохраняет/restores разумную позицию.
6. Ту же проблему проверить в FileTree, Backlinks, Outline, Session/Buffers, search result lists, Help, Kanban. В идеале сделать **один маленький UI utility** для удержания выбранного элемента в viewport и использовать адаптер в каждом компоненте вместо 6 локальных решений.

**Тесты:** 1000 элементов, видны 12; 50×`j` → selected row видна, `G` → последний виден, `gg` → первый виден, `k` возвращается, filter shrinking, resize window, wheel interaction после keyboard, selection не выходит за экран. Для каждого теста хранить scroll offset/visible range и assert invariant. Проверь при HiDPI scaling и длинных строках.

### UI-P0.2. Редактор: caret/Visual следует за scroll

1. Протестировать обычный Insert/Normal `j/k`, `G/gg`, `Ctrl-d/u`, PageDown/PageUp, `10j`, `v` + движение вниз, мышиный drag до края viewport, wheel/trackpad, outline JumpToHeading.
2. Проанализировать `native/editor/viewport.rs` и `editor/heights.rs`: перестроение height index при каждой revision, влияние virtual line estimates, anchor restoration и условие видимости caret, когда paragraph **ещё не измерен**. Не исправлять баг тупой константой `LINE_HEIGHT` для всех строк.
3. Сделать **двухфазную** политику: сначала оценить target source row и безопасный offset, затем при появлении измеренного galley скорректировать cursor rect. Если cursor далеко за viewport — jump по index для lazy measurement; нельзя ждать, пока offscreen paragraph внезапно будет нарисован.
4. Стабильный scroll anchor при наборе текста/появлении высоких headings, soft-wrap и изменении размера. Не сбрасывать viewport на верх документа при вводе символа или переключении буфера; для каждого View сохранять собственную позицию.
5. Поведение Visual selection при прокрутке: anchor/source ranges стабильны, caret остается виден, глифы отрисовываются верно; прокрутка не должна самопроизвольно отменять выделение.
6. Wheel должен попадать в hovered/focused pane; при открытом overlay события/scroll не уходят под него в editor. Horizontal scroll только там, где он применим (широкие code/long lines).

**Тесты:** 2k строк с headings H1 + CJK/emoji + длинными wrap-абзацами, переход на последнюю строку, клавиатурное удержание selection, resize в узкое окно, text insertion above viewport, две View одного Buffer с независимыми scroll offsets. Не заявлять, что Editor bug был той же причиной, что Navigator, без доказательства.

### UI-P0.3. Help / Which-Key: большая, скроллируемая справка

1. **Разделить два режима по геометрии.** Prefix Which-Key — аккуратная широкая contextual hint panel у нижней части экрана, высота content-fit с разумным cap. **Полная Help** — большая центрированная панель/overlay, примерно 78–90% доступной ширины и 75–85% высоты, с responsive maximum и минимальными боковыми отступами.
2. Help header и footer **фиксированные**, между ними реальный внутренний `ScrollArea` без `max_height(320)`. На узких окнах адаптивный 1-колоночный layout; на широких 2–3 колонки **только если читаемость не страдает**, не размещать десять карточек плотной сеткой.
3. Все способы scrolling: wheel, touchpad, scrollbar, `j/k`, `gg/G`, PageUp/PageDown, `Ctrl-d/u` в Normal Help; `Esc` корректно закрывает/возвращает прежний focus. `/` включает фильтр справки (Insert), после `Esc` возврат к Help Normal. Клавиши при активном Help не идут в Markdown/Flash. Подсветить текущую секцию и отобразить scroll progress по необходимости.
4. Help строится из **текущего command registry**, список статических categories `default_help_categories` использовать лишь как fallback для собственно Vim motions, пока соответствующее API Core не позволяет более строгую модель. Не обещать пользователю несуществующих команд.
5. Prefix Which-Key не должен занимать непропорционально большую часть редактора, но должен оставлять достаточно горизонтального пространства; пользовательское описание, клавиша и категория визуально различимы.

**Тесты:** 80+ help items, прокрутка до последнего, возврат к первому, filtered query, resize/HiDPI, scroll wheel не двигает underlying editor, закрытие восстанавливает focus, мелкое окно не скрывает кнопку закрытия/последний элемент.

### UI-P0.4. Сразу убрать критическую утечку событий `s`

1. Удалить/защитить ранний глобальный перехват `s` в `native/events.rs`: Flash разрешается **только если Focus == Editor && Mode == Normal**, нет активного Editable/IME/Dialog/WhichKey/Navigator, нет pending text input; идеал — зарегистрированная контекстная команда.
2. Modal input для создания/переименования папки/заметки/vault должен принимать `s`, `j/k`, `d`, `:` и Space как буквальный текст в Insert; autocomplete/fuzzy работают без случайных глобальных команд.
3. Соблюдать event ownership egui: Text events идут ровно одному text-field, не дублируются key events, IME Preedit/Commit не превращаются в motions. `Esc` не закрывает сразу все уровни overlays.
4. Переподключить Backlinks/RowModel и routing через общий focus model насколько возможно на существующем API. Не создавать второй несовместимый Vim parser внутри UI.

**Тест:** набрать `session`, `source`, `glass`, `диссертация` в разных modal fields — нет Flash, текст сохранён; `s` Normal editor вызывает Flash.

**UI-P0 GATE:** пока `selected row visible`, editor caret-follow и полноценная прокрутка Help не работают, **не приступать к следующим этапам**. Это реальные пользовательские баги, не cosmetic.

## 3. UI-P1 — единая большая и спокойная дизайн-система

**Визуальный ориентир Obsidian:** цельный dock workspace, тонкие 1px разделители низкого контраста, спокойные тона и оттенки, минималистичные контролы, аккуратный typographic scale, пропорциональные читаемые шрифты, минимальная «терминальность» лишь в keys/status/commands. **Не делать raw TUI, «оконный зоопарк», множество мелких и разорванных панелей.**

### 3.1. Размеры — большие и свободные, но адаптивные

Примерные _дизайн-токены_, не строгие hardcode-константы (учитывать HiDPI, окна 800×600 и ultra-wide):

| Поверхность                    | Желаемый размер по умолчанию                              | Поведение                                            |
| ------------------------------ | --------------------------------------------------------- | ---------------------------------------------------- |
| Navigator Files/Search/Buffers | **80–90 vw**, ограничение max ~1280–1440 dp; **72–84 vh** | центр, крупный список + широкий preview, responsive  |
| Navigator списка               | **40–55%** ширины Navigator                               | высота почти вся свободная внутри панели             |
| Preview pane                   | **45–60%** ширины Navigator                               | читаемый текст, responsive, отдельный scroll         |
| Full Help                      | **78–90 vw × 75–85 vh**                                   | фикс. header/footer + большая прокручиваемая область |
| Floating TOC pinned/side       | **300–420 dp** на широком экране                          | resize; при недостатке места overlay или collapsible |
| Sidebar vault/files            | **280–360 dp**, resizable                                 | иерархия, отступы, dense/comfortable mode            |
| Right backlinks/outline        | **300–400 dp**, resizable                                 | полноценная читабельность, не обрезать текст         |
| Основная текстовая колонка     | комфортные **720–860 dp**                                 | центр; при узком размере fluid                       |
| Создание/переименование        | от **480–680 dp**, но ≤viewport                           | крупные текстовые поля и подсказки                   |
| Item row                       | **34–44 dp** comfortable                                  | иконка + имя + subtitle + область клика              |
| Spacing                        | 4/8/12/16/20/24/32 dp                                     | внутри панели, между блоками просторные 16–24 dp     |

Не вводить `.set_width(900)` на любом экране. Пусть Layout рассчитывает available screen и сохраняет user-resize/положение. Обеспечить масштабирование для 800×600/1280×720/1920×1080/HiDPI. На больших экранах пространство идёт прежде всего на содержимое, preview и комфортные поля, **а не на бесполезные большие пустые padding**.

### 3.2. Tokens/components и consistency

- Расширить существующий `theme.rs` для semantic palette: `bg_canvas/bg_panel/bg_elevated/bg_hover/bg_selected`, `text_primary/text_secondary/text_muted`, `border_subtle/border_focus`, `link/accent/selection`, status semantics, font sizes, spacing, radius, shadow/elevation, density, pane sizing. Убрать дубли цветов/пикселей из `native/creation/`, `native/hints.rs`, Navigator и остальных panels.
- Настроить 2 typography families: proportional для текста/UI; monospace для code/commands/keycaps. Учесть mixed glyph metrics, scalable line height, platform fallback и Unicode.
- Базовые обёртки: `DockPanel`, `PanelHeader`, `PaneSplitter`, `ListRow`, `TreeRow`, `SearchInput`, `PreviewPane`, `ModalHost`, `Dialog`, `HelpPanel`, `HintRow`, `ColorSwatch`, `StatusBar`, `Toast`, `ContextMenu`. Не делать огромного widget-framework; переиспользуй тонкие функции.
- Цвета — оттенки, контраст/hover/focus/selection логично различимы; active state важнее случайной рамки. Линии тонкие и непрерывные. Не использовать цвет как единственный сигнал состояния.
- Dock-панели не плавающие, фон целен, нет разрыва между ними. Попапы/оверлеи одной формы (soft radius 8–10 dp, лёгкая тень), не «один маленький из центра, второй прилеплен снизу, третий огромный».
- Сохранение layout, размера панелей, resizes, projection per view; keyboard `Ctrl-w h/j/k/l` и доступность tab order. Focus mode скрывает боковые панели по запросу.
- Внешний вид не обязан имитировать каждый пиксель Obsidian, но должен ощущаться как одно зрелое приложение, а не набор разностильных прототипов.

**UI-P1 GATE:** screenshots на 800×600, 1280×720, 1920×1080 (dark/light); Navigator/Help/creation/Sidebar выглядят как части одного продукта; тексты не клипуются, интерфейс не требует мыши.

## 4. UI-P2 — Navigator как SessionX + единый Vim keyboard UX

### 4.1. Одна большая панель, разные providers

Один реиспользуемый `Navigator` поддерживает modes: Files, Folders, Vaults, Search Content, Mixed, Buffers/Sessions, Recent, Outline/Headings, Backlinks/Links, Commands, Colors, Boards, Tasks. Базовый дизайн: фиксированный header+breadcumbs, большая строка поиска, виртуализованный список слева, превью справа, footer с actions. Меняется **поставщик данных**, а не механика управления/геометрия.

**Search variants:** file/title/path fuzzy, content FTS, mixed categorized, hierarchical folder tree (включая пустые папки), workspace session switch, scoped result views. UI **не реализует** SQLite/ranking; использует existing Store search API с асинхронными запросами. Состояния request generation/cancellation передаются без stale overwrite.

**Keyboard:**

| Контекст             | Сочетание        | Поведение                                                    |
| -------------------- | ---------------- | ------------------------------------------------------------ |
| Editor Normal        | `Space f`        | Files/Folders Navigator                                      |
| Editor Normal        | `Space s`        | Sessions/Buffers                                             |
| Editor Normal        | `Space /`        | Mixed Search                                                 |
| Editor Normal        | `Space v`        | Vaults                                                       |
| Editor Normal        | `Space o`        | TOC/Headings                                                 |
| Editor Normal        | `Space l`        | Link picker/backlinks                                        |
| Editor Visual        | `Space l`        | Сделать ссылку из выделенного текста                         |
| Editor Normal/Visual | `Space c`        | Color palette (Visual=apply selection)                       |
| Editor Normal        | `Space b`        | Boards/Kanban                                                |
| Editor Normal        | `Space r`        | Note Stream/Recent                                           |
| Editor Normal        | `Space g`        | Graph later                                                  |
| Global               | `Ctrl-p`         | Commands Palette                                             |
| Editor Normal        | `:`              | Beautiful native command line, completion                    |
| Normal active pane   | `Ctrl-w h/j/k/l` | Focus panes (не Ctrl-j/k)                                    |
| Navigator Normal     | `j/k/gg/G/h/l`   | Move/first-last/collapse-expand                              |
| Navigator Normal     | `/` or `i`       | Search Insert                                                |
| Navigator Insert     | `Esc`            | Back to Navigator Normal                                     |
| Navigator Normal     | `Enter`          | Open/activate                                                |
| Navigator Normal     | `a/r/d`          | Create/rename/delete через typed action, confirm destructive |
| Navigator Normal     | `Tab/Shift-Tab`  | Switch provider/source                                       |
| Navigator Normal     | `p`, `?`         | preview toggle / contextual Help                             |

Migration: если `Space c` было Commands, предложить compatible alias или документированную миграцию и контроль collisions. Never bind `s` Flash в любых полях Insert. Ctrl-j/k как обязательные motions **не нужны**.

### 4.2. Which-Key и помощь

- Which-Key/hints строятся из **активного Command Registry**, показывают контекст, группировки, breadcrumb, понятные названия, searchable command descriptions, disabled reasons. Один ключ не должен одновременно обслуживать две разные функции в одном контексте.
- Уменьшить глубину хоткеев: частые actions ≤2 символов после `Space`; всё редкое — в Navigator/Palette. Рабочие сценарии объединены крупными ветвями, как sessionX.
- Для многократных операций — гидра/loop mode: пока panel открыта, `j/k/h/l`, `a/r/d` повторяются без переоткрытия Navigator. Не вводить отдельные Vim интерпретаторы для каждого widget.
- Подсказки для текста, файлов, доски/графа и context menu единообразны. Help должен точно соответствовать реальным keys.

### 4.3. Панели, деревья, буферы

- Active FocusTarget = Editor, FileTree, Backlinks, Navigator, TextField, CommandLine, Modal, Board, TOC, Graph. UI поддерживает focus-stack restoration, correct event ownership and scoped hotkeys.
- Explorer `h/l` fold/unfold/parent/child, `j/k` rows, `gg/G`, `/` fuzzy, multi-select, rename/create/delete via Store file operation plans. Empty dirs видны только после Store соответствующего API; до этого не изобретать фиктивный catalog.
- Buffers/Sessions: быстрый fuzzy switch, preview, close other tabs, reopen recently closed, pin, later split panes. Одна заметка в двух View имеет независимый cursor/scroll и общий текст.
- Hover/click сохраняются для пользователей мыши, но весь основной workflow выполняется клавиатурой.

### 4.4. Командная строка и completion

Внизу редактора — спокойная `:`-строка с аккуратным inline-autocomplete для Command Registry, аргументов, путей, заголовков, хранилищ, тэгов. История, типы аргументов, диагностика, preview destructive effects. В Insert — обычный ввод; `Esc` — Command Normal, `j/k` выбирают историю/результаты, Enter выполняет action. Не вводить ещё один parser команд при имеющемся core registry.

## 5. UI-P3 — Markdown-отображение, грамотное выделение, цвета

1. Интегрировать уже существующий `MarkdownFacts`/Parser Core с UI `ProjectionMode Source / LivePreview / Reading`. Heading hierarchy H1–H6, emphasis/bold/strikethrough/highlight, code blocks, links/wiki embeds, tables, tasks, quotes/callouts, footnotes, image/PDF, позднее Math/Mermaid. Реальные AST spans/source mapping вместо regexp по строкам.
2. Smart edit Enter/Tab/ShiftTab для списка/quotes вызывают **core transactions**, а не дублирующиеся UI-вставки. Preview не должен удалять исходные символы.
3. **Visual selection geometry bug**: core обеспечивает UTF8 grapheme byte range; UI отвечает за конкретные rectangles по сформированному `egui::Galley`. Ширина каждого cluster определяется реальными boundaries/advances, высота visual row — реальными max ascent/descent/line-height. Не использовать условные 8.5 px и «высоту обычной буквы» для всей строки, когда есть Ж/emoji/заголовок. Учесть combining, emoji ZWJ, CJK, tab, soft wrap, multiple font sizes, ligatures, hidden markup. Для VisualLine rectangular semantics явные, line high glyph not cropped. `DisplayMap` coordinates и hit-test должны совпадать с painting.
4. Flash `s` по словам только Editor Normal; `S` по AST objects/ranges с labels, если Core API доступен. `operator-pending` и `Visual select` используют один диапазон, UI только отмечает.
5. Color: через `Space c` большая удобная палитра для foreground/background и highlight, favorite/recent/theme colors, remove; apply formatting atomic core edit plan, не вручную оборачивать каждую строку в UI. Аналогичная визуальная палитра для callout, task labels, folder/graph nodes (там хранить metadata, а не HTML в тексте). Цвет — важный полноценный инструмент проекта, а не один акцент темы.
6. Рендеринг больших Markdown заметок виртуализировать, дорогие syntax trees/layouts кешировать по revision/theme/width. Изменение темы не должно перепарсивать весь текст.

**Тесты:** Unicode visual rects W/i/Ж/界/👩‍💻/e\u{0301}, high heading, wrapped line, mixed font, colored span, source↔visual hit test, one Undo, no layout jitter while scrolling.

## 6. UI-P4 — Floating TOC, Folder Wiki, лента заметок

### 6.1. Floating TOC (обязательный плагин и развитие)

Идеи брать из [pkm-er/obsidian-floating-toc-plugin](https://github.com/pkm-er/obsidian-floating-toc-plugin) (MIT): активный heading, auto-sync по caret/scroll, collapse, поиск, быстрый переход, sticky parent chain, drag-reorder только через корректные source ranges. **Не копировать Obsidian DOM/TypeScript**; реализовать native egui адаптер к Core Markdown facts и Store catalog.

Нужна **одна большая, комфортная иерархическая панель**, которую можно pin в dock либо вызвать как overlay, с scope: Active File / Folder / Vault. Внутри: `folder → note → heading → block`, Recent visited и соседние связанные файлы. `j/k/h/l`, `gg/G`, `/`, Enter open, `o` open in adjacent pane; активная позиция всегда видна благодаря UI-P0 scroll controller. User can navigate and return without mouse.

### 6.2. Folder Wiki

Папка имеет мастер-файл `index.md`/настраиваемое имя, а UI показывается как wiki dashboard: notes tree, headings, ideas/tasks, metadata tables, backlinks, recent changes. Генерируемая часть — виртуальная preview или безопасно ограниченный auto-generated block от Store, не перезаписывающий ручные записи. Может разворачиваться `folder -> note -> heading`, фильтроваться и упорядочиваться Vim-моторами; дизайн на уровне Obsidian Bases, не сырая таблица.

### 6.3. Note Stream / возвращение к старым заметкам

Одно визуальное представление многих **раздельных Markdown файлов**: пользователю кажется, что ведёт последовательный дневник/поток заметок, но каждая запись физически остаётся `.md`. Создание записи открывает отдельный файл и встраивает карточку в поток. Фильтры today/unfinished/related/revisit, карточки старых неоткрытых записей, объяснение почему показаны. Scroll/keyboard/preview должны быть быстрыми даже при 1000 записей — virtual rows, not concat text. Нет автоматических дубликатов note content.

## 7. UI-P5 — Kanban, Queries, Graph, Git и плагины (поздние этапы)

- **Kanban**: большая комфортная доска, а не 3 узких карточки; markdown tasks в Store/Core source ranges, keyboard `h/l` columns, `j/k` cards, `a` create, `r` rename, `m` move, `d` delete with confirmation, Enter edit/open, `/` filter. Drag/drop вызывает тот же typed action, доступна undo/save/reopen. Колонки responsive/горизонтальный скролл; выбраная карточка видна после keyboard move. Theme tokens + nice spacing/icons/colors.
- **Dataview/Bases-like views**: таблицы/списки/карточки из Store typed query; виртуализация, фильтры, сортировки, preview; без выполнения произвольного кода. `.base` interoperability later после формата.
- **Knowledge graph**: force-directed, local/global, radial, directed, rooted spanning tree (cross-links остаются), breadcrumbs, keyboard motions `h/j/k/l`, `/`, Flash targets, node colors/categories. Из графа можно показать дерево, но граф с циклами неизбежно имеет дополнительные связи — их не стирать.
- **Git**: состояния, сравнение двух revisions, diff/history/picker; commit/restore только по явному действию, в UI confirmation. Не делать SSH/auth manager без потребности.
- **Plugins/extensibility**: сначала регистрация новых commands/views/navigator sources/themes в trait adapters. Не обещать hot-swap native Rust crates; для настоящих 3rd-party sandboxed plugins позже версионированный Wasm API/capabilities.
- **Spellchecking and text completion**: асинхронные UX popovers для словаря, ссылок, paths, history, vault terms; source ranking и fuzzy responsiveness в духе coq_nvim; отдельные disabled zones code/TeX и independent source toggles. Вся логика индексирования слов — Store/Core, UI только отображает suggestions.

## 8. Производительность, тесты, приёмка и запреты

**Нельзя** заменять egui/eframe на GPUI/Masonry или переписывать всего редактора, пока нет benchmark-а, подтверждающего необходимость. Существующая ветка использует `eframe = 0.36.2`. Не добавлять дубли stack layers, runtime-поиск по filesystem из draw loop, тяжёлые regex в paint, map of all rendered glyphs of 10k notes.

### UX acceptance matrix — эти сценарии обязательны

| №   | Действие                                             | Наблюдаемый результат                                                             |
| --- | ---------------------------------------------------- | --------------------------------------------------------------------------------- |
| 1   | Navigator открыть 1000 элементов, нажать `j` 100 раз | selected item на экране, scroll следует                                           |
| 2   | `G`, затем `gg`, затем PageDown                      | правильная scroll position и выбранный ряд                                        |
| 3   | в редакторе перейти `G`, 20×`j`, `v`+30×`j`          | caret и selection видимы; без телепорта viewport                                  |
| 4   | 500 строк со смешанными H1, emoji, wrap; resize      | стабилен scroll anchor; курсор следует                                            |
| 5   | `?` Help с 80+ команд, `j/k`, wheel, PgDown, `G`     | виден последний элемент; editor под Help не скроллится                            |
| 6   | узкое окно ~800×600                                  | Navigator/Help занимают доступное место, элементы не обрезаны, скроллинг работает |
| 7   | широкое окно 1920×1080                               | просторные 2-колоночные панели, полноценный preview, не кучка малых cards         |
| 8   | при rename folder ввести `session`                   | имя текстовое, Flash не срабатывает                                               |
| 9   | Visual выделить `Ж`, `W`, `i`, `界`, `👩‍💻`            | полная ширина grapheme, реальная высота строки                                    |
| 10  | открыть Navigator, ввести поиск, Esc, j/k, Enter     | корректный mode transition и focus restoration                                    |
| 11  | нажать Space, подсказка, вызвать команду             | registry/Which-Key/Palette имеют одну семантику                                   |
| 12  | `Ctrl-w h/j/k/l` между dock panes                    | фокус следует, выбранная строка подсвечена, без Ctrl-j/k                          |
| 13  | активировать TOC heading, открыть note, вернуться    | linked source positions и independent scroll per view                             |
| 14  | Kanban `m` move card → undo → reopen                 | исходный Markdown восстановлен без рассогласования                                |
| 15  | цветной выделенный текст Undo/Redo                   | корректные Markdown bytes и render                                                |

### Замеры

- Запустить доступные `src/perf_scenarios` (scroll/startup/memory), дополнить графикой влияния `100/1000/10000` заметок без полного UI render всех записей.
- Целевой frame budget на обычной сцене p95 ≤16.7 ms при 60Hz **как измеримая цель**; separate input→paint latency и p95 memory/cold startup. Любое изменение scrolling должно учитывать wheel/scroll acceleration и HiDPI.
- Screenshot checks dark/light and density, focus indication, disabled states, narrow and wide layouts. Не заявляй результаты, если тесты не запускались.

### Проверка сборки, если есть workspace

```
cargo fmt --all -- --check
cargo check -p truff-ui
cargo test -p truff-ui
cargo clippy -p truff-ui --all-targets -- -D warnings
```

Дополнить model/headless tests для scroll helper и manual smoke для окна. Если тестовой ОС/GUI/display нет, делай headless geometry/layout tests и обозначь, что ручная проверка нужна; это не повод объявить баг полностью исправленным без воспроизведения.

## 9. Порядок реализации — только внутри UI crate

```
UI-0  Audit + reproduce + baseline
 ↓
UI-P0 Navigator scroll-follow + Editor caret scroll-follow
 ↓    Help scrolling/large overlay + s Flash input scope
UI-P1 Responsive spacious Obsidian-like design system
 ↓
UI-P2 One Navigator / WhichKey / Commands / buffer navigation
 ↓
UI-P3 Markdown rendering / Visual glyph geometry / color palette
 ↓
UI-P4 Floating TOC / Folder Wiki / Note Stream
 ↓
UI-P5 Kanban / Queries / Graph / Git UI / completion / plugins
```

В **первом PR** сосредоточься на `UI-P0`, не разводи масштабный redesign одновременно с устранением функционального бага. Следующим PR — широкие панели и общие токены; затем остальные подсистемы. Существующие уже реализованные функции не переделывать без нужды.

## 10. Отчёт для приёмки

Ответить:

```
UI status (UI-P0 ... UI-P5):
Files changed:
Scroll root causes confirmed (Navigator / Editor / Help separately):
How ensure-visible works (and virtualized offscreen rows):
Responsive layout dimensions at 800×600 / 1280×720 / 1920×1080:
Focus, Insert and Flash scoping changes:
Screenshot / visual regression evidence:
Tests and cargo commands/results:
Perf baseline / after (real measurements):
Required Core/Store API (Cross-crate requests, no edits made there):
Remaining bugs / limitations:
```

**Самое важное:** не засчитывай «scroll работает» потому, что selection меняет индекс. Приёмка — выбранный объект **автоматически попадает в видимую область**, в том числе в виртуализированном списке при `G` и после 100 `j`. **Панели должны быть большими и свободными**, а не крохотными модалками с вложенными несинхронными scrollbar-ами.
