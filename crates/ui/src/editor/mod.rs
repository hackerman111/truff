pub mod display;
pub mod input;
pub mod layout;
pub mod paint;

pub use display::{Affinity, DisplayChunk, DisplayMap, ProjectionMode, SourceHit, VisualPos};
pub use input::{EditorInput, InputAdapter};
pub use layout::{
    BlockInfo, EditorLayout, HeightIndex, ScrollAnchor, VirtualizedLayout,
    snap_to_grapheme_boundary,
};
pub use paint::EditorPainter;

use crop::Rope;
use masonry::core::keyboard::{Key, KeyState, NamedKey};
use masonry::core::{
    AccessCtx, BoxConstraints, ChildrenIds, EventCtx, LayoutCtx, PaintCtx, PointerButton,
    PointerEvent, PropertiesMut, PropertiesRef, RegisterCtx, TextEvent, Update, UpdateCtx, Widget,
};
use masonry::kurbo::{Point, Rect, Size};
use masonry::vello::Scene;
use unicode_segmentation::UnicodeSegmentation;

use crate::theme::Theme;

/// Retained editor probe holding text buffer, selection, IME state, and visual layout.
pub struct EditorProbe {
    rope: Rope,
    cursor: usize,
    selection_anchor: Option<usize>,
    ime_preedit: Option<String>,
    layout: EditorLayout,
    wrap_width: Option<f32>,
    layout_dirty: bool,
}

impl Default for EditorProbe {
    fn default() -> Self {
        Self::new("")
    }
}

impl EditorProbe {
    /// Creates a new editor probe with initial text.
    pub fn new(text: &str) -> Self {
        Self {
            rope: Rope::from(text),
            cursor: text.len(),
            selection_anchor: None,
            ime_preedit: None,
            layout: EditorLayout::new(16.0),
            wrap_width: None,
            layout_dirty: true,
        }
    }

    /// Access the underlying rope buffer.
    pub fn text(&self) -> &Rope {
        &self.rope
    }

    /// Convert current rope buffer to String.
    pub fn text_as_string(&self) -> String {
        self.rope.to_string()
    }

    /// Current cursor position as a byte offset.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Current active selection as `(start, end)` byte range if any.
    pub fn selection(&self) -> Option<(usize, usize)> {
        self.selection_anchor.map(|anchor| {
            if anchor < self.cursor {
                (anchor, self.cursor)
            } else {
                (self.cursor, anchor)
            }
        })
    }

    /// Returns active IME preedit string if in composition mode.
    pub fn ime_preedit(&self) -> Option<&str> {
        self.ime_preedit.as_deref()
    }

    /// Ensures that the layout is up-to-date with the current text buffer.
    pub fn ensure_layout(&mut self) {
        if self.layout_dirty {
            let text = self.text_as_string();
            self.layout.compute(&text, self.wrap_width);
            self.layout_dirty = false;
        }
    }

    /// Recomputes Parley layout with an optional wrapping width.
    pub fn relayout(&mut self, wrap_width: Option<f32>) {
        self.wrap_width = wrap_width;
        let text = self.text_as_string();
        self.layout.compute(&text, wrap_width);
        self.layout_dirty = false;
    }

    /// Hit tests a point in editor local coordinates, returning a byte offset
    /// guaranteed to be a valid UTF-8 and grapheme boundary.
    pub fn hit_test(&mut self, x: f32, y: f32) -> usize {
        self.ensure_layout();
        let text = self.text_as_string();
        self.layout.hit_test(&text, x, y)
    }

    /// Handles an abstract editor input event.
    pub fn handle_input(&mut self, input: EditorInput) {
        match input {
            EditorInput::ImePreedit { text, .. } => {
                // Preedit MUST NOT touch the persistent Rope!
                if text.is_empty() {
                    self.ime_preedit = None;
                } else {
                    self.ime_preedit = Some(text);
                }
            }
            EditorInput::ImeCancel => {
                self.ime_preedit = None;
            }
            EditorInput::ImeCommit(commit_text) => {
                // Clear preedit immediately upon commit.
                self.ime_preedit = None;
                if !commit_text.is_empty() {
                    self.insert_str(&commit_text);
                }
            }
            EditorInput::InsertText(text) => {
                self.insert_str(&text);
            }
            EditorInput::DeleteBackward => {
                self.delete_backward();
            }
            EditorInput::DeleteForward => {
                self.delete_forward();
            }
            EditorInput::MoveCursorLeft { extend_selection } => {
                self.move_left(extend_selection);
            }
            EditorInput::MoveCursorRight { extend_selection } => {
                self.move_right(extend_selection);
            }
            EditorInput::SetSelection { anchor, head } => {
                let max_len = self.rope.byte_len();
                let clamped_anchor = anchor.min(max_len);
                let clamped_head = head.min(max_len);
                let text = self.text_as_string();
                let valid_anchor = snap_to_grapheme_boundary(&text, clamped_anchor);
                let valid_head = snap_to_grapheme_boundary(&text, clamped_head);
                self.selection_anchor = Some(valid_anchor);
                self.cursor = valid_head;
            }
            EditorInput::SelectAll => {
                self.selection_anchor = Some(0);
                self.cursor = self.rope.byte_len();
            }
            EditorInput::Paste(text) => {
                self.insert_str(&text);
            }
        }
    }

    /// Copies selected text if a selection is active.
    pub fn copy_selection(&self) -> Option<String> {
        let (start, end) = self.selection()?;
        if start == end {
            return None;
        }
        let full = self.text_as_string();
        Some(full[start..end].to_string())
    }

    /// Cuts selected text, removing it from buffer and returning it.
    pub fn cut_selection(&mut self) -> Option<String> {
        let copied = self.copy_selection()?;
        if let Some((start, end)) = self.selection() {
            self.rope.delete(start..end);
            self.cursor = start;
            self.selection_anchor = None;
            self.layout_dirty = true;
        }
        Some(copied)
    }

    /// Pastes text at cursor, replacing selection if active.
    pub fn paste(&mut self, text: &str) {
        self.insert_str(text);
    }

    fn delete_selection_if_active(&mut self) -> bool {
        if let Some((start, end)) = self.selection()
            && start < end
        {
            self.rope.delete(start..end);
            self.cursor = start;
            self.selection_anchor = None;
            self.layout_dirty = true;
            return true;
        }
        self.selection_anchor = None;
        false
    }

    fn insert_str(&mut self, s: &str) {
        self.delete_selection_if_active();
        let cur = self.cursor;
        self.rope.insert(cur, s);
        self.cursor = cur + s.len();
        self.selection_anchor = None;
        self.layout_dirty = true;
    }

    fn delete_backward(&mut self) {
        if self.delete_selection_if_active() {
            return;
        }
        if self.cursor == 0 {
            return;
        }

        let text = self.text_as_string();
        // Find previous grapheme boundary before cursor.
        let mut prev = 0;
        for (idx, cluster) in text.grapheme_indices(true) {
            let next = idx + cluster.len();
            if next >= self.cursor {
                prev = idx;
                break;
            }
        }

        self.rope.delete(prev..self.cursor);
        self.cursor = prev;
        self.layout_dirty = true;
    }

    fn delete_forward(&mut self) {
        if self.delete_selection_if_active() {
            return;
        }
        if self.cursor >= self.rope.byte_len() {
            return;
        }

        let text = self.text_as_string();
        let mut next = self.cursor;
        for (idx, cluster) in text.grapheme_indices(true) {
            if idx >= self.cursor {
                next = idx + cluster.len();
                break;
            }
        }

        if next > self.cursor {
            self.rope.delete(self.cursor..next);
            self.layout_dirty = true;
        }
    }

    fn move_left(&mut self, extend_selection: bool) {
        if !extend_selection {
            if let Some((start, _)) = self.selection() {
                self.cursor = start;
                self.selection_anchor = None;
                return;
            }
        } else if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor);
        }

        if self.cursor == 0 {
            return;
        }

        let text = self.text_as_string();
        let mut target = 0;
        for (idx, cluster) in text.grapheme_indices(true) {
            let end = idx + cluster.len();
            if end >= self.cursor {
                target = idx;
                break;
            }
        }
        self.cursor = target;
    }

    fn move_right(&mut self, extend_selection: bool) {
        if !extend_selection {
            if let Some((_, end)) = self.selection() {
                self.cursor = end;
                self.selection_anchor = None;
                return;
            }
        } else if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor);
        }

        if self.cursor >= self.rope.byte_len() {
            return;
        }

        let text = self.text_as_string();
        let mut target = self.cursor;
        for (idx, cluster) in text.grapheme_indices(true) {
            if idx >= self.cursor {
                target = idx + cluster.len();
                break;
            }
        }
        self.cursor = target;
    }
}

/// Masonry retained Widget wrapping `EditorProbe`.
pub struct EditorWidget {
    pub probe: EditorProbe,
    pub theme: Theme,
}

impl EditorWidget {
    pub fn new(initial_text: &str) -> Self {
        Self {
            probe: EditorProbe::new(initial_text),
            theme: Theme::default(),
        }
    }
}

impl Widget for EditorWidget {
    type Action = masonry::core::NoAction;

    fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

    fn layout(
        &mut self,
        _ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let max_width = bc.max().width;
        let wrap_width = if max_width.is_finite() && max_width > 48.0 {
            Some((max_width - 48.0) as f32)
        } else if max_width.is_finite() && max_width > 0.0 {
            Some(max_width as f32)
        } else {
            None
        };
        self.probe.relayout(wrap_width);
        let height = (self.probe.layout.height() + 48.0).max(24.0) as f64;
        let width = if max_width.is_finite() {
            max_width
        } else {
            (self.probe.layout.width() + 48.0) as f64
        };
        bc.constrain(Size::new(width, height))
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        let size = ctx.size();
        let padding = 24.0;
        let origin = Point::new(padding, padding);

        // 1. Fill background with theme color
        EditorPainter::paint_background(scene, size, &self.theme);

        self.probe.ensure_layout();
        let parley_layout = self.probe.layout.parley_layout();

        // 2. Selection highlights
        if let Some((start, end)) = self.probe.selection() {
            let anchor = parley::layout::Cursor::from_byte_index(
                parley_layout,
                start,
                parley::layout::Affinity::Downstream,
            );
            let focus = parley::layout::Cursor::from_byte_index(
                parley_layout,
                end,
                parley::layout::Affinity::Downstream,
            );
            let selection = parley::layout::Selection::new(anchor, focus);
            selection.geometry_with(parley_layout, |bbox, _line| {
                let rect = Rect::new(
                    origin.x + bbox.x0,
                    origin.y + bbox.y0,
                    origin.x + bbox.x1,
                    origin.y + bbox.y1,
                );
                EditorPainter::paint_selection_rect(scene, rect, &self.theme);
            });
        }

        // 3. Text rendering
        let [r, g, b, a] = self.theme.text_primary.to_rgba_u8();
        let default_color = masonry::vello::peniko::Color::from_rgba8(r, g, b, a);
        EditorPainter::paint_text(scene, origin, parley_layout, default_color);

        // 4. Cursor / Caret
        let cursor_offset = self.probe.cursor();
        let cursor = parley::layout::Cursor::from_byte_index(
            parley_layout,
            cursor_offset,
            parley::layout::Affinity::Downstream,
        );
        let bbox = cursor.geometry(parley_layout, 2.0);
        let cursor_pos = Point::new(origin.x + bbox.x0, origin.y + bbox.y0);
        let cursor_height = (bbox.y1 - bbox.y0).max(18.0);
        EditorPainter::paint_cursor(scene, cursor_pos, cursor_height, false, &self.theme);
    }

    fn accessibility_role(&self) -> masonry::accesskit::Role {
        masonry::accesskit::Role::TextInput
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        _node: &mut masonry::accesskit::Node,
    ) {
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }

    fn accepts_pointer_interaction(&self) -> bool {
        true
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn accepts_text_input(&self) -> bool {
        true
    }

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        if let PointerEvent::Down(down) = event
            && down.button == Some(PointerButton::Primary)
        {
            let hit = self.probe.hit_test(
                (down.state.position.x - 24.0).max(0.0) as f32,
                (down.state.position.y - 24.0).max(0.0) as f32,
            );
            self.probe.cursor = hit;
            self.probe.selection_anchor = None;
            ctx.request_render();
        }
    }

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        match event {
            TextEvent::Keyboard(key_event) => {
                if key_event.state != KeyState::Down || key_event.is_composing {
                    return;
                }
                let shift = key_event.modifiers.shift();
                let ctrl = key_event.modifiers.ctrl() || key_event.modifiers.meta();

                match &key_event.key {
                    Key::Character(s) if !ctrl => {
                        self.probe.handle_input(EditorInput::InsertText(s.clone()));
                        ctx.request_render();
                    }
                    Key::Named(NamedKey::Enter) => {
                        self.probe
                            .handle_input(EditorInput::InsertText("\n".to_string()));
                        ctx.request_render();
                    }
                    Key::Named(NamedKey::Backspace) => {
                        self.probe.handle_input(EditorInput::DeleteBackward);
                        ctx.request_render();
                    }
                    Key::Named(NamedKey::Delete) => {
                        self.probe.handle_input(EditorInput::DeleteForward);
                        ctx.request_render();
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        self.probe.handle_input(EditorInput::MoveCursorLeft {
                            extend_selection: shift,
                        });
                        ctx.request_render();
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        self.probe.handle_input(EditorInput::MoveCursorRight {
                            extend_selection: shift,
                        });
                        ctx.request_render();
                    }
                    Key::Character(s) if ctrl && s.eq_ignore_ascii_case("a") => {
                        self.probe.handle_input(EditorInput::SelectAll);
                        ctx.request_render();
                    }
                    Key::Character(s) if ctrl && s.eq_ignore_ascii_case("c") => {
                        if let Some(text) = self.probe.copy_selection() {
                            ctx.set_clipboard(text);
                        }
                    }
                    Key::Character(s) if ctrl && s.eq_ignore_ascii_case("x") => {
                        if let Some(text) = self.probe.cut_selection() {
                            ctx.set_clipboard(text);
                            ctx.request_render();
                        }
                    }
                    _ => {}
                }
            }
            TextEvent::Ime(ime) => match ime {
                masonry::core::Ime::Preedit(text, cursor) => {
                    self.probe.handle_input(EditorInput::ImePreedit {
                        text: text.clone(),
                        cursor: *cursor,
                    });
                    ctx.request_render();
                }
                masonry::core::Ime::Commit(text) => {
                    self.probe
                        .handle_input(EditorInput::ImeCommit(text.clone()));
                    ctx.request_render();
                }
                _ => {}
            },
            TextEvent::ClipboardPaste(text) => {
                self.probe.handle_input(EditorInput::Paste(text.clone()));
                ctx.request_render();
            }
            _ => {}
        }
    }

    fn update(
        &mut self,
        _ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _event: &Update,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ime_commit_once() {
        let mut probe = EditorProbe::new("Hello ");
        assert_eq!(probe.text_as_string(), "Hello ");
        assert_eq!(probe.ime_preedit(), None);

        // Step 1: IME preedit arrives
        probe.handle_input(EditorInput::ImePreedit {
            text: "при".to_string(),
            cursor: Some((3, 3)),
        });

        // Preedit must be recorded in probe state...
        assert_eq!(probe.ime_preedit(), Some("при"));
        // ...but MUST NOT leak into the persistent Rope text!
        assert_eq!(probe.text_as_string(), "Hello ");

        // Step 2: Second IME preedit update before commit
        probe.handle_input(EditorInput::ImePreedit {
            text: "привет".to_string(),
            cursor: Some((6, 6)),
        });
        assert_eq!(probe.ime_preedit(), Some("привет"));
        assert_eq!(probe.text_as_string(), "Hello ");

        // Step 3: IME commit arrives
        probe.handle_input(EditorInput::ImeCommit("привет".to_string()));

        // Preedit must be cleared
        assert_eq!(probe.ime_preedit(), None);
        // Commit text must be inserted into Rope EXACTLY ONCE
        assert_eq!(probe.text_as_string(), "Hello привет");
        assert_eq!(probe.cursor(), "Hello привет".len());

        // Step 4: Another input does not duplicate the commit
        probe.handle_input(EditorInput::InsertText("!".to_string()));
        assert_eq!(probe.text_as_string(), "Hello привет!");
    }

    #[test]
    fn probe_hit_test() {
        let content = "Первая строка: Привет мир! 🦀\nВторая строка: 👨‍👩‍👧‍👦 семья й̆\nТретья строка: English text.";
        let mut probe = EditorProbe::new(content);
        probe.relayout(Some(300.0));

        let probe_coords = [
            (-100.0, -50.0),
            (0.0, 0.0),
            (10.0, 5.0),
            (50.0, 15.0),
            (150.0, 25.0),
            (300.0, 40.0),
            (500.0, 80.0),
            (1000.0, 1000.0),
        ];

        for (x, y) in probe_coords {
            let hit = probe.hit_test(x, y);

            // Mandatory requirement: every hit returns a valid UTF-8 boundary
            assert!(
                hit <= content.len(),
                "Hit offset {hit} exceeds content length {}",
                content.len()
            );
            assert!(
                content.is_char_boundary(hit),
                "Hit offset {hit} at ({x}, {y}) cuts across a UTF-8 character boundary!"
            );

            // Grapheme cluster preservation: hit must not slice an extended grapheme cluster
            let snapped = snap_to_grapheme_boundary(content, hit);
            assert_eq!(
                hit, snapped,
                "Hit offset {hit} at ({x}, {y}) sliced a grapheme cluster!"
            );

            // String slicing at hit must not panic
            let _prefix = &content[..hit];
            let _suffix = &content[hit..];
        }
    }

    #[test]
    fn test_selection_and_clipboard() {
        let mut probe = EditorProbe::new("Rust Programming");
        probe.handle_input(EditorInput::SetSelection { anchor: 0, head: 4 });
        assert_eq!(probe.selection(), Some((0, 4)));
        assert_eq!(probe.copy_selection(), Some("Rust".to_string()));

        let cut = probe.cut_selection();
        assert_eq!(cut, Some("Rust".to_string()));
        assert_eq!(probe.text_as_string(), " Programming");

        probe.paste("Advanced");
        assert_eq!(probe.text_as_string(), "Advanced Programming");
    }
}
