//! A menu of commands in groups, as the command menu of edit mode and the context menu list them:
//! the filter typed into it, which matches the starts of the labels' words, the highlighted
//! command, and what each key does to them
//! ([editing.md §2.3](../../../docs/specs/editing.md#23-command-menu),
//! [ui.md §5](../../../docs/specs/ui.md#5-context-menu-fr-key-06), D-50, FR-KEY-06).

use super::commands::{Command, NodeCommand};

/// A key of an open menu or picker, as an intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuInput {
    /// Typing a character of the filter.
    Type(char),
    /// Erasing the filter's last character.
    Erase,
    /// Highlighting the entry above.
    Previous,
    /// Highlighting the entry below.
    Next,
    /// Choosing the highlighted entry.
    Choose,
    /// Closing the menu.
    Dismiss,
}

/// The open menu as a frontend shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandMenuView<'a> {
    /// What was typed into it.
    pub filter: &'a str,
    /// The commands the filter matches, in order, with a divider between groups; never empty.
    pub rows: Vec<MenuRow<'a>>,
    /// The position of the highlighted command in `rows`, which is never a divider.
    pub selected: usize,
}

/// A row of the open menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuRow<'a> {
    /// A command with its label.
    Command {
        /// What the menu shows, which its filter matches.
        label: &'a str,
        /// The command choosing it gives.
        command: Command,
    },
    /// A line between two groups of commands, which cannot be highlighted.
    Divider,
}

/// The open menu: the commands it offers in groups, the filter, which always matches one of them,
/// and the highlighted command, counted among those the filter matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CommandMenu {
    groups: Vec<Vec<NodeCommand>>,
    filter: String,
    selected: usize,
}

/// What an input did that the menu's owner decides on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Outcome {
    /// The highlighted command was chosen.
    Run(Command),
    /// This character was typed, and not added to the filter, since no command would match it.
    Unmatched(char),
    /// Erasing found the filter empty.
    Erased,
    /// The menu was dismissed.
    Dismissed,
}

impl CommandMenu {
    /// The menu offering the commands of `groups`, with no filter and the first highlighted.
    pub(super) fn new(groups: Vec<Vec<NodeCommand>>) -> Self {
        Self {
            groups,
            filter: String::new(),
            selected: 0,
        }
    }

    /// What was typed into the menu.
    pub(super) fn filter(&self) -> &str {
        &self.filter
    }

    /// The groups with the commands whose label `filter` matches, leaving out groups with none.
    fn matching_groups<'a>(
        &'a self,
        filter: &'a str,
    ) -> impl Iterator<Item = Vec<&'a NodeCommand>> {
        self.groups
            .iter()
            .map(move |group| {
                group
                    .iter()
                    .filter(|command| matches(&command.label, filter))
                    .collect::<Vec<_>>()
            })
            .filter(|group| !group.is_empty())
    }

    /// The commands whose label the filter matches.
    fn matching(&self) -> impl Iterator<Item = &NodeCommand> {
        self.matching_groups(&self.filter).flatten()
    }

    /// The menu as a frontend shows it.
    pub(super) fn view(&self) -> CommandMenuView<'_> {
        let mut rows = Vec::new();
        let mut selected = 0;
        for (index, group) in self.matching_groups(&self.filter).enumerate() {
            if index > 0 {
                rows.push(MenuRow::Divider);
            }
            for command in group {
                if rows.iter().filter(|row| **row != MenuRow::Divider).count() == self.selected {
                    selected = rows.len();
                }
                rows.push(MenuRow::Command {
                    label: &command.label,
                    command: command.command,
                });
            }
        }
        CommandMenuView {
            filter: &self.filter,
            rows,
            selected,
        }
    }

    /// Applies `input`, and returns what it did that the menu's owner decides on, or `None` when
    /// it only changed the filter or the highlight.
    pub(super) fn input(&mut self, input: MenuInput) -> Option<Outcome> {
        match input {
            MenuInput::Type(c) => {
                let filter = format!("{}{c}", self.filter);
                if self.matching_groups(&filter).next().is_none() {
                    return Some(Outcome::Unmatched(c));
                }
                self.filter = filter;
                self.selected = 0;
                None
            }
            MenuInput::Erase => {
                self.selected = 0;
                self.filter.pop().is_none().then_some(Outcome::Erased)
            }
            MenuInput::Previous => {
                self.selected = self.selected.saturating_sub(1);
                None
            }
            MenuInput::Next => {
                let last = self.matching().count().saturating_sub(1);
                self.selected = (self.selected + 1).min(last);
                None
            }
            MenuInput::Choose => self
                .matching()
                .nth(self.selected)
                .map(|command| Outcome::Run(command.command)),
            MenuInput::Dismiss => Some(Outcome::Dismissed),
        }
    }
}

/// Whether `filter` matches `label`: each of its words, ignoring case, starts a word of the label,
/// in order (D-50). A filter starting with a space matches nothing, so that a `/` typed between
/// words stays text.
fn matches(label: &str, filter: &str) -> bool {
    if filter.starts_with(char::is_whitespace) {
        return false;
    }
    let label = label.to_lowercase();
    let mut words = label.split_whitespace();
    filter
        .to_lowercase()
        .split_whitespace()
        .all(|wanted| words.any(|word| word.starts_with(wanted)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{DateTag, format::Typed};

    /// A menu offering the command of [`COMMANDS`] at the same position for each of `labels`,
    /// in groups of the sizes `GROUPS`.
    fn offering(labels: &[&'static str]) -> CommandMenu {
        let mut commands = labels
            .iter()
            .zip(COMMANDS)
            .map(|(&label, command)| NodeCommand {
                label: label.into(),
                command,
            });
        CommandMenu::new(
            GROUPS
                .iter()
                .map(|&size| commands.by_ref().take(size).collect())
                .collect(),
        )
    }

    /// The sizes of the groups of [`LABELS`].
    const GROUPS: [usize; 3] = [2, 4, 2];

    /// A command for each of [`LABELS`], each different.
    const COMMANDS: [Command; 8] = [
        Command::FocusNext,
        Command::FocusPrevious,
        Command::FocusFirst,
        Command::FocusLast,
        Command::FocusParent,
        Command::Collapse,
        Command::Expand,
        Command::ZoomIn,
    ];

    const LABELS: [&str; 8] = [
        "Project",
        "Section",
        "Set Start Date",
        "Set Due Date",
        "Due Today",
        "Clear Due Date",
        "Color: Red",
        "Delete",
    ];

    fn typed(menu: &mut CommandMenu, text: &str) -> Option<Outcome> {
        text.chars()
            .map(|c| menu.input(MenuInput::Type(c)))
            .last()
            .flatten()
    }

    /// The labels the menu shows, with `|` for a divider.
    fn shown(menu: &CommandMenu) -> Vec<&str> {
        menu.view()
            .rows
            .into_iter()
            .map(|row| match row {
                MenuRow::Command { label, .. } => label,
                MenuRow::Divider => "|",
            })
            .collect()
    }

    // D-50
    #[test]
    fn opens_with_every_command_and_the_first_highlighted() {
        let menu = offering(&LABELS);
        assert_eq!(
            shown(&menu),
            [
                "Project",
                "Section",
                "|",
                "Set Start Date",
                "Set Due Date",
                "Due Today",
                "Clear Due Date",
                "|",
                "Color: Red",
                "Delete",
            ]
        );
        assert_eq!((menu.view().filter, menu.view().selected), ("", 0));
    }

    // D-50
    #[test]
    fn the_filter_matches_word_prefixes_in_order_ignoring_case() {
        for (filter, expected) in [
            (
                "date",
                &["Set Start Date", "Set Due Date", "Clear Due Date"][..],
            ),
            ("DUE", &["Set Due Date", "Due Today", "Clear Due Date"]),
            ("s", &["Section", "|", "Set Start Date", "Set Due Date"]),
            (
                "d",
                &[
                    "Set Start Date",
                    "Set Due Date",
                    "Due Today",
                    "Clear Due Date",
                    "|",
                    "Delete",
                ],
            ),
            ("r", &["Color: Red"]),
            ("set d", &["Set Start Date", "Set Due Date"]),
            ("set due ", &["Set Due Date"]),
            ("color: r", &["Color: Red"]),
            ("p", &["Project"]),
        ] {
            let mut menu = offering(&LABELS);
            assert_eq!(typed(&mut menu, filter), None, "{filter}");
            assert_eq!(shown(&menu), expected, "{filter}");
            assert_eq!(menu.view().filter, filter);
        }
    }

    // D-50
    #[test]
    fn words_match_only_in_order_and_from_their_start() {
        let mut menu = offering(&LABELS);
        assert_eq!(typed(&mut menu, "date s"), Some(Outcome::Unmatched('s')));
        assert_eq!(menu.filter(), "date ");
        let mut menu = offering(&LABELS);
        assert_eq!(typed(&mut menu, "a"), Some(Outcome::Unmatched('a')));
        assert_eq!(menu.filter(), "");
    }

    // D-44, D-50, FR-KEY-06
    #[test]
    fn a_character_that_would_match_nothing_is_not_added_to_the_filter() {
        let mut menu = offering(&LABELS);
        assert_eq!(typed(&mut menu, "c"), None);
        assert_eq!(typed(&mut menu, "h"), Some(Outcome::Unmatched('h')));
        assert_eq!(menu.filter(), "c");
        assert_eq!(shown(&menu), ["Clear Due Date", "|", "Color: Red"]);
        let mut menu = offering(&LABELS);
        assert_eq!(
            menu.input(MenuInput::Type(' ')),
            Some(Outcome::Unmatched(' ')),
            "a slash between words"
        );
        assert_eq!(menu.filter(), "");
    }

    // D-50
    #[test]
    fn enter_runs_the_highlighted_match_and_a_new_filter_highlights_the_first() {
        let mut menu = offering(&LABELS);
        typed(&mut menu, "d");
        menu.input(MenuInput::Next);
        menu.input(MenuInput::Next);
        assert_eq!(menu.view().selected, 2);
        typed(&mut menu, "u");
        assert_eq!(menu.view().selected, 0);
        menu.input(MenuInput::Next);
        assert_eq!(
            menu.input(MenuInput::Choose),
            Some(Outcome::Run(COMMANDS[4])),
            "Due Today"
        );
    }

    // D-50
    #[test]
    fn choosing_a_type_retypes_the_block() {
        let mut menu = CommandMenu::new(vec![vec![
            NodeCommand {
                label: "Project".into(),
                command: Command::Retype(Typed::Project),
            },
            NodeCommand {
                label: "Clear Start Date".into(),
                command: Command::ClearDate(DateTag::Start),
            },
        ]]);
        assert_eq!(
            menu.input(MenuInput::Choose),
            Some(Outcome::Run(Command::Retype(Typed::Project)))
        );
    }

    // D-50
    #[test]
    fn the_highlight_skips_the_dividers() {
        let mut menu = offering(&LABELS);
        menu.input(MenuInput::Next);
        assert_eq!(menu.view().selected, 1);
        menu.input(MenuInput::Next);
        assert_eq!(menu.view().selected, 3, "past the divider");
        typed(&mut menu, "d");
        for _ in 0..4 {
            menu.input(MenuInput::Next);
        }
        assert_eq!(menu.view().selected, 5, "Delete, after the divider");
        menu.input(MenuInput::Previous);
        assert_eq!(menu.view().selected, 3);
        assert_eq!(
            menu.input(MenuInput::Choose),
            Some(Outcome::Run(COMMANDS[5]))
        );
    }

    // D-44
    #[test]
    fn arrows_move_within_the_matches() {
        let mut menu = offering(&LABELS);
        menu.input(MenuInput::Previous);
        assert_eq!(menu.view().selected, 0);
        typed(&mut menu, "set");
        for _ in 0..9 {
            menu.input(MenuInput::Next);
        }
        assert_eq!(menu.view().selected, 1);
        menu.input(MenuInput::Previous);
        assert_eq!(
            menu.input(MenuInput::Choose),
            Some(Outcome::Run(COMMANDS[2]))
        );
    }

    // D-44
    #[test]
    fn erase_shortens_the_filter_and_then_closes() {
        let mut menu = offering(&LABELS);
        typed(&mut menu, "s");
        menu.input(MenuInput::Next);
        assert_eq!(menu.input(MenuInput::Erase), None);
        assert_eq!((menu.view().filter, menu.view().selected), ("", 0));
        assert_eq!(shown(&menu).len(), LABELS.len() + 2);
        assert_eq!(menu.input(MenuInput::Erase), Some(Outcome::Erased));
    }

    // D-44
    #[test]
    fn dismissing_keeps_the_filter() {
        let mut menu = offering(&LABELS);
        typed(&mut menu, "de");
        assert_eq!(menu.input(MenuInput::Dismiss), Some(Outcome::Dismissed));
        assert_eq!(menu.filter(), "de");
    }
}
