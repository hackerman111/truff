//! Which-key and Flash overlay controllers.

mod flash;
mod link_completion;
pub mod text_action;
mod which_key;

pub use flash::{FlashKind, FlashOverlay, FlashTarget};
pub use link_completion::{LinkCandidate, LinkCompletionOverlay};
pub use text_action::{
    ColorOption, PALETTE, TextActionMode, TextActionOverlay, TextActionResponse, word_boundary_at,
};
pub(crate) use which_key::{HelpItem, key_label};
pub use which_key::{WhichKeyCategory, WhichKeyMode, WhichKeyOverlay};
