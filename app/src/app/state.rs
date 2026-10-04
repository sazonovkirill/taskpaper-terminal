//! Application state and what it exposes for a frontend to show.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use crate::document::{Block, BlockKind, Document, Item, ItemId, blocks, format};

/// The demo outline shown when the app starts without a path (FR-DOC-05, D-27).
const DEMO: &str = include_str!("../../assets/demo.taskpaper");

/// Everything the app knows: the open document, where it came from, the focused row and the
/// collapsed blocks.
#[derive(Debug, Clone)]
pub struct State {
    document: Document,
    path: Option<PathBuf>,
    pub(super) focus: Option<ItemId>,
    /// The ids of collapsed blocks, kept in memory only (D-26).
    pub(super) folds: HashSet<ItemId>,
}

/// One visible block of the outline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The block this row shows: the id of its first line.
    pub id: ItemId,
    /// Depth below the top level, starting at 0.
    pub depth: usize,
    /// The block's type.
    pub kind: BlockKind,
    /// The text of each of its lines, without tags; never empty.
    pub lines: Vec<String>,
    /// Whether the block carries `@done`, with or without a value (D-17).
    pub done: bool,
    /// How many child blocks its fold hides; 0 when it is expanded or has none.
    pub hidden_children: usize,
}

impl State {
    /// State for a document opened from `path`, or held only in memory when `path` is `None`.
    #[must_use]
    pub fn new(document: Document, path: Option<PathBuf>) -> Self {
        let mut state = Self {
            document,
            path,
            focus: None,
            folds: HashSet::new(),
        };
        state.focus = state.rows().first().map(|row| row.id);
        state
    }

    /// State showing the demo outline, held only in memory.
    #[must_use]
    pub fn demo() -> Self {
        Self::new(format::parse(DEMO), None)
    }

    /// The open document.
    #[must_use]
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// The file the document was opened from, if any.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// The focused row's item: the first row when opened, `None` only when there are no rows
    /// (FR-NAV-01).
    #[must_use]
    pub fn focus(&self) -> Option<ItemId> {
        self.focus
    }

    /// Every visible block of the document in order, one row each: collapsed blocks hide their
    /// children and special nodes are never shown (FR-NAV-02, FR-SPC-01).
    #[must_use]
    pub fn rows(&self) -> Vec<Row> {
        self.blocks(true)
            .into_iter()
            .map(|(block, depth)| Row {
                id: block.id(),
                depth,
                kind: block.kind(),
                lines: block
                    .lines()
                    .iter()
                    .filter_map(|&id| self.document.get(id))
                    .map(Item::display_text)
                    .collect(),
                done: block.done(),
                hidden_children: if self.folds.contains(&block.id()) {
                    self.children(&block).len()
                } else {
                    0
                },
            })
            .collect()
    }

    /// Every block of the document in order with its depth; with `folded`, only those not hidden
    /// by a collapsed ancestor.
    pub(super) fn blocks(&self, folded: bool) -> Vec<(Block, usize)> {
        let mut out = Vec::new();
        self.push_blocks(self.document.root(), 0, folded, &mut out);
        out
    }

    fn push_blocks(&self, line: ItemId, depth: usize, folded: bool, out: &mut Vec<(Block, usize)>) {
        for block in blocks::children(&self.document, line) {
            let last_line = block.last_line();
            let open = !(folded && self.folds.contains(&block.id()));
            out.push((block, depth));
            if open {
                self.push_blocks(last_line, depth + 1, folded, out);
            }
        }
    }

    /// The child blocks of `block`.
    pub(super) fn children(&self, block: &Block) -> Vec<Block> {
        blocks::children(&self.document, block.last_line())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(state: &State) -> Vec<(usize, BlockKind, String)> {
        state
            .rows()
            .into_iter()
            .map(|row| (row.depth, row.kind, row.lines.join("|")))
            .collect()
    }

    #[test]
    fn rows_list_every_block_with_depth_kind_and_display_text() {
        let state = State::new(format::parse("A:\n\t- b\n\t\tc\nd"), None);
        assert_eq!(
            summary(&state),
            [
                (0, BlockKind::Project, "A".to_owned()),
                (1, BlockKind::Task, "b".to_owned()),
                (2, BlockKind::Note, "c".to_owned()),
                (0, BlockKind::Note, "d".to_owned()),
            ]
        );
        let ids: Vec<_> = state
            .document()
            .descendants(state.document().root())
            .map(|(id, _)| id)
            .collect();
        assert_eq!(
            state
                .rows()
                .into_iter()
                .map(|row| row.id)
                .collect::<Vec<_>>(),
            ids
        );
    }

    // FR-BLK-03, FR-SPC-01
    #[test]
    fn note_runs_are_one_row_and_special_nodes_no_row() {
        let state = State::new(
            format::parse("P:\n\tone @x\n\n\ttwo\n\t\tC @hidden\n\t\t\tc\n\t\t- child\n\t--- @y"),
            None,
        );
        assert_eq!(
            summary(&state),
            [
                (0, BlockKind::Project, "P".to_owned()),
                (1, BlockKind::Note, "one||two".to_owned()),
                (2, BlockKind::Task, "child".to_owned()),
                (1, BlockKind::Divider, "---".to_owned()),
            ]
        );
    }

    // FR-TAG-01, FR-DONE-01, D-17
    #[test]
    fn rows_hide_tags_and_mark_blocks_tagged_done() {
        let state = State::new(
            format::parse(
                "- a @done\n- b @done(2026-10-01) @x\n- c @due(1)\nP: @done\n- d @Done\nn\nm @done",
            ),
            None,
        );
        let rows: Vec<_> = state
            .rows()
            .into_iter()
            .map(|row| (row.lines.join("|"), row.done))
            .collect();
        assert_eq!(
            rows,
            [
                ("a".to_owned(), true),
                ("b".to_owned(), true),
                ("c".to_owned(), false),
                ("P".to_owned(), true),
                ("d".to_owned(), false),
                ("n|m".to_owned(), true),
            ]
        );
    }

    #[test]
    fn empty_document_has_no_rows() {
        assert_eq!(State::new(Document::default(), None).rows(), Vec::new());
    }

    #[test]
    fn focuses_the_first_row_or_nothing_when_empty() {
        let state = State::new(format::parse("A:\n\t- b"), None);
        assert_eq!(state.focus(), state.rows().first().map(|row| row.id));
        assert!(state.focus().is_some());
        assert_eq!(State::new(Document::default(), None).focus(), None);
    }

    #[test]
    fn remembers_the_path() {
        let state = State::new(
            Document::default(),
            Some(PathBuf::from("/notes/todo.taskpaper")),
        );
        assert_eq!(state.path(), Some(Path::new("/notes/todo.taskpaper")));
        assert_eq!(State::demo().path(), None);
    }

    /// The canonical-outline rules of file-format.md §3.1, checked on the parsed tree.
    fn assert_canonical(document: &Document) {
        let items: Vec<&Item> = document
            .descendants(document.root())
            .filter_map(|(id, _)| document.get(id))
            .collect();
        let placed = |item: &&Item| item.extra_indent() == 0;
        let not_empty_parent = |item: &&Item| !item.text().is_empty() || item.children().is_empty();
        let trimmed = |item: &&Item| !item.text().starts_with(char::is_whitespace);
        let blank = |item: &&&Item| item.text().is_empty();
        assert!(items.iter().all(placed), "placement");
        assert!(items.iter().all(not_empty_parent), "empty parent");
        assert!(items.iter().all(trimmed), "leading whitespace");
        let trailing_parents: Vec<Option<ItemId>> = items
            .iter()
            .rev()
            .take_while(blank)
            .copied()
            .map(Item::parent)
            .collect();
        let top_level = vec![Some(document.root()); trailing_parents.len()];
        assert_eq!(trailing_parents, top_level, "trailing indented blank");
    }

    // FR-DOC-05, D-27
    #[test]
    fn demo_is_a_canonical_outline_with_every_implemented_type() {
        let state = State::demo();
        assert_canonical(state.document());
        let rows = state.rows();
        for kind in [
            BlockKind::Project,
            BlockKind::Task,
            BlockKind::Note,
            BlockKind::Section,
            BlockKind::Code,
            BlockKind::Url,
            BlockKind::Divider,
        ] {
            assert!(
                rows.iter().any(|row| row.kind == kind),
                "demo lacks a {kind:?}"
            );
        }
        assert!(rows.iter().any(|row| row.depth >= 2), "demo lacks nesting");
        assert_eq!(
            rows.first().map(|row| row.lines.clone()),
            Some(vec!["Trip to Lisbon".to_owned()])
        );
        assert!(
            rows.iter()
                .any(|row| row.kind == BlockKind::Task && row.done),
            "demo lacks a done task"
        );
        let document = state.document();
        let tagged = document
            .descendants(document.root())
            .filter_map(|(id, _)| document.get(id))
            .filter(|item| !item.tags().is_empty())
            .count();
        assert!(tagged >= 3, "demo lacks tagged lines");
        assert!(
            rows.iter()
                .flat_map(|row| &row.lines)
                .all(|line| !line.contains('@')),
            "demo shows a tag"
        );
        assert!(
            rows.iter()
                .any(|row| row.kind == BlockKind::Note && row.lines.len() > 1),
            "demo lacks a multi-line note"
        );
        let hidden = document
            .descendants(document.root())
            .filter_map(|(id, _)| document.get(id))
            .any(|item| item.tags().iter().any(|tag| tag.name == "hidden"));
        assert!(hidden, "demo lacks a special node");
    }
}
