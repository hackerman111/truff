# Дизайн: Поддержка различных шрифтов, моноширинный шрифт по умолчанию и базовые настройки

**Дата:** 2026-10-11  
**Статус:** Согласовано  
**Цель:** Добавить поддержку различных шрифтов с моноширинным шрифтом по умолчанию, модальное окно настроек с персистентностью в `config.toml`, и динамическое масштабирование типографики редактора от базового размера шрифта (`base_font_size`).

---

## 1. Контекст и цели
В Truff v2 шрифты и размеры в редакторе были жестко закодированы в `crates/ui/src/native/paint.rs` с использованием пропорционального шрифта для основного текста и заголовков.
Требуется:
1. Сделать моноширинный шрифт (`Monospace`) основным шрифтом по умолчанию.
2. Поддержать выбор различных шрифтов (семейств), включая возможность указания пользовательского TTF/OTF шрифта.
3. Добавить базовые настройки с сохранением в `$XDG_CONFIG_HOME/truff/config.toml`.
4. Реализовать настройку базового размера шрифта (`base_font_size`), от которого пропорционально масштабируются все элементы редактора (заголовки H1–H6, блоки кода, высота строк), сохраняя компактность и пропорции панелей интерфейса.
5. Реализовать доступ к настройкам через модальное окно с векторной иконкой шестерёнки (⚙) в сайдбаре и Vim-командами `:settings` / `:config`.

---

## 2. Архитектура и структуры данных

### 2.1 Конфигурация типографики (`TypographyConfig`)
В `crates/ui/src/app/global_config.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypographyConfig {
    #[serde(default = "default_font_family")]
    pub font_family: String,

    #[serde(default = "default_base_font_size")]
    pub base_font_size: f32,

    #[serde(default = "default_line_height_multiplier")]
    pub line_height_multiplier: f32,

    #[serde(default)]
    pub custom_font_path: Option<PathBuf>,
}
```

- `default_font_family()`: `"monospace"`
- `default_base_font_size()`: `15.0`
- `default_line_height_multiplier()`: `1.5`

### 2.2 Интеграция с `GlobalConfig` и `Shell`
В `GlobalConfig` добавляется поле `pub typography: TypographyConfig`.
В `Shell` добавляются методы:
- `pub fn typography(&self) -> &TypographyConfig`
- `pub fn set_typography(&mut self, config: TypographyConfig)`

При инициализации `Application::new()` конфигурация загружается из `GlobalConfig` и передаётся в `Shell`.

---

## 3. Шрифты в egui и динамическое масштабирование

### 3.1 Регистрация шрифтов
В `crates/ui/src/native/mod.rs` при инициализации и при обновлении настроек:
- Если указан `custom_font_path` и файл существует, байты шрифта загружаются и регистрируются в `egui::FontDefinitions`.
- Настраивается приоритет семейств шрифтов (`FontFamily::Monospace` / `FontFamily::Proportional`).
- Контекст обновляется через `context.set_fonts(...)`.

### 3.2 Масштабирование типографики редактора
В `crates/ui/src/native/paint.rs`:
`resolve_font_and_line_height(source: &str, typography: &TypographyConfig) -> (FontId, f32)`

Масштабирование от `base = typography.base_font_size`:
- `# Heading 1`: `size = (base * 1.8).round()`, `line_height = (size * 1.3).round()`
- `## Heading 2`: `size = (base * 1.45).round()`, `line_height = (size * 1.35).round()`
- `### Heading 3`: `size = (base * 1.25).round()`, `line_height = (size * 1.4).round()`
- `#### Heading 4`: `size = (base * 1.1).round()`, `line_height = (size * 1.45).round()`
- `##### Heading 5`: `size = (base * 1.0).round()`, `line_height = (size * 1.5).round()`
- `###### Heading 6`: `size = (base * 0.95).round()`, `line_height = (size * 1.5).round()`
- ```` ``` ```` / code: `size = (base * 0.9).round()`, семейство всегда `FontFamily::Monospace`, `line_height = (size * 1.5).round()`
- Основной текст: `size = base`, семейство по умолчанию `FontFamily::Monospace` (или `FontFamily::Proportional`), `line_height = (size * typography.line_height_multiplier).round()`

---

## 4. Интерфейс настроек и элементы управления

### 4.1 Компонент `SettingsDialog` (`crates/ui/src/native/creation/settings.rs`)
Включается в `CreationDialogs`:
- Модальное окно в едином стиле интерфейса Truff (рамка, отступы 20px, темная палитра).
- Поля:
  - Выбор шрифта (Monospace, Proportional, Custom).
  - Путь к файлу при выборе Custom.
  - Базовый размер шрифта (слайдер 10–32 px, кнопки `[-]` и `[+]`, сброс к 15 px).
  - Интерактивное превью текста.
  - Выбор темы оформления.
  - Кнопки: «Применить и сохранить» (`primary_button`), «Отмена» (`secondary_button`).

### 4.2 Векторная иконка и команды
- Векторная отрисовка `Icon::Gear` в `crates/ui/src/native/icons.rs`.
- Кнопка с иконкой `Icon::Gear` в шапке сайдбара `crates/ui/src/native/sidebar.rs`.
- Команды `:settings` и `:config` в `crates/ui/src/app/commands/appearance.rs`.

---

## 5. Тестирование и валидация
1. Модульные тесты `GlobalConfig` с `TypographyConfig` (дефолтные значения, TOML round-trip).
2. Тесты формул масштабирования `resolve_font_and_line_height`.
3. Тесты команд `:settings` и `:config`.
4. Полные проверки проекта:
   - `cargo check --workspace --all-targets`
   - `cargo test --workspace`
   - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
   - `cargo fmt --all -- --check`
