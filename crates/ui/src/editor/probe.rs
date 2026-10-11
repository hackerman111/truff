//! Headless viewport probe using the same transactional buffer and input adapter.
use super::{EditorInput, EditorLayout, InputAdapter};
use truff_core::text::{Buffer, BufferId, EditError};

pub struct EditorProbe {
    buffer: Buffer,
    cursor: usize,
    selection: Option<(usize, usize)>,
    input: InputAdapter,
    layout: EditorLayout,
    presentation: String,
    wrap_width: Option<f32>,
    dirty: bool,
}

impl Default for EditorProbe {
    fn default() -> Self {
        Self::new("").expect("empty buffer is valid")
    }
}
impl EditorProbe {
    pub fn new(text: &str) -> Result<Self, EditError> {
        Ok(Self {
            buffer: Buffer::from_text(BufferId::next(), text)?,
            cursor: text.len(),
            selection: None,
            input: InputAdapter::new(),
            layout: EditorLayout::new(16.0),
            presentation: String::new(),
            wrap_width: None,
            dirty: true,
        })
    }
    pub fn text(&self) -> &crop::Rope {
        self.buffer.rope()
    }
    pub fn text_as_string(&self) -> String {
        self.buffer.as_str()
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    pub fn selection(&self) -> Option<(usize, usize)> {
        self.selection
    }
    pub fn ime_preedit(&self) -> Option<&str> {
        self.input.ime_preedit()
    }
    pub fn ensure_layout(&mut self) {
        if self.dirty {
            let mut end = self.buffer.byte_len().min(4096);
            while !self.buffer.rope().is_char_boundary(end) {
                end -= 1;
            }
            self.presentation = self.buffer.slice(0..end).to_string();
            self.layout.compute(&self.presentation, self.wrap_width);
            self.dirty = false;
        }
    }
    pub fn relayout(&mut self, width: Option<f32>) {
        self.wrap_width = width;
        self.dirty = true;
        self.ensure_layout();
    }
    pub fn hit_test(&mut self, x: f32, y: f32) -> usize {
        self.ensure_layout();
        self.layout.hit_test(&self.presentation, x, y)
    }
    pub fn handle_input(&mut self, input: EditorInput) -> Result<(), EditError> {
        let changed = self.input.handle_input(
            &mut self.buffer,
            &mut self.cursor,
            &mut self.selection,
            input,
        )?;
        self.dirty |= changed.is_some();
        Ok(())
    }
}
