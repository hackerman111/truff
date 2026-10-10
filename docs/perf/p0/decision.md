# P0: смена frontend после замера настоящих окон

Дата: 2026-10-11. Пользователь выбрал egui/glow и максимальную оптимизацию.
Production frontend переводится на eframe/egui с glow. Сохраняются единственные
Workspace, Buffer, VimState и Application; Masonry frontend подлежит удалению.
Перенос и повторная native проверка ещё выполняются; этот документ не объявляет
P0–P5 или релиз завершёнными.

Предыдущее подтверждение Masonry отозвано: 13.8 MiB headless probe не измеряли
расход окна. Headless проверки Unicode/IME также не заменяют проверку событий
системного окна.

## Сравнение native процессов

Release, Linux Wayland, NVIDIA RTX 5060, драйвер 615.71.09. Это единичные
сравнительные пробы; они не являются p95 или полными perf workloads §10.3.
VmRSS взят из `/proc/<pid>/status`, PSS — из `smaps_rollup`.

| Окно | VmRSS, MiB | Ограничение сравнения |
| --- | ---: | --- |
| Masonry, backend по умолчанию, пустой vault | 584.6 | Загружены также Mesa/LLVM |
| Masonry, только Vulkan, пустой vault | 446.4 | Без дополнительных Mesa/LLVM |
| Masonry, Vulkan, около 1 MiB кириллицы и emoji | 490.3 | Реальное окно приложения |
| egui/glow, пустое окно, обычный EGL выбор | 308.3 | Загружены также Mesa/LLVM |
| egui/glow, пустое окно, NVIDIA EGL vendor | 131.4 | Сравнительный probe |
| egui/glow, около 1 MiB кириллицы и emoji, NVIDIA EGL vendor | 140.1 | Сравнительный probe; PSS 50.4 MiB |

Raw samples сохранены в [native-comparison](native-comparison/).
В старых samples `startup_seconds` включает ожидание стабилизации 3 секунды и
не является startup latency; `note_bytes` — запрошенный размер, генератор
округляет его вверх до целой Unicode строки. Исправленный инструмент записывает
фактический размер и называет время `elapsed_with_stabilization_seconds`.

Последние два замера использовали внешнюю настройку GLVND
`__EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/10_nvidia.json`.
Это условие замера на данном хосте, а не переносимый hardcoded путь приложения.
Нужен повторный замер production binary без неявных настроек окружения.

## Бюджет и обязательные проверки

Первоначальный RSS ≤100 MiB не выполнен ни одним проверенным native backend
на данном NVIDIA драйвере. Только отображённые библиотеки GPU compiler и EGL
занимают около 96 MiB RSS. По §10.2 план допускает обоснованный пересмотр P0;
для этого хоста выбран ориентир ≤160 MiB RSS на пустом окне/1 MiB probe.
Пока это целевой бюджет, не подтверждённый результат production приложения.

Обязательны сохранённые ограниченные layout-кэши, shaping видимой области
включая длинные строки, единый transactional ввод через core, изоляция IME
preedit, Unicode/grapheme hit testing, clipboard и keyboard-only проход.
Idle окно должно ожидать события; worker replies пробуждают его через callback,
а recovery/which-key используют следующий конкретный deadline.

Завершение P0 требует native проверки этих свойств на выбранном frontend и
повторных измерений всего production процесса. Остальные бюджеты §10 остаются
проверяемыми требованиями.
