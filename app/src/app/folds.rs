//! The blocks collapsed in the outline, shared by the rows worked out from them rather than
//! copied, so that each look-up of the rows compares them at once however many there are
//! (N-PERF-02).

use std::{collections::HashSet, ops::Deref, rc::Rc};

use crate::document::ItemId;

/// The ids of collapsed blocks. A clone shares them until either side changes, and two that
/// share them are equal without going through them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Folds(Rc<HashSet<ItemId>>);

impl Folds {
    /// Collapses `id`; whether it was not collapsed. Ids already collapsed leave the folds
    /// shared.
    pub(super) fn insert(&mut self, id: ItemId) -> bool {
        match Rc::get_mut(&mut self.0) {
            Some(folds) => folds.insert(id),
            None => !self.0.contains(&id) && Rc::make_mut(&mut self.0).insert(id),
        }
    }

    /// Expands `id`; whether it was collapsed. Ids not collapsed leave the folds shared.
    pub(super) fn remove(&mut self, id: ItemId) -> bool {
        match Rc::get_mut(&mut self.0) {
            Some(folds) => folds.remove(&id),
            None => self.0.contains(&id) && Rc::make_mut(&mut self.0).remove(&id),
        }
    }
}

impl Deref for Folds {
    type Target = HashSet<ItemId>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromIterator<ItemId> for Folds {
    fn from_iter<I: IntoIterator<Item = ItemId>>(ids: I) -> Self {
        Self(Rc::new(ids.into_iter().collect()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::format;

    #[test]
    fn folds_change_only_where_they_change_and_stay_shared_otherwise() {
        let document = format::parse("a\nb");
        let [a, b] =
            [0, 1].map(|at| document.get(document.root()).expect("the root").children()[at]);
        let mut folds = Folds::default();
        assert!(folds.insert(a));
        assert!(!folds.insert(a), "already collapsed");
        let shared = folds.clone();
        assert!(!folds.insert(a));
        assert!(!folds.remove(b));
        assert!(Rc::ptr_eq(&folds.0, &shared.0), "nothing changed");
        assert!(folds.insert(b));
        assert!(!Rc::ptr_eq(&folds.0, &shared.0));
        assert_eq!(*shared, HashSet::from([a]), "the clone keeps what it had");
        assert!(folds.remove(a));
        assert!(!folds.remove(a), "already expanded");
        assert_eq!(*folds, HashSet::from([b]));
        let shared = folds.clone();
        assert!(folds.remove(b));
        assert!(
            folds.is_empty() && shared.contains(&b),
            "only the copy changed"
        );
        assert_eq!(
            [a, b].into_iter().collect::<Folds>(),
            Folds::from_iter([b, a])
        );
    }
}
