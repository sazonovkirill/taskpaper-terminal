//! The update function: messages are user intents and reports from the frontend, effects are
//! what the frontend must do.

use std::{path::PathBuf, time::Duration};

use jiff::civil::Date;

use super::{Mode, State, TextEdit, Version, saving::Ending, structure::NewBlock};
use crate::document::{Block, Item, ItemId, blocks};

/// A user intent or a report from the frontend, independent of how a frontend receives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// The frontend is ready to carry out effects; sent once, first.
    Started,
    /// The frontend's clock reads `now`, the time since it started. A frontend sends one before
    /// every other message and whenever [`State::deadline`] passes.
    Tick {
        /// The time since the frontend started.
        now: Duration,
    },
    /// The [`Disk::Save`] under way succeeded.
    Saved,
    /// The [`Disk::Save`] under way failed.
    SaveFailed {
        /// Why, for the status line.
        reason: String,
    },
    /// The [`Disk::Save`] under way was not done: the file on disk is no longer the version it
    /// expected (file-io.md §3).
    SaveRefused {
        /// The file's bytes now, or `None` when there is no file.
        contents: Option<Vec<u8>>,
    },
    /// The open file may have changed on disk: the frontend saw it change or regained focus.
    CheckFile,
    /// What a [`Disk::Check`] found.
    FileChecked {
        /// The file's bytes, or `None` when there is no file.
        contents: Option<Vec<u8>>,
    },
    /// Resolve the conflict by overwriting the file on disk with the local version.
    KeepMine,
    /// Resolve the conflict by reloading the file on disk, discarding the local version.
    TakeTheirs,
    /// Show or hide the diff between the file on disk and the local version.
    ToggleDiff,
    /// The [`Disk::ConflictCopy`] under way was written.
    ConflictCopied {
        /// Where the copy is.
        copy: PathBuf,
    },
    /// The [`Disk::ConflictCopy`] under way failed.
    ConflictCopyFailed {
        /// Why, for the status line.
        reason: String,
    },
    /// A [`Disk::Backup`] failed.
    BackupFailed {
        /// Why, for the status line.
        reason: String,
    },
    /// Leave the app, saving first; if that save fails, ask (file-io.md §2).
    Quit,
    /// The process is told to end (`SIGTERM`, `SIGHUP`): save first and end even if that fails.
    Terminate,
    /// Focus the next visible row.
    FocusNext,
    /// Focus the previous visible row.
    FocusPrevious,
    /// Focus the first visible row.
    FocusFirst,
    /// Focus the last visible row.
    FocusLast,
    /// Move focus down by a page of `rows` rows, the height of the frontend's view.
    PageDown {
        /// Rows in a page.
        rows: usize,
    },
    /// Move focus up by a page of `rows` rows, the height of the frontend's view.
    PageUp {
        /// Rows in a page.
        rows: usize,
    },
    /// Focus the parent block.
    FocusParent,
    /// Focus the next sibling block.
    FocusNextSibling,
    /// Focus the previous sibling block.
    FocusPreviousSibling,
    /// Focus the focused block's last child, expanding the block if it is collapsed.
    FocusLastChild,
    /// Collapse the focused block; if it is a leaf or already collapsed, focus its parent.
    Collapse,
    /// Expand the focused block; if it is already expanded, focus its first child.
    Expand,
    /// Expand the focused block if it is collapsed, else collapse it.
    ToggleFold,
    /// Collapse the focused block's whole subtree if any of it is expanded, else expand it all.
    ToggleSubtree,
    /// Expand every collapsed block at the shallowest depth that has one.
    ExpandLevel,
    /// Collapse every visible expanded block at the deepest depth that has one.
    CollapseLevel,
    /// Expand every block under the zoom.
    ExpandAll,
    /// Collapse every block under the zoom.
    CollapseAll,
    /// Show the focused block as the root.
    ZoomIn,
    /// Show the zoom block's parent as the root, focusing the block zoomed out of.
    ZoomOut,
    /// Show the whole document.
    GoHome,
    /// Return to the previous zoom in the history.
    Back,
    /// Go to the next zoom in the history.
    Forward,
    /// Complete the focused block, or uncomplete it when it is done, dating it `today`
    /// (FR-DONE-01).
    ToggleDone {
        /// The frontend's local date.
        today: Date,
    },
    /// Revert the latest edit (FR-OPS-08); in edit mode, the latest change to the text.
    Undo,
    /// Replay the latest undone edit (FR-OPS-08); in edit mode, the latest undone change to the
    /// text.
    Redo,
    /// Edit the focused block's text with the caret at its end (FR-MODE-01).
    EditAtEnd,
    /// Edit the focused block's text with the caret at its start (FR-MODE-01).
    EditAtStart,
    /// Change the text being edited (FR-MODE-03).
    Text(TextEdit),
    /// Stop editing and return to navigate mode on the same block (FR-MODE-01).
    StopEditing,
    /// Add a block after the focused block's subtree and edit it (FR-MODE-04).
    NewBelow,
    /// Add a block before the focused block and edit it (FR-MODE-04).
    NewAbove,
    /// Add a first child to the focused block, expanding it, and edit it (FR-MODE-04).
    NewChild,
    /// Stop editing and add the next block: a first child when the block shows children, else
    /// a block below it (editing.md §3).
    NewNext,
    /// Delete the focused block with its subtree (FR-OPS-06).
    Delete,
}

impl Message {
    /// Whether this intent belongs to navigate mode, so that it ends an edit session first,
    /// rather than to edit mode or the frontend's reports, which leave the session alone.
    fn ends_editing(&self) -> bool {
        match self {
            Self::Started
            | Self::Tick { .. }
            | Self::Saved
            | Self::SaveFailed { .. }
            | Self::SaveRefused { .. }
            | Self::CheckFile
            | Self::FileChecked { .. }
            | Self::ToggleDiff
            | Self::ConflictCopied { .. }
            | Self::ConflictCopyFailed { .. }
            | Self::BackupFailed { .. }
            | Self::Undo
            | Self::Redo
            | Self::EditAtEnd
            | Self::EditAtStart
            | Self::Text(_)
            | Self::StopEditing => false,
            Self::Quit
            | Self::Terminate
            | Self::KeepMine
            | Self::TakeTheirs
            | Self::FocusNext
            | Self::FocusPrevious
            | Self::FocusFirst
            | Self::FocusLast
            | Self::PageDown { .. }
            | Self::PageUp { .. }
            | Self::FocusParent
            | Self::FocusNextSibling
            | Self::FocusPreviousSibling
            | Self::FocusLastChild
            | Self::Collapse
            | Self::Expand
            | Self::ToggleFold
            | Self::ToggleSubtree
            | Self::ExpandLevel
            | Self::CollapseLevel
            | Self::ExpandAll
            | Self::CollapseAll
            | Self::ZoomIn
            | Self::ZoomOut
            | Self::GoHome
            | Self::Back
            | Self::Forward
            | Self::ToggleDone { .. }
            | Self::NewBelow
            | Self::NewAbove
            | Self::NewChild
            | Self::NewNext
            | Self::Delete => true,
        }
    }
}

/// Work the frontend performs for the core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// End the session.
    Quit,
    /// Disk work, carried out in order and away from the frontend's event handling (N-UX-01).
    Disk(Disk),
}

/// Disk work for the frontend; each kind reports its result back as a [`Message`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Disk {
    /// Make sure today's backup copy of the file at `path` exists, copying the file as it is on
    /// disk if not (FR-DOC-07); a failure comes back as [`Message::BackupFailed`].
    Backup {
        /// The open file.
        path: PathBuf,
    },
    /// Replace the file at `path` with `text` atomically (F-FILE-04), unless just before that
    /// the file is no longer at `expected`; the result comes back as [`Message::Saved`],
    /// [`Message::SaveFailed`] or [`Message::SaveRefused`] (file-io.md §3).
    Save {
        /// The open file.
        path: PathBuf,
        /// The serialized document.
        text: String,
        /// The version the file must still be, or `None` when there must be no file.
        expected: Option<Version>,
    },
    /// Read the file at `path`; the result comes back as [`Message::FileChecked`].
    Check {
        /// The open file.
        path: PathBuf,
    },
    /// Write `contents` to a new conflict copy in the backups folder of the file at `path`, even
    /// when backups are off; the result comes back as [`Message::ConflictCopied`] or
    /// [`Message::ConflictCopyFailed`] (file-io.md §3).
    ConflictCopy {
        /// The open file.
        path: PathBuf,
        /// The version a conflict choice discards.
        contents: Vec<u8>,
    },
}

impl State {
    /// Applies `message` and returns the effects the frontend must carry out.
    pub fn update(&mut self, message: Message) -> Vec<Effect> {
        let mut effects = Vec::new();
        if message.ends_editing() {
            self.stop_editing();
        }
        match message {
            Message::Started => effects = self.backup_on_open(),
            Message::Tick { now } => self.now = now,
            Message::Saved => self.saved(),
            Message::SaveFailed { reason } => self.save_failed(reason),
            Message::SaveRefused { contents } => self.save_refused(contents),
            Message::CheckFile => effects = self.check_file(),
            Message::FileChecked { contents } => self.found(contents),
            Message::KeepMine => effects = self.keep_mine(),
            Message::TakeTheirs => effects = self.take_theirs(),
            Message::ToggleDiff => self.toggle_diff(),
            Message::ConflictCopied { copy } => self.conflict_copied(copy),
            Message::ConflictCopyFailed { reason } => self.conflict_copy_failed(reason),
            Message::BackupFailed { reason } => self.backup_failed(reason),
            Message::Quit => self.autosave.end(Ending::Quit),
            Message::Terminate => self.autosave.end(Ending::Terminate),
            Message::FocusNext => self.move_focus(|at, last| at.saturating_add(1).min(last)),
            Message::FocusPrevious => self.move_focus(|at, _| at.saturating_sub(1)),
            Message::FocusFirst => self.move_focus(|_, _| 0),
            Message::FocusLast => self.move_focus(|_, last| last),
            Message::PageDown { rows } => {
                self.move_focus(|at, last| at.saturating_add(rows).min(last));
            }
            Message::PageUp { rows } => self.move_focus(|at, _| at.saturating_sub(rows)),
            Message::FocusParent => self.focus_parent(),
            Message::FocusNextSibling => self.move_to_sibling(|at| at.checked_add(1)),
            Message::FocusPreviousSibling => self.move_to_sibling(|at| at.checked_sub(1)),
            Message::FocusLastChild => self.focus_last_child(),
            Message::Collapse => self.collapse(),
            Message::Expand => self.expand(),
            Message::ToggleFold => self.toggle_fold(),
            Message::ToggleSubtree => self.toggle_subtree(),
            Message::ExpandLevel => self.expand_level(),
            Message::CollapseLevel => self.collapse_level(),
            Message::ExpandAll => {
                for (block, _) in self.blocks(false) {
                    self.folds.remove(&block.id());
                }
            }
            Message::CollapseAll => {
                let all = self.blocks(false).into_iter().map(|(block, _)| block);
                self.fold(all);
            }
            Message::ZoomIn => {
                if let Some(block) = self.focused_block() {
                    self.go_to(Some(block.id()));
                }
            }
            Message::ZoomOut => {
                if let Some(zoom) = self.zoom {
                    self.go_to(blocks::parent(self.document(), zoom).map(|block| block.id()));
                }
            }
            Message::GoHome => self.go_to(None),
            Message::Back => self.walk_history(self.history_at.checked_sub(1)),
            Message::Forward => self.walk_history(self.history_at.checked_add(1)),
            Message::ToggleDone { today } => self.toggle_done(today),
            Message::Undo => match self.mode() {
                Mode::Navigate => self.undo(),
                Mode::Edit => self.undo_typing(),
            },
            Message::Redo => match self.mode() {
                Mode::Navigate => self.redo(),
                Mode::Edit => self.redo_typing(),
            },
            Message::EditAtEnd => self.start_editing(true),
            Message::EditAtStart => self.start_editing(false),
            Message::Text(edit) => self.edit_text(edit),
            Message::StopEditing => self.stop_editing(),
            Message::NewBelow => self.add_block(NewBlock::Below),
            Message::NewAbove => self.add_block(NewBlock::Above),
            Message::NewChild => self.add_block(NewBlock::Child),
            Message::NewNext => self.add_block(NewBlock::Next),
            Message::Delete => self.delete_focused(),
        }
        self.repair_focus();
        self.expire_notice();
        effects.extend(self.save_if_due());
        effects
    }

    /// The focused block and whether it is the title row.
    pub(super) fn focused(&self) -> Option<(Block, bool)> {
        let id = self.focus?;
        Some((blocks::block(self.document(), id)?, Some(id) == self.zoom))
    }

    /// The focused block unless it is the title, which stands for the view's root: it does not
    /// fold, zoom in or move to its parent or siblings.
    fn focused_block(&self) -> Option<Block> {
        self.focused()
            .and_then(|(block, title)| (!title).then_some(block))
    }

    /// Zooms to `zoom`, a new entry in the history that drops any forward entries.
    fn go_to(&mut self, zoom: Option<ItemId>) {
        if zoom == self.zoom {
            return;
        }
        self.history.truncate(self.history_at + 1);
        self.history.push(zoom);
        self.history_at = self.history.len() - 1;
        self.show(zoom);
    }

    /// Shows the history entry at `at`, if there is one.
    fn walk_history(&mut self, at: Option<usize>) {
        if let Some(at) = at
            && let Some(&zoom) = self.history.get(at)
        {
            self.history_at = at;
            self.show(zoom);
        }
    }

    /// Shows `zoom` as the root and focuses the block zoomed out of when it is visible, else the
    /// first block under the title, else the title.
    fn show(&mut self, zoom: Option<ItemId>) {
        let left = self.zoom;
        self.zoom = zoom;
        let rows = self.rows();
        self.focus = left
            .filter(|&id| rows.iter().any(|row| row.id == id))
            .or_else(|| {
                rows.iter()
                    .find(|row| !row.title)
                    .or(rows.first())
                    .map(|row| row.id)
            });
    }

    fn focus_parent(&mut self) {
        if let Some(parent) = self
            .focused_block()
            .and_then(|block| blocks::parent(self.document(), block.id()))
        {
            self.focus = Some(parent.id());
        }
    }

    /// Focuses the sibling at the index `to` picks from the focused block's index among its
    /// siblings, if there is one.
    fn move_to_sibling(&mut self, to: impl FnOnce(usize) -> Option<usize>) {
        let Some(id) = self.focused_block().map(|block| block.id()) else {
            return;
        };
        let siblings = self
            .document()
            .get(id)
            .and_then(Item::parent)
            .map(|line| blocks::children(self.document(), line))
            .unwrap_or_default();
        if let Some(at) = siblings.iter().position(|block| block.id() == id)
            && let Some(sibling) = to(at).and_then(|index| siblings.get(index))
        {
            self.focus = Some(sibling.id());
        }
    }

    /// On the title, focuses the last block below it and leaves the zoom block's fold alone.
    fn focus_last_child(&mut self) {
        let Some((block, title)) = self.focused() else {
            return;
        };
        if let Some(child) = self.children(&block).last() {
            if !title {
                self.folds.remove(&block.id());
            }
            self.focus = Some(child.id());
        }
    }

    fn collapse(&mut self) {
        let Some(block) = self.focused_block() else {
            return;
        };
        if self.is_expanded(&block) {
            self.folds.insert(block.id());
        } else {
            self.focus_parent();
        }
    }

    fn expand(&mut self) {
        let Some((block, title)) = self.focused() else {
            return;
        };
        if (title || !self.folds.remove(&block.id()))
            && let Some(child) = self.children(&block).first()
        {
            self.focus = Some(child.id());
        }
    }

    fn toggle_fold(&mut self) {
        let Some(block) = self.focused_block() else {
            return;
        };
        if !self.folds.remove(&block.id()) {
            self.fold([block]);
        }
    }

    /// On the title, acts on the blocks below it.
    fn toggle_subtree(&mut self) {
        let Some((block, title)) = self.focused() else {
            return;
        };
        let mut subtree = if title {
            Vec::new()
        } else {
            vec![block.clone()]
        };
        self.push_descendants(&block, &mut subtree);
        if subtree.iter().any(|block| self.is_expanded(block)) {
            self.fold(subtree);
        } else {
            for block in &subtree {
                self.folds.remove(&block.id());
            }
        }
    }

    /// Appends every descendant block of `block`, collapsed or not, to `out`.
    fn push_descendants(&self, block: &Block, out: &mut Vec<Block>) {
        for child in self.children(block) {
            self.push_descendants(&child, out);
            out.push(child);
        }
    }

    fn expand_level(&mut self) {
        let collapsed: Vec<(Block, usize)> = self
            .blocks(false)
            .into_iter()
            .filter(|(block, _)| {
                self.folds.contains(&block.id()) && !self.children(block).is_empty()
            })
            .collect();
        let Some(depth) = collapsed.iter().map(|&(_, depth)| depth).min() else {
            return;
        };
        for (block, _) in collapsed.iter().filter(|&&(_, d)| d == depth) {
            self.folds.remove(&block.id());
        }
    }

    fn collapse_level(&mut self) {
        let expanded: Vec<(Block, usize)> = self
            .blocks(true)
            .into_iter()
            .filter(|(block, _)| self.is_expanded(block))
            .collect();
        let Some(depth) = expanded.iter().map(|&(_, depth)| depth).max() else {
            return;
        };
        let deepest = expanded
            .into_iter()
            .filter(|&(_, d)| d == depth)
            .map(|(block, _)| block);
        self.fold(deepest);
    }

    /// Whether `block` has children and is not collapsed.
    pub(super) fn is_expanded(&self, block: &Block) -> bool {
        !self.folds.contains(&block.id()) && !self.children(block).is_empty()
    }

    /// Collapses each of `blocks` that has children; a leaf has nothing to fold.
    fn fold(&mut self, blocks: impl IntoIterator<Item = Block>) {
        for block in blocks {
            if !self.children(&block).is_empty() {
                self.folds.insert(block.id());
            }
        }
    }

    /// Moves focus off a block a fold hid: to the next visible block in document order, else
    /// the previous one (navigation §2). A hidden block always has a visible ancestor before
    /// it, so one of the two exists.
    fn repair_focus(&mut self) {
        let Some(focus) = self.focus else { return };
        let visible: Vec<_> = self.rows().into_iter().map(|row| row.id).collect();
        if visible.contains(&focus) {
            return;
        }
        let order: Vec<_> = self
            .blocks(false)
            .into_iter()
            .map(|(block, _)| block.id())
            .collect();
        let at = order.iter().position(|&id| id == focus).unwrap_or_default();
        self.focus = order
            .iter()
            .skip(at)
            .chain(order.iter().take(at).rev())
            .find(|id| visible.contains(id))
            .copied();
    }

    /// Focuses the row at the index `to` picks from the focused index and the last index. Without
    /// a focus there are no rows, so there is nothing to move.
    fn move_focus(&mut self, to: impl FnOnce(usize, usize) -> usize) {
        let rows = self.rows();
        let Some(at) = rows.iter().position(|row| Some(row.id) == self.focus) else {
            return;
        };
        if let Some(row) = rows.get(to(at, rows.len() - 1)) {
            self.focus = Some(row.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    use crate::document::{Document, format};

    /// Five rows: A, b, c, d, e.
    fn state() -> State {
        State::new(format::parse("A:\n\t- b\n\t\tc\nd\n- e"), None)
    }

    /// The index of the focused row.
    fn focused(state: &State) -> Option<usize> {
        state
            .rows()
            .iter()
            .position(|row| Some(row.id) == state.focus())
    }

    /// Focuses row `index` by moving down from the first row.
    fn at(index: usize) -> State {
        let mut state = state();
        for _ in 0..index {
            state.update(Message::FocusNext);
        }
        assert_eq!(focused(&state), Some(index));
        state
    }

    #[track_caller]
    fn check(from: usize, message: Message, expected: usize) {
        let mut state = at(from);
        let name = format!("{message:?}");
        assert_eq!(state.update(message), [], "{name} has no effects");
        assert_eq!(focused(&state), Some(expected), "{name} from {from}");
    }

    #[test]
    fn quit_asks_the_frontend_to_quit() {
        assert_eq!(state().update(Message::Quit), [Effect::Quit]);
    }

    // FR-NAV-01
    #[test]
    fn next_and_previous_move_one_row_and_stop_at_the_ends() {
        check(0, Message::FocusNext, 1);
        check(3, Message::FocusNext, 4);
        check(4, Message::FocusNext, 4);
        check(4, Message::FocusPrevious, 3);
        check(1, Message::FocusPrevious, 0);
        check(0, Message::FocusPrevious, 0);
    }

    // FR-NAV-01
    #[test]
    fn first_and_last_jump_to_the_ends() {
        check(2, Message::FocusFirst, 0);
        check(0, Message::FocusFirst, 0);
        check(2, Message::FocusLast, 4);
        check(4, Message::FocusLast, 4);
    }

    // FR-NAV-01
    #[test]
    fn pages_move_by_the_page_height_and_stop_at_the_ends() {
        check(0, Message::PageDown { rows: 2 }, 2);
        check(1, Message::PageDown { rows: 3 }, 4);
        check(3, Message::PageDown { rows: 2 }, 4);
        check(0, Message::PageDown { rows: usize::MAX }, 4);
        check(4, Message::PageUp { rows: 3 }, 1);
        check(2, Message::PageUp { rows: 3 }, 0);
        check(2, Message::PageDown { rows: 0 }, 2);
        check(2, Message::PageUp { rows: 0 }, 2);
    }

    #[test]
    fn a_single_row_keeps_its_focus() {
        let mut state = State::new(format::parse("- only"), None);
        let only = state.focus();
        assert!(only.is_some());
        for message in [
            Message::FocusNext,
            Message::FocusPrevious,
            Message::FocusLast,
            Message::PageDown { rows: 5 },
        ] {
            state.update(message.clone());
            assert_eq!(state.focus(), only, "{message:?}");
        }
    }

    /// Each visible row as its depth, display text and hidden-children count.
    fn visible(state: &State) -> Vec<(usize, String, usize)> {
        state
            .rows()
            .into_iter()
            .map(|row| (row.depth, row.lines.join("|"), row.hidden_children))
            .collect()
    }

    /// The display text of the focused row.
    fn focus_text(state: &State) -> String {
        let rows = state.rows();
        let row = rows.iter().find(|row| Some(row.id) == state.focus());
        row.map(|row| row.lines.join("|")).unwrap_or_default()
    }

    /// `text` with focus moved down `index` rows.
    fn open(text: &str, index: usize) -> State {
        let mut state = State::new(format::parse(text), None);
        for _ in 0..index {
            state.update(Message::FocusNext);
        }
        state
    }

    /// Three levels: A > b > c, A > d, then e.
    const TREE: &str = "A:\n\t- b\n\t\t- c\n\t- d\n- e";

    // FR-NAV-02
    #[test]
    fn collapse_hides_children_and_counts_them() {
        let mut state = open(TREE, 0);
        state.update(Message::Collapse);
        assert_eq!(
            visible(&state),
            [(0, "A".to_owned(), 2), (0, "e".to_owned(), 0)]
        );
        assert_eq!(focus_text(&state), "A");
    }

    // FR-NAV-01, FR-NAV-02
    #[test]
    fn collapse_on_a_leaf_or_collapsed_block_focuses_the_parent() {
        let mut state = open(TREE, 2);
        state.update(Message::Collapse);
        assert_eq!(focus_text(&state), "b");
        state.update(Message::Collapse);
        assert_eq!(focus_text(&state), "b");
        state.update(Message::Collapse);
        assert_eq!(focus_text(&state), "A");
        let mut top = open(TREE, 4);
        top.update(Message::Collapse);
        assert_eq!(focus_text(&top), "e", "a top-level leaf has no parent");
    }

    // FR-NAV-01, FR-NAV-02
    #[test]
    fn expand_opens_then_focuses_the_first_child() {
        let mut state = open(TREE, 0);
        state.update(Message::Collapse);
        state.update(Message::Expand);
        assert_eq!(visible(&state).len(), 5);
        assert_eq!(focus_text(&state), "A");
        state.update(Message::Expand);
        assert_eq!(focus_text(&state), "b");
        let mut leaf = open(TREE, 4);
        leaf.update(Message::Expand);
        assert_eq!(focus_text(&leaf), "e");
    }

    // FR-NAV-02
    #[test]
    fn toggle_fold_flips_a_block_and_ignores_leaves() {
        let mut state = open(TREE, 1);
        state.update(Message::ToggleFold);
        assert_eq!(visible(&state)[1], (1, "b".to_owned(), 1));
        state.update(Message::ToggleFold);
        assert_eq!(visible(&state)[1], (1, "b".to_owned(), 0));
        let mut leaf = open(TREE, 4);
        leaf.update(Message::ToggleFold);
        assert_eq!(leaf.folds, HashSet::new());
    }

    // FR-NAV-02
    #[test]
    fn toggle_subtree_collapses_any_expanded_part_else_expands_all() {
        let mut state = open(TREE, 0);
        state.update(Message::ToggleSubtree);
        assert_eq!(state.folds.len(), 2, "A and b, not the leaves");
        state.update(Message::Expand);
        assert_eq!(visible(&state)[1], (1, "b".to_owned(), 1));
        state.update(Message::ToggleSubtree);
        assert_eq!(state.folds.len(), 2);
        state.update(Message::ToggleSubtree);
        assert_eq!(visible(&state).len(), 5);
        assert_eq!(state.folds, HashSet::new());
    }

    // FR-NAV-02
    #[test]
    fn levels_expand_the_shallowest_and_collapse_the_deepest() {
        let mut state = open(TREE, 0);
        state.update(Message::CollapseLevel);
        assert_eq!(visible(&state)[1], (1, "b".to_owned(), 1));
        assert_eq!(visible(&state)[0], (0, "A".to_owned(), 0));
        state.update(Message::CollapseLevel);
        assert_eq!(visible(&state)[0], (0, "A".to_owned(), 2));
        state.update(Message::CollapseLevel);
        assert_eq!(state.folds.len(), 2, "nothing left to collapse");
        state.update(Message::ExpandLevel);
        assert_eq!(visible(&state)[1], (1, "b".to_owned(), 1));
        state.update(Message::ExpandLevel);
        assert_eq!(visible(&state).len(), 5);
        state.update(Message::ExpandLevel);
        assert_eq!(state.folds, HashSet::new());
    }

    // FR-NAV-02
    #[test]
    fn expand_level_skips_collapsed_leaves() {
        let mut state = open("- a\n- b\n\t- c\n\t\t- d", 0);
        state.folds.insert(state.focus().expect("a focus"));
        state.update(Message::FocusNext);
        state.update(Message::FocusNext);
        state.update(Message::ToggleFold);
        assert_eq!(visible(&state)[2], (1, "c".to_owned(), 1));
        state.update(Message::ExpandLevel);
        assert_eq!(visible(&state)[2], (1, "c".to_owned(), 0));
    }

    // FR-NAV-02
    #[test]
    fn expand_all_and_collapse_all() {
        let mut state = open(TREE, 0);
        state.update(Message::CollapseAll);
        assert_eq!(
            visible(&state),
            [(0, "A".to_owned(), 2), (0, "e".to_owned(), 0)]
        );
        assert_eq!(state.folds.len(), 2);
        state.update(Message::ExpandAll);
        assert_eq!(visible(&state).len(), 5);
    }

    // navigation §2
    #[test]
    fn a_hidden_focus_moves_to_the_next_visible_block_else_the_previous() {
        let mut state = open(TREE, 2);
        state.update(Message::CollapseAll);
        assert_eq!(focus_text(&state), "e");
        let mut last = open("A:\n\t- b\n\t\t- c", 2);
        last.update(Message::CollapseAll);
        assert_eq!(focus_text(&last), "A");
    }

    // FR-NAV-01
    #[test]
    fn parent_and_siblings() {
        let mut state = open(TREE, 2);
        state.update(Message::FocusParent);
        assert_eq!(focus_text(&state), "b");
        state.update(Message::FocusNextSibling);
        assert_eq!(focus_text(&state), "d");
        state.update(Message::FocusNextSibling);
        assert_eq!(focus_text(&state), "d");
        state.update(Message::FocusPreviousSibling);
        assert_eq!(focus_text(&state), "b");
        state.update(Message::FocusPreviousSibling);
        assert_eq!(focus_text(&state), "b");
        state.update(Message::FocusParent);
        state.update(Message::FocusParent);
        assert_eq!(focus_text(&state), "A");
        state.update(Message::FocusNextSibling);
        assert_eq!(focus_text(&state), "e");
    }

    // FR-NAV-01
    #[test]
    fn last_child_expands_a_collapsed_block_and_ignores_leaves() {
        let mut state = open(TREE, 0);
        state.update(Message::Collapse);
        state.update(Message::FocusLastChild);
        assert_eq!(visible(&state).len(), 5);
        assert_eq!(focus_text(&state), "d");
        state.update(Message::FocusLastChild);
        assert_eq!(focus_text(&state), "d", "a leaf has no children");
        let mut zoomed = open(TREE, 0);
        zoomed.update(Message::Collapse);
        zoomed.update(Message::ZoomIn);
        zoomed.update(Message::FocusFirst);
        zoomed.update(Message::FocusLastChild);
        assert_eq!(focus_text(&zoomed), "d", "the title's last child");
        assert_eq!(zoomed.folds.len(), 1, "the title does not unfold");
    }

    // FR-SPC-01
    #[test]
    fn a_block_with_only_special_children_is_a_leaf() {
        let mut state = open("- a\n\tC @hidden\n- b", 0);
        state.update(Message::ToggleSubtree);
        state.update(Message::CollapseAll);
        state.update(Message::Collapse);
        assert_eq!(state.folds, HashSet::new());
        assert_eq!(visible(&state)[0], (0, "a".to_owned(), 0));
    }

    /// The display text of the zoom block, or `Home`.
    fn zoom_text(state: &State) -> String {
        state
            .breadcrumbs()
            .pop()
            .unwrap_or_else(|| "Home".to_owned())
    }

    // FR-NAV-03
    #[test]
    fn zoom_in_shows_a_title_and_focuses_the_first_child() {
        let mut state = open(TREE, 1);
        state.update(Message::ZoomIn);
        assert_eq!(state.zoom(), state.rows().first().map(|row| row.id));
        assert_eq!(state.breadcrumbs(), ["A", "b"]);
        assert_eq!(
            visible(&state),
            [(0, "b".to_owned(), 0), (1, "c".to_owned(), 0)]
        );
        assert_eq!(
            state.rows().iter().map(|row| row.title).collect::<Vec<_>>(),
            [true, false]
        );
        assert_eq!(focus_text(&state), "c");
        state.update(Message::ZoomIn);
        assert_eq!(zoom_text(&state), "c");
        assert_eq!(focus_text(&state), "c", "a leaf focuses its title");
        state.update(Message::ZoomIn);
        assert_eq!(zoom_text(&state), "c", "the title does not zoom");
    }

    // FR-NAV-03
    #[test]
    fn zoom_out_and_home_focus_the_block_left() {
        let mut state = open(TREE, 2);
        state.update(Message::ZoomIn);
        state.update(Message::ZoomOut);
        assert_eq!(zoom_text(&state), "b");
        assert_eq!(focus_text(&state), "c");
        state.update(Message::ZoomOut);
        state.update(Message::ZoomOut);
        assert_eq!(zoom_text(&state), "Home");
        assert_eq!(focus_text(&state), "A");
        state.update(Message::ZoomOut);
        assert_eq!(zoom_text(&state), "Home");
        state.update(Message::FocusNext);
        state.update(Message::ZoomIn);
        state.update(Message::GoHome);
        assert_eq!(focus_text(&state), "b");
    }

    // FR-NAV-03
    #[test]
    fn going_to_a_view_whose_block_left_is_hidden_focuses_its_first_block() {
        let mut state = open(TREE, 1);
        let b = state.focus().expect("a focus");
        state.update(Message::ZoomIn);
        state.update(Message::ZoomIn);
        assert_eq!(zoom_text(&state), "c");
        state.folds.insert(b);
        state.update(Message::GoHome);
        assert_eq!(
            focus_text(&state),
            "A",
            "c is hidden inside the collapsed b"
        );
    }

    // FR-NAV-04
    #[test]
    fn back_and_forward_walk_the_history_and_a_new_zoom_drops_the_rest() {
        let mut state = open(TREE, 1);
        state.update(Message::ZoomIn);
        state.update(Message::ZoomIn);
        state.update(Message::Back);
        assert_eq!(zoom_text(&state), "b");
        state.update(Message::Back);
        assert_eq!(zoom_text(&state), "Home");
        state.update(Message::Back);
        assert_eq!(zoom_text(&state), "Home");
        state.update(Message::Forward);
        state.update(Message::Forward);
        assert_eq!(zoom_text(&state), "c");
        state.update(Message::Forward);
        assert_eq!(zoom_text(&state), "c");
        state.update(Message::Back);
        state.update(Message::GoHome);
        assert_eq!(state.history.len(), 3);
        state.update(Message::Forward);
        assert_eq!(zoom_text(&state), "Home");
        state.update(Message::GoHome);
        assert_eq!(state.history.len(), 3, "staying put is not a new entry");
    }

    // FR-NAV-02, FR-NAV-03
    #[test]
    fn the_title_does_not_fold_or_move_to_its_parent_or_siblings() {
        let mut state = open(TREE, 1);
        state.update(Message::ZoomIn);
        state.update(Message::FocusPrevious);
        assert_eq!(focus_text(&state), "b");
        for message in [
            Message::Collapse,
            Message::ToggleFold,
            Message::FocusParent,
            Message::FocusNextSibling,
            Message::FocusPreviousSibling,
        ] {
            state.update(message.clone());
            assert_eq!(focus_text(&state), "b", "{message:?}");
            assert_eq!(state.folds, HashSet::new(), "{message:?}");
        }
        state.update(Message::Expand);
        assert_eq!(focus_text(&state), "c");
        state.update(Message::FocusParent);
        assert_eq!(focus_text(&state), "b", "the title is the parent");
    }

    // FR-NAV-02, FR-NAV-03
    #[test]
    fn folding_under_a_zoom_stays_under_it() {
        let mut state = open("A:\n\t- b\n\t\t- c\nD:\n\t- e", 0);
        state.update(Message::ZoomIn);
        state.update(Message::CollapseAll);
        assert_eq!(state.folds.len(), 1, "only b");
        state.update(Message::FocusPrevious);
        state.update(Message::ToggleSubtree);
        assert_eq!(
            state.folds,
            HashSet::new(),
            "the title expands what is below it"
        );
        state.update(Message::ToggleSubtree);
        assert_eq!(state.folds.len(), 1);
        state.update(Message::GoHome);
        state.update(Message::FocusLast);
        state.update(Message::Collapse);
        state.update(Message::Collapse);
        state.update(Message::FocusFirst);
        state.update(Message::ZoomIn);
        state.update(Message::ExpandAll);
        assert_eq!(state.folds.len(), 1, "D stays collapsed");
    }

    // FR-NAV-03
    #[test]
    fn a_collapsed_block_shows_its_children_when_zoomed() {
        let mut state = open(TREE, 0);
        state.update(Message::Collapse);
        state.update(Message::ZoomIn);
        assert_eq!(visible(&state)[0], (0, "A".to_owned(), 0));
        assert_eq!(visible(&state).len(), 4);
    }

    #[test]
    fn motions_in_an_empty_document_keep_no_focus() {
        let mut state = State::new(Document::default(), None);
        for message in [
            Message::FocusNext,
            Message::FocusPrevious,
            Message::FocusFirst,
            Message::FocusLast,
            Message::PageDown { rows: 5 },
            Message::PageUp { rows: 5 },
            Message::FocusParent,
            Message::FocusNextSibling,
            Message::FocusPreviousSibling,
            Message::FocusLastChild,
            Message::Collapse,
            Message::Expand,
            Message::ToggleFold,
            Message::ToggleSubtree,
            Message::ExpandLevel,
            Message::CollapseLevel,
            Message::ExpandAll,
            Message::CollapseAll,
            Message::ZoomIn,
            Message::ZoomOut,
            Message::GoHome,
            Message::Back,
            Message::Forward,
        ] {
            assert_eq!(state.update(message.clone()), [], "{message:?}");
            assert_eq!(state.focus(), None, "{message:?}");
        }
    }
}
