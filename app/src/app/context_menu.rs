//! The context menu: the commands that apply where the keys are, on the focused Navigator entry,
//! the selection or the focused block, or on the focused pane's header or one of its breadcrumbs,
//! grouped, to filter and choose from
//! ([ui.md §5](../../../docs/specs/ui.md#5-context-menu-fr-key-06),
//! [keybindings.md §4](../../../docs/specs/keybindings.md#4-other-contexts), FR-KEY-06).

use super::{
    Command, CommandMenuView, Effect, Inputs, Location, MenuInput, PaneChange, State,
    command_menu::{CommandMenu, Outcome},
    commands::{Focused, NodeCommand, context_commands},
};

impl State {
    /// The context menu, while it is open (FR-KEY-06).
    #[must_use]
    pub fn context_menu(&self) -> Option<CommandMenuView<'_>> {
        self.context_menu.as_ref().map(CommandMenu::view)
    }

    /// `\`: opens the context menu on the focused Navigator entry while the Navigator is open,
    /// else on the selected blocks, else on the focused block. In the Today view it leaves out
    /// what the view does not take (navigation.md §7). Nothing opens without anything to offer.
    pub(super) fn open_context_menu(&mut self) {
        let groups = if self.navigator_focused() {
            self.navigator_commands().unwrap_or_default()
        } else {
            self.block_commands()
        };
        self.context_menu = (!groups.is_empty()).then(|| CommandMenu::new(groups));
    }

    /// Opens the context menu on the focused pane's header, or with a depth on its breadcrumb at
    /// that depth: going to the breadcrumb unless the pane is there and opening it in a new pane,
    /// or on the header zooming out and going home where the pane can; then opening the pane's
    /// location in a new pane and closing the pane when there are others (ui.md §5).
    pub(super) fn open_pane_menu(&mut self, crumb: Option<usize>) {
        let here = self.pane.location;
        let place: Vec<Command> = match crumb {
            Some(depth) => match self.crumb(depth) {
                Some(location) => (location != here)
                    .then_some(if depth == 0 {
                        Command::GoHome
                    } else {
                        Command::GoToCrumb(depth)
                    })
                    .into_iter()
                    .chain([Command::Pane(PaneChange::OpenCrumb(depth))])
                    .collect(),
                None => return,
            },
            None => self
                .zoom()
                .map(|_| Command::ZoomOut)
                .into_iter()
                .chain((here != Location::Outline(None)).then_some(Command::GoHome))
                .collect(),
        };
        let place: Vec<NodeCommand> = place.into_iter().map(NodeCommand::named).collect();
        let groups = [place, self.pane_commands(false)]
            .into_iter()
            .filter(|group| !group.is_empty())
            .collect();
        self.context_menu = Some(CommandMenu::new(groups));
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
            page_down_rows: 0,
            page_up_rows: 0,
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
        app::{
            Command, Location, Menu, MenuRow, Message, NavigatorInput, PaneChange, SelectionChange,
            SpecialView,
        },
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
        state.update(Message::ToggleNavigator);
        state.update(Message::Navigator(NavigatorInput::Choose));
        assert_eq!(state.location(), Location::View(SpecialView::Today));
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
    fn opens_on_the_focused_navigator_entry_while_the_navigator_is_open() {
        let mut state = open(TEXT);
        state.update(Message::ToggleNavigator);
        state.update(Message::OpenContextMenu);
        assert_eq!(
            shown(&state),
            ["Go to Entry", "Open in New Pane", "|", "Return to Outline"]
        );
        assert_eq!(
            menu(&mut state, MenuInput::Choose),
            [Effect::Run(Command::Navigator(NavigatorInput::Choose))]
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

    /// Opens the menu of the focused pane's header, or of its breadcrumb at `crumb`, and returns
    /// what it lists, or `None` when nothing opened.
    fn pane_menu(state: &mut State, crumb: Option<usize>) -> Option<Vec<String>> {
        state.update(Message::Menu(Menu::ContextMenu, MenuInput::Dismiss));
        state.update(Message::OpenPaneMenu(crumb));
        state.context_menu().map(|_| shown(state))
    }

    // FR-KEY-06, ui.md §5
    #[test]
    fn a_pane_header_offers_zooming_out_going_home_and_the_panes() {
        let mut state = open(TEXT);
        assert_eq!(
            pane_menu(&mut state, None).expect("a menu"),
            ["Open Location in New Pane"]
        );
        state.update(Message::ZoomIn);
        state.update(Message::Pane(PaneChange::OpenLocation));
        assert_eq!(
            pane_menu(&mut state, None).expect("a menu"),
            [
                "Zoom Out",
                "Go Home",
                "|",
                "Open Location in New Pane",
                "Close Pane"
            ]
        );
        state.update(Message::ToggleNavigator);
        state.update(Message::Navigator(NavigatorInput::Focus(0)));
        state.update(Message::Navigator(NavigatorInput::Choose));
        assert_eq!(state.pane.location, Location::View(SpecialView::Today));
        assert_eq!(
            pane_menu(&mut state, None).expect("a menu"),
            ["Go Home", "|", "Open Location in New Pane", "Close Pane"]
        );
    }

    // FR-KEY-06, ui.md §5
    #[test]
    fn a_crumb_offers_going_there_unless_the_pane_is_there_and_opening_it_in_a_new_pane() {
        let mut state = open(TEXT);
        state.update(Message::ZoomIn);
        assert_eq!(
            pane_menu(&mut state, Some(0)).expect("a menu"),
            [
                "Go Home",
                "Open in New Pane",
                "|",
                "Open Location in New Pane"
            ]
        );
        assert_eq!(
            pane_menu(&mut state, Some(1)).expect("a menu"),
            ["Open in New Pane", "|", "Open Location in New Pane"]
        );
        assert_eq!(pane_menu(&mut state, Some(2)), None, "no crumb that deep");
        state.update(Message::FocusNext);
        state.update(Message::ZoomIn);
        pane_menu(&mut state, Some(1));
        assert_eq!(
            menu(&mut state, MenuInput::Choose),
            [Effect::Run(Command::GoToCrumb(1))]
        );
        assert_eq!(state.context_menu(), None);
    }
}
