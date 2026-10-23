//! Incremental markdown parsing, structure extraction, and document facts.

pub mod database;
pub mod facts;
pub mod flash;
pub mod graph;
pub mod kanban;
pub mod link_selection;
pub mod links;
pub mod presentation;
pub mod smart_edit;
pub mod snapshot;
pub mod style;
pub mod syntax;
pub mod templates;

pub use database::{
    DatabaseRecord, DatabaseRow, DatabaseTableResult, DatabaseTableSource, InlineSqlBlock,
    SqlComparisonOp, SqlCondition, SqlQuery, SqlSortOrder, evaluate_sql_query, extract_sql_blocks,
};
pub use facts::{
    BlockFact, DocFacts, HeadingFact, MarkdownError, MarkdownFacts, TagFact, extract_inline_tags,
};
pub use flash::{FlashMode, FlashTarget, FlashTargetKind, collect_flash_targets};
pub use graph::{
    GraphConfig, GraphEdge, GraphLayoutKind, GraphModel, GraphNode, InlineGraphBlock,
    extract_graph_blocks, plan_upsert_graph_block,
};
pub use kanban::{
    KANBAN_MARKER, KanbanBoard, KanbanCard, KanbanColumn, is_kanban_document, plan_add_card,
    plan_add_column, plan_color_card, plan_delete_card, plan_edit_card, plan_move_card,
    plan_reorder_card, plan_toggle_card,
};
pub use link_selection::{ExtractNotePlan, LinkSelection, LinkTarget};
pub use links::LinkOccurrence;
pub use presentation::{LinePresentation, ListKind, ListMarker, toggle_task};
pub use smart_edit::{
    find_task_checkbox_on_line, plan_enter, plan_shift_tab, plan_tab, plan_toggle_task,
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
pub use templates::{
    NoteTemplateKind, WIKI_AUTOCOLLECT_END, WIKI_AUTOCOLLECT_START, WikiNoteSummary,
    build_wiki_autocollect_block, is_wiki_master_filename, render_template,
    sync_wiki_master_content,
};
