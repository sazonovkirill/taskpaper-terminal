//! The Document layer: the outline tree and the `.taskpaper` format. Depends on `std` and small pure crates only.

pub mod attachments;
pub mod blocks;
pub mod dates;
pub mod format;
pub mod inline;
pub mod metadata;
pub mod model;
pub mod properties;
pub mod structure;

pub use blocks::{Attachment, Block, BlockKind, HiddenData, TagColor};
pub use format::DecodeError;
pub use metadata::{DateChange, DateTag, Flag};
pub use model::{
    Document, EditError, FileFormat, IndentUnit, Item, ItemId, ItemKind, LineEnding, Op, Snapshot,
    Stamp, Transaction,
};
