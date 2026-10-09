//! The command registry: every command a user can give, with its name, the modes it belongs to
//! and its default key bindings in one table ([`REGISTRY`]), and the open Navigator's in
//! another ([`NAVIGATOR`]), which the keymap reads and the
//! menus, the command palette and the shortcuts panel list
//! ([architecture.md §1](../../../docs/specs/architecture.md#1-layers-and-modules),
//! [keybindings.md](../../../docs/specs/keybindings.md)).

mod node;
mod registry;

use std::borrow::Cow;

use jiff::civil::Date;

pub(super) use node::{Focused, NodeCommand, context_commands, node_commands};
pub use registry::{Entry, Group, NAVIGATOR, REGISTRY, shortcuts};

use super::{
    AttachmentsInput, Location, Message, NavigatorInput, NewBlock, NodeInput, PaneChange, Paste,
    QuickAddInput, QuickAddTo, SaveAs, SelectionChange, Sibling, SpecialView, Symbols, TextEdit,
};
use crate::document::{
    Attachment, DateChange, DateTag, Flag, TagColor, format::Typed, inline::Marker,
};

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
    /// Focus the first sibling.
    FocusFirstSibling,
    /// Focus the last sibling.
    FocusLastSibling,
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
    /// Pick the theme from the frontend's themes, which it previews and keeps itself, since colors
    /// are presentation (FR-UI-06).
    ChooseTheme,
    /// Open the settings window over the frontend's settings, which it applies and keeps itself
    /// (FR-SET-02).
    OpenSettings,
    /// Animate this much, which the frontend carries out and keeps itself, since animation is
    /// presentation (FR-UI-11).
    SetAnimations(Animations),
    /// Open or close the shortcuts panel.
    ToggleShortcuts,
    /// Scroll the shortcuts panel down a page, which the frontend carries out, since scrolling
    /// is presentation (D-55).
    ScrollShortcutsDown,
    /// Scroll the shortcuts panel up a page, which the frontend carries out.
    ScrollShortcutsUp,
    /// Zoom into the focused block.
    ZoomIn,
    /// Zoom out to the parent.
    ZoomOut,
    /// Show the whole document.
    GoHome,
    /// Show this special view in the focused pane (D-64, D-80).
    GoToView(SpecialView),
    /// Show the focused pane's breadcrumb at this depth as its root, as
    /// [`Message::GoToCrumb`] counts it; only a breadcrumb's context menu offers it.
    GoToCrumb(usize),
    /// Go back in the zoom history.
    Back,
    /// Go forward in the zoom history.
    Forward,
    /// Open or close the Navigator (D-79).
    ToggleNavigator,
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
    /// Show and change every property of the focused block in the Edit Node dialog.
    EditNode,
    /// Open the attachments panel on the focused block's attachment of this kind (FR-ATT-03).
    OpenAttachment(Attachment),
    /// Find a block by name and go to it.
    GoToSymbol,
    /// Find a project by name and go to it (D-65).
    GoToProject,
    /// Find a block by name and move the selection under it.
    MoveTo,
    /// Add a task or note to a project without leaving the view, with Quick Add opened on the
    /// destination this names (D-96).
    QuickAdd(QuickAddTo),
    /// Find a command by name and give it, which the frontend opens with the bindings it shows
    /// (FR-NAV-06).
    GoToCommand,
    /// Count the blocks and words under the focused block (FR-OPS-09).
    Count,
    /// Write the document to a path the user types and continue with that file (FR-DOC-06).
    SaveAs,
    /// Open the context menu on the focused block, the selection or the open Navigator's focused entry
    /// (FR-KEY-06).
    ContextMenu,
    /// A key of the open Navigator, which its entries' context menu offers (FR-UI-01).
    Navigator(NavigatorInput),
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
    /// Open, focus or close panes.
    Pane(PaneChange),
    /// Make the focused pane narrower, which the frontend carries out, since widths are
    /// presentation (FR-UI-03).
    NarrowPane,
    /// Make the focused pane wider, which the frontend carries out.
    WidenPane,
}

/// How much a frontend animates: the `animations` setting, which the core only names for the
/// commands that switch it (FR-UI-11, [ui.md §9.3](../../../docs/specs/ui.md#93-setting)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Animations {
    /// Every animation.
    Full,
    /// Only the flashes that inform, each in one step, and a static spinner.
    Reduced,
    /// No animation, and a static spinner.
    Off,
}

/// What a frontend knows that a command's message carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inputs {
    /// The local date, which date commands write.
    pub today: Date,
    /// The rows a page down motion moves past.
    pub page_down_rows: usize,
    /// The rows a page up motion moves past.
    pub page_up_rows: usize,
}

impl Command {
    /// The command's name, which the command palette and the shortcuts panel show.
    #[must_use]
    pub fn name(self) -> Cow<'static, str> {
        match self {
            Self::ChangeDate(tag, change) => change_date_name(tag, change).into(),
            Self::ClearDate(tag) => format!("Clear {} Date", date_name(tag)).into(),
            Self::OpenDatePrompt(tag) => format!("Set {} Date", date_name(tag)).into(),
            Self::SetColor(Some(color)) => format!("Color: {}", color.name()).into(),
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
            Self::FocusFirstSibling => "First Sibling".into(),
            Self::FocusLastSibling => "Last Sibling".into(),
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
            Self::ChooseTheme => "Choose Theme".into(),
            Self::OpenSettings => "Open Settings".into(),
            Self::SetAnimations(Animations::Full) => "Animations: Full".into(),
            Self::SetAnimations(Animations::Reduced) => "Animations: Reduced".into(),
            Self::SetAnimations(Animations::Off) => "Animations: Off".into(),
            Self::ToggleShortcuts => "Shortcuts".into(),
            Self::ScrollShortcutsDown => "Scroll Shortcuts Down".into(),
            Self::ScrollShortcutsUp => "Scroll Shortcuts Up".into(),
            Self::ZoomIn => "Zoom In".into(),
            Self::ZoomOut => "Zoom Out".into(),
            Self::GoHome => "Go Home".into(),
            Self::GoToView(view) => format!("Go to {}", view.name()).into(),
            Self::GoToCrumb(_) => "Zoom Here".into(),
            Self::Back => "Back".into(),
            Self::Forward => "Forward".into(),
            Self::ToggleNavigator => "Open Navigator".into(),
            Self::ToggleDone => "Toggle Done".into(),
            Self::ToggleFlag(Flag::Star) => "Toggle Star".into(),
            Self::ToggleFlag(Flag::Bookmark) => "Toggle Bookmark".into(),
            Self::ToggleFlag(Flag::Flagged) => "Toggle Flag".into(),
            Self::CycleColor => "Cycle Color".into(),
            Self::SetColor(None) => "Remove Color".into(),
            Self::OpenColorPicker => "Pick Color".into(),
            Self::SetSection(true) | Self::Retype(Typed::Section) => "Make Section".into(),
            Self::SetSection(false) => "Remove Section".into(),
            Self::OpenTypePicker => "Convert Type".into(),
            Self::Retype(Typed::Project) => "Project".into(),
            Self::Retype(Typed::Task) => "Task".into(),
            Self::Retype(Typed::Text) => "Note".into(),
            Self::EditNode => "Edit Node".into(),
            Self::OpenAttachment(Attachment::Note) => "Open Note".into(),
            Self::OpenAttachment(Attachment::Comments) => "Open Comments".into(),
            Self::GoToSymbol => "Go to Symbol".into(),
            Self::GoToProject => "Go to Project".into(),
            Self::MoveTo => "Move to…".into(),
            Self::QuickAdd(QuickAddTo::Chosen) => "Quick Add".into(),
            Self::QuickAdd(QuickAddTo::Inbox) => "Quick Add to Inbox".into(),
            Self::QuickAdd(QuickAddTo::Today) => "Quick Add to Today".into(),
            Self::GoToCommand => "Go to Command".into(),
            Self::Count => "Count Blocks and Words".into(),
            Self::SaveAs => "Save As".into(),
            Self::ContextMenu => "Context Menu".into(),
            Self::Navigator(input) => navigator_name(input),
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
            Self::Select(change) => selection_name(change).into(),
            Self::StopEditing => "Stop Editing".into(),
            Self::OpenCommandMenu => "Command Menu".into(),
            Self::NewNext => "Stop Editing and Add Next Block".into(),
            Self::CycleType => "Cycle Type".into(),
            Self::EditPrevious => "Edit Previous Block".into(),
            Self::EditNext => "Edit Next Block".into(),
            Self::ToggleMarker(Marker::Bold) => "Bold".into(),
            Self::ToggleMarker(Marker::Code) => "Inline Code".into(),
            Self::Pane(change) => pane_name(change),
            Self::NarrowPane => "Narrow Pane".into(),
            Self::WidenPane => "Widen Pane".into(),
        }
    }

    /// The message that gives the command, carrying what it needs of `inputs`, or `None` for
    /// [`Command::Retype`], which changes an edit session rather than sending a message, for
    /// [`Command::ToggleWrap`], [`Command::SetAnimations`], [`Command::NarrowPane`],
    /// [`Command::WidenPane`] and the scrolling of the shortcuts panel, which the frontend carries
    /// out, and for [`Command::GoToCommand`],
    /// [`Command::ChooseTheme`] and [`Command::OpenSettings`], whose messages carry the bindings,
    /// the themes and the settings the frontend shows.
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
                rows: inputs.page_down_rows,
            },
            Self::PageUp => Message::PageUp {
                rows: inputs.page_up_rows,
            },
            Self::FocusParent => Message::FocusParent,
            Self::FocusNextSibling => Message::FocusSibling(Sibling::Next),
            Self::FocusPreviousSibling => Message::FocusSibling(Sibling::Previous),
            Self::FocusFirstSibling => Message::FocusSibling(Sibling::First),
            Self::FocusLastSibling => Message::FocusSibling(Sibling::Last),
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
            Self::ToggleShortcuts => Message::ToggleShortcuts,
            Self::ZoomIn => Message::ZoomIn,
            Self::ZoomOut => Message::ZoomOut,
            Self::GoHome => Message::GoTo(Location::Outline(None)),
            Self::GoToView(view) => Message::GoTo(Location::View(view)),
            Self::GoToCrumb(depth) => Message::GoToCrumb(depth),
            Self::Back => Message::Back,
            Self::Forward => Message::Forward,
            Self::ToggleNavigator => Message::ToggleNavigator,
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
            Self::Retype(_)
            | Self::ToggleWrap
            | Self::ChooseTheme
            | Self::OpenSettings
            | Self::SetAnimations(_)
            | Self::GoToCommand
            | Self::NarrowPane
            | Self::WidenPane
            | Self::ScrollShortcutsDown
            | Self::ScrollShortcutsUp => return None,
            Self::EditNode => Message::NodeEditor(NodeInput::Open),
            Self::OpenAttachment(kind) => Message::Attachments(AttachmentsInput::Open(kind)),
            Self::GoToSymbol => Message::OpenGoToSymbol(Symbols::All),
            Self::GoToProject => Message::OpenGoToSymbol(Symbols::Projects),
            Self::MoveTo => Message::OpenMoveTo,
            Self::QuickAdd(to) => Message::QuickAdd(QuickAddInput::Open(to)),
            Self::Count => Message::Count,
            Self::SaveAs => Message::SaveAs(SaveAs::Open),
            Self::ContextMenu => Message::OpenContextMenu,
            Self::Navigator(input) => Message::Navigator(input),
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
            Self::Pane(change) => Message::Pane(change),
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

/// The name of [`Command::ChangeDate`] making `change` to the date `tag`.
fn change_date_name(tag: DateTag, change: DateChange) -> String {
    let change = match change {
        DateChange::Today => "Today",
        DateChange::Tomorrow => "Tomorrow",
        DateChange::Earlier => "One Day Earlier",
        DateChange::Later => "One Day Later",
    };
    format!("{} {change}", date_name(tag))
}

/// The name of [`Command::Pane`] making `change`.
fn pane_name(change: PaneChange) -> Cow<'static, str> {
    match change {
        PaneChange::Open | PaneChange::OpenCrumb(_) => "Open in New Pane".into(),
        PaneChange::OpenLocation => "Open Location in New Pane".into(),
        PaneChange::Focus(at) => format!("Focus Pane {}", at + 1).into(),
        PaneChange::FocusLeft => "Focus Pane Left".into(),
        PaneChange::FocusRight => "Focus Pane Right".into(),
        PaneChange::Close => "Close Pane".into(),
    }
}

/// The name of the command that changes the selection as `change` says.
fn selection_name(change: SelectionChange) -> &'static str {
    match change {
        SelectionChange::ExtendDown => "Extend Selection Down",
        SelectionChange::ExtendUp => "Extend Selection Up",
        SelectionChange::Toggle => "Toggle Selected",
        SelectionChange::All => "Select All",
        SelectionChange::Clear => "Clear Selection",
    }
}

/// The name of [`Command::Navigator`] with `input`.
fn navigator_name(input: NavigatorInput) -> Cow<'static, str> {
    match input {
        NavigatorInput::Next => "Next Entry".into(),
        NavigatorInput::Previous => "Previous Entry".into(),
        NavigatorInput::Collapse => "Collapse Entry".into(),
        NavigatorInput::Expand => "Expand Entry".into(),
        NavigatorInput::Choose => "Go to Entry".into(),
        NavigatorInput::Leave => "Return to Outline".into(),
        NavigatorInput::Focus(at) => format!("Focus Entry {}", at + 1).into(),
        NavigatorInput::GoToInbox => "Go to Inbox".into(),
        NavigatorInput::Type(c) => format!("Type {c}").into(),
        NavigatorInput::Erase => "Erase Filter Character".into(),
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
        page_down_rows: 7,
        page_up_rows: 5,
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
            (Command::ToggleShortcuts, "Shortcuts"),
            (Command::ScrollShortcutsDown, "Scroll Shortcuts Down"),
            (Command::ScrollShortcutsUp, "Scroll Shortcuts Up"),
            (Command::ContextMenu, "Context Menu"),
            (Command::Navigator(NavigatorInput::Next), "Next Entry"),
            (
                Command::Navigator(NavigatorInput::Previous),
                "Previous Entry",
            ),
            (
                Command::Navigator(NavigatorInput::Collapse),
                "Collapse Entry",
            ),
            (Command::Navigator(NavigatorInput::Expand), "Expand Entry"),
            (Command::Navigator(NavigatorInput::Choose), "Go to Entry"),
            (
                Command::Navigator(NavigatorInput::Leave),
                "Return to Outline",
            ),
            (
                Command::Navigator(NavigatorInput::Focus(2)),
                "Focus Entry 3",
            ),
            (Command::Navigator(NavigatorInput::GoToInbox), "Go to Inbox"),
            (Command::Navigator(NavigatorInput::Type('w')), "Type w"),
            (
                Command::Navigator(NavigatorInput::Erase),
                "Erase Filter Character",
            ),
            (Command::GoToCrumb(1), "Zoom Here"),
            (Command::GoToView(SpecialView::Today), "Go to Today"),
            (Command::GoToView(SpecialView::Upcoming), "Go to Upcoming"),
            (Command::GoToView(SpecialView::Overdue), "Go to Overdue"),
            (Command::GoToView(SpecialView::Starred), "Go to Starred"),
            (Command::GoToProject, "Go to Project"),
            (Command::QuickAdd(QuickAddTo::Chosen), "Quick Add"),
            (Command::QuickAdd(QuickAddTo::Inbox), "Quick Add to Inbox"),
            (Command::QuickAdd(QuickAddTo::Today), "Quick Add to Today"),
            (Command::Pane(PaneChange::OpenCrumb(1)), "Open in New Pane"),
            (Command::Pane(PaneChange::Open), "Open in New Pane"),
            (
                Command::Pane(PaneChange::OpenLocation),
                "Open Location in New Pane",
            ),
            (Command::Pane(PaneChange::Focus(0)), "Focus Pane 1"),
            (Command::Pane(PaneChange::Focus(8)), "Focus Pane 9"),
            (Command::Pane(PaneChange::FocusLeft), "Focus Pane Left"),
            (Command::Pane(PaneChange::FocusRight), "Focus Pane Right"),
            (Command::Pane(PaneChange::Close), "Close Pane"),
            (Command::NarrowPane, "Narrow Pane"),
            (Command::WidenPane, "Widen Pane"),
        ] {
            assert_eq!(command.name(), name);
        }
    }

    #[test]
    fn a_command_sends_its_message_with_the_date_and_page_it_needs() {
        let today = INPUTS.today;
        for (command, message) in [
            (Command::PageDown, Message::PageDown { rows: 7 }),
            (Command::PageUp, Message::PageUp { rows: 5 }),
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
            (Command::ToggleShortcuts, Message::ToggleShortcuts),
            (Command::Count, Message::Count),
            (Command::SaveAs, Message::SaveAs(SaveAs::Open)),
            (Command::ContextMenu, Message::OpenContextMenu),
            (
                Command::Navigator(NavigatorInput::Choose),
                Message::Navigator(NavigatorInput::Choose),
            ),
            (
                Command::Pane(PaneChange::Focus(2)),
                Message::Pane(PaneChange::Focus(2)),
            ),
            (Command::GoToCrumb(1), Message::GoToCrumb(1)),
            (Command::GoHome, Message::GoTo(Location::Outline(None))),
            (
                Command::GoToView(SpecialView::Today),
                Message::GoTo(Location::View(SpecialView::Today)),
            ),
            (
                Command::GoToView(SpecialView::Upcoming),
                Message::GoTo(Location::View(SpecialView::Upcoming)),
            ),
            (
                Command::GoToView(SpecialView::Starred),
                Message::GoTo(Location::View(SpecialView::Starred)),
            ),
            (Command::GoToSymbol, Message::OpenGoToSymbol(Symbols::All)),
            (
                Command::GoToProject,
                Message::OpenGoToSymbol(Symbols::Projects),
            ),
            (
                Command::QuickAdd(QuickAddTo::Today),
                Message::QuickAdd(QuickAddInput::Open(QuickAddTo::Today)),
            ),
        ] {
            assert_eq!(command.message(&INPUTS), Some(message), "{command:?}");
        }
    }

    // D-44: a type change is a change of the edit session, which no message makes; wrapping is
    // the frontend's (FR-UI-05), and so are the bindings Go to Command shows (FR-NAV-06), the
    // widths of panes (navigation.md §5), the scrolling of the shortcuts panel (D-55), the
    // themes (FR-UI-06) and the settings (FR-SET-02).
    #[test]
    fn retyping_toggling_wrap_going_to_a_command_resizing_and_scrolling_send_no_message() {
        assert_eq!(Command::Retype(Typed::Project).message(&INPUTS), None);
        assert_eq!(Command::ToggleWrap.message(&INPUTS), None);
        assert_eq!(Command::GoToCommand.message(&INPUTS), None);
        assert_eq!(Command::ChooseTheme.message(&INPUTS), None);
        assert_eq!(Command::OpenSettings.message(&INPUTS), None);
        assert_eq!(
            Command::SetAnimations(Animations::Off).message(&INPUTS),
            None
        );
        assert_eq!(Command::NarrowPane.message(&INPUTS), None);
        assert_eq!(Command::WidenPane.message(&INPUTS), None);
        assert_eq!(Command::ScrollShortcutsDown.message(&INPUTS), None);
        assert_eq!(Command::ScrollShortcutsUp.message(&INPUTS), None);
    }

    #[test]
    fn every_command_of_the_registry_but_those_sends_a_message() {
        for entry in REGISTRY {
            let none = matches!(
                entry.command,
                Command::Retype(_)
                    | Command::ToggleWrap
                    | Command::GoToCommand
                    | Command::ChooseTheme
                    | Command::OpenSettings
                    | Command::SetAnimations(_)
                    | Command::NarrowPane
                    | Command::WidenPane
                    | Command::ScrollShortcutsDown
                    | Command::ScrollShortcutsUp
            );
            assert_eq!(entry.command.message(&INPUTS).is_none(), none, "{entry:?}");
        }
    }
}
