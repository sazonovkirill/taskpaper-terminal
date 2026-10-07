//! The order the user gives the Today view's entries, which lives in the app and the kept layout
//! and never in the document, so reordering leaves the file as it is
//! ([navigation.md §7](../../../docs/specs/navigation.md#7-today-view-fr-tag-04-d-7), D-57).

use std::collections::{HashMap, HashSet};

use super::State;
use crate::document::{Block, ItemId};

/// The blocks the user placed in the Today view, in the order they were placed; entries never
/// placed come after them, in document order. A block keeps its place while it is out of the
/// list, so it returns there.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct TodayOrder {
    placed: Vec<ItemId>,
}

impl TodayOrder {
    /// The order placing `placed` in turn, each block once.
    pub(super) fn new(placed: impl IntoIterator<Item = ItemId>) -> Self {
        let mut seen = HashSet::new();
        Self {
            placed: placed.into_iter().filter(|&id| seen.insert(id)).collect(),
        }
    }

    /// The placed blocks in order.
    pub(super) fn placed(&self) -> &[ItemId] {
        &self.placed
    }

    /// Sorts `entries`, given in document order, into this order: the placed ones in their order,
    /// then the others as they were.
    pub(super) fn arrange<T>(&self, entries: &mut [T], id: impl Fn(&T) -> ItemId) {
        let places: HashMap<ItemId, usize> = self
            .placed
            .iter()
            .enumerate()
            .map(|(at, &id)| (id, at))
            .collect();
        entries.sort_by_key(|entry| places.get(&id(entry)).copied().unwrap_or(usize::MAX));
    }

    /// Moves each run of the `moving` entries among the `shown` ones a step up, or down with
    /// `down`, past the entry next to it, and places every entry; a run at the end it moves
    /// toward stays, and so does one that would pass a run that stayed. `entries` are all the
    /// entries in this order, `shown` those of them a pane shows, in the same order; an entry
    /// not shown keeps its place among them. When no entry moves, nothing is placed.
    pub(super) fn shift(
        &mut self,
        entries: &[ItemId],
        shown: &[ItemId],
        moving: &[ItemId],
        down: bool,
    ) {
        let mut moved = shown.to_vec();
        if down {
            moved.reverse();
        }
        let mut any = false;
        for at in 1..moved.len() {
            let pair = (moved.get(at - 1), moved.get(at));
            if let (Some(before), Some(this)) = pair
                && moving.contains(this)
                && !moving.contains(before)
            {
                moved.swap(at - 1, at);
                any = true;
            }
        }
        if !any {
            return;
        }
        if down {
            moved.reverse();
        }
        let unplaced: Vec<ItemId> = entries
            .iter()
            .filter(|id| !self.placed.contains(id))
            .copied()
            .collect();
        self.placed.extend(unplaced);
        let mut moved = moved.into_iter();
        for slot in &mut self.placed {
            if shown.contains(slot)
                && let Some(id) = moved.next()
            {
                *slot = id;
            }
        }
    }

    /// Keeps each placed block's place on the block `rematch` finds for it in a reloaded
    /// document, and drops it when it finds none (file-io.md §3).
    pub(super) fn rematch(&mut self, rematch: impl Fn(ItemId) -> Option<ItemId>) {
        *self = Self::new(self.placed.iter().filter_map(|&id| rematch(id)));
    }
}

impl State {
    /// Moves the selected entries, else the focused one, a step up or down in the Today view's
    /// order, which every pane showing the view shares, leaving the document as it is and making
    /// no step to undo.
    pub(super) fn reorder_entries(&mut self, down: bool) {
        let shown: Vec<ItemId> = self.rows().iter().map(|row| row.id).collect();
        let moving: Vec<ItemId> = self.targets().iter().map(Block::id).collect();
        let mut every = self.pane.clone();
        every.show_done = true;
        let entries: Vec<ItemId> = self
            .today_rows(&every)
            .into_iter()
            .map(|row| row.id)
            .collect();
        self.today_order.shift(&entries, &shown, &moving, down);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use jiff::civil::date;

    use super::*;
    use crate::{
        app::{Location, Message, PaneChange, SelectionChange},
        document::{Flag, blocks, format},
    };

    /// The ids of five blocks, standing for the letters a to e.
    fn five() -> Vec<ItemId> {
        let document = format::parse("a\nb\nc\nd\ne\n");
        blocks::children(&document, document.root())
            .iter()
            .map(Block::id)
            .collect()
    }

    /// The ids of `ids` that `letters` name.
    fn of(ids: &[ItemId], letters: &str) -> Vec<ItemId> {
        letters
            .bytes()
            .map(|letter| ids[usize::from(letter - b'a')])
            .collect()
    }

    /// The letters `order` arranges the entries `letters` of `ids` into.
    fn arranged(ids: &[ItemId], order: &TodayOrder, letters: &str) -> String {
        let mut entries = of(ids, letters);
        order.arrange(&mut entries, |&id| id);
        entries
            .iter()
            .map(|id| {
                let at = ids.iter().position(|x| x == id).expect("one of the five");
                char::from(b'a' + u8::try_from(at).expect("five"))
            })
            .collect()
    }

    #[test]
    fn placed_blocks_come_first_in_their_order_then_the_others_in_document_order() {
        let ids = five();
        assert_eq!(arranged(&ids, &TodayOrder::default(), "abcde"), "abcde");
        let order = TodayOrder::new(of(&ids, "dbd"));
        assert_eq!(order.placed(), of(&ids, "db"), "each block once");
        assert_eq!(arranged(&ids, &order, "abcde"), "dbace");
        assert_eq!(arranged(&ids, &order, "ace"), "ace");
    }

    /// The five entries in their order after moving `moving` among `shown`, down with `down`,
    /// in the order placing `placed`.
    fn shifted(placed: &str, shown: &str, moving: &str, down: bool) -> String {
        let ids = five();
        let mut order = TodayOrder::new(of(&ids, placed));
        let mut entries = of(&ids, "abcde");
        order.arrange(&mut entries, |&id| id);
        let mut visible = of(&ids, shown);
        order.arrange(&mut visible, |&id| id);
        order.shift(&entries, &visible, &of(&ids, moving), down);
        arranged(&ids, &order, "abcde")
    }

    #[test]
    fn an_entry_moves_past_its_neighbor_and_every_entry_is_placed() {
        assert_eq!(shifted("", "abcde", "c", false), "acbde");
        assert_eq!(shifted("", "abcde", "c", true), "abdce");
        assert_eq!(shifted("ed", "abcde", "d", false), "deabc");
        assert_eq!(shifted("ed", "abcde", "a", true), "edbac");
        let ids = five();
        let mut order = TodayOrder::default();
        order.shift(&ids, &ids, &of(&ids, "c"), false);
        assert_eq!(order.placed(), of(&ids, "acbde"));
    }

    #[test]
    fn an_entry_at_the_end_it_moves_toward_stays_unplaced() {
        assert_eq!(shifted("", "abcde", "a", false), "abcde");
        assert_eq!(shifted("", "abcde", "e", true), "abcde");
        assert_eq!(shifted("ed", "abcde", "e", false), "edabc");
        assert_eq!(shifted("", "abcde", "", true), "abcde");
        let ids = five();
        let mut order = TodayOrder::default();
        order.shift(&ids, &ids, &of(&ids, "a"), false);
        assert_eq!(order.placed(), [], "nothing placed");
    }

    #[test]
    fn runs_move_together_and_a_run_that_stayed_holds_the_next() {
        assert_eq!(shifted("", "abcde", "cd", false), "acdbe");
        assert_eq!(shifted("", "abcde", "bd", false), "badce");
        assert_eq!(shifted("", "abcde", "abd", false), "abdce");
        assert_eq!(shifted("", "abcde", "bde", true), "acbde");
        assert_eq!(shifted("", "abcde", "ab", false), "abcde");
    }

    #[test]
    fn entries_not_shown_keep_their_places() {
        assert_eq!(shifted("", "ace", "c", false), "cbade");
        assert_eq!(shifted("", "ace", "e", false), "abedc");
        assert_eq!(shifted("", "ace", "a", true), "cbade");
    }

    #[test]
    fn rematching_keeps_the_places_of_the_blocks_it_finds() {
        let ids = five();
        let mut order = TodayOrder::new(of(&ids, "cab"));
        order.rematch(|id| (id != ids[0]).then_some(id));
        assert_eq!(order.placed(), of(&ids, "cb"));
        order.rematch(|id| (id == ids[2]).then_some(ids[1]));
        assert_eq!(order.placed(), of(&ids, "b"), "each block once");
    }

    /// `text` opened on 5 October 2026 in the Today view.
    fn today(text: &str) -> State {
        let mut state = State::new(format::parse(text), None);
        state.update(Message::Tick {
            now: Duration::ZERO,
            today: date(2026, 10, 5),
        });
        state.go_to(Location::Today);
        state
    }

    /// The text of each row.
    fn listed(state: &State) -> Vec<String> {
        state
            .rows()
            .iter()
            .map(|row| row.text.to_string())
            .collect()
    }

    /// The text of the focused block.
    fn focused(state: &State) -> String {
        let id = state.focus().expect("a focus");
        state
            .rows()
            .iter()
            .find(|row| row.id == id)
            .expect("shown")
            .text
            .to_string()
    }

    // D-57, navigation §7
    #[test]
    fn moving_an_entry_reorders_the_list_and_leaves_the_document_as_it_is() {
        let text = "P:\n\t- a @today\n\t- b @today\n- c @due(2026-10-01)\n- x\n";
        let mut state = today(text);
        let revision = state.document().revision();
        state.update(Message::FocusLast);
        assert_eq!(state.update(Message::MoveUp), []);
        assert_eq!(listed(&state), ["a", "c", "b"]);
        state.update(Message::MoveUp);
        assert_eq!(listed(&state), ["c", "a", "b"]);
        assert_eq!(focused(&state), "c", "the focus moves with the entry");
        state.update(Message::MoveDown);
        assert_eq!(listed(&state), ["a", "c", "b"]);
        assert_eq!(format::serialize(state.document()), text);
        assert_eq!(state.document().revision(), revision, "nothing to save");
        state.update(Message::Undo);
        assert_eq!(listed(&state), ["a", "c", "b"], "not a step to undo");
        state.update(Message::GoTo(Location::Outline(None)));
        assert_eq!(listed(&state), ["P", "a", "b", "c", "x"], "the outline");
    }

    // D-57, FR-OPS-01
    #[test]
    fn selected_entries_move_together() {
        let mut state = today("- a @today\n- b @today\n- c @today\n- d @today\n");
        state.update(Message::FocusNext);
        state.update(Message::Select(SelectionChange::ExtendDown));
        state.update(Message::MoveDown);
        assert_eq!(listed(&state), ["a", "d", "b", "c"]);
        state.update(Message::MoveDown);
        assert_eq!(listed(&state), ["a", "d", "b", "c"], "nowhere to go");
        assert_eq!(focused(&state), "c");
    }

    // D-57, D-13, FR-DONE-02
    #[test]
    fn panes_share_the_order_and_an_entry_hidden_in_one_keeps_its_place() {
        let mut state = today("- a @today\n- b @today @done\n- c @today\n- d\n");
        state.update(Message::Pane(PaneChange::OpenLocation));
        state.update(Message::ToggleShowDone);
        assert_eq!(listed(&state), ["a", "c"]);
        state.update(Message::FocusLast);
        state.update(Message::MoveUp);
        assert_eq!(listed(&state), ["c", "a"]);
        state.update(Message::Pane(PaneChange::FocusLeft));
        assert_eq!(listed(&state), ["c", "b", "a"]);
    }

    // D-57
    #[test]
    fn new_entries_come_last_and_an_entry_that_returns_keeps_its_place() {
        let mut state = today("- a @today\n- b @today\n- c @today\n- d\n");
        state.update(Message::FocusLast);
        state.update(Message::MoveUp);
        assert_eq!(listed(&state), ["a", "c", "b"]);
        state.update(Message::FocusFirst);
        state.update(Message::ToggleFlag(Flag::Today));
        assert_eq!(listed(&state), ["c", "b"]);
        state.update(Message::GoTo(Location::Outline(None)));
        for _ in 0..3 {
            state.update(Message::FocusNext);
        }
        state.update(Message::ToggleFlag(Flag::Today));
        state.update(Message::FocusFirst);
        state.update(Message::ToggleFlag(Flag::Today));
        state.go_to(Location::Today);
        assert_eq!(listed(&state), ["a", "c", "b", "d"]);
    }
}
