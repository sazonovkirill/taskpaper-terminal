//! The selection: which blocks the next command acts on besides the focused one, how keys extend,
//! toggle and clear it, and which blocks a command then takes
//! ([navigation.md §2](../../../docs/specs/navigation.md#2-focus-and-motion-fr-nav-01), FR-OPS-01).

use super::State;
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
}

/// The selected blocks and the run that extending grows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Selection {
    /// The selected blocks, in the order they were selected.
    picked: Vec<ItemId>,
    /// The run of rows the latest extensions selected, while the focus is still at its end.
    run: Option<Run>,
}

/// A run of rows selected by extending from `anchor` to `head`, on top of the blocks selected
/// before it began.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Run {
    anchor: ItemId,
    head: ItemId,
    before: Vec<ItemId>,
}

impl Selection {
    /// Whether the block `id` is selected.
    pub(super) fn contains(&self, id: ItemId) -> bool {
        self.picked.contains(&id)
    }

    /// This selection with each block replaced by the one `to` finds for it, or dropped when it
    /// finds none, and no run, as when the document is replaced.
    pub(super) fn mapped(&self, to: impl Fn(ItemId) -> Option<ItemId>) -> Self {
        Self {
            picked: self.picked.iter().filter_map(|&id| to(id)).collect(),
            run: None,
        }
    }
}

impl State {
    /// Applies `change` to the selection.
    pub(super) fn change_selection(&mut self, change: SelectionChange) {
        match change {
            SelectionChange::ExtendDown => self.extend_selection(true),
            SelectionChange::ExtendUp => self.extend_selection(false),
            SelectionChange::Toggle => self.toggle_selected(),
            SelectionChange::All => self.select_all(),
            SelectionChange::Clear => self.clear_selection(),
        }
    }

    /// `J`/`K`: moves the focus a row down or up and selects every row from where the run began
    /// to the focus; a run begins at the focus unless the focus is where the latest one ended.
    fn extend_selection(&mut self, down: bool) {
        let Some(focus) = self.focus else { return };
        let rows: Vec<ItemId> = self.rows().into_iter().map(|row| row.id).collect();
        let run = match self.selection.run.take() {
            Some(run) if run.head == focus => run,
            _ => Run {
                anchor: focus,
                head: focus,
                before: self.selection.picked.clone(),
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
        for &id in rows.get(from.min(to)..=from.max(to)).unwrap_or_default() {
            if !picked.contains(&id) {
                picked.push(id);
            }
        }
        self.focus = rows.get(to).copied();
        self.selection = Selection {
            picked,
            run: self.focus.map(|head| Run { head, ..run }),
        };
    }

    /// `v`: selects the focused block, or unselects it when it is selected.
    fn toggle_selected(&mut self) {
        let Some(focus) = self.focus else { return };
        let picked = &mut self.selection.picked;
        if let Some(at) = picked.iter().position(|&id| id == focus) {
            picked.remove(at);
        } else {
            picked.push(focus);
        }
        self.selection.run = None;
    }

    /// `Ctrl+A`: selects every row below the title (FR-OPS-01).
    fn select_all(&mut self) {
        let picked = self
            .rows()
            .into_iter()
            .filter(|row| !row.title)
            .map(|row| row.id)
            .collect();
        self.selection = Selection { picked, run: None };
    }

    /// `Esc`: selects nothing.
    pub(super) fn clear_selection(&mut self) {
        self.selection = Selection::default();
    }

    /// Unselects the blocks no row shows any more, because they were deleted, folded away or
    /// left out of the zoom, so a command never acts on a block the user cannot see.
    pub(super) fn keep_visible_selection(&mut self) {
        if !self.selecting() {
            return;
        }
        let rows: Vec<ItemId> = self.rows().into_iter().map(|row| row.id).collect();
        self.selection.picked.retain(|id| rows.contains(id));
        // A run whose head is hidden ends anyway, because the focus left it.
        if self
            .selection
            .run
            .as_ref()
            .is_some_and(|run| !rows.contains(&run.anchor))
        {
            self.selection.run = None;
        }
    }

    /// Whether any block is selected.
    pub(super) fn selecting(&self) -> bool {
        !self.selection.picked.is_empty()
    }

    /// The blocks a block command acts on: the selected ones in the order the rows show them,
    /// else the focused block, the title included (FR-OPS-07).
    pub(super) fn targets(&self) -> Vec<Block> {
        let selected: Vec<Block> = self
            .rows()
            .into_iter()
            .filter(|row| self.selection.contains(row.id))
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
        let selected = |id: ItemId| self.selection.contains(id);
        self.rows()
            .into_iter()
            .filter(|row| !row.title && selected(row.id))
            .filter(|row| {
                !std::iter::successors(blocks::parent(self.document(), row.id), |above| {
                    blocks::parent(self.document(), above.id())
                })
                .any(|above| Some(above.id()) != self.zoom && selected(above.id()))
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
            .into_iter()
            .filter(|row| row.selected)
            .map(|row| row.text)
            .collect()
    }

    /// The text of the focused row.
    fn focused(state: &State) -> String {
        let rows = state.rows();
        let row = rows.iter().find(|row| Some(row.id) == state.focus());
        row.map(|row| row.text.clone()).unwrap_or_default()
    }

    /// The display text of each block a command would act on.
    fn texts(blocks: &[Block], state: &State) -> Vec<String> {
        blocks
            .iter()
            .map(|block| state.document().get(block.id()).unwrap().display_text())
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

    #[test]
    fn selection_keys_in_an_empty_document_select_nothing() {
        let mut state = State::new(crate::document::Document::default(), None);
        for message in [
            Message::Select(SelectionChange::ExtendDown),
            Message::Select(SelectionChange::ExtendUp),
            Message::Select(SelectionChange::Toggle),
            Message::Select(SelectionChange::All),
        ] {
            state.update(message.clone());
            assert!(!state.selecting(), "{message:?}");
        }
        assert_eq!(state.targets(), Vec::new());
    }
}
