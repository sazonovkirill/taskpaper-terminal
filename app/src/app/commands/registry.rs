//! The registry's tables: every command with the mode it belongs to and its default bindings
//! ([keybindings.md §2–3](../../../../docs/specs/keybindings.md#2-navigate-mode)), the keys of
//! the open sidebar (§4), and the commands of each mode the shortcuts panel lists, grouped
//! (FR-KEY-04).

use super::Command;
use crate::{
    app::{Mode, NewBlock, PaneChange, Paste, SelectionChange, SidebarInput, SpecialView},
    document::{DateChange, DateTag, Flag, TagColor, format::Typed, inline::Marker},
};

/// A command in one mode with its default bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    /// The command.
    pub command: Command,
    /// The mode it is given in.
    pub mode: Mode,
    /// Its keys, each written as in the config file: `mod+mod+key` in lower case for a key with
    /// `ctrl`, `alt` or `shift`, the character itself for a key without them, or several keys
    /// separated by spaces for a sequence ([keybindings.md §5]). A command can have none.
    ///
    /// [keybindings.md §5]: ../../../../docs/specs/keybindings.md#5-configuration
    pub keys: &'static [&'static str],
}

/// An [`Entry`] of a command, a mode and keys.
macro_rules! entry {
    ($command:expr, $mode:expr, $keys:expr $(,)?) => {
        Entry {
            command: $command,
            mode: $mode,
            keys: $keys,
        }
    };
}

/// Every command in every mode it belongs to, in the order of [keybindings.md] within each
/// mode. Edit mode's standard text keys (caret motion, selection and deletion) are the text
/// field's, not commands.
///
/// [keybindings.md]: ../../../../docs/specs/keybindings.md
pub const REGISTRY: &[Entry] = &[
    entry!(Command::Quit, Mode::Navigate, &["ctrl+q"]),
    entry!(Command::FocusNext, Mode::Navigate, &["j", "down"]),
    entry!(Command::FocusPrevious, Mode::Navigate, &["k", "up"]),
    entry!(Command::Collapse, Mode::Navigate, &["h", "left"]),
    entry!(Command::Expand, Mode::Navigate, &["l", "right"]),
    entry!(
        Command::FocusNextSibling,
        Mode::Navigate,
        &["ctrl+down", "g j"]
    ),
    entry!(
        Command::FocusPreviousSibling,
        Mode::Navigate,
        &["ctrl+up", "g k"]
    ),
    entry!(Command::FocusParent, Mode::Navigate, &["g p"]),
    entry!(Command::FocusLastChild, Mode::Navigate, &["g l"]),
    entry!(Command::FocusFirst, Mode::Navigate, &["g g", "home"]),
    entry!(Command::FocusLast, Mode::Navigate, &["G", "end"]),
    entry!(Command::PageDown, Mode::Navigate, &["ctrl+d", "pagedown"]),
    entry!(Command::PageUp, Mode::Navigate, &["ctrl+u", "pageup"]),
    entry!(Command::ToggleFold, Mode::Navigate, &["'", "space"]),
    entry!(Command::ToggleSubtree, Mode::Navigate, &["\""]),
    entry!(Command::ExpandLevel, Mode::Navigate, &["="]),
    entry!(Command::CollapseLevel, Mode::Navigate, &["-"]),
    entry!(Command::ExpandAll, Mode::Navigate, &["+"]),
    entry!(Command::CollapseAll, Mode::Navigate, &["_"]),
    entry!(
        Command::ZoomIn,
        Mode::Navigate,
        &[".", "alt+l", "alt+right", "g z"],
    ),
    entry!(
        Command::ZoomOut,
        Mode::Navigate,
        &[">", "alt+h", "alt+left", "g u"],
    ),
    entry!(Command::GoHome, Mode::Navigate, &["ctrl+'", "g h"]),
    entry!(
        Command::GoToView(SpecialView::Today),
        Mode::Navigate,
        &["g t"]
    ),
    entry!(
        Command::GoToView(SpecialView::Upcoming),
        Mode::Navigate,
        &["g n"]
    ),
    entry!(
        Command::GoToView(SpecialView::Overdue),
        Mode::Navigate,
        &["g o"]
    ),
    entry!(
        Command::GoToView(SpecialView::Colors),
        Mode::Navigate,
        &["g C"]
    ),
    entry!(
        Command::GoToView(SpecialView::Starred),
        Mode::Navigate,
        &["g *"]
    ),
    entry!(
        Command::GoToView(SpecialView::Flagged),
        Mode::Navigate,
        &["g f"]
    ),
    entry!(
        Command::Sidebar(SidebarInput::GoToInbox),
        Mode::Navigate,
        &["g i"]
    ),
    entry!(Command::Back, Mode::Navigate, &["H", "alt+shift+left"]),
    entry!(Command::Forward, Mode::Navigate, &["L", "alt+shift+right"]),
    entry!(Command::Edit, Mode::Navigate, &["enter"]),
    entry!(Command::New(NewBlock::Below, None), Mode::Navigate, &["o"]),
    entry!(
        Command::New(NewBlock::Below, Some(Typed::Section)),
        Mode::Navigate,
        &["o s"],
    ),
    entry!(
        Command::New(NewBlock::Below, Some(Typed::Project)),
        Mode::Navigate,
        &["o p"],
    ),
    entry!(
        Command::New(NewBlock::Below, Some(Typed::Task)),
        Mode::Navigate,
        &["o t"],
    ),
    entry!(
        Command::New(NewBlock::Below, Some(Typed::Text)),
        Mode::Navigate,
        &["o n"],
    ),
    entry!(Command::New(NewBlock::Above, None), Mode::Navigate, &["O"]),
    entry!(
        Command::New(NewBlock::Above, Some(Typed::Section)),
        Mode::Navigate,
        &["O s"],
    ),
    entry!(
        Command::New(NewBlock::Above, Some(Typed::Project)),
        Mode::Navigate,
        &["O p"],
    ),
    entry!(
        Command::New(NewBlock::Above, Some(Typed::Task)),
        Mode::Navigate,
        &["O t"],
    ),
    entry!(
        Command::New(NewBlock::Above, Some(Typed::Text)),
        Mode::Navigate,
        &["O n"],
    ),
    entry!(
        Command::New(NewBlock::LastChild, None),
        Mode::Navigate,
        &["i"],
    ),
    entry!(
        Command::New(NewBlock::LastChild, Some(Typed::Section)),
        Mode::Navigate,
        &["i s"],
    ),
    entry!(
        Command::New(NewBlock::LastChild, Some(Typed::Project)),
        Mode::Navigate,
        &["i p"],
    ),
    entry!(
        Command::New(NewBlock::LastChild, Some(Typed::Task)),
        Mode::Navigate,
        &["i t"],
    ),
    entry!(
        Command::New(NewBlock::LastChild, Some(Typed::Text)),
        Mode::Navigate,
        &["i n"],
    ),
    entry!(
        Command::New(NewBlock::FirstChild, None),
        Mode::Navigate,
        &["I"],
    ),
    entry!(
        Command::New(NewBlock::FirstChild, Some(Typed::Section)),
        Mode::Navigate,
        &["I s"],
    ),
    entry!(
        Command::New(NewBlock::FirstChild, Some(Typed::Project)),
        Mode::Navigate,
        &["I p"],
    ),
    entry!(
        Command::New(NewBlock::FirstChild, Some(Typed::Task)),
        Mode::Navigate,
        &["I t"],
    ),
    entry!(
        Command::New(NewBlock::FirstChild, Some(Typed::Text)),
        Mode::Navigate,
        &["I n"],
    ),
    entry!(Command::New(NewBlock::End, None), Mode::Navigate, &["a"]),
    entry!(
        Command::New(NewBlock::End, Some(Typed::Section)),
        Mode::Navigate,
        &["a s"],
    ),
    entry!(
        Command::New(NewBlock::End, Some(Typed::Project)),
        Mode::Navigate,
        &["a p"],
    ),
    entry!(
        Command::New(NewBlock::End, Some(Typed::Task)),
        Mode::Navigate,
        &["a t"],
    ),
    entry!(
        Command::New(NewBlock::End, Some(Typed::Text)),
        Mode::Navigate,
        &["a n"],
    ),
    entry!(Command::New(NewBlock::Start, None), Mode::Navigate, &["A"]),
    entry!(
        Command::New(NewBlock::Start, Some(Typed::Section)),
        Mode::Navigate,
        &["A s"],
    ),
    entry!(
        Command::New(NewBlock::Start, Some(Typed::Project)),
        Mode::Navigate,
        &["A p"],
    ),
    entry!(
        Command::New(NewBlock::Start, Some(Typed::Task)),
        Mode::Navigate,
        &["A t"],
    ),
    entry!(
        Command::New(NewBlock::Start, Some(Typed::Text)),
        Mode::Navigate,
        &["A n"],
    ),
    entry!(
        Command::Select(SelectionChange::ExtendDown),
        Mode::Navigate,
        &["J", "shift+down"],
    ),
    entry!(
        Command::Select(SelectionChange::ExtendUp),
        Mode::Navigate,
        &["K", "shift+up"],
    ),
    entry!(
        Command::Select(SelectionChange::Toggle),
        Mode::Navigate,
        &["v"],
    ),
    entry!(
        Command::Select(SelectionChange::All),
        Mode::Navigate,
        &["ctrl+a"],
    ),
    entry!(
        Command::Select(SelectionChange::Clear),
        Mode::Navigate,
        &["esc"],
    ),
    entry!(Command::Indent, Mode::Navigate, &["tab"]),
    entry!(Command::Outdent, Mode::Navigate, &["shift+tab"]),
    entry!(Command::MoveUp, Mode::Navigate, &["alt+k", "alt+up"]),
    entry!(Command::MoveDown, Mode::Navigate, &["alt+j", "alt+down"]),
    entry!(Command::Group, Mode::Navigate, &["g G"]),
    entry!(Command::MoveTo, Mode::Navigate, &["g m"]),
    entry!(Command::Delete, Mode::Navigate, &["d d", "delete"]),
    entry!(Command::Duplicate, Mode::Navigate, &["D"]),
    entry!(Command::Copy, Mode::Navigate, &["y", "ctrl+c"]),
    entry!(Command::Cut, Mode::Navigate, &["ctrl+x"]),
    entry!(
        Command::Paste(Paste::Below),
        Mode::Navigate,
        &["p", "ctrl+v"],
    ),
    entry!(
        Command::Paste(Paste::Above),
        Mode::Navigate,
        &["P", "alt+v"],
    ),
    entry!(Command::Undo, Mode::Navigate, &["z", "ctrl+z"]),
    entry!(
        Command::Redo,
        Mode::Navigate,
        &["Z", "ctrl+shift+z", "ctrl+y"],
    ),
    entry!(Command::ToggleDone, Mode::Navigate, &["x", "ctrl+enter"]),
    entry!(
        Command::ChangeDate(DateTag::Start, DateChange::Today),
        Mode::Navigate,
        &["t"],
    ),
    entry!(
        Command::ChangeDate(DateTag::Start, DateChange::Tomorrow),
        Mode::Navigate,
        &["w"],
    ),
    entry!(
        Command::ChangeDate(DateTag::Due, DateChange::Today),
        Mode::Navigate,
        &["T"],
    ),
    entry!(
        Command::ChangeDate(DateTag::Due, DateChange::Tomorrow),
        Mode::Navigate,
        &["W"],
    ),
    entry!(
        Command::ChangeDate(DateTag::Start, DateChange::Earlier),
        Mode::Navigate,
        &["["],
    ),
    entry!(
        Command::ChangeDate(DateTag::Start, DateChange::Later),
        Mode::Navigate,
        &["]"],
    ),
    entry!(
        Command::ChangeDate(DateTag::Due, DateChange::Earlier),
        Mode::Navigate,
        &["{"],
    ),
    entry!(
        Command::ChangeDate(DateTag::Due, DateChange::Later),
        Mode::Navigate,
        &["}"],
    ),
    entry!(
        Command::OpenDatePrompt(DateTag::Start),
        Mode::Navigate,
        &["m s"],
    ),
    entry!(
        Command::OpenDatePrompt(DateTag::Due),
        Mode::Navigate,
        &["m d"],
    ),
    entry!(Command::ClearDate(DateTag::Start), Mode::Navigate, &[]),
    entry!(Command::ClearDate(DateTag::Due), Mode::Navigate, &[]),
    entry!(
        Command::SetColor(Some(TagColor::Red)),
        Mode::Navigate,
        &["c 1"],
    ),
    entry!(
        Command::SetColor(Some(TagColor::Orange)),
        Mode::Navigate,
        &["c 2"],
    ),
    entry!(
        Command::SetColor(Some(TagColor::Yellow)),
        Mode::Navigate,
        &["c 3"],
    ),
    entry!(
        Command::SetColor(Some(TagColor::Green)),
        Mode::Navigate,
        &["c 4"],
    ),
    entry!(
        Command::SetColor(Some(TagColor::Blue)),
        Mode::Navigate,
        &["c 5"],
    ),
    entry!(
        Command::SetColor(Some(TagColor::Violet)),
        Mode::Navigate,
        &["c 6"],
    ),
    entry!(Command::SetColor(None), Mode::Navigate, &["C"]),
    entry!(Command::CycleColor, Mode::Navigate, &["c c"]),
    entry!(Command::OpenColorPicker, Mode::Navigate, &["m c"]),
    entry!(Command::ToggleFlag(Flag::Star), Mode::Navigate, &["*"]),
    entry!(
        Command::ToggleFlag(Flag::Bookmark),
        Mode::Navigate,
        &["m b"],
    ),
    entry!(
        Command::ToggleFlag(Flag::Flagged),
        Mode::Navigate,
        &["m f", "!"],
    ),
    entry!(Command::OpenTypePicker, Mode::Navigate, &["m y"]),
    entry!(Command::SetSection(true), Mode::Navigate, &["S"]),
    entry!(Command::SetSection(false), Mode::Navigate, &["s"]),
    entry!(Command::OpenDetails, Mode::Navigate, &["m i"]),
    entry!(Command::OpenUrl, Mode::Navigate, &["g x"]),
    entry!(Command::ToggleShowDone, Mode::Navigate, &["ctrl+o", "g c"]),
    entry!(Command::ToggleWrap, Mode::Navigate, &["alt+z", "g w"]),
    entry!(Command::ChooseTheme, Mode::Navigate, &["ctrl+t"]),
    entry!(Command::OpenSettings, Mode::Navigate, &["g ,"]),
    entry!(Command::GoToSymbol, Mode::Navigate, &["ctrl+k", "g s", "/"]),
    entry!(Command::GoToProject, Mode::Navigate, &["g P"]),
    entry!(Command::ToggleShortcuts, Mode::Navigate, &["?"]),
    entry!(Command::ScrollShortcutsDown, Mode::Navigate, &[")"]),
    entry!(Command::ScrollShortcutsUp, Mode::Navigate, &["("]),
    entry!(Command::ToggleSidebar, Mode::Navigate, &["ctrl+g"]),
    entry!(Command::GoToCommand, Mode::Navigate, &["ctrl+p", ":"]),
    entry!(Command::Count, Mode::Navigate, &[]),
    entry!(Command::SaveAs, Mode::Navigate, &[]),
    entry!(
        Command::ContextMenu,
        Mode::Navigate,
        &["\\", "shift+f10", "menu"],
    ),
    entry!(
        Command::Pane(PaneChange::Open),
        Mode::Navigate,
        &["ctrl+\\", "ctrl+w v"],
    ),
    entry!(
        Command::Pane(PaneChange::OpenLocation),
        Mode::Navigate,
        &["ctrl+w s"],
    ),
    entry!(
        Command::Pane(PaneChange::Focus(0)),
        Mode::Navigate,
        &["alt+1", "g 1"],
    ),
    entry!(
        Command::Pane(PaneChange::Focus(1)),
        Mode::Navigate,
        &["alt+2", "g 2"],
    ),
    entry!(
        Command::Pane(PaneChange::Focus(2)),
        Mode::Navigate,
        &["alt+3", "g 3"],
    ),
    entry!(
        Command::Pane(PaneChange::Focus(3)),
        Mode::Navigate,
        &["alt+4", "g 4"],
    ),
    entry!(
        Command::Pane(PaneChange::Focus(4)),
        Mode::Navigate,
        &["alt+5", "g 5"],
    ),
    entry!(
        Command::Pane(PaneChange::Focus(5)),
        Mode::Navigate,
        &["alt+6", "g 6"],
    ),
    entry!(
        Command::Pane(PaneChange::Focus(6)),
        Mode::Navigate,
        &["alt+7", "g 7"],
    ),
    entry!(
        Command::Pane(PaneChange::Focus(7)),
        Mode::Navigate,
        &["alt+8", "g 8"],
    ),
    entry!(
        Command::Pane(PaneChange::Focus(8)),
        Mode::Navigate,
        &["alt+9", "g 9"],
    ),
    entry!(
        Command::Pane(PaneChange::FocusLeft),
        Mode::Navigate,
        &["ctrl+shift+left", "ctrl+w left", "ctrl+w h"],
    ),
    entry!(
        Command::Pane(PaneChange::FocusRight),
        Mode::Navigate,
        &["ctrl+shift+right", "ctrl+w right", "ctrl+w l"],
    ),
    entry!(
        Command::Pane(PaneChange::Close),
        Mode::Navigate,
        &["ctrl+w c"]
    ),
    entry!(
        Command::NarrowPane,
        Mode::Navigate,
        &["ctrl+w <", "alt+shift+h"],
    ),
    entry!(
        Command::WidenPane,
        Mode::Navigate,
        &["ctrl+w >", "alt+shift+l"],
    ),
    entry!(Command::Quit, Mode::Edit, &["ctrl+q"]),
    entry!(Command::StopEditing, Mode::Edit, &["esc"]),
    entry!(Command::NewNext, Mode::Edit, &["enter"]),
    entry!(Command::EditPrevious, Mode::Edit, &["up"]),
    entry!(Command::EditNext, Mode::Edit, &["down"]),
    entry!(Command::OpenCommandMenu, Mode::Edit, &["/"]),
    entry!(Command::CycleType, Mode::Edit, &["ctrl+t"]),
    entry!(Command::Retype(Typed::Project), Mode::Edit, &[]),
    entry!(Command::Retype(Typed::Task), Mode::Edit, &[]),
    entry!(Command::Retype(Typed::Text), Mode::Edit, &[]),
    entry!(Command::Retype(Typed::Section), Mode::Edit, &[]),
    entry!(Command::Indent, Mode::Edit, &["tab"]),
    entry!(Command::Outdent, Mode::Edit, &["shift+tab"]),
    entry!(Command::ToggleMarker(Marker::Bold), Mode::Edit, &["ctrl+b"]),
    entry!(
        Command::ToggleMarker(Marker::Code),
        Mode::Edit,
        &["alt+`", "ctrl+e"],
    ),
    entry!(Command::Undo, Mode::Edit, &["ctrl+z"]),
    entry!(Command::Redo, Mode::Edit, &["ctrl+shift+z", "ctrl+y"]),
    entry!(Command::Copy, Mode::Edit, &["ctrl+c"]),
    entry!(Command::Cut, Mode::Edit, &["ctrl+x"]),
    entry!(Command::Paste(Paste::Below), Mode::Edit, &["ctrl+v"]),
];

/// The keys of the open sidebar, each command's written as in [`Entry::keys`]; other keys do
/// nothing there ([keybindings.md §4]).
///
/// [keybindings.md §4]: ../../../../docs/specs/keybindings.md#4-other-contexts
pub const SIDEBAR: &[(Command, &[&str])] = &[
    (Command::Sidebar(SidebarInput::Next), &["down", "ctrl+n"]),
    (Command::Sidebar(SidebarInput::Previous), &["up", "ctrl+p"]),
    (Command::Sidebar(SidebarInput::Collapse), &["left"]),
    (Command::Sidebar(SidebarInput::Expand), &["right"]),
    (Command::Sidebar(SidebarInput::Choose), &["enter"]),
    (
        Command::Pane(PaneChange::Open),
        &["alt+enter", "ctrl+v", "ctrl+t"],
    ),
    (Command::Sidebar(SidebarInput::Erase), &["backspace"]),
    (Command::Sidebar(SidebarInput::Leave), &["esc"]),
    (Command::ContextMenu, &["shift+f10", "menu"]),
    (Command::ToggleSidebar, &["ctrl+g"]),
    (Command::Quit, &["ctrl+q"]),
];

/// A group of bindings as [keybindings.md] groups them, which the shortcuts panel lists the
/// commands under.
///
/// [keybindings.md]: ../../../../docs/specs/keybindings.md
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    /// Moving the focus.
    Moving,
    /// Folding blocks.
    Folding,
    /// Zooming and the zoom history.
    Zoom,
    /// Editing and creating blocks in navigate mode.
    Creating,
    /// Selecting blocks.
    Selection,
    /// Changing the outline's structure, the clipboard and undo.
    Structure,
    /// A block's dates, tags, color and type.
    Metadata,
    /// The view, panels and quitting.
    View,
    /// Edit mode's own commands.
    Editing,
}

impl Group {
    /// The group's name, which heads its commands in the shortcuts panel.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Moving => "Moving",
            Self::Folding => "Folding",
            Self::Zoom => "Zoom and History",
            Self::Creating => "Editing and Creating",
            Self::Selection => "Selection",
            Self::Structure => "Structure",
            Self::Metadata => "Block Metadata",
            Self::View => "View and Panels",
            Self::Editing => "Editing",
        }
    }
}

/// The most useful commands of each mode, which the shortcuts panel lists, grouped and in the
/// order of [keybindings.md] (FR-KEY-04, [ui.md §4]). Every one is bound in its mode.
///
/// [keybindings.md]: ../../../../docs/specs/keybindings.md
/// [ui.md §4]: ../../../../docs/specs/ui.md#4-shortcuts-panel-fr-key-04
const SHORTCUTS: &[(Mode, Group, &[Command])] = &[
    (
        Mode::Navigate,
        Group::Moving,
        &[
            Command::FocusNext,
            Command::FocusPrevious,
            Command::Collapse,
            Command::Expand,
            Command::FocusFirst,
            Command::FocusLast,
        ],
    ),
    (
        Mode::Navigate,
        Group::Folding,
        &[
            Command::ToggleFold,
            Command::ExpandAll,
            Command::CollapseAll,
        ],
    ),
    (
        Mode::Navigate,
        Group::Zoom,
        &[
            Command::ZoomIn,
            Command::ZoomOut,
            Command::Back,
            Command::Forward,
            Command::GoToView(SpecialView::Today),
            Command::GoToView(SpecialView::Upcoming),
            Command::GoToView(SpecialView::Overdue),
            Command::GoToView(SpecialView::Colors),
            Command::GoToView(SpecialView::Starred),
            Command::GoToView(SpecialView::Flagged),
            Command::Sidebar(SidebarInput::GoToInbox),
        ],
    ),
    (
        Mode::Navigate,
        Group::Creating,
        &[
            Command::Edit,
            Command::New(NewBlock::Below, None),
            Command::New(NewBlock::LastChild, None),
        ],
    ),
    (
        Mode::Navigate,
        Group::Selection,
        &[
            Command::Select(SelectionChange::ExtendDown),
            Command::Select(SelectionChange::Toggle),
        ],
    ),
    (
        Mode::Navigate,
        Group::Structure,
        &[
            Command::Indent,
            Command::Outdent,
            Command::MoveUp,
            Command::MoveDown,
            Command::Delete,
            Command::Copy,
            Command::Paste(Paste::Below),
            Command::Undo,
            Command::Redo,
        ],
    ),
    (
        Mode::Navigate,
        Group::Metadata,
        &[
            Command::ToggleDone,
            Command::ChangeDate(DateTag::Start, DateChange::Today),
            Command::ChangeDate(DateTag::Due, DateChange::Today),
            Command::OpenDatePrompt(DateTag::Due),
            Command::OpenColorPicker,
            Command::ToggleFlag(Flag::Bookmark),
            Command::OpenDetails,
        ],
    ),
    (
        Mode::Navigate,
        Group::View,
        &[
            Command::ToggleShowDone,
            Command::ToggleWrap,
            Command::GoToSymbol,
            Command::GoToProject,
            Command::GoToCommand,
            Command::OpenSettings,
            Command::ContextMenu,
            Command::Pane(PaneChange::Open),
            Command::Pane(PaneChange::Close),
            Command::Quit,
        ],
    ),
    (
        Mode::Edit,
        Group::Editing,
        &[
            Command::StopEditing,
            Command::NewNext,
            Command::EditPrevious,
            Command::EditNext,
            Command::OpenCommandMenu,
            Command::CycleType,
            Command::ToggleMarker(Marker::Bold),
            Command::ToggleMarker(Marker::Code),
        ],
    ),
    (
        Mode::Edit,
        Group::Structure,
        &[
            Command::Indent,
            Command::Outdent,
            Command::Undo,
            Command::Redo,
            Command::Copy,
            Command::Paste(Paste::Below),
        ],
    ),
    (Mode::Edit, Group::View, &[Command::Quit]),
];

/// What the shortcuts panel lists in `mode`: each group with the registry entries of its
/// commands, which hold their keys (FR-KEY-04).
#[must_use]
pub fn shortcuts(mode: Mode) -> Vec<(Group, Vec<&'static Entry>)> {
    SHORTCUTS
        .iter()
        .filter(|(listed, ..)| *listed == mode)
        .map(|&(_, group, commands)| {
            let entries = commands
                .iter()
                .filter_map(|&command| {
                    REGISTRY
                        .iter()
                        .find(|entry| entry.command == command && entry.mode == mode)
                })
                .collect();
            (group, entries)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn each_command_appears_once_in_each_of_its_modes() {
        let mut seen = HashSet::new();
        for entry in REGISTRY {
            assert!(
                seen.insert(format!("{:?} {:?}", entry.command, entry.mode)),
                "{entry:?}"
            );
        }
    }

    #[test]
    fn no_key_is_bound_twice_in_a_mode() {
        let mut seen = HashSet::new();
        for entry in REGISTRY {
            for keys in entry.keys {
                assert!(
                    seen.insert(format!("{keys} {:?}", entry.mode)),
                    "{keys} in {:?}",
                    entry.mode
                );
            }
        }
    }

    // FR-KEY-04
    #[test]
    fn the_shortcuts_panel_lists_bound_commands_of_its_mode_once() {
        for (mode, _, commands) in SHORTCUTS {
            for command in *commands {
                assert!(
                    REGISTRY.iter().any(|entry| entry.command == *command
                        && entry.mode == *mode
                        && !entry.keys.is_empty()),
                    "{command:?} is not bound in {mode:?}"
                );
            }
        }
        for mode in [Mode::Navigate, Mode::Edit] {
            let listed: Vec<_> = shortcuts(mode)
                .into_iter()
                .flat_map(|(_, entries)| entries)
                .collect();
            let distinct: HashSet<_> = listed.iter().map(|entry| entry.command.name()).collect();
            assert_eq!(distinct.len(), listed.len(), "{mode:?}");
            assert!(listed.iter().all(|entry| entry.mode == mode), "{mode:?}");
        }
    }

    // FR-KEY-04
    #[test]
    fn the_shortcuts_panel_groups_each_modes_commands_in_the_order_of_the_key_bindings() {
        let groups = |mode| -> Vec<_> {
            shortcuts(mode)
                .into_iter()
                .map(|(group, entries)| (group.name(), entries.len()))
                .collect()
        };
        assert_eq!(
            groups(Mode::Navigate),
            [
                ("Moving", 6),
                ("Folding", 3),
                ("Zoom and History", 11),
                ("Editing and Creating", 3),
                ("Selection", 2),
                ("Structure", 9),
                ("Block Metadata", 7),
                ("View and Panels", 10),
            ]
        );
        assert_eq!(
            groups(Mode::Edit),
            [("Editing", 8), ("Structure", 6), ("View and Panels", 1)]
        );
        let edit = shortcuts(Mode::Edit);
        let (_, editing) = &edit[0];
        assert_eq!(editing[0].keys, ["esc"], "edit mode's own keys");
        let (_, view) = &shortcuts(Mode::Navigate)[7];
        assert_eq!(view[9].keys, ["ctrl+q"]);
    }

    // FR-UI-01, FR-KEY-06, keybindings.md §4
    #[test]
    fn the_sidebar_binds_each_key_once_to_a_command_with_a_message() {
        let mut seen = HashSet::new();
        for (command, keys) in SIDEBAR {
            assert!(!keys.is_empty(), "{command:?}");
            for keys in *keys {
                assert!(seen.insert(*keys), "{keys}");
                assert!(!keys.contains(' '), "no sequence: {keys}");
            }
        }
        let context_menu = REGISTRY
            .iter()
            .find(|entry| entry.command == Command::ContextMenu)
            .map(|entry| entry.keys);
        assert_eq!(
            SIDEBAR
                .iter()
                .find(|(command, _)| *command == Command::ContextMenu)
                .map(|&(_, keys)| keys),
            context_menu.map(|keys| &keys[1..]),
            "the context menu opens with the same keys everywhere, but for the \\ the Navigator's \
             filter takes (D-86)"
        );
    }

    #[test]
    fn edit_mode_binds_single_keys() {
        for entry in REGISTRY.iter().filter(|entry| entry.mode == Mode::Edit) {
            assert!(
                entry.keys.iter().all(|keys| !keys.contains(' ')),
                "{entry:?}"
            );
        }
    }
}
