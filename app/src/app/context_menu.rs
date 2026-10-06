//! The context menu: the commands that apply where the keys are, on the focused sidebar entry,
//! the selection or the focused block, grouped, to filter and choose from
//! ([ui.md §5](../../../docs/specs/ui.md#5-context-menu-fr-key-06),
//! [keybindings.md §4](../../../docs/specs/keybindings.md#4-other-contexts), FR-KEY-06).

use super::{
    CommandMenuView, Effect, Inputs, MenuInput, State,
    command_menu::{CommandMenu, Outcome},
    commands::{Focused, NodeCommand, context_commands},
};

impl State {
    /// The context menu, while it is open (FR-KEY-06).
    #[must_use]
    pub fn context_menu(&self) -> Option<CommandMenuView<'_>> {
        self.context_menu.as_ref().map(CommandMenu::view)
    }

    /// `\`: opens the context menu on the focused sidebar entry while the sidebar has the keys,
    /// else on the selected blocks, else on the focused block. In the Today view it leaves out
    /// what the view does not take (navigation.md §7). Nothing opens without anything to offer.
    pub(super) fn open_context_menu(&mut self) {
        let groups = if self.sidebar_focused() {
            self.sidebar_commands().unwrap_or_default()
        } else {
            self.block_commands()
        };
        self.context_menu = (!groups.is_empty()).then(|| CommandMenu::new(groups));
    }

    /// The commands on the selected blocks, else the focused block, that the pane takes where it
    /// is, then those of the panes.
    fn block_commands(&self) -> Vec<Vec<NodeCommand>> {
        let blocks = self.targets();
        if blocks.is_empty() {
            return Vec::new();
        }
        let focused = (!self.selecting()).then(|| Focused {
            link: !self.open_url().is_empty(),
            foldable: self
                .focused_block()
                .is_some_and(|block| !self.children(&block).is_empty()),
        });
        // Only whether a command's message is taken here matters, not the date or page it
        // carries.
        let inputs = Inputs {
            today: self.today,
            page_rows: 0,
        };
        let alone = focused.is_some();
        let mut groups = context_commands(&blocks, focused);
        groups.push(self.pane_commands(alone));
        for group in &mut groups {
            group.retain(|entry| {
                entry
                    .command
                    .message(&inputs)
                    .and_then(|message| self.as_located(message))
                    .is_some()
            });
        }
        groups.retain(|group| !group.is_empty());
        groups
    }

    /// Applies `input` to the open context menu, and once a command is chosen, closes it and
    /// asks the frontend to give the command as its keys would. `Esc` closes it; a character
    /// that would match nothing is not typed.
    pub(super) fn context_menu_input(&mut self, input: MenuInput) -> Option<Effect> {
        let outcome = self.context_menu.as_mut()?.input(input)?;
        match outcome {
            Outcome::Run(command) => {
                self.context_menu = None;
                Some(Effect::Run(command))
            }
            Outcome::Dismissed => {
                self.context_menu = None;
                None
            }
            Outcome::Unmatched(_) | Outcome::Erased => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use jiff::civil::date;

    use super::*;
    use crate::{
        app::{Command, Location, Menu, MenuRow, Message, SelectionChange, SidebarInput},
        document::format,
    };

    const TEXT: &str = "Trip:\n\t- Book the hotel @due(2026-10-06)\n\t- Pack\nOther\n";

    fn open(text: &str) -> State {
        let mut state = State::new(format::parse(text), None);
        state.update(Message::Tick {
            now: Duration::ZERO,
            today: date(2026, 10, 6),
        });
        state
    }

    fn menu(state: &mut State, input: MenuInput) -> Vec<Effect> {
        state.update(Message::Menu(Menu::ContextMenu, input))
    }

    /// The labels the open menu shows, with `|` for a divider.
    fn shown(state: &State) -> Vec<String> {
        state
            .context_menu()
            .expect("the context menu is open")
            .rows
            .into_iter()
            .map(|row| match row {
                MenuRow::Command { label, .. } => label.to_owned(),
                MenuRow::Divider => "|".to_owned(),
            })
            .collect()
    }

    // FR-KEY-06
    #[test]
    fn opens_on_the_focused_block_with_its_own_commands_and_labels() {
        let mut state = open(TEXT);
        assert_eq!(state.context_menu(), None);
        state.update(Message::FocusNext);
        state.update(Message::OpenContextMenu);
        let listed = shown(&state);
        assert_eq!(listed[..2], ["Edit", "New Block Below"]);
        assert!(listed.contains(&"Clear Due Date".to_owned()));
        assert!(listed.contains(&"Zoom In".to_owned()));
        assert!(!listed.contains(&"Toggle Fold".to_owned()), "a leaf");
        assert!(!listed.contains(&"Open Link".to_owned()), "no link");
        state.update(Message::FocusFirst);
        state.update(Message::OpenContextMenu);
        assert!(shown(&state).contains(&"Toggle Fold".to_owned()));
        let mut linked = open("- See https://example.com\n");
        linked.update(Message::OpenContextMenu);
        let listed = shown(&linked);
        let panes = listed.iter().rposition(|label| label == "|").unwrap();
        assert_eq!(listed[panes - 1], "Open Link", "last of the view group");
        assert_eq!(
            listed[panes + 1..],
            ["Open in New Pane", "Open Location in New Pane"],
            "the panes group, without Close Pane while there is one pane"
        );
    }

    // FR-KEY-06, FR-OPS-07
    #[test]
    fn opens_on_the_selection_without_the_focused_blocks_own_commands() {
        let mut state = open(TEXT);
        state.update(Message::Select(SelectionChange::ExtendDown));
        state.update(Message::OpenContextMenu);
        let shown = shown(&state);
        assert_eq!(shown[0], "Copy");
        assert!(!shown.contains(&"Edit".to_owned()));
        assert!(!shown.contains(&"Zoom In".to_owned()));
        assert!(
            shown.contains(&"Clear Due Date".to_owned()),
            "the second has one"
        );
    }

    // FR-KEY-06, navigation.md §7
    #[test]
    fn in_the_today_view_it_leaves_out_what_the_view_does_not_take() {
        let mut state = open(TEXT);
        state.update(Message::FocusSidebar);
        state.update(Message::Sidebar(SidebarInput::Choose));
        assert_eq!(state.location(), Location::Today);
        state.update(Message::OpenContextMenu);
        let shown = shown(&state);
        for absent in ["New Block Below", "Paste Below", "Indent", "Group"] {
            assert!(!shown.contains(&absent.to_owned()), "{absent}");
        }
        assert_eq!(shown[..4], ["Edit", "Copy", "Cut", "Duplicate"]);
        assert!(shown.contains(&"Move to…".to_owned()));
        assert!(
            shown.contains(&"Move Up".to_owned()),
            "reorders entries (D-57)"
        );
    }

    // FR-KEY-06
    #[test]
    fn opens_on_the_focused_sidebar_entry_while_the_sidebar_has_the_keys() {
        let mut state = open(TEXT);
        state.update(Message::FocusSidebar);
        state.update(Message::OpenContextMenu);
        assert_eq!(
            shown(&state),
            [
                "Go to Entry",
                "Open in New Pane",
                "|",
                "Return to Outline",
                "Hide Sidebar"
            ]
        );
        assert_eq!(
            menu(&mut state, MenuInput::Choose),
            [Effect::Run(Command::Sidebar(SidebarInput::Choose))]
        );
    }

    #[test]
    fn nothing_opens_without_a_block() {
        let mut state = open("");
        state.update(Message::OpenContextMenu);
        assert_eq!(state.context_menu(), None);
    }

    // FR-KEY-06, keybindings.md §4
    #[test]
    fn typing_filters_enter_runs_the_highlighted_command_and_closes_the_menu() {
        let mut state = open(TEXT);
        state.update(Message::OpenContextMenu);
        for c in "comp".chars() {
            assert_eq!(menu(&mut state, MenuInput::Type(c)), []);
        }
        assert_eq!(shown(&state), ["Complete"]);
        assert_eq!(state.context_menu().unwrap().filter, "comp");
        assert_eq!(
            menu(&mut state, MenuInput::Choose),
            [Effect::Run(Command::ToggleDone)]
        );
        assert_eq!(state.context_menu(), None);
    }

    // FR-KEY-06, keybindings.md §4
    #[test]
    fn what_matches_nothing_is_not_typed_and_esc_closes_the_menu() {
        let mut state = open(TEXT);
        state.update(Message::OpenContextMenu);
        assert_eq!(menu(&mut state, MenuInput::Erase), []);
        assert_eq!(menu(&mut state, MenuInput::Type('q')), []);
        assert_eq!(state.context_menu().unwrap().filter, "");
        menu(&mut state, MenuInput::Type('d'));
        assert_eq!(menu(&mut state, MenuInput::Erase), []);
        assert_eq!(state.context_menu().unwrap().filter, "");
        assert_eq!(menu(&mut state, MenuInput::Dismiss), []);
        assert_eq!(state.context_menu(), None);
        assert_eq!(menu(&mut state, MenuInput::Choose), [], "no menu is open");
    }
}
