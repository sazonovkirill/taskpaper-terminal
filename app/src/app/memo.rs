//! Keeping what is computed from the state while what it is computed from stays the same, since
//! the core and the frontend read the same views several times for each input (N-PERF-02).

use std::cell::RefCell;

/// How many keys a memo keeps values for: enough for every pane on a screen.
const KEPT: usize = 8;

/// The values computed for the latest few keys, where a key holds everything its value is
/// computed from, so an equal key gives the same value.
#[derive(Debug, Clone)]
pub(super) struct Memo<K, V> {
    entries: RefCell<Vec<(K, V)>>,
}

impl<K, V> Default for Memo<K, V> {
    fn default() -> Self {
        Self {
            entries: RefCell::new(Vec::new()),
        }
    }
}

impl<K: PartialEq, V: Clone> Memo<K, V> {
    /// The value kept for `key`, or else the one `compute` gives, kept for it from now on.
    pub(super) fn get(&self, key: K, compute: impl FnOnce() -> V) -> V {
        if let Some((_, value)) = self.entries.borrow().iter().find(|(kept, _)| *kept == key) {
            return value.clone();
        }
        let value = compute();
        let mut entries = self.entries.borrow_mut();
        if entries.len() == KEPT {
            entries.remove(0);
        }
        entries.push((key, value.clone()));
        value
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    // N-PERF-02
    #[test]
    fn a_value_is_computed_once_per_key_while_kept() {
        let memo = Memo::default();
        let computed = Cell::new(0);
        let get = |key: usize| {
            memo.get(key, || {
                computed.set(computed.get() + 1);
                key * 10
            })
        };
        assert_eq!(get(1), 10);
        assert_eq!(get(2), 20);
        assert_eq!(get(1), 10);
        assert_eq!(computed.get(), 2, "the kept value comes back");
        for key in 3..=9 {
            get(key);
        }
        assert_eq!(computed.get(), 9);
        assert_eq!(get(2), 20, "the oldest went first");
        assert_eq!(computed.get(), 9);
        assert_eq!(get(1), 10);
        assert_eq!(computed.get(), 10, "computed again once dropped");
    }
}
