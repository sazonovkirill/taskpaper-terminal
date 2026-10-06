//! A pane: one view of the outline with its own location and history, focused block, selection
//! and show-completed setting, while the document and the folds are shared by every pane
//! ([navigation.md §5](../../../docs/specs/navigation.md#5-panes-fr-ui-02--fr-ui-04-d-13),
//! FR-UI-02, D-13).

use super::{Location, selection::Selection};
use crate::document::{Block, ItemId};

/// What one pane shows and where its keys act.
#[derive(Debug, Clone)]
pub(super) struct Pane {
    /// The focused row's item, `None` only when the pane has no rows (FR-NAV-01).
    pub(super) focus: Option<ItemId>,
    /// The selected blocks (FR-OPS-01).
    pub(super) selection: Selection,
    /// What the pane shows: the outline below a zoom, or the Today view (FR-NAV-03, D-7).
    pub(super) location: Location,
    /// Every location visited, oldest first, for back and forward (FR-NAV-04).
    pub(super) history: Vec<Location>,
    /// The position of the current location in `history`.
    pub(super) history_at: usize,
    /// Whether done blocks show, with everything inside them (FR-DONE-02).
    pub(super) show_done: bool,
}

impl Pane {
    /// A pane at `location`, the only one in its history, that shows completed blocks and has
    /// nothing focused or selected yet.
    pub(super) fn at(location: Location) -> Self {
        Self {
            focus: None,
            selection: Selection::default(),
            location,
            history: vec![location],
            history_at: 0,
            show_done: true,
        }
    }

    /// The block shown as the root, or `None` at home and in the Today view (FR-NAV-03).
    pub(super) fn zoom(&self) -> Option<ItemId> {
        match self.location {
            Location::Outline(zoom) => zoom,
            Location::Today => None,
        }
    }

    /// Whether `block` shows where the blocks around it do: not when it is done while completed
    /// blocks are hidden (FR-DONE-02).
    pub(super) fn shows(&self, block: &Block) -> bool {
        self.show_done || !block.done()
    }
}
