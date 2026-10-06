//! The command menu that `/` opens in edit mode: the commands it offers for the edited block, the
//! filter typed after `/`, the highlighted command, and what each key does to them
//! ([editing.md §2.3](../../../../docs/specs/editing.md#23-command-menu), D-44, D-50).

use super::super::node_commands::{Action, NodeCommand};

/// A key of the open command menu or a picker, as an intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuInput {
    /// Typing a character of the filter.
    Type(char),
    /// Erasing the filter's last character, or closing the menu when the filter is empty.
    Erase,
    /// Highlighting the entry above.
    Previous,
    /// Highlighting the entry below.
    Next,
    /// Choosing the highlighted entry.
    Choose,
    /// Closing the menu and typing what it took as text.
    Dismiss,
}

/// The open menu as a frontend shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandMenuView<'a> {
    /// What was typed after `/`.
    pub filter: &'a str,
    /// The commands the filter matches, in order, with a divider between groups; never empty.
    pub rows: Vec<MenuRow>,
    /// The position of the highlighted command in `rows`, which is never a divider.
    pub selected: usize,
}

/// A row of the open menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuRow {
    /// A command, by its label.
    Command(&'static str),
    /// A line between two groups of commands, which cannot be highlighted.
    Divider,
}

/// The open menu: the commands it offers in groups, the filter and the highlighted command,
/// counted among those the filter matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CommandMenu {
    groups: Vec<Vec<NodeCommand>>,
    filter: String,
    selected: usize,
}

/// How an input closed the menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Closed {
    /// The chosen command runs.
    Run(Action),
    /// This text is typed, the `/` and the filter that chose nothing.
    Literal(String),
    /// Nothing is typed: the `/` was erased.
    Erased,
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

    /// The groups with the commands whose label the filter matches, leaving out groups with
    /// none.
    fn matching_groups(&self) -> impl Iterator<Item = Vec<&NodeCommand>> {
        self.groups
            .iter()
            .map(|group| {
                group
                    .iter()
                    .filter(|command| matches(command.label, &self.filter))
                    .collect::<Vec<_>>()
            })
            .filter(|group| !group.is_empty())
    }

    /// The commands whose label the filter matches.
    fn matching(&self) -> impl Iterator<Item = &NodeCommand> {
        self.matching_groups().flatten()
    }

    /// The menu as a frontend shows it.
    pub(super) fn view(&self) -> CommandMenuView<'_> {
        let mut rows = Vec::new();
        let mut selected = 0;
        for (index, group) in self.matching_groups().enumerate() {
            if index > 0 {
                rows.push(MenuRow::Divider);
            }
            for command in group {
                if rows.iter().filter(|row| **row != MenuRow::Divider).count() == self.selected {
                    selected = rows.len();
                }
                rows.push(MenuRow::Command(command.label));
            }
        }
        CommandMenuView {
            filter: &self.filter,
            rows,
            selected,
        }
    }

    /// The `/` and the filter, as typed.
    fn literal(&self) -> String {
        format!("/{}", self.filter)
    }

    /// Applies `input`, and returns how it closed the menu, or `None` while it stays open. A
    /// filter that matches no command closes the menu, so that `/` followed by text, as in a
    /// path, stays text.
    pub(super) fn input(&mut self, input: MenuInput) -> Option<Closed> {
        match input {
            MenuInput::Type(c) => {
                self.filter.push(c);
                self.selected = 0;
                self.matching()
                    .next()
                    .is_none()
                    .then(|| Closed::Literal(self.literal()))
            }
            MenuInput::Erase => {
                self.selected = 0;
                self.filter.pop().is_none().then_some(Closed::Erased)
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
                .map(|command| Closed::Run(command.action.clone())),
            MenuInput::Dismiss => Some(Closed::Literal(self.literal())),
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
    use crate::{
        app::Message,
        document::{DateTag, format::Typed},
    };

    /// A menu offering a command for each of `labels`, each running a message of its own, in
    /// groups of the sizes `GROUPS`.
    fn offering(labels: &[&'static str]) -> CommandMenu {
        let mut commands = labels
            .iter()
            .enumerate()
            .map(|(index, &label)| NodeCommand {
                label,
                action: Action::Run(Message::PageDown { rows: index }),
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

    fn typed(menu: &mut CommandMenu, text: &str) -> Option<Closed> {
        text.chars()
            .map(|c| menu.input(MenuInput::Type(c)))
            .last()
            .flatten()
    }

    /// The labels the menu shows, with `|` for a divider.
    fn shown(menu: &CommandMenu) -> Vec<&'static str> {
        menu.view()
            .rows
            .into_iter()
            .map(|row| match row {
                MenuRow::Command(label) => label,
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
        assert_eq!(
            typed(&mut menu, "date s"),
            Some(Closed::Literal("/date s".to_owned()))
        );
        let mut menu = offering(&LABELS);
        assert_eq!(
            typed(&mut menu, "ate"),
            Some(Closed::Literal("/ate".to_owned()))
        );
    }

    // D-44, D-50
    #[test]
    fn a_filter_that_matches_nothing_closes_the_menu_as_text() {
        let mut menu = offering(&LABELS);
        assert_eq!(
            typed(&mut menu, "h"),
            Some(Closed::Literal("/h".to_owned()))
        );
        let mut menu = offering(&LABELS);
        assert_eq!(
            menu.input(MenuInput::Type(' ')),
            Some(Closed::Literal("/ ".to_owned())),
            "a slash between words"
        );
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
            Some(Closed::Run(Action::Run(Message::PageDown { rows: 4 }))),
            "Due Today"
        );
    }

    // D-50
    #[test]
    fn choosing_a_type_retypes_the_block() {
        let mut menu = CommandMenu::new(vec![vec![
            NodeCommand {
                label: "Project",
                action: Action::Retype(Typed::Project),
            },
            NodeCommand {
                label: "Clear Start Date",
                action: Action::Run(Message::ClearDate(DateTag::Start)),
            },
        ]]);
        assert_eq!(
            menu.input(MenuInput::Choose),
            Some(Closed::Run(Action::Retype(Typed::Project)))
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
            Some(Closed::Run(Action::Run(Message::PageDown { rows: 5 })))
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
            Some(Closed::Run(Action::Run(Message::PageDown { rows: 2 })))
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
        assert_eq!(menu.input(MenuInput::Erase), Some(Closed::Erased));
    }

    // D-44
    #[test]
    fn dismissing_types_the_slash_and_the_filter() {
        let mut menu = offering(&LABELS);
        assert_eq!(
            menu.input(MenuInput::Dismiss),
            Some(Closed::Literal("/".to_owned()))
        );
        let mut menu = offering(&LABELS);
        typed(&mut menu, "de");
        assert_eq!(
            menu.input(MenuInput::Dismiss),
            Some(Closed::Literal("/de".to_owned()))
        );
    }
}
