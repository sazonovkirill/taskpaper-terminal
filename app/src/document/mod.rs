//! The Document layer: the outline tree and the `.taskpaper` format. Depends on `std` and small pure crates only.

pub mod format;
pub mod model;

pub use format::DecodeError;
pub use model::{Document, FileFormat, IndentUnit, Item, ItemId, ItemKind, LineEnding};
