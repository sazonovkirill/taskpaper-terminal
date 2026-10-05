//! Application state and what it exposes for a frontend to show.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    time::Duration,
};

use super::{History, external::External, saving::Autosave};
use crate::document::{Block, BlockKind, Document, Item, ItemId, blocks};

/// How long a status-line notice stays.
pub const NOTICE_TIME: Duration = Duration::from_secs(10);

/// Everything the app knows: the open document with its undo history, where it came from and
/// whether it is saved, the focused row, the collapsed blocks, the zoom with its history and the
/// status-line notice.
#[derive(Debug, Clone)]
pub struct State {
    pub(super) document: Document,
    /// The edits to undo and redo (FR-OPS-08).
    pub(super) edits: History,
    path: Option<PathBuf>,
    origin: Origin,
    access: Access,
    pub(super) autosave: Autosave,
    /// The file on disk as the core knows it, and a conflict with it.
    pub(super) external: External,
    /// The frontend's time at the latest tick.
    pub(super) now: Duration,
    /// The notice the status line shows, and until when.
    notice: Option<(Notice, Duration)>,
    pub(super) focus: Option<ItemId>,
    /// The ids of collapsed blocks, kept in memory only (D-26).
    pub(super) folds: HashSet<ItemId>,
    /// The block shown as the root, or `None` at home (FR-NAV-03).
    pub(super) zoom: Option<ItemId>,
    /// Every zoom visited, oldest first, for back and forward (FR-NAV-04).
    pub(super) history: Vec<Option<ItemId>>,
    /// The position of the current zoom in `history`.
    pub(super) history_at: usize,
}

/// Whether the open file may be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Changes are saved to it.
    ReadWrite,
    /// It is never written, because the user may not write it (file-io.md §1).
    ReadOnly,
}

/// Who chose the open file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// The user named it.
    Chosen,
    /// The app made it in `/tmp` for a start without a path; it is not backed up and the status
    /// line marks it until it is saved elsewhere (file-io.md §1, §2).
    Temporary,
}

/// A message the status line shows for a while.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// A backup could not be made, for this reason; shown once per session (FR-DOC-07).
    BackupFailed {
        /// What the frontend reported.
        reason: String,
    },
    /// An edit was refused because the file is read-only (file-io.md §1).
    ReadOnly,
    /// An edit, undo or redo did not fit the document, for this reason, and changed nothing.
    EditFailed {
        /// What did not fit.
        reason: String,
    },
    /// The file changed on disk and was reloaded (file-io.md §3).
    Reloaded,
    /// A conflict was resolved by keeping the local version; the file's version is in `copy`.
    KeptMine {
        /// The conflict copy.
        copy: PathBuf,
    },
    /// A conflict was resolved by reloading the file; the local version is in `copy`.
    TookTheirs {
        /// The conflict copy.
        copy: PathBuf,
    },
    /// The conflict copy for a choice could not be written, so the choice was cancelled.
    ConflictCopyFailed {
        /// What the frontend reported.
        reason: String,
    },
    /// The file on disk cannot be taken because it cannot be read as a document.
    CannotTakeTheirs {
        /// Why.
        reason: String,
    },
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
    /// Whether this is the title row of a zoomed view: the zoom block itself, above its children.
    pub title: bool,
}

impl State {
    /// State for a document opened from `path`, which holds it as it is, or held only in memory
    /// when `path` is `None`.
    #[must_use]
    pub fn new(document: Document, path: Option<PathBuf>) -> Self {
        let mut state = Self {
            autosave: Autosave::new(document.revision()),
            external: External::default(),
            document,
            edits: History::default(),
            path,
            origin: Origin::Chosen,
            access: Access::ReadWrite,
            now: Duration::ZERO,
            notice: None,
            focus: None,
            folds: HashSet::new(),
            zoom: None,
            history: vec![None],
            history_at: 0,
        };
        state.focus = state.rows().first().map(|row| row.id);
        state
    }

    /// State for a document recovered after a crash for the file at `path`, which lacks it, so
    /// it is saved like a change (file-io.md §4).
    #[must_use]
    pub fn recovered(document: Document, path: Option<PathBuf>) -> Self {
        Self {
            autosave: Autosave::unsaved(document.revision()),
            ..Self::new(document, path)
        }
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

    /// This state with the file made by `origin`.
    #[must_use]
    pub fn with_origin(self, origin: Origin) -> Self {
        Self { origin, ..self }
    }

    /// Who chose the open file.
    #[must_use]
    pub fn origin(&self) -> Origin {
        self.origin
    }

    /// This state with the file opened with `access`.
    #[must_use]
    pub fn with_access(self, access: Access) -> Self {
        Self { access, ..self }
    }

    /// Whether the open file may be written.
    #[must_use]
    pub fn access(&self) -> Access {
        self.access
    }

    /// The notice the status line shows, if any.
    #[must_use]
    pub fn notice(&self) -> Option<&Notice> {
        self.notice.as_ref().map(|(notice, _)| notice)
    }

    /// Shows `notice` for [`NOTICE_TIME`] from the latest tick.
    pub(super) fn notify(&mut self, notice: Notice) {
        self.notice = Some((notice, self.now.saturating_add(NOTICE_TIME)));
    }

    /// Drops the notice once its time is up.
    pub(super) fn expire_notice(&mut self) {
        if self
            .notice
            .as_ref()
            .is_some_and(|&(_, until)| self.now >= until)
        {
            self.notice = None;
        }
    }

    /// When the core next has work to do if only the clock moves: a save falls due or a notice
    /// expires.
    #[must_use]
    pub fn deadline(&self) -> Option<Duration> {
        let notice = self.notice.as_ref().map(|&(_, until)| until);
        notice.into_iter().chain(self.save_deadline()).min()
    }

    /// The focused row's item: the first row when opened, `None` only when there are no rows
    /// (FR-NAV-01).
    #[must_use]
    pub fn focus(&self) -> Option<ItemId> {
        self.focus
    }

    /// The block shown as the root, or `None` at home (FR-NAV-03).
    #[must_use]
    pub fn zoom(&self) -> Option<ItemId> {
        self.zoom
    }

    /// The text of the first line of each block from the top level down to the zoom block; empty
    /// at home (FR-NAV-03).
    #[must_use]
    pub fn breadcrumbs(&self) -> Vec<String> {
        let mut crumbs: Vec<String> = std::iter::successors(self.zoom_block(), |block| {
            blocks::parent(&self.document, block.id())
        })
        .filter_map(|block| self.document.get(block.id()))
        .map(Item::display_text)
        .collect();
        crumbs.reverse();
        crumbs
    }

    /// The visible rows: when zoomed, a title row for the zoom block at depth 0, then its child
    /// blocks from depth 1, so depth always follows the hierarchy (D-31). Collapsed blocks hide
    /// their children and special nodes are never shown (FR-NAV-02, FR-NAV-03, FR-SPC-01).
    #[must_use]
    pub fn rows(&self) -> Vec<Row> {
        let title = self.zoom_block().map(|block| self.row(&block, 0, true));
        let below = usize::from(title.is_some());
        title
            .into_iter()
            .chain(
                self.blocks(true)
                    .into_iter()
                    .map(|(block, depth)| self.row(&block, depth + below, false)),
            )
            .collect()
    }

    fn row(&self, block: &Block, depth: usize, title: bool) -> Row {
        Row {
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
            hidden_children: if !title && self.folds.contains(&block.id()) {
                self.children(block).len()
            } else {
                0
            },
            title,
        }
    }

    /// The zoom block, or `None` at home.
    pub(super) fn zoom_block(&self) -> Option<Block> {
        blocks::block(&self.document, self.zoom?)
    }

    /// Every block below the zoom in order with its depth; with `folded`, only those not hidden
    /// by a collapsed ancestor. The zoom block's own fold hides nothing.
    pub(super) fn blocks(&self, folded: bool) -> Vec<(Block, usize)> {
        let top = self
            .zoom_block()
            .map_or(self.document.root(), |block| block.last_line());
        let mut out = Vec::new();
        self.push_blocks(top, 0, folded, &mut out);
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
    use crate::document::format;

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
        assert_eq!(State::new(Document::default(), None).path(), None);
    }

    // FR-DOC-05
    #[test]
    fn a_file_is_chosen_unless_marked_temporary() {
        let state = State::new(Document::default(), Some(PathBuf::from("/tmp/t.taskpaper")));
        assert_eq!(state.origin(), Origin::Chosen);
        let temporary = state.with_origin(Origin::Temporary);
        assert_eq!(temporary.origin(), Origin::Temporary);
        assert_eq!(temporary.path(), Some(Path::new("/tmp/t.taskpaper")));
    }
}
