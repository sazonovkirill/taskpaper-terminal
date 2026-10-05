//! The Document layer: the outline tree and the `.taskpaper` format. Depends on `std` and small pure crates only.

pub mod blocks;
pub mod format;
pub mod metadata;
pub mod model;
pub mod structure;

pub use blocks::{Block, BlockKind};
pub use format::DecodeError;
pub use metadata::{DateChange, DateTag};
pub use model::{
    Document, EditError, FileFormat, IndentUnit, Item, ItemId, ItemKind, LineEnding, Op, Snapshot,
    Transaction,
};
