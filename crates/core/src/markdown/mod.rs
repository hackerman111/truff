//! Incremental markdown parsing, structure extraction, and document facts.

pub mod facts;
pub mod links;
pub mod presentation;
pub mod style;
pub mod syntax;

pub use facts::{BlockFact, DocFacts, HeadingFact, MarkdownError, MarkdownFacts};
pub use links::LinkOccurrence;
pub use style::{
    CalloutKind, StyleError, apply_highlight, apply_text_color, remove_style, wrap_callout,
};
pub use syntax::{IncrementalSyntaxTree, SyntaxSnapshot};

mod link_selection;
pub use link_selection::LinkSelection;
