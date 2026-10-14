# Store: измерения первого батча

В таблице приведены медианы и полный диапазон пяти парных запусков из CSV;
before/after запускались по очереди с чередованием порядка. Значения —
`ns_per_op`, переведённые в микросекунды на операцию.

| Сценарий | До, µs/операцию: медиана [min, max] | После, µs/операцию: медиана [min, max] |
|---|---:|---:|
| `resolve_basename_1000` | 141.952 [135.440, 144.000] | 4.646 [4.586, 4.863] |
| `fts_search_page_1000` | 708.672 [679.558, 743.678] | 713.613 [706.202, 835.512] |
| `reindex_single_in_1000` | 105.190 [102.306, 107.588] | 18.060 [17.651, 18.326] |
| `index_100_basename_links` | 15049.667 [14634.950, 15447.109] | 820.048 [787.489, 962.256] |
| `catalog_reopen_100_unchanged` | 44494.075 [43149.826, 47264.945] | 982.022 [835.932, 1012.713] |
| `save_dispatch_1mib` | 41.424 [40.760, 57.982] | 0.046 [0.045, 0.050] |

## Методика и воспроизведение

Соберите примеры в baseline checkout и checkout с изменениями, затем запускайте
их по очереди пять раз на версию, чередуя before/after. Исходный baseline —
`35f4cdef92155f56aa7a0dcf4bb8fc9dfb03729d`; baseline examples скопированы из
текущей версии, чтобы сохранить одинаковый harness. Не запускайте сборку и
другие тяжёлые измерения одновременно.

```sh
rtk proxy cargo build --release -p truff-store --examples --locked --offline
rtk proxy cargo build --release -p truff-core --example core_snapshot_perf --locked --offline
rtk proxy target/release/examples/store_perf
rtk proxy target/release/examples/catalog_perf
rtk proxy target/release/examples/core_snapshot_perf
```

`store_perf` строит in-memory индекс из 1000 небольших заметок; две операции
прогрева выполняются перед измерением, `TRUFF_PERF_ITERS` по умолчанию равен 1.
`catalog_perf` создаёт временный vault из 100 заметок, делает два прогревочных
открытия и измеряет пять повторных открытий. `core_snapshot_perf` измеряет 1000
операций dispatch для сохранения уже открытого буфера размером 1 MiB. CSV
содержат пять значений на сценарий.

## Ограничения

- Это warm-cache измерения, не оценка холодного дискового чтения. При старте
  каталога сканируются пути, читаются и хешируются все Markdown-файлы; после
  сверки fingerprint неизменённые документы пропускают повторную индексацию.
- У `fts_search_page_1000` диапазоны before/after перекрываются, поэтому эффект
  изменения на FTS-поиск по этим данным нейтрален.
- `save_dispatch_1mib` измеряет dispatch в `Workspace` для уже открытого буфера;
  снимок измеряет dispatch без файлового I/O и не замеряет запись файла.
- `store_perf --probe-window` подтвердил ошибку SQLite
  `unable to use function snippet in the requested context` для сочетания
  `COUNT(*) OVER()` с FTS5 `snippet`. Оставлены отдельные кэшированные запросы
  count и страницы со snippet; оба запроса используют `prepare_cached`.
- RSS и пиковое потребление памяти не измерялись.

Исходные данные: `before.csv` и `after.csv`. Корпуса и таймеры находятся в
`crates/store/examples/store_perf.rs`, `crates/store/examples/catalog_perf.rs`
и `crates/core/examples/core_snapshot_perf.rs`.

## Recovery snapshot writes

Финальное сравнение `recovery-before.csv` и `recovery-after.csv` использует
`recovery_perf`: 16 KiB content, 2 warmup writes и 100 измеренных записей с
пятью чередующимися парами запусков. Каждая операция увеличивает revision того
же buffer, удерживая не более двух валидных snapshot и выполняя cleanup. Измеряется весь
`write_snapshot`, включая budget scan, а не только fsync. `TMPDIR` направлен в
`$PWD/target/perf-io` на файловой системе проекта, не в `/tmp` tmpfs.

| Результат | До, ms/op: медиана [min, max] | После, ms/op: медиана [min, max] |
|---|---:|---:|
| `recovery_write_16k` | 5.227 [5.119, 5.376] | 5.087 [5.030, 5.275] |

Диапазоны перекрываются, устойчивый выигрыш не установлен. Сохранён первый
directory fsync после atomic rename; второй выполняется только если последующая
очистка изменила directory entries. Родительские fsync также сохранены.
Предыдущий эксперимент с кэшированием родителя (`recovery-parent-cache.csv` и
`recovery-without-cache.csv`) показал лишь около 0.8% при перекрывающихся
диапазонах, поэтому цену кэша — три fd/open/stats/mutex — не приняли.

RecoveryManager проверяет symlink на каждом ancestor пути recovery directory,
включая canonical vault root; покрытие — интеграционный тест
`recovery_directory`. Это не доказывает устойчивость к потере питания или все
crash-сценарии. RSS для store не измерялся.

```sh
rtk proxy cargo build --release -p truff-store --example recovery_perf --locked --offline
rtk proxy mkdir -p target/perf-io
rtk proxy env TMPDIR="$PWD/target/perf-io" target/release/examples/recovery_perf
```

Измерительный harness находится в `crates/store/examples/recovery_perf.rs`.
