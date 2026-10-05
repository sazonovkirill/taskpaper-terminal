//! The undo and redo history: which committed transactions can be reverted and replayed, in what
//! order (FR-OPS-08).

use crate::document::{Document, EditError, ItemId, Transaction};

/// Where the view returns to when a transaction is undone or redone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Restore {
    /// The focused block's item.
    pub focus: Option<ItemId>,
}

/// The transactions to undo, latest last, and the undone ones to redo.
#[derive(Debug, Clone, Default)]
pub struct History {
    /// Each committed transaction's inverse, ready to apply.
    undo: Vec<Transaction<Restore>>,
    /// Each undone transaction, ready to apply again.
    redo: Vec<Transaction<Restore>>,
}

impl History {
    /// Applies `transaction` to `document` and makes it the next step to undo, dropping everything
    /// undone before it. A transaction without ops is not recorded.
    ///
    /// # Errors
    ///
    /// The [`EditError`] of an op that does not fit; then nothing changes.
    pub fn commit(
        &mut self,
        document: &mut Document,
        mut transaction: Transaction<Restore>,
    ) -> Result<(), EditError> {
        document.apply(&mut transaction)?;
        self.record(transaction);
        Ok(())
    }

    /// Makes `transaction`, which the document has already been changed by, the next step to
    /// undo, dropping everything undone before it, as for a change applied while it was being
    /// made, such as an edit session's typing. A transaction without ops is not recorded.
    pub fn record(&mut self, transaction: Transaction<Restore>) {
        if !transaction.ops().is_empty() {
            self.undo.push(transaction.inverse());
            self.redo.clear();
        }
    }

    /// Reverts the latest step and returns where the view was before it, or `None` when there is
    /// nothing to undo.
    ///
    /// # Errors
    ///
    /// The [`EditError`] when the document no longer matches the step; then nothing changes.
    pub fn undo(&mut self, document: &mut Document) -> Result<Option<Restore>, EditError> {
        step(&mut self.undo, &mut self.redo, document)
    }

    /// Replays the latest undone step and returns where the view was after it, or `None` when
    /// there is nothing to redo.
    ///
    /// # Errors
    ///
    /// The [`EditError`] when the document no longer matches the step; then nothing changes.
    pub fn redo(&mut self, document: &mut Document) -> Result<Option<Restore>, EditError> {
        step(&mut self.redo, &mut self.undo, document)
    }
}

/// Applies the latest transaction of `from` and moves its inverse onto `to`.
fn step(
    from: &mut Vec<Transaction<Restore>>,
    to: &mut Vec<Transaction<Restore>>,
    document: &mut Document,
) -> Result<Option<Restore>, EditError> {
    let Some(mut transaction) = from.pop() else {
        return Ok(None);
    };
    if let Err(error) = document.apply(&mut transaction) {
        from.push(transaction);
        return Err(error);
    }
    let restore = *transaction.after();
    to.push(transaction.inverse());
    Ok(Some(restore))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Op, format::parse};

    const BEFORE: Restore = Restore { focus: None };

    fn texts(document: &Document) -> Vec<String> {
        document
            .descendants(document.root())
            .map(|(id, _)| document.get(id).unwrap().text().to_owned())
            .collect()
    }

    /// A transaction renaming the line `from` to `to`, unfocused before and focused on it after.
    fn rename(document: &Document, from: &str, to: &str) -> Transaction<Restore> {
        let id = document
            .descendants(document.root())
            .map(|(id, _)| id)
            .find(|&id| document.get(id).unwrap().text() == from)
            .unwrap();
        let op = Op::SetText {
            id,
            before: from.into(),
            after: to.into(),
        };
        Transaction::new(vec![op], BEFORE, Restore { focus: Some(id) })
    }

    fn commit(history: &mut History, document: &mut Document, from: &str, to: &str) {
        let transaction = rename(document, from, to);
        history.commit(document, transaction).unwrap();
    }

    // FR-OPS-08
    #[test]
    fn undo_and_redo_walk_the_committed_steps_and_restore_the_view() {
        let mut document = parse("a\nb");
        let mut history = History::default();
        let a = *rename(&document, "a", "A").after();
        let b = *rename(&document, "b", "B").after();
        commit(&mut history, &mut document, "a", "A");
        commit(&mut history, &mut document, "b", "B");
        assert_eq!(texts(&document), ["A", "B"]);
        assert_eq!(history.undo(&mut document), Ok(Some(BEFORE)));
        assert_eq!(texts(&document), ["A", "b"]);
        assert_eq!(history.undo(&mut document), Ok(Some(BEFORE)));
        assert_eq!(texts(&document), ["a", "b"]);
        assert_eq!(history.undo(&mut document), Ok(None));
        assert_eq!(history.redo(&mut document), Ok(Some(a)));
        assert_eq!(texts(&document), ["A", "b"]);
        assert_eq!(history.redo(&mut document), Ok(Some(b)));
        assert_eq!(texts(&document), ["A", "B"]);
        assert_eq!(history.redo(&mut document), Ok(None));
        assert_eq!(document.revision(), 6);
    }

    #[test]
    fn committing_drops_what_was_undone() {
        let mut document = parse("a");
        let mut history = History::default();
        commit(&mut history, &mut document, "a", "b");
        history.undo(&mut document).unwrap();
        commit(&mut history, &mut document, "a", "c");
        assert_eq!(history.redo(&mut document), Ok(None));
        assert_eq!(texts(&document), ["c"]);
        history.undo(&mut document).unwrap();
        assert_eq!(texts(&document), ["a"]);
    }

    #[test]
    fn empty_and_refused_commits_keep_the_history() {
        let mut document = parse("a");
        let mut history = History::default();
        commit(&mut history, &mut document, "a", "b");
        history.undo(&mut document).unwrap();
        let empty = Transaction::new(Vec::new(), BEFORE, BEFORE);
        history.commit(&mut document, empty).unwrap();
        let mut refused = rename(&document, "a", "c");
        refused = Transaction::new(refused.inverse().ops().to_vec(), BEFORE, BEFORE);
        assert!(history.commit(&mut document, refused).is_err());
        assert_eq!(texts(&document), ["a"]);
        assert_eq!(document.revision(), 2);
        assert!(history.redo(&mut document).unwrap().is_some());
        assert_eq!(
            texts(&document),
            ["b"],
            "the undone step is still there to redo"
        );
    }

    // FR-OPS-08
    #[test]
    fn a_recorded_change_is_undone_and_redone_like_a_committed_one() {
        let mut document = parse("a");
        let mut history = History::default();
        commit(&mut history, &mut document, "a", "b");
        history.undo(&mut document).unwrap();
        let mut typed = rename(&document, "a", "abc");
        document.apply(&mut typed).unwrap();
        history.record(typed);
        assert_eq!(
            history.redo(&mut document),
            Ok(None),
            "the undone step is dropped"
        );
        assert_eq!(history.undo(&mut document), Ok(Some(BEFORE)));
        assert_eq!(texts(&document), ["a"]);
        assert!(history.redo(&mut document).unwrap().is_some());
        assert_eq!(texts(&document), ["abc"]);
        history.record(Transaction::new(Vec::new(), BEFORE, BEFORE));
        history.undo(&mut document).unwrap();
        assert_eq!(texts(&document), ["a"], "an empty change is not a step");
    }

    #[test]
    fn a_step_that_no_longer_fits_stays_in_the_history() {
        let mut document = parse("a");
        let mut history = History::default();
        commit(&mut history, &mut document, "a", "b");
        let mut external = rename(&document, "b", "x");
        document.apply(&mut external).unwrap();
        assert!(matches!(
            history.undo(&mut document),
            Err(EditError::Stale { .. })
        ));
        assert_eq!(texts(&document), ["x"]);
        document.apply(&mut external.inverse()).unwrap();
        assert_eq!(history.undo(&mut document), Ok(Some(BEFORE)));
        assert_eq!(texts(&document), ["a"]);
    }
}
