//! The App layer: application state, messages, the update function and effects. Depends only on
//! the document layer, so the same core can run behind another frontend.

pub mod demo;
mod edit_mode;
mod editing;
pub mod external;
pub mod history;
pub mod saving;
pub mod state;
mod structure;
pub mod text_field;
pub mod update;

pub use edit_mode::{Editor, Mode};
pub use external::{DiffLine, Version};
pub use history::{History, Restore};
pub use saving::SaveState;
pub use state::{Access, Notice, Origin, Row, State};
pub use text_field::{Motion, TextEdit};
pub use update::{Disk, Effect, Message};
