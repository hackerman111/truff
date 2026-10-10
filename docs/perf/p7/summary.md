# P7 / T13: интеграция нативного приложения и границы проверки

Дата проверки: 2026-10-10. Выпуск v1 пока не подтверждён.
Прежнее утверждение «все бюджеты пройдены» отозвано: сохранённые
`samples.csv` и `metadata.toml` относятся к headless-компонентам. В старом
генераторе `PASS`, idle CPU и release checklist печатались без соответствующих
проверок, а отсутствующие значения подменялись числами.

## Что исправлено

Обычный `truff --vault /path/to/vault` создаёт настоящее дерево Masonry:
`AppWidget` → панель vault, `EditorWidget`, панель backlinks. Корневой виджет
принимает нативные события и передаёт клавиши редактора в `Shell::feed_key` /
`VimState`; clipboard и IME применяют транзакции к тому же `Workspace`.
`EditorWidget` отображает реальный `ViewId`, `Buffer` и `DisplayMap`, сохраняет
виртуализированный layout, рисует курсор, выделение, цвет и Flash. `EditorProbe`
остался только в явно выбранных headless-пробах.

Чтение, подтверждение записи, индекс и операции с файлами подключены к GUI.
Rename обновляет путь буфера и загруженные заметки с переписанными входящими
ссылками. Во время операции ввод заморожен. Состояние dirty снимается после
SaveAck, закрытие окна сохраняет recovery и сессию. Отменённая до savepoint
правка удаляет старый recovery через очередь worker'а; это проверяется до
закрытия окна. Перенос recovery при restart не переиспользует исторические
идентификаторы. Загрузки после rename отправляются постепенно с учётом
заполнения очереди, а dirty overlay backlinks существует только внутри запроса.

## Воспроизводимая проверка

```bash
rtk cargo build --offline --locked --release -p truff-ui
rtk proxy python3 tools/gui/native_smoke.py --binary target/release/truff
```

Нативный harness требует Hyprland с Lua API, `hyprctl`, `grim` и раскладку `us`.
Он запускает собственный процесс с новым временным vault, адресует события по
PID, восстанавливает раскладку и оставляет файлы/лог/снимки в `/tmp`.
Проверен сценарий:

1. Vault → переход `Ctrl-w h` в список файлов → `j`/`Enter` открывает другую
   заметку → `gg`/`Enter` возвращает первую; `Ctrl-w l/h` проходит остальные панели.
2. Insert/Esc → Flash → visual selection → `:color` → `:w`.
3. Выделение → `:link b` → `:w` → `:follow b#target` → `:backlinks` → Enter.
4. `:rename renamed.md` переписывает входящую ссылку в `b.md`; `:w` → закрытие
   через оконный менеджер → новый процесс с той же активной заметкой и текстом.

Файлы проверены автоматически. Снимки окна просмотрены отдельно:
[переключение панелей](native/panes.png), [цвет](native/color.png),
[backlinks](native/backlinks.png), [перезапуск](native/restart.png).
Это не OCR-проверка кадра и не измерение input-to-present.

`crates/ui/tests/gui.rs` проверяет события Masonry через production `RenderRoot`:
Normal `dd`/undo/`ciw`, IME без сохранения preedit и без двойного commit,
Unicode/paste, полный workflow, ограниченный cache при прокрутке 10k строк,
restore dirty/recovery и отменённой правки, блокировку cut во время rename.
Это проверка дерева виджетов без Wayland renderer. Старый `e2e_workflow.rs`
является headless backend fixture и сам по себе не доказывает работу окна.
`app_storage.rs` дополнительно проверяет перенос двух dirty snapshots и rename
с 60 resident backlinks; `store/tests/catalog.rs` проверяет переписывание ссылок
после запроса backlinks с отменённой dirty правкой.

Выполнены `fmt --check`, `check --workspace --all-targets`,
`test --workspace` (73 теста) и Clippy с `--all-targets --all-features -- -D warnings`.
Cargo check/test/clippy выполнены с `--offline --locked`; release build также прошёл.
Новые зависимости и изменения lockfile не потребовались.

## Нативная память и простой

Хост: Linux x86_64, i5-12400F, NVIDIA RTX 5060 / driver 615.71.09,
kernel 7.2.9-arch1-1; rustc 1.95.0. Release binary, Wayland, окно 956×1047,
две небольшие заметки, замер после restart. Граница — весь GUI-процесс.

| Показатель                           |  Запуск 1 |  Запуск 2 | Финальная сборка |
| ------------------------------------ | --------: | --------: | ---------------: |
| VmRSS (`/proc/status`)               | 587.7 MiB | 587.8 MiB |        588.3 MiB |
| RSS (`smaps_rollup`)                 | 446.0 MiB | 446.2 MiB |        446.6 MiB |
| PSS (`smaps_rollup`)                 | 265.6 MiB | 265.8 MiB |        266.2 MiB |
| Средний CPU за 30s, доля одного ядра |     1.53% |     0.20% |            0.20% |

Сырые данные: [запуск 1](native/release-sample-1.json),
[запуск 2](native/release-sample-2.json),
[финальная сборка](native/process-sample.json),
[метаданные и SHA-256 бинарника](native/metadata.json). VmRSS и smaps имеют разные результаты
учёта; приведены оба исходных показателя, без выбора меньшего для прохождения
бюджета. VRAM и CPU heap отдельно не измерялись. Эти три запуска не дают p95
и не являются полным набором §10.3.

Уже этот небольшой vault показывает превышение бюджета памяти нативного окна.
Простой в одном запуске превышает лимит 1%; повтор даёт меньшее значение.
Стабильное выполнение idle-бюджета ещё не доказано. Числа 30.68/17.67 MiB из
прежнего headless-отчёта не являются эквивалентным baseline для этого окна.
Оптимизацию или выполнение GUI memory/latency budgets не заявляем.

## Что остаётся до v1

- Разобрать стоимость GUI backend и подтвердить решение P0 с реальной памятью
  окна; текущий нативный memory gate не пройден.
- Пройти полный release workload §10.3: empty/10k/stress, latency distributions,
  input-to-present, indexing/search одновременно с вводом, HiDPI и font fallback.
- Подключить сохранение jump history и проверить всю матрицу восстановления
  видов/сессии; текущая интеграция сохраняет активный документ, курсор и scroll,
  но не записывает `jump_history`.

Генератор `--scenario` теперь сообщает только поставленные ему headless-метрики
и не выдаёт release сертификат. `tools/perf/measure.sh` по умолчанию пишет в
`docs/perf/p7/headless`, чтобы не заменять этот отчёт.
