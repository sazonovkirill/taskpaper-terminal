//! The commands that apply to blocks, with the labels a menu shows for them: those of one block,
//! as the edit-mode command menu lists them (D-50,
//! [editing.md §2.3](../../../../docs/specs/editing.md#23-command-menu)), and those of the focused
//! block or the selection, as the context menu lists them
//! ([ui.md §5](../../../../docs/specs/ui.md#5-context-menu-fr-key-06), FR-KEY-06).

use std::{borrow::Cow, slice};

use super::Command;
use crate::{
    app::{NewBlock, Paste},
    document::{Block, BlockKind, DateChange, DateTag, Flag, TagColor, format::Typed},
};

/// A command on one block with the label a menu shows for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::app) struct NodeCommand {
    /// What a menu shows, which its filter matches: the command's name, or what it does to this
    /// block where that depends on the block's state.
    pub(in crate::app) label: Cow<'static, str>,
    /// The command.
    pub(in crate::app) command: Command,
}

impl NodeCommand {
    /// The command under its name.
    pub(in crate::app) fn named(command: Command) -> Self {
        Self {
            label: command.name(),
            command,
        }
    }

    /// The command under `label`.
    pub(in crate::app) fn labelled(label: &'static str, command: Command) -> Self {
        Self {
            label: label.into(),
            command,
        }
    }
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
    let section = match block.kind() {
        BlockKind::Text if block.section() => Some(NodeCommand::labelled(
            "Remove Section",
            Command::Retype(Typed::Text),
        )),
        BlockKind::Text => Some(NodeCommand::named(Command::Retype(Typed::Section))),
        BlockKind::Project | BlockKind::Task => None,
    };
    let types = TYPES
        .into_iter()
        .map(|typed| NodeCommand::named(Command::Retype(typed)))
        .chain(section)
        .collect();
    let blocks = slice::from_ref(block);
    let itself = [Command::OpenDetails]
        .into_iter()
        .chain(link.then_some(Command::OpenUrl))
        .chain([Command::Duplicate, Command::Delete])
        .map(NodeCommand::named)
        .collect();
    vec![types, status(blocks), dates(blocks), colors(blocks), itself]
}

/// What a context menu needs to know of the focused block beyond its tags, when no other block
/// is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) struct Focused {
    /// Whether it has a link to open.
    pub(in crate::app) link: bool,
    /// Whether it has children to fold.
    pub(in crate::app) foldable: bool,
}

/// Every command a context menu offers on `blocks`, the selected blocks or the focused block
/// alone, in groups as [ui.md §5] orders them: editing, structure, status, dates, colors, types
/// and the view. `focused` says what the focused block has when it is alone, and the commands
/// that act on the focused block only, such as editing it or zooming in, are offered only then.
/// Labels follow the blocks' state as the commands do: Uncomplete when every block is done,
/// Remove Bookmark when every block has a bookmark. A command that would change nothing is left
/// out, and so is a group left empty.
///
/// [ui.md §5]: ../../../../docs/specs/ui.md#5-context-menu-fr-key-06
pub(in crate::app) fn context_commands(
    blocks: &[Block],
    focused: Option<Focused>,
) -> Vec<Vec<NodeCommand>> {
    let alone = |commands: &[Command]| {
        if focused.is_some() {
            commands.to_vec()
        } else {
            Vec::new()
        }
    };
    let editing = alone(&[
        Command::Edit,
        Command::New(NewBlock::Below, None),
        Command::New(NewBlock::Above, None),
        Command::New(NewBlock::LastChild, None),
    ])
    .into_iter()
    .chain([
        Command::Copy,
        Command::Cut,
        Command::Paste(Paste::Below),
        Command::Paste(Paste::Above),
        Command::Duplicate,
        Command::Delete,
    ])
    .map(NodeCommand::named)
    .collect();
    let structure = [
        Command::Indent,
        Command::Outdent,
        Command::MoveUp,
        Command::MoveDown,
        Command::Group,
        Command::MoveTo,
    ]
    .into_iter()
    .map(NodeCommand::named)
    .collect();
    let text = |section: bool| {
        blocks
            .iter()
            .any(|block| block.kind() == BlockKind::Text && block.section() == section)
    };
    let types = alone(&[Command::OpenTypePicker])
        .into_iter()
        .chain(text(false).then_some(Command::SetSection(true)))
        .chain(text(true).then_some(Command::SetSection(false)))
        .map(NodeCommand::named)
        .collect();
    let view = alone(&[Command::ZoomIn])
        .into_iter()
        .chain(
            focused
                .is_some_and(|focused| focused.foldable)
                .then_some(Command::ToggleFold),
        )
        .chain(alone(&[Command::Count, Command::OpenDetails]))
        .chain(
            focused
                .is_some_and(|focused| focused.link)
                .then_some(Command::OpenUrl),
        )
        .map(NodeCommand::named)
        .collect();
    let mut groups = vec![
        editing,
        structure,
        status(blocks),
        dates(blocks),
        colors(blocks),
        types,
        view,
    ];
    groups.retain(|group| !group.is_empty());
    groups
}

/// Completing, Today and the bookmark, labelled for what they do to `blocks`: uncompleting when
/// all are done, removing the bookmark when all have one.
fn status(blocks: &[Block]) -> Vec<NodeCommand> {
    let done = if blocks.iter().all(Block::done) {
        "Uncomplete"
    } else {
        "Complete"
    };
    let bookmark = if blocks.iter().all(Block::bookmark) {
        "Remove Bookmark"
    } else {
        "Bookmark"
    };
    vec![
        NodeCommand::labelled(done, Command::ToggleDone),
        NodeCommand::named(Command::ToggleFlag(Flag::Star)),
        NodeCommand::labelled(bookmark, Command::ToggleFlag(Flag::Bookmark)),
    ]
}

/// Setting the dates, and clearing those that any of `blocks` has.
fn dates(blocks: &[Block]) -> Vec<NodeCommand> {
    let has = |date: fn(&Block) -> bool| blocks.iter().any(date);
    [
        Command::OpenDatePrompt(DateTag::Start),
        Command::OpenDatePrompt(DateTag::Due),
        Command::ChangeDate(DateTag::Start, DateChange::Today),
        Command::ChangeDate(DateTag::Start, DateChange::Tomorrow),
        Command::ChangeDate(DateTag::Due, DateChange::Today),
        Command::ChangeDate(DateTag::Due, DateChange::Tomorrow),
    ]
    .into_iter()
    .chain(has(|block| block.start().is_some()).then_some(Command::ClearDate(DateTag::Start)))
    .chain(has(|block| block.due().is_some()).then_some(Command::ClearDate(DateTag::Due)))
    .map(NodeCommand::named)
    .collect()
}

/// Each color, and removing it when any of `blocks` has one.
fn colors(blocks: &[Block]) -> Vec<NodeCommand> {
    COLORS
        .into_iter()
        .map(|color| Command::SetColor(Some(color)))
        .chain(
            blocks
                .iter()
                .any(|block| block.color().is_some())
                .then_some(Command::SetColor(None)),
        )
        .map(NodeCommand::named)
        .collect()
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
                vec!["Complete", "Toggle Star", "Bookmark"],
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
        assert_eq!(groups[1], ["Uncomplete", "Toggle Star", "Remove Bookmark"]);
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
            ("Toggle Star", Command::ToggleFlag(Flag::Star)),
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

    /// The labels of the context menu's commands on the top-level blocks of `text`, each group's
    /// joined by `, `, the focused one alone having what `focused` says.
    fn context(text: &str, focused: Option<Focused>) -> Vec<String> {
        let document = format::parse(text);
        let blocks: Vec<Block> = blocks::children(&document, document.root());
        context_commands(&blocks, focused)
            .into_iter()
            .map(|group| {
                group
                    .iter()
                    .map(|command| command.label.as_ref())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .collect()
    }

    const ALONE: Focused = Focused {
        link: false,
        foldable: false,
    };

    // FR-KEY-06, ui.md §5
    #[test]
    fn the_context_menu_offers_a_blocks_commands_grouped_from_editing_to_the_view() {
        assert_eq!(
            context("- a\n", Some(ALONE)),
            [
                "Edit, New Block Below, New Block Above, New Block as Last Child, Copy, Cut, \
                 Paste Below, Paste Above, Duplicate, Delete",
                "Indent, Outdent, Move Up, Move Down, Group, Move to…",
                "Complete, Toggle Star, Bookmark",
                "Set Start Date, Set Due Date, Start Today, Start Tomorrow, Due Today, \
                 Due Tomorrow",
                "Color: Red, Color: Orange, Color: Yellow, Color: Green, Color: Blue, \
                 Color: Violet",
                "Convert Type",
                "Zoom In, Count Blocks and Words, Show Details",
            ]
        );
    }

    // FR-KEY-06, ui.md §5
    #[test]
    fn the_context_menu_follows_the_focused_blocks_state() {
        let linked = Focused {
            link: true,
            foldable: true,
        };
        let groups = context(
            "a @done @start(2026-10-01) @due(2026-10-09) @bookmark @blue\n",
            Some(linked),
        );
        assert_eq!(groups[2], "Uncomplete, Toggle Star, Remove Bookmark");
        assert!(groups[3].ends_with("Due Tomorrow, Clear Start Date, Clear Due Date"));
        assert!(groups[4].ends_with("Color: Violet, Remove Color"));
        assert_eq!(groups[5], "Convert Type, Make Section");
        assert_eq!(
            groups[6],
            "Zoom In, Toggle Fold, Count Blocks and Words, Show Details, Open Link"
        );
        assert_eq!(
            context("a @section\n", Some(ALONE))[5],
            "Convert Type, Remove Section"
        );
    }

    // FR-KEY-06, FR-OPS-07
    #[test]
    fn on_a_selection_the_context_menu_offers_what_acts_on_every_selected_block() {
        let groups = context("- a @done @due(2026-10-09)\nb @section\nc\n", None);
        assert_eq!(
            groups,
            [
                "Copy, Cut, Paste Below, Paste Above, Duplicate, Delete",
                "Indent, Outdent, Move Up, Move Down, Group, Move to…",
                "Complete, Toggle Star, Bookmark",
                "Set Start Date, Set Due Date, Start Today, Start Tomorrow, Due Today, \
                 Due Tomorrow, Clear Due Date",
                "Color: Red, Color: Orange, Color: Yellow, Color: Green, Color: Blue, \
                 Color: Violet",
                "Make Section, Remove Section",
            ],
            "no group of the focused block's own commands is left empty"
        );
        let done = context("- a @done @bookmark\n- b @done @bookmark\n", None);
        assert_eq!(done[2], "Uncomplete, Toggle Star, Remove Bookmark");
        assert_eq!(done.len(), 5, "tasks have no section to make");
    }
}
