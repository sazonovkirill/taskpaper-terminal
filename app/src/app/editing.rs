//! Changing the document: each command commits its line ops as one undoable step, undo and redo
//! walk those steps, and a read-only file refuses them all
//! ([editing.md](../../../docs/specs/editing.md), FR-OPS-08, file-io.md §1).

use jiff::civil::Date;

use super::{Access, History, Notice, Restore, State};
use crate::document::{Document, EditError, Op, Transaction, metadata};

impl State {
    /// Completes the focused block, or uncompletes it when it is done (FR-DONE-01).
    pub(super) fn toggle_done(&mut self, today: Date) {
        if let Some((block, _)) = self.focused() {
            let ops = metadata::toggle_done(self.document(), &block, today);
            self.commit(ops);
        }
    }

    /// Reverts the latest step and returns to where the view was before it (FR-OPS-08).
    pub(super) fn undo(&mut self) {
        self.step(History::undo);
    }

    /// Replays the latest undone step and returns to where the view was after it (FR-OPS-08).
    pub(super) fn redo(&mut self) {
        self.step(History::redo);
    }

    /// Applies `ops` as one undoable step that keeps the focus where it is, after the creation of
    /// a block being edited, unless the file is read-only, which only says so.
    pub(super) fn commit(&mut self, ops: Vec<Op>) {
        if self.access() == Access::ReadOnly {
            self.notify(Notice::ReadOnly);
            return;
        }
        self.record_creation();
        let restore = Restore { focus: self.focus };
        let transaction = Transaction::new(ops, restore, restore);
        if let Err(error) = self.edits.commit(&mut self.document, transaction) {
            self.edit_failed(&error);
        }
    }

    /// Takes one step through the history and focuses where that step's view was.
    fn step(
        &mut self,
        step: fn(&mut History, &mut Document) -> Result<Option<Restore>, EditError>,
    ) {
        match step(&mut self.edits, &mut self.document) {
            Ok(Some(restore)) => self.focus = restore.focus,
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

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, time::Duration};

    use jiff::civil::date;

    use super::*;
    use crate::{
        app::{Disk, Effect, Message, Origin, Version},
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
                    text: text.to_owned(),
                    expected: Some(Version::of(on_disk.as_bytes())),
                }),
            ]
        };
        state.update(done());
        let later = |state: &mut State, seconds| {
            state.update(Message::Tick {
                now: Duration::from_secs(seconds),
            })
        };
        assert_eq!(
            later(&mut state, 1),
            save("- a @done(2026-10-05)\n", "- a\n")
        );
        state.update(Message::Saved);
        state.update(Message::Undo);
        assert_eq!(
            later(&mut state, 2),
            save("- a\n", "- a @done(2026-10-05)\n")
        );
        state.update(Message::Saved);
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
}
