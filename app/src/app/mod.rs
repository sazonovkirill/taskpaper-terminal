//! The App layer: application state, messages, the update function and effects. Depends only on
//! the document layer, so the same core can run behind another frontend.

mod clipboard;
mod color_picker;
pub mod commands;
mod date_prompt;
pub mod demo;
mod details;
mod edit_mode;
mod editing;
pub mod external;
mod finder;
mod go_to_symbol;
pub mod history;
mod links;
mod move_to;
mod picker;
pub mod saving;
mod selection;
mod shortcuts;
mod sidebar;
pub mod state;
mod structure;
pub mod text_field;
mod today;
mod type_picker;
pub mod update;

pub use clipboard::Paste;
pub use commands::{Command, Inputs};
pub use date_prompt::{DateInput, DatePromptView};
pub use edit_mode::{CommandMenuView, Editor, MenuInput, MenuRow, Mode};
pub use external::{DiffLine, Version};
pub use finder::{Candidate, FinderView};
pub use history::{History, Restore};
pub use picker::PickerView;
pub use saving::SaveState;
pub use selection::SelectionChange;
pub use sidebar::{Fold, SidebarEntry, SidebarInput, SidebarView};
pub use state::{Access, Location, Notice, Origin, Row, State};
pub use structure::NewBlock;
pub use text_field::{Motion, TextEdit};
pub use type_picker::Conversion;
pub use update::{Disk, Effect, Message};
