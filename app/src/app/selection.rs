//! The selection: which blocks the next command acts on besides the focused one, how keys extend,
//! toggle and clear it, and which blocks a command then takes
//! ([navigation.md §2](../../../docs/specs/navigation.md#2-focus-and-motion-fr-nav-01), FR-OPS-01).

use std::collections::HashSet;

use super::{State, pane::Pane};
use crate::document::{Block, ItemId, blocks};

/// A change to which blocks are selected (FR-OPS-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionChange {
    /// Focus the next row and select every row from where this run of extensions began.
    ExtendDown,
    /// Focus the previous row and select every row from where this run of extensions began.
    ExtendUp,
    /// Select the focused block, or unselect it when it is selected.
    Toggle,
    /// Select every block below the title.
    All,
    /// Select nothing.
    Clear,
    /// Start visual mode at the focused block, or end it and select nothing (D-135).
    Visual,
}

/// The selected blocks and the run that extending grows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Selection {
    /// The selected blocks; a set, as every row asks whether it is one of them.
    picked: HashSet<ItemId>,
    /// The run of rows the latest extensions selected, while the focus is still at its end.
    run: Option<Run>,
    /// The block visual mode began at, while it is on: the selection is then every row from it
    /// to the focus (D-135).
    visual: Option<ItemId>,
}

/// A run of rows selected by extending from `anchor` to `head`, on top of the blocks selected
/// before it began.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Run {
    anchor: ItemId,
    head: ItemId,
    before: HashSet<ItemId>,
}

impl Selection {
    /// Whether the block `id` is selected.
    pub(super) fn contains(&self, id: ItemId) -> bool {
        self.picked.contains(&id)
    }

    /// The blocks selected in only one of this selection and `other`.
    pub(super) fn differences(&self, other: &Self) -> HashSet<ItemId> {
        self.picked
            .symmetric_difference(&other.picked)
            .copied()
            .collect()
    }

    /// This selection with each block replaced by the one `to` finds for it, or dropped when it
    /// finds none, and no run or visual mode, as when the document is replaced.
    pub(super) fn mapped(&self, to: impl Fn(ItemId) -> Option<ItemId>) -> Self {
        Self {
            picked: self.picked.iter().filter_map(|&id| to(id)).collect(),
            ..Self::default()
        }
    }

    /// Ends visual mode, keeping the blocks it selected.
    pub(super) fn end_visual(&mut self) {
        self.visual = None;
    }
}

impl State {
    /// Applies `change` to the selection.
    pub(super) fn change_selection(&mut self, change: SelectionChange) {
        match change {
            SelectionChange::ExtendDown => self.extend_selection(true),
            SelectionChange::ExtendUp => self.extend_selection(false),
            SelectionChange::Toggle => {
                self.pane.selection.end_visual();
                self.toggle_selected();
            }
            SelectionChange::All => self.select_all(),
            SelectionChange::Clear => self.clear_selection(),
            SelectionChange::Visual => self.toggle_visual(),
        }
    }

    /// `V`: starts visual mode with the focused block alone selected, or ends it and selects
    /// nothing.
    fn toggle_visual(&mut self) {
        let start = self.pane.focus.filter(|_| !self.visual());
        self.pane.selection = Selection {
            picked: start.into_iter().collect(),
            run: None,
            visual: start,
        };
    }

    /// Whether the focused pane is in visual mode (D-135).
    #[must_use]
    pub fn visual(&self) -> bool {
        self.pane.selection.visual.is_some()
    }

    /// Selects every row of `pane` from where its visual mode began to its focus, or ends visual
    /// mode, keeping the selection, when either no longer shows.
    pub(super) fn follow_visual(&self, pane: &mut Pane) {
        let Some(anchor) = pane.selection.visual else {
            return;
        };
        let rows = self.rows_of(pane);
        let at = |id| rows.iter().position(|row| row.id == id);
        let (Some(from), Some(to)) = (at(anchor), pane.focus.and_then(at)) else {
            pane.selection.end_visual();
            return;
        };
        pane.selection.picked = rows
            .iter_from(from.min(to))
            .take(from.abs_diff(to) + 1)
            .map(|row| row.id)
            .collect();
        pane.selection.run = None;
    }

    /// `J`/`K`: moves the focus a row down or up and selects every row from where the run began
    /// to the focus; a run begins at the focus unless the focus is where the latest one ended.
    fn extend_selection(&mut self, down: bool) {
        let Some(focus) = self.pane.focus else { return };
        let rows: Vec<ItemId> = self.rows().iter().map(|row| row.id).collect();
        let run = match self.pane.selection.run.take() {
            Some(run) if run.head == focus => run,
            _ => Run {
                anchor: focus,
                head: focus,
                before: self.pane.selection.picked.clone(),
            },
        };
        let at = rows.iter().position(|&id| id == focus).unwrap_or_default();
        let to = if down {
            at.saturating_add(1).min(rows.len() - 1)
        } else {
            at.saturating_sub(1)
        };
        let from = rows
            .iter()
            .position(|&id| id == run.anchor)
            .unwrap_or_default();
        let mut picked = run.before.clone();
        picked.extend(rows.get(from.min(to)..=from.max(to)).unwrap_or_default());
        self.pane.focus = rows.get(to).copied();
        self.pane.selection = Selection {
            picked,
            run: self.pane.focus.map(|head| Run { head, ..run }),
            visual: self.pane.selection.visual,
        };
    }

    /// `v`: selects the focused block, or unselects it when it is selected.
    fn toggle_selected(&mut self) {
        let Some(focus) = self.pane.focus else { return };
        let picked = &mut self.pane.selection.picked;
        if !picked.remove(&focus) {
            picked.insert(focus);
        }
        self.pane.selection.run = None;
    }

    /// `Ctrl+A`: selects every row below the title (FR-OPS-01).
    fn select_all(&mut self) {
        let picked = self
            .rows()
            .iter()
            .filter(|row| !row.title)
            .map(|row| row.id)
            .collect();
        self.pane.selection = Selection {
            picked,
            ..Selection::default()
        };
    }

    /// `Esc`: selects nothing.
    pub(super) fn clear_selection(&mut self) {
        self.pane.selection = Selection::default();
    }

    /// Unselects the blocks no row of `pane` shows any more, because they were deleted, folded away or
    /// left out of the zoom, so a command never acts on a block the user cannot see.
    pub(super) fn keep_visible_selection(&self, pane: &mut Pane) {
        if pane.selection.picked.is_empty() {
            return;
        }
        let rows: HashSet<ItemId> = self.rows_of(pane).iter().map(|row| row.id).collect();
        pane.selection.picked.retain(|id| rows.contains(id));
        // A run whose head is hidden ends anyway, because the focus left it.
        if pane
            .selection
            .run
            .as_ref()
            .is_some_and(|run| !rows.contains(&run.anchor))
        {
            pane.selection.run = None;
        }
    }

    /// Whether any block is selected.
    pub(super) fn selecting(&self) -> bool {
        !self.pane.selection.picked.is_empty()
    }

    /// The blocks a block command acts on: the selected ones in the order the rows show them,
    /// else the focused block, the title included (FR-OPS-07).
    pub(super) fn targets(&self) -> Vec<Block> {
        let selected: Vec<Block> = self
            .rows()
            .iter()
            .filter(|row| self.pane.selection.contains(row.id))
            .filter_map(|row| blocks::block(self.document(), row.id))
            .collect();
        if selected.is_empty() {
            self.focused().map(|(block, _)| block).into_iter().collect()
        } else {
            selected
        }
    }

    /// The blocks a structural command moves with their subtrees: the selected ones below the
    /// title that are not inside another selected one below it, in row order, else the focused block
    /// unless it is the title, which stands for the view's root (editing.md §4).
    pub(super) fn roots(&self) -> Vec<Block> {
        if !self.selecting() {
            return self.focused_block().into_iter().collect();
        }
        let selected = |id: ItemId| self.pane.selection.contains(id);
        self.rows()
            .iter()
            .filter(|row| !row.title && selected(row.id))
            .filter(|row| {
                !std::iter::successors(blocks::parent(self.document(), row.id), |above| {
                    blocks::parent(self.document(), above.id())
                })
                .any(|above| Some(above.id()) != self.zoom() && selected(above.id()))
            })
            .filter_map(|row| blocks::block(self.document(), row.id))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app::Message, document::format};

    /// Four tasks under a project: P, a, b, c, then d.
    const LIST: &str = "P:\n\t- a\n\t- b\n\t- c\n- d";

    /// `text` with focus moved down `index` rows.
    fn open(text: &str, index: usize) -> State {
        let mut state = State::new(format::parse(text), None);
        for _ in 0..index {
            state.update(Message::FocusNext);
        }
        state
    }

    fn send(state: &mut State, messages: &[Message]) {
        for message in messages {
            state.update(message.clone());
        }
    }

    /// The text of each selected row.
    fn selected(state: &State) -> Vec<String> {
        state
            .rows()
            .iter()
            .filter(|row| row.selected)
            .map(|row| row.text.to_string())
            .collect()
    }

    /// The text of the focused row.
    fn focused(state: &State) -> String {
        let rows = state.rows();
        let row = rows.iter().find(|row| Some(row.id) == state.focus());
        row.map(|row| row.text.to_string()).unwrap_or_default()
    }

    /// The display text of each block a command would act on.
    fn texts(blocks: &[Block], state: &State) -> Vec<String> {
        blocks
            .iter()
            .map(|block| {
                state
                    .document()
                    .get(block.id())
                    .unwrap()
                    .display_text()
                    .to_owned()
            })
            .collect()
    }

    // FR-OPS-01
    #[test]
    fn extending_selects_the_rows_from_where_the_run_began_to_the_focus() {
        let mut state = open(LIST, 1);
        assert_eq!(selected(&state), Vec::<String>::new());
        send(&mut state, &[Message::Select(SelectionChange::ExtendDown)]);
        assert_eq!(selected(&state), ["a", "b"]);
        assert_eq!(focused(&state), "b");
        send(&mut state, &[Message::Select(SelectionChange::ExtendDown)]);
        assert_eq!(selected(&state), ["a", "b", "c"]);
        send(
            &mut state,
            &[
                Message::Select(SelectionChange::ExtendUp),
                Message::Select(SelectionChange::ExtendUp),
                Message::Select(SelectionChange::ExtendUp),
            ],
        );
        assert_eq!(selected(&state), ["P", "a"], "back past where it began");
        assert_eq!(focused(&state), "P");
        send(&mut state, &[Message::Select(SelectionChange::ExtendUp)]);
        assert_eq!(selected(&state), ["P", "a"], "the first row stays the end");
    }

    // FR-OPS-01
    #[test]
    fn extending_at_the_last_row_selects_it() {
        let mut state = open(LIST, 4);
        send(&mut state, &[Message::Select(SelectionChange::ExtendDown)]);
        assert_eq!(selected(&state), ["d"]);
    }

    // FR-OPS-01
    #[test]
    fn a_new_run_begins_where_the_focus_moved_and_keeps_what_was_selected() {
        let mut state = open(LIST, 0);
        send(
            &mut state,
            &[
                Message::Select(SelectionChange::ExtendDown),
                Message::FocusNext,
                Message::FocusNext,
                Message::Select(SelectionChange::ExtendDown),
            ],
        );
        assert_eq!(selected(&state), ["P", "a", "c", "d"]);
        send(&mut state, &[Message::Select(SelectionChange::ExtendUp)]);
        assert_eq!(
            selected(&state),
            ["P", "a", "c"],
            "the run shrinks, the earlier rows stay"
        );
    }

    // FR-OPS-01
    #[test]
    fn v_toggles_the_focused_block_and_esc_clears_all() {
        let mut state = open(LIST, 1);
        send(
            &mut state,
            &[
                Message::Select(SelectionChange::Toggle),
                Message::FocusNext,
                Message::FocusNext,
                Message::Select(SelectionChange::Toggle),
            ],
        );
        assert_eq!(selected(&state), ["a", "c"]);
        send(&mut state, &[Message::Select(SelectionChange::Toggle)]);
        assert_eq!(selected(&state), ["a"]);
        send(&mut state, &[Message::Select(SelectionChange::ExtendUp)]);
        assert_eq!(selected(&state), ["a", "b", "c"], "a run after toggling");
        send(&mut state, &[Message::Select(SelectionChange::Clear)]);
        assert_eq!(selected(&state), Vec::<String>::new());
        assert_eq!(focused(&state), "b", "the focus stays");
    }

    // FR-OPS-01, FR-NAV-03
    #[test]
    fn select_all_selects_every_row_below_the_title() {
        let mut state = open(LIST, 0);
        send(&mut state, &[Message::Select(SelectionChange::All)]);
        assert_eq!(selected(&state), ["P", "a", "b", "c", "d"]);
        send(
            &mut state,
            &[Message::Select(SelectionChange::Clear), Message::ZoomIn],
        );
        send(&mut state, &[Message::Select(SelectionChange::All)]);
        assert_eq!(selected(&state), ["a", "b", "c"]);
    }

    // FR-OPS-01
    #[test]
    fn blocks_no_row_shows_leave_the_selection() {
        let mut state = open(LIST, 1);
        send(
            &mut state,
            &[
                Message::Select(SelectionChange::All),
                Message::FocusParent,
                Message::Collapse,
            ],
        );
        assert_eq!(selected(&state), ["P", "d"]);
        send(&mut state, &[Message::Expand]);
        assert_eq!(selected(&state), ["P", "d"], "hidden ones do not come back");
    }

    // FR-OPS-01
    #[test]
    fn a_run_whose_start_is_hidden_ends() {
        let mut state = open("- x\nP:\n\t- a\n- d", 2);
        send(
            &mut state,
            &[
                Message::Select(SelectionChange::ExtendUp),
                Message::Collapse,
                Message::Select(SelectionChange::ExtendDown),
            ],
        );
        assert_eq!(focused(&state), "d");
        assert_eq!(selected(&state), ["P", "d"], "a new run from P");
    }

    // FR-OPS-01
    #[test]
    fn a_run_whose_end_is_hidden_ends() {
        let mut state = open(LIST, 2);
        send(
            &mut state,
            &[
                Message::Select(SelectionChange::ExtendDown),
                Message::FocusParent,
                Message::Collapse,
                Message::Expand,
                Message::FocusNext,
                Message::FocusNext,
                Message::FocusNext,
            ],
        );
        assert_eq!(focused(&state), "c");
        send(&mut state, &[Message::Select(SelectionChange::ExtendUp)]);
        assert_eq!(selected(&state), ["b", "c"], "a new run from c");
    }

    // FR-OPS-07
    #[test]
    fn commands_act_on_the_selected_blocks_in_row_order_else_the_focused_one() {
        let mut state = open(LIST, 3);
        assert_eq!(texts(&state.targets(), &state), ["c"]);
        send(
            &mut state,
            &[
                Message::Select(SelectionChange::Toggle),
                Message::FocusFirst,
                Message::Select(SelectionChange::Toggle),
            ],
        );
        assert_eq!(texts(&state.targets(), &state), ["P", "c"]);
        assert_eq!(texts(&state.roots(), &state), ["P"], "c is inside P");
    }

    // FR-OPS-07, FR-NAV-03
    #[test]
    fn the_title_is_a_target_but_never_a_root() {
        let mut state = open(LIST, 0);
        send(&mut state, &[Message::ZoomIn, Message::FocusFirst]);
        assert_eq!(texts(&state.targets(), &state), ["P"]);
        assert_eq!(texts(&state.roots(), &state), Vec::<String>::new());
        send(&mut state, &[Message::Select(SelectionChange::ExtendDown)]);
        assert_eq!(texts(&state.targets(), &state), ["P", "a"]);
        assert_eq!(texts(&state.roots(), &state), ["a"]);
    }

    // FR-OPS-01, D-135
    #[test]
    fn visual_mode_selects_the_rows_from_its_anchor_to_the_focus() {
        let mut state = open(LIST, 2);
        send(&mut state, &[Message::Select(SelectionChange::Toggle)]);
        send(&mut state, &[Message::FocusNext]);
        send(&mut state, &[Message::Select(SelectionChange::Visual)]);
        assert!(state.visual());
        assert_eq!(selected(&state), ["c"], "it replaces the selection");
        send(&mut state, &[Message::FocusNext]);
        assert_eq!(selected(&state), ["c", "d"]);
        send(&mut state, &[Message::FocusFirst]);
        assert_eq!(selected(&state), ["P", "a", "b", "c"], "past the anchor");
        send(&mut state, &[Message::Select(SelectionChange::ExtendDown)]);
        assert_eq!(selected(&state), ["a", "b", "c"], "J moves as j does");
        send(&mut state, &[Message::Select(SelectionChange::Visual)]);
        assert!(!state.visual());
        assert_eq!(selected(&state), Vec::<String>::new(), "V again clears");
        send(&mut state, &[Message::FocusNext]);
        assert_eq!(selected(&state), Vec::<String>::new());
    }

    // FR-OPS-01, D-135
    #[test]
    fn esc_ends_visual_mode_and_clears_the_selection() {
        let mut state = open(LIST, 1);
        send(
            &mut state,
            &[
                Message::Select(SelectionChange::Visual),
                Message::FocusNext,
                Message::Select(SelectionChange::Clear),
            ],
        );
        assert!(!state.visual());
        assert_eq!(selected(&state), Vec::<String>::new());
    }

    // FR-OPS-01, D-135
    #[test]
    fn a_command_that_changes_the_document_or_copies_ends_visual_mode_on_the_selection() {
        for command in [Message::Indent, Message::Copy] {
            let mut state = open(LIST, 2);
            send(
                &mut state,
                &[Message::Select(SelectionChange::Visual), Message::FocusNext],
            );
            send(&mut state, std::slice::from_ref(&command));
            assert!(!state.visual(), "{command:?}");
            assert_eq!(selected(&state), ["b", "c"], "{command:?}");
            send(&mut state, &[Message::FocusNext]);
            assert_eq!(
                selected(&state),
                ["b", "c"],
                "{command:?}: the focus moves alone"
            );
        }
    }

    // FR-OPS-01, D-135
    #[test]
    fn v_and_select_all_end_visual_mode_keeping_its_rows() {
        let mut state = open(LIST, 1);
        send(
            &mut state,
            &[
                Message::Select(SelectionChange::Visual),
                Message::FocusNext,
                Message::Select(SelectionChange::Toggle),
            ],
        );
        assert!(!state.visual());
        assert_eq!(selected(&state), ["a"]);
        send(
            &mut state,
            &[
                Message::Select(SelectionChange::Visual),
                Message::Select(SelectionChange::All),
            ],
        );
        assert!(!state.visual());
        send(&mut state, &[Message::FocusFirst]);
        assert_eq!(selected(&state), ["P", "a", "b", "c", "d"]);
    }

    // FR-OPS-01, D-135
    #[test]
    fn visual_mode_ends_when_its_anchor_no_longer_shows() {
        let mut state = open(LIST, 2);
        send(
            &mut state,
            &[
                Message::Select(SelectionChange::Visual),
                Message::FocusParent,
                Message::Collapse,
            ],
        );
        assert!(!state.visual());
        assert_eq!(selected(&state), ["P"]);
        send(&mut state, &[Message::FocusNext]);
        assert_eq!(selected(&state), ["P"]);
    }

    // FR-OPS-01, D-135
    #[test]
    fn each_pane_has_its_own_visual_mode() {
        let mut state = open(LIST, 1);
        send(
            &mut state,
            &[
                Message::Select(SelectionChange::Visual),
                Message::Pane(crate::app::PaneChange::Open),
            ],
        );
        assert!(!state.visual(), "the new pane");
        send(&mut state, &[Message::FocusNext]);
        assert_eq!(selected(&state), Vec::<String>::new());
    }

    #[test]
    fn selection_keys_in_an_empty_document_select_nothing() {
        let mut state = State::new(crate::document::Document::default(), None);
        for message in [
            Message::Select(SelectionChange::ExtendDown),
            Message::Select(SelectionChange::ExtendUp),
            Message::Select(SelectionChange::Toggle),
            Message::Select(SelectionChange::All),
            Message::Select(SelectionChange::Visual),
        ] {
            state.update(message.clone());
            assert!(!state.selecting(), "{message:?}");
            assert!(!state.visual(), "{message:?}");
        }
        assert_eq!(state.targets(), Vec::new());
    }
}
