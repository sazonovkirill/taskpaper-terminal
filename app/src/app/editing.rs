//! Changing the document: each command commits its line ops as one undoable step, undo and redo
//! walk those steps, and a read-only file refuses them all
//! ([editing.md](../../../docs/specs/editing.md), FR-OPS-08, file-io.md §1).

use jiff::civil::Date;

use super::{Access, History, Mode, Notice, Restore, State};
use crate::document::{
    Block, DateChange, Document, EditError, Flag, ItemId, Op, Transaction,
    format::canonical, metadata,
};

impl State {
    /// Completes the selected blocks, else the focused block, or uncompletes them when all are
    /// done (FR-DONE-01, FR-OPS-07).
    pub(super) fn toggle_done(&mut self, today: Date) {
        let ops = metadata::toggle_done_all(self.document(), &self.targets(), today);
        self.commit_any(ops);
    }

    /// Sets or moves the Date of the selected blocks, else the focused block, as `change` says
    /// (FR-TAG-06, FR-OPS-07, D-140).
    pub(super) fn change_date(&mut self, change: DateChange, today: Date) {
        let ops = self
            .targets()
            .iter()
            .filter_map(|block| metadata::change_date(self.document(), block, change, today))
            .collect();
        self.commit_any(ops);
    }

    /// Removes the Date of the selected blocks, else the focused block (FR-TAG-03, FR-OPS-07,
    /// D-140).
    pub(super) fn clear_date(&mut self) {
        let ops = self
            .targets()
            .iter()
            .filter_map(|block| metadata::set_date(self.document(), block, None))
            .collect();
        self.commit_any(ops);
    }

    /// Adds the bare tag `flag` to the selected blocks, else the focused block, or removes it when
    /// all have it (FR-TAG-04, D-11, FR-OPS-07).
    pub(super) fn toggle_flag(&mut self, flag: Flag) {
        let targets = self.targets();
        let ops = metadata::toggle_flag_all(self.document(), &targets, flag);
        if flag == Flag::Bookmark && !ops.is_empty() {
            self.notify(Notice::Bookmark {
                added: !targets.iter().all(Block::bookmark),
            });
        }
        self.commit_any(ops);
    }

    /// Reverts the latest step and returns to where the view was before it, or in edit mode
    /// takes back the latest change to the text being edited (FR-OPS-08).
    pub(super) fn undo(&mut self) {
        match self.mode() {
            Mode::Navigate => self.step(History::undo),
            Mode::Edit => self.undo_typing(),
        }
    }

    /// Replays the latest undone step and returns to where the view was after it, or in edit
    /// mode the latest undone change to the text being edited (FR-OPS-08).
    pub(super) fn redo(&mut self) {
        match self.mode() {
            Mode::Navigate => self.step(History::redo),
            Mode::Edit => self.redo_typing(),
        }
    }

    /// Applies `ops` as one undoable step that keeps the focus where it is, after what an edit
    /// session has done so far, unless the file is read-only, which only says so.
    pub(super) fn commit(&mut self, ops: Vec<Op>) {
        self.commit_focusing(ops, self.pane.focus);
    }

    /// Applies `ops` as one undoable step, as [`State::commit`] does, that ends with the focus on
    /// `focus`, and says so when they repair a line as [`repair_notice`] tells.
    pub(super) fn commit_focusing(&mut self, ops: Vec<Op>, focus: Option<ItemId>) {
        if self.access() == Access::ReadOnly {
            self.notify(Notice::ReadOnly);
            return;
        }
        self.record_edit_so_far();
        let repaired = repair_notice(&ops);
        let before = Restore {
            focus: self.pane.focus,
        };
        let transaction = Transaction::new(ops, before, Restore { focus });
        match self.edits.commit(&mut self.document, transaction) {
            Ok(()) => {
                self.pane.focus = focus;
                if let Some(notice) = repaired {
                    self.notify(notice);
                }
            }
            Err(error) => self.edit_failed(&error),
        }
    }

    /// Applies `ops` as one undoable step, as [`State::commit`] does, unless there are none: a
    /// command that changes nothing records no step and is not refused.
    pub(super) fn commit_any(&mut self, ops: Vec<Op>) {
        if !ops.is_empty() {
            self.commit(ops);
        }
    }

    /// Takes one step through the history and focuses where that step's view was.
    fn step(
        &mut self,
        step: fn(&mut History, &mut Document) -> Result<Option<Restore>, EditError>,
    ) {
        match step(&mut self.edits, &mut self.document) {
            Ok(Some(restore)) => self.pane.focus = restore.focus,
            Ok(None) => {}
            Err(error) => self.edit_failed(&error),
        }
    }

    /// Reports an edit that did not fit the document, which it left unchanged.
    fn edit_failed(&mut self, error: &EditError) {
        self.notify(Notice::EditFailed {
            reason: error.to_string(),
        });
    }
}

/// The notice for the repair of a line `ops` make that the user is told about
/// (file-format.md §3.1): an empty line given the placeholder, else a line losing the whitespace
/// at its start.
pub(super) fn repair_notice(ops: &[Op]) -> Option<Notice> {
    if canonical::fills_placeholder(ops) {
        Some(Notice::PlaceholderFilled)
    } else if canonical::trims_whitespace(ops) {
        Some(Notice::WhitespaceTrimmed)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, time::Duration};

    use jiff::civil::date;

    use super::*;
    use crate::{
        app::{Disk, DocumentText, Effect, Message, Origin, SelectionChange, Version},
        document::{ItemId, format},
    };

    const TODAY: Date = date(2026, 10, 5);

    fn done() -> Message {
        Message::ToggleDone { today: TODAY }
    }

    /// Each line's text.
    fn texts(state: &State) -> Vec<String> {
        let document = state.document();
        document
            .descendants(document.root())
            .map(|(id, _)| document.get(id).unwrap().text().to_owned())
            .collect()
    }

    /// `text` with focus moved down `index` rows.
    fn open(text: &str, index: usize) -> State {
        let mut state = State::new(format::parse(text), None);
        for _ in 0..index {
            state.update(Message::FocusNext);
        }
        state
    }

    // FR-DONE-01, D-4
    #[test]
    fn toggle_done_completes_and_uncompletes_the_focused_block() {
        let mut state = open("- a\n- b @due(1)", 1);
        assert_eq!(state.update(done()), []);
        assert_eq!(texts(&state), ["- a", "- b @due(1) @done(2026-10-05)"]);
        assert!(state.rows()[1].done);
        state.update(done());
        assert_eq!(texts(&state), ["- a", "- b @due(1)"]);
        assert!(!state.rows()[1].done);
    }

    // FR-DONE-01, FR-NAV-03
    #[test]
    fn toggle_done_works_on_the_zoom_title() {
        let mut state = open("P:\n\t- a", 0);
        state.update(Message::ZoomIn);
        state.update(Message::FocusFirst);
        state.update(done());
        assert_eq!(texts(&state), ["P: @done(2026-10-05)", "- a"]);
    }

    #[test]
    fn toggle_done_in_an_empty_document_does_nothing() {
        let mut state = State::new(Document::default(), None);
        assert_eq!(state.update(done()), []);
        assert_eq!(state.document().revision(), 0);
        assert_eq!(state.update(Message::Undo), []);
        assert_eq!(state.update(Message::Redo), []);
    }

    // FR-TAG-04, D-11, FR-OPS-08
    #[test]
    fn toggle_flag_adds_and_removes_today_and_bookmarks_in_undoable_steps() {
        let mut state = open("- a\n- b", 1);
        assert_eq!(state.update(Message::ToggleFlag(Flag::Star)), []);
        assert!(state.rows()[1].starred);
        assert_eq!(state.notice(), None, "today shows as a chip");
        state.update(Message::ToggleFlag(Flag::Bookmark));
        assert_eq!(texts(&state), ["- a", "- b @star @bookmark"]);
        assert_eq!(state.notice(), Some(&Notice::Bookmark { added: true }));
        state.update(Message::ToggleFlag(Flag::Star));
        assert!(!state.rows()[1].starred);
        state.update(Message::ToggleFlag(Flag::Bookmark));
        assert_eq!(texts(&state), ["- a", "- b"]);
        assert_eq!(state.notice(), Some(&Notice::Bookmark { added: false }));
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a", "- b @bookmark"]);
        let mut empty = State::new(Document::default(), None);
        empty.update(Message::ToggleFlag(Flag::Bookmark));
        assert_eq!(empty.notice(), None);
        assert_eq!(empty.document().revision(), 0);
    }

    // D-11, file-io.md §1
    #[test]
    fn a_read_only_file_refuses_a_bookmark_and_says_so() {
        let mut state = State::new(
            format::parse("- a"),
            Some(PathBuf::from("/notes/todo.taskpaper")),
        )
        .with_access(Access::ReadOnly);
        state.update(Message::ToggleFlag(Flag::Bookmark));
        assert_eq!(texts(&state), ["- a"]);
        assert_eq!(state.notice(), Some(&Notice::ReadOnly));
    }

    fn quick(tag: DateTag, change: DateChange) -> Message {
        Message::ChangeDate {
            tag,
            change,
            today: TODAY,
        }
    }

    // FR-TAG-06, FR-OPS-08
    #[test]
    fn change_date_sets_the_focused_block_and_undoes_in_one_step() {
        let mut state = open("- a\n- b @due(2026-10-01)", 1);
        assert_eq!(state.update(quick(DateTag::Start, DateChange::Today)), []);
        assert_eq!(
            texts(&state),
            ["- a", "- b @due(2026-10-01) @start(2026-10-05)"]
        );
        state.update(quick(DateTag::Due, DateChange::Later));
        assert_eq!(
            texts(&state),
            ["- a", "- b @due(2026-10-02) @start(2026-10-05)"]
        );
        let row = &state.rows()[1];
        assert_eq!((row.start, row.due), (Some(TODAY), Some(date_of(2))));
        state.update(Message::Undo);
        assert_eq!(
            texts(&state),
            ["- a", "- b @due(2026-10-01) @start(2026-10-05)"]
        );
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a", "- b @due(2026-10-01)"]);
    }

    fn date_of(day: i8) -> Date {
        date(2026, 10, day)
    }

    #[test]
    fn change_date_that_changes_nothing_records_no_step() {
        let mut state = open("- a @due(2026-10-06)", 0);
        state.update(quick(DateTag::Due, DateChange::Tomorrow));
        assert_eq!(state.document().revision(), 0);
        state.update(quick(DateTag::Due, DateChange::Today));
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a @due(2026-10-06)"]);
    }

    // FR-TAG-06, FR-NAV-03
    #[test]
    fn change_date_works_on_the_zoom_title_and_not_in_an_empty_document() {
        let mut state = open("P:\n\t- a", 0);
        state.update(Message::ZoomIn);
        state.update(Message::FocusFirst);
        state.update(quick(DateTag::Due, DateChange::Earlier));
        assert_eq!(texts(&state), ["P: @due(2026-10-04)", "- a"]);
        let mut empty = State::new(Document::default(), None);
        assert_eq!(empty.update(quick(DateTag::Start, DateChange::Today)), []);
        assert_eq!(empty.document().revision(), 0);
    }

    // file-io.md §1
    #[test]
    fn a_read_only_file_refuses_dates() {
        let mut state = State::new(
            format::parse("- a"),
            Some(PathBuf::from("/notes/todo.taskpaper")),
        )
        .with_access(Access::ReadOnly);
        state.update(quick(DateTag::Due, DateChange::Today));
        assert_eq!(texts(&state), ["- a"]);
        assert_eq!(state.notice(), Some(&Notice::ReadOnly));
    }

    // FR-OPS-08, N-UX-02
    #[test]
    fn undo_and_redo_walk_the_edits_and_restore_the_focus() {
        let mut state = open("- a\n- b", 0);
        state.update(done());
        state.update(Message::FocusNext);
        state.update(done());
        state.update(Message::FocusFirst);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a @done(2026-10-05)", "- b"]);
        assert_eq!(state.focus(), Some(state.rows()[1].id), "back on b");
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a", "- b"]);
        assert_eq!(state.focus(), Some(state.rows()[0].id));
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a", "- b"], "nothing more to undo");
        state.update(Message::Redo);
        state.update(Message::Redo);
        assert_eq!(
            texts(&state),
            ["- a @done(2026-10-05)", "- b @done(2026-10-05)"]
        );
        assert_eq!(state.focus(), Some(state.rows()[1].id));
        let revision = state.document().revision();
        state.update(Message::Redo);
        assert_eq!(
            state.document().revision(),
            revision,
            "nothing more to redo"
        );
    }

    // FR-OPS-08
    #[test]
    fn a_new_edit_drops_the_undone_ones() {
        let mut state = open("- a\n- b", 0);
        state.update(done());
        state.update(Message::Undo);
        state.update(Message::FocusNext);
        state.update(done());
        state.update(Message::Redo);
        assert_eq!(texts(&state), ["- a", "- b @done(2026-10-05)"]);
    }

    // FR-DOC-02
    #[test]
    fn edits_undo_and_redo_are_saved() {
        let path = PathBuf::from("/notes/todo.taskpaper");
        let mut state = State::new(format::parse("- a\n"), Some(path.clone()))
            .with_version(Some(Version::of(b"- a\n")));
        let save = |text: &str, on_disk: &str| {
            vec![
                Effect::Disk(Disk::Backup { path: path.clone() }),
                Effect::Disk(Disk::Save {
                    path: path.clone(),
                    text: DocumentText::of(&format::parse(text)),
                    expected: Some(Version::of(on_disk.as_bytes())),
                }),
            ]
        };
        state.update(done());
        let later = |state: &mut State, seconds| {
            state.update(Message::Tick {
                now: Duration::from_secs(seconds),
                today: jiff::civil::Date::default(),
            })
        };
        assert_eq!(
            later(&mut state, 1),
            save("- a @done(2026-10-05)\n", "- a\n")
        );
        state.update(Message::Saved {
            version: Version::of(b"- a @done(2026-10-05)\n"),
        });
        state.update(Message::Undo);
        assert_eq!(
            later(&mut state, 2),
            save("- a\n", "- a @done(2026-10-05)\n")
        );
        state.update(Message::Saved {
            version: Version::of(b"- a\n"),
        });
        state.update(Message::Redo);
        assert_eq!(
            later(&mut state, 3),
            save("- a @done(2026-10-05)\n", "- a\n")
        );
    }

    // file-io.md §1
    #[test]
    fn a_read_only_file_refuses_edits_with_a_notice() {
        let mut state = State::new(
            format::parse("- a"),
            Some(PathBuf::from("/notes/todo.taskpaper")),
        )
        .with_access(Access::ReadOnly);
        assert_eq!(state.update(done()), []);
        assert_eq!(texts(&state), ["- a"]);
        assert_eq!(state.notice(), Some(&Notice::ReadOnly));
        assert_eq!(state.document().revision(), 0);
    }

    /// The text of each block the latest update changed.
    fn changed(state: &State) -> Vec<String> {
        state
            .changed()
            .iter()
            .map(|&id| state.document().get(id).unwrap().text().to_owned())
            .collect()
    }

    // FR-UI-11
    #[test]
    fn each_update_reports_the_blocks_it_changed() {
        let mut state = open("A:\n\t- b\n\t\tc\n- d\n- e", 1);
        state.update(done());
        assert_eq!(changed(&state), ["- b @done(2026-10-05)"]);
        state.update(Message::FocusNext);
        assert_eq!(
            changed(&state),
            Vec::<String>::new(),
            "a motion changes nothing"
        );
        state.update(Message::FocusPrevious);
        state.update(Message::Outdent);
        assert_eq!(
            changed(&state),
            ["- b @done(2026-10-05)", "c"],
            "a block moves with the blocks inside it"
        );
        state.update(Message::Undo);
        assert_eq!(changed(&state), ["- b @done(2026-10-05)", "c"]);
        state.update(Message::Delete);
        assert_eq!(
            changed(&state),
            Vec::<String>::new(),
            "deleted blocks are no longer there"
        );
        state.update(Message::Undo);
        assert_eq!(changed(&state), ["- b @done(2026-10-05)", "c"]);
        assert!(!state.refused());
    }

    // FR-UI-11, file-io.md §1
    #[test]
    fn an_update_refused_says_so_until_the_next_one() {
        let mut state = State::new(
            format::parse("- a\n- b"),
            Some(PathBuf::from("/notes/todo.taskpaper")),
        )
        .with_access(Access::ReadOnly);
        state.update(done());
        assert!(state.refused());
        assert_eq!(state.changed(), []);
        state.update(Message::FocusNext);
        assert!(!state.refused(), "the next update is not refused");
        assert_eq!(
            state.notice(),
            Some(&Notice::ReadOnly),
            "while the notice stays"
        );
        state.update(Message::ToggleFlag(Flag::Bookmark));
        assert!(state.refused());
        state.update(Message::GoTo(crate::app::Location::View(
            crate::app::SpecialView::Today,
        )));
        state.update(Message::Pasted {
            text: "- x".to_owned(),
            at: crate::app::Paste::Below,
        });
        assert!(
            !state.refused(),
            "a message the view does not take resets it too"
        );
    }

    // FR-UI-11
    #[test]
    fn a_command_that_changes_nothing_is_not_refused() {
        let mut state = open("- a", 0);
        state.update(Message::Undo);
        assert!(!state.refused());
        state.update(Message::ClearDate(DateTag::Due));
        assert!(!state.refused());
        assert_eq!(state.changed(), []);
        state.update(Message::Count);
        assert!(state.notice().is_some());
        assert!(!state.refused(), "a notice that informs refuses nothing");
    }

    #[test]
    fn a_temporary_file_takes_edits() {
        let mut state = State::new(
            format::parse("- a"),
            Some(PathBuf::from("/tmp/demo.taskpaper")),
        )
        .with_origin(Origin::Temporary);
        state.update(done());
        assert_eq!(texts(&state), ["- a @done(2026-10-05)"]);
    }

    #[test]
    fn a_step_that_no_longer_fits_is_reported_and_kept() {
        let mut state = open("- a", 0);
        state.update(done());
        let id = state.focus().unwrap();
        let op = Op::SetText {
            id,
            before: "- a @done(2026-10-05)".to_owned(),
            after: "- x".to_owned(),
        };
        state
            .document
            .apply(&mut Transaction::new(vec![op], (), ()))
            .unwrap();
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- x"]);
        assert!(matches!(state.notice(), Some(Notice::EditFailed { .. })));
        state
            .document
            .apply(&mut Transaction::new(vec![op_back(id)], (), ()))
            .unwrap();
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a"], "the step was kept");
    }

    fn op_back(id: ItemId) -> Op {
        Op::SetText {
            id,
            before: "- x".to_owned(),
            after: "- a @done(2026-10-05)".to_owned(),
        }
    }

    #[test]
    fn a_command_whose_ops_do_not_fit_is_reported_and_not_recorded() {
        let mut state = open("- a", 0);
        let id = state.focus().unwrap();
        state.commit(vec![op_back(id)]);
        assert_eq!(texts(&state), ["- a"]);
        assert!(matches!(state.notice(), Some(Notice::EditFailed { .. })));
        state.update(Message::Undo);
        assert_eq!(state.document().revision(), 0, "nothing to undo");
    }

    /// `text` with the rows at `rows` selected with `v`, the focus left on the last of them.
    fn picking(text: &str, rows: &[usize]) -> State {
        let mut state = open(text, 0);
        for &row in rows {
            state.update(Message::FocusFirst);
            for _ in 0..row {
                state.update(Message::FocusNext);
            }
            state.update(Message::Select(SelectionChange::Toggle));
        }
        state
    }

    // FR-OPS-07, FR-DONE-01, FR-OPS-08
    #[test]
    fn toggle_done_completes_every_selected_block_in_one_step() {
        let mut state = picking("- a\n- b @done(1)\n- c\n- d", &[0, 1, 2]);
        state.update(done());
        assert_eq!(
            texts(&state),
            [
                "- a @done(2026-10-05)",
                "- b @done(1)",
                "- c @done(2026-10-05)",
                "- d"
            ]
        );
        state.update(done());
        assert_eq!(texts(&state), ["- a", "- b", "- c", "- d"], "all were done");
        state.update(Message::Undo);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a", "- b @done(1)", "- c", "- d"]);
    }

    // FR-OPS-07, FR-TAG-06
    #[test]
    fn change_date_moves_each_selected_blocks_own_date() {
        let mut state = picking("- a @due(2026-10-01)\n- b\n- c", &[0, 1]);
        state.update(quick(DateTag::Due, DateChange::Later));
        assert_eq!(
            texts(&state),
            ["- a @due(2026-10-02)", "- b @due(2026-10-06)", "- c"]
        );
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a @due(2026-10-01)", "- b", "- c"]);
    }

    // FR-OPS-07, FR-TAG-04, D-11
    #[test]
    fn toggle_flag_adds_to_every_selected_block_unless_all_have_it() {
        let mut state = picking("- a @bookmark\n- b\n- c", &[0, 1]);
        state.update(Message::ToggleFlag(Flag::Bookmark));
        assert_eq!(texts(&state), ["- a @bookmark", "- b @bookmark", "- c"]);
        assert_eq!(state.notice(), Some(&Notice::Bookmark { added: true }));
        state.update(Message::ToggleFlag(Flag::Bookmark));
        assert_eq!(texts(&state), ["- a", "- b", "- c"]);
        assert_eq!(state.notice(), Some(&Notice::Bookmark { added: false }));
        state.update(Message::ToggleFlag(Flag::Star));
        assert_eq!(texts(&state), ["- a @star", "- b @star", "- c"]);
    }

    // FR-TAG-03, D-50, FR-OPS-07
    #[test]
    fn clear_date_removes_the_date_of_the_targets_as_one_step() {
        let mut state = open(
            "- a @start(2026-10-01) @due(2026-10-09)\n- b @due(1)\n- c",
            0,
        );
        state.update(Message::Select(SelectionChange::ExtendDown));
        state.update(Message::Select(SelectionChange::ExtendDown));
        state.update(Message::ClearDate(DateTag::Due));
        assert_eq!(texts(&state), ["- a @start(2026-10-01)", "- b", "- c"]);
        state.update(Message::ClearDate(DateTag::Start));
        assert_eq!(texts(&state), ["- a", "- b", "- c"]);
        state.update(Message::ClearDate(DateTag::Start));
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a @start(2026-10-01)", "- b", "- c"]);
    }
}
