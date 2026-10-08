//! A walk down the outline that names the blocks above each block it reaches, as the entries of
//! special views and finders show them: each name is worked out once for every block below it,
//! and only once one of those is shown (N-PERF-02).

use std::{cell::OnceCell, sync::Arc};

use super::State;
use crate::document::ItemId;

/// A line a walk down the outline went into, with the text of each block from the top level
/// down to it, which the blocks it holds show above themselves.
pub(super) struct Above<'a> {
    line: ItemId,
    /// The line it is in, or `None` for the root.
    parent: Option<&'a Above<'a>>,
    path: OnceCell<Arc<[Arc<str>]>>,
    detail: OnceCell<Arc<str>>,
}

impl<'a> Above<'a> {
    /// The root, above the top level, whose path is empty.
    pub(super) fn top(root: ItemId) -> Self {
        Self {
            line: root,
            parent: None,
            path: OnceCell::new(),
            detail: OnceCell::new(),
        }
    }

    /// The block `line` in this one.
    pub(super) fn below(&'a self, line: ItemId) -> Self {
        Self {
            line,
            parent: Some(self),
            path: OnceCell::new(),
            detail: OnceCell::new(),
        }
    }

    /// The line.
    pub(super) fn line(&self) -> ItemId {
        self.line
    }

    /// The text of each block from the top level down to the line, as [`State::shown_text`]
    /// gives it.
    pub(super) fn path(&self, state: &State) -> Arc<[Arc<str>]> {
        Arc::clone(self.path.get_or_init(|| {
            match self.parent {
                Some(parent) => parent
                    .path(state)
                    .iter()
                    .cloned()
                    .chain([state.shared_shown_text(self.line)])
                    .collect(),
                None => Arc::default(),
            }
        }))
    }

    /// The path as one text, its blocks joined by ` › `, as a finder shows it.
    pub(super) fn detail(&self, state: &State) -> Arc<str> {
        Arc::clone(
            self.detail
                .get_or_init(|| self.path(state).join(" › ").into()),
        )
    }
}
