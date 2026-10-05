//! Edits as primitive line operations that can be applied and inverted
//! ([data-model.md §4–5](../../../../docs/specs/data-model.md#4-edits)).

use std::collections::HashMap;

use thiserror::Error;

use super::{Document, Item, ItemId};

/// A primitive change to the line tree; every edit, its undo and its redo is a sequence of these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// Replaces a line's text.
    SetText {
        /// The line.
        id: ItemId,
        /// Its text before the change.
        before: String,
        /// Its text after the change.
        after: String,
    },
    /// Puts a line with its subtree at a position.
    Insert {
        /// The new parent.
        parent: ItemId,
        /// The position among the parent's children.
        index: usize,
        /// The lines to put there.
        subtree: Snapshot,
    },
    /// Takes a line with its subtree out of the document.
    Remove {
        /// The parent it is taken from.
        parent: ItemId,
        /// Its position among the parent's children.
        index: usize,
        /// The lines taken, exactly as they are in the document.
        subtree: Snapshot,
    },
    /// Moves a line with its subtree.
    Move {
        /// The line.
        id: ItemId,
        /// Its parent and position among that parent's children before the move.
        from: (ItemId, usize),
        /// Its parent and position after the move, counted once it has left `from`.
        to: (ItemId, usize),
    },
    /// Changes how many indent levels a line has beyond its expected depth.
    SetExtraIndent {
        /// The line.
        id: ItemId,
        /// The extra levels before the change.
        before: usize,
        /// The extra levels after the change.
        after: usize,
    },
}

/// A line with its whole subtree, as [`Op::Insert`] puts it in and [`Op::Remove`] takes it out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// The top line, whose parent is left unset.
    top: (ItemId, Item),
    /// The lines of its subtree in document order.
    below: Vec<(ItemId, Item)>,
}

/// The ops of one user command, with view-defined tokens for restoring the focus and selection
/// as they were before and after it; `document::model` never looks inside them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction<T> {
    ops: Vec<Op>,
    before: T,
    after: T,
}

/// Why an op does not fit the document; applying a transaction that fails changes nothing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EditError {
    /// The line is not in the document, or is its virtual root.
    #[error("line {id:?} is not in the document")]
    Missing {
        /// The line.
        id: ItemId,
    },
    /// The line is not in the state the op expects: its text, extra indent, position or subtree
    /// differ.
    #[error("line {id:?} has changed since the edit was made")]
    Stale {
        /// The line.
        id: ItemId,
    },
    /// The id to insert is already in the document or was not made by it.
    #[error("line {id:?} cannot be inserted: its id is in use or unknown")]
    IdUnavailable {
        /// The id.
        id: ItemId,
    },
    /// The position is past the end of the parent's children.
    #[error("position {index} is past the children of line {parent:?}")]
    Position {
        /// The parent.
        parent: ItemId,
        /// The position.
        index: usize,
    },
    /// The move would put a line inside its own subtree.
    #[error("line {id:?} cannot move into its own subtree")]
    IntoOwnSubtree {
        /// The line.
        id: ItemId,
    },
}

impl Op {
    /// The op that undoes this one.
    #[must_use]
    pub fn inverse(self) -> Self {
        match self {
            Self::SetText { id, before, after } => Self::SetText {
                id,
                before: after,
                after: before,
            },
            Self::Insert {
                parent,
                index,
                subtree,
            } => Self::Remove {
                parent,
                index,
                subtree,
            },
            Self::Remove {
                parent,
                index,
                subtree,
            } => Self::Insert {
                parent,
                index,
                subtree,
            },
            Self::Move { id, from, to } => Self::Move {
                id,
                from: to,
                to: from,
            },
            Self::SetExtraIndent { id, before, after } => Self::SetExtraIndent {
                id,
                before: after,
                after: before,
            },
        }
    }
}

impl Snapshot {
    /// A new line without children; `id` comes from [`Document::new_id`].
    #[must_use]
    pub fn line(id: ItemId, text: String) -> Self {
        Self {
            top: (
                id,
                Item {
                    parent: None,
                    children: Vec::new(),
                    text,
                    extra_indent: 0,
                    original: None,
                },
            ),
            below: Vec::new(),
        }
    }

    /// The id of the top line.
    #[must_use]
    pub fn id(&self) -> ItemId {
        self.top.0
    }

    /// Every line, top line first.
    fn lines(&self) -> impl Iterator<Item = &(ItemId, Item)> {
        std::iter::once(&self.top).chain(&self.below)
    }
}

impl<T> Transaction<T> {
    /// The ops of a command, with the restore tokens from before and after it.
    #[must_use]
    pub fn new(ops: Vec<Op>, before: T, after: T) -> Self {
        Self { ops, before, after }
    }

    /// The ops in the order they apply.
    #[must_use]
    pub fn ops(&self) -> &[Op] {
        &self.ops
    }

    /// The restore token from after the command.
    #[must_use]
    pub fn after(&self) -> &T {
        &self.after
    }

    /// The transaction that undoes this one: the inverse ops in reverse order, with the tokens
    /// swapped.
    #[must_use]
    pub fn inverse(self) -> Self {
        Self {
            ops: self.ops.into_iter().rev().map(Op::inverse).collect(),
            before: self.after,
            after: self.before,
        }
    }
}

impl Document {
    /// A fresh id for a line to put in with [`Op::Insert`]; it finds nothing until then.
    pub fn new_id(&mut self) -> ItemId {
        self.items.insert(None)
    }

    /// The line's parent and its position among the parent's children; `None` for the root and
    /// for lines not in the document.
    #[must_use]
    pub fn position(&self, id: ItemId) -> Option<(ItemId, usize)> {
        let parent = self.get(id)?.parent?;
        let index = self
            .children_of(parent)
            .iter()
            .position(|&child| child == id)?;
        Some((parent, index))
    }

    /// The line with its subtree as it is now, for building an [`Op::Remove`]; `None` for the root
    /// and for lines not in the document.
    #[must_use]
    pub fn snapshot(&self, id: ItemId) -> Option<Snapshot> {
        let top = Item {
            parent: None,
            ..self.line(id).ok()?.clone()
        };
        let below = self
            .descendants(id)
            .filter_map(|(id, _)| Some((id, self.get(id)?.clone())))
            .collect();
        Some(Snapshot {
            top: (id, top),
            below,
        })
    }

    /// A copy of `subtree`, which may come from another document, under fresh ids of this one,
    /// ready for an [`Op::Insert`]. The copied lines were never read from this document's file,
    /// so they are written in its format rather than as the lines they copy.
    ///
    /// # Panics
    ///
    /// Never: every parent and child a line of a snapshot names is in the snapshot, apart from
    /// the top line's parent, which is unset.
    pub fn fresh_copy(&mut self, subtree: &Snapshot) -> Snapshot {
        let ids: HashMap<ItemId, ItemId> = subtree
            .lines()
            .map(|&(id, _)| (id, self.new_id()))
            .collect();
        let fresh = |id: &ItemId| {
            *ids.get(id)
                .expect("a snapshot holds every line its lines name")
        };
        let copy = |(id, item): &(ItemId, Item)| {
            let item = Item {
                parent: item.parent.as_ref().map(fresh),
                children: item.children.iter().map(fresh).collect(),
                original: None,
                ..item.clone()
            };
            (fresh(id), item)
        };
        Snapshot {
            top: copy(&subtree.top),
            below: subtree.below.iter().map(copy).collect(),
        }
    }

    /// Applies every op of `transaction` in order, then resets the extra indent of each line that
    /// would no longer be read back under its parent and appends those changes to the transaction
    /// ([file-format.md §3.1](../../../../docs/specs/file-format.md#31-canonical-outlines), rule 1).
    /// The revision goes up when the transaction holds any op.
    ///
    /// # Errors
    ///
    /// The [`EditError`] of the first op that does not fit; the document and the transaction are
    /// then left as they were.
    ///
    /// # Panics
    ///
    /// Never: undoing the ops that applied before a failing one always fits the document.
    pub fn apply<T>(&mut self, transaction: &mut Transaction<T>) -> Result<(), EditError> {
        for (applied, op) in transaction.ops.iter().enumerate() {
            if let Err(error) = self.apply_op(op) {
                for op in transaction.ops.iter().take(applied).rev() {
                    self.apply_op(&op.clone().inverse())
                        .expect("the inverse of an op that just applied fits the document");
                }
                return Err(error);
            }
        }
        transaction.ops.extend(self.repair_placement());
        if !transaction.ops.is_empty() {
            self.revision += 1;
        }
        Ok(())
    }

    /// Applies one op without the placement repairs [`Document::apply`] makes after a whole
    /// transaction, so that a copy of the document can stand for it partway through one.
    ///
    /// # Errors
    ///
    /// The [`EditError`] when the op does not fit; then nothing changes.
    pub(crate) fn apply_op(&mut self, op: &Op) -> Result<(), EditError> {
        match op {
            Op::SetText { id, before, after } => {
                let item = self.line_mut(*id)?;
                if item.text != *before {
                    return Err(EditError::Stale { id: *id });
                }
                item.text.clone_from(after);
            }
            Op::SetExtraIndent { id, before, after } => {
                let item = self.line_mut(*id)?;
                if item.extra_indent != *before {
                    return Err(EditError::Stale { id: *id });
                }
                item.extra_indent = *after;
            }
            Op::Insert {
                parent,
                index,
                subtree,
            } => self.insert(*parent, *index, subtree)?,
            Op::Remove {
                parent,
                index,
                subtree,
            } => self.remove(*parent, *index, subtree)?,
            Op::Move { id, from, to } => self.move_line(*id, *from, *to)?,
        }
        Ok(())
    }

    fn insert(
        &mut self,
        parent: ItemId,
        index: usize,
        subtree: &Snapshot,
    ) -> Result<(), EditError> {
        let siblings = self.get(parent).ok_or(EditError::Missing { id: parent })?;
        if index > siblings.children.len() {
            return Err(EditError::Position { parent, index });
        }
        if let Some(&(id, _)) = subtree
            .lines()
            .find(|&&(id, _)| !matches!(self.items.get(id), Some(None)))
        {
            return Err(EditError::IdUnavailable { id });
        }
        let top = subtree.id();
        for (id, item) in subtree.lines() {
            if let Some(slot) = self.items.get_mut(*id) {
                *slot = Some(item.clone());
            }
        }
        if let Some(Some(item)) = self.items.get_mut(top) {
            item.parent = Some(parent);
        }
        if let Some(Some(item)) = self.items.get_mut(parent) {
            item.children.insert(index, top);
        }
        Ok(())
    }

    fn remove(
        &mut self,
        parent: ItemId,
        index: usize,
        subtree: &Snapshot,
    ) -> Result<(), EditError> {
        let id = subtree.id();
        self.line(id)?;
        if self.position(id) != Some((parent, index)) || self.snapshot(id).as_ref() != Some(subtree)
        {
            return Err(EditError::Stale { id });
        }
        for (id, _) in subtree.lines() {
            if let Some(slot) = self.items.get_mut(*id) {
                *slot = None;
            }
        }
        if let Some(Some(item)) = self.items.get_mut(parent) {
            item.children.remove(index);
        }
        Ok(())
    }

    fn move_line(
        &mut self,
        id: ItemId,
        from: (ItemId, usize),
        to: (ItemId, usize),
    ) -> Result<(), EditError> {
        self.line(id)?;
        if self.position(id) != Some(from) {
            return Err(EditError::Stale { id });
        }
        let (parent, index) = to;
        let target = self.get(parent).ok_or(EditError::Missing { id: parent })?;
        let room = target
            .children
            .len()
            .saturating_sub(usize::from(parent == from.0));
        if index > room {
            return Err(EditError::Position { parent, index });
        }
        if std::iter::successors(Some(parent), |&above| self.get(above)?.parent).any(|a| a == id) {
            return Err(EditError::IntoOwnSubtree { id });
        }
        if let Some(Some(item)) = self.items.get_mut(from.0) {
            item.children.remove(from.1);
        }
        if let Some(Some(item)) = self.items.get_mut(parent) {
            item.children.insert(index, id);
        }
        if let Some(Some(item)) = self.items.get_mut(id) {
            item.parent = Some(parent);
        }
        Ok(())
    }

    /// One forward pass in document order: a line whose nearest preceding shallower line is not
    /// its parent gets extra indent 0, which places it right under its parent; that can only
    /// change the depth of lines after it.
    fn repair_placement(&mut self) -> Vec<Op> {
        let lines: Vec<(ItemId, ItemId, usize)> = self
            .descendants(self.root)
            .filter_map(|(id, _)| {
                let item = self.get(id)?;
                Some((id, item.parent?, item.extra_indent))
            })
            .collect();
        let mut depths: HashMap<ItemId, usize> = HashMap::new();
        let mut open: Vec<(ItemId, usize)> = Vec::new();
        let mut repairs = Vec::new();
        for (id, parent, extra_indent) in lines {
            let base = depths.get(&parent).map_or(0, |depth| depth + 1);
            let mut depth = base + extra_indent;
            close_lines_at_or_below(&mut open, depth);
            if open.last().map_or(self.root, |&(line, _)| line) != parent {
                repairs.push(Op::SetExtraIndent {
                    id,
                    before: extra_indent,
                    after: 0,
                });
                if let Some(Some(item)) = self.items.get_mut(id) {
                    item.extra_indent = 0;
                }
                depth = base;
                close_lines_at_or_below(&mut open, depth);
            }
            depths.insert(id, depth);
            open.push((id, depth));
        }
        repairs
    }

    /// The line `id`, refusing the virtual root.
    fn line(&self, id: ItemId) -> Result<&Item, EditError> {
        self.get(id)
            .filter(|_| id != self.root)
            .ok_or(EditError::Missing { id })
    }

    fn line_mut(&mut self, id: ItemId) -> Result<&mut Item, EditError> {
        self.items
            .get_mut(id)
            .and_then(Option::as_mut)
            .filter(|_| id != self.root)
            .ok_or(EditError::Missing { id })
    }
}

/// Drops the open lines at `depth` or deeper, which cannot be the parent of a line at `depth`.
fn close_lines_at_or_below(open: &mut Vec<(ItemId, usize)>, depth: usize) {
    while open
        .pop_if(|&mut (_, open_depth)| open_depth >= depth)
        .is_some()
    {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::format::parse;

    /// Each line as tabs for its tree depth, `+` per extra indent level and its text.
    fn show(document: &Document) -> Vec<String> {
        document
            .descendants(document.root())
            .map(|(id, depth)| {
                let item = document.get(id).unwrap();
                format!(
                    "{}{}{}",
                    "\t".repeat(depth),
                    "+".repeat(item.extra_indent()),
                    item.text()
                )
            })
            .collect()
    }

    /// The document written with one tab per absolute depth, as the serializer indents it.
    fn written(document: &Document) -> String {
        let mut depths = HashMap::new();
        let mut lines = Vec::new();
        for (id, _) in document.descendants(document.root()) {
            let item = document.get(id).unwrap();
            let base = depths.get(&item.parent().unwrap()).map_or(0, |d| d + 1);
            let depth = base + item.extra_indent();
            depths.insert(id, depth);
            lines.push(format!("{}{}", "\t".repeat(depth), item.text()));
        }
        lines.join("\n")
    }

    /// Asserts placement (rule 1): writing the document and reading it back gives the same tree.
    #[track_caller]
    fn assert_placed(document: &Document) {
        let reread = parse(&written(document));
        assert_eq!(show(&reread), show(document), "placement");
    }

    fn id(document: &Document, text: &str) -> ItemId {
        document
            .descendants(document.root())
            .map(|(id, _)| id)
            .find(|&id| document.get(id).unwrap().text() == text)
            .unwrap()
    }

    fn transaction(ops: Vec<Op>) -> Transaction<()> {
        Transaction::new(ops, (), ())
    }

    /// Applies `ops` to a copy of `document` and checks placement, that the inverse restores
    /// `document` and that applying them again redoes the change; returns the changed copy.
    #[track_caller]
    fn apply_and_undo(document: &Document, ops: Vec<Op>) -> Document {
        let mut changed = document.clone();
        let mut forward = transaction(ops);
        changed.apply(&mut forward).unwrap();
        assert_placed(&changed);
        let after = show(&changed);
        let mut backward = forward.inverse();
        changed.apply(&mut backward).unwrap();
        assert_eq!(show(&changed), show(document), "undo restores the document");
        changed.apply(&mut backward.inverse()).unwrap();
        assert_eq!(show(&changed), after, "redo restores the change");
        changed
    }

    fn set_text(document: &Document, text: &str, after: &str) -> Op {
        Op::SetText {
            id: id(document, text),
            before: text.to_owned(),
            after: after.to_owned(),
        }
    }

    fn remove(document: &Document, id: ItemId) -> Op {
        let (parent, index) = document.position(id).unwrap();
        Op::Remove {
            parent,
            index,
            subtree: document.snapshot(id).unwrap(),
        }
    }

    #[test]
    fn set_text_changes_one_line_and_bumps_the_revision() {
        let mut document = parse("- a\n- b");
        let a = id(&document, "- a");
        let mut change = transaction(vec![set_text(&document, "- a", "- c")]);
        document.apply(&mut change).unwrap();
        assert_eq!(show(&document), ["- c", "- b"]);
        assert_eq!(document.get(a).unwrap().text(), "- c");
        assert_eq!(document.revision(), 1);
        document.apply(&mut change.inverse()).unwrap();
        assert_eq!(show(&document), ["- a", "- b"]);
        assert_eq!(document.revision(), 2);
    }

    #[test]
    fn stale_or_missing_lines_are_refused_without_change() {
        let mut document = parse("- a\n\t- b");
        let a = id(&document, "- a");
        let root = document.root();
        let unknown = document.new_id();
        let cases = [
            (set_text(&document, "- a", "x"), Ok(())),
            (
                Op::SetText {
                    id: a,
                    before: "- z".into(),
                    after: "x".into(),
                },
                Err(EditError::Stale { id: a }),
            ),
            (
                Op::SetText {
                    id: root,
                    before: String::new(),
                    after: "x".into(),
                },
                Err(EditError::Missing { id: root }),
            ),
            (
                Op::SetExtraIndent {
                    id: a,
                    before: 1,
                    after: 0,
                },
                Err(EditError::Stale { id: a }),
            ),
            (
                Op::SetExtraIndent {
                    id: unknown,
                    before: 0,
                    after: 1,
                },
                Err(EditError::Missing { id: unknown }),
            ),
            (
                Op::SetExtraIndent {
                    id: root,
                    before: 0,
                    after: 1,
                },
                Err(EditError::Missing { id: root }),
            ),
        ];
        for (op, expected) in cases {
            let mut probe = document.clone();
            let mut change = transaction(vec![op.clone()]);
            assert_eq!(probe.apply(&mut change), expected, "{op:?}");
            if expected.is_err() {
                assert_eq!(show(&probe), show(&document));
                assert_eq!(probe.revision(), 0);
                assert_eq!(change.ops(), [op]);
            }
        }
        document.apply(&mut transaction(Vec::new())).unwrap();
        assert_eq!(
            document.revision(),
            0,
            "an empty transaction changes nothing"
        );
    }

    #[test]
    fn a_failing_op_rolls_back_the_ops_before_it() {
        let mut document = parse("- a\n- b\n\t- c");
        let b = id(&document, "- b");
        let ops = vec![
            set_text(&document, "- a", "- x"),
            remove(&document, id(&document, "- c")),
            Op::SetExtraIndent {
                id: b,
                before: 0,
                after: 2,
            },
            set_text(&document, "- a", "- y"),
        ];
        let mut change = transaction(ops.clone());
        let a = id(&document, "- a");
        assert_eq!(document.apply(&mut change), Err(EditError::Stale { id: a }));
        assert_eq!(show(&document), ["- a", "- b", "\t- c"]);
        assert_eq!(document.revision(), 0);
        assert_eq!(change.ops(), ops);
    }

    #[test]
    fn set_extra_indent_changes_the_indent_beyond_the_parent() {
        let document = parse("A:\n\t- b");
        let b = id(&document, "- b");
        let after = apply_and_undo(
            &document,
            vec![Op::SetExtraIndent {
                id: b,
                before: 0,
                after: 2,
            }],
        );
        assert_eq!(show(&after), ["A:", "\t++- b"]);
    }

    #[test]
    fn insert_puts_a_new_line_in_and_undo_takes_it_out() {
        let mut document = parse("A:\n\t- b");
        let a = id(&document, "A:");
        let new = document.new_id();
        assert_eq!(document.get(new), None);
        assert_eq!(document.position(new), None);
        let insert = Op::Insert {
            parent: a,
            index: 0,
            subtree: Snapshot::line(new, "- n".into()),
        };
        let after = apply_and_undo(&document, vec![insert]);
        assert_eq!(show(&after), ["A:", "\t- n", "\t- b"]);
        assert_eq!(after.position(new), Some((a, 0)));
        assert_eq!(after.get(new).unwrap().parent(), Some(a));
    }

    #[test]
    fn insert_refuses_bad_parents_positions_and_ids() {
        let mut document = parse("A:\n\t- b");
        let a = id(&document, "A:");
        let b = id(&document, "- b");
        let new = document.new_id();
        let removed = document.new_id();
        let root = document.root();
        let insert = |parent, index, id| {
            transaction(vec![Op::Insert {
                parent,
                index,
                subtree: Snapshot::line(id, "x".into()),
            }])
        };
        let mut probe = document.clone();
        assert_eq!(
            probe.apply(&mut insert(removed, 0, new)),
            Err(EditError::Missing { id: removed })
        );
        assert_eq!(
            probe.apply(&mut insert(a, 2, new)),
            Err(EditError::Position {
                parent: a,
                index: 2
            })
        );
        assert_eq!(
            probe.apply(&mut insert(a, 1, b)),
            Err(EditError::IdUnavailable { id: b })
        );
        assert_eq!(
            probe.apply(&mut insert(a, 1, root)),
            Err(EditError::IdUnavailable { id: root })
        );
        assert_eq!(show(&probe), show(&document));
        probe.apply(&mut insert(a, 1, new)).unwrap();
        assert_eq!(show(&probe), ["A:", "\t- b", "\tx"]);
        assert_eq!(
            probe.apply(&mut insert(a, 0, new)),
            Err(EditError::IdUnavailable { id: new })
        );
    }

    #[test]
    fn remove_takes_a_subtree_out_and_undo_puts_back_the_same_ids() {
        let mut document = parse("A:\n\t- b\n\t\t\tc\n\t\t\t\td\n\t- e");
        let b = id(&document, "- b");
        let c = id(&document, "c");
        let d = id(&document, "d");
        let removal = remove(&document, b);
        let mut change = transaction(vec![removal]);
        document.apply(&mut change).unwrap();
        assert_eq!(show(&document), ["A:", "\t- e"]);
        for line in [b, c, d] {
            assert_eq!(document.get(line), None);
            assert_eq!(document.snapshot(line), None);
        }
        document.apply(&mut change.inverse()).unwrap();
        assert_eq!(
            show(&document),
            ["A:", "\t- b", "\t\t+c", "\t\t\td", "\t- e"]
        );
        assert_eq!(document.get(d).unwrap().parent(), Some(c));
        assert_eq!(document.get(c).unwrap().parent(), Some(b));
    }

    #[test]
    fn remove_refuses_lines_that_moved_or_changed() {
        let mut document = parse("A:\n\t- b\n\t\tc\n\t- e");
        let a = id(&document, "A:");
        let b = id(&document, "- b");
        let removal = remove(&document, b);
        let subtree = document.snapshot(b).unwrap();
        let mut probe = document.clone();
        let mut misplaced = transaction(vec![Op::Remove {
            parent: a,
            index: 1,
            subtree: subtree.clone(),
        }]);
        assert_eq!(probe.apply(&mut misplaced), Err(EditError::Stale { id: b }));
        probe
            .apply(&mut transaction(vec![set_text(&document, "c", "C")]))
            .unwrap();
        assert_eq!(
            probe.apply(&mut transaction(vec![removal.clone()])),
            Err(EditError::Stale { id: b })
        );
        document
            .apply(&mut transaction(vec![removal.clone()]))
            .unwrap();
        assert_eq!(
            document.apply(&mut transaction(vec![removal])),
            Err(EditError::Missing { id: b })
        );
    }

    #[test]
    fn position_and_snapshot_skip_the_root() {
        let document = parse("x");
        let root = document.root();
        let x = id(&document, "x");
        assert_eq!(document.position(root), None);
        assert_eq!(document.snapshot(root), None);
        assert_eq!(document.position(x), Some((root, 0)));
        assert_eq!(document.snapshot(x).unwrap().id(), x);
    }

    #[test]
    fn move_puts_a_subtree_at_its_target_counted_after_leaving() {
        let document = parse("A:\n\t- b\n\t\tc\n\t- e\nF:");
        let a = id(&document, "A:");
        let b = id(&document, "- b");
        let f = id(&document, "F:");
        let within = apply_and_undo(
            &document,
            vec![Op::Move {
                id: b,
                from: (a, 0),
                to: (a, 1),
            }],
        );
        assert_eq!(show(&within), ["A:", "\t- e", "\t- b", "\t\tc", "F:"]);
        let across = apply_and_undo(
            &document,
            vec![Op::Move {
                id: b,
                from: (a, 0),
                to: (f, 0),
            }],
        );
        assert_eq!(show(&across), ["A:", "\t- e", "F:", "\t- b", "\t\tc"]);
        let root = document.root();
        let out = apply_and_undo(
            &document,
            vec![Op::Move {
                id: b,
                from: (a, 0),
                to: (root, 2),
            }],
        );
        assert_eq!(show(&out), ["A:", "\t- e", "F:", "- b", "\tc"]);
    }

    #[test]
    fn move_refuses_stale_sources_bad_targets_and_cycles() {
        let mut document = parse("A:\n\t- b\n\t\tc\n\t- e");
        let a = id(&document, "A:");
        let b = id(&document, "- b");
        let c = id(&document, "c");
        let root = document.root();
        let foreign = document.new_id();
        let cases = [
            ((b, (a, 1), (a, 0)), EditError::Stale { id: b }),
            ((root, (a, 0), (a, 0)), EditError::Missing { id: root }),
            (
                (b, (a, 0), (a, 2)),
                EditError::Position {
                    parent: a,
                    index: 2,
                },
            ),
            (
                (b, (a, 0), (root, 2)),
                EditError::Position {
                    parent: root,
                    index: 2,
                },
            ),
            (
                (b, (a, 0), (foreign, 0)),
                EditError::Missing { id: foreign },
            ),
            ((b, (a, 0), (b, 0)), EditError::IntoOwnSubtree { id: b }),
            ((b, (a, 0), (c, 0)), EditError::IntoOwnSubtree { id: b }),
        ];
        for ((line, from, to), expected) in cases {
            let mut probe = document.clone();
            let mut change = transaction(vec![Op::Move { id: line, from, to }]);
            assert_eq!(probe.apply(&mut change), Err(expected));
            assert_eq!(show(&probe), show(&document));
        }
    }

    // file-format.md §3.1 rule 1
    #[test]
    fn structural_ops_reset_extra_indent_that_would_change_the_parent() {
        let mut document = parse("\t\tA\nB");
        let root = document.root();
        let b = id(&document, "B");
        let a = id(&document, "A");
        let mut swap = transaction(vec![Op::Move {
            id: b,
            from: (root, 1),
            to: (root, 0),
        }]);
        document.apply(&mut swap).unwrap();
        assert_eq!(show(&document), ["B", "A"]);
        assert_eq!(
            swap.ops().last(),
            Some(&Op::SetExtraIndent {
                id: a,
                before: 2,
                after: 0
            })
        );
        document.apply(&mut swap.inverse()).unwrap();
        assert_eq!(show(&document), ["++A", "B"]);
    }

    // file-format.md §3.1 rule 1
    #[test]
    fn placement_repair_resets_only_the_violating_lines() {
        let document = parse("P:\n\t\t\tY\n\t\t\t\tZ\n\tX\nQ:\n\t\t\tW");
        let p = id(&document, "P:");
        let x = id(&document, "X");
        let after = apply_and_undo(
            &document,
            vec![Op::Move {
                id: x,
                from: (p, 1),
                to: (p, 0),
            }],
        );
        assert_eq!(show(&after), ["P:", "\tX", "\tY", "\t\tZ", "Q:", "\t++W"]);

        let mut siblings = parse("A\nB");
        let b = id(&siblings, "B");
        let mut indent = transaction(vec![Op::SetExtraIndent {
            id: b,
            before: 0,
            after: 2,
        }]);
        siblings.apply(&mut indent).unwrap();
        assert_eq!(show(&siblings), ["A", "B"]);
        assert_eq!(indent.ops().len(), 2);
    }

    // file-format.md §3.1 rule 1
    #[test]
    fn placed_lines_keep_their_extra_indent() {
        let mut document = parse("\t\tA\n\t\tB\n\t\t\tC\n\t\tD\nE");
        let mut change = transaction(vec![set_text(&document, "E", "F")]);
        document.apply(&mut change).unwrap();
        assert_eq!(change.ops().len(), 1, "no repairs");
        assert_eq!(show(&document), ["++A", "++B", "\tC", "++D", "F"]);
    }

    /// Every removal and every move of every line in a varied outline keeps placement, and its
    /// inverse restores the outline exactly.
    // FR-OPS-08
    #[test]
    fn every_remove_and_move_is_placed_and_undone_exactly() {
        let source = "\t\tA:\n\t\t\t- b\n\t\t\t\t\tc\n\t\t\td\nE:\n\t\t- f\n\t- g\n\t\th\n- i";
        let document = parse(source);
        let lines: Vec<ItemId> = document
            .descendants(document.root())
            .map(|(id, _)| id)
            .collect();
        let parents: Vec<ItemId> = std::iter::once(document.root())
            .chain(lines.iter().copied())
            .collect();
        for &line in &lines {
            apply_and_undo(&document, vec![remove(&document, line)]);
            let from = document.position(line).unwrap();
            for &parent in &parents {
                let inside =
                    std::iter::successors(Some(parent), |&up| document.get(up).unwrap().parent())
                        .any(|up| up == line);
                if inside {
                    continue;
                }
                let room =
                    document.get(parent).unwrap().children().len() - usize::from(parent == from.0);
                for index in 0..=room {
                    apply_and_undo(
                        &document,
                        vec![Op::Move {
                            id: line,
                            from,
                            to: (parent, index),
                        }],
                    );
                }
            }
        }
    }

    #[test]
    fn inverses_undo_each_op_and_swap_the_tokens() {
        let mut document = parse("a");
        let a = id(&document, "a");
        let root = document.root();
        let new = document.new_id();
        let ops = vec![
            set_text(&document, "a", "b"),
            Op::SetExtraIndent {
                id: a,
                before: 0,
                after: 1,
            },
            Op::Insert {
                parent: root,
                index: 1,
                subtree: Snapshot::line(new, "n".into()),
            },
            Op::Move {
                id: a,
                from: (root, 0),
                to: (root, 1),
            },
        ];
        for op in &ops {
            assert_ne!(&op.clone().inverse(), op);
            assert_eq!(&op.clone().inverse().inverse(), op);
        }
        let change = Transaction::new(ops.clone(), 1, 2);
        let undo = change.inverse();
        assert_eq!(*undo.after(), 1);
        let expected: Vec<Op> = ops.iter().rev().cloned().map(Op::inverse).collect();
        assert_eq!(undo.ops(), expected);
        assert_eq!(*undo.inverse().after(), 2);
    }

    #[test]
    fn a_fresh_copy_of_another_documents_subtree_goes_in_under_new_ids() {
        let source = parse("A:\n\t- b\n\t\tc\n\t- d");
        let mut document = parse("x:\n  - y");
        let copy = document.fresh_copy(&source.snapshot(id(&source, "A:")).unwrap());
        let root = document.root();
        let changed = apply_and_undo(
            &document,
            vec![Op::Insert {
                parent: root,
                index: 1,
                subtree: copy,
            }],
        );
        assert_eq!(
            show(&changed),
            ["x:", "\t- y", "A:", "\t- b", "\t\tc", "\t- d"]
        );
        let a = id(&changed, "A:");
        let c = id(&changed, "c");
        assert_eq!(changed.position(id(&changed, "- d")), Some((a, 1)));
        assert_eq!(changed.position(c), Some((id(&changed, "- b"), 0)));
        assert_eq!(
            crate::document::format::serialize(&changed),
            "x:\n  - y\nA:\n  - b\n    c\n  - d",
            "written in this document's format, not as the tab-indented lines it copies"
        );
    }
}
