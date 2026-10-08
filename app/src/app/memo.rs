//! Keeping what is computed from the state while what it is computed from stays the same, since
//! the core and the frontend read the same views several times for each input (N-PERF-02).

use std::cell::RefCell;

/// How many keys a memo keeps values for: enough for every pane on a screen and the Navigator.
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

impl<K: PartialEq + Clone, V: Clone> Memo<K, V> {
    /// The value kept for `key`, or else the one `compute` gives, kept for it from now on.
    pub(super) fn get(&self, key: K, compute: impl FnOnce() -> V) -> V {
        self.get_or_derive(key, |_| None::<()>, |_| compute())
    }

    /// The value kept for `key`, or else the one `compute` gives from the key and value kept
    /// that `rank` ranks highest, the latest of those, if it ranks any, kept for `key` from now
    /// on; for values cheaper to work out from an earlier one than anew.
    pub(super) fn get_or_derive<R: Ord>(
        &self,
        key: K,
        rank: impl Fn(&K) -> Option<R>,
        compute: impl FnOnce(Option<(K, V)>) -> V,
    ) -> V {
        let base = {
            let entries = self.entries.borrow();
            if let Some((_, value)) = entries.iter().find(|(kept, _)| *kept == key) {
                return value.clone();
            }
            entries
                .iter()
                .filter_map(|entry| Some((rank(&entry.0)?, entry)))
                .max_by(|(a, _), (b, _)| a.cmp(b))
                .map(|(_, entry)| entry.clone())
        };
        let value = compute(base);
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

    // N-PERF-02
    #[test]
    fn a_value_is_derived_from_the_latest_kept_one_ranked_highest() {
        let memo = Memo::default();
        // Keys of the same parity fit, those ending in the same digit best.
        let derive = |key: usize| {
            memo.get_or_derive(
                key,
                |kept| (kept % 2 == key % 2).then_some(kept % 10 == key % 10),
                |base| (key, base.map(|(kept, _)| kept)),
            )
        };
        assert_eq!(derive(1), (1, None), "nothing kept");
        assert_eq!(derive(2), (2, None), "nothing that fits");
        assert_eq!(derive(3), (3, Some(1)));
        assert_eq!(derive(5), (5, Some(3)), "the latest that fits");
        assert_eq!(derive(13), (13, Some(3)), "the one that fits best");
        assert_eq!(derive(1), (1, None), "kept for the key");
    }
}
