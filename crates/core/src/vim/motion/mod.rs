mod cursor;
mod find;
mod objects;
mod search;
mod types;
mod word;

pub use cursor::{eval_motion, eval_operator_motion_range, line_content_end, snap_grapheme};
pub(crate) use objects::eval_delimited_object_range;
pub use objects::eval_text_object_range;
pub use search::literal_search;
pub use types::{
    FindKind, Motion, SyntaxObjectRange, TextObject, TextObjectKind, TextObjectTarget,
};
