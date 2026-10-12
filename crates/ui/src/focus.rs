//! Deterministic focus routing and history stack for panels and overlays (§10).

/// Active focused pane or overlay in the application layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus {
    Sidebar,
    #[default]
    Editor,
    RightPanel,
    Navigator,
    CommandBar,
}

/// Focus manager maintaining current target and deterministic restoration history.
#[derive(Debug, Clone)]
pub struct FocusManager {
    current: Focus,
    history: Vec<Focus>,
}

impl Default for FocusManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FocusManager {
    /// Creates a new focus manager starting at Editor.
    pub fn new() -> Self {
        Self {
            current: Focus::Editor,
            history: Vec::new(),
        }
    }

    /// Returns the currently focused component.
    pub fn current(&self) -> Focus {
        self.current
    }

    /// Directly sets focus without pushing previous focus to history.
    pub fn set_focus(&mut self, next: Focus) {
        self.current = next;
    }

    /// Pushes current focus to the history stack and shifts to `next` (e.g. opening Navigator).
    pub fn push_focus(&mut self, next: Focus) {
        if self.current != next {
            self.history.push(self.current);
            self.current = next;
        }
    }

    /// Pops previously focused component and restores it, defaulting to Editor if stack is empty.
    pub fn pop_focus(&mut self) -> Focus {
        while let Some(prev) = self.history.pop() {
            // Do not pop back into transient overlay targets
            if prev != Focus::Navigator && prev != Focus::CommandBar {
                self.current = prev;
                return prev;
            }
        }
        self.current = Focus::Editor;
        Focus::Editor
    }
}
