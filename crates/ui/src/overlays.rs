use truff_core::config::KeyHint;

/// Contextual overlay displaying available keybindings after a prefix.
#[derive(Debug, Clone)]
pub struct WhichKeyOverlay {
    hints: Vec<KeyHint>,
    visible: bool,
    elapsed_ms: u64,
    delay_ms: u64,
}

impl Default for WhichKeyOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl WhichKeyOverlay {
    /// Default delay before showing which-key automatically: 300ms (§7).
    pub const DEFAULT_DELAY_MS: u64 = 300;

    /// Creates a new WhichKey overlay controller.
    pub fn new() -> Self {
        Self {
            hints: Vec::new(),
            visible: false,
            elapsed_ms: 0,
            delay_ms: Self::DEFAULT_DELAY_MS,
        }
    }

    /// Sets candidate hints when an incomplete key prefix is entered.
    pub fn set_prefix_hints(&mut self, hints: Vec<KeyHint>) {
        self.hints = hints;
        self.elapsed_ms = 0;
        self.visible = false;
    }

    /// Shows which-key immediately (e.g. via explicit command).
    pub fn show_immediately(&mut self, hints: Vec<KeyHint>) {
        self.hints = hints;
        self.visible = true;
    }

    /// Hides the overlay and clears pending hints.
    pub fn hide(&mut self) {
        self.visible = false;
        self.hints.clear();
        self.elapsed_ms = 0;
    }

    /// Advances the timer by `delta_ms`. Shows overlay if threshold is reached.
    pub fn tick(&mut self, delta_ms: u64) {
        if !self.visible && !self.hints.is_empty() {
            self.elapsed_ms += delta_ms;
            if self.elapsed_ms >= self.delay_ms {
                self.visible = true;
            }
        }
    }

    /// Returns `true` if the overlay is currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Returns the currently displayed hints.
    pub fn hints(&self) -> &[KeyHint] {
        &self.hints
    }

    /// Formats hints for display in the popup window.
    pub fn formatted_lines(&self) -> Vec<String> {
        self.hints
            .iter()
            .map(|h| format!("{:<8} -> {}", h.key, h.description))
            .collect()
    }
}
