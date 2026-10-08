//! Go to Command: a fuzzy finder over the commands of navigate mode, each with its bindings, that
//! gives the chosen one
//! ([navigation.md §8](../../../docs/specs/navigation.md#8-go-to-symbol-and-go-to-command-fr-nav-05-fr-nav-06),
//! FR-NAV-06).

use super::{
    Command, Effect, MenuInput, Mode, State,
    commands::REGISTRY,
    finder::{Candidate, Finder, FinderView},
    picker::Picked,
};

impl State {
    /// Go to Command, while it is open (FR-NAV-06).
    #[must_use]
    pub fn go_to_command(&self) -> Option<FinderView<'_, Command>> {
        self.commands.as_ref().map(Finder::view)
    }

    /// `Ctrl+P`: opens Go to Command over every command of navigate mode but itself, by name,
    /// each with its entry of `bindings`, if it has one.
    pub(super) fn open_go_to_command(&mut self, bindings: &[(Command, String)]) {
        let mut candidates: Vec<Candidate<Command>> = REGISTRY
            .iter()
            .filter(|entry| entry.mode == Mode::Navigate && entry.command != Command::GoToCommand)
            .map(|entry| Candidate {
                value: entry.command,
                text: entry.command.name().into_owned(),
                detail: bindings
                    .iter()
                    .find(|(command, _)| *command == entry.command)
                    .map(|(_, keys)| keys.clone())
                    .unwrap_or_default(),
                tier: 0,
            })
            .collect();
        candidates.sort_by(|a, b| a.text.cmp(&b.text));
        self.commands = Some(Finder::new(candidates));
    }

    /// Applies `input` to Go to Command, and once a command is chosen, closes it and asks the
    /// frontend to give the command.
    pub(super) fn go_to_command_input(&mut self, input: MenuInput) -> Option<Effect> {
        let closed = self.commands.as_mut()?.input(input)?;
        self.commands = None;
        match closed {
            Picked::Chosen(command) => Some(Effect::Run(command)),
            Picked::Cancelled => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::{Menu, Message},
        document::format,
    };

    fn state() -> State {
        State::new(format::parse("- a\n"), None)
    }

    fn typed(state: &mut State, text: &str) {
        for c in text.chars() {
            assert_eq!(
                state.update(Message::Menu(Menu::GoToCommand, MenuInput::Type(c))),
                []
            );
        }
    }

    /// The name and detail of each entry Go to Command lists.
    fn listed(state: &State) -> Vec<(String, String)> {
        state
            .go_to_command()
            .unwrap()
            .entries
            .iter()
            .map(|entry| (entry.text.clone(), entry.detail.clone()))
            .collect()
    }

    // FR-NAV-06
    #[test]
    fn lists_every_command_of_navigate_mode_but_itself_by_name_with_its_bindings() {
        let mut state = state();
        assert_eq!(state.go_to_command(), None);
        state.update(Message::OpenGoToCommand(vec![(
            Command::Quit,
            "Ctrl+Q".to_owned(),
        )]));
        let listed = listed(&state);
        let navigate: Vec<Command> = REGISTRY
            .iter()
            .filter(|entry| entry.mode == Mode::Navigate)
            .map(|entry| entry.command)
            .collect();
        assert_eq!(listed.len(), navigate.len() - 1);
        assert_eq!(listed[0], ("Animations: Full".to_owned(), String::new()));
        assert!(listed.is_sorted(), "by name");
        assert!(listed.contains(&("Quit".to_owned(), "Ctrl+Q".to_owned())));
        assert!(listed.iter().all(|(name, _)| name != "Go to Command"));
        assert!(
            listed
                .iter()
                .any(|(name, _)| name == "Count Blocks and Words"),
            "a command without keys is listed"
        );
        assert!(
            listed.iter().all(|(name, _)| name != "Stop Editing"),
            "edit mode's commands are not"
        );
    }

    // FR-NAV-06
    #[test]
    fn choosing_a_command_closes_it_and_asks_to_give_the_command() {
        let mut state = state();
        state.update(Message::OpenGoToCommand(Vec::new()));
        typed(&mut state, "zoom in");
        assert_eq!(
            state.update(Message::Menu(Menu::GoToCommand, MenuInput::Choose)),
            [Effect::Run(Command::ZoomIn)]
        );
        assert_eq!(state.go_to_command(), None);
    }

    #[test]
    fn dismissing_or_choosing_nothing_gives_no_command() {
        let mut state = state();
        state.update(Message::OpenGoToCommand(Vec::new()));
        assert_eq!(
            state.update(Message::Menu(Menu::GoToCommand, MenuInput::Dismiss)),
            []
        );
        assert_eq!(state.go_to_command(), None);
        state.update(Message::OpenGoToCommand(Vec::new()));
        typed(&mut state, "zzzz");
        assert_eq!(
            state.update(Message::Menu(Menu::GoToCommand, MenuInput::Choose)),
            []
        );
        assert!(
            state.go_to_command().is_some(),
            "nothing to choose keeps it open"
        );
    }

    #[test]
    fn keys_without_go_to_command_open_do_nothing() {
        let mut state = state();
        assert_eq!(
            state.update(Message::Menu(Menu::GoToCommand, MenuInput::Choose)),
            []
        );
        assert_eq!(state.go_to_command(), None);
    }
}
