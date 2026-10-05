//! The type menu that `/` opens in edit mode: its entries, the filter typed after `/`, the
//! highlighted entry, and what each key does to them
//! ([editing.md §2.3](../../../../docs/specs/editing.md#23-type-menu), D-44).

use crate::document::format::Typed;

/// One entry of the type menu: a type, its name and the letter that chooses it at once, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuEntry {
    /// The type the entry gives the block.
    pub typed: Typed,
    /// The type's name, which the menu shows and the filter matches.
    pub name: &'static str,
    /// The letter that, typed first after `/`, chooses the entry at once.
    pub shortcut: Option<char>,
}

/// Every entry, in the order the menu lists them (D-44).
const ENTRIES: [MenuEntry; 5] = [
    MenuEntry {
        typed: Typed::Project,
        name: "Project",
        shortcut: Some('p'),
    },
    MenuEntry {
        typed: Typed::Section,
        name: "Section",
        shortcut: Some('s'),
    },
    MenuEntry {
        typed: Typed::Text,
        name: "Note",
        shortcut: Some('n'),
    },
    MenuEntry {
        typed: Typed::Task,
        name: "Task",
        shortcut: Some('t'),
    },
    MenuEntry {
        typed: Typed::Divider,
        name: "Divider",
        shortcut: None,
    },
];

/// A key of the open type menu or color picker, as an intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuInput {
    /// Typing a character: a shortcut letter right after `/`, else more of the filter.
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
pub struct TypeMenuView<'a> {
    /// What was typed after `/`.
    pub filter: &'a str,
    /// The entries the filter matches, in order; never empty.
    pub entries: Vec<MenuEntry>,
    /// The position of the highlighted entry in `entries`.
    pub selected: usize,
}

/// The open menu: the filter and the highlighted entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct TypeMenu {
    filter: String,
    selected: usize,
}

/// How an input closed the menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Closed {
    /// The block takes this type.
    Apply(Typed),
    /// This text is typed, the `/` and the filter that chose nothing.
    Literal(String),
    /// Nothing is typed: the `/` was erased.
    Erased,
}

impl TypeMenu {
    /// The entries whose name holds the filter, ignoring case.
    fn entries(&self) -> Vec<MenuEntry> {
        let filter = self.filter.to_lowercase();
        ENTRIES
            .into_iter()
            .filter(|entry| entry.name.to_lowercase().contains(&filter))
            .collect()
    }

    /// The menu as a frontend shows it.
    pub(super) fn view(&self) -> TypeMenuView<'_> {
        TypeMenuView {
            filter: &self.filter,
            entries: self.entries(),
            selected: self.selected,
        }
    }

    /// The `/` and the filter, as typed.
    fn literal(&self) -> String {
        format!("/{}", self.filter)
    }

    /// Applies `input`, and returns how it closed the menu, or `None` while it stays open. A
    /// filter that matches no entry closes the menu, so that `/` followed by text, as in a path,
    /// stays text.
    pub(super) fn input(&mut self, input: MenuInput) -> Option<Closed> {
        match input {
            MenuInput::Type(c) => {
                let shortcut = ENTRIES
                    .into_iter()
                    .find(|entry| entry.shortcut == Some(c.to_ascii_lowercase()))
                    .filter(|_| self.filter.is_empty());
                if let Some(entry) = shortcut {
                    return Some(Closed::Apply(entry.typed));
                }
                self.filter.push(c);
                self.selected = 0;
                self.entries()
                    .is_empty()
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
                self.selected = (self.selected + 1).min(self.entries().len().saturating_sub(1));
                None
            }
            MenuInput::Choose => self
                .entries()
                .get(self.selected)
                .map(|entry| Closed::Apply(entry.typed)),
            MenuInput::Dismiss => Some(Closed::Literal(self.literal())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(menu: &TypeMenu) -> Vec<&'static str> {
        menu.view().entries.iter().map(|entry| entry.name).collect()
    }

    // D-44
    #[test]
    fn opens_with_every_entry_and_the_first_highlighted() {
        let menu = TypeMenu::default();
        assert_eq!(
            names(&menu),
            ["Project", "Section", "Note", "Task", "Divider"]
        );
        let shortcuts: String = menu
            .view()
            .entries
            .iter()
            .filter_map(|e| e.shortcut)
            .collect();
        assert_eq!(shortcuts, "psnt");
        assert_eq!((menu.view().filter, menu.view().selected), ("", 0));
    }

    // D-44
    #[test]
    fn a_shortcut_letter_right_after_the_slash_chooses_at_once() {
        for (c, typed) in [
            ('p', Typed::Project),
            ('s', Typed::Section),
            ('n', Typed::Text),
            ('t', Typed::Task),
            ('S', Typed::Section),
        ] {
            let mut menu = TypeMenu::default();
            assert_eq!(menu.input(MenuInput::Type(c)), Some(Closed::Apply(typed)));
        }
    }

    // D-44
    #[test]
    fn other_text_filters_by_name_ignoring_case() {
        let mut menu = TypeMenu::default();
        assert_eq!(menu.input(MenuInput::Type('e')), None);
        assert_eq!(names(&menu), ["Project", "Section", "Note", "Divider"]);
        menu.input(MenuInput::Next);
        assert_eq!(menu.input(MenuInput::Type('C')), None);
        assert_eq!(names(&menu), ["Project", "Section"]);
        assert_eq!(menu.view().selected, 0, "a new filter highlights the first");
        assert_eq!(menu.view().filter, "eC");
        assert_eq!(menu.input(MenuInput::Type('t')), None, "not first: filters");
        assert_eq!(names(&menu), ["Project", "Section"]);
        menu.input(MenuInput::Next);
        assert_eq!(
            menu.input(MenuInput::Choose),
            Some(Closed::Apply(Typed::Section))
        );
    }

    // D-44
    #[test]
    fn the_divider_is_chosen_by_its_name() {
        let mut menu = TypeMenu::default();
        assert_eq!(menu.input(MenuInput::Type('d')), None, "d is no shortcut");
        assert_eq!(names(&menu), ["Divider"]);
        menu.input(MenuInput::Type('i'));
        menu.input(MenuInput::Type('v'));
        assert_eq!(
            menu.input(MenuInput::Choose),
            Some(Closed::Apply(Typed::Divider))
        );
    }

    // D-44
    #[test]
    fn a_filter_that_matches_nothing_closes_the_menu_as_text() {
        let mut menu = TypeMenu::default();
        assert_eq!(menu.input(MenuInput::Type('a')), None);
        assert_eq!(names(&menu), ["Task"]);
        assert_eq!(
            menu.input(MenuInput::Type('x')),
            Some(Closed::Literal("/ax".to_owned()))
        );
        let mut menu = TypeMenu::default();
        assert_eq!(
            menu.input(MenuInput::Type(' ')),
            Some(Closed::Literal("/ ".to_owned()))
        );
    }

    // D-44
    #[test]
    fn arrows_move_within_the_entries() {
        let mut menu = TypeMenu::default();
        menu.input(MenuInput::Previous);
        assert_eq!(menu.view().selected, 0);
        for _ in 0..9 {
            menu.input(MenuInput::Next);
        }
        assert_eq!(menu.view().selected, 4);
        menu.input(MenuInput::Previous);
        assert_eq!(
            menu.input(MenuInput::Choose),
            Some(Closed::Apply(Typed::Task))
        );
    }

    // D-44
    #[test]
    fn erase_shortens_the_filter_and_then_closes() {
        let mut menu = TypeMenu::default();
        menu.input(MenuInput::Type('o'));
        menu.input(MenuInput::Next);
        assert_eq!(menu.input(MenuInput::Erase), None);
        assert_eq!((menu.view().filter, menu.view().selected), ("", 0));
        assert_eq!(names(&menu).len(), 5);
        assert_eq!(menu.input(MenuInput::Erase), Some(Closed::Erased));
    }

    // D-44
    #[test]
    fn dismissing_types_the_slash_and_the_filter() {
        let mut menu = TypeMenu::default();
        assert_eq!(
            menu.input(MenuInput::Dismiss),
            Some(Closed::Literal("/".to_owned()))
        );
        let mut menu = TypeMenu::default();
        menu.input(MenuInput::Type('r'));
        assert_eq!(
            menu.input(MenuInput::Dismiss),
            Some(Closed::Literal("/r".to_owned()))
        );
    }
}
