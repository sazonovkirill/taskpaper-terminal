//! The update function: messages are user intents, effects are what the frontend must do.

use super::State;
use crate::document::{Block, Item, blocks};

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
    /// Focus the parent block.
    FocusParent,
    /// Focus the next sibling block.
    FocusNextSibling,
    /// Focus the previous sibling block.
    FocusPreviousSibling,
    /// Collapse the focused block; if it is a leaf or already collapsed, focus its parent.
    Collapse,
    /// Expand the focused block; if it is already expanded, focus its first child.
    Expand,
    /// Expand the focused block if it is collapsed, else collapse it.
    ToggleFold,
    /// Collapse the focused block's whole subtree if any of it is expanded, else expand it all.
    ToggleSubtree,
    /// Expand every collapsed block at the shallowest depth that has one.
    ExpandLevel,
    /// Collapse every visible expanded block at the deepest depth that has one.
    CollapseLevel,
    /// Expand every block.
    ExpandAll,
    /// Collapse every block.
    CollapseAll,
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
            Message::FocusParent => self.focus_parent(),
            Message::FocusNextSibling => self.move_to_sibling(|at| at.checked_add(1)),
            Message::FocusPreviousSibling => self.move_to_sibling(|at| at.checked_sub(1)),
            Message::Collapse => self.collapse(),
            Message::Expand => self.expand(),
            Message::ToggleFold => self.toggle_fold(),
            Message::ToggleSubtree => self.toggle_subtree(),
            Message::ExpandLevel => self.expand_level(),
            Message::CollapseLevel => self.collapse_level(),
            Message::ExpandAll => self.folds.clear(),
            Message::CollapseAll => {
                let all = self.blocks(false).into_iter().map(|(block, _)| block);
                self.fold(all);
            }
        }
        self.repair_focus();
        Vec::new()
    }

    /// The focused block.
    fn focused_block(&self) -> Option<Block> {
        blocks::block(self.document(), self.focus?)
    }

    fn focus_parent(&mut self) {
        if let Some(parent) = self
            .focus
            .and_then(|id| blocks::parent(self.document(), id))
        {
            self.focus = Some(parent.id());
        }
    }

    /// Focuses the sibling at the index `to` picks from the focused block's index among its
    /// siblings, if there is one.
    fn move_to_sibling(&mut self, to: impl FnOnce(usize) -> Option<usize>) {
        let Some(id) = self.focus else { return };
        let siblings = self
            .document()
            .get(id)
            .and_then(Item::parent)
            .map(|line| blocks::children(self.document(), line))
            .unwrap_or_default();
        if let Some(at) = siblings.iter().position(|block| block.id() == id)
            && let Some(sibling) = to(at).and_then(|index| siblings.get(index))
        {
            self.focus = Some(sibling.id());
        }
    }

    fn collapse(&mut self) {
        let Some(block) = self.focused_block() else {
            return;
        };
        if self.is_expanded(&block) {
            self.folds.insert(block.id());
        } else {
            self.focus_parent();
        }
    }

    fn expand(&mut self) {
        let Some(block) = self.focused_block() else {
            return;
        };
        if !self.folds.remove(&block.id())
            && let Some(child) = self.children(&block).first()
        {
            self.focus = Some(child.id());
        }
    }

    fn toggle_fold(&mut self) {
        let Some(block) = self.focused_block() else {
            return;
        };
        if !self.folds.remove(&block.id()) {
            self.fold([block]);
        }
    }

    fn toggle_subtree(&mut self) {
        let Some(block) = self.focused_block() else {
            return;
        };
        let mut subtree = vec![block.clone()];
        self.push_descendants(&block, &mut subtree);
        if subtree.iter().any(|block| self.is_expanded(block)) {
            self.fold(subtree);
        } else {
            for block in &subtree {
                self.folds.remove(&block.id());
            }
        }
    }

    /// Appends every descendant block of `block`, collapsed or not, to `out`.
    fn push_descendants(&self, block: &Block, out: &mut Vec<Block>) {
        for child in self.children(block) {
            self.push_descendants(&child, out);
            out.push(child);
        }
    }

    fn expand_level(&mut self) {
        let collapsed: Vec<(Block, usize)> = self
            .blocks(false)
            .into_iter()
            .filter(|(block, _)| {
                self.folds.contains(&block.id()) && !self.children(block).is_empty()
            })
            .collect();
        let Some(depth) = collapsed.iter().map(|&(_, depth)| depth).min() else {
            return;
        };
        for (block, _) in collapsed.iter().filter(|&&(_, d)| d == depth) {
            self.folds.remove(&block.id());
        }
    }

    fn collapse_level(&mut self) {
        let expanded: Vec<(Block, usize)> = self
            .blocks(true)
            .into_iter()
            .filter(|(block, _)| self.is_expanded(block))
            .collect();
        let Some(depth) = expanded.iter().map(|&(_, depth)| depth).max() else {
            return;
        };
        let deepest = expanded
            .into_iter()
            .filter(|&(_, d)| d == depth)
            .map(|(block, _)| block);
        self.fold(deepest);
    }

    /// Whether `block` has children and is not collapsed.
    fn is_expanded(&self, block: &Block) -> bool {
        !self.folds.contains(&block.id()) && !self.children(block).is_empty()
    }

    /// Collapses each of `blocks` that has children; a leaf has nothing to fold.
    fn fold(&mut self, blocks: impl IntoIterator<Item = Block>) {
        for block in blocks {
            if !self.children(&block).is_empty() {
                self.folds.insert(block.id());
            }
        }
    }

    /// Moves focus off a block a fold hid: to the next visible block in document order, else
    /// the previous one (navigation §2). A hidden block always has a visible ancestor before
    /// it, so one of the two exists.
    fn repair_focus(&mut self) {
        let Some(focus) = self.focus else { return };
        let visible: Vec<_> = self.rows().into_iter().map(|row| row.id).collect();
        if visible.contains(&focus) {
            return;
        }
        let order: Vec<_> = self
            .blocks(false)
            .into_iter()
            .map(|(block, _)| block.id())
            .collect();
        let at = order.iter().position(|&id| id == focus).unwrap_or_default();
        self.focus = order
            .iter()
            .skip(at)
            .chain(order.iter().take(at).rev())
            .find(|id| visible.contains(id))
            .copied();
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
    use std::collections::HashSet;

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

    /// Each visible row as its depth, display text and hidden-children count.
    fn visible(state: &State) -> Vec<(usize, String, usize)> {
        state
            .rows()
            .into_iter()
            .map(|row| (row.depth, row.lines.join("|"), row.hidden_children))
            .collect()
    }

    /// The display text of the focused row.
    fn focus_text(state: &State) -> String {
        let rows = state.rows();
        let row = rows.iter().find(|row| Some(row.id) == state.focus());
        row.map(|row| row.lines.join("|")).unwrap_or_default()
    }

    /// `text` with focus moved down `index` rows.
    fn open(text: &str, index: usize) -> State {
        let mut state = State::new(format::parse(text), None);
        for _ in 0..index {
            state.update(Message::FocusNext);
        }
        state
    }

    /// Three levels: A > b > c, A > d, then e.
    const TREE: &str = "A:\n\t- b\n\t\t- c\n\t- d\n- e";

    // FR-NAV-02
    #[test]
    fn collapse_hides_children_and_counts_them() {
        let mut state = open(TREE, 0);
        state.update(Message::Collapse);
        assert_eq!(
            visible(&state),
            [(0, "A".to_owned(), 2), (0, "e".to_owned(), 0)]
        );
        assert_eq!(focus_text(&state), "A");
    }

    // FR-NAV-01, FR-NAV-02
    #[test]
    fn collapse_on_a_leaf_or_collapsed_block_focuses_the_parent() {
        let mut state = open(TREE, 2);
        state.update(Message::Collapse);
        assert_eq!(focus_text(&state), "b");
        state.update(Message::Collapse);
        assert_eq!(focus_text(&state), "b");
        state.update(Message::Collapse);
        assert_eq!(focus_text(&state), "A");
        let mut top = open(TREE, 4);
        top.update(Message::Collapse);
        assert_eq!(focus_text(&top), "e", "a top-level leaf has no parent");
    }

    // FR-NAV-01, FR-NAV-02
    #[test]
    fn expand_opens_then_focuses_the_first_child() {
        let mut state = open(TREE, 0);
        state.update(Message::Collapse);
        state.update(Message::Expand);
        assert_eq!(visible(&state).len(), 5);
        assert_eq!(focus_text(&state), "A");
        state.update(Message::Expand);
        assert_eq!(focus_text(&state), "b");
        let mut leaf = open(TREE, 4);
        leaf.update(Message::Expand);
        assert_eq!(focus_text(&leaf), "e");
    }

    // FR-NAV-02
    #[test]
    fn toggle_fold_flips_a_block_and_ignores_leaves() {
        let mut state = open(TREE, 1);
        state.update(Message::ToggleFold);
        assert_eq!(visible(&state)[1], (1, "b".to_owned(), 1));
        state.update(Message::ToggleFold);
        assert_eq!(visible(&state)[1], (1, "b".to_owned(), 0));
        let mut leaf = open(TREE, 4);
        leaf.update(Message::ToggleFold);
        assert_eq!(leaf.folds, HashSet::new());
    }

    // FR-NAV-02
    #[test]
    fn toggle_subtree_collapses_any_expanded_part_else_expands_all() {
        let mut state = open(TREE, 0);
        state.update(Message::ToggleSubtree);
        assert_eq!(state.folds.len(), 2, "A and b, not the leaves");
        state.update(Message::Expand);
        assert_eq!(visible(&state)[1], (1, "b".to_owned(), 1));
        state.update(Message::ToggleSubtree);
        assert_eq!(state.folds.len(), 2);
        state.update(Message::ToggleSubtree);
        assert_eq!(visible(&state).len(), 5);
        assert_eq!(state.folds, HashSet::new());
    }

    // FR-NAV-02
    #[test]
    fn levels_expand_the_shallowest_and_collapse_the_deepest() {
        let mut state = open(TREE, 0);
        state.update(Message::CollapseLevel);
        assert_eq!(visible(&state)[1], (1, "b".to_owned(), 1));
        assert_eq!(visible(&state)[0], (0, "A".to_owned(), 0));
        state.update(Message::CollapseLevel);
        assert_eq!(visible(&state)[0], (0, "A".to_owned(), 2));
        state.update(Message::CollapseLevel);
        assert_eq!(state.folds.len(), 2, "nothing left to collapse");
        state.update(Message::ExpandLevel);
        assert_eq!(visible(&state)[1], (1, "b".to_owned(), 1));
        state.update(Message::ExpandLevel);
        assert_eq!(visible(&state).len(), 5);
        state.update(Message::ExpandLevel);
        assert_eq!(state.folds, HashSet::new());
    }

    // FR-NAV-02
    #[test]
    fn expand_level_skips_collapsed_leaves() {
        let mut state = open("- a\n- b\n\t- c\n\t\t- d", 0);
        state.folds.insert(state.focus().expect("a focus"));
        state.update(Message::FocusNext);
        state.update(Message::FocusNext);
        state.update(Message::ToggleFold);
        assert_eq!(visible(&state)[2], (1, "c".to_owned(), 1));
        state.update(Message::ExpandLevel);
        assert_eq!(visible(&state)[2], (1, "c".to_owned(), 0));
    }

    // FR-NAV-02
    #[test]
    fn expand_all_and_collapse_all() {
        let mut state = open(TREE, 0);
        state.update(Message::CollapseAll);
        assert_eq!(
            visible(&state),
            [(0, "A".to_owned(), 2), (0, "e".to_owned(), 0)]
        );
        assert_eq!(state.folds.len(), 2);
        state.update(Message::ExpandAll);
        assert_eq!(visible(&state).len(), 5);
    }

    // navigation §2
    #[test]
    fn a_hidden_focus_moves_to_the_next_visible_block_else_the_previous() {
        let mut state = open(TREE, 2);
        state.update(Message::CollapseAll);
        assert_eq!(focus_text(&state), "e");
        let mut last = open("A:\n\t- b\n\t\t- c", 2);
        last.update(Message::CollapseAll);
        assert_eq!(focus_text(&last), "A");
    }

    // FR-NAV-01
    #[test]
    fn parent_and_siblings() {
        let mut state = open(TREE, 2);
        state.update(Message::FocusParent);
        assert_eq!(focus_text(&state), "b");
        state.update(Message::FocusNextSibling);
        assert_eq!(focus_text(&state), "d");
        state.update(Message::FocusNextSibling);
        assert_eq!(focus_text(&state), "d");
        state.update(Message::FocusPreviousSibling);
        assert_eq!(focus_text(&state), "b");
        state.update(Message::FocusPreviousSibling);
        assert_eq!(focus_text(&state), "b");
        state.update(Message::FocusParent);
        state.update(Message::FocusParent);
        assert_eq!(focus_text(&state), "A");
        state.update(Message::FocusNextSibling);
        assert_eq!(focus_text(&state), "e");
    }

    // FR-SPC-01
    #[test]
    fn a_block_with_only_special_children_is_a_leaf() {
        let mut state = open("- a\n\tC @hidden\n- b", 0);
        state.update(Message::ToggleSubtree);
        state.update(Message::CollapseAll);
        state.update(Message::Collapse);
        assert_eq!(state.folds, HashSet::new());
        assert_eq!(visible(&state)[0], (0, "a".to_owned(), 0));
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
            Message::FocusParent,
            Message::FocusNextSibling,
            Message::FocusPreviousSibling,
            Message::Collapse,
            Message::Expand,
            Message::ToggleFold,
            Message::ToggleSubtree,
            Message::ExpandLevel,
            Message::CollapseLevel,
            Message::ExpandAll,
            Message::CollapseAll,
        ] {
            assert_eq!(state.update(message), [], "{message:?}");
            assert_eq!(state.focus(), None, "{message:?}");
        }
    }
}
