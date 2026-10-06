//! The command registry: every command a user can give, with its name, the modes it belongs to
//! and its default key bindings in one table ([`REGISTRY`]), which the keymap reads and the
//! menus, the command palette and the shortcuts panel list
//! ([architecture.md §1](../../../docs/specs/architecture.md#1-layers-and-modules),
//! [keybindings.md](../../../docs/specs/keybindings.md)).

mod node;
mod registry;

use std::borrow::Cow;

use jiff::civil::Date;

pub(super) use node::{NodeCommand, node_commands};
pub use registry::{Entry, REGISTRY};

use super::{Message, NewBlock, Paste, SelectionChange, TextEdit};
use crate::document::{DateChange, DateTag, Flag, TagColor, format::Typed, inline::Marker};

/// A command: one user intent, whatever key, menu or palette gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Leave the app, saving first.
    Quit,
    /// Focus the next visible block.
    FocusNext,
    /// Focus the previous visible block.
    FocusPrevious,
    /// Focus the first block.
    FocusFirst,
    /// Focus the last block.
    FocusLast,
    /// Move focus down a page.
    PageDown,
    /// Move focus up a page.
    PageUp,
    /// Focus the parent block.
    FocusParent,
    /// Focus the next sibling.
    FocusNextSibling,
    /// Focus the previous sibling.
    FocusPreviousSibling,
    /// Focus the last child.
    FocusLastChild,
    /// Collapse, or go to the parent.
    Collapse,
    /// Expand, or go to the first child.
    Expand,
    /// Toggle the focused block's fold.
    ToggleFold,
    /// Toggle the fold of the focused block's whole subtree.
    ToggleSubtree,
    /// Expand every block by one level.
    ExpandLevel,
    /// Collapse every block by one level.
    CollapseLevel,
    /// Expand every block.
    ExpandAll,
    /// Collapse every block.
    CollapseAll,
    /// Show or hide completed blocks.
    ToggleShowDone,
    /// Turn word wrap on or off, which the frontend carries out itself, since wrapping is
    /// presentation (FR-UI-05).
    ToggleWrap,
    /// Zoom into the focused block.
    ZoomIn,
    /// Zoom out to the parent.
    ZoomOut,
    /// Show the whole document.
    GoHome,
    /// Go back in the zoom history.
    Back,
    /// Go forward in the zoom history.
    Forward,
    /// Complete or uncomplete.
    ToggleDone,
    /// Change a date without a prompt.
    ChangeDate(DateTag, DateChange),
    /// Remove a date.
    ClearDate(DateTag),
    /// Set a date with the date prompt.
    OpenDatePrompt(DateTag),
    /// Add or remove a bare tag.
    ToggleFlag(Flag),
    /// Give the next color.
    CycleColor,
    /// Give a color, or none.
    SetColor(Option<TagColor>),
    /// Pick a color in the color picker.
    OpenColorPicker,
    /// Make text blocks sections, or with `false` plain text.
    SetSection(bool),
    /// Convert the type in the type picker.
    OpenTypePicker,
    /// Give the block being edited a type, as a change of its edit session (D-44); only the
    /// command menu offers it.
    Retype(Typed),
    /// Show the details popup.
    OpenDetails,
    /// Find a block by name and go to it.
    GoToSymbol,
    /// Undo.
    Undo,
    /// Redo.
    Redo,
    /// Edit the focused block.
    Edit,
    /// Add a block where and of the type this says.
    New(NewBlock, Option<Typed>),
    /// Delete.
    Delete,
    /// Duplicate.
    Duplicate,
    /// Copy.
    Copy,
    /// Cut.
    Cut,
    /// Paste.
    Paste(Paste),
    /// Indent.
    Indent,
    /// Outdent.
    Outdent,
    /// Move up.
    MoveUp,
    /// Move down.
    MoveDown,
    /// Open the focused block's link.
    OpenUrl,
    /// Group under a new block.
    Group,
    /// Change the selection.
    Select(SelectionChange),
    /// Stop editing.
    StopEditing,
    /// Open the command menu while editing.
    OpenCommandMenu,
    /// Stop editing and add the next block.
    NewNext,
    /// Cycle the edited block's type.
    CycleType,
    /// Edit the previous block.
    EditPrevious,
    /// Edit the next block.
    EditNext,
    /// Toggle inline formatting around the edited text's selection.
    ToggleMarker(Marker),
}

/// What a frontend knows that a command's message carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inputs {
    /// The local date, which date commands write.
    pub today: Date,
    /// The rows a page motion moves.
    pub page_rows: usize,
}

impl Command {
    /// The command's name, which the command palette and the shortcuts panel show.
    #[must_use]
    pub fn name(self) -> Cow<'static, str> {
        match self {
            Self::ChangeDate(tag, change) => {
                let change = match change {
                    DateChange::Today => "Today",
                    DateChange::Tomorrow => "Tomorrow",
                    DateChange::Earlier => "One Day Earlier",
                    DateChange::Later => "One Day Later",
                };
                format!("{} {change}", date_name(tag)).into()
            }
            Self::ClearDate(tag) => format!("Clear {} Date", date_name(tag)).into(),
            Self::OpenDatePrompt(tag) => format!("Set {} Date", date_name(tag)).into(),
            Self::SetColor(Some(color)) => format!("Color: {}", color_name(color)).into(),
            Self::New(at, typed) => new_name(at, typed).into(),
            Self::Quit => "Quit".into(),
            Self::FocusNext => "Next Block".into(),
            Self::FocusPrevious => "Previous Block".into(),
            Self::FocusFirst => "First Block".into(),
            Self::FocusLast => "Last Block".into(),
            Self::PageDown => "Page Down".into(),
            Self::PageUp => "Page Up".into(),
            Self::FocusParent => "Parent".into(),
            Self::FocusNextSibling => "Next Sibling".into(),
            Self::FocusPreviousSibling => "Previous Sibling".into(),
            Self::FocusLastChild => "Last Child".into(),
            Self::Collapse => "Collapse".into(),
            Self::Expand => "Expand".into(),
            Self::ToggleFold => "Toggle Fold".into(),
            Self::ToggleSubtree => "Toggle Subtree Fold".into(),
            Self::ExpandLevel => "Expand One Level".into(),
            Self::CollapseLevel => "Collapse One Level".into(),
            Self::ExpandAll => "Expand All".into(),
            Self::CollapseAll => "Collapse All".into(),
            Self::ToggleShowDone => "Show/Hide Completed".into(),
            Self::ToggleWrap => "Toggle Word Wrap".into(),
            Self::ZoomIn => "Zoom In".into(),
            Self::ZoomOut => "Zoom Out".into(),
            Self::GoHome => "Go Home".into(),
            Self::Back => "Back".into(),
            Self::Forward => "Forward".into(),
            Self::ToggleDone => "Toggle Done".into(),
            Self::ToggleFlag(Flag::Today) => "Toggle Today".into(),
            Self::ToggleFlag(Flag::Bookmark) => "Toggle Bookmark".into(),
            Self::CycleColor => "Cycle Color".into(),
            Self::SetColor(None) => "Remove Color".into(),
            Self::OpenColorPicker => "Pick Color".into(),
            Self::SetSection(true) | Self::Retype(Typed::Section) => "Make Section".into(),
            Self::SetSection(false) => "Remove Section".into(),
            Self::OpenTypePicker => "Convert Type".into(),
            Self::Retype(Typed::Project) => "Project".into(),
            Self::Retype(Typed::Task) => "Task".into(),
            Self::Retype(Typed::Text) => "Note".into(),
            Self::OpenDetails => "Show Details".into(),
            Self::GoToSymbol => "Go to Symbol".into(),
            Self::Undo => "Undo".into(),
            Self::Redo => "Redo".into(),
            Self::Edit => "Edit".into(),
            Self::Delete => "Delete".into(),
            Self::Duplicate => "Duplicate".into(),
            Self::Copy => "Copy".into(),
            Self::Cut => "Cut".into(),
            Self::Paste(Paste::Below) => "Paste Below".into(),
            Self::Paste(Paste::Above) => "Paste Above".into(),
            Self::Indent => "Indent".into(),
            Self::Outdent => "Outdent".into(),
            Self::MoveUp => "Move Up".into(),
            Self::MoveDown => "Move Down".into(),
            Self::OpenUrl => "Open Link".into(),
            Self::Group => "Group".into(),
            Self::Select(SelectionChange::ExtendDown) => "Extend Selection Down".into(),
            Self::Select(SelectionChange::ExtendUp) => "Extend Selection Up".into(),
            Self::Select(SelectionChange::Toggle) => "Toggle Selected".into(),
            Self::Select(SelectionChange::All) => "Select All".into(),
            Self::Select(SelectionChange::Clear) => "Clear Selection".into(),
            Self::StopEditing => "Stop Editing".into(),
            Self::OpenCommandMenu => "Command Menu".into(),
            Self::NewNext => "Stop Editing and Add Next Block".into(),
            Self::CycleType => "Cycle Type".into(),
            Self::EditPrevious => "Edit Previous Block".into(),
            Self::EditNext => "Edit Next Block".into(),
            Self::ToggleMarker(Marker::Bold) => "Bold".into(),
            Self::ToggleMarker(Marker::Code) => "Inline Code".into(),
        }
    }

    /// The message that gives the command, carrying what it needs of `inputs`, or `None` for
    /// [`Command::Retype`], which changes an edit session rather than sending a message, and for
    /// [`Command::ToggleWrap`], which the frontend carries out.
    #[must_use]
    pub fn message(self, inputs: &Inputs) -> Option<Message> {
        let today = inputs.today;
        Some(match self {
            Self::Quit => Message::Quit,
            Self::FocusNext => Message::FocusNext,
            Self::FocusPrevious => Message::FocusPrevious,
            Self::FocusFirst => Message::FocusFirst,
            Self::FocusLast => Message::FocusLast,
            Self::PageDown => Message::PageDown {
                rows: inputs.page_rows,
            },
            Self::PageUp => Message::PageUp {
                rows: inputs.page_rows,
            },
            Self::FocusParent => Message::FocusParent,
            Self::FocusNextSibling => Message::FocusNextSibling,
            Self::FocusPreviousSibling => Message::FocusPreviousSibling,
            Self::FocusLastChild => Message::FocusLastChild,
            Self::Collapse => Message::Collapse,
            Self::Expand => Message::Expand,
            Self::ToggleFold => Message::ToggleFold,
            Self::ToggleSubtree => Message::ToggleSubtree,
            Self::ExpandLevel => Message::ExpandLevel,
            Self::CollapseLevel => Message::CollapseLevel,
            Self::ExpandAll => Message::ExpandAll,
            Self::CollapseAll => Message::CollapseAll,
            Self::ToggleShowDone => Message::ToggleShowDone,
            Self::ZoomIn => Message::ZoomIn,
            Self::ZoomOut => Message::ZoomOut,
            Self::GoHome => Message::GoHome,
            Self::Back => Message::Back,
            Self::Forward => Message::Forward,
            Self::ToggleDone => Message::ToggleDone { today },
            Self::ChangeDate(tag, change) => Message::ChangeDate { tag, change, today },
            Self::ClearDate(tag) => Message::ClearDate(tag),
            Self::OpenDatePrompt(tag) => Message::OpenDatePrompt { tag, today },
            Self::ToggleFlag(flag) => Message::ToggleFlag(flag),
            Self::CycleColor => Message::CycleColor,
            Self::SetColor(color) => Message::SetColor(color),
            Self::OpenColorPicker => Message::OpenColorPicker,
            Self::SetSection(on) => Message::SetSection(on),
            Self::OpenTypePicker => Message::OpenTypePicker,
            Self::Retype(_) | Self::ToggleWrap => return None,
            Self::OpenDetails => Message::OpenDetails,
            Self::GoToSymbol => Message::OpenGoToSymbol,
            Self::Undo => Message::Undo,
            Self::Redo => Message::Redo,
            Self::Edit => Message::EditAtEnd,
            Self::New(at, typed) => Message::New(at, typed),
            Self::Delete => Message::Delete,
            Self::Duplicate => Message::Duplicate,
            Self::Copy => Message::Copy,
            Self::Cut => Message::Cut,
            Self::Paste(at) => Message::Paste(at),
            Self::Indent => Message::Indent,
            Self::Outdent => Message::Outdent,
            Self::MoveUp => Message::MoveUp,
            Self::MoveDown => Message::MoveDown,
            Self::OpenUrl => Message::OpenUrl,
            Self::Group => Message::Group,
            Self::Select(change) => Message::Select(change),
            Self::StopEditing => Message::StopEditing,
            Self::OpenCommandMenu => Message::OpenCommandMenu { today },
            Self::NewNext => Message::NewNext,
            Self::CycleType => Message::CycleType,
            Self::EditPrevious => Message::EditPrevious,
            Self::EditNext => Message::EditNext,
            Self::ToggleMarker(marker) => Message::Text(TextEdit::Toggle(marker)),
        })
    }
}

/// How a date's tag is named in command names.
fn date_name(tag: DateTag) -> &'static str {
    match tag {
        DateTag::Start => "Start",
        DateTag::Due => "Due",
    }
}

/// How a color is named in command names.
fn color_name(color: TagColor) -> &'static str {
    match color {
        TagColor::Red => "Red",
        TagColor::Orange => "Orange",
        TagColor::Yellow => "Yellow",
        TagColor::Green => "Green",
        TagColor::Blue => "Blue",
        TagColor::Violet => "Violet",
    }
}

/// The name of [`Command::New`] adding a block of the type `typed` at `at`.
fn new_name(at: NewBlock, typed: Option<Typed>) -> String {
    let typed = match typed {
        None => "Block",
        Some(Typed::Project) => "Project",
        Some(Typed::Section) => "Section",
        Some(Typed::Task) => "Task",
        Some(Typed::Text) => "Note",
    };
    let at = match at {
        NewBlock::Below => "Below",
        NewBlock::Above => "Above",
        NewBlock::FirstChild => "as First Child",
        NewBlock::LastChild => "as Last Child",
        NewBlock::Next => "Next",
        NewBlock::Start => "at Start",
        NewBlock::End => "at End",
    };
    format!("New {typed} {at}")
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::app::Mode;

    const INPUTS: Inputs = Inputs {
        today: jiff::civil::date(2026, 10, 6),
        page_rows: 7,
    };

    #[test]
    fn every_command_of_a_mode_has_a_name_of_its_own() {
        for mode in [Mode::Navigate, Mode::Edit] {
            let names: Vec<_> = REGISTRY
                .iter()
                .filter(|entry| entry.mode == mode)
                .map(|entry| entry.command.name())
                .collect();
            let distinct: HashSet<_> = names.iter().collect();
            assert_eq!(distinct.len(), names.len(), "{mode:?}");
            assert!(names.iter().all(|name| !name.is_empty()), "{mode:?}");
        }
    }

    #[test]
    fn names_say_what_the_parts_of_a_command_are() {
        for (command, name) in [
            (
                Command::ChangeDate(DateTag::Start, DateChange::Today),
                "Start Today",
            ),
            (
                Command::ChangeDate(DateTag::Due, DateChange::Tomorrow),
                "Due Tomorrow",
            ),
            (
                Command::ChangeDate(DateTag::Start, DateChange::Earlier),
                "Start One Day Earlier",
            ),
            (
                Command::ChangeDate(DateTag::Due, DateChange::Later),
                "Due One Day Later",
            ),
            (Command::ClearDate(DateTag::Due), "Clear Due Date"),
            (Command::OpenDatePrompt(DateTag::Start), "Set Start Date"),
            (Command::SetColor(Some(TagColor::Orange)), "Color: Orange"),
            (Command::SetColor(None), "Remove Color"),
            (Command::New(NewBlock::Below, None), "New Block Below"),
            (
                Command::New(NewBlock::FirstChild, Some(Typed::Project)),
                "New Project as First Child",
            ),
            (
                Command::New(NewBlock::LastChild, Some(Typed::Section)),
                "New Section as Last Child",
            ),
            (
                Command::New(NewBlock::Next, Some(Typed::Task)),
                "New Task Next",
            ),
            (
                Command::New(NewBlock::Start, Some(Typed::Text)),
                "New Note at Start",
            ),
            (Command::New(NewBlock::End, None), "New Block at End"),
            (Command::New(NewBlock::Above, None), "New Block Above"),
            (Command::Retype(Typed::Text), "Note"),
            (Command::Retype(Typed::Section), "Make Section"),
            (Command::ToggleMarker(Marker::Code), "Inline Code"),
        ] {
            assert_eq!(command.name(), name);
        }
    }

    #[test]
    fn a_command_sends_its_message_with_the_date_and_page_it_needs() {
        let today = INPUTS.today;
        for (command, message) in [
            (Command::PageDown, Message::PageDown { rows: 7 }),
            (Command::PageUp, Message::PageUp { rows: 7 }),
            (Command::ToggleDone, Message::ToggleDone { today }),
            (
                Command::ChangeDate(DateTag::Due, DateChange::Later),
                Message::ChangeDate {
                    tag: DateTag::Due,
                    change: DateChange::Later,
                    today,
                },
            ),
            (
                Command::OpenDatePrompt(DateTag::Start),
                Message::OpenDatePrompt {
                    tag: DateTag::Start,
                    today,
                },
            ),
            (Command::OpenCommandMenu, Message::OpenCommandMenu { today }),
            (Command::Edit, Message::EditAtEnd),
            (
                Command::ToggleMarker(Marker::Bold),
                Message::Text(TextEdit::Toggle(Marker::Bold)),
            ),
            (
                Command::New(NewBlock::Above, Some(Typed::Task)),
                Message::New(NewBlock::Above, Some(Typed::Task)),
            ),
            (
                Command::ClearDate(DateTag::Due),
                Message::ClearDate(DateTag::Due),
            ),
            (Command::NewNext, Message::NewNext),
        ] {
            assert_eq!(command.message(&INPUTS), Some(message), "{command:?}");
        }
    }

    // D-44: a type change is a change of the edit session, which no message makes; wrapping is
    // the frontend's (FR-UI-05).
    #[test]
    fn retyping_and_toggling_wrap_send_no_message() {
        assert_eq!(Command::Retype(Typed::Project).message(&INPUTS), None);
        assert_eq!(Command::ToggleWrap.message(&INPUTS), None);
    }

    #[test]
    fn every_command_of_the_registry_but_retyping_and_toggling_wrap_sends_a_message() {
        for entry in REGISTRY {
            let none = matches!(entry.command, Command::Retype(_) | Command::ToggleWrap);
            assert_eq!(entry.command.message(&INPUTS).is_none(), none, "{entry:?}");
        }
    }
}
