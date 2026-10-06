//! The commands that apply to one block, each with the label a menu shows and what running it
//! does, as the edit-mode command menu lists them (D-50,
//! [editing.md §2.3](../../../docs/specs/editing.md#23-command-menu)). The command registry of
//! step 4.1 is meant to absorb this list.

use jiff::civil::Date;

use super::Message;
use crate::document::{Block, BlockKind, DateChange, DateTag, Flag, TagColor, format::Typed};

/// A command on one block: its label and what running it does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NodeCommand {
    /// What a menu shows, which its filter matches.
    pub(super) label: &'static str,
    /// What running it does.
    pub(super) action: Action,
}

/// What running a command of the menu does, which also tells a frontend which command it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Give the block being edited this type, as a change of the edit session (D-44).
    Retype(Typed),
    /// Send this message, which acts on the focused block.
    Run(Message),
}

/// The type entries, in the order the menu lists them (D-52).
const TYPES: [(Typed, &str); 3] = [
    (Typed::Project, "Project"),
    (Typed::Task, "Task"),
    (Typed::Text, "Note"),
];

/// Each color with its entry's label.
const COLORS: [(TagColor, &str); 6] = [
    (TagColor::Red, "Color: Red"),
    (TagColor::Orange, "Color: Orange"),
    (TagColor::Yellow, "Color: Yellow"),
    (TagColor::Green, "Color: Green"),
    (TagColor::Blue, "Color: Blue"),
    (TagColor::Violet, "Color: Violet"),
];

/// Every command that applies to `block` in menu order, in groups a menu sets apart: types, with
/// Make Section or Remove Section for a text block (D-52), status, dates, colors and the block
/// itself (D-50). `link` says whether the block has a link to
/// open and `today` is the local date the date commands write. Labels follow the block's state,
/// and a command that would change nothing, such as clearing a date it does not have, is left
/// out, so no group is empty.
pub(super) fn node_commands(block: &Block, link: bool, today: Date) -> Vec<Vec<NodeCommand>> {
    let run = |label, message| NodeCommand {
        label,
        action: Action::Run(message),
    };
    let date = |label, tag, change| run(label, Message::ChangeDate { tag, change, today });
    let retype = |label, typed| NodeCommand {
        label,
        action: Action::Retype(typed),
    };
    let section = match block.kind() {
        BlockKind::Text if block.section() => Some(retype("Remove Section", Typed::Text)),
        BlockKind::Text => Some(retype("Make Section", Typed::Section)),
        BlockKind::Project | BlockKind::Task => None,
    };
    let types = TYPES
        .into_iter()
        .map(|(typed, label)| retype(label, typed))
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
        run(done, Message::ToggleDone { today }),
        run("Toggle Today", Message::ToggleFlag(Flag::Today)),
        run(bookmark, Message::ToggleFlag(Flag::Bookmark)),
    ];
    let dates = [
        run(
            "Set Start Date",
            Message::OpenDatePrompt {
                tag: DateTag::Start,
                today,
            },
        ),
        run(
            "Set Due Date",
            Message::OpenDatePrompt {
                tag: DateTag::Due,
                today,
            },
        ),
        date("Start Today", DateTag::Start, DateChange::Today),
        date("Start Tomorrow", DateTag::Start, DateChange::Tomorrow),
        date("Due Today", DateTag::Due, DateChange::Today),
        date("Due Tomorrow", DateTag::Due, DateChange::Tomorrow),
    ]
    .into_iter()
    .chain(
        block
            .start()
            .map(|_| run("Clear Start Date", Message::ClearDate(DateTag::Start))),
    )
    .chain(
        block
            .due()
            .map(|_| run("Clear Due Date", Message::ClearDate(DateTag::Due))),
    )
    .collect();
    let colors = COLORS
        .into_iter()
        .map(|(color, label)| run(label, Message::SetColor(Some(color))))
        .chain(
            block
                .color()
                .map(|_| run("Remove Color", Message::SetColor(None))),
        )
        .collect();
    let itself = [run("Show Details", Message::OpenDetails)]
        .into_iter()
        .chain(link.then(|| run("Open Link", Message::OpenUrl)))
        .chain([
            run("Duplicate", Message::Duplicate),
            run("Delete", Message::Delete),
        ])
        .collect();
    vec![types, status, dates, colors, itself]
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;
    use crate::document::{blocks, format};

    const TODAY: Date = date(2026, 10, 6);

    /// The labels of the commands for the only block of `line`, which has a link when `linked`,
    /// group by group.
    fn groups(line: &str, linked: bool) -> Vec<Vec<&'static str>> {
        let document = format::parse(line);
        let (id, _) = document.descendants(document.root()).next().unwrap();
        let block = blocks::block(&document, id).unwrap();
        node_commands(&block, linked, TODAY)
            .into_iter()
            .map(|group| group.into_iter().map(|command| command.label).collect())
            .collect()
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
        for (line, label, typed) in [
            ("a", "Make Section", Typed::Section),
            ("a @section", "Remove Section", Typed::Text),
        ] {
            let document = format::parse(line);
            let (id, _) = document.descendants(document.root()).next().unwrap();
            let block = blocks::block(&document, id).unwrap();
            let command = node_commands(&block, false, TODAY)
                .swap_remove(0)
                .swap_remove(3);
            assert_eq!(
                command,
                NodeCommand {
                    label,
                    action: Action::Retype(typed)
                }
            );
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
        assert_eq!(groups[3].last(), Some(&"Remove Color"));
        assert_eq!(
            groups[4],
            ["Show Details", "Open Link", "Duplicate", "Delete"]
        );
    }

    // D-50
    #[test]
    fn each_command_runs_its_existing_message() {
        let document = format::parse("- a @start(2026-10-01) @due(2026-10-09) @red");
        let (id, _) = document.descendants(document.root()).next().unwrap();
        let block = blocks::block(&document, id).unwrap();
        let action = |label: &str| {
            node_commands(&block, true, TODAY)
                .into_iter()
                .flatten()
                .find(|command| command.label == label)
                .unwrap()
                .action
        };
        let run = Action::Run;
        let cases = [
            ("Project", Action::Retype(Typed::Project)),
            ("Note", Action::Retype(Typed::Text)),
            ("Task", Action::Retype(Typed::Task)),
            ("Complete", run(Message::ToggleDone { today: TODAY })),
            (
                "Set Start Date",
                run(Message::OpenDatePrompt {
                    tag: DateTag::Start,
                    today: TODAY,
                }),
            ),
            (
                "Set Due Date",
                run(Message::OpenDatePrompt {
                    tag: DateTag::Due,
                    today: TODAY,
                }),
            ),
            (
                "Start Today",
                run(Message::ChangeDate {
                    tag: DateTag::Start,
                    change: DateChange::Today,
                    today: TODAY,
                }),
            ),
            (
                "Start Tomorrow",
                run(Message::ChangeDate {
                    tag: DateTag::Start,
                    change: DateChange::Tomorrow,
                    today: TODAY,
                }),
            ),
            (
                "Due Today",
                run(Message::ChangeDate {
                    tag: DateTag::Due,
                    change: DateChange::Today,
                    today: TODAY,
                }),
            ),
            (
                "Due Tomorrow",
                run(Message::ChangeDate {
                    tag: DateTag::Due,
                    change: DateChange::Tomorrow,
                    today: TODAY,
                }),
            ),
            ("Clear Start Date", run(Message::ClearDate(DateTag::Start))),
            ("Clear Due Date", run(Message::ClearDate(DateTag::Due))),
            ("Toggle Today", run(Message::ToggleFlag(Flag::Today))),
            ("Bookmark", run(Message::ToggleFlag(Flag::Bookmark))),
            ("Color: Red", run(Message::SetColor(Some(TagColor::Red)))),
            (
                "Color: Orange",
                run(Message::SetColor(Some(TagColor::Orange))),
            ),
            (
                "Color: Yellow",
                run(Message::SetColor(Some(TagColor::Yellow))),
            ),
            (
                "Color: Green",
                run(Message::SetColor(Some(TagColor::Green))),
            ),
            ("Color: Blue", run(Message::SetColor(Some(TagColor::Blue)))),
            (
                "Color: Violet",
                run(Message::SetColor(Some(TagColor::Violet))),
            ),
            ("Remove Color", run(Message::SetColor(None))),
            ("Show Details", run(Message::OpenDetails)),
            ("Open Link", run(Message::OpenUrl)),
            ("Duplicate", run(Message::Duplicate)),
            ("Delete", run(Message::Delete)),
        ];
        for (label, expected) in cases {
            assert_eq!(action(label), expected, "{label}");
        }
    }
}
