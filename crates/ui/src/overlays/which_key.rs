use truff_core::config::KeyHint;

mod help;
mod help_state;
pub(crate) use help_state::HelpItem;

pub(crate) fn key_label(key: truff_core::vim::Key) -> String {
    if key == truff_core::vim::Key::Char(' ') {
        "Space".into()
    } else {
        key.to_string()
    }
}

/// Display mode of WhichKey overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WhichKeyMode {
    #[default]
    Prefix,
    Help,
}

/// A categorized group of keybindings for the help screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhichKeyCategory {
    pub name: String,
    pub hints: Vec<KeyHint>,
}

/// Contextual overlay displaying available keybindings after a prefix or a full help cheatsheet.
#[derive(Debug, Clone)]
pub struct WhichKeyOverlay {
    hints: Vec<KeyHint>,
    categories: Vec<WhichKeyCategory>,
    mode: WhichKeyMode,
    title: String,
    visible: bool,
    elapsed_ms: u64,
    delay_ms: u64,
    pub(crate) help: help_state::HelpState,
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
            categories: Vec::new(),
            mode: WhichKeyMode::Prefix,
            title: String::new(),
            visible: false,
            elapsed_ms: 0,
            delay_ms: Self::DEFAULT_DELAY_MS,
            help: help_state::HelpState::default(),
        }
    }

    /// Returns the current overlay mode.
    pub fn mode(&self) -> WhichKeyMode {
        self.mode
    }

    /// Returns the overlay title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the displayed categories when in help mode.
    pub fn categories(&self) -> &[WhichKeyCategory] {
        &self.categories
    }

    /// Sets candidate hints when an incomplete key prefix is entered.
    pub fn set_prefix_hints(&mut self, hints: Vec<KeyHint>) {
        self.set_prefix_hints_with_title("", hints);
    }

    /// Sets candidate hints with a specific title when a key prefix is entered.
    pub fn set_prefix_hints_with_title(
        &mut self,
        title: impl Into<String>,
        mut hints: Vec<KeyHint>,
    ) {
        hints.retain(|h| !is_task_hint(&h.description));
        self.mode = WhichKeyMode::Prefix;
        self.title = title.into();
        self.hints = hints;
        self.categories.clear();
        self.elapsed_ms = 0;
        self.visible = false;
    }

    /// Shows which-key immediately (e.g. via explicit command).
    pub fn show_immediately(&mut self, mut hints: Vec<KeyHint>) {
        hints.retain(|h| !is_task_hint(&h.description));
        self.mode = WhichKeyMode::Prefix;
        self.title.clear();
        self.hints = hints;
        self.categories.clear();
        self.visible = true;
    }

    /// Shows the full help cheatsheet immediately.
    pub fn show_help(&mut self, categories: Vec<WhichKeyCategory>) {
        self.mode = WhichKeyMode::Help;
        self.title = "Справка по горячим клавишам".to_string();
        self.hints.clear();
        self.categories = categories
            .into_iter()
            .map(|mut cat| {
                cat.hints.retain(|h| !is_task_hint(&h.description));
                cat
            })
            .filter(|cat| !cat.hints.is_empty())
            .collect();
        self.help = help_state::HelpState::default();
        self.help.set_items(
            self.categories
                .iter()
                .flat_map(|category| {
                    category.hints.iter().map(|hint| HelpItem {
                        category: category.name.clone(),
                        keys: key_label(hint.key),
                        description: hint.description.clone(),
                    })
                })
                .collect(),
        );
        self.visible = true;
    }

    /// Hides the overlay and clears pending hints and categories.
    pub fn hide(&mut self) {
        self.visible = false;
        self.hints = Vec::new();
        self.categories = Vec::new();
        self.title = String::new();
        self.mode = WhichKeyMode::Prefix;
        self.elapsed_ms = 0;
        self.help = help_state::HelpState::default();
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

fn is_task_hint(description: &str) -> bool {
    let lower = description.to_lowercase();
    lower.contains("task") || lower.contains("таск") || lower.contains("задач")
}
