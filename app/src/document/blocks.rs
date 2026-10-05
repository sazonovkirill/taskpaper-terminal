//! Blocks, what the user navigates, derived from the line tree
//! ([data-model.md §3](../../../docs/specs/data-model.md#3-blocks)).

use super::model::{Document, Item, ItemId, ItemKind};

/// The type of a block (FR-BLK-01–07).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// A project line.
    Project,
    /// A task line.
    Task,
    /// A run of plain note lines.
    Note,
    /// A line tagged `@section`.
    Section,
    /// A note run whose first line carries `@code`.
    Code,
    /// A line tagged `@url`.
    Url,
    /// A note line reading `---`.
    Divider,
}

/// One block: a line, or a run of consecutive sibling note lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    id: ItemId,
    last_line: ItemId,
    lines: Vec<ItemId>,
    kind: BlockKind,
    done: bool,
}

impl Block {
    /// The block's id: the id of its first line.
    #[must_use]
    pub fn id(&self) -> ItemId {
        self.id
    }

    /// Its lines in order.
    #[must_use]
    pub fn lines(&self) -> &[ItemId] {
        &self.lines
    }

    /// The line whose children are the block's children.
    #[must_use]
    pub fn last_line(&self) -> ItemId {
        self.last_line
    }

    /// Its type.
    #[must_use]
    pub fn kind(&self) -> BlockKind {
        self.kind
    }

    /// Whether any of its lines carries `@done`, with or without a date (D-17).
    #[must_use]
    pub fn done(&self) -> bool {
        self.done
    }
}

/// The blocks formed by the children of the line `parent` (or the root), in order.
/// Special nodes (`@hidden` lines and their subtrees) form no block and never join a run.
#[must_use]
pub fn children(document: &Document, parent: ItemId) -> Vec<Block> {
    let lines: Vec<(ItemId, &Item)> = document
        .get(parent)
        .map(Item::children)
        .unwrap_or_default()
        .iter()
        .filter_map(|&id| Some((id, document.get(id)?)))
        .filter(|(_, item)| !has_tag(item, "hidden"))
        .collect();
    let mut blocks = Vec::new();
    let mut rest = lines.as_slice();
    while let Some(((first_id, first), after)) = rest.split_first() {
        let code = first.kind() == ItemKind::Note && has_tag(first, "code");
        let mut run = vec![(*first_id, *first)];
        rest = after;
        if code || is_plain_note(first) {
            while let Some(((id, item), after)) = rest.split_first()
                && run
                    .last()
                    .is_some_and(|(_, last)| last.children().is_empty())
                && joins_run(item, code)
            {
                run.push((*id, *item));
                rest = after;
            }
        }
        blocks.push(run_block((*first_id, first), &run, code));
    }
    blocks
}

/// The block whose first line is `id`, or `None` when no block starts there.
#[must_use]
pub fn block(document: &Document, id: ItemId) -> Option<Block> {
    let parent = document.get(id)?.parent()?;
    children(document, parent)
        .into_iter()
        .find(|block| block.id == id)
}

/// The block whose children include the block `id`, or `None` at the top level.
#[must_use]
pub fn parent(document: &Document, id: ItemId) -> Option<Block> {
    let line = document.get(id)?.parent()?;
    let above = document.get(line)?.parent()?;
    children(document, above)
        .into_iter()
        .find(|block| block.last_line == line)
}

/// Whether a blank line without children put at `index` among the children of the line `parent`
/// would join the note or code run before it or the note run after it, becoming part of that
/// block (D-2, D-18).
#[must_use]
pub fn blank_line_joins_run(document: &Document, parent: ItemId, index: usize) -> bool {
    let shown = |&id: &ItemId| {
        document
            .get(id)
            .filter(|item| !has_tag(item, "hidden"))
            .map(|item| (id, item))
    };
    let lines = document.get(parent).map(Item::children).unwrap_or_default();
    let (before, after) = lines.split_at(index.min(lines.len()));
    let joins_before = before
        .iter()
        .rev()
        .find_map(shown)
        .is_some_and(|(id, item)| {
            item.children().is_empty()
                && children(document, parent).iter().any(|block| {
                    block.last_line == id && matches!(block.kind, BlockKind::Note | BlockKind::Code)
                })
        });
    joins_before
        || after
            .iter()
            .find_map(shown)
            .is_some_and(|(_, item)| joins_run(item, false))
}

/// Whether `item` continues a run: any note line inside code, else a plain note line;
/// a line carrying `@code` always starts a new run.
fn joins_run(item: &Item, code: bool) -> bool {
    !has_tag(item, "code")
        && if code {
            item.kind() == ItemKind::Note
        } else {
            is_plain_note(item)
        }
}

/// The block made of `run`, whose first line is `first`; its type is checked in data-model §3's order.
fn run_block((id, first): (ItemId, &Item), run: &[(ItemId, &Item)], code: bool) -> Block {
    let kind = if code {
        BlockKind::Code
    } else if is_divider(first) {
        BlockKind::Divider
    } else if has_tag(first, "section") {
        BlockKind::Section
    } else if has_tag(first, "url") {
        BlockKind::Url
    } else {
        match first.kind() {
            ItemKind::Task => BlockKind::Task,
            ItemKind::Project => BlockKind::Project,
            ItemKind::Note => BlockKind::Note,
        }
    };
    let lines: Vec<ItemId> = run.iter().map(|&(line, _)| line).collect();
    Block {
        id,
        last_line: lines.last().copied().unwrap_or(id),
        lines,
        kind,
        done: run.iter().any(|(_, item)| has_tag(item, "done")),
    }
}

/// A note line that is not a divider, section or URL.
fn is_plain_note(item: &Item) -> bool {
    item.kind() == ItemKind::Note
        && !is_divider(item)
        && !has_tag(item, "section")
        && !has_tag(item, "url")
}

/// A note line whose text without tags is exactly `---`.
fn is_divider(item: &Item) -> bool {
    item.kind() == ItemKind::Note && item.display_text() == "---"
}

fn has_tag(item: &Item, name: &str) -> bool {
    item.tags().iter().any(|tag| tag.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::format::parse;

    /// Each block below `parent`, recursively, as `depth kind [texts]` plus ` done` when done.
    fn outline(document: &Document) -> Vec<String> {
        fn walk(document: &Document, parent: ItemId, depth: usize, out: &mut Vec<String>) {
            for block in children(document, parent) {
                let texts: Vec<&str> = block
                    .lines()
                    .iter()
                    .map(|&id| document.get(id).unwrap().text())
                    .collect();
                let done = if block.done() { " done" } else { "" };
                out.push(format!("{depth} {:?} {texts:?}{done}", block.kind()));
                walk(document, block.last_line(), depth + 1, out);
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

    #[test]
    fn single_lines_are_blocks_of_their_type() {
        check(
            "P:\n\t- t\n\tS @section\n\tL @url(https://x.y)\n\t---\n\tn",
            &[
                r#"0 Project ["P:"]"#,
                r#"1 Task ["- t"]"#,
                r#"1 Section ["S @section"]"#,
                r#"1 Url ["L @url(https://x.y)"]"#,
                r#"1 Divider ["---"]"#,
                r#"1 Note ["n"]"#,
            ],
        );
    }

    #[test]
    fn consecutive_sibling_notes_form_one_block_with_blank_lines() {
        check(
            "a\n\nb\n- t\nc",
            &[
                r#"0 Note ["a", "", "b"]"#,
                r#"0 Task ["- t"]"#,
                r#"0 Note ["c"]"#,
            ],
        );
    }

    #[test]
    fn a_note_with_children_ends_its_run_and_owns_them() {
        check(
            "a\nb\n\tchild\nc",
            &[
                r#"0 Note ["a", "b"]"#,
                r#"1 Note ["child"]"#,
                r#"0 Note ["c"]"#,
            ],
        );
    }

    #[test]
    fn dividers_sections_and_urls_end_a_note_run() {
        check(
            "a\n---\nb\nS @section\nc\nL @url(u)\nd",
            &[
                r#"0 Note ["a"]"#,
                r#"0 Divider ["---"]"#,
                r#"0 Note ["b"]"#,
                r#"0 Section ["S @section"]"#,
                r#"0 Note ["c"]"#,
                r#"0 Url ["L @url(u)"]"#,
                r#"0 Note ["d"]"#,
            ],
        );
    }

    #[test]
    fn divider_ignores_tags_but_not_other_text() {
        check("--- @red", &[r#"0 Divider ["--- @red"]"#]);
        check("a\n----", &[r#"0 Note ["a", "----"]"#]);
        check("- ---", &[r#"0 Task ["- ---"]"#]);
    }

    #[test]
    fn code_line_starts_a_new_run() {
        check(
            "a\nx @code\ny\nz @code\nw",
            &[
                r#"0 Note ["a"]"#,
                r#"0 Code ["x @code", "y"]"#,
                r#"0 Code ["z @code", "w"]"#,
            ],
        );
    }

    #[test]
    fn code_run_keeps_dividers_sections_and_urls() {
        check(
            "f() @code\n---\nS @section\nL @url(u)\n\nend",
            &[r#"0 Code ["f() @code", "---", "S @section", "L @url(u)", "", "end"]"#],
        );
    }

    #[test]
    fn code_run_ends_before_tasks_and_projects_and_after_children() {
        check(
            "x @code\n- t\ny @code\nP:\nz @code\nk\n\tc\nl",
            &[
                r#"0 Code ["x @code"]"#,
                r#"0 Task ["- t"]"#,
                r#"0 Code ["y @code"]"#,
                r#"0 Project ["P:"]"#,
                r#"0 Code ["z @code", "k"]"#,
                r#"1 Note ["c"]"#,
                r#"0 Note ["l"]"#,
            ],
        );
    }

    #[test]
    fn code_tag_on_a_task_or_project_does_not_make_code() {
        check(
            "- t @code\nP: @code",
            &[r#"0 Task ["- t @code"]"#, r#"0 Project ["P: @code"]"#],
        );
    }

    #[test]
    fn code_tag_beats_divider_and_section() {
        check("--- @code", &[r#"0 Code ["--- @code"]"#]);
        check(
            "S @section @code\nx",
            &[r#"0 Code ["S @section @code", "x"]"#],
        );
    }

    #[test]
    fn section_and_url_tags_win_over_task_and_project() {
        check(
            "- t @section\nP: @url(u)",
            &[r#"0 Section ["- t @section"]"#, r#"0 Url ["P: @url(u)"]"#],
        );
    }

    #[test]
    fn special_nodes_are_skipped_but_still_end_a_run() {
        check(
            "a\nb\n\tC @hidden @comments\n\t\tcomment\nc\nX @hidden\nd",
            &[r#"0 Note ["a", "b"]"#, r#"0 Note ["c", "d"]"#],
        );
    }

    #[test]
    fn done_comes_from_any_line_of_the_block_but_not_special_nodes() {
        check(
            "- a @done\n- b @done(2026-10-04)\n- c @done(soon)\nn\nm @done\n- d\n\tx @hidden @done",
            &[
                r#"0 Task ["- a @done"] done"#,
                r#"0 Task ["- b @done(2026-10-04)"] done"#,
                r#"0 Task ["- c @done(soon)"] done"#,
                r#"0 Note ["n", "m @done"] done"#,
                r#"0 Task ["- d"]"#,
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
        let document = parse("P:\n\ta\n\tb\n\t\t- t\n- u");
        let top = children(&document, document.root());
        let notes = &children(&document, top[0].last_line())[0];
        let task = &children(&document, notes.last_line())[0];
        assert_eq!(block(&document, notes.id()).as_ref(), Some(notes));
        assert_eq!(
            block(&document, notes.last_line()),
            None,
            "not a first line"
        );
        assert_eq!(parent(&document, task.id()).as_ref(), Some(notes));
        assert_eq!(parent(&document, notes.id()).as_ref(), Some(&top[0]));
        assert_eq!(parent(&document, top[1].id()), None);
        assert_eq!(block(&document, document.root()), None);
        assert_eq!(parent(&document, document.root()), None);
    }

    /// Whether a blank line at `index` under the line reading `parent` (the root when empty)
    /// joins a run.
    #[track_caller]
    fn joins(text: &str, parent: &str, index: usize) -> bool {
        let document = parse(text);
        let parent = document
            .descendants(document.root())
            .map(|(id, _)| id)
            .find(|&id| document.get(id).unwrap().text() == parent)
            .unwrap_or(document.root());
        blank_line_joins_run(&document, parent, index)
    }

    // D-2, OQ-26
    #[test]
    fn a_blank_line_joins_the_note_run_before_or_after_it() {
        assert!(joins("a\n- t", "", 1), "after a note");
        assert!(joins("- t\na", "", 1), "before a note");
        assert!(joins("a\nb", "", 1), "inside a run");
        assert!(
            joins("P:\n\ta", "P:", 0),
            "as the first child before a note"
        );
        assert!(!joins("- t\n- u", "", 1));
        assert!(!joins("P:\n- u", "", 1));
        assert!(!joins("", "", 0), "nothing to join");
        assert!(!joins("P:", "P:", 0));
    }

    // D-2, D-18, OQ-26
    #[test]
    fn a_blank_line_joins_code_before_it_but_not_after_it() {
        assert!(
            joins("f() @code\n---\n- t", "", 2),
            "code ending in a divider"
        );
        assert!(!joins("- t\nf() @code", "", 1), "a @code line starts a run");
    }

    // D-2
    #[test]
    fn a_blank_line_does_not_join_a_note_with_children_or_other_types() {
        assert!(!joins("a\n\tc\n- t", "", 1), "the run ends after children");
        assert!(
            joins("- t\na\n\tc", "", 1),
            "but a note after it with children"
        );
        for other in ["---", "S @section", "L @url(u)"] {
            assert!(!joins(&format!("{other}\n{other}"), "", 1), "{other}");
        }
    }

    // FR-SPC-01
    #[test]
    fn a_blank_line_joins_across_special_nodes() {
        assert!(joins("a\nX @hidden\n- t", "", 2));
        assert!(joins("- t\nX @hidden\na", "", 1));
        assert!(!joins("- t\nX @hidden\n- u", "", 2));
    }

    #[test]
    fn a_position_past_the_end_counts_as_the_end() {
        assert!(joins("a", "", 5));
    }

    #[test]
    fn block_ids_and_lines() {
        let document = parse("a\nb\n- t");
        let blocks = children(&document, document.root());
        let root_lines = document.get(document.root()).unwrap().children().to_vec();
        assert_eq!(blocks[0].id(), root_lines[0]);
        assert_eq!(blocks[0].lines(), &root_lines[..2]);
        assert_eq!(blocks[0].last_line(), root_lines[1]);
        assert_eq!(blocks[1].id(), root_lines[2]);
        assert_eq!(blocks[1].last_line(), root_lines[2]);
    }
}
