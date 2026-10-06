//! The App layer: application state, messages, the update function and effects. Depends only on
//! the document layer, so the same core can run behind another frontend.

mod clipboard;
mod color_picker;
mod date_prompt;
pub mod demo;
mod details;
mod edit_mode;
mod editing;
pub mod external;
pub mod history;
mod links;
mod node_commands;
mod picker;
pub mod saving;
mod selection;
pub mod state;
mod structure;
pub mod text_field;
mod type_picker;
pub mod update;

pub use clipboard::Paste;
pub use date_prompt::{DateInput, DatePromptView};
pub use edit_mode::{CommandMenuView, Editor, MenuInput, MenuRow, Mode};
pub use external::{DiffLine, Version};
pub use history::{History, Restore};
pub use picker::PickerView;
pub use saving::SaveState;
pub use selection::SelectionChange;
pub use state::{Access, Notice, Origin, Row, State};
pub use text_field::{Motion, TextEdit};
pub use type_picker::Conversion;
pub use update::{Disk, Effect, Message};
