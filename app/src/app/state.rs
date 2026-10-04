//! Application state and what it exposes for a frontend to show.

use std::path::{Path, PathBuf};

use crate::document::{Document, ItemId, ItemKind, format};

/// The demo outline shown when the app starts without a path (FR-DOC-05, D-27).
const DEMO: &str = include_str!("../../assets/demo.taskpaper");

/// Everything the app knows: the open document and where it came from.
#[derive(Debug, Clone)]
pub struct State {
    document: Document,
    path: Option<PathBuf>,
}

/// One visible row of the outline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The item this row shows.
    pub id: ItemId,
    /// Depth below the top level, starting at 0.
    pub depth: usize,
    /// The item's TaskPaper type.
    pub kind: ItemKind,
    /// The text to show, without tags.
    pub text: String,
    /// Whether the line carries `@done`, with or without a value (D-17).
    pub done: bool,
}

impl State {
    /// State for a document opened from `path`, or held only in memory when `path` is `None`.
    #[must_use]
    pub fn new(document: Document, path: Option<PathBuf>) -> Self {
        Self { document, path }
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

    /// Every line of the document in order, one row each.
    #[must_use]
    pub fn rows(&self) -> Vec<Row> {
        self.document
            .descendants(self.document.root())
            .filter_map(|(id, depth)| {
                let item = self.document.get(id)?;
                Some(Row {
                    id,
                    depth,
                    kind: item.kind(),
                    text: item.display_text(),
                    done: item.tags().iter().any(|tag| tag.name == "done"),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Item;

    fn summary(state: &State) -> Vec<(usize, ItemKind, String)> {
        state
            .rows()
            .into_iter()
            .map(|row| (row.depth, row.kind, row.text))
            .collect()
    }

    #[test]
    fn rows_list_every_line_with_depth_kind_and_display_text() {
        let state = State::new(format::parse("A:\n\t- b\n\t\tc\nd"), None);
        assert_eq!(
            summary(&state),
            [
                (0, ItemKind::Project, "A".to_owned()),
                (1, ItemKind::Task, "b".to_owned()),
                (2, ItemKind::Note, "c".to_owned()),
                (0, ItemKind::Note, "d".to_owned()),
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

    // FR-TAG-01, FR-DONE-01, D-17
    #[test]
    fn rows_hide_tags_and_mark_lines_tagged_done() {
        let state = State::new(
            format::parse("- a @done\n- b @done(2026-10-01) @x\n- c @due(1)\nP: @done\n- d @Done"),
            None,
        );
        let rows: Vec<_> = state
            .rows()
            .into_iter()
            .map(|row| (row.text, row.done))
            .collect();
        assert_eq!(
            rows,
            [
                ("a".to_owned(), true),
                ("b".to_owned(), true),
                ("c".to_owned(), false),
                ("P".to_owned(), true),
                ("d".to_owned(), false),
            ]
        );
    }

    #[test]
    fn empty_document_has_no_rows() {
        assert_eq!(State::new(Document::default(), None).rows(), Vec::new());
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
        for kind in [ItemKind::Project, ItemKind::Task, ItemKind::Note] {
            assert!(
                rows.iter().any(|row| row.kind == kind),
                "demo lacks a {kind:?}"
            );
        }
        assert!(rows.iter().any(|row| row.depth >= 2), "demo lacks nesting");
        assert_eq!(
            rows.first().map(|row| row.text.as_str()),
            Some("Trip to Lisbon")
        );
        assert!(
            rows.iter()
                .any(|row| row.kind == ItemKind::Task && row.done),
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
            rows.iter().all(|row| !row.text.contains('@')),
            "demo shows a tag"
        );
    }
}
