//! The App layer: application state, messages, the update function and effects. Depends only on
//! the document layer, so the same core can run behind another frontend.

pub mod history;
pub mod saving;
pub mod state;
pub mod update;

pub use history::{History, Restore};
pub use saving::SaveState;
pub use state::{Access, Notice, Row, State};
pub use update::{Disk, Effect, Message};
