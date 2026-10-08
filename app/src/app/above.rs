//! The blocks above each row of a walk over the outline's rows, named as the entries of special
//! views and finders show them: the names above a block are worked out once for every row it
//! holds, and only once one of those is shown (N-PERF-02).

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

    /// The text of each block above the row reached, as [`State::shown_text`] gives it. It
    /// starts from the path of the deepest line that has one and is kept for its line only, so a
    /// row thousands of levels deep takes neither stack nor a path for each level (N-REL-01).
    pub(super) fn path(&self, state: &State) -> Arc<[Arc<str>]> {
        let Some(last) = self.lines.last() else {
            return Arc::default();
        };
        Arc::clone(last.path.get_or_init(|| {
            let known = self
                .lines
                .iter()
                .rposition(|line| line.path.get().is_some());
            let above = known.and_then(|at| self.lines.get(at)?.path.get());
            let below = known.map_or(0, |at| at + 1);
            above
                .into_iter()
                .flat_map(|path| path.iter().cloned())
                .chain(
                    self.lines
                        .get(below..)
                        .unwrap_or_default()
                        .iter()
                        .map(|line| state.shared_shown_text(line.id)),
                )
                .collect()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Document, format};

    /// The path of each line of `state`'s outline whose text is in `asked`, walking every line
    /// in order and asking for no other path.
    fn paths(state: &State, asked: &[&str]) -> Vec<Vec<String>> {
        let document = &state.document;
        let mut above = Above::default();
        let mut paths = Vec::new();
        for (id, depth) in document.descendants(document.root()) {
            above.reach(depth);
            if asked.contains(&document.get(id).unwrap().plain_text()) {
                paths.push(above.path(state).iter().map(ToString::to_string).collect());
            }
            above.enter(id);
        }
        paths
    }

    #[test]
    fn the_path_names_each_block_above_from_the_top_level() {
        let state = State::new(
            format::parse("A:\n\t**B**:\n\t\t- c\n\t\tD:\n\t\t\t- e\n\t- f\ng"),
            None,
        );
        assert_eq!(
            paths(&state, &["c", "e", "f", "g"]),
            [
                vec!["A", "B"],
                vec!["A", "B", "D"],
                vec!["A"],
                Vec::<&str>::new()
            ]
        );
    }

    // N-REL-01
    #[test]
    fn the_path_of_a_row_deeper_than_a_stack_could_take_a_level_at_a_time() {
        let levels = 100_000;
        let state = State::new(Document::chain(levels), None);
        let path = &paths(&state, &[&(levels - 1).to_string()])[0];
        assert_eq!(path.len(), levels - 1);
        assert_eq!(path.last().unwrap(), &(levels - 2).to_string());
    }
}
