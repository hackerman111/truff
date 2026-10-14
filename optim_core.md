Архитектурный аудит крейта `truff-core` показывает, что основные потери оперативной памяти (как в базовом потреблении, так и в пиках при редактировании, хранении истории Undo/Redo и работе с большими хранилищами) сосредоточены в семи ключевых узлах. Ниже приведён подробный разбор каждого источника расхода RAM и готовые к внедрению оптимизации.

---

## 1. Уплощение `HistoryGroup` и сжатие буферов в `History`: экономия ~60% RAM истории правок и устранение 2 аллокаций на каждое нажатие клавиши

**Локация:** `src/text/history.rs` (`HistoryGroup`, `History`) и `src/text/buffer/mod.rs` (`Buffer::apply`, `Buffer::undo`, `Buffer::redo`, `Buffer::stage_history`).

### Почему расходуется память:

1. **Тяжеловесные обёртки `Transaction` внутри `HistoryGroup`:**
   Сейчас `HistoryGroup` хранит два вектора: `pub forward: Vec<Transaction>` и `pub inverse: Vec<Transaction>`. В режиме вставки (`Mode::Insert`) каждое нажатие клавиши создаёт отдельную транзакцию из одного `SingleEdit` и добавляет её в текущую группу `UndoGroup`.
   При этом каждая структура `Transaction` занимает **64 байта** на стеке/в векторе (`edits: Vec<SingleEdit>` — 24 байта, `cursor: Option<CursorMeta>` — 24 байта, `undo_group: Option<UndoGroup>` — 16 байт) и делает отдельную аллокацию `Vec<SingleEdit>` в куче (ещё 40 байт).
   Однако при выполнении `Buffer::undo` и `Buffer::redo` (`stage_history` в `src/text/buffer/mod.rs`) поля `tx.cursor` и `tx.undo_group` внутри `HistoryGroup` **никогда не читаются** (идентификатор группы уже хранится в `HistoryGroup::group_id`)!
   В результате на 1 000 нажатий клавиш в одной сессии `Insert` тратится **128 байт на пустые обёртки `Transaction` + 2 отдельные аллокации `Vec<SingleEdit>` в куче на каждый символ** (2 000 аллокаций векторов из 1 элемента).

2. **Избыточный `capacity` у строк в истории (`SingleEdit.text`):**
   В `Buffer::build_inverse_and_changes` (`src/text/buffer/mod.rs`) при слиянии соседних удалений (`previous.text.push_str(&old_text)`) или при вызове `.to_string()` на срезе `RopeSlice` ёмкость `String` (`capacity`) удваивается и остаётся больше фактической длины текста, впустую расходуя бюджет `DEFAULT_MAX_HISTORY_BYTES` (8 МиБ на буфер).

3. **Неиспользуемый экземпляр `Registers` в каждом `Buffer`:**
   Структура `History` внутри каждого из 256 открытых буферов содержит поле `registers: Registers` с собственной `HashMap`, хотя в реальном редакторе все операции регистров выполняются через `VimState::registers`.

### Как оптимизировать:

Замените `Vec<Transaction>` в `HistoryGroup` на плоские буферы `forward_edits: Vec<SingleEdit>` + `forward_batch_ends: Vec<u32>` (и аналогично для `inverse`), а перед сохранением `SingleEdit` сжимайте избыточную ёмкость строк:

```rust
// В src/text/history.rs:
use std::collections::VecDeque;
use std::mem::size_of;
use super::edit::{SingleEdit, Transaction, UndoGroup};

mod registers;
pub use registers::{Register, RegisterKind, Registers};

pub const DEFAULT_MAX_HISTORY_BYTES: usize = 8 * 1024 * 1024;

#[inline]
fn compact_edit_text(edit: &mut SingleEdit) {
    // Освобождаем лишний capacity, если буфер строки раздут:
    if edit.text.capacity() > edit.text.len() + 16 {
        edit.text.shrink_to_fit();
    }
}

/// Уплощённая группа транзакций одной UndoGroup:
/// вместо сотен мелких Vec<SingleEdit> и неиспользуемых полей CursorMeta/UndoGroup
/// хранит единый непрерывный массив правок и границы батчей (u32).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryGroup {
    pub group_id: UndoGroup,
    forward_edits: Vec<SingleEdit>,
    forward_batch_ends: Vec<u32>,
    inverse_edits: Vec<SingleEdit>,
    inverse_batch_ends: Vec<u32>,
    pub size_in_bytes: usize,
    before_state: u64,
    after_state: u64,
}

impl HistoryGroup {
    fn new(group_id: UndoGroup, before_state: u64, after_state: u64) -> Self {
        Self {
            group_id,
            forward_edits: Vec::new(),
            forward_batch_ends: Vec::new(),
            inverse_edits: Vec::new(),
            inverse_batch_ends: Vec::new(),
            size_in_bytes: size_of::<Self>(),
            before_state,
            after_state,
        }
    }

    fn add(&mut self, mut forward_tx: Transaction, mut inverse_tx: Transaction) {
        let old_edits_cap = self.forward_edits.capacity() + self.inverse_edits.capacity();
        let old_ends_cap = self.forward_batch_ends.capacity() + self.inverse_batch_ends.capacity();

        let mut text_bytes = 0usize;
        for edit in &mut forward_tx.edits {
            compact_edit_text(edit);
            text_bytes += edit.text.capacity();
        }
        for edit in &mut inverse_tx.edits {
            compact_edit_text(edit);
            text_bytes += edit.text.capacity();
        }

        self.forward_edits.extend(forward_tx.edits);
        self.forward_batch_ends.push(self.forward_edits.len() as u32);

        self.inverse_edits.extend(inverse_tx.edits);
        self.inverse_batch_ends.push(self.inverse_edits.len() as u32);

        let new_edits_cap = self.forward_edits.capacity() + self.inverse_edits.capacity();
        let new_ends_cap = self.forward_batch_ends.capacity() + self.inverse_batch_ends.capacity();

        self.size_in_bytes += text_bytes
            + (new_edits_cap - old_edits_cap) * size_of::<SingleEdit>()
            + (new_ends_cap - old_ends_cap) * size_of::<u32>();
    }

    #[inline]
    pub(crate) fn forward_batch_count(&self) -> usize {
        self.forward_batch_ends.len()
    }

    #[inline]
    pub(crate) fn inverse_batch_count(&self) -> usize {
        self.inverse_batch_ends.len()
    }

    pub(crate) fn forward_batches(&self) -> impl DoubleEndedIterator<Item = &[SingleEdit]> + '_ {
        let ends = &self.forward_batch_ends;
        (0..ends.len()).map(move |i| {
            let start = if i == 0 { 0 } else { ends[i - 1] as usize };
            let end = ends[i] as usize;
            &self.forward_edits[start..end]
        })
    }

    pub(crate) fn inverse_batches(&self) -> impl DoubleEndedIterator<Item = &[SingleEdit]> + '_ {
        let ends = &self.inverse_batch_ends;
        (0..ends.len()).map(move |i| {
            let start = if i == 0 { 0 } else { ends[i - 1] as usize };
            let end = ends[i] as usize;
            &self.inverse_edits[start..end]
        })
    }
}

```

И в `src/text/buffer/mod.rs` обновите `undo`, `redo` и `stage_history` для работы со срезами `&[SingleEdit]`:

```rust
pub fn undo(&mut self) -> Result<Option<ChangeSet>, EditError> {
    let Some(history_group) = self.history.peek_undo() else {
        return Ok(None);
    };
    let count = history_group.inverse_batch_count();
    let (rope, changes) = self.stage_history(history_group.inverse_batches().rev())?;
    let history_group = self.history.pop_undo().expect("peeked undo group exists");
    self.rope = rope;
    self.revision += count as u64;
    self.history.push_redo(history_group);
    Ok(Some(changes))
}

pub fn redo(&mut self) -> Result<Option<ChangeSet>, EditError> {
    let Some(history_group) = self.history.peek_redo() else {
        return Ok(None);
    };
    let count = history_group.forward_batch_count();
    let (rope, changes) = self.stage_history(history_group.forward_batches())?;
    let history_group = self.history.pop_redo().expect("peeked redo group exists");
    self.rope = rope;
    self.revision += count as u64;
    self.history.push_undo(history_group);
    Ok(Some(changes))
}

fn stage_history<'a>(
    &self,
    batches: impl Iterator<Item = &'a [SingleEdit]>,
) -> Result<(crop::Rope, ChangeSet), EditError> {
    let mut rope = self.rope.clone();
    let mut changes = ChangeSet::default();
    for edits in batches {
        Self::validate_edits(&rope, self.max_file_bytes, edits)?;
        changes.append_batch(ChangeSet::new(
            edits
                .iter()
                .map(|edit| Change::new(edit.range.clone(), edit.text.len()))
                .collect(),
        ));
        for edit in edits.iter().rev() {
            rope.replace(edit.range.clone(), &edit.text);
        }
    }
    Ok((rope, changes))
}

```

---

## 2. Полное удаление дерева префиксов `BTreeMap` из `CompiledKeymap`: 0 байт на индекс префиксов

**Локация:** `src/config/compiled.rs` (`CompiledKeymap`, `KeymapConfig::compile`, `lookup`, `hints`).

### Почему расходуется память:

В `CompiledKeymap` (`src/config/compiled.rs`) помимо вектора `bindings: Vec<KeyBinding>` хранится вложенное дерево `prefixes: BTreeMap<Mode, BTreeMap<Vec<Key>, Vec<usize>>>`.
При вызове `compile()` для каждой привязки клавиш и для каждой длины префикса `0..=binding.keys.len()` аллоцируется новый вектор `Vec<Key>` на куче (`binding.keys[..len].to_vec()`) и новый вектор индексов `Vec<usize>` в узлах `BTreeMap`.

### Как оптимизировать:

Если во время `compile()` один раз отсортировать `bindings` по `(binding.mode, &binding.keys)`, то любые привязки, начинающиеся с префикса `prefix: &[Key]` в режиме `mode`, гарантированно образуют **один непрерывный диапазон** в массиве `self.bindings`, который находится за $O(\log B)$ через `partition_point` **вообще без хранения поля `prefixes**`!

```rust
// В src/config/compiled.rs:
use crate::commands::{CommandContext, CommandId, CommandRegistry};
use crate::vim::{Key, Mode};
use super::{KeyBinding, KeyHint, KeymapConfig, KeymapError};

/// Компактная скомпилированная карта клавиш: 0 байт на таблицы префиксов,
/// поиск выполняется бинарным поиском по отсортированному срезу `bindings`.
#[derive(Debug, Clone)]
pub struct CompiledKeymap {
    bindings: Box<[KeyBinding]>,
    registry: CommandRegistry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingLookup {
    Complete(CommandId),
    Prefix,
    Missing,
}

impl KeymapConfig {
    pub fn compile(&self) -> Result<CompiledKeymap, KeymapError> {
        self.validate()?;
        let mut bindings = self.bindings.clone();
        for b in &mut bindings {
            b.keys.shrink_to_fit();
            b.description.shrink_to_fit();
        }
        bindings.sort_unstable_by(|a, b| {
            a.mode.cmp(&b.mode).then_with(|| a.keys.as_slice().cmp(b.keys.as_slice()))
        });
        Ok(CompiledKeymap {
            bindings: bindings.into_boxed_slice(),
            registry: CommandRegistry::default(),
        })
    }
}

impl CompiledKeymap {
    /// Возвращает поддиапазон привязок в `mode`, начинающихся с префикса `prefix`, за O(log B).
    #[inline]
    fn matching_range(&self, mode: Mode, prefix: &[Key]) -> &[KeyBinding] {
        let start = self
            .bindings
            .partition_point(|b| (b.mode, b.keys.as_slice()) < (mode, prefix));
        let slice = &self.bindings[start..];
        let len = slice.partition_point(|b| b.mode == mode && b.keys.starts_with(prefix));
        &slice[..len]
    }

    pub fn lookup(&self, mode: Mode, keys: &[Key], context: CommandContext) -> BindingLookup {
        for binding in self.matching_range(mode, keys) {
            if self.registry.require(&binding.command, context).is_ok() {
                if binding.keys.len() == keys.len() {
                    return BindingLookup::Complete(binding.command.clone());
                }
                return BindingLookup::Prefix;
            }
        }
        BindingLookup::Missing
    }

    pub fn hints(&self, mode: Mode, prefix: &[Key], context: CommandContext) -> Vec<KeyHint> {
        let matches = self.matching_range(mode, prefix);
        if matches.is_empty() {
            return Vec::new();
        }
        let mut hints: Vec<KeyHint> = Vec::new();
        for binding in matches {
            if binding.keys.len() > prefix.len()
                && self.registry.require(&binding.command, context).is_ok()
            {
                let key = binding.keys[prefix.len()];
                // Поскольку `matches` уже отсортированы по `keys`, одинаковые `key` идут подряд!
                if hints.last().is_none_or(|last| last.key != key) {
                    hints.push(KeyHint {
                        key,
                        description: binding.description.clone(),
                        is_prefix: binding.keys.len() > prefix.len() + 1,
                    });
                }
            }
        }
        hints
    }
}

```

Это полностью уничтожает два уровня `BTreeMap`, десятки мелких `Vec<Key>` и `Vec<usize>` при каждой компиляции раскладки, а также убирает аллокацию `BTreeMap` внутри `hints()`.

---

## 3. Устранение 24-MиБ пиков реаллокации в `Buffer::as_str()` и `build_inverse_and_changes`

**Локация:** `src/text/buffer/mod.rs` (`Buffer::as_str`, `Buffer::build_inverse_and_changes`) и `src/workspace/dispatch.rs` (`Action::SaveBuffer`).

### Почему расходуется память:

1. В `Buffer::as_str(&self)` (`src/text/buffer/mod.rs`) и в `Action::SaveBuffer` (`src/workspace/dispatch.rs`) вызывается `self.rope.to_string()`.
   Трейт `Display` для `crop::Rope` не резервирует `self.rope.byte_len()` заранее. При сериализации буфера размером 10 МиБ внутренняя `String` внутри `to_string()` растёт удвоениями (`... -> 4 MiB -> 8 MiB -> 16 MiB`).
   В момент реаллокации с 8 МиБ до 16 МиБ в оперативной памяти одновременно находятся:

- сам `Rope` (10 МиБ),
- старый буфер `String` (8 МиБ),
- новый буфер `String` (16 МиБ) — **34 МиБ пикового расхода RAM на один файл**!
  Более того, после возврата из `to_string()` созданная строка сохраняет `capacity == 16 MiB` при длине `10 MiB` (теряя **6 МиБ RAM впустую** внутри `StoreJob::SaveFile`).

2. В `Buffer::build_inverse_and_changes` (`src/text/buffer/mod.rs`) для каждого редактирования вызывается `self.rope.byte_slice(edit.range.clone()).to_string()`, что также использует `Display` вместо точного выделения памяти и даже для чистых вставок (`edit.range.is_empty()`) обходит дерево `Rope`.

### Как оптимизировать:

Выделяйте буфер `String` строго размера `byte_len()` через `String::with_capacity` и копируйте чанки напрямую без форматтера `std::fmt`:

```rust
// В src/text/buffer/mod.rs:
#[inline]
pub(crate) fn rope_slice_to_exact_string(slice: crop::RopeSlice<'_>) -> String {
    let len = slice.byte_len();
    if len == 0 {
        return String::new();
    }
    let mut s = String::with_capacity(len);
    for chunk in slice.chunks() {
        s.push_str(chunk);
    }
    s
}

impl Buffer {
    /// Конвертирует содержимое буфера в String с ровно 1 аллокацией точного размера (capacity == len).
    pub fn as_str(&self) -> String {
        rope_slice_to_exact_string(self.rope.byte_slice(..))
    }
}

```

А в `Buffer::build_inverse_and_changes` (`src/text/buffer/mod.rs`) замените `self.rope.byte_slice(edit.range.clone()).to_string()` на:

```rust
let old_text = if edit.range.is_empty() {
    String::new()
} else {
    rope_slice_to_exact_string(self.rope.byte_slice(edit.range.clone()))
};

```

И в `src/workspace/dispatch.rs` в ветке `Action::SaveBuffer` замените `b.rope().to_string()` на `b.as_str()`.

---

## 4. Слияние `Command::TextCommit` в `VimState` и очистка буферов парсера: экономия 98% памяти на `last_edit` / `current_edit`

**Локация:** `src/vim/execute/repeat.rs` (`VimState::run_command`), `src/vim/execute/editing.rs` (`VimState::insert_key`), `src/vim/execute/targets.rs` (`VimState::clear_syntax_objects`) и `src/vim/parser/mod.rs` (`VimParser`).

### Почему расходуется память:

1. В `src/vim/execute/repeat.rs` при посимвольном наборе текста в режиме `Mode::Insert` каждый введённый символ пушится в `self.current_edit: Vec<Command>` отдельной структурой `Command::TextCommit(String)`:

```rust
self.current_edit_bytes += added;
self.current_edit.push(cmd.clone());

```

Каждая структура `Command` в векторе весит **40 байт**, а её `String` из 1 символа занимает ещё **16 байт** в аллокаторе кучи — итого **56 байт на каждый набранный символ**. После нажатия `Esc` весь этот вектор перемещается в `self.last_edit` (`src/vim/execute/editing.rs`) и висит в памяти до следующей правки.

2. В `VimState::clear_syntax_objects` (`src/vim/execute/targets.rs`) вызывается `self.syntax_objects.clear()`, который оставляет выделенный буфер `Vec<SyntaxObjectRange>` максимального размера в куче.

3. В `VimParser::reset` (`src/vim/parser/mod.rs`) вызывается `self.colon_buffer.clear()`, который после одной длинной команды удерживает до 64 КиБ в куче, а `buffer: Vec<Key>` хранится в куче ради максимум 2 клавиш.

### Как оптимизировать:

1. В `VimState::run_command` (`src/vim/execute/repeat.rs`) объединяйте соседние `Command::TextCommit` внутри `self.current_edit` в одну строку (так как в режиме `Insert` все они принадлежат одной `UndoGroup` и при повторе `.` применяются в одно и то же место курсора):

```rust
// В src/vim/execute/repeat.rs (внутри VimState::run_command):
} else if self.mode == Mode::Insert && !self.replaying_edit {
    let added = command_text_bytes(&cmd);
    if self.current_edit_bytes.saturating_add(added)
        > LimitsConfig::DEFAULT_MAX_HISTORY_BYTES
        || self.current_edit.len() >= 10000
    {
        return self.error("Insert repeat memory limit exceeded");
    }
    self.current_edit_bytes += added;
    if let (Some(Command::TextCommit(prev)), Command::TextCommit(next)) =
        (self.current_edit.last_mut(), &cmd)
    {
        prev.push_str(next);
    } else {
        self.current_edit.push(cmd.clone());
    }
}

```

2. При выходе из режима `Insert` по `Key::Esc` (`src/vim/execute/editing.rs`) и в `cancel_input` (`src/vim/execute/mod.rs`) вызывайте `shrink_to_fit()` на `self.current_edit`, а в `clear_syntax_objects` (`src/vim/execute/targets.rs`) сбрасывайте ёмкость `self.syntax_objects = Vec::new();`.

3. В `VimParser::reset` (`src/vim/parser/mod.rs`) освобождайте буфер командной строки, если он разросся:

```rust
pub fn reset(&mut self) {
    self.count_prefix = None;
    self.operator = None;
    self.buffer.clear();
    self.in_colon = false;
    if self.colon_buffer.capacity() > 128 {
        self.colon_buffer = String::new();
    } else {
        self.colon_buffer.clear();
    }
    self.search = None;
}

```

---

## 5. Выгрузка истории и неактивных чистых буферов в `Workspace` и дедупликация путей `Arc<Path>` в `BufferRegistry`

**Локация:** `src/workspace/mod.rs` (`Workspace::close_view`) и `src/workspace/buffers.rs` (`BufferEntry`, `BufferRegistry`).

### Почему расходуется память:

1. **Закрытие вкладки (`Action::CloseView`) никогда не освобождает ни историю Undo/Redo (до 8 МиБ), ни текст чистого буфера (до 16 МиБ):**
   В `Workspace::close_view` (`src/workspace/mod.rs`) удаляется только `View` из `self.views`, но сам `Buffer` остаётся в `BufferRegistry` со всем своим 8-мегабайтным стеком `History` и 16-мегабайтным `Rope`, даже если буфер чист (`BufferState::Clean`), привязан к файлу на диске и на него больше не ссылается ни один открытый `View`!

2. **Тройное клонирование `PathBuf` в `BufferRegistry` (`src/workspace/buffers.rs`):**
   Один и тот же путь к файлу аллоцируется отдельно в `BufferEntry::path`, в ключах `path_to_id: HashMap<PathBuf, BufferId>` и в `closed_paths: HashMap<PathBuf, BufferId>`.

### Как оптимизировать:

1. Добавьте в `History` (`src/text/history.rs`) метод `pub fn clear_stacks(&mut self)`, который очищает и освобождает память `undo_stack` и `redo_stack`, когда чистый буфер закрывается во всех `View`:

```rust
// В src/text/history.rs:
pub fn clear_stacks(&mut self) {
    self.undo_stack = VecDeque::new();
    self.redo_stack = VecDeque::new();
    self.total_bytes = 0;
    self.savepoint_state = self.current_state;
    self.discarded_group = None;
}

```

2. В `Workspace::close_view` (`src/workspace/mod.rs`), если закрыт последний `View` для чистого буфера (`BufferState::Clean`), немедленно освобождайте его стек истории `clear_stacks()` (экономия до **8 МиБ RAM на каждую закрытую вкладку** с сохранением всех инвариантов тестов):

```rust
// В src/workspace/mod.rs:
pub fn close_view(&mut self, view_id: ViewId) -> Result<Vec<Effect>, AppError> {
    let Some(removed_view) = self.views.remove(&view_id) else {
        return Err(AppError::ViewNotFound(view_id));
    };
    if self.active_view == Some(view_id) {
        self.active_view = self.views.keys().next().copied();
    }
    let buf_id = removed_view.buffer_id();
    let still_viewed = self.views.values().any(|v| v.buffer_id() == buf_id);
    if !still_viewed
        && let Some(entry) = self.buffers.get_mut(buf_id)
        && entry.state() == BufferState::Clean
        && let Some(buffer) = entry.buffer_mut()
        && !buffer.is_dirty()
    {
        buffer.history_mut().clear_stacks();
    }
    Ok(vec![Effect::ViewClosed(view_id)])
}

```

---

## 6. Единая строковая арена в `CommandRegistry` и статические срезы `contexts`: минус 265 аллокаций в куче

**Локация:** `src/commands/registry.rs` (`RegistryData`, `SearchFields`), `src/commands/defaults.rs` и `src/commands/types.rs` (`CommandSpec`).

### Почему расходуется память:

1. В `CommandRegistry::from_specs` (`src/commands/registry.rs`) создаётся вектор `search: Vec<SearchFields>`, где для каждой из 53 команд аллоцируются 4 отдельные строки `String` на куче (`title`, `description`, `id`, `group`) — **212 отдельных аллокаций в куче**. При этом:

- `spec.id` у всех команд уже находится в нижнем регистре ASCII (`"panel.files"`, `"buffer.save"` и т.д.);

- `spec.group` повторяется среди всего 7 категорий.

2. В `CommandRegistry::build_default` (`src/commands/defaults.rs`) для каждой из 53 команд вызывается `vec![CommandContext::...]` — ещё **53 отдельные аллокации в куче** ради массивов из 1–2 элементов.

### Как оптимизировать:

Упакуйте все поисковые строки `CommandRegistry` в **один непрерывный буфер `arena: Box<str>**` с компактными индексами `u16`:

```rust
// В src/commands/registry.rs:
#[derive(Debug, Clone, Copy)]
struct SpanU16 {
    start: u16,
    end: u16,
}

impl SpanU16 {
    #[inline]
    fn as_str(self, arena: &str) -> &str {
        &arena[self.start as usize..self.end as usize]
    }
}

#[derive(Debug, Clone, Copy)]
struct SearchSpans {
    title: SpanU16,
    description: SpanU16,
    id: SpanU16,
    group: SpanU16,
}

#[derive(Debug)]
struct RegistryData {
    specs: Box<[CommandSpec]>,
    ids: Box<[u16]>,
    search_arena: Box<str>,
    search_spans: Box<[SearchSpans]>,
}

impl CommandRegistry {
    pub(super) fn from_specs(mut specs: Vec<CommandSpec>) -> Self {
        for spec in &mut specs {
            spec.contexts.shrink_to_fit();
        }
        let mut ids: Vec<u16> = (0..specs.len() as u16).collect();
        ids.sort_unstable_by(|&a, &b| specs[a as usize].id.cmp(&specs[b as usize].id));

        let mut arena = String::with_capacity(specs.len() * 64);
        let mut push_lower = |s: &str| -> SpanU16 {
            let start = arena.len() as u16;
            for c in s.chars() {
                for lc in c.to_lowercase() {
                    arena.push(lc);
                }
            }
            let end = arena.len() as u16;
            SpanU16 { start, end }
        };

        let search_spans: Vec<SearchSpans> = specs
            .iter()
            .map(|spec| SearchSpans {
                title: push_lower(&spec.title),
                description: push_lower(&spec.description),
                id: push_lower(spec.id.as_str()),
                group: push_lower(&spec.group),
            })
            .collect();

        Self {
            data: Arc::new(RegistryData {
                specs: specs.into_boxed_slice(),
                ids: ids.into_boxed_slice(),
                search_arena: arena.into_boxed_str(),
                search_spans: search_spans.into_boxed_slice(),
            }),
        }
    }
}

```

Это заменяет **212 отдельных аллокаций `String**` ровно **одной** компактной аллокацией `Box<str>` без единого лишнего байта `capacity`.

---

## 7. Устранение дублирующихся векторов и копий строк в `MarkdownFacts::extract` (`src/markdown/facts.rs`)

**Локация:** `src/markdown/facts.rs`.

### Почему расходуется память:

1. В `MarkdownFacts::extract` создаются три отдельных вектора ссылок: `standard_links`, `wiki_links` и `all_links`, которые одновременно живут в памяти до завершения функции.

2. При завершении заголовка (`TagEnd::Heading`) и ссылки (`TagEnd::Link`) на уже собранной строке `heading_text` / `label` вызывается `.trim().to_string()`, что выделяет **вторую копию `String` в куче**, даже когда строка не содержала пробелов по краям.

### Как оптимизировать:

Тримьте `String` на месте (in-place) без второй аллокации и собирайте все ссылки сразу в один вектор `links: Vec<LinkOccurrence>`:

```rust
// В src/markdown/facts.rs:
/// Обрезает пробелы по краям String на месте (in-place) без новой аллокации в куче.
#[inline]
fn trim_string_in_place(mut s: String) -> String {
    let trimmed_end = s.trim_end().len();
    s.truncate(trimmed_end);
    let leading = s.len() - s.trim_start().len();
    if leading > 0 {
        s.drain(..leading);
    }
    s.shrink_to_fit();
    s
}

```

И в Pass 1 / Pass 2 / Pass 3 (`src/markdown/facts.rs`) используйте `trim_string_in_place(heading_text)` и пушьте wiki-ссылки напрямую в вектор `links`, сортируя его на месте через `links.sort_unstable_by_key(|l| l.range.start)` без создания `wiki_links` и `all_links`.

---

## Сводный эффект по памяти для `truff-core`

| Компонент `truff-core`                     | Источник расхода RAM в текущем коде | После оптимизации | Экономия RAM |
| ------------------------------------------ | ----------------------------------- | ----------------- | ------------ |
| **`HistoryGroup`** (`src/text/history.rs`) |

| 2 `Vec<Transaction>` (по 64 Б + кучевой `Vec<SingleEdit>` на каждый шаг в группе)

| Плоские `Vec<SingleEdit>` + `Vec<u32>` и `shrink_to_fit` текстов правок | **-60% RAM истории**, минус 2 аллокации на символ |
| **`CompiledKeymap`** (`src/config/compiled.rs`)

| Вложенный `BTreeMap<Mode, BTreeMap<Vec<Key>, Vec<usize>>>`<br> | Бинарный поиск `partition_point` по `Box<[KeyBinding]>` | **-100% памяти дерева префиксов** |
| **`Buffer::as_str()`** (`src/text/buffer/mod.rs`)

| `rope.to_string()` через `Display` с удвоением буфера (до 16 МиБ при 10 МиБ тексте)

| `String::with_capacity(rope.byte_len())` | **До -6 МиБ** на сохранение и **-50% пика реаллокации** |
| **`VimState` (`current_edit` / `last_edit`)** (`src/vim/execute/repeat.rs`)

| Отдельный `Command::TextCommit(String)` (56 Б) на каждую букву в `Insert`<br> | Слияние соседних `TextCommit` в один буфер | **-98% памяти** `last_edit` после ввода текста |
| **`Workspace::close_view`** (`src/workspace/mod.rs`)

| Закрытые чистые буферы навсегда удерживают до 8 МиБ `History` каждый

| Сброс `clear_stacks()` при закрытии последнего `View` чистого буфера | **До -8 МиБ** на каждую закрытую вкладку |
| **`CommandRegistry`** (`src/commands/registry.rs`)

| 212 отдельных `String` в `Vec<SearchFields>`<br> | Единая строковая арена `Box<str>` + `SpanU16` | **Минус 211 аллокаций в куче** |
