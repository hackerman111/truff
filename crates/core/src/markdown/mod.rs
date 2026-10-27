//! Incremental markdown parsing, structure extraction, and document facts.

pub mod facts;
pub mod flash;
pub mod link_selection;
pub mod links;
pub mod presentation;
pub mod smart_edit;
pub mod snapshot;
pub mod style;
pub mod syntax;

pub use facts::{BlockFact, DocFacts, HeadingFact, MarkdownError, MarkdownFacts};
pub use flash::{FlashMode, FlashTarget, FlashTargetKind, collect_flash_targets};
pub use link_selection::{ExtractNotePlan, LinkSelection, LinkTarget};
pub use links::LinkOccurrence;
pub use presentation::{LinePresentation, ListKind, ListMarker, toggle_task};
pub use smart_edit::{
    find_task_checkbox_on_line, plan_backspace, plan_enter, plan_shift_tab, plan_tab,
    plan_toggle_task,
};
pub use snapshot::{
    CalloutFact, CodeBlockFact, FrontmatterFact, MarkdownSnapshot, SectionFact, SemanticKind,
    SemanticNode, TableFact, TableRowFact, TaskFact,
};
pub use style::{
    CalloutKind, ColorSpan, StyleError, apply_highlight, apply_text_color, color_spans,
    remove_style, wrap_callout,
};
pub use syntax::{IncrementalSyntaxTree, SyntaxSnapshot, rope_byte_to_point};
