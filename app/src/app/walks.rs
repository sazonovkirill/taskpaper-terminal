//! Lists worked out by a walk over the outline's rows with nothing collapsed, such as a finder's
//! candidates or a special view's entries, kept chunk by chunk of those rows, so that after a
//! change only the chunks it touched are walked again (N-PERF-02).

use std::{cell::RefCell, collections::HashMap};

use super::{
    Row, State,
    above::{Above, Signature},
    rows::{RowChunk, Rows},
};

/// How many walks it keeps: enough for every pane on a screen and a finder.
const KEPT: usize = 8;

/// The latest walks over rows, each kept for the key that holds everything else its list is
/// worked out from.
#[derive(Debug, Clone)]
pub(super) struct Walks<K, T> {
    kept: RefCell<Vec<(K, Walk<T>)>>,
}

/// A walk's passes over the chunks of rows, each kept for its chunk.
type Walk<T> = HashMap<RowChunk, Walked<T>>;

impl<K, T> Default for Walks<K, T> {
    fn default() -> Self {
        Self {
            kept: RefCell::new(Vec::new()),
        }
    }
}

/// A walk's pass over one chunk of rows: where the walk was when it reached the chunk, where it
/// was when it left, and what it listed from the chunk's rows.
#[derive(Debug, Clone)]
struct Walked<T> {
    reached: Signature,
    left: Above,
    listed: Vec<T>,
}

impl<K: PartialEq, T: Clone> Walks<K, T> {
    /// What `step` lists walking `rows` of `state`'s outline in order, for `key`: `step` goes to
    /// each row, keeping track of the blocks above it, and adds to the list. A chunk of rows
    /// reached below the same blocks as in the last walk for `key` lists what it listed then,
    /// without being walked.
    pub(super) fn list(
        &self,
        key: K,
        rows: &Rows,
        state: &State,
        mut step: impl FnMut(&mut Above, &Row, &mut Vec<T>),
    ) -> Vec<T> {
        let mut kept = self.kept.borrow_mut();
        let mut before = kept
            .iter()
            .position(|(kept, _)| *kept == key)
            .map(|at| kept.remove(at).1)
            .unwrap_or_default();
        let mut walked = HashMap::new();
        let mut list = Vec::new();
        let mut above = Above::default();
        for chunk in rows.chunks() {
            let reached = above.signature(state);
            let pass = match before.remove(&chunk) {
                Some(pass) if pass.reached == reached => {
                    list.extend(pass.listed.iter().cloned());
                    pass
                }
                _ => {
                    let from = list.len();
                    for row in chunk.rows() {
                        step(&mut above, row, &mut list);
                    }
                    Walked {
                        reached,
                        left: above.clone(),
                        listed: list.get(from..).unwrap_or_default().to_vec(),
                    }
                }
            };
            above = pass.left.clone();
            walked.insert(chunk, pass);
        }
        if kept.len() == KEPT {
            kept.remove(0);
        }
        kept.push((key, walked));
        list
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::*;
    use crate::{
        app::Location,
        document::{ItemId, Op, Transaction, format},
    };

    /// How many rows `walks` goes to listing the ids of the rows of `state`'s outline under `key`.
    fn walked(walks: &Walks<usize, ItemId>, key: usize, state: &State) -> usize {
        let rows = state.unfolded_rows(Location::Outline(None), true);
        let mut steps = 0;
        let listed = walks.list(key, &rows, state, |_, row, ids| {
            steps += 1;
            ids.push(row.id);
        });
        assert_eq!(listed, rows.iter().map(|row| row.id).collect::<Vec<_>>());
        steps
    }

    // N-PERF-02
    #[test]
    fn a_walk_goes_again_only_to_the_chunks_a_change_built_again() {
        let mut text = "Top:\n".to_owned();
        for project in 0..40 {
            writeln!(text, "\tP{project}:").unwrap();
            for task in 0..50 {
                writeln!(text, "\t\t- {project} {task}").unwrap();
            }
        }
        let mut state = State::new(format::parse(&text), None);
        let walks = Walks::default();
        let all = state.document.descendants(state.document.root()).count();
        assert_eq!(walked(&walks, 0, &state), all);
        assert_eq!(walked(&walks, 0, &state), 0, "nothing changed");
        assert_eq!(walked(&walks, 1, &state), all, "another key");
        for key in 2..=KEPT {
            walked(&walks, key, &state);
        }
        assert_eq!(walked(&walks, 0, &state), all, "the oldest walk is gone");
        let task = state
            .document
            .descendants(state.document.root())
            .nth(20 * 51 + 3);
        let id = task.unwrap().0;
        let before = state.document.get(id).unwrap().text().to_owned();
        let op = Op::SetText {
            id,
            before,
            after: "- t".to_owned(),
        };
        state
            .document
            .apply(&mut Transaction::new(vec![op], (), ()))
            .unwrap();
        let again = walked(&walks, 0, &state);
        assert!(again > 0 && again <= all / 2, "walked {again} rows again");
    }
}
