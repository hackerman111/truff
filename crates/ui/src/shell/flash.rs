use crate::overlays::{FlashKind, FlashOverlay};

use super::{InputError, Shell, ShellAction};

impl Shell {
    pub fn flash(&self) -> &FlashOverlay {
        &self.flash
    }

    pub fn start_flash(&mut self, range: std::ops::Range<usize>) {
        let _ = self.start_flash_kind(range, FlashKind::Word);
    }

    /// Scans only the requested visible source slice. A chosen sentence/paragraph
    /// is expanded by the core object resolver after its label is selected.
    pub fn start_flash_kind(
        &mut self,
        range: std::ops::Range<usize>,
        kind: FlashKind,
    ) -> Result<(), InputError> {
        self.start_flash_ranges(&[range], kind)
    }

    /// Actual shaped source windows, rather than their possibly giant source envelope.
    /// At most 1024 windows and 1 MiB of source text may be scanned per activation.
    pub fn start_flash_ranges(
        &mut self,
        ranges: &[std::ops::Range<usize>],
        kind: FlashKind,
    ) -> Result<(), InputError> {
        use unicode_segmentation::UnicodeSegmentation;
        let buffer = self.editor_buffer().ok_or(InputError::UnavailableTarget)?;
        if ranges.len() > 1024 {
            return Err(InputError::Vim(
                "Flash viewport window limit exceeded".into(),
            ));
        }
        let mut total = 0usize;
        for range in ranges {
            if range.start > range.end || range.end > buffer.byte_len() {
                return Err(InputError::Vim("Invalid Flash viewport range".into()));
            }
            if !buffer.is_char_boundary(range.start) || !buffer.is_char_boundary(range.end) {
                return Err(InputError::Vim(
                    "Flash viewport is not a UTF-8 boundary".into(),
                ));
            }
            total = total.saturating_add(range.len());
        }
        if total > 1024 * 1024 {
            return Err(InputError::Vim(
                "Flash viewport text limit exceeded; pass shaped source windows".into(),
            ));
        }
        // A remapped operator prefix must reach the Vim parser before Flash consumes it.
        if !self.binding_prefix.is_empty() {
            let prefix = std::mem::take(&mut self.binding_prefix);
            let custom = self.custom_keymap;
            self.custom_keymap = false;
            for key in prefix {
                if let ShellAction::Error(error) = self.feed_key(key) {
                    self.custom_keymap = custom;
                    return Err(InputError::Vim(error));
                }
            }
            self.custom_keymap = custom;
        }
        let buffer = self.editor_buffer().ok_or(InputError::UnavailableTarget)?;
        let mut targets = Vec::new();
        if kind == FlashKind::SyntaxObject {
            let objects = self
                .editor_vim
                .syntax_object_ranges(buffer.revision())
                .ok_or_else(|| {
                    InputError::Vim("Syntax targets unavailable at this revision".into())
                })?;
            targets.extend(
                objects
                    .iter()
                    .filter(|o| {
                        ranges
                            .iter()
                            .any(|range| o.around.start < range.end && o.around.end > range.start)
                    })
                    .take(100)
                    .map(|o| o.around.clone()),
            );
        } else {
            for range in ranges {
                if targets.len() == 100 {
                    break;
                }
                let visible = buffer.slice(range.clone()).to_string();
                let candidates = match kind {
                    FlashKind::Word => FlashOverlay::generate_targets(&visible, range.start)
                        .into_iter()
                        .map(|t| t.source_range)
                        .collect::<Vec<_>>(),
                    FlashKind::Sentence => visible
                        .split_sentence_bound_indices()
                        .filter(|(_, s)| !s.trim().is_empty())
                        .take(100)
                        .map(|(offset, s)| {
                            let whitespace = s.len() - s.trim_start().len();
                            range.start + offset + whitespace..range.start + offset + s.len()
                        })
                        .collect(),
                    FlashKind::Paragraph => {
                        let mut candidates = Vec::new();
                        let mut offset = range.start;
                        let mut start = None;
                        for line in visible.split_inclusive('\n') {
                            if line.trim().is_empty() {
                                if let Some(start) = start.take() {
                                    candidates.push(start..offset);
                                }
                            } else if start.is_none() {
                                start = Some(offset);
                            }
                            offset += line.len();
                            if candidates.len() == 100 {
                                break;
                            }
                        }
                        if candidates.len() < 100
                            && let Some(start) = start
                        {
                            candidates.push(start..offset);
                        }
                        candidates
                    }
                    FlashKind::SyntaxObject => unreachable!(),
                };
                for target in candidates {
                    if !targets.contains(&target) {
                        targets.push(target);
                    }
                    if targets.len() == 100 {
                        break;
                    }
                }
            }
        }
        let candidates = targets
            .into_iter()
            .map(|range| {
                let text = buffer.slice(range.clone()).to_string();
                (range, text)
            })
            .collect::<Vec<_>>();
        self.flash.activate_candidates(candidates);
        self.flash_kind = kind;
        Ok(())
    }
}
