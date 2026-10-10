# Truff v2

Нативный Markdown-редактор на Masonry. Обычный запуск открывает окно с vault,
редактором и backlinks. Текст принадлежит одному `Workspace`; клавиши редактора
проходят через `Shell` в `VimState`, а виджет отображает текущий `ViewId`.

```bash
rtk cargo run --locked -p truff-ui -- --vault /path/to/vault
```

Без `--vault` каталог открывается командой `:vault /path/to/vault` внутри окна.
Каталог должен существовать. Заметки появляются слева; первая заметка открывается
автоматически, при повторном запуске восстанавливается активная заметка с курсором
и прокруткой.

| Действие | Клавиши / команда |
| --- | --- |
| Вставка, обычный режим | `i`, `Esc` |
| Движения и правки Vim | `h/j/k/l`, `w/b/e`, `gg/G`, `dd`, `ciw`, `u`, `Ctrl-r` |
| Выделение | `v`, `V`; например `ve` выделяет слово |
| Перейти между панелями | `Ctrl-w h`, `Ctrl-w l` |
| Выбрать и открыть файл / backlink | `j/k`, `Enter` |
| Flash по словам видимой области | `s` в Normal, затем метка; `Esc` отменяет |
| Прокрутка | Колесо, `PageUp/PageDown` |
| Сохранить | `:w`, `Ctrl-s` |
| Открыть / создать заметку | `:e path.md`, `:new path.md` |
| Цвет выделенного текста | `:color #3388ff` |
| Сделать ссылку из выделения | `:link note` или `:link note#heading` |
| Открыть ссылку | `:follow` под курсором или `:follow note#heading` |
| Показать входящие ссылки | `:backlinks`; фокус переходит вправо |
| Переименовать с обновлением ссылок | `:rename new-name.md` после сохранения |
| Переместить заметку в `.truff/trash` | `:trash`; в панели vault — `d` |
| Тема / отображение | `:theme light`, `:theme dark`, `:source`, `:preview` |
| Закрыть вид заметки | `:q` / `:bd` для чистого буфера; `:wq` после подтверждения записи |

Цвет записывается обычным `<span style="color:#RRGGBB">…</span>`.
В Live Preview маркеры раскрываются возле курсора и выделения; цвет текста виден
в самом редакторе. Clipboard и IME используют атомарные транзакции того же буфера;
preedit не сохраняется как текст заметки.

Закрытие окна завершает принятые записи и сохраняет сессию. Несохранённые правки
попадают в recovery, не заменяя файл заметки. Состояние dirty снимается после
подтверждения записи. Rename блокируется, если затрагивает несохранённый буфер.

Проверки:

```bash
rtk cargo fmt --all -- --check
rtk cargo check --offline --locked --workspace --all-targets
rtk cargo test --offline --locked --workspace
rtk cargo clippy --offline --locked --workspace --all-targets --all-features -- -D warnings
rtk cargo build --offline --locked --release -p truff-ui
rtk proxy python3 tools/gui/native_smoke.py --binary target/release/truff
```

Последняя команда требует работающую сессию Hyprland с Lua API, `hyprctl` и `grim`.
Она создаёт временный vault, отправляет клавиши только своему окну, сохраняет
снимки и проверяет файлы после save/rename/restart. Раскладка клавиатуры временно
переключается на `us` и восстанавливается при выходе.

`--probe`, `--headless` и `--scenario` запускают отдельные headless-пробы.
Их результаты не измеряют цену нативного окна и presentation. Текущие доказательства
и незакрытые требования выпуска v1: [отчёт P7](docs/perf/p7/summary.md).
