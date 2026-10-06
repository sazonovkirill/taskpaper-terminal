//! The registry's table: every command with the mode it belongs to and its default bindings
//! ([keybindings.md §2–3](../../../../docs/specs/keybindings.md#2-navigate-mode)).

use super::Command;
use crate::{
    app::{Mode, NewBlock, Paste, SelectionChange},
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
    entry!(Command::FocusNextSibling, Mode::Navigate, &["ctrl+down"]),
    entry!(Command::FocusPreviousSibling, Mode::Navigate, &["ctrl+up"]),
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
    entry!(Command::ToggleFlag(Flag::Today), Mode::Navigate, &["m t"]),
    entry!(
        Command::ToggleFlag(Flag::Bookmark),
        Mode::Navigate,
        &["m b"],
    ),
    entry!(Command::OpenTypePicker, Mode::Navigate, &["m y"]),
    entry!(Command::SetSection(true), Mode::Navigate, &["S"]),
    entry!(Command::SetSection(false), Mode::Navigate, &["s"]),
    entry!(Command::OpenDetails, Mode::Navigate, &["m i"]),
    entry!(Command::OpenUrl, Mode::Navigate, &["g x"]),
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
