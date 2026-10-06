//! Application state and what it exposes for a frontend to show.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    time::Duration,
};

use jiff::civil::Date;

use super::{
    Command, History,
    date_prompt::DatePrompt,
    edit_mode::{Resume, Session},
    external::External,
    finder::Finder,
    picker::Picker,
    saving::Autosave,
    selection::Selection,
    sidebar::Sidebar,
    type_picker::Conversion,
};
use crate::document::{
    Block, BlockKind, Document, Item, ItemId, TagColor, blocks,
    inline::{self, Syntax},
};

/// How long a status-line notice stays.
pub const NOTICE_TIME: Duration = Duration::from_secs(10);

/// Everything the app knows: the open document with its undo history, where it came from and
/// whether it is saved, the focused row and any edit session on it, the selected blocks, the
/// collapsed blocks, whether completed blocks show, the zoom with its history, the sidebar and
/// the status-line notice.
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
    /// The frontend's local date at the latest tick, which the Today view lists blocks for (D-7).
    pub(super) today: Date,
    /// The notice the status line shows, and until when.
    notice: Option<(Notice, Duration)>,
    pub(super) focus: Option<ItemId>,
    /// The selected blocks (FR-OPS-01).
    pub(super) selection: Selection,
    /// The edit session of the focused block, in edit mode (FR-MODE-01).
    pub(super) edit: Option<Session>,
    /// The color picker over the focused block, while it is open (FR-TAG-05).
    pub(super) color_picker: Option<Picker<Option<TagColor>>>,
    /// The prompt for a block's start or due date, while it is open (FR-TAG-03).
    pub(super) date_prompt: Option<DatePrompt>,
    /// The type picker over the focused block, while it is open (FR-BLK-08).
    pub(super) type_picker: Option<Picker<Conversion>>,
    /// Whether the details popup is open over the focused block (D-3).
    pub(super) details: bool,
    /// Go to Symbol, while it is open (FR-NAV-05).
    pub(super) symbols: Option<Finder<ItemId>>,
    /// Move to, while it is open (FR-OPS-05).
    pub(super) destinations: Option<Finder<ItemId>>,
    /// Go to Command, while it is open (FR-NAV-06).
    pub(super) commands: Option<Finder<Command>>,
    /// Where editing resumes once a command run from the command menu is done (D-50).
    pub(super) resume: Option<Resume>,
    /// The ids of collapsed blocks, kept in memory only (D-26).
    pub(super) folds: HashSet<ItemId>,
    /// Whether done blocks show, with everything inside them (FR-DONE-02).
    pub(super) show_done: bool,
    /// Whether the shortcuts panel is open (FR-KEY-04).
    pub(super) shortcuts: bool,
    /// What the pane shows: the outline below a zoom, or the Today view (FR-NAV-03, D-7).
    pub(super) location: Location,
    /// Every location visited, oldest first, for back and forward (FR-NAV-04).
    pub(super) history: Vec<Location>,
    /// The position of the current location in `history`.
    pub(super) history_at: usize,
    /// The navigation sidebar (FR-UI-01).
    pub(super) sidebar: Sidebar,
}

/// What a pane shows (navigation.md §4, §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Location {
    /// The outline below this zoom block, or below the document root at home (FR-NAV-03).
    Outline(Option<ItemId>),
    /// The Today view: a flat list of the blocks for today (FR-TAG-04, D-7).
    Today,
}

impl Location {
    /// This location with its zoom block replaced by what `rematch` finds for it, home when it
    /// finds nothing.
    pub(super) fn rematched(self, rematch: impl FnOnce(ItemId) -> Option<ItemId>) -> Self {
        match self {
            Self::Outline(zoom) => Self::Outline(zoom.and_then(rematch)),
            Self::Today => Self::Today,
        }
    }
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
    /// Typing would have made a tag, which TaskPaper has no way to escape; tags are set with
    /// commands (OQ-25).
    TypedTag,
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
    /// Empty lines ending the outline below the top level were moved to the top level before
    /// saving, where the file reads them back (file-format.md §3.1, rule 3).
    BlanksLifted,
    /// Making sections, or plain text of sections, skipped this many blocks that are tasks or
    /// projects, which cannot be sections (D-52).
    NotSections {
        /// How many.
        skipped: usize,
    },
    /// A bookmark was added to the focused block, or removed when `added` is false (D-11).
    Bookmark {
        /// Whether the block is bookmarked now.
        added: bool,
    },
    /// How many blocks are under the focused block and how many words they hold (FR-OPS-09).
    Count {
        /// The blocks.
        blocks: usize,
        /// The words of their text.
        words: usize,
    },
    /// The browser could not be started on a URL (FR-BLK-06).
    OpenUrlFailed {
        /// The URL.
        url: String,
        /// What the frontend reported.
        reason: String,
    },
}

/// One visible block of the outline.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent facts about a block that a frontend shows, not states of one machine"
)]
pub struct Row {
    /// The block this row shows: the id of its line.
    pub id: ItemId,
    /// Depth below the top level, starting at 0.
    pub depth: usize,
    /// The block's type.
    pub kind: BlockKind,
    /// Whether it is a section: a text block carrying `@section` (FR-BLK-04, D-52).
    pub section: bool,
    /// Whether it is a divider: a text block reading `---` (FR-BLK-07, D-52).
    pub divider: bool,
    /// Its text without type syntax and tags.
    pub text: String,
    /// Whether the block carries `@done`, with or without a value (D-17).
    pub done: bool,
    /// Whether a block above it is done, also one above the zoom; its own `@done` is `done`
    /// (D-40).
    pub inside_done: bool,
    /// How many child blocks its fold hides; 0 when it is expanded or has none.
    pub hidden_children: usize,
    /// Whether this is the title row of a zoomed view: the zoom block itself, above its children.
    pub title: bool,
    /// The date of its `@start`, if it holds one (FR-TAG-03).
    pub start: Option<Date>,
    /// The date of its `@due`, if it holds one (FR-TAG-03).
    pub due: Option<Date>,
    /// Whether it carries `@today` (FR-TAG-04).
    pub today: bool,
    /// Whether it carries `@bookmark` (D-11).
    pub bookmark: bool,
    /// The color that tints its row's background (FR-TAG-05, D-48).
    pub color: Option<TagColor>,
    /// Whether its line carries `@url`, so its title is a link (FR-BLK-06, D-45).
    pub url: bool,
    /// Whether it is selected (FR-OPS-01).
    pub selected: bool,
    /// The text of each block above it, from the top level down, as an entry of the Today view
    /// shows it; empty in an outline (D-7).
    pub path: Vec<String>,
}

impl Row {
    /// Whether its due date has passed on `today`, the local date; a block that is done, or
    /// inside a done block, is overdue no longer (FR-TAG-03).
    #[must_use]
    pub fn overdue(&self, today: Date) -> bool {
        !self.done && !self.inside_done && self.due.is_some_and(|due| due < today)
    }
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
            today: Date::default(),
            notice: None,
            focus: None,
            selection: Selection::default(),
            edit: None,
            color_picker: None,
            date_prompt: None,
            type_picker: None,
            details: false,
            symbols: None,
            destinations: None,
            commands: None,
            resume: None,
            folds: HashSet::new(),
            show_done: true,
            shortcuts: false,
            location: Location::Outline(None),
            history: vec![Location::Outline(None)],
            history_at: 0,
            sidebar: Sidebar::default(),
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

    /// The block shown as the root, or `None` at home and in the Today view (FR-NAV-03).
    #[must_use]
    pub fn zoom(&self) -> Option<ItemId> {
        match self.location {
            Location::Outline(zoom) => zoom,
            Location::Today => None,
        }
    }

    /// What the pane shows (navigation.md §4, §7).
    #[must_use]
    pub fn location(&self) -> Location {
        self.location
    }

    /// The text of each block from the top level down to the zoom block as shown, without the
    /// delimiters of its formatting; empty at home and in the Today view (FR-NAV-03, FR-FMT-01,
    /// FR-FMT-03).
    #[must_use]
    pub fn breadcrumbs(&self) -> Vec<String> {
        self.zoom_block()
            .map(|block| self.path_to(&block))
            .unwrap_or_default()
    }

    /// The text of each block from the top level down to `block`, itself included, as
    /// [`State::shown_text`] gives it.
    pub(super) fn path_to(&self, block: &Block) -> Vec<String> {
        let mut path: Vec<String> = std::iter::successors(Some(block.clone()), |block| {
            blocks::parent(&self.document, block.id())
        })
        .map(|block| self.shown_text(&block))
        .collect();
        path.reverse();
        path
    }

    /// The text of `block` as shown, without the delimiters of its formatting.
    pub(super) fn shown_text(&self, block: &Block) -> String {
        let text = self
            .document
            .get(block.id())
            .map(Item::display_text)
            .unwrap_or_default();
        let runs = inline::runs(&text, Syntax::of(block.kind()));
        runs.iter().map(|run| run.text).collect()
    }

    /// The visible rows: the outline's, or in the Today view its entries (D-7).
    #[must_use]
    pub fn rows(&self) -> Vec<Row> {
        match self.location {
            Location::Outline(_) => self.outline_rows(),
            Location::Today => self.today_rows(),
        }
    }

    /// The outline's rows: when zoomed, a title row for the zoom block at depth 0, then its child
    /// blocks from depth 1, so depth always follows the hierarchy (D-31). Collapsed blocks hide
    /// their children, done blocks are hidden with theirs while completed blocks are, and special
    /// nodes are never shown (FR-NAV-02, FR-NAV-03, FR-DONE-02, FR-SPC-01). A row inside a done
    /// block, also one above the zoom, says so (D-40).
    fn outline_rows(&self) -> Vec<Row> {
        let title = self.zoom_block().map(|block| {
            let inside_done =
                std::iter::successors(blocks::parent(&self.document, block.id()), |above| {
                    blocks::parent(&self.document, above.id())
                })
                .any(|above| above.done());
            self.row(&block, 0, true, inside_done)
        });
        let below = usize::from(title.is_some());
        let top_done = title
            .as_ref()
            .is_some_and(|row| row.done || row.inside_done);
        // Whether each block on the path to the current one is done or inside a done block.
        let mut path: Vec<bool> = Vec::new();
        let rows = self.blocks(true).into_iter().map(|(block, depth)| {
            path.truncate(depth);
            let inside_done = path.last().copied().unwrap_or(top_done);
            path.push(inside_done || block.done());
            self.row(&block, depth + below, false, inside_done)
        });
        title.into_iter().chain(rows).collect()
    }

    /// The row of `block` at `depth` in an outline, the title row with `title`.
    pub(super) fn row(&self, block: &Block, depth: usize, title: bool, inside_done: bool) -> Row {
        Row {
            id: block.id(),
            depth,
            kind: block.kind(),
            section: block.section(),
            divider: block.divider(),
            text: self
                .document
                .get(block.id())
                .map(Item::display_text)
                .unwrap_or_default(),
            done: block.done(),
            inside_done,
            hidden_children: if !title && self.folds.contains(&block.id()) {
                self.children(block).len()
            } else {
                0
            },
            title,
            start: block.start(),
            due: block.due(),
            today: block.today(),
            bookmark: block.bookmark(),
            color: block.color(),
            url: block.url(),
            selected: self.selection.contains(block.id()),
            path: Vec::new(),
        }
    }

    /// The zoom block, or `None` at home.
    pub(super) fn zoom_block(&self) -> Option<Block> {
        blocks::block(&self.document, self.zoom()?)
    }

    /// Every block below the zoom in order with its depth; with `visible`, only those the rows
    /// show: none hidden by a collapsed ancestor or, while completed blocks are hidden, by being
    /// done or inside a done block. The zoom block's own fold hides nothing.
    pub(super) fn blocks(&self, visible: bool) -> Vec<(Block, usize)> {
        let top = self
            .zoom_block()
            .map_or(self.document.root(), |block| block.id());
        let mut out = Vec::new();
        self.push_blocks(top, 0, visible, &mut out);
        out
    }

    fn push_blocks(
        &self,
        line: ItemId,
        depth: usize,
        visible: bool,
        out: &mut Vec<(Block, usize)>,
    ) {
        for block in blocks::children(&self.document, line) {
            if visible && !self.shows(&block) {
                continue;
            }
            let id = block.id();
            let open = !(visible && self.folds.contains(&id));
            out.push((block, depth));
            if open {
                self.push_blocks(id, depth + 1, visible, out);
            }
        }
    }

    /// Whether `block` shows where the blocks around it do: not when it is done while completed
    /// blocks are hidden (FR-DONE-02).
    pub(super) fn shows(&self, block: &Block) -> bool {
        self.show_done || !block.done()
    }

    /// The child blocks of `block` that show when it is expanded (FR-DONE-02).
    pub(super) fn shown_children(&self, block: &Block) -> Vec<Block> {
        let mut children = self.children(block);
        children.retain(|child| self.shows(child));
        children
    }

    /// The child blocks of `block`.
    pub(super) fn children(&self, block: &Block) -> Vec<Block> {
        blocks::children(&self.document, block.id())
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
            .map(|row| (row.depth, row.kind, row.text))
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
                (2, BlockKind::Text, "c".to_owned()),
                (0, BlockKind::Text, "d".to_owned()),
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

    // D-43, FR-SPC-01
    #[test]
    fn every_line_is_a_row_and_special_nodes_no_row() {
        let state = State::new(
            format::parse("P:\n\tone @x\n\n\ttwo\n\t\tC @hidden\n\t\t\tc\n\t\t- child\n\t--- @y"),
            None,
        );
        assert_eq!(
            summary(&state),
            [
                (0, BlockKind::Project, "P".to_owned()),
                (1, BlockKind::Text, "one".to_owned()),
                (1, BlockKind::Text, String::new()),
                (1, BlockKind::Text, "two".to_owned()),
                (2, BlockKind::Task, "child".to_owned()),
                (1, BlockKind::Text, "---".to_owned()),
            ]
        );
        let flags: Vec<(bool, bool)> = state
            .rows()
            .iter()
            .map(|row| (row.section, row.divider))
            .collect();
        assert_eq!(flags.last(), Some(&(false, true)), "a divider is text");
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
                ("n".to_owned(), false),
                ("m".to_owned(), true),
            ]
        );
    }

    // FR-TAG-03
    #[test]
    fn rows_carry_their_dates_and_say_when_they_are_overdue() {
        let today = jiff::civil::date(2026, 10, 5);
        let state = State::new(
            format::parse(
                "- a @start(2026-10-01) @due(2026-10-04)\n- b @due(2026-10-05)\n- c @due(x)\n- d @due(2026-10-04) @done\nP: @done\n\t- e @due(2026-10-01)\n- f @start(2026-10-01)",
            ),
            None,
        );
        let dates: Vec<_> = state
            .rows()
            .into_iter()
            .map(|row| (row.text.clone(), row.start, row.due, row.overdue(today)))
            .collect();
        let day = |d| Some(jiff::civil::date(2026, 10, d));
        let row = |text: &str, start, due, overdue| (text.to_owned(), start, due, overdue);
        assert_eq!(
            dates,
            [
                row("a", day(1), day(4), true),
                row("b", None, day(5), false),
                row("c", None, None, false),
                row("d", None, day(4), false),
                row("P", None, None, false),
                row("e", None, day(1), false),
                row("f", day(1), None, false),
            ]
        );
    }

    /// Each row's text, whether it is done and whether it is inside a done block.
    fn completion(state: &State) -> Vec<(String, bool, bool)> {
        state
            .rows()
            .into_iter()
            .map(|row| (row.text, row.done, row.inside_done))
            .collect()
    }

    // FR-DONE-01, D-40
    #[test]
    fn rows_inside_a_done_block_of_any_type_are_marked_at_any_depth() {
        let state = State::new(
            format::parse(
                "P: @done\n\t- a\n\t\tn\n\t- b @done\nQ:\n\t- c\n\t\tS @section @done\n\t\t\tm\n\t\t\t\t- e\n\t- f\no @done(1)\n\t- g\n- h",
            ),
            None,
        );
        let row = |text: &str, done, inside| (text.to_owned(), done, inside);
        assert_eq!(
            completion(&state),
            [
                row("P", true, false),
                row("a", false, true),
                row("n", false, true),
                row("b", true, true),
                row("Q", false, false),
                row("c", false, false),
                row("S", true, false),
                row("m", false, true),
                row("e", false, true),
                row("f", false, false),
                row("o", true, false),
                row("g", false, true),
                row("h", false, false),
            ]
        );
    }

    // FR-DONE-01, FR-NAV-03, D-40
    #[test]
    fn a_zoom_inside_or_onto_a_done_block_marks_its_title_and_rows() {
        let zoomed = |text: &str, down: usize| {
            let mut state = State::new(format::parse(text), None);
            for _ in 0..down {
                state.update(crate::app::Message::FocusNext);
            }
            state.update(crate::app::Message::ZoomIn);
            completion(&state)
        };
        let row = |text: &str, done, inside| (text.to_owned(), done, inside);
        assert_eq!(
            zoomed("P: @done\n\tQ:\n\t\t- a\n\t\t\t- b", 1),
            [
                row("Q", false, true),
                row("a", false, true),
                row("b", false, true)
            ],
            "a done block above the zoom"
        );
        assert_eq!(
            zoomed("P:\n\tQ: @done\n\t\t- a\n\t\t\t- b", 1),
            [
                row("Q", true, false),
                row("a", false, true),
                row("b", false, true)
            ],
            "a done zoom block"
        );
        assert_eq!(
            zoomed("P:\n\tQ:\n\t\t- a @done\n\t\t\t- b\n\t\t- c", 1),
            [
                row("Q", false, false),
                row("a", true, false),
                row("b", false, true),
                row("c", false, false),
            ],
            "nothing done above the zoom"
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
