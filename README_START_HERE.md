# Truff — мини-комплект Markdown Editor
**Почему:** большой UI-план не помещается в контекст агента. Этот комплект формализует поведение редактора, а исполняемые задания разделены по папкам.

## Для работы с одним крейтом/папкой

1. **ME-A**: приложи `crates/ui/src/editor/` + `01_EDITOR_MODEL_AGENT_PROMPT.md`. Получи `EDITOR_MODEL_API.md`.
2. **ME-B**: приложи `crates/ui/src/native/editor/` + `02_NATIVE_EDITOR_AGENT_PROMPT.md` + небольшой `EDITOR_MODEL_API.md`. Получи `NATIVE_EDITOR_GEOMETRY.md`.
3. **ME-C**: приложи `crates/ui/src/native/events.rs`, `crates/ui/src/shell/`, `crates/ui/src/app/commands/` + `03_MARKDOWN_EDITOR_INTEGRATION_AGENT_PROMPT.md` + итоговые API summaries. Это подключение клавиатуры/команд.
4. **Контракт поведения:** `00_MARKDOWN_EDITOR_BEHAVIOR_CONTRACT.md` — единственный полный документ. При ограниченном контексте передай агенту только его тематические §, перечисленные в prompt.

## Правила
- У агента ровно одна папка (или перечисленные integration paths) на запись.
- Реализацию внешнего API не воображать: согласовывать через `INTEGRATION_REQUESTS.md`.
- Не смешивать баги скролла Help/Navigator с editor viewport: Help/Picker находятся в других папках UI.
- Source/LivePreview/Reading — не три независимых редактора.
- Existing support в core (AST/style/link) переиспользуется, а не копируется.
- Сначала P0: scroll, Visual selection, focus, затем smart editing и render.
- Файл проекта НЕ был изменён этими документами.
