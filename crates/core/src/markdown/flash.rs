use std::ops::Range;

use serde::{Deserialize, Serialize};

use super::snapshot::{MarkdownSnapshot, SemanticKind};
use crate::text::Buffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FlashMode {
    Word,
    Sentence,
    Paragraph,
    SyntaxObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FlashTargetKind {
    Word,
    Sentence,
    Paragraph,
    Syntax(SemanticKind),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlashTarget {
    pub kind: FlashTargetKind,
    pub range: Range<usize>,
}

/// Collects pure source target ranges for Flash jumping in `visible_range` without screen glyph labels.
pub fn collect_flash_targets(
    buffer: &Buffer,
    snapshot: Option<&MarkdownSnapshot>,
    visible_range: Range<usize>,
    mode: FlashMode,
) -> Vec<FlashTarget> {
    let start = visible_range.start.min(buffer.byte_len());
    let end = visible_range.end.min(buffer.byte_len());
    if start >= end {
        return Vec::new();
    }

    match mode {
        FlashMode::Word => {
            let slice = buffer.slice(start..end).to_string();
            let mut targets = Vec::new();
            let mut in_word = false;
            let mut word_start = 0;
            for (idx, c) in slice.char_indices() {
                let is_word_char = c.is_alphanumeric() || c == '_';
                if is_word_char && !in_word {
                    in_word = true;
                    word_start = idx;
                } else if !is_word_char && in_word {
                    in_word = false;
                    targets.push(FlashTarget {
                        kind: FlashTargetKind::Word,
                        range: (start + word_start)..(start + idx),
                    });
                }
            }
            if in_word {
                targets.push(FlashTarget {
                    kind: FlashTargetKind::Word,
                    range: (start + word_start)..(start + slice.len()),
                });
            }
            targets
        }
        FlashMode::Sentence => {
            let slice = buffer.slice(start..end).to_string();
            let mut targets = Vec::new();
            let mut sent_start = 0;
            let chars: Vec<(usize, char)> = slice.char_indices().collect();
            for i in 0..chars.len() {
                let (_idx, c) = chars[i];
                if matches!(c, '.' | '!' | '?' | '\n') {
                    let end_pos = if i + 1 < chars.len() {
                        chars[i + 1].0
                    } else {
                        slice.len()
                    };
                    if end_pos > sent_start {
                        let text = slice[sent_start..end_pos].trim();
                        if !text.is_empty() {
                            targets.push(FlashTarget {
                                kind: FlashTargetKind::Sentence,
                                range: (start + sent_start)..(start + end_pos),
                            });
                        }
                    }
                    sent_start = end_pos;
                }
            }
            if sent_start < slice.len() && !slice[sent_start..].trim().is_empty() {
                targets.push(FlashTarget {
                    kind: FlashTargetKind::Sentence,
                    range: (start + sent_start)..(start + slice.len()),
                });
            }
            targets
        }
        FlashMode::Paragraph => {
            let slice = buffer.slice(start..end).to_string();
            let mut targets = Vec::new();
            let mut para_start = 0;
            for part in slice.split("\n\n") {
                let p_len = part.len();
                if !part.trim().is_empty() {
                    targets.push(FlashTarget {
                        kind: FlashTargetKind::Paragraph,
                        range: (start + para_start)..(start + para_start + p_len),
                    });
                }
                para_start += p_len + 2;
            }
            targets
        }
        FlashMode::SyntaxObject => {
            if let Some(s) = snapshot {
                s.semantic_objects_in(start..end)
                    .into_iter()
                    .map(|node| FlashTarget {
                        kind: FlashTargetKind::Syntax(node.kind),
                        range: node.range.clone(),
                    })
                    .collect()
            } else {
                collect_flash_targets(buffer, None, start..end, FlashMode::Word)
            }
        }
    }
}
