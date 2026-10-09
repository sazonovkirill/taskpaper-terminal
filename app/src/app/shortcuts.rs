//! The shortcuts panel that `?` opens in the secondary sidebar: the most useful bindings of the
//! current mode, grouped ([ui.md §4](../../../docs/specs/ui.md#4-shortcuts-panel-fr-key-04),
//! FR-KEY-04, D-28).

use super::{
    State,
    commands::{self, Entry, Group},
};

impl State {
    /// While the panel is open, what it lists for the current mode: each group with the registry
    /// entries of its commands (FR-KEY-04).
    #[must_use]
    pub fn shortcuts(&self) -> Option<Vec<(Group, Vec<&'static Entry>)>> {
        self.shortcuts.then(|| {
            let mut groups = commands::shortcuts(self.mode());
            for (_, entries) in &mut groups {
                entries.retain(|entry| self.offers(entry.command));
            }
            groups
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::{Message, Mode},
        document::format,
    };

    /// The names of the groups the open panel lists.
    fn groups(state: &State) -> Option<Vec<&'static str>> {
        state
            .shortcuts()
            .map(|groups| groups.iter().map(|(group, _)| group.name()).collect())
    }

    // FR-KEY-04
    #[test]
    fn the_panel_opens_and_closes_and_lists_the_current_modes_bindings() {
        let mut state = State::new(format::parse("- a"), None);
        assert_eq!(state.shortcuts(), None, "closed at start");
        state.update(Message::ToggleShortcuts);
        assert_eq!(state.shortcuts(), Some(commands::shortcuts(Mode::Navigate)));
        state.update(Message::EditAtEnd);
        assert_eq!(
            groups(&state),
            Some(vec!["Editing", "Structure", "View and Panels"]),
            "it stays open while editing and lists edit mode's keys"
        );
        assert_eq!(state.mode(), Mode::Edit);
        state.update(Message::StopEditing);
        state.update(Message::ToggleShortcuts);
        assert_eq!(state.shortcuts(), None);
        state.update(Message::ToggleShortcuts);
        state.update(Message::CloseShortcuts);
        assert_eq!(state.shortcuts(), None);
        state.update(Message::CloseShortcuts);
        assert_eq!(
            state.shortcuts(),
            None,
            "closing a closed panel does nothing"
        );
    }

    // FR-KEY-04
    #[test]
    fn toggling_the_panel_keeps_an_edit_session() {
        let mut state = State::new(format::parse("- a"), None);
        state.update(Message::EditAtEnd);
        state.update(Message::ToggleShortcuts);
        assert_eq!(state.mode(), Mode::Edit);
        state.update(Message::CloseShortcuts);
        assert_eq!(state.mode(), Mode::Edit);
        assert_eq!(state.shortcuts(), None);
    }
}
