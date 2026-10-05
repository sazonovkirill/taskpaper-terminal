//! A picker over a fixed list of named entries, such as the color picker: the filter typed into
//! it, the highlighted entry and what each key does to them
//! ([keybindings.md §4](../../../docs/specs/keybindings.md#4-other-contexts)).

use super::MenuInput;

/// An open picker as a frontend shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerView<'a, T> {
    /// What was typed to filter the entries.
    pub filter: &'a str,
    /// The entries whose name the filter matches, in order, each a value with its name.
    pub entries: Vec<(T, &'static str)>,
    /// The position of the highlighted entry in `entries`.
    pub selected: usize,
}

/// An open picker: its entries, the filter and the highlighted entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Picker<T: 'static> {
    entries: &'static [(T, &'static str)],
    filter: String,
    selected: usize,
}

/// How an input closed a picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Picked<T> {
    /// This entry was chosen.
    Chosen(T),
    /// Nothing was chosen.
    Cancelled,
}

impl<T: Copy + PartialEq> Picker<T> {
    /// A picker listing every one of `entries` with `current` highlighted, or the first when
    /// `current` is none of them.
    pub(super) fn new(entries: &'static [(T, &'static str)], current: T) -> Self {
        Self {
            entries,
            filter: String::new(),
            selected: entries
                .iter()
                .position(|&(value, _)| value == current)
                .unwrap_or_default(),
        }
    }

    /// The picker as a frontend shows it.
    pub(super) fn view(&self) -> PickerView<'_, T> {
        PickerView {
            filter: &self.filter,
            entries: self.matching(),
            selected: self.selected,
        }
    }

    /// The entries whose name holds the filter, ignoring case.
    fn matching(&self) -> Vec<(T, &'static str)> {
        let filter = self.filter.to_lowercase();
        self.entries
            .iter()
            .filter(|(_, name)| name.to_lowercase().contains(&filter))
            .copied()
            .collect()
    }

    /// Applies `input`, and returns how it closed the picker, or `None` while it stays open.
    pub(super) fn input(&mut self, input: MenuInput) -> Option<Picked<T>> {
        match input {
            MenuInput::Type(c) => {
                self.filter.push(c);
                self.selected = 0;
            }
            MenuInput::Erase => {
                if self.filter.pop().is_some() {
                    self.selected = 0;
                }
            }
            MenuInput::Previous => self.selected = self.selected.saturating_sub(1),
            MenuInput::Next => {
                self.selected = (self.selected + 1).min(self.matching().len().saturating_sub(1));
            }
            MenuInput::Choose => {
                return self
                    .matching()
                    .get(self.selected)
                    .map(|&(value, _)| Picked::Chosen(value));
            }
            MenuInput::Dismiss => return Some(Picked::Cancelled),
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTRIES: [(u8, &str); 4] = [(1, "One"), (2, "Two"), (3, "Three"), (0, "None")];

    /// The names the picker lists and the highlighted one.
    fn listed(picker: &Picker<u8>) -> (Vec<&'static str>, usize) {
        let view = picker.view();
        (
            view.entries.iter().map(|&(_, name)| name).collect(),
            view.selected,
        )
    }

    #[test]
    fn opens_on_the_current_entry_or_the_first() {
        let picker = Picker::new(&ENTRIES, 3);
        assert_eq!(listed(&picker), (vec!["One", "Two", "Three", "None"], 2));
        assert_eq!(picker.view().filter, "");
        assert_eq!(listed(&Picker::new(&ENTRIES, 9)).1, 0);
    }

    #[test]
    fn arrows_move_within_the_entries_and_enter_chooses() {
        let mut picker = Picker::new(&ENTRIES, 2);
        assert_eq!(picker.input(MenuInput::Previous), None);
        assert_eq!(picker.input(MenuInput::Previous), None);
        assert_eq!(listed(&picker).1, 0, "the highlight stops at the first");
        for _ in 0..9 {
            picker.input(MenuInput::Next);
        }
        assert_eq!(listed(&picker).1, 3, "the highlight stops at the last");
        assert_eq!(picker.input(MenuInput::Choose), Some(Picked::Chosen(0)));
    }

    #[test]
    fn typing_filters_by_name_ignoring_case_and_erasing_widens_it() {
        let mut picker = Picker::new(&ENTRIES, 0);
        assert_eq!(picker.input(MenuInput::Type('O')), None);
        assert_eq!(listed(&picker), (vec!["One", "Two", "None"], 0));
        picker.input(MenuInput::Next);
        picker.input(MenuInput::Type('n'));
        assert_eq!(listed(&picker), (vec!["One", "None"], 0));
        assert_eq!(picker.view().filter, "On");
        picker.input(MenuInput::Next);
        assert_eq!(picker.input(MenuInput::Erase), None);
        assert_eq!(listed(&picker), (vec!["One", "Two", "None"], 0));
        picker.input(MenuInput::Erase);
        picker.input(MenuInput::Next);
        picker.input(MenuInput::Erase);
        assert_eq!(listed(&picker).1, 1, "nothing to erase keeps the highlight");
        picker.input(MenuInput::Type('x'));
        assert_eq!(listed(&picker), (vec![], 0));
        picker.input(MenuInput::Next);
        assert_eq!(picker.input(MenuInput::Choose), None, "nothing to choose");
    }

    #[test]
    fn dismissing_cancels() {
        let mut picker = Picker::new(&ENTRIES, 1);
        assert_eq!(picker.input(MenuInput::Dismiss), Some(Picked::Cancelled));
    }
}
