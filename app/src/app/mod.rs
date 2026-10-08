//! The App layer: application state, messages, the update function and effects. Depends only on
//! the document layer, so the same core can run behind another frontend.

mod above;
mod clipboard;
mod color_picker;
mod colors;
mod command_menu;
pub mod commands;
mod context_menu;
mod count;
mod date_prompt;
pub mod demo;
mod edit_mode;
mod editing;
pub mod external;
mod finder;
mod folds;
mod go_to_command;
mod go_to_symbol;
pub mod history;
mod links;
mod logbook;
mod memo;
mod move_to;
mod navigator;
mod node_editor;
mod overdue;
mod pane;
mod picker;
mod quick_add;
mod rows;
mod save_as;
pub mod saving;
mod selection;
mod settings;
mod shortcuts;
mod special_view;
pub mod state;
mod structure;
pub mod text_field;
mod text_path;
mod theme_picker;
mod today;
mod today_order;
mod type_picker;
mod upcoming;
pub mod update;
mod view_state;

pub use clipboard::Paste;
pub use command_menu::{CommandMenuView, MenuInput, MenuRow};
pub use commands::{Animations, Command, Inputs};
pub use date_prompt::{DateInput, DatePromptView};
pub use edit_mode::{Editor, Mode};
pub use external::{DiffLine, Version};
pub use finder::{Candidate, Detail, Entries, FinderView};
pub use go_to_symbol::Symbols;
pub use history::{History, Restore};
pub use navigator::{Fold, NavigatorEntry, NavigatorInput, NavigatorView};
pub use node_editor::{
    DateValue, FieldOutcome, FieldView, NodeEditorView, NodeInput, NodeRow, TagView,
};
pub use pane::{PaneChange, PaneId, PaneView};
pub use picker::PickerView;
pub use quick_add::{Missing, QuickAddInput, QuickAddTo, QuickAddView};
pub use rows::Rows;
pub use save_as::{SaveAs, SaveAsStatus, SaveAsView};
pub use saving::{DocumentText, SaveState};
pub use selection::SelectionChange;
pub use settings::{
    Setting, SettingKind, SettingRow, SettingValue, SettingsColumn, SettingsInput, SettingsView,
    Typed,
};
pub use special_view::{Heading, SpecialView, ViewGroup};
pub use state::{Access, Location, Notice, Origin, Row, State};
pub use structure::NewBlock;
pub use text_field::{Motion, TextEdit};
pub use type_picker::Conversion;
pub use update::{Configuration, Disk, Effect, Menu, Message};
pub use view_state::ViewState;
