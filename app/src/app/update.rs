//! The update function: messages are user intents, effects are what the frontend must do.

use super::State;

/// A user intent, independent of how a frontend receives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    /// Leave the app.
    Quit,
    /// Focus the next visible row.
    FocusNext,
    /// Focus the previous visible row.
    FocusPrevious,
    /// Focus the first visible row.
    FocusFirst,
    /// Focus the last visible row.
    FocusLast,
    /// Move focus down by a page of `rows` rows, the height of the frontend's view.
    PageDown {
        /// Rows in a page.
        rows: usize,
    },
    /// Move focus up by a page of `rows` rows, the height of the frontend's view.
    PageUp {
        /// Rows in a page.
        rows: usize,
    },
}

/// Work the frontend performs for the core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// End the session.
    Quit,
}

impl State {
    /// Applies `message` and returns the effects the frontend must carry out.
    pub fn update(&mut self, message: Message) -> Vec<Effect> {
        match message {
            Message::Quit => return vec![Effect::Quit],
            Message::FocusNext => self.move_focus(|at, last| at.saturating_add(1).min(last)),
            Message::FocusPrevious => self.move_focus(|at, _| at.saturating_sub(1)),
            Message::FocusFirst => self.move_focus(|_, _| 0),
            Message::FocusLast => self.move_focus(|_, last| last),
            Message::PageDown { rows } => {
                self.move_focus(|at, last| at.saturating_add(rows).min(last));
            }
            Message::PageUp { rows } => self.move_focus(|at, _| at.saturating_sub(rows)),
        }
        Vec::new()
    }

    /// Focuses the row at the index `to` picks from the focused index and the last index. Without
    /// a focus there are no rows, so there is nothing to move.
    fn move_focus(&mut self, to: impl FnOnce(usize, usize) -> usize) {
        let rows = self.rows();
        let Some(at) = rows.iter().position(|row| Some(row.id) == self.focus) else {
            return;
        };
        if let Some(row) = rows.get(to(at, rows.len() - 1)) {
            self.focus = Some(row.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Document, format};

    /// Five rows: A, b, c, d, e.
    fn state() -> State {
        State::new(format::parse("A:\n\t- b\n\t\tc\nd\n- e"), None)
    }

    /// The index of the focused row.
    fn focused(state: &State) -> Option<usize> {
        state
            .rows()
            .iter()
            .position(|row| Some(row.id) == state.focus())
    }

    /// Focuses row `index` by moving down from the first row.
    fn at(index: usize) -> State {
        let mut state = state();
        for _ in 0..index {
            state.update(Message::FocusNext);
        }
        assert_eq!(focused(&state), Some(index));
        state
    }

    #[track_caller]
    fn check(from: usize, message: Message, expected: usize) {
        let mut state = at(from);
        assert_eq!(state.update(message), [], "{message:?} has no effects");
        assert_eq!(focused(&state), Some(expected), "{message:?} from {from}");
    }

    #[test]
    fn quit_asks_the_frontend_to_quit() {
        assert_eq!(State::demo().update(Message::Quit), [Effect::Quit]);
    }

    // FR-NAV-01
    #[test]
    fn next_and_previous_move_one_row_and_stop_at_the_ends() {
        check(0, Message::FocusNext, 1);
        check(3, Message::FocusNext, 4);
        check(4, Message::FocusNext, 4);
        check(4, Message::FocusPrevious, 3);
        check(1, Message::FocusPrevious, 0);
        check(0, Message::FocusPrevious, 0);
    }

    // FR-NAV-01
    #[test]
    fn first_and_last_jump_to_the_ends() {
        check(2, Message::FocusFirst, 0);
        check(0, Message::FocusFirst, 0);
        check(2, Message::FocusLast, 4);
        check(4, Message::FocusLast, 4);
    }

    // FR-NAV-01
    #[test]
    fn pages_move_by_the_page_height_and_stop_at_the_ends() {
        check(0, Message::PageDown { rows: 2 }, 2);
        check(1, Message::PageDown { rows: 3 }, 4);
        check(3, Message::PageDown { rows: 2 }, 4);
        check(0, Message::PageDown { rows: usize::MAX }, 4);
        check(4, Message::PageUp { rows: 3 }, 1);
        check(2, Message::PageUp { rows: 3 }, 0);
        check(2, Message::PageDown { rows: 0 }, 2);
        check(2, Message::PageUp { rows: 0 }, 2);
    }

    #[test]
    fn a_single_row_keeps_its_focus() {
        let mut state = State::new(format::parse("- only"), None);
        let only = state.focus();
        assert!(only.is_some());
        for message in [
            Message::FocusNext,
            Message::FocusPrevious,
            Message::FocusLast,
            Message::PageDown { rows: 5 },
        ] {
            state.update(message);
            assert_eq!(state.focus(), only, "{message:?}");
        }
    }

    #[test]
    fn motions_in_an_empty_document_keep_no_focus() {
        let mut state = State::new(Document::default(), None);
        for message in [
            Message::FocusNext,
            Message::FocusPrevious,
            Message::FocusFirst,
            Message::FocusLast,
            Message::PageDown { rows: 5 },
            Message::PageUp { rows: 5 },
        ] {
            assert_eq!(state.update(message), [], "{message:?}");
            assert_eq!(state.focus(), None, "{message:?}");
        }
    }
}
