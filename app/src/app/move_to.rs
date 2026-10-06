//! Move to: a fuzzy finder over the blocks the selected blocks, else the focused block, can move
//! under, ranked as in Go to Symbol, that makes them the chosen block's last children and follows
//! them there ([editing.md §4](../../../docs/specs/editing.md#4-structural-operations-fr-ops),
//! FR-OPS-05).

use super::{
    Access, Location, MenuInput, Notice, State,
    finder::{Finder, FinderView},
    picker::Picked,
};
use crate::document::{Block, ItemId, blocks, structure};

impl State {
    /// Move to, while it is open (FR-OPS-05).
    #[must_use]
    pub fn move_to(&self) -> Option<FinderView<'_, ItemId>> {
        self.destinations.as_ref().map(Finder::view)
    }

    /// `gm`: opens Move to over every block but the ones that would move and what they hold, as
    /// Go to Symbol lists them. A read-only file refuses with a notice.
    pub(super) fn open_move_to(&mut self) {
        let moving = self.roots();
        if moving.is_empty() {
            return;
        }
        if self.access() == Access::ReadOnly {
            self.notify(Notice::ReadOnly);
            return;
        }
        self.destinations = Some(Finder::new(self.block_candidates(&moving)));
    }

    /// Applies `input` to Move to, and once a block is chosen, moves the selected blocks, else
    /// the focused block, there.
    pub(super) fn move_to_input(&mut self, input: MenuInput) {
        let Some(closed) = self
            .destinations
            .as_mut()
            .and_then(|finder| finder.input(input))
        else {
            return;
        };
        self.destinations = None;
        if let Picked::Chosen(id) = closed {
            self.move_selected_to(id);
        }
    }

    /// Makes the selected blocks, else the focused block, the last children of the block `to` as
    /// one step to undo, and shows them there: the focus stays on a block that moves and goes to
    /// the last of them otherwise; `to` and the blocks above it in the zoom expand, and a `to`
    /// outside the zoom becomes the zoom (FR-OPS-05, FR-OPS-07).
    fn move_selected_to(&mut self, to: ItemId) {
        let moving = self.roots();
        let Some(destination) = blocks::block(&self.document, to) else {
            return;
        };
        let Some(ops) = structure::move_to(&self.document, &moving, &destination) else {
            return;
        };
        let inside = |id: ItemId| {
            std::iter::successors(Some(id), |&line| self.document.get(line)?.parent())
                .any(|line| moving.iter().any(|block| block.id() == line))
        };
        let focus = self
            .focus
            .filter(|&id| inside(id))
            .or_else(|| moving.last().map(Block::id));
        self.commit_focusing(ops, focus);
        let above: Vec<ItemId> =
            std::iter::successors(Some(to), |&line| self.document.get(line)?.parent()).collect();
        let zoom = self.zoom();
        if zoom.is_none_or(|zoom| above.contains(&zoom)) {
            for line in above.iter().take_while(|&&line| Some(line) != zoom) {
                self.folds.remove(line);
            }
        } else {
            self.go_to(Location::Outline(Some(to)));
            self.focus = focus;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::{
        app::{Menu, Message, SelectionChange},
        document::format,
    };

    const TEXT: &str = "Home:\n\t- Paint the hall\n\tNotes @bookmark\n---\nWork:\n\t- Write report\n\t\t- Draft\n\tLater @section\n\t\t- Call Ann";

    /// `text` with the focus moved down `index` rows.
    fn open(text: &str, index: usize) -> State {
        let mut state = State::new(format::parse(text), None);
        for _ in 0..index {
            state.update(Message::FocusNext);
        }
        state
    }

    /// Each line as a tab per depth and its text.
    fn lines(state: &State) -> Vec<String> {
        let document = state.document();
        document
            .descendants(document.root())
            .map(|(id, depth)| {
                format!("{}{}", "\t".repeat(depth), document.get(id).unwrap().text())
            })
            .collect()
    }

    /// The text of the block `id`.
    fn text(state: &State, id: Option<ItemId>) -> String {
        state.document().get(id.unwrap()).unwrap().display_text()
    }

    /// The text of each entry Move to lists.
    fn listed(state: &State) -> Vec<String> {
        let view = state.move_to().unwrap();
        view.entries
            .iter()
            .map(|entry| entry.text.clone())
            .collect()
    }

    /// Types `query` into Move to and chooses the first match.
    fn choose(state: &mut State, query: &str) {
        for c in query.chars() {
            state.update(Message::Menu(Menu::MoveTo, MenuInput::Type(c)));
        }
        state.update(Message::Menu(Menu::MoveTo, MenuInput::Choose));
    }

    // FR-OPS-05, navigation.md §8
    #[test]
    fn lists_every_other_block_ranked_as_go_to_symbol_but_the_moved_one_and_what_it_holds() {
        let mut state = open(TEXT, 5);
        assert_eq!(state.move_to(), None);
        state.update(Message::OpenMoveTo);
        assert_eq!(
            listed(&state),
            [
                "Notes",
                "Home",
                "Work",
                "Later",
                "Paint the hall",
                "Call Ann"
            ]
        );
        let view = state.move_to().unwrap();
        assert_eq!(view.entries[3].detail, "Work", "with its path");
    }

    // FR-OPS-05, FR-DONE-02
    #[test]
    fn hidden_completed_blocks_are_not_listed() {
        let mut state = open("A:\n\t- done @done\n\t\t- inside\n\t- open\nB:", 0);
        state.update(Message::ToggleShowDone);
        state.update(Message::FocusLast);
        state.update(Message::OpenMoveTo);
        assert_eq!(listed(&state), ["A", "open"]);
    }

    // FR-OPS-05, FR-OPS-08
    #[test]
    fn choosing_a_block_makes_the_focused_block_its_last_child_in_one_undo_step() {
        let mut state = open(TEXT, 5);
        state.update(Message::OpenMoveTo);
        choose(&mut state, "home");
        assert_eq!(state.move_to(), None);
        assert_eq!(
            lines(&state),
            [
                "Home:",
                "\t- Paint the hall",
                "\tNotes @bookmark",
                "\t- Write report",
                "\t\t- Draft",
                "---",
                "Work:",
                "\tLater @section",
                "\t\t- Call Ann",
            ]
        );
        assert_eq!(text(&state, state.focus()), "Write report");
        state.update(Message::Undo);
        assert_eq!(lines(&state), lines(&open(TEXT, 0)));
        assert_eq!(text(&state, state.focus()), "Write report");
        state.update(Message::Redo);
        assert_eq!(lines(&state)[3], "\t- Write report");
    }

    // FR-OPS-05, FR-OPS-07
    #[test]
    fn the_selection_moves_in_order_and_the_focus_goes_with_it() {
        let mut state = open(TEXT, 1);
        state.update(Message::Select(SelectionChange::Toggle));
        state.update(Message::FocusLast);
        state.update(Message::Select(SelectionChange::Toggle));
        state.update(Message::FocusFirst);
        state.update(Message::OpenMoveTo);
        assert!(!listed(&state).contains(&"Call Ann".to_owned()));
        choose(&mut state, "report");
        assert_eq!(
            lines(&state)[4..],
            [
                "\t- Write report",
                "\t\t- Draft",
                "\t\t- Paint the hall",
                "\t\t- Call Ann",
                "\tLater @section",
            ]
        );
        assert_eq!(
            text(&state, state.focus()),
            "Call Ann",
            "the last moved block, as the focus was not one of them"
        );
        let mut state = open(TEXT, 1);
        state.update(Message::Select(SelectionChange::Toggle));
        state.update(Message::FocusLast);
        state.update(Message::Select(SelectionChange::Toggle));
        state.update(Message::FocusFirst);
        state.update(Message::FocusNext);
        state.update(Message::OpenMoveTo);
        choose(&mut state, "report");
        assert_eq!(
            text(&state, state.focus()),
            "Paint the hall",
            "the focus moved"
        );
    }

    // FR-OPS-05, FR-NAV-02
    #[test]
    fn a_collapsed_destination_and_the_blocks_above_it_expand() {
        let mut state = open("A:\n\tB:\n\t\t- b\n- x", 1);
        state.update(Message::Collapse);
        state.update(Message::FocusParent);
        state.update(Message::Collapse);
        state.update(Message::FocusNext);
        state.update(Message::OpenMoveTo);
        choose(&mut state, "B");
        let rows: Vec<String> = state.rows().into_iter().map(|row| row.text).collect();
        assert_eq!(rows, ["A", "B", "b", "x"]);
        assert_eq!(text(&state, state.focus()), "x");
    }

    // FR-OPS-05, FR-NAV-03
    #[test]
    fn a_destination_inside_the_zoom_keeps_it_and_one_outside_becomes_it() {
        let mut state = open(TEXT, 4);
        state.update(Message::ZoomIn);
        state.update(Message::FocusLast);
        state.update(Message::OpenMoveTo);
        choose(&mut state, "report");
        assert_eq!(text(&state, state.zoom()), "Work");
        assert_eq!(text(&state, state.focus()), "Call Ann");
        state.update(Message::OpenMoveTo);
        choose(&mut state, "Home");
        assert_eq!(text(&state, state.zoom()), "Home");
        assert_eq!(text(&state, state.focus()), "Call Ann");
        assert_eq!(state.rows().last().unwrap().text, "Call Ann");
        state.update(Message::Back);
        assert_eq!(text(&state, state.zoom()), "Work", "a step in the history");
    }

    // FR-OPS-05
    #[test]
    fn the_zoom_title_can_be_the_destination_but_does_not_move() {
        let mut state = open(TEXT, 4);
        state.update(Message::ZoomIn);
        state.update(Message::FocusFirst);
        state.update(Message::OpenMoveTo);
        assert_eq!(state.move_to(), None, "nothing to move");
        state.update(Message::FocusNext);
        state.update(Message::FocusNext);
        state.update(Message::OpenMoveTo);
        choose(&mut state, "work");
        assert_eq!(
            lines(&state)[5..8],
            ["\t- Write report", "\tLater @section", "\t\t- Call Ann"]
        );
        assert_eq!(lines(&state)[8], "\t- Draft");
        assert_eq!(text(&state, state.zoom()), "Work");
    }

    #[test]
    fn dismissing_or_choosing_nothing_moves_nothing() {
        let mut state = open(TEXT, 5);
        state.update(Message::OpenMoveTo);
        state.update(Message::Menu(Menu::MoveTo, MenuInput::Dismiss));
        assert_eq!(state.move_to(), None);
        state.update(Message::OpenMoveTo);
        choose(&mut state, "zzz");
        assert!(state.move_to().is_some(), "nothing to choose keeps it open");
        assert_eq!(lines(&state), lines(&open(TEXT, 0)));
        assert_eq!(state.document().revision(), 0);
    }

    #[test]
    fn keys_without_move_to_open_do_nothing() {
        let mut state = open(TEXT, 5);
        state.update(Message::Menu(Menu::MoveTo, MenuInput::Choose));
        assert_eq!(state.document().revision(), 0);
    }

    // file-io.md §1
    #[test]
    fn a_read_only_file_refuses_to_open_it() {
        let mut state = State::new(
            format::parse(TEXT),
            Some(PathBuf::from("/notes/todo.taskpaper")),
        )
        .with_access(Access::ReadOnly);
        state.update(Message::OpenMoveTo);
        assert_eq!(state.move_to(), None);
        assert_eq!(state.notice(), Some(&Notice::ReadOnly));
    }

    #[test]
    fn a_destination_inside_what_moves_once_the_focus_changed_is_not_moved_to() {
        let mut state = open(TEXT, 5);
        state.update(Message::OpenMoveTo);
        state.update(Message::FocusPrevious);
        choose(&mut state, "later");
        assert_eq!(state.move_to(), None);
        assert_eq!(state.document().revision(), 0);
    }

    #[test]
    fn a_destination_deleted_while_listed_is_not_moved_to() {
        let mut state = open(TEXT, 5);
        state.update(Message::OpenMoveTo);
        for c in "ann".chars() {
            state.update(Message::Menu(Menu::MoveTo, MenuInput::Type(c)));
        }
        state.document = format::parse("Other:\n\t- Task");
        state.update(Message::Menu(Menu::MoveTo, MenuInput::Choose));
        assert_eq!(lines(&state), ["Other:", "\t- Task"]);
    }
}
