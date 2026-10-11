# Дизайн: Поддержка Which-Key и вызов Help по клавише `?`

## Цель и контекст
Обеспечить интуитивную и быструю навигацию по клавиатурным комбинациям и командам приложения Truff v2.
1. При наборе префиксных комбинаций (например, `<C-w>` или кастомных префиксов) через 300 мс (или немедленно) снизу экрана отображается аккуратная панель Which-Key со списком доступных продолжений.
2. В Normal mode нажатие клавиши `?` (а также ввод команды `:help`) открывает интерактивную панель справки Which-Key с полным перечнем горячих клавиш и команд, разбитых по категориям.
3. Нажатие `Esc` мгновенно скрывает оверлей.

## Архитектура и структуры данных

### `crates/ui/src/overlays.rs`
Модель `WhichKeyOverlay` расширяется для поддержки двух режимов:
- `WhichKeyMode::Prefix`: отображение вариантов продолжения текущей цепочки клавиш.
- `WhichKeyMode::Help`: отображение структурированного каталога горячих клавиш по категориям.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WhichKeyMode {
    Prefix,
    Help,
}

#[derive(Debug, Clone)]
pub struct WhichKeyCategory {
    pub name: String,
    pub hints: Vec<KeyHint>,
}

#[derive(Debug, Clone)]
pub struct WhichKeyOverlay {
    hints: Vec<KeyHint>,
    categories: Vec<WhichKeyCategory>,
    mode: WhichKeyMode,
    title: String,
    visible: bool,
    elapsed_ms: u64,
    delay_ms: u64,
}
```

Ключевые методы:
- `set_prefix_hints(title, hints)` — настройка префиксного режима с задержкой 300 мс.
- `show_help(categories)` — немедленное отображение полного справочника.
- `hide()` — сокрытие оверлея и очистка состояния.
- `default_help_categories()` — возвращает структурированный каталог:
  - *Навигация (Motions)*: `h`, `j`, `k`, `l`, `w`, `b`, `e`, `gg`, `G`, `0`, `$`.
  - *Правка (Edits)*: `i`, `a`, `x`, `dd`, `cc`, `yy`, `p`, `u`, `Ctrl-r`.
  - *Панели и Flash*: `Ctrl-w h/l/j/k`, `s` (Flash jump), `?` (Справка).
  - *Команды*: `:w`, `:wq`, `:vault`, `:link`, `:backlinks`, `:rename`, `:trash`, `:theme`, `:new`, `:help`.

## Маршрутизация ввода

### `crates/ui/src/shell.rs`
1. При получении `Key::Char('?')`:
   - Если режим `Mode::Normal` и фокус не находится в поле ввода команды или поиска:
     - Вызывается `self.which_key.show_help(WhichKeyOverlay::default_help_categories())`.
     - Возвращается `ShellAction::Incomplete`.
2. При получении `Key::Esc`:
   - Если `which_key.is_visible()`, вызывается `self.which_key.hide()`.
3. При вводе `<C-w>`:
   - Устанавливаются префиксные подсказки для управления окнами с заголовком `<C-w> Окна и панели`.
4. Ввод подсказки при отображённом оверлее:
   - Если нажата клавиша из активного набора подсказок префикса, выполняется команда и оверлей закрывается.

### `crates/ui/src/app/commands.rs`
Команда `:help`:
```rust
"help" => {
    self.shell.which_key_mut().show_help(WhichKeyOverlay::default_help_categories());
    self.status = "Which-key справка открыта (Esc для закрытия)".into();
    Ok(())
}
```

## Визуальный интерфейс (`crates/ui/src/native/`)
- Оверлей рисуется как плавающее окно внизу экрана:
  - `egui::Window::new("which_key_overlay")`
  - `.title_bar(false)`
  - `.collapsible(false)`
  - `.resizable(false)`
  - `.anchor(egui::Align2::CENTER_BOTTOM, [0.0, -28.0])`
- Стилизация:
  - Рамка: `theme.border` / `theme.accent_primary`.
  - Фон: `theme.bg_secondary` со скруглением 8 px.
  - Бейджи клавиш: моноширинный акцентный блок на фоне `theme.bg_tertiary`.
  - В режиме справки: сетка из колонок по категориям для быстрого визуального сканирования.

## Тестирование
- Тесты таймера и отображения префикса (300 мс).
- Тест срабатывания `?` в Normal mode и отсутствие эффекта в Insert mode.
- Тест сокрытия по `Esc`.
- Тест команды `:help`.
- Тест рендеринга виджета в `egui`.
