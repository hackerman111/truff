/// A visible target for Flash letter jump navigation.
#[derive(Debug, Clone, PartialEq)]
pub struct FlashTarget {
    /// Letter label (e.g. "a", "s", "d", "f", "j", "k", "l", ";", "aa", ...).
    pub label: String,
    /// Target source byte range.
    pub source_range: std::ops::Range<usize>,
    /// Word / object text.
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlashKind {
    #[default]
    Word,
    Sentence,
    Paragraph,
    SyntaxObject,
}

/// Controller managing Flash-style letter jump overlays (§8).
#[derive(Debug, Clone, Default)]
pub struct FlashOverlay {
    targets: Vec<FlashTarget>,
    query: String,
    active: bool,
    search_prompt: String,
    waiting_search_chars: bool,
    search_windows: Vec<(std::ops::Range<usize>, String)>,
}

impl FlashOverlay {
    /// Canonical home-row letters followed by top and bottom row letters.
    pub const HOME_ROW_LETTERS: &'static [char] = &[
        'a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l', 'q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o',
        'p', 'z', 'x', 'c', 'v', 'b', 'n', 'm',
    ];

    /// Creates a new `FlashOverlay`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if Flash navigation is currently active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Whether Flash is waiting for 2 search characters before assigning labels.
    pub fn is_waiting_search_chars(&self) -> bool {
        self.waiting_search_chars
    }

    /// Current 2-character search query.
    pub fn search_prompt(&self) -> &str {
        &self.search_prompt
    }

    /// Remove a character from search prompt on Backspace.
    pub fn backspace(&mut self) {
        if self.waiting_search_chars {
            self.search_prompt.pop();
        } else if !self.query.is_empty() {
            self.query.pop();
        }
    }

    /// Current active flash targets.
    pub fn targets(&self) -> &[FlashTarget] {
        &self.targets
    }

    /// Current accumulated query prefix.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Activates Flash in 2-character Neovim mode: waits for 2 chars then labels occurrences.
    pub fn activate_search(&mut self, windows: Vec<(std::ops::Range<usize>, String)>) {
        self.targets.clear();
        self.query.clear();
        self.search_prompt.clear();
        self.search_windows = windows;
        self.waiting_search_chars = true;
        self.active = true;
    }

    /// Activates the Flash overlay by generating targets ONLY for the visible viewport text.
    ///
    /// GUARANTEE: Targets are strictly extracted from `visible_text` (viewport) with source ranges
    /// rebased by `viewport_start_offset`. Targets outside the viewport are NEVER generated!
    pub fn activate(&mut self, visible_text: &str, viewport_start_offset: usize) {
        self.targets = Self::generate_targets(visible_text, viewport_start_offset);
        self.query.clear();
        self.search_prompt.clear();
        self.waiting_search_chars = false;
        self.search_windows.clear();
        self.active = !self.targets.is_empty();
    }

    /// Cancels and deactivates Flash navigation.
    pub fn cancel(&mut self) {
        self.active = false;
        self.waiting_search_chars = false;
        self.targets.clear();
        self.query.clear();
        self.search_prompt.clear();
        self.search_windows.clear();
    }

    /// Feeds an input character.
    ///
    /// If the typed sequence uniquely matches a Flash target:
    /// returns `Some(target.source_range)` and deactivates.
    /// If no targets match the prefix, cancels and returns `None`.
    pub fn feed_char(&mut self, ch: char) -> Option<std::ops::Range<usize>> {
        if !self.active {
            return None;
        }

        if self.waiting_search_chars {
            self.search_prompt.push(ch);
            if self.search_prompt.chars().count() < 2 {
                return None;
            }
            // 2 characters typed: search in windows for matches (case-insensitive)
            let query = self.search_prompt.to_lowercase();
            let mut candidates = Vec::new();
            for (window_range, text) in &self.search_windows {
                let text_lower = text.to_lowercase();
                let mut start_idx = 0;
                while let Some(pos) = text_lower[start_idx..].find(&query) {
                    let match_idx = start_idx + pos;
                    let match_len = query.len();
                    let source_start = window_range.start + match_idx;
                    let source_end = source_start + match_len;
                    let word_end = text[match_idx..]
                        .find(|c: char| !c.is_alphanumeric() && c != '_')
                        .map_or(text.len(), |e| match_idx + e);
                    let candidate_text = text[match_idx..word_end].to_string();
                    candidates.push((source_start..source_end, candidate_text));
                    if candidates.len() >= 100 {
                        break;
                    }
                    start_idx = match_idx + match_len.max(1);
                    if start_idx >= text.len() {
                        break;
                    }
                }
                if candidates.len() >= 100 {
                    break;
                }
            }

            if candidates.is_empty() {
                self.cancel();
                return None;
            }

            self.targets = Self::label_candidates(candidates);
            self.waiting_search_chars = false;
            self.query.clear();
            return None;
        }

        self.query.push(ch);

        // Check for exact match
        if let Some(target) = self.targets.iter().find(|t| t.label == self.query) {
            let range = target.source_range.clone();
            self.cancel();
            return Some(range);
        }

        // Check if any target has self.query as prefix
        let has_prefix_matches = self
            .targets
            .iter()
            .any(|t| t.label.starts_with(&self.query));
        if !has_prefix_matches {
            self.cancel();
        }

        None
    }

    /// Generates Flash targets for words strictly within the visible text.
    pub fn generate_targets(visible_text: &str, source_offset: usize) -> Vec<FlashTarget> {
        Self::label_candidates(Self::word_ranges(visible_text, source_offset).map(|range| {
            let text =
                visible_text[range.start - source_offset..range.end - source_offset].to_owned();
            (range, text)
        }))
    }

    pub(crate) fn word_ranges(
        visible_text: &str,
        source_offset: usize,
    ) -> impl Iterator<Item = std::ops::Range<usize>> + '_ {
        use unicode_segmentation::UnicodeSegmentation;
        visible_text
            .split_word_bound_indices()
            .filter(|(_, word)| word.trim().chars().any(char::is_alphanumeric))
            .take(100)
            .map(move |(offset, word)| source_offset + offset..source_offset + offset + word.len())
    }

    pub fn activate_candidates(
        &mut self,
        candidates: impl IntoIterator<Item = (std::ops::Range<usize>, String)>,
    ) {
        self.targets = Self::label_candidates(candidates);
        self.query.clear();
        self.search_prompt.clear();
        self.waiting_search_chars = false;
        self.search_windows.clear();
        self.active = !self.targets.is_empty();
    }

    /// Syntax/semantic producers provide source ranges; labeling stays UI-owned.
    pub fn activate_ranges(&mut self, ranges: impl IntoIterator<Item = std::ops::Range<usize>>) {
        self.activate_candidates(ranges.into_iter().map(|r| (r, String::new())));
    }
    fn label_candidates(
        candidates: impl IntoIterator<Item = (std::ops::Range<usize>, String)>,
    ) -> Vec<FlashTarget> {
        let alphabet = Self::HOME_ROW_LETTERS;
        let mut targets: Vec<_> = candidates
            .into_iter()
            .take(100)
            .map(|(source_range, text)| FlashTarget {
                label: String::new(),
                source_range,
                text,
            })
            .collect();
        let double = targets.len() > alphabet.len();
        for (i, target) in targets.iter_mut().enumerate() {
            target.label = if double {
                format!(
                    "{}{}",
                    alphabet[i / alphabet.len()],
                    alphabet[i % alphabet.len()]
                )
            } else {
                alphabet[i].to_string()
            };
        }
        targets
    }
}
