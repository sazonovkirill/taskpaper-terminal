//! The commands that apply to one block, with the labels a menu shows for it, as the edit-mode
//! command menu lists them (D-50,
//! [editing.md §2.3](../../../../docs/specs/editing.md#23-command-menu)).

use std::borrow::Cow;

use super::Command;
use crate::document::{Block, BlockKind, DateChange, DateTag, Flag, TagColor, format::Typed};

/// A command on one block with the label a menu shows for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::app) struct NodeCommand {
    /// What a menu shows, which its filter matches: the command's name, or what it does to this
    /// block where that depends on the block's state.
    pub(in crate::app) label: Cow<'static, str>,
    /// The command.
    pub(in crate::app) command: Command,
}

/// The type entries, in the order the menu lists them (D-52).
const TYPES: [Typed; 3] = [Typed::Project, Typed::Task, Typed::Text];

/// The colors in the order the menu lists them.
const COLORS: [TagColor; 6] = [
    TagColor::Red,
    TagColor::Orange,
    TagColor::Yellow,
    TagColor::Green,
    TagColor::Blue,
    TagColor::Violet,
];

/// Every command that applies to `block` in menu order, in groups a menu sets apart: types, with
/// Make Section or Remove Section for a text block (D-52), status, dates, colors and the block
/// itself (D-50). `link` says whether the block has a link to open. Labels follow the block's
/// state, and a command that would change nothing, such as clearing a date it does not have, is
/// left out, so no group is empty.
pub(in crate::app) fn node_commands(block: &Block, link: bool) -> Vec<Vec<NodeCommand>> {
    let named = |command: Command| NodeCommand {
        label: command.name(),
        command,
    };
    let labelled = |label: &'static str, command| NodeCommand {
        label: label.into(),
        command,
    };
    let section = match block.kind() {
        BlockKind::Text if block.section() => {
            Some(labelled("Remove Section", Command::Retype(Typed::Text)))
        }
        BlockKind::Text => Some(named(Command::Retype(Typed::Section))),
        BlockKind::Project | BlockKind::Task => None,
    };
    let types = TYPES
        .into_iter()
        .map(|typed| named(Command::Retype(typed)))
        .chain(section)
        .collect();
    let done = if block.done() {
        "Uncomplete"
    } else {
        "Complete"
    };
    let bookmark = if block.bookmark() {
        "Remove Bookmark"
    } else {
        "Bookmark"
    };
    let status = vec![
        labelled(done, Command::ToggleDone),
        named(Command::ToggleFlag(Flag::Today)),
        labelled(bookmark, Command::ToggleFlag(Flag::Bookmark)),
    ];
    let dates = [
        Command::OpenDatePrompt(DateTag::Start),
        Command::OpenDatePrompt(DateTag::Due),
        Command::ChangeDate(DateTag::Start, DateChange::Today),
        Command::ChangeDate(DateTag::Start, DateChange::Tomorrow),
        Command::ChangeDate(DateTag::Due, DateChange::Today),
        Command::ChangeDate(DateTag::Due, DateChange::Tomorrow),
    ]
    .into_iter()
    .chain(block.start().map(|_| Command::ClearDate(DateTag::Start)))
    .chain(block.due().map(|_| Command::ClearDate(DateTag::Due)))
    .map(named)
    .collect();
    let colors = COLORS
        .into_iter()
        .map(|color| Command::SetColor(Some(color)))
        .chain(block.color().map(|_| Command::SetColor(None)))
        .map(named)
        .collect();
    let itself = [Command::OpenDetails]
        .into_iter()
        .chain(link.then_some(Command::OpenUrl))
        .chain([Command::Duplicate, Command::Delete])
        .map(named)
        .collect();
    vec![types, status, dates, colors, itself]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{blocks, format};

    /// The labels of the commands for the only block of `line`, which has a link when `linked`,
    /// group by group.
    fn groups(line: &str, linked: bool) -> Vec<Vec<String>> {
        commands(line, linked)
            .into_iter()
            .map(|group| {
                group
                    .into_iter()
                    .map(|command| command.label.into_owned())
                    .collect()
            })
            .collect()
    }

    /// The commands for the only block of `line`, which has a link when `linked`.
    fn commands(line: &str, linked: bool) -> Vec<Vec<NodeCommand>> {
        let document = format::parse(line);
        let (id, _) = document.descendants(document.root()).next().unwrap();
        let block = blocks::block(&document, id).unwrap();
        node_commands(&block, linked)
    }

    // D-50
    #[test]
    fn a_plain_block_offers_every_command_that_changes_it_in_groups() {
        assert_eq!(
            groups("- a", false),
            [
                vec!["Project", "Task", "Note"],
                vec!["Complete", "Toggle Today", "Bookmark"],
                vec![
                    "Set Start Date",
                    "Set Due Date",
                    "Start Today",
                    "Start Tomorrow",
                    "Due Today",
                    "Due Tomorrow",
                ],
                vec![
                    "Color: Red",
                    "Color: Orange",
                    "Color: Yellow",
                    "Color: Green",
                    "Color: Blue",
                    "Color: Violet",
                ],
                vec!["Show Details", "Duplicate", "Delete"],
            ]
        );
    }

    // D-52
    #[test]
    fn a_text_block_can_be_made_a_section_and_a_section_text_again() {
        let types = |line| groups(line, false).swap_remove(0);
        assert_eq!(types("a"), ["Project", "Task", "Note", "Make Section"]);
        assert_eq!(types("---"), ["Project", "Task", "Note", "Make Section"]);
        assert_eq!(
            types("a @section"),
            ["Project", "Task", "Note", "Remove Section"]
        );
        assert_eq!(types("P: @section"), ["Project", "Task", "Note"]);
        for (line, typed) in [("a", Typed::Section), ("a @section", Typed::Text)] {
            let command = commands(line, false).swap_remove(0).swap_remove(3).command;
            assert_eq!(command, Command::Retype(typed), "{line}");
        }
    }

    // D-50
    #[test]
    fn labels_and_entries_follow_the_blocks_state() {
        let groups = groups(
            "- a @done @start(2026-10-01) @due(2026-10-09) @bookmark @blue",
            true,
        );
        assert_eq!(groups[1], ["Uncomplete", "Toggle Today", "Remove Bookmark"]);
        assert_eq!(groups[2][6..], ["Clear Start Date", "Clear Due Date"]);
        assert_eq!(groups[3].last().map(String::as_str), Some("Remove Color"));
        assert_eq!(
            groups[4],
            ["Show Details", "Open Link", "Duplicate", "Delete"]
        );
    }

    // D-50
    #[test]
    fn each_entry_is_its_command() {
        let commands = commands("- a @start(2026-10-01) @due(2026-10-09) @red", true);
        let command = |label: &str| {
            commands
                .iter()
                .flatten()
                .find(|command| command.label == label)
                .unwrap()
                .command
        };
        let cases = [
            ("Project", Command::Retype(Typed::Project)),
            ("Task", Command::Retype(Typed::Task)),
            ("Note", Command::Retype(Typed::Text)),
            ("Complete", Command::ToggleDone),
            ("Toggle Today", Command::ToggleFlag(Flag::Today)),
            ("Bookmark", Command::ToggleFlag(Flag::Bookmark)),
            ("Set Start Date", Command::OpenDatePrompt(DateTag::Start)),
            ("Set Due Date", Command::OpenDatePrompt(DateTag::Due)),
            (
                "Start Today",
                Command::ChangeDate(DateTag::Start, DateChange::Today),
            ),
            (
                "Start Tomorrow",
                Command::ChangeDate(DateTag::Start, DateChange::Tomorrow),
            ),
            (
                "Due Today",
                Command::ChangeDate(DateTag::Due, DateChange::Today),
            ),
            (
                "Due Tomorrow",
                Command::ChangeDate(DateTag::Due, DateChange::Tomorrow),
            ),
            ("Clear Start Date", Command::ClearDate(DateTag::Start)),
            ("Clear Due Date", Command::ClearDate(DateTag::Due)),
            ("Color: Red", Command::SetColor(Some(TagColor::Red))),
            ("Color: Orange", Command::SetColor(Some(TagColor::Orange))),
            ("Color: Yellow", Command::SetColor(Some(TagColor::Yellow))),
            ("Color: Green", Command::SetColor(Some(TagColor::Green))),
            ("Color: Blue", Command::SetColor(Some(TagColor::Blue))),
            ("Color: Violet", Command::SetColor(Some(TagColor::Violet))),
            ("Remove Color", Command::SetColor(None)),
            ("Show Details", Command::OpenDetails),
            ("Open Link", Command::OpenUrl),
            ("Duplicate", Command::Duplicate),
            ("Delete", Command::Delete),
        ];
        for (label, expected) in cases {
            assert_eq!(command(label), expected, "{label}");
        }
    }
}
