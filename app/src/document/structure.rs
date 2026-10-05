//! Adding and deleting blocks as line ops: where a new block goes, which type it takes, and
//! which lines a deleted block takes with it ([editing.md §3–4](../../../docs/specs/editing.md#3-creating-blocks-fr-mode-04),
//! OQ-03, OQ-26).

use super::{
    blocks::{self, Block, BlockKind},
    format::canonical,
    model::{Document, ItemId, Op, Snapshot},
};

/// Where a new block goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place<'a> {
    /// After the block's subtree, as its next sibling.
    Below(&'a Block),
    /// Before the block, as its previous sibling.
    Above(&'a Block),
    /// As the block's first child.
    FirstChild(&'a Block),
    /// As the first top-level block.
    Start,
}

/// An empty block at `place` that the user is about to type into: its line and the ops that put
/// it there, or `None` when `place` names a block that is not in `document`.
///
/// It is a task after or before a task and under a project or task, otherwise a note, unless a
/// note would join the note run next to it, which no other block type does (OQ-03, OQ-26). A
/// line that now has children but no text gets the placeholder, and so does a line that loses
/// the leading whitespace an only line keeps (file-format.md §3.1, rules 2 and 4).
#[must_use]
pub fn new_block(document: &mut Document, place: Place<'_>) -> Option<(ItemId, Vec<Op>)> {
    let (parent, index) = match place {
        Place::Below(block) => {
            let (parent, index) = document.position(block.last_line())?;
            (parent, index + 1)
        }
        Place::Above(block) => document.position(block.id())?,
        Place::FirstChild(block) => (block.last_line(), 0),
        Place::Start => (document.root(), 0),
    };
    let task = match place {
        Place::Below(block) | Place::Above(block) => block.kind() == BlockKind::Task,
        Place::FirstChild(block) => matches!(block.kind(), BlockKind::Project | BlockKind::Task),
        Place::Start => false,
    } || blocks::blank_line_joins_run(document, parent, index);
    let mut ops = repairs(document, parent);
    let id = document.new_id();
    let text = if task { "- " } else { "" };
    ops.push(Op::Insert {
        parent,
        index,
        subtree: Snapshot::line(id, text.to_owned()),
    });
    Some((id, ops))
}

/// The ops repairing what a new line under `parent` breaks: an empty parent would be read back
/// elsewhere, and so would an only line starting with whitespace once it is no longer the only
/// line.
fn repairs(document: &Document, parent: ItemId) -> Vec<Op> {
    let mut lines = document.descendants(document.root()).map(|(id, _)| id);
    let only_line = lines.next().filter(|_| lines.next().is_none());
    std::iter::once(parent)
        .chain(only_line.filter(|&id| id != parent))
        .filter(|&id| id != document.root())
        .filter_map(|id| {
            let item = document.get(id)?;
            let has_children = id == parent || !item.children().is_empty();
            let after = canonical::repaired(item.text(), has_children, false);
            (after != item.text()).then(|| Op::SetText {
                id,
                before: item.text().to_owned(),
                after,
            })
        })
        .collect()
}

/// The ops that take `block` out of `document` with its subtree, its last line first, so that
/// each line is still where its op expects it (FR-OPS-06).
#[must_use]
pub fn delete(document: &Document, block: &Block) -> Vec<Op> {
    block
        .lines()
        .iter()
        .rev()
        .filter_map(|&id| {
            let (parent, index) = document.position(id)?;
            Some(Op::Remove {
                parent,
                index,
                subtree: document.snapshot(id)?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Transaction, format};

    /// Each line as one tab per depth and its text, with the new line marked `>`.
    fn lines(document: &Document, new: Option<ItemId>) -> Vec<String> {
        document
            .descendants(document.root())
            .map(|(id, depth)| {
                let mark = if Some(id) == new { ">" } else { "" };
                format!(
                    "{}{mark}{}",
                    "\t".repeat(depth),
                    document.get(id).unwrap().text()
                )
            })
            .collect()
    }

    /// The block whose first line reads `text`.
    fn block(document: &Document, text: &str) -> Block {
        let id = document
            .descendants(document.root())
            .map(|(id, _)| id)
            .find(|&id| document.get(id).unwrap().text() == text)
            .unwrap();
        blocks::block(document, id).unwrap()
    }

    #[derive(Clone, Copy)]
    enum At {
        Below,
        Above,
        Child,
    }

    /// The lines of `text` once a new block is added `at` the block reading `of`, with the
    /// transaction checked to undo.
    #[track_caller]
    fn added(text: &str, at: At, of: &str) -> Vec<String> {
        let mut document = format::parse(text);
        let block = block(&document, of);
        let place = match at {
            At::Below => Place::Below(&block),
            At::Above => Place::Above(&block),
            At::Child => Place::FirstChild(&block),
        };
        applied(&mut document, place)
    }

    #[track_caller]
    fn applied(document: &mut Document, place: Place<'_>) -> Vec<String> {
        let before = lines(document, None);
        let (id, ops) = new_block(document, place).unwrap();
        let mut transaction = Transaction::new(ops, (), ());
        document.apply(&mut transaction).unwrap();
        let after = lines(document, Some(id));
        document.apply(&mut transaction.inverse()).unwrap();
        assert_eq!(lines(document, None), before, "undo");
        after
    }

    // FR-MODE-04, editing.md §3
    #[test]
    fn below_goes_after_the_subtree_and_above_before_the_block() {
        assert_eq!(
            added("- a\n\t- b\n- c", At::Below, "- a"),
            ["- a", "\t- b", ">- ", "- c"]
        );
        assert_eq!(
            added("- a\n\t- b\n- c", At::Above, "- c"),
            ["- a", "\t- b", ">- ", "- c"]
        );
        assert_eq!(
            added("P:\n\tn\n\tm\n\t\t- t", At::Below, "n"),
            ["P:", "\tn", "\tm", "\t\t- t", "\t>"],
            "after a note block's last line and its children, which end its run"
        );
    }

    // FR-MODE-04, OQ-03
    #[test]
    fn a_first_child_goes_under_the_last_line() {
        assert_eq!(
            added("P:\n\t- a", At::Child, "P:"),
            ["P:", "\t>- ", "\t- a"]
        );
        assert_eq!(added("n\nm", At::Child, "n"), ["n", "m", "\t>"]);
    }

    // OQ-03
    #[test]
    fn a_task_continues_a_task_and_everything_else_gets_a_note() {
        assert_eq!(added("- a", At::Below, "- a"), ["- a", ">- "]);
        assert_eq!(added("- a", At::Above, "- a"), [">- ", "- a"]);
        for other in ["P:", "---", "S @section", "f() @code\n\tx"] {
            let first = other.lines().next().unwrap();
            assert_eq!(added(other, At::Above, first)[0], ">", "{other:?}");
        }
        assert_eq!(added("P:", At::Below, "P:"), ["P:", ">"]);
    }

    // OQ-03
    #[test]
    fn a_child_of_a_project_or_task_is_a_task_else_a_note() {
        assert_eq!(added("P:", At::Child, "P:"), ["P:", "\t>- "]);
        assert_eq!(added("- a", At::Child, "- a"), ["- a", "\t>- "]);
        assert_eq!(added("---", At::Child, "---"), ["---", "\t>"]);
        assert_eq!(added("n", At::Child, "n"), ["n", "\t>"]);
    }

    // OQ-26
    #[test]
    fn a_note_that_would_join_a_note_run_is_a_task_instead() {
        assert_eq!(added("n", At::Below, "n"), ["n", ">- "]);
        assert_eq!(added("P:\nn", At::Below, "P:"), ["P:", ">- ", "n"]);
        assert_eq!(added("n\n\tm", At::Child, "n"), ["n", "\t>- ", "\tm"]);
        assert_eq!(
            added("---\nn", At::Child, "---"),
            ["---", "\t>", "n"],
            "a child does not join its parent's siblings"
        );
    }

    // FR-MODE-04
    #[test]
    fn the_first_block_of_an_empty_document_is_a_note() {
        let mut document = Document::default();
        assert_eq!(applied(&mut document, Place::Start), [">"]);
        let mut document = format::parse("n");
        assert_eq!(applied(&mut document, Place::Start), [">- ", "n"]);
    }

    // file-format §3.1
    #[test]
    fn an_empty_parent_gets_the_placeholder() {
        assert_eq!(added("a\n\n- t", At::Child, "a"), ["a", "…", "\t>", "- t"]);
    }

    // file-format §2, §3.1
    #[test]
    fn an_only_line_loses_its_leading_whitespace_once_it_has_company() {
        assert_eq!(added("  - a", At::Below, "  - a"), ["- a", ">- "]);
        assert_eq!(
            added("  - a", At::Child, "  - a"),
            ["- a", "\t>"],
            "it was a note, which the whitespace made it"
        );
    }

    // file-format §3.1
    #[test]
    fn an_empty_only_line_stays_empty_beside_a_new_block() {
        let mut document = format::parse("\n");
        let block = blocks::children(&document, document.root())[0].clone();
        assert_eq!(applied(&mut document, Place::Below(&block)), ["", ">- "]);
    }

    #[test]
    fn a_block_not_in_the_document_gets_no_new_block() {
        let other = format::parse("- x\n- y");
        let stale = block(&other, "- y");
        let mut document = format::parse("- a");
        assert_eq!(new_block(&mut document, Place::Below(&stale)), None);
        assert_eq!(new_block(&mut document, Place::Above(&stale)), None);
    }

    /// The lines of `text` once the block reading `of` is deleted, with the ops checked to undo.
    #[track_caller]
    fn deleted(text: &str, of: &str) -> Vec<String> {
        let mut document = format::parse(text);
        let block = block(&document, of);
        let mut transaction = Transaction::new(delete(&document, &block), (), ());
        document.apply(&mut transaction).unwrap();
        let after = lines(&document, None);
        document.apply(&mut transaction.inverse()).unwrap();
        assert_eq!(lines(&document, None), lines(&format::parse(text), None));
        after
    }

    // FR-OPS-06
    #[test]
    fn delete_takes_every_line_of_the_block_and_its_subtree() {
        assert_eq!(deleted("- a\n\t- b\n- c", "- a"), ["- c"]);
        assert_eq!(
            deleted("P:\n\tn\n\t\n\tm\n\t\t- t\n\t- u", "n"),
            ["P:", "\t- u"]
        );
        assert_eq!(
            deleted("n\n- t\nm", "- t"),
            ["n", "m"],
            "the notes around it now form one block"
        );
        assert_eq!(
            deleted("a\n\tb @hidden\n- c", "a"),
            ["- c"],
            "special nodes go with it"
        );
    }

    #[test]
    fn delete_of_a_block_not_in_the_document_does_nothing() {
        let other = format::parse("- x\n- y");
        let stale = block(&other, "- y");
        assert_eq!(delete(&format::parse("- a"), &stale), []);
    }
}
