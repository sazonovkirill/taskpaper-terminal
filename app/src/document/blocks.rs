//! Blocks, what the user navigates: every line that is not a special node is one block, whose
//! type is derived from its text ([data-model.md §3](../../../docs/specs/data-model.md#3-blocks)).

use super::model::{Document, Item, ItemId, ItemKind};

/// The type of a block (FR-BLK-01–04, FR-BLK-06–07).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// A project line.
    Project,
    /// A task line.
    Task,
    /// A line of any other text, blank lines included.
    Text,
    /// A line tagged `@section`.
    Section,
    /// A line tagged `@url`.
    Url,
    /// A text line reading `---`.
    Divider,
}

/// One block: a line of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    id: ItemId,
    kind: BlockKind,
    done: bool,
}

impl Block {
    /// The block of the line `item`, whose id is `id`.
    fn of(id: ItemId, item: &Item) -> Self {
        Self {
            id,
            kind: kind(item),
            done: has_tag(item, "done"),
        }
    }

    /// The block's id: the id of its line.
    #[must_use]
    pub fn id(&self) -> ItemId {
        self.id
    }

    /// Its type.
    #[must_use]
    pub fn kind(&self) -> BlockKind {
        self.kind
    }

    /// Whether its line carries `@done`, with or without a date (D-17).
    #[must_use]
    pub fn done(&self) -> bool {
        self.done
    }
}

/// The blocks formed by the children of the line `parent` (or the root), in order. Special nodes
/// (`@hidden` lines and their subtrees) form no block.
#[must_use]
pub fn children(document: &Document, parent: ItemId) -> Vec<Block> {
    document
        .get(parent)
        .map(Item::children)
        .unwrap_or_default()
        .iter()
        .filter_map(|&id| Some((id, document.get(id)?)))
        .filter(|(_, item)| !is_special(item))
        .map(|(id, item)| Block::of(id, item))
        .collect()
}

/// The block of the line `id`, or `None` for the root, a special node or a line not in
/// `document`.
#[must_use]
pub fn block(document: &Document, id: ItemId) -> Option<Block> {
    let item = document.get(id)?;
    item.parent()?;
    (!is_special(item)).then(|| Block::of(id, item))
}

/// The block whose children include the block `id`, or `None` at the top level.
#[must_use]
pub fn parent(document: &Document, id: ItemId) -> Option<Block> {
    block(document, document.get(id)?.parent()?)
}

/// Whether `item` starts a special node, which with its subtree forms no block (FR-SPC-01).
#[must_use]
pub fn is_special(item: &Item) -> bool {
    has_tag(item, "hidden")
}

/// The type of the line `item`, checked in data-model §3's order.
fn kind(item: &Item) -> BlockKind {
    if item.kind() == ItemKind::Note && item.display_text() == "---" {
        BlockKind::Divider
    } else if has_tag(item, "section") {
        BlockKind::Section
    } else if has_tag(item, "url") {
        BlockKind::Url
    } else {
        match item.kind() {
            ItemKind::Task => BlockKind::Task,
            ItemKind::Project => BlockKind::Project,
            ItemKind::Note => BlockKind::Text,
        }
    }
}

fn has_tag(item: &Item, name: &str) -> bool {
    item.tags().iter().any(|tag| tag.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::format::parse;

    /// Each block below `parent`, recursively, as `depth kind text` plus ` done` when done.
    fn outline(document: &Document) -> Vec<String> {
        fn walk(document: &Document, parent: ItemId, depth: usize, out: &mut Vec<String>) {
            for block in children(document, parent) {
                let text = document.get(block.id()).unwrap().text();
                let done = if block.done() { " done" } else { "" };
                out.push(format!("{depth} {:?} {text:?}{done}", block.kind()));
                walk(document, block.id(), depth + 1, out);
            }
        }
        let mut out = Vec::new();
        walk(document, document.root(), 0, &mut out);
        out
    }

    #[track_caller]
    fn check(text: &str, expected: &[&str]) {
        assert_eq!(outline(&parse(text)), expected, "blocks of {text:?}");
    }

    // FR-BLK-01, FR-BLK-02, FR-BLK-04, FR-BLK-06, FR-BLK-07
    #[test]
    fn every_line_is_a_block_of_its_type() {
        check(
            "P:\n\t- t\n\tS @section\n\tL @url(https://x.y)\n\t---\n\tn",
            &[
                r#"0 Project "P:""#,
                r#"1 Task "- t""#,
                r#"1 Section "S @section""#,
                r#"1 Url "L @url(https://x.y)""#,
                r#"1 Divider "---""#,
                r#"1 Text "n""#,
            ],
        );
    }

    // D-43
    #[test]
    fn consecutive_text_lines_and_blank_lines_are_blocks_of_their_own() {
        check(
            "a\n\nb\n\tc\n- t",
            &[
                r#"0 Text "a""#,
                r#"0 Text """#,
                r#"0 Text "b""#,
                r#"1 Text "c""#,
                r#"0 Task "- t""#,
            ],
        );
    }

    // D-43
    #[test]
    fn code_is_text_whose_tag_changes_nothing() {
        check(
            "f() @code\n- t @code\n--- @code",
            &[
                r#"0 Text "f() @code""#,
                r#"0 Task "- t @code""#,
                r#"0 Divider "--- @code""#,
            ],
        );
    }

    #[test]
    fn divider_ignores_tags_but_not_other_text() {
        check("--- @red", &[r#"0 Divider "--- @red""#]);
        check("----", &[r#"0 Text "----""#]);
        check("- ---", &[r#"0 Task "- ---""#]);
    }

    #[test]
    fn section_and_url_tags_win_over_task_and_project() {
        check(
            "- t @section\nP: @url(u)\nS @section @url(u)",
            &[
                r#"0 Section "- t @section""#,
                r#"0 Url "P: @url(u)""#,
                r#"0 Section "S @section @url(u)""#,
            ],
        );
    }

    // FR-SPC-01
    #[test]
    fn special_nodes_and_their_subtrees_are_skipped() {
        check(
            "a\n\tC @hidden @comments\n\t\tcomment\nX @hidden\nd",
            &[r#"0 Text "a""#, r#"0 Text "d""#],
        );
    }

    // D-17
    #[test]
    fn done_comes_from_the_line_with_or_without_a_date() {
        check(
            "- a @done\n- b @done(2026-10-04)\n- c @done(soon)\nm @done\n- d",
            &[
                r#"0 Task "- a @done" done"#,
                r#"0 Task "- b @done(2026-10-04)" done"#,
                r#"0 Task "- c @done(soon)" done"#,
                r#"0 Text "m @done" done"#,
                r#"0 Task "- d""#,
            ],
        );
    }

    #[test]
    fn empty_document_and_unknown_parent_have_no_blocks() {
        let document = parse("");
        assert_eq!(children(&document, document.root()), Vec::new());
        let other = parse("a");
        let stale = children(&other, other.root())[0].id();
        assert_eq!(children(&document, stale), Vec::new());
    }

    #[test]
    fn finds_a_block_and_its_parent_by_id() {
        let document = parse("P:\n\ta\n\t\t- t\n\th @hidden\n- u");
        let top = children(&document, document.root());
        let note = &children(&document, top[0].id())[0];
        let task = &children(&document, note.id())[0];
        assert_eq!(block(&document, note.id()).as_ref(), Some(note));
        assert_eq!(block(&document, task.id()).as_ref(), Some(task));
        assert_eq!(parent(&document, task.id()).as_ref(), Some(note));
        assert_eq!(parent(&document, note.id()).as_ref(), Some(&top[0]));
        assert_eq!(parent(&document, top[1].id()), None);
        let hidden = document.get(top[0].id()).unwrap().children()[1];
        assert_eq!(block(&document, hidden), None, "a special node");
        assert_eq!(block(&document, document.root()), None);
        assert_eq!(parent(&document, document.root()), None);
    }

    // FR-SPC-01
    #[test]
    fn a_line_tagged_hidden_is_special() {
        let document = parse("a @hidden\nb @hidden(x)\nc @hide\nd hidden\n- e @x @hidden");
        let special: Vec<bool> = document
            .descendants(document.root())
            .map(|(id, _)| is_special(document.get(id).unwrap()))
            .collect();
        assert_eq!(special, [true, true, false, false, true]);
    }
}
