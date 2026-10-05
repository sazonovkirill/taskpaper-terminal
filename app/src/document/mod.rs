//! The Document layer: the outline tree and the `.taskpaper` format. Depends on `std` and small pure crates only.

pub mod blocks;
pub mod dates;
pub mod format;
pub mod inline;
pub mod metadata;
pub mod model;
pub mod structure;

pub use blocks::{Block, BlockKind, Details, HiddenData, TagColor};
pub use format::DecodeError;
pub use metadata::{ConvertError, DateChange, DateTag, Flag};
pub use model::{
    Document, EditError, FileFormat, IndentUnit, Item, ItemId, ItemKind, LineEnding, Op, Snapshot,
    Transaction,
};
