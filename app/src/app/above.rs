//! The blocks above each row of a walk over the outline's rows, named as the entries of special
//! views and finders show them: each name is worked out once for every row below it, and only
//! once one of those is shown (N-PERF-02).

use std::{cell::OnceCell, sync::Arc};

use super::State;
use crate::document::ItemId;

/// The blocks above the row a walk over the outline's rows, in order and from the top level, is
/// at: from the top level down to its parent.
#[derive(Debug, Default)]
pub(super) struct Above {
    lines: Vec<Line>,
}

/// A block above the row a walk is at, with the text of each block from the top level down to
/// it, which the rows it holds show above themselves.
#[derive(Debug)]
struct Line {
    id: ItemId,
    path: OnceCell<Arc<[Arc<str>]>>,
}

impl Above {
    /// Goes to a row at `depth`, below the rows entered before it at lower depths.
    pub(super) fn reach(&mut self, depth: usize) {
        self.lines.truncate(depth);
    }

    /// Goes into the block `id` of the row reached last, which the rows below it are inside.
    pub(super) fn enter(&mut self, id: ItemId) {
        self.lines.push(Line {
            id,
            path: OnceCell::new(),
        });
    }

    /// The top-level block above the row reached, if it is not at the top level.
    pub(super) fn top(&self) -> Option<ItemId> {
        self.lines.first().map(|line| line.id)
    }

    /// The text of each block above the row reached, as [`State::shown_text`] gives it.
    pub(super) fn path(&self, state: &State) -> Arc<[Arc<str>]> {
        self.path_of(self.lines.len(), state)
    }

    /// The path of the first `len` lines.
    fn path_of(&self, len: usize, state: &State) -> Arc<[Arc<str>]> {
        let Some(line) = len.checked_sub(1).and_then(|last| self.lines.get(last)) else {
            return Arc::default();
        };
        Arc::clone(line.path.get_or_init(|| {
            self.path_of(len - 1, state)
                .iter()
                .cloned()
                .chain([state.shared_shown_text(line.id)])
                .collect()
        }))
    }
}
