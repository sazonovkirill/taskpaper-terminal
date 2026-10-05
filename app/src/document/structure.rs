//! Adding, deleting and moving blocks as line ops: where a new block goes, which type it takes,
//! which lines a deleted block takes with it and where a moved one lands
//! ([editing.md §3–4](../../../docs/specs/editing.md#3-creating-blocks-fr-mode-04), OQ-03, OQ-26).

use std::collections::HashMap;

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

/// A type the user picks for a new block (FR-MODE-06, D-26).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Typed {
    /// A project.
    Project,
    /// A section.
    Section,
    /// A task.
    Task,
}

impl Typed {
    /// The line of an empty block of this type that is the `index`th child of the line `parent`.
    /// A section that would join the code run before it, as a note line does, is written as a
    /// project, which ends the run and is still a section (D-18, data-model.md §3).
    fn empty_line(self, document: &Document, parent: ItemId, index: usize) -> &'static str {
        match self {
            Self::Project => ":",
            Self::Section if blocks::note_line_joins_code(document, parent, index) => ": @section",
            Self::Section => "@section",
            Self::Task => "- ",
        }
    }
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
    let (parent, index) = match place {
        Place::Below(block) => {
            let (parent, index) = document.position(block.last_line())?;
            (parent, index + 1)
        }
        Place::Above(block) => document.position(block.id())?,
        Place::FirstChild(block) => (block.last_line(), 0),
        Place::Start => (document.root(), 0),
    };
    let text = typed.map_or_else(
        || fitting_line(document, place, parent, index),
        |typed| typed.empty_line(document, parent, index),
    );
    let mut ops = repairs(document, parent);
    let id = document.new_id();
    ops.push(Op::Insert {
        parent,
        index,
        subtree: Snapshot::line(id, text.to_owned()),
    });
    Some((id, ops))
}

/// The line of an empty block at `place`, the `index`th child of the line `parent`, whose type
/// the user did not pick: a task after or before a task and under a project or task, else a
/// note, unless a note would join the note run next to it (OQ-03, OQ-26).
fn fitting_line(
    document: &Document,
    place: Place<'_>,
    parent: ItemId,
    index: usize,
) -> &'static str {
    let task = match place {
        Place::Below(block) | Place::Above(block) => block.kind() == BlockKind::Task,
        Place::FirstChild(block) => matches!(block.kind(), BlockKind::Project | BlockKind::Task),
        Place::Start => false,
    } || blocks::blank_line_joins_run(document, parent, index);
    if task { "- " } else { "" }
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
/// the line `top`, the root or the zoom block's last line; `None` when it has nowhere to go
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
        Shift::Indent => (previous?.last_line(), Spot::End),
        Shift::Outdent if parent != top => (document.get(parent)?.parent()?, Spot::After(parent)),
        Shift::Outdent => return None,
        Shift::Up => match previous {
            Some(previous) => (parent, Spot::Before(previous.id())),
            None => (parents_neighbor(document, parent, top, -1)?, Spot::End),
        },
        Shift::Down => match next {
            Some(next) => (parent, Spot::After(next.last_line())),
            None => (parents_neighbor(document, parent, top, 1)?, Spot::Start),
        },
    };
    let mut ops = repairs(document, to);
    ops.extend(relocate(document, block.lines(), to, spot)?);
    Some(ops)
}

/// Where among a line's children moved lines go.
#[derive(Debug, Clone, Copy)]
enum Spot {
    Start,
    /// After the last child that is not a special node.
    End,
    Before(ItemId),
    After(ItemId),
}

/// The last line of the block `by` places after (or before, when negative) the block whose
/// children are those of the line `parent`, or `None` when `parent` is `top` or that block has
/// no such neighbor.
fn parents_neighbor(document: &Document, parent: ItemId, top: ItemId, by: isize) -> Option<ItemId> {
    if parent == top {
        return None;
    }
    let siblings = blocks::children(document, document.get(parent)?.parent()?);
    let at = siblings
        .iter()
        .position(|sibling| sibling.last_line() == parent)?;
    Some(siblings.get(at.checked_add_signed(by)?)?.last_line())
}

/// The ops moving `lines`, each with its subtree, to `spot` among the children of the line `to`,
/// in order, the end being before the special nodes that end the children so that they stay
/// last (special-nodes.md §1); `None` when a line or the spot is not in `document`.
fn relocate(document: &Document, lines: &[ItemId], to: ItemId, spot: Spot) -> Option<Vec<Op>> {
    let mut children = HashMap::new();
    let mut ops = Vec::new();
    let mut previous = None;
    for &line in lines {
        let from = document.get(line)?.parent()?;
        let siblings = children_of(&mut children, document, from);
        let from_index = siblings.iter().position(|&id| id == line)?;
        siblings.remove(from_index);
        let targets = children_of(&mut children, document, to);
        let find = |id: ItemId| targets.iter().position(|&target| target == id);
        let index = match previous.map_or(spot, Spot::After) {
            Spot::Start => 0,
            Spot::End => targets
                .iter()
                .rposition(|&id| {
                    document
                        .get(id)
                        .is_some_and(|item| !blocks::is_special(item))
                })
                .map_or(0, |last| last + 1),
            Spot::Before(id) => find(id)?,
            Spot::After(id) => find(id)? + 1,
        };
        targets.insert(index, line);
        previous = Some(line);
        ops.push(Op::Move {
            id: line,
            from: (from, from_index),
            to: (to, index),
        });
    }
    Some(ops)
}

/// The children of `line` in `children`, which holds them as the ops built so far leave them,
/// taken from `document` the first time.
fn children_of<'a>(
    children: &'a mut HashMap<ItemId, Vec<ItemId>>,
    document: &Document,
    line: ItemId,
) -> &'a mut Vec<ItemId> {
    children.entry(line).or_insert_with(|| {
        document
            .get(line)
            .map(|item| item.children().to_vec())
            .unwrap_or_default()
    })
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

    // FR-MODE-06, OQ-26
    #[test]
    fn a_typed_block_has_its_type_wherever_it_goes() {
        let mut document = format::parse("n\n\tm\n- t");
        let note = block(&document, "n");
        let task = block(&document, "- t");
        for (typed, line, kind) in [
            (Typed::Project, ":", BlockKind::Project),
            (Typed::Section, "@section", BlockKind::Section),
            (Typed::Task, "- ", BlockKind::Task),
        ] {
            for place in [
                Place::Below(&note),
                Place::Above(&task),
                Place::FirstChild(&note),
                Place::Start,
            ] {
                let (id, ops) = new_block(&mut document, place, Some(typed)).unwrap();
                let mut transaction = Transaction::new(ops, (), ());
                document.apply(&mut transaction).unwrap();
                assert_eq!(document.get(id).unwrap().text(), line, "{place:?}");
                assert_eq!(blocks::block(&document, id).unwrap().kind(), kind);
                document.apply(&mut transaction.inverse()).unwrap();
            }
        }
    }

    // FR-MODE-06, D-18
    #[test]
    fn a_section_after_code_is_written_as_a_project_to_stay_a_section() {
        let mut document = format::parse("f() @code\nx\n- t");
        let code = block(&document, "f() @code");
        assert_eq!(
            applied_as(&mut document, Place::Below(&code), Some(Typed::Section)),
            ["f() @code", "x", ">: @section", "- t"]
        );
        let (id, ops) =
            new_block(&mut document, Place::Below(&code), Some(Typed::Section)).unwrap();
        document.apply(&mut Transaction::new(ops, (), ())).unwrap();
        assert_eq!(
            blocks::block(&document, id).unwrap().kind(),
            BlockKind::Section
        );
        assert_eq!(
            block(&document, "f() @code").lines().len(),
            2,
            "the code stays as it was"
        );
    }

    // file-format §3.1
    #[test]
    fn a_typed_block_repairs_its_parent_too() {
        let mut document = format::parse("a\n\n- t");
        let run = block(&document, "a");
        assert_eq!(
            applied_as(&mut document, Place::FirstChild(&run), Some(Typed::Section)),
            ["a", "…", "\t>@section", "- t"]
        );
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
        assert_eq!(new_block(&mut document, Place::Below(&stale), None), None);
        assert_eq!(new_block(&mut document, Place::Above(&stale), None), None);
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

    /// The lines of `text` once the block reading `of` shifts below the line reading `top`
    /// (the root when `None`), with the ops checked to undo; `None` when it does not move.
    #[track_caller]
    fn shifted_below(text: &str, of: &str, by: Shift, top: Option<&str>) -> Option<Vec<String>> {
        let mut document = format::parse(text);
        let top = top.map_or(document.root(), |top| block(&document, top).last_line());
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
            shifted("- a\nn\nm\n- d", "n", Shift::Indent).unwrap(),
            ["- a", "\tn", "\tm", "- d"],
            "every line of a note"
        );
        assert_eq!(
            shifted("n\nm\n- t", "- t", Shift::Indent).unwrap(),
            ["n", "m", "\t- t"],
            "under the last line of a note"
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
        assert_eq!(
            shifted("n\nm\n\t- a", "- a", Shift::Outdent).unwrap(),
            ["n", "m", "- a"],
            "after the last line of a note"
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
            ["- a", "- d", "n", "m"],
            "every line of a note"
        );
        assert_eq!(
            shifted("- a\nn\nm\n- d", "- d", Shift::Up).unwrap(),
            ["- a", "- d", "n", "m"],
            "past every line of a note"
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
}
