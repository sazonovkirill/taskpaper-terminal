//! Adding, deleting, moving and grouping blocks as line ops: where a new block goes, which type it
//! takes and where a moved one lands ([editing.md §3–4](../../../docs/specs/editing.md#3-creating-blocks-fr-mode-04),
//! OQ-03, special-nodes.md §1).

use super::{
    blocks::{self, Block, BlockKind},
    format::{LineParts, Typed, canonical},
    model::{Document, ItemId, Op, Snapshot, Transaction},
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
    /// As the block's last child, before the special nodes that end its children.
    LastChild(&'a Block),
    /// As the first top-level block.
    Start,
}

/// An empty block at `place` that the user is about to type into: its line and the ops that put
/// it there, or `None` when `place` names a block that is not in `document`.
///
/// It has the type `typed` when one is given, else the type that fits where it goes. A line that
/// now has children but no text gets the placeholder, and so does a line that loses the leading
/// whitespace an only line keeps (file-format.md §3.1, rules 2 and 4).
#[must_use]
pub fn new_block(
    document: &mut Document,
    place: Place<'_>,
    typed: Option<Typed>,
) -> Option<(ItemId, Vec<Op>)> {
    let (parts, text) =
        LineParts::default().retyped(typed.unwrap_or_else(|| fitting_type(place)), "");
    new_line(document, place, parts.line(&text))
}

/// A block reading `line` at `place`, as Quick Add adds it (D-93): its id and the ops that put
/// it there, repairing what it breaks as [`new_block`] does, or `None` when `place` names a block
/// that is not in `document`.
#[must_use]
pub fn new_line(
    document: &mut Document,
    place: Place<'_>,
    line: String,
) -> Option<(ItemId, Vec<Op>)> {
    let (parent, index) = position(document, place)?;
    let mut ops = repairs(document, parent);
    let id = document.new_id();
    ops.push(Op::Insert {
        parent,
        index,
        subtree: Snapshot::line(id, line),
    });
    Some((id, ops))
}

/// The parent of a block at `place` and its position among the parent's children, or `None`
/// when `place` names a block that is not in `document`.
fn position(document: &Document, place: Place<'_>) -> Option<(ItemId, usize)> {
    Some(match place {
        Place::Below(block) => {
            let (parent, index) = document.position(block.id())?;
            (parent, index + 1)
        }
        Place::Above(block) => document.position(block.id())?,
        Place::FirstChild(block) => {
            document.get(block.id())?;
            (block.id(), 0)
        }
        Place::LastChild(block) => {
            let children = document.get(block.id())?.children();
            (block.id(), end(document, children))
        }
        Place::Start => (document.root(), 0),
    })
}

/// The top-level lines of `pasted`, a document read from pasted text, with their subtrees put at
/// `place` in order under fresh ids: their new lines and the ops that put them there, or `None`
/// when `place` names a block that is not in `document` (FR-OPS-06).
#[must_use]
pub fn paste(
    document: &mut Document,
    place: Place<'_>,
    pasted: &Document,
) -> Option<(Vec<ItemId>, Vec<Op>)> {
    let (parent, index) = position(document, place)?;
    let copies: Vec<Snapshot> = pasted
        .get(pasted.root())?
        .children()
        .iter()
        .filter_map(|&top| Some(document.fresh_copy(&pasted.snapshot(top)?)))
        .collect();
    if copies.is_empty() {
        return Some((Vec::new(), Vec::new()));
    }
    let tops = copies.iter().map(Snapshot::id).collect();
    Some((tops, insert(document, parent, index, copies)))
}

/// The type of an empty block at `place` whose type the user did not pick: a task after or
/// before a task and under a project or task, else text (OQ-03).
fn fitting_type(place: Place<'_>) -> Typed {
    let task = match place {
        Place::Below(block) | Place::Above(block) => block.kind() == BlockKind::Task,
        Place::FirstChild(block) | Place::LastChild(block) => {
            matches!(block.kind(), BlockKind::Project | BlockKind::Task)
        }
        Place::Start => false,
    };
    if task { Typed::Task } else { Typed::Text }
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

/// The op that takes `block` out of `document` with its subtree (FR-OPS-06), or `None` when it is
/// not in `document`.
#[must_use]
pub fn delete(document: &Document, block: &Block) -> Option<Op> {
    let (parent, index) = document.position(block.id())?;
    Some(Op::Remove {
        parent,
        index,
        subtree: document.snapshot(block.id())?,
    })
}

/// Copies of `blocks`, none inside another and in document order, each with its subtree under
/// fresh ids right after the original's subtree: the copies' lines in that order and the ops that
/// put them there, or `None` when one of the blocks is not in `document` (FR-OPS-06).
#[must_use]
pub fn duplicate(document: &mut Document, blocks: &[Block]) -> Option<(Vec<ItemId>, Vec<Op>)> {
    let mut copies = Vec::new();
    let mut ops = Vec::new();
    // Last first, so that putting one in leaves the places of those before it as they are.
    for block in blocks.iter().rev() {
        let (parent, index) = document.position(block.id())?;
        let copy = document.fresh_copy(&document.snapshot(block.id())?);
        copies.push(copy.id());
        ops.extend(insert(document, parent, index + 1, vec![copy]));
    }
    copies.reverse();
    Some((copies, ops))
}

/// The ops that put `subtrees` one after another from `index` among the children of `parent`,
/// with the [`repairs`] that needs, and then the lines put in repaired where they would be read
/// back elsewhere: a copy of a document's only line, which may start with whitespace, does once
/// it is no longer the only line (file-format.md §3.1, rules 2 and 4).
///
/// # Panics
///
/// Never: the ops are built from the document as it is, with ids it made for them.
fn insert(document: &Document, parent: ItemId, index: usize, subtrees: Vec<Snapshot>) -> Vec<Op> {
    let tops: Vec<ItemId> = subtrees.iter().map(Snapshot::id).collect();
    let mut ops = repairs(document, parent);
    ops.extend(
        subtrees
            .into_iter()
            .zip(index..)
            .map(|(subtree, index)| Op::Insert {
                parent,
                index,
                subtree,
            }),
    );
    let mut after = document.clone();
    after
        .apply(&mut Transaction::new(ops.clone(), (), ()))
        .expect("the ops are built from the document as it is, with ids it made for them");
    let only_line = after.descendants(after.root()).nth(1).is_none();
    let lines = tops
        .iter()
        .flat_map(|&top| std::iter::once(top).chain(after.descendants(top).map(|(id, _)| id)));
    let fixes: Vec<Op> = lines
        .filter_map(|id| {
            let item = after.get(id)?;
            let fixed = canonical::repaired(item.text(), !item.children().is_empty(), only_line);
            (fixed != item.text()).then(|| Op::SetText {
                id,
                before: item.text().to_owned(),
                after: fixed,
            })
        })
        .collect();
    ops.extend(fixes);
    ops
}

/// Where a block moves among the blocks around it (editing.md §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shift {
    /// To the end of the previous sibling's children (FR-OPS-02).
    Indent,
    /// To just after its parent, leaving the siblings after it where they are (FR-OPS-02).
    Outdent,
    /// Before the previous sibling, or at the first position to the end of the children of the
    /// parent's previous sibling (FR-OPS-03).
    Up,
    /// After the next sibling, or at the last position to the start of the children of the
    /// parent's next sibling (FR-OPS-03).
    Down,
}

/// The ops that move `block` with its subtree as `shift` says without leaving the children of
/// the line `top`, the root or the zoom block; `None` when it has nowhere to go
/// there or is not in `document`. A line that gets children but has no text gets the
/// placeholder (file-format.md §3.1, rule 2).
#[must_use]
pub fn shift(document: &Document, block: &Block, shift: Shift, top: ItemId) -> Option<Vec<Op>> {
    let parent = document.get(block.id())?.parent()?;
    let siblings = blocks::children(document, parent);
    let at = siblings
        .iter()
        .position(|sibling| sibling.id() == block.id())?;
    let previous = at.checked_sub(1).and_then(|at| siblings.get(at));
    let next = siblings.get(at + 1);
    let (to, spot) = match shift {
        Shift::Indent => (previous?.id(), Spot::End),
        Shift::Outdent if parent != top => (document.get(parent)?.parent()?, Spot::After(parent)),
        Shift::Outdent => return None,
        Shift::Up => match previous {
            Some(previous) => (parent, Spot::Before(previous.id())),
            None => (neighbor(document, parent, top, -1)?, Spot::End),
        },
        Shift::Down => match next {
            Some(next) => (parent, Spot::After(next.id())),
            None => (neighbor(document, parent, top, 1)?, Spot::Start),
        },
    };
    let mut ops = repairs(document, to);
    ops.push(relocate(document, block.id(), to, spot)?);
    Some(ops)
}

/// Where among a line's children a moved line goes.
#[derive(Debug, Clone, Copy)]
enum Spot {
    Start,
    /// After the last child that is not a special node.
    End,
    Before(ItemId),
    After(ItemId),
}

/// The ops that shift each of `blocks`, none inside another and in document order, as [`shift`]
/// does, one after another so that they keep their order: last first when they move out or down,
/// and a block stays when the neighbor it would pass is one of them that stayed (FR-OPS-02,
/// FR-OPS-03, FR-OPS-07). Empty when none of them moves.
///
/// # Panics
///
/// Never: each block's ops are built from a copy of the document as the ops before them left it.
#[must_use]
pub fn shift_all(document: &Document, blocks: &[Block], by: Shift, top: ItemId) -> Vec<Op> {
    let mut order: Vec<&Block> = blocks.iter().collect();
    if matches!(by, Shift::Outdent | Shift::Down) {
        order.reverse();
    }
    let passes = match by {
        Shift::Indent | Shift::Up => Some(-1),
        Shift::Down => Some(1),
        Shift::Outdent => None,
    };
    let mut moved = document.clone();
    let mut stayed = Vec::new();
    let mut ops = Vec::new();
    for block in order {
        let blocked = passes
            .and_then(|by| neighbor(&moved, block.id(), top, by))
            .is_some_and(|passed| stayed.contains(&passed));
        match shift(&moved, block, by, top).filter(|_| !blocked) {
            Some(step) => {
                moved
                    .apply(&mut Transaction::new(step.clone(), (), ()))
                    .expect("the ops are built from the copy as it is");
                ops.extend(step);
            }
            None => stayed.push(block.id()),
        }
    }
    ops
}

/// A new empty text block where the first of `blocks` is, with `blocks`, none inside another and
/// in document order, moved under it in that order: its line and the ops that do this, or `None`
/// when `blocks` is empty or one of them is not in `document` (FR-OPS-04). The new line has
/// children but no text until the user types one (file-format.md §3.1, rule 2).
///
/// # Panics
///
/// Never: each op is built from a copy of the document as the ops before it left it.
#[must_use]
pub fn group(document: &mut Document, blocks: &[Block]) -> Option<(ItemId, Vec<Op>)> {
    let (id, ops) = new_block(document, Place::Above(blocks.first()?), Some(Typed::Text))?;
    Some((id, append(document, ops, blocks, id)?))
}

/// The ops that move `blocks`, none inside another and in document order, with their subtrees
/// to the end of the children of the block `to`, or of the top level when `to` is `None`, before
/// the special nodes that end them, in that order (FR-OPS-05, D-56); `None` when `to` is one of
/// them or inside one, or a block is not in `document`. A line that gets children but has no
/// text gets the placeholder (file-format.md §3.1, rule 2).
#[must_use]
pub fn move_to(document: &Document, blocks: &[Block], to: Option<&Block>) -> Option<Vec<Op>> {
    let to = to.map_or(document.root(), Block::id);
    document.get(to)?;
    let mut above = std::iter::successors(Some(to), |&line| document.get(line)?.parent());
    if above.any(|line| blocks.iter().any(|block| block.id() == line)) {
        return None;
    }
    append(document, repairs(document, to), blocks, to)
}

/// `ops`, then the ops that move each of `blocks` in turn with its subtree to the end of the
/// children of the line `to` as `ops` leave `document`; `None` when a block is not in it.
///
/// # Panics
///
/// Never: each op is built from a copy of the document as the ops before it left it, and `to`
/// is in no block's subtree.
fn append(document: &Document, mut ops: Vec<Op>, blocks: &[Block], to: ItemId) -> Option<Vec<Op>> {
    let mut moved = document.clone();
    let mut step = ops.clone();
    for block in blocks {
        moved
            .apply(&mut Transaction::new(step, (), ()))
            .expect("the ops are built from the copy as it is");
        let op = relocate(&moved, block.id(), to, Spot::End)?;
        ops.push(op.clone());
        step = vec![op];
    }
    Some(ops)
}

/// The block `by` places after (or before, when negative) the block `line`, or `None` when
/// `line` is `top` or has no such neighbor.
fn neighbor(document: &Document, line: ItemId, top: ItemId, by: isize) -> Option<ItemId> {
    if line == top {
        return None;
    }
    let siblings = blocks::children(document, document.get(line)?.parent()?);
    let at = siblings.iter().position(|sibling| sibling.id() == line)?;
    Some(siblings.get(at.checked_add_signed(by)?)?.id())
}

/// The op moving `line` with its subtree to `spot` among the children of the line `to`, the end
/// being before the special nodes that end the children; `None` when the line or the spot is not
/// in `document`.
fn relocate(document: &Document, line: ItemId, to: ItemId, spot: Spot) -> Option<Op> {
    let from = document.position(line)?;
    let mut targets = document.get(to)?.children().to_vec();
    if from.0 == to {
        targets.remove(from.1);
    }
    let find = |id: ItemId| targets.iter().position(|&target| target == id);
    let index = match spot {
        Spot::Start => 0,
        Spot::End => end(document, &targets),
        Spot::Before(id) => find(id)?,
        Spot::After(id) => find(id)? + 1,
    };
    Some(Op::Move {
        id: line,
        from,
        to: (to, index),
    })
}

/// The index after the last of `children` that is not a special node, so that the special nodes
/// ending them stay last (special-nodes.md §1).
fn end(document: &Document, children: &[ItemId]) -> usize {
    children
        .iter()
        .rposition(|&id| {
            document
                .get(id)
                .is_some_and(|item| !blocks::is_special(item))
        })
        .map_or(0, |last| last + 1)
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

    /// The block whose line reads `text`.
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
        FirstChild,
        LastChild,
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
            At::FirstChild => Place::FirstChild(&block),
            At::LastChild => Place::LastChild(&block),
        };
        applied(&mut document, place)
    }

    #[track_caller]
    fn applied(document: &mut Document, place: Place<'_>) -> Vec<String> {
        applied_as(document, place, None)
    }

    #[track_caller]
    fn applied_as(document: &mut Document, place: Place<'_>, typed: Option<Typed>) -> Vec<String> {
        let before = lines(document, None);
        let (id, ops) = new_block(document, place, typed).unwrap();
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
            added("P:\n\tn\n\t\t- t\n\tm", At::Below, "n"),
            ["P:", "\tn", "\t\t- t", "\t>", "\tm"],
            "after its children"
        );
    }

    // FR-MODE-04, OQ-03
    #[test]
    fn a_first_child_goes_first_under_the_block() {
        assert_eq!(
            added("P:\n\t- a", At::FirstChild, "P:"),
            ["P:", "\t>- ", "\t- a"]
        );
        assert_eq!(added("n\nm", At::FirstChild, "n"), ["n", "\t>", "m"]);
    }

    // FR-MODE-04, special-nodes.md §1
    #[test]
    fn a_last_child_goes_after_the_children_but_before_special_nodes() {
        assert_eq!(
            added("P:\n\t- a\n\t\t- b", At::LastChild, "P:"),
            ["P:", "\t- a", "\t\t- b", "\t>- "]
        );
        assert_eq!(
            added("- t\n\t- a\n\tC @hidden @comments", At::LastChild, "- t"),
            ["- t", "\t- a", "\t>- ", "\tC @hidden @comments"]
        );
        assert_eq!(added("n\n\tm", At::LastChild, "n"), ["n", "\tm", "\t>"]);
    }

    // D-93
    #[test]
    fn a_new_line_keeps_its_text_and_goes_before_special_nodes() {
        let mut document = format::parse("- t\n\tC @hidden @comments\nn");
        let task = block(&document, "- t");
        let (id, ops) = new_line(
            &mut document,
            Place::LastChild(&task),
            "- x @due(2026-10-08)".to_owned(),
        )
        .unwrap();
        let mut transaction = Transaction::new(ops, (), ());
        document.apply(&mut transaction).unwrap();
        assert_eq!(
            lines(&document, Some(id)),
            [
                "- t",
                "\t>- x @due(2026-10-08)",
                "\tC @hidden @comments",
                "n"
            ]
        );
        document.apply(&mut transaction.inverse()).unwrap();
        assert_eq!(
            lines(&document, None),
            ["- t", "\tC @hidden @comments", "n"]
        );
    }

    // OQ-03
    #[test]
    fn a_task_continues_a_task_and_everything_else_gets_a_note() {
        assert_eq!(added("- a", At::Below, "- a"), ["- a", ">- "]);
        assert_eq!(added("- a", At::Above, "- a"), [">- ", "- a"]);
        for other in ["P:", "---", "S @section", "n\n\tx"] {
            let first = other.lines().next().unwrap();
            assert_eq!(added(other, At::Above, first)[0], ">", "{other:?}");
        }
        assert_eq!(added("P:", At::Below, "P:"), ["P:", ">"]);
        assert_eq!(added("n", At::Below, "n"), ["n", ">"], "text after text");
    }

    // OQ-03
    #[test]
    fn a_child_of_a_project_or_task_is_a_task_else_a_note() {
        assert_eq!(added("P:", At::FirstChild, "P:"), ["P:", "\t>- "]);
        assert_eq!(added("- a", At::FirstChild, "- a"), ["- a", "\t>- "]);
        assert_eq!(added("---", At::FirstChild, "---"), ["---", "\t>"]);
        assert_eq!(added("n", At::FirstChild, "n"), ["n", "\t>"]);
    }

    // FR-MODE-04
    #[test]
    fn the_first_block_of_an_empty_document_is_text() {
        let mut document = Document::default();
        assert_eq!(applied(&mut document, Place::Start), [">"]);
        let mut document = format::parse("n");
        assert_eq!(applied(&mut document, Place::Start), [">", "n"]);
    }

    // FR-MODE-06
    #[test]
    fn a_typed_block_has_its_type_wherever_it_goes() {
        let mut document = format::parse("n\n\tm\n- t");
        let note = block(&document, "n");
        let task = block(&document, "- t");
        for (typed, line, kind) in [
            (Typed::Project, ":", BlockKind::Project),
            (Typed::Section, "@section", BlockKind::Text),
            (Typed::Task, "- ", BlockKind::Task),
        ] {
            for place in [
                Place::Below(&note),
                Place::Above(&task),
                Place::FirstChild(&note),
                Place::LastChild(&note),
                Place::Start,
            ] {
                let (id, ops) = new_block(&mut document, place, Some(typed)).unwrap();
                let mut transaction = Transaction::new(ops, (), ());
                document.apply(&mut transaction).unwrap();
                assert_eq!(document.get(id).unwrap().text(), line, "{place:?}");
                let block = blocks::block(&document, id).unwrap();
                assert_eq!(block.kind(), kind);
                assert_eq!(block.section(), typed == Typed::Section);
                document.apply(&mut transaction.inverse()).unwrap();
            }
        }
    }

    // file-format §3.1
    #[test]
    fn a_typed_block_repairs_its_parent_too() {
        let mut document = format::parse("a\n\n- t");
        let blank = blocks::children(&document, document.root())[1].clone();
        assert_eq!(
            applied_as(
                &mut document,
                Place::FirstChild(&blank),
                Some(Typed::Section)
            ),
            ["a", "…", "\t>@section", "- t"]
        );
    }

    // file-format §3.1
    #[test]
    fn an_empty_parent_gets_the_placeholder() {
        assert_eq!(added("\n- t", At::FirstChild, ""), ["…", "\t>", "- t"]);
    }

    // file-format §2, §3.1
    #[test]
    fn an_only_line_loses_its_leading_whitespace_once_it_has_company() {
        assert_eq!(added("  - a", At::Below, "  - a"), ["- a", ">"]);
        assert_eq!(
            added("  - a", At::FirstChild, "  - a"),
            ["- a", "\t>"],
            "it was a note, which the whitespace made it"
        );
    }

    // file-format §3.1
    #[test]
    fn an_empty_only_line_stays_empty_beside_a_new_block() {
        let mut document = format::parse("\n");
        let block = blocks::children(&document, document.root())[0].clone();
        assert_eq!(applied(&mut document, Place::Below(&block)), ["", ">"]);
    }

    #[test]
    fn a_block_not_in_the_document_gets_no_new_block() {
        let other = format::parse("- x\n- y");
        let stale = block(&other, "- y");
        let mut document = format::parse("- a");
        assert_eq!(new_block(&mut document, Place::Below(&stale), None), None);
        assert_eq!(new_block(&mut document, Place::Above(&stale), None), None);
        assert_eq!(
            new_block(&mut document, Place::LastChild(&stale), None),
            None
        );
        assert_eq!(
            new_block(&mut document, Place::FirstChild(&stale), None),
            None
        );
    }

    /// The lines of `text` once the block reading `of` is deleted, with the ops checked to undo.
    #[track_caller]
    fn deleted(text: &str, of: &str) -> Vec<String> {
        let mut document = format::parse(text);
        let block = block(&document, of);
        let mut transaction = Transaction::new(vec![delete(&document, &block).unwrap()], (), ());
        document.apply(&mut transaction).unwrap();
        let after = lines(&document, None);
        document.apply(&mut transaction.inverse()).unwrap();
        assert_eq!(lines(&document, None), lines(&format::parse(text), None));
        after
    }

    // FR-OPS-06
    #[test]
    fn delete_takes_the_line_and_its_subtree() {
        assert_eq!(deleted("- a\n\t- b\n- c", "- a"), ["- c"]);
        assert_eq!(deleted("P:\n\tn\n\t\t- t\n\tm", "n"), ["P:", "\tm"]);
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
        assert_eq!(delete(&format::parse("- a"), &stale), None);
    }

    /// The lines of `text` once the block reading `of` shifts below the line reading `top`
    /// (the root when `None`), with the ops checked to undo; `None` when it does not move.
    #[track_caller]
    fn shifted_below(text: &str, of: &str, by: Shift, top: Option<&str>) -> Option<Vec<String>> {
        let mut document = format::parse(text);
        let top = top.map_or(document.root(), |top| block(&document, top).id());
        let ops = shift(&document, &block(&document, of), by, top)?;
        let mut transaction = Transaction::new(ops, (), ());
        document.apply(&mut transaction).unwrap();
        let after = lines(&document, None);
        document.apply(&mut transaction.inverse()).unwrap();
        assert_eq!(lines(&document, None), lines(&format::parse(text), None));
        Some(after)
    }

    #[track_caller]
    fn shifted(text: &str, of: &str, by: Shift) -> Option<Vec<String>> {
        shifted_below(text, of, by, None)
    }

    // FR-OPS-02
    #[test]
    fn indent_makes_the_block_the_last_child_of_the_previous_sibling() {
        assert_eq!(
            shifted("- a\n\t- x\n- b\n\t- c\n- d", "- b", Shift::Indent).unwrap(),
            ["- a", "\t- x", "\t- b", "\t\t- c", "- d"]
        );
        assert_eq!(
            shifted("n\nm\n- t", "- t", Shift::Indent).unwrap(),
            ["n", "m", "\t- t"],
            "under the line before it only"
        );
        assert_eq!(shifted("- a\n- b", "- a", Shift::Indent), None, "first");
    }

    // FR-OPS-02, file-format §3.1
    #[test]
    fn a_blank_line_that_gets_children_gets_the_placeholder() {
        assert_eq!(
            shifted("n\n\n- t", "- t", Shift::Indent).unwrap(),
            ["n", "…", "\t- t"]
        );
    }

    // FR-OPS-02
    #[test]
    fn outdent_puts_the_block_after_its_parent_and_leaves_the_siblings_after_it() {
        assert_eq!(
            shifted(
                "P:\n\t- a\n\t- b\n\t\t- c\n\t- d\nQ:",
                "- b",
                Shift::Outdent
            )
            .unwrap(),
            ["P:", "\t- a", "\t- d", "- b", "\t- c", "Q:"]
        );
        assert_eq!(shifted("- a\n\t- b", "- a", Shift::Outdent), None);
        assert_eq!(
            shifted_below("P:\n\t- a", "- a", Shift::Outdent, Some("P:")),
            None,
            "not out of the zoom"
        );
    }

    // FR-OPS-03
    #[test]
    fn up_and_down_swap_the_block_with_its_neighbor() {
        let text = "- a\n- b\n\t- c\n- d";
        assert_eq!(
            shifted(text, "- b", Shift::Up).unwrap(),
            ["- b", "\t- c", "- a", "- d"]
        );
        assert_eq!(
            shifted(text, "- b", Shift::Down).unwrap(),
            ["- a", "- d", "- b", "\t- c"]
        );
        assert_eq!(
            shifted("- a\nn\nm\n- d", "n", Shift::Down).unwrap(),
            ["- a", "m", "n", "- d"],
            "past one line of text"
        );
        assert_eq!(shifted("- a\n- b", "- a", Shift::Up), None);
        assert_eq!(shifted("- a\n- b", "- b", Shift::Down), None);
    }

    // FR-OPS-03
    #[test]
    fn at_the_edge_the_block_crosses_into_the_parents_neighbor() {
        let text = "P:\n\t- a\nQ:\n\t- b\n\t- c";
        assert_eq!(
            shifted(text, "- b", Shift::Up).unwrap(),
            ["P:", "\t- a", "\t- b", "Q:", "\t- c"],
            "to the end of the previous one"
        );
        assert_eq!(
            shifted(text, "- a", Shift::Down).unwrap(),
            ["P:", "Q:", "\t- a", "\t- b", "\t- c"],
            "to the start of the next one"
        );
        assert_eq!(shifted(text, "- c", Shift::Down), None, "no next one");
        assert_eq!(shifted("P:\n\t- a", "- a", Shift::Up), None);
        assert_eq!(
            shifted_below(text, "- b", Shift::Up, Some("Q:")),
            None,
            "not out of the zoom"
        );
    }

    // FR-SPC-01, special-nodes.md §1
    #[test]
    fn special_nodes_stay_where_they_are_and_last() {
        assert_eq!(
            shifted("- a\nh @hidden\n- b", "- b", Shift::Up).unwrap(),
            ["- b", "- a", "h @hidden"]
        );
        assert_eq!(
            shifted("- a\n\th @hidden\n- b", "- b", Shift::Indent).unwrap(),
            ["- a", "\t- b", "\th @hidden"],
            "the last child before the trailing ones"
        );
        assert_eq!(
            shifted("P:\n\t- a\n\th @hidden\nQ:\n\t- b", "- b", Shift::Up).unwrap(),
            ["P:", "\t- a", "\t- b", "\th @hidden", "Q:"]
        );
        assert_eq!(
            shifted("P:\n\th @hidden\n- b", "- b", Shift::Indent).unwrap(),
            ["P:", "\t- b", "\th @hidden"]
        );
        assert_eq!(
            shifted("P:\n\tg @hidden\n\t- a\nQ:\n\t- b", "- b", Shift::Up).unwrap(),
            ["P:", "\tg @hidden", "\t- a", "\t- b", "Q:"],
            "one placed elsewhere is left there"
        );
    }

    #[test]
    fn a_block_not_in_the_document_does_not_shift() {
        let other = format::parse("- x\n- y");
        let stale = block(&other, "- y");
        let document = format::parse("- a");
        for by in [Shift::Indent, Shift::Outdent, Shift::Up, Shift::Down] {
            assert_eq!(shift(&document, &stale, by, document.root()), None);
        }
    }

    /// The lines of `text` once the blocks reading `of` all shift together, with the ops checked
    /// to undo.
    #[track_caller]
    fn all_shifted(text: &str, of: &[&str], by: Shift) -> Vec<String> {
        let mut document = format::parse(text);
        let blocks: Vec<Block> = of.iter().map(|of| block(&document, of)).collect();
        let ops = shift_all(&document, &blocks, by, document.root());
        let mut transaction = Transaction::new(ops, (), ());
        document.apply(&mut transaction).unwrap();
        let after = lines(&document, None);
        document.apply(&mut transaction.inverse()).unwrap();
        assert_eq!(lines(&document, None), lines(&format::parse(text), None));
        after
    }

    // FR-OPS-02, FR-OPS-07
    #[test]
    fn many_blocks_indent_and_outdent_together_in_order() {
        let text = "- x\n- a\n- b\n- y\n- c";
        assert_eq!(
            all_shifted(text, &["- a", "- b", "- c"], Shift::Indent),
            ["- x", "\t- a", "\t- b", "- y", "\t- c"]
        );
        assert_eq!(
            all_shifted("- a\n- b\n- c", &["- a", "- b"], Shift::Indent),
            ["- a", "- b", "- c"],
            "the first cannot, so the one after it stays beside it"
        );
        assert_eq!(
            all_shifted("P:\n\t- a\n\t- b\n\t- c", &["- a", "- b"], Shift::Outdent),
            ["P:", "\t- c", "- a", "- b"]
        );
    }

    // FR-OPS-03, FR-OPS-07
    #[test]
    fn many_blocks_move_up_and_down_together_in_order() {
        let text = "- x\n- a\n- b\n- y";
        assert_eq!(
            all_shifted(text, &["- a", "- b"], Shift::Up),
            ["- a", "- b", "- x", "- y"]
        );
        assert_eq!(
            all_shifted(text, &["- a", "- b"], Shift::Down),
            ["- x", "- y", "- a", "- b"]
        );
        assert_eq!(
            all_shifted("- a\n- b\n- c", &["- a", "- b"], Shift::Up),
            ["- a", "- b", "- c"],
            "nothing passes the first that stayed"
        );
        assert_eq!(
            all_shifted("- a\n- b\n- c", &["- b", "- c"], Shift::Down),
            ["- a", "- b", "- c"],
            "nor the last that stayed"
        );
        assert_eq!(
            all_shifted("P:\n\t- x\nQ:\n\t- a\n\t- b", &["- a", "- b"], Shift::Up),
            ["P:", "\t- x", "\t- a", "\t- b", "Q:"],
            "across into the parent's neighbor"
        );
        assert_eq!(
            all_shifted("P:\n\t- a\n\t- b\nQ:\n\t- x", &["- a", "- b"], Shift::Down),
            ["P:", "Q:", "\t- a", "\t- b", "\t- x"]
        );
    }

    #[test]
    fn no_blocks_shift_without_ops() {
        let document = format::parse("- a");
        assert_eq!(shift_all(&document, &[], Shift::Up, document.root()), []);
    }

    /// The lines of `text` once the blocks reading `of` are grouped, with the new line marked
    /// and the ops checked to undo; `None` when nothing is grouped.
    #[track_caller]
    fn grouped(text: &str, of: &[&str]) -> Option<Vec<String>> {
        let mut document = format::parse(text);
        let blocks: Vec<Block> = of.iter().map(|of| block(&document, of)).collect();
        let (id, ops) = group(&mut document, &blocks)?;
        let mut transaction = Transaction::new(ops, (), ());
        document.apply(&mut transaction).unwrap();
        let after = lines(&document, Some(id));
        document.apply(&mut transaction.inverse()).unwrap();
        assert_eq!(lines(&document, None), lines(&format::parse(text), None));
        Some(after)
    }

    // FR-OPS-04
    #[test]
    fn group_puts_the_blocks_under_a_new_text_block_where_the_first_was() {
        assert_eq!(
            grouped("- x\n- a\n\t- c\n- y\n- b", &["- a", "- b"]).unwrap(),
            ["- x", ">", "\t- a", "\t\t- c", "\t- b", "- y"]
        );
        assert_eq!(
            grouped("P:\n\t- a\nQ:\n\t- b", &["- a", "- b"]).unwrap(),
            ["P:", "\t>", "\t\t- a", "\t\t- b", "Q:"],
            "from different lists, where the first was"
        );
        assert_eq!(grouped("- a", &[]), None);
    }

    #[test]
    fn a_group_with_a_block_not_in_the_document_is_not_made() {
        let other = format::parse("- x\n- y\n- z\n- w");
        let mut document = format::parse("- a");
        let blocks = [block(&document, "- a"), block(&other, "- w")];
        assert_eq!(group(&mut document, &blocks[1..]), None);
        assert_eq!(group(&mut document, &blocks), None);
    }

    /// The lines of `text` once the blocks reading `of` move under the block reading `to`, with
    /// the ops checked to undo; `None` when nothing moves.
    #[track_caller]
    fn moved_to(text: &str, of: &[&str], to: &str) -> Option<Vec<String>> {
        let mut document = format::parse(text);
        let blocks: Vec<Block> = of.iter().map(|of| block(&document, of)).collect();
        let ops = move_to(&document, &blocks, Some(&block(&document, to)))?;
        let mut transaction = Transaction::new(ops, (), ());
        document.apply(&mut transaction).unwrap();
        let after = lines(&document, None);
        document.apply(&mut transaction.inverse()).unwrap();
        assert_eq!(lines(&document, None), lines(&format::parse(text), None));
        Some(after)
    }

    // FR-OPS-05, FR-OPS-07
    #[test]
    fn move_to_makes_the_blocks_the_last_children_of_the_destination_in_order() {
        assert_eq!(
            moved_to(
                "- a
	- c
P:
	- x
- b",
                &["- a", "- b"],
                "P:"
            )
            .unwrap(),
            ["P:", "	- x", "	- a", "		- c", "	- b"]
        );
        assert_eq!(
            moved_to(
                "P:
	- a
		- c
	- x
Q:
	- b",
                &["- a", "- b"],
                "- x"
            )
            .unwrap(),
            ["P:", "	- x", "		- a", "			- c", "		- b", "Q:"],
            "from different lists and depths"
        );
        assert_eq!(
            moved_to(
                "P:
	- a
	- x",
                &["- a"],
                "P:"
            )
            .unwrap(),
            ["P:", "	- x", "	- a"],
            "to the end of its own list"
        );
    }

    // FR-OPS-05, special-nodes.md §1
    #[test]
    fn moved_blocks_go_before_the_special_nodes_ending_the_children() {
        assert_eq!(
            moved_to(
                "P:
	- x
	h @hidden
- a",
                &["- a"],
                "P:"
            )
            .unwrap(),
            ["P:", "	- x", "	- a", "	h @hidden"]
        );
    }

    // FR-OPS-05, D-56, special-nodes.md §1
    #[test]
    fn move_to_the_top_level_makes_the_blocks_the_last_top_level_blocks() {
        let mut document = format::parse("P:\n\tQ:\n\t\t- x\n\tR:\nS:\nh @hidden");
        let blocks = [block(&document, "Q:"), block(&document, "R:")];
        let ops = move_to(&document, &blocks, None).unwrap();
        let mut transaction = Transaction::new(ops, (), ());
        document.apply(&mut transaction).unwrap();
        assert_eq!(
            lines(&document, None),
            ["P:", "S:", "Q:", "\t- x", "R:", "h @hidden"]
        );
        document.apply(&mut transaction.inverse()).unwrap();
        assert_eq!(lines(&document, None)[1], "\tQ:");
    }

    // file-format.md §3.1
    #[test]
    fn a_blank_destination_gets_the_placeholder() {
        assert_eq!(
            moved_to(
                "n

- t",
                &["- t"],
                ""
            )
            .unwrap(),
            ["n", "…", "	- t"]
        );
    }

    #[test]
    fn nothing_moves_into_itself_or_a_block_it_holds() {
        let text = "- a
	- c
		- d
- b";
        assert_eq!(moved_to(text, &["- a"], "- a"), None);
        assert_eq!(moved_to(text, &["- b", "- a"], "- d"), None);
        assert_eq!(moved_to(text, &["- a"], "- b").unwrap().len(), 4);
    }

    #[test]
    fn nothing_moves_to_or_from_a_block_not_in_the_document() {
        let other = format::parse(
            "- x
- y
- z
- w",
        );
        let document = format::parse(
            "- a
- b",
        );
        let (a, b, w) = (
            block(&document, "- a"),
            block(&document, "- b"),
            block(&other, "- w"),
        );
        assert_eq!(move_to(&document, std::slice::from_ref(&a), Some(&w)), None);
        assert_eq!(move_to(&document, std::slice::from_ref(&w), None), None);
        assert_eq!(move_to(&document, &[a, w], Some(&b)), None);
    }

    /// The lines of `text` once the blocks reading `of` are duplicated, with the copies marked
    /// and the ops checked to undo; `None` when one of them is not found.
    #[track_caller]
    fn duplicated(text: &str, of: &[&str]) -> Vec<String> {
        let mut document = format::parse(text);
        let blocks: Vec<Block> = of.iter().map(|of| block(&document, of)).collect();
        let (copies, ops) = duplicate(&mut document, &blocks).unwrap();
        assert_eq!(copies.len(), blocks.len());
        let mut transaction = Transaction::new(ops, (), ());
        document.apply(&mut transaction).unwrap();
        let after: Vec<String> = document
            .descendants(document.root())
            .map(|(id, depth)| {
                let mark = if copies.contains(&id) { ">" } else { "" };
                let text = document.get(id).unwrap().text();
                format!("{}{mark}{text}", "\t".repeat(depth))
            })
            .collect();
        document.apply(&mut transaction.inverse()).unwrap();
        assert_eq!(lines(&document, None), lines(&format::parse(text), None));
        after
    }

    // FR-OPS-06
    #[test]
    fn a_duplicate_with_its_subtree_goes_after_the_original() {
        assert_eq!(
            duplicated("- x\nP: @a\n\t- b\n\t\tn\n\tC @hidden\n- y", &["P: @a"]),
            [
                "- x",
                "P: @a",
                "\t- b",
                "\t\tn",
                "\tC @hidden",
                ">P: @a",
                "\t- b",
                "\t\tn",
                "\tC @hidden",
                "- y",
            ]
        );
    }

    // FR-OPS-06, FR-OPS-07
    #[test]
    fn each_of_several_blocks_is_duplicated_after_itself() {
        assert_eq!(
            duplicated("- a\n- b\nP:\n\t- c", &["- a", "- b", "- c"]),
            ["- a", ">- a", "- b", ">- b", "P:", "\t- c", "\t>- c"]
        );
    }

    // file-format.md §3.1, rule 4
    #[test]
    fn the_copy_of_an_only_line_drops_its_leading_whitespace_with_the_original() {
        assert_eq!(duplicated("  x", &["  x"]), ["x", ">x"]);
        assert_eq!(
            duplicated("a\n\nb", &[""]),
            ["a", "", ">", "b"],
            "a blank line stays"
        );
    }

    #[test]
    fn copies_get_ids_of_their_own_and_the_lines_they_copy_stay() {
        let mut document = format::parse("- a\n\t- b");
        let a = block(&document, "- a");
        let (copies, ops) = duplicate(&mut document, std::slice::from_ref(&a)).unwrap();
        document.apply(&mut Transaction::new(ops, (), ())).unwrap();
        assert_ne!(copies, [a.id()]);
        let ids: Vec<ItemId> = document
            .descendants(document.root())
            .map(|(id, _)| id)
            .collect();
        assert_eq!(ids.len(), 4);
        assert!(
            ids.iter()
                .all(|id| ids.iter().filter(|other| *other == id).count() == 1)
        );
        assert_eq!(document.get(a.id()).unwrap().text(), "- a");
    }

    #[test]
    fn a_block_not_in_the_document_is_not_duplicated() {
        let other = format::parse("- x\n- y");
        let mut document = format::parse("- a");
        let blocks = [block(&document, "- a"), block(&other, "- y")];
        assert_eq!(duplicate(&mut document, &blocks), None);
    }

    /// The lines of `text` once `pasted` is pasted `at` the block reading `of`, or at the start
    /// without one, with the pasted top lines marked and the ops checked to undo.
    #[track_caller]
    fn pasted(text: &str, at: At, of: Option<&str>, pasted: &str) -> Vec<String> {
        let mut document = format::parse(text);
        let block = of.map(|of| block(&document, of));
        let place = match (at, &block) {
            (_, None) => Place::Start,
            (At::Below, Some(block)) => Place::Below(block),
            (At::Above, Some(block)) => Place::Above(block),
            (At::FirstChild, Some(block)) => Place::FirstChild(block),
            (At::LastChild, Some(block)) => Place::LastChild(block),
        };
        let (tops, ops) = paste(&mut document, place, &format::parse(pasted)).unwrap();
        let mut transaction = Transaction::new(ops, (), ());
        document.apply(&mut transaction).unwrap();
        let after: Vec<String> = document
            .descendants(document.root())
            .map(|(id, depth)| {
                let mark = if tops.contains(&id) { ">" } else { "" };
                let text = document.get(id).unwrap().text();
                format!("{}{mark}{text}", "\t".repeat(depth))
            })
            .collect();
        document.apply(&mut transaction.inverse()).unwrap();
        assert_eq!(lines(&document, None), lines(&format::parse(text), None));
        after
    }

    // FR-OPS-06, editing.md §4
    #[test]
    fn pasted_lines_go_below_or_above_the_block_at_its_depth_keeping_their_structure() {
        let text = "P:\n\t- a\n\t\t- b\n\t- c";
        let clip = "- x @due(1)\n\t- y\n- z\n";
        assert_eq!(
            pasted(text, At::Below, Some("- a"), clip),
            [
                "P:",
                "\t- a",
                "\t\t- b",
                "\t>- x @due(1)",
                "\t\t- y",
                "\t>- z",
                "\t- c"
            ]
        );
        assert_eq!(
            pasted(text, At::Above, Some("- a"), clip),
            [
                "P:",
                "\t>- x @due(1)",
                "\t\t- y",
                "\t>- z",
                "\t- a",
                "\t\t- b",
                "\t- c"
            ]
        );
        assert_eq!(
            pasted(text, At::FirstChild, Some("P:"), "n"),
            ["P:", "\t>n", "\t- a", "\t\t- b", "\t- c"]
        );
        assert_eq!(
            pasted(text, At::LastChild, Some("P:"), "n"),
            ["P:", "\t- a", "\t\t- b", "\t- c", "\t>n"]
        );
        assert_eq!(
            pasted("", At::Below, None, clip),
            [">- x @due(1)", "\t- y", ">- z"]
        );
    }

    // FR-OPS-06, file-format.md §3.1
    #[test]
    fn pasted_lines_are_placed_and_repaired_where_they_would_be_read_back_elsewhere() {
        assert_eq!(
            pasted("- a\n- b", At::Below, Some("- a"), "\t\t- x\n- y"),
            ["- a", ">- x", ">- y", "- b"],
            "a deeper first line is placed at the block's depth"
        );
        assert_eq!(
            pasted("- a", At::Below, Some("- a"), "  - x"),
            ["- a", ">- x"],
            "a single line keeps its leading whitespace only as the only line"
        );
        assert_eq!(pasted("", At::Below, None, "  - x"), [">  - x"]);
        assert_eq!(
            pasted("  a", At::Below, Some("  a"), "b"),
            ["a", ">b"],
            "nor does the line that was the only one"
        );
    }

    #[test]
    fn pasting_nothing_changes_nothing() {
        assert_eq!(pasted("- a", At::Below, Some("- a"), ""), ["- a"]);
        let mut document = format::parse("\n- a");
        let blank = block(&document, "");
        assert_eq!(
            paste(
                &mut document,
                Place::FirstChild(&blank),
                &Document::default()
            ),
            Some((Vec::new(), Vec::new())),
            "not even the placeholder an empty parent would need"
        );
    }

    #[test]
    fn pasting_at_a_block_not_in_the_document_pastes_nothing() {
        let other = format::parse("- x\n- y");
        let mut document = format::parse("- a");
        let stranger = block(&other, "- y");
        assert_eq!(
            paste(&mut document, Place::Below(&stranger), &format::parse("b")),
            None
        );
    }
}
