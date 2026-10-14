# Core: оптимизации и измерения

Измерено 2026-10-14 на Intel Core i5-12400F, Linux x86_64, rustc 1.95.0
(59807616e), release без дополнительных RUSTFLAGS. Исходная версия:
`35f4cdef92155f56aa7a0dcf4bb8fc9dfb03729d`. Новая версия — рабочий diff.

Сравнение выполнено последовательно: пять пар запусков, порядок before/after
чередуется, `TRUFF_PERF_ITERS=10`. Каждый сценарий имеет два прогревочных вызова
(у state-сценариев — два полных пакета). Компиляция и другие benchmark не шли
параллельно. CPU affinity/governor не фиксировались, поэтому небольшие различия
нельзя считать доказанным выигрышем. Таблица показывает медианы пяти запусков;
диапазоны и исходные значения доступны в CSV.

| Сценарий | До, мкс/операцию | После, мкс/операцию | Отношение до/после |
|---|---:|---:|---:|
| Forward search, count=1, около EOF | 1298 | 0.753 | 1723× |
| Backward search, count=50, около EOF | 67665 | 1.828 | 37016× |
| Markdown facts, 5000 code ranges | 23266 | 2517 | 9.24× |
| Default keymap + validate, прогретый registry | 3.765 | 2.278 | 1.65× |
| Left, count=2000 | 265.9 | 177.3 | 1.50× |
| Right, count=2000 | 492.5 | 414.8 | 1.19× |
| Incremental syntax, правка в конце | 807.4 | 787.0 | 1.03×, диапазоны пересекаются |
| Создание ChangeSet из 5000 edits | 2.786 | 7.707 | 0.36×, регрессия |
| Один map_pos, пакет из 512 anchors | 0.804 | 0.021 | 38.3× |
| Вставка через Buffer с history limit 128 KiB | 0.185 | 0.185 | 1.00× |
| Запись именованного регистра, 64 KiB | 2.586 | 1.306 | 1.98× |

Поиск использует документ 1.5 MiB; рядом с курсором есть нужные совпадения.
Он теперь останавливается после count совпадений, вместо полного сканирования
документа на каждое повторение. Эти множители относятся к данному расположению
совпадений: поиск отсутствующей строки всё ещё требует полного прохода.
Перекрытия, wrap, Unicode и огромные count проверяются отдельно по reference.
Рабочие массивы ограничены длиной запроса, все совпадения не собираются.

Проверка facts использует 5000 строк с inline code и wiki links, движения —
одну строку ASCII длиной 10000 байтов. Syntax-сценарий правит последний символ
документа около 9 KiB; обе Rope версии приготовлены до таймера. Изменён API:
старые byte/point координаты передаются явно до изменения текста, parser читает
Rope чанками. У IncrementalSyntaxTree пока нет production caller в редакторе;
результат не является измерением задержки печати или FPS.

Индекс ChangeSet добавляет O(edits) предварительную работу и один isize на edit
для отсортированных непересекающихся batches от 32 edits: 40000 байтов полезных
данных при 5000 edits, плюс поле Vec в ChangeSet. Малые batches не аллоцируют
индекс. На данном workload создание дороже примерно на 4.9 мкс, окупаемость —
приблизительно семь map_pos при сходном распределении anchors. Это полезно при
rebasing jump history: до 256 пар anchor/head на view. Для одной каретки и
большого batch предварительная работа может быть дороже линейного поиска.
Границы соседних edits и последовательные координаты grouped undo/redo
сверяются с линейной моделью.

Изменение shrink-политики истории не дало измеримого ускорения в этом сценарии.
Оно сохраняет capacity до значительного избытка, а общий лимит по-прежнему
учитывает свободные slots. Около лимита это может вытеснить немного больше
старых undo groups. Ускорение или экономия памяти для истории не заявляются.

Изменение регистров сохраняет публичный Register с String; именованный и
безымянный register разделяют Arc, append использует copy-on-write. Paste
держит shared register и собирает результат с заранее рассчитанной capacity.
Это не измерение peak RSS: allocator/heap/RSS отдельно не профилировались.

## Воспроизведение

```sh
rtk proxy cargo build --release -p truff-core --examples --locked --offline
rtk proxy env TRUFF_PERF_ITERS=10 target/release/examples/core_perf
rtk proxy env TRUFF_PERF_ITERS=10 target/release/examples/core_state_perf
```

`core_perf` принимает имя сценария первым аргументом. Корпуса и границы таймера
заданы в `crates/core/examples/core_perf.rs` и `core_state_perf.rs`; библиотеки
для benchmarking не добавлены. Для before нужно создать отдельный checkout
исходного commit, скопировать examples и адаптировать только syntax-вызов к
старому API: `edit_and_parse(edit_start, 1, 1, text)`, где text попеременно
`syntax_alternate` и `syntax_initial`. В baseline points вычислял сам старый API.
Остальные сценарии используют одинаковый код и входные данные.

`baseline.csv` — первоначальный замер до изменений, по одному базовому пакету.
Для таблицы использованы повторные парные замеры с множителем 10:
`paired-before.csv`, `after.csv`, `state-before.csv`, `state-after.csv`.

## Текущее состояние и проверки

`BufferChange` подключён к UI layout cache, а clean закрытые buffers выгружаются
после подтверждённого удаления recovery snapshot. Защиты retention учитывают
dirty/conflict buffers, ожидающие save/load, queued reload/recovery, pending
recovery jobs и незавершённую file operation. Выгрузка ждёт cleanup ack для
текущего snapshot key; при повторном открытии тот же `BufferId` сохраняет
revision floor и отвергает запоздавший ответ загрузки с более старой ревизией.
Layout cache сверяет число объявленных edit batches с изменением revision:
пропущенное уведомление, даже перед очередной обычной правкой, вызывает полный
rebuild вместо повторного использования устаревших строк.

Итоговый прогон после исправления revision-gap: `rtk cargo fmt --all -- --check`,
`rtk cargo check --workspace --all-targets`, `rtk cargo test --workspace`
(257 tests, 53 suites) и `rtk cargo clippy --workspace --all-targets --all-features
-- -D warnings` прошли. `rtk git diff --check` также прошёл. Состояния остальных
пунктов см. в `../optimization-inventory.md`.

## Resident memory после закрытия заметок

Пять парных чередующихся запусков используют один HEAD-compatible UI example:
32 Markdown заметки по 512 KiB, каждая открывается, приложение дожидается idle,
закрывает view и снова дожидается idle. RSS/PSS читаются из Linux
`/proc/self/smaps_rollup` после цикла и до shutdown. Результат отражает headless
процесс целиком; это не GUI baseline, peak memory или оценка редактора для всех
нагрузок. Один пустой startup scratch buffer остаётся resident: всего buffers
33 → 1, тогда как текст заметок уменьшается с 16 MiB до 0.

| Метрика | До, MiB: медиана [min, max] | После, MiB: медиана [min, max] |
|---|---:|---:|
| RSS | 39.848 [39.742, 39.879] | 20.344 [20.332, 20.352] |
| PSS | 37.419 [37.345, 37.488] | 17.896 [17.889, 17.903] |

```sh
rtk proxy cargo build --release -p truff-ui --example buffer_residency_perf --locked --offline
rtk proxy env XDG_CONFIG_HOME=/tmp/truff-perf-xdg target/release/examples/buffer_residency_perf
```

Перед каждым запуском создайте отдельный пустой `XDG_CONFIG_HOME`; example
создаёт и удаляет собственный временный vault. CSV: `residency-before.csv` и
`residency-after.csv`; источник workload —
`crates/ui/examples/buffer_residency_perf.rs`.

## Подсказки CompiledKeymap

Порядок кандидатов по следующей клавише и доступность команд в трёх контекстах
определяются один раз при компиляции. Выдача подсказок идёт одним проходом;
описание клонируется только для первого доступного binding каждой клавиши.
Публичный изменяемый `KeymapConfig` не содержит кэша. Reference-тест сверяет
lookup и hints с исходным порядком bindings, включая обратный порядок, все
контексты и команду Global.

Пять парных запусков чередовали baseline/current. В `hints_perf` конфигурация
создана вне таймера: после 100 прогревов измерены 100000 чередующихся вызовов
для пустого префикса и `g`; отдельный сценарий делает 100 прогревов и 1000
компиляций той же конфигурации.

| Сценарий | До, ns/op: медиана [min, max] | После, ns/op: медиана [min, max] |
|---|---:|---:|
| hints | 604 [598, 631] | 235 [225, 240] |
| compile | 8245 [8100, 8576] | 7673 [7399, 8177] |

Выдача hints ускорилась примерно в 2.57 раза на стандартной конфигурации.
Для compile диапазоны пересекаются. Первый вариант с сортировкой временного
Vec при каждом вызове был отклонён: 1014 [1006, 1078] ns/op против исходных
633 [613, 658] в том прогоне. Вместо него упорядочивание перенесено в compile.
Сохранён CSV отклонённого варианта `hints-per-call-sort.csv`; финальные парные
данные находятся в `hints-before.csv` и `hints-after.csv`.

```sh
rtk proxy cargo build --release -p truff-core --example hints_perf --locked --offline
rtk proxy target/release/examples/hints_perf
```

## History edit capacity

The capacity example applies one transaction to each of 1000 fresh buffers;
buffer and transaction setup are outside the timer. The timed section includes
`Buffer::apply` and reading `history.total_bytes()`. Five paired runs alternated
baseline/current order. Baseline is
`35f4cdef92155f56aa7a0dcf4bb8fc9dfb03729d`; both checkouts use the same copied
example harness.

| Scenario | Before, ns/op median [min, max] | After, ns/op median [min, max] | History bytes before → after |
|---|---:|---:|---:|
| Oversized text (1 MiB capacity, 6-byte `字abc`) | 159 [157, 218] | 1859 [1365, 2326] | 1049608 → 1038 |
| Ordinary text | 157 [154, 166] | 169 [164, 205] | 1040 → 1040 |

The oversized case saves retained history capacity but adds about 1.7 µs median
per apply in this microbenchmark. Ordinary timings are close and their ranges
overlap; this is not evidence of an ordinary-edit speedup. The benchmark does
not measure process RSS or peak allocation.

```sh
rtk proxy cargo build --release -p truff-core --example history_capacity_perf --locked --offline
rtk proxy target/release/examples/history_capacity_perf
```

Repeat five times per checkout in alternating order. Results are in
`capacity-before.csv` and `capacity-after.csv`; timer and workload are defined
in `crates/core/examples/history_capacity_perf.rs`.
