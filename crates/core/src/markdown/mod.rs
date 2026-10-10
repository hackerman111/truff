//! Incremental markdown parsing, structure extraction, and document facts.

pub mod facts;
pub mod links;
pub mod syntax;

pub use facts::{BlockFact, DocFacts, HeadingFact, MarkdownError, MarkdownFacts};
pub use links::LinkOccurrence;
pub use syntax::{IncrementalSyntaxTree, SyntaxSnapshot};
