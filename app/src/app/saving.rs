//! When the open file is written and backed up: a save a moment after the last change, one save
//! at a time, retries after a failure, the saves that end a session and the backup before each
//! save ([file-io.md §2](../../../docs/specs/file-io.md#2-saving)). The core only decides; the
//! frontend carries out the [`Disk`] effects and reports back.

use std::{path::PathBuf, time::Duration};

use super::{Access, Effect, Notice, Origin, State, update::Disk};
use crate::document::format;

/// How long after the last change a save starts (file-io.md §2, `autosave_delay_ms`).
pub const DELAY: Duration = Duration::from_secs(1);

/// What the status line says about saving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveState<'a> {
    /// The file on disk holds every change.
    Saved,
    /// Changes are waiting for their save or being written.
    Unsaved,
    /// The last save failed, for this reason; the next change and quitting try again.
    Failed {
        /// What the frontend reported.
        reason: &'a str,
    },
}

/// How the session ends, from the gentlest to the most final.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Ending {
    /// The user quits: save first, and ask before ending if that fails.
    Quit,
    /// The process was told to end: save first, and end even if that fails.
    Terminate,
    /// End without saving.
    Discard,
}

/// A save that failed.
#[derive(Debug, Clone)]
struct Failure {
    revision: u64,
    reason: String,
}

/// The saving policy's memory of the document's revisions ([`crate::document::Document::revision`]).
#[derive(Debug, Clone)]
pub(super) struct Autosave {
    /// The revision the file on disk holds, or `None` when it holds none of them, as after the
    /// local version was kept in a conflict and must overwrite the file.
    saved: Option<u64>,
    /// The latest revision seen, and when it was first seen.
    seen: u64,
    changed_at: Duration,
    /// The revision being written; at most one save runs at a time.
    writing: Option<u64>,
    failure: Option<Failure>,
    ending: Option<Ending>,
    /// Whether a quit's save failed, so quitting again ends without saving.
    confirm_quit: bool,
    /// Whether a backup failure was reported; it is reported once per session.
    backup_warned: bool,
    /// Whether the next save starts without waiting for the delay.
    urgent: bool,
}

impl Autosave {
    /// The policy for a document opened at `revision`, which the file holds.
    pub(super) fn new(revision: u64) -> Self {
        Self {
            saved: Some(revision),
            ..Self::unsaved(revision)
        }
    }

    /// The policy for a document at `revision` that the file does not hold: it is due for a
    /// save from the start.
    pub(super) fn unsaved(revision: u64) -> Self {
        Self {
            saved: None,
            seen: revision,
            changed_at: Duration::ZERO,
            writing: None,
            failure: None,
            ending: None,
            confirm_quit: false,
            backup_warned: false,
            urgent: false,
        }
    }

    /// Notes `revision` at `now`: a new one restarts the delay and withdraws the quit question.
    fn observe(&mut self, revision: u64, now: Duration) {
        if revision != self.seen {
            self.seen = revision;
            self.changed_at = now;
            self.confirm_quit = false;
        }
    }

    /// Whether a save of `revision` starts at `now`. Ending the session saves at once and retries
    /// a failed revision once; otherwise a failed revision waits for the next change.
    fn due(&self, revision: u64, now: Duration) -> bool {
        self.writing.is_none()
            && Some(revision) != self.saved
            && match self.ending {
                Some(Ending::Quit | Ending::Terminate) => true,
                Some(Ending::Discard) => false,
                None => {
                    self.urgent
                        || !self.failed_at(revision) && now >= self.changed_at.saturating_add(DELAY)
                }
            }
    }

    fn failed_at(&self, revision: u64) -> bool {
        self.failure
            .as_ref()
            .is_some_and(|failure| failure.revision == revision)
    }

    /// When a save is next due without anything else happening, if one is.
    fn deadline(&self) -> Option<Duration> {
        (self.ending.is_none()
            && self.writing.is_none()
            && Some(self.seen) != self.saved
            && !self.failed_at(self.seen))
        .then(|| self.changed_at.saturating_add(DELAY))
    }

    /// Whether the session can end now: the file holds every change, or it is ended without them.
    fn finished(&self, revision: u64, saveable: bool) -> bool {
        match self.ending {
            None => false,
            Some(Ending::Discard) => true,
            // A save under way is of a revision newer than the saved one, so this waits for it.
            Some(Ending::Quit | Ending::Terminate) => !saveable || Some(revision) == self.saved,
        }
    }

    /// Asks to end the session; a quit after a quit's failed save ends without saving, and a
    /// more final ending is never softened.
    pub(super) fn end(&mut self, ending: Ending) {
        let ending = if ending == Ending::Quit && self.confirm_quit {
            Ending::Discard
        } else {
            ending
        };
        self.ending = self.ending.max(Some(ending));
    }

    /// How the session was asked to end, if it was.
    pub(super) fn ending(&self) -> Option<Ending> {
        self.ending
    }

    /// A save of `revision` starts.
    fn start(&mut self, revision: u64) {
        self.writing = Some(revision);
        self.urgent = false;
    }

    /// The save under way succeeded.
    fn saved(&mut self) {
        if let Some(revision) = self.writing.take() {
            self.saved = Some(revision);
            self.failure = None;
            self.confirm_quit = false;
        }
    }

    /// The save under way failed for `reason`. A quit waiting for it is withdrawn and asks
    /// instead; a termination ends anyway.
    fn failed(&mut self, reason: String) {
        let Some(revision) = self.writing.take() else {
            return;
        };
        self.failure = Some(Failure { revision, reason });
        self.ending_failed();
    }

    /// What ending the session needed failed: a quit is withdrawn and asks instead, and a
    /// termination ends anyway.
    pub(super) fn ending_failed(&mut self) {
        match self.ending {
            Some(Ending::Quit) => {
                self.ending = None;
                self.confirm_quit = true;
            }
            Some(Ending::Terminate) => self.ending = Some(Ending::Discard),
            Some(Ending::Discard) | None => {}
        }
    }

    /// The save under way was not done because the file changed on disk; it is not a failure.
    fn refused(&mut self) {
        self.writing = None;
    }

    /// The local version must overwrite the file on disk, at once, even if unchanged.
    pub(super) fn overwrite(&mut self) {
        self.saved = None;
        self.urgent = true;
    }

    /// The document was replaced by the file on disk, now at `revision`: nothing is unsaved, and
    /// a failure or quit question about the replaced version no longer applies.
    pub(super) fn reloaded(&mut self, revision: u64) {
        self.saved = Some(revision);
        self.seen = revision;
        self.failure = None;
        self.confirm_quit = false;
        self.urgent = false;
    }
}

impl State {
    /// The file to save to, unless there is none or it opened read-only.
    fn saveable_path(&self) -> Option<PathBuf> {
        match self.access() {
            Access::ReadWrite => self.path().map(PathBuf::from),
            Access::ReadOnly => None,
        }
    }

    /// A backup of the opened file (FR-DOC-07).
    pub(super) fn backup_on_open(&self) -> Vec<Effect> {
        self.backup().into_iter().collect()
    }

    /// Making sure today's backup of the file exists, unless there is no file or it is the
    /// temporary one, which is not backed up (file-io.md §2).
    fn backup(&self) -> Option<Effect> {
        match self.origin() {
            Origin::Chosen => self
                .path()
                .map(|path| Effect::Disk(Disk::Backup { path: path.into() })),
            Origin::Temporary => None,
        }
    }

    /// Notes a change since the last message, then starts a save that is due, after making sure
    /// today's backup exists, or ends a session whose saves are done (FR-DOC-02, F-FILE-05).
    /// Nothing is saved while the conflict prompt is open, and ending the session then leaves
    /// the conflict (file-io.md §3).
    pub(super) fn save_if_due(&mut self) -> Vec<Effect> {
        let revision = self.document().revision();
        self.autosave.observe(revision, self.now);
        if self.external.in_conflict() {
            return self.leave_conflict();
        }
        let path = self.saveable_path();
        if let Some(path) = &path
            && self.autosave.due(revision, self.now)
            && !self.editing_holds_save()
        {
            self.autosave.start(revision);
            let text = format::serialize(self.document());
            let expected = self.external.start_save(&text);
            let save = Effect::Disk(Disk::Save {
                path: path.clone(),
                text,
                expected,
            });
            return self.backup().into_iter().chain([save]).collect();
        }
        if self.autosave.finished(revision, path.is_some()) {
            return vec![Effect::Quit];
        }
        Vec::new()
    }

    /// The save under way succeeded.
    pub(super) fn saved(&mut self) {
        self.autosave.saved();
        self.external.save_ended(true);
    }

    /// The save under way failed for `reason`.
    pub(super) fn save_failed(&mut self, reason: String) {
        self.autosave.failed(reason);
        self.external.save_ended(false);
    }

    /// The save under way was not done because the file on disk changed to `contents`.
    pub(super) fn save_refused(&mut self, contents: Option<Vec<u8>>) {
        self.autosave.refused();
        self.external.save_ended(false);
        self.found(contents);
    }

    /// Shows a backup failure, the first time one happens in the session.
    pub(super) fn backup_failed(&mut self, reason: String) {
        if !self.autosave.backup_warned {
            self.autosave.backup_warned = true;
            self.notify(Notice::BackupFailed { reason });
        }
    }

    /// When a save is next due without any message but the clock's, if one is.
    pub(super) fn save_deadline(&self) -> Option<Duration> {
        self.saveable_path()
            .filter(|_| !self.external.in_conflict())
            .and(self.autosave.deadline())
            .filter(|_| !self.editing_holds_save())
    }

    /// Whether the file holds every change, and whether the last save failed.
    #[must_use]
    pub fn save_state(&self) -> SaveState<'_> {
        match &self.autosave.failure {
            Some(failure) => SaveState::Failed {
                reason: &failure.reason,
            },
            None if Some(self.document().revision()) != self.autosave.saved => SaveState::Unsaved,
            None => SaveState::Saved,
        }
    }

    /// The serialized document while the file lacks some of it: a save is due or under way, or
    /// the last one failed. A crash must not lose it (N-REL-01).
    #[must_use]
    pub fn unsaved_text(&self) -> Option<String> {
        (self.save_state() != SaveState::Saved).then(|| format::serialize(self.document()))
    }

    /// Whether quitting was refused because its save failed, so quitting again ends without
    /// saving (file-io.md §2).
    #[must_use]
    pub fn confirming_quit(&self) -> bool {
        self.autosave.confirm_quit
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::{
        app::{Message, Version, state::NOTICE_TIME},
        document::{Op, Transaction},
    };

    const FILE: &str = "/notes/todo.taskpaper";

    fn ms(millis: u64) -> Duration {
        Duration::from_millis(millis)
    }

    /// A file holding one line, `a`, opened at time 0.
    fn opened() -> State {
        let mut state = State::new(format::parse("a\n"), Some(PathBuf::from(FILE)));
        state.update(Message::Tick { now: ms(0) });
        state
    }

    /// Sets the text of the document's only line to `text` at time `now`, as a command would:
    /// the frontend's tick, then the change, then the update that follows every message.
    fn edit(state: &mut State, text: &str, now: Duration) -> Vec<Effect> {
        state.update(Message::Tick { now });
        let document = &mut state.document;
        let id = document.descendants(document.root()).next().unwrap().0;
        let before = document.get(id).unwrap().text().to_owned();
        let op = Op::SetText {
            id,
            before,
            after: text.to_owned(),
        };
        document
            .apply(&mut Transaction::new(vec![op], (), ()))
            .unwrap();
        state.update(Message::Tick { now })
    }

    fn tick(state: &mut State, now: Duration) -> Vec<Effect> {
        state.update(Message::Tick { now })
    }

    /// The effects of a save of `text` while there is no file yet.
    fn save(text: &str) -> Vec<Effect> {
        save_over(text, None)
    }

    /// The effects of a save of `text` over the file holding `expected`.
    fn save_over(text: &str, expected: Option<&str>) -> Vec<Effect> {
        let path = PathBuf::from(FILE);
        vec![
            Effect::Disk(Disk::Backup { path: path.clone() }),
            Effect::Disk(Disk::Save {
                path,
                text: text.to_owned(),
                expected: expected.map(|text| Version::of(text.as_bytes())),
            }),
        ]
    }

    fn failed(reason: &str) -> Message {
        Message::SaveFailed {
            reason: reason.to_owned(),
        }
    }

    // FR-DOC-02, F-FILE-05
    #[test]
    fn saves_a_second_after_the_last_change() {
        let mut state = opened();
        assert_eq!(state.save_state(), SaveState::Saved);
        assert_eq!(state.deadline(), None);
        assert_eq!(edit(&mut state, "b", ms(100)), []);
        assert_eq!(state.save_state(), SaveState::Unsaved);
        assert_eq!(state.deadline(), Some(ms(1100)));
        assert_eq!(
            edit(&mut state, "c", ms(600)),
            [],
            "a change restarts the delay"
        );
        assert_eq!(state.deadline(), Some(ms(1600)));
        assert_eq!(tick(&mut state, ms(1599)), []);
        assert_eq!(tick(&mut state, ms(1600)), save("c\n"));
        assert_eq!(state.save_state(), SaveState::Unsaved);
        assert_eq!(state.deadline(), None, "nothing is due while the save runs");
        assert_eq!(state.update(Message::Saved), []);
        assert_eq!(state.save_state(), SaveState::Saved);
        assert_eq!(tick(&mut state, ms(5000)), []);
    }

    // N-UX-01
    #[test]
    fn runs_one_save_at_a_time_and_saves_what_changed_meanwhile() {
        let mut state = opened();
        edit(&mut state, "b", ms(0));
        assert_eq!(tick(&mut state, ms(1000)), save("b\n"));
        edit(&mut state, "c", ms(1200));
        assert_eq!(
            tick(&mut state, ms(2200)),
            [],
            "the first save is still running"
        );
        assert_eq!(state.update(Message::Saved), save_over("c\n", Some("b\n")));
        assert_eq!(state.update(Message::Saved), []);
        assert_eq!(state.save_state(), SaveState::Saved);
    }

    #[test]
    fn a_save_that_brings_back_the_saved_text_still_saves() {
        let mut state = opened();
        edit(&mut state, "b", ms(0));
        edit(&mut state, "a", ms(0));
        assert_eq!(state.save_state(), SaveState::Unsaved);
        assert_eq!(tick(&mut state, ms(1000)), save("a\n"));
    }

    // FR-DOC-02
    #[test]
    fn a_failed_save_waits_for_the_next_change() {
        let mut state = opened();
        edit(&mut state, "b", ms(0));
        tick(&mut state, ms(1000));
        assert_eq!(state.update(failed("disk full")), []);
        assert_eq!(
            state.save_state(),
            SaveState::Failed {
                reason: "disk full"
            }
        );
        assert_eq!(state.deadline(), None);
        assert_eq!(tick(&mut state, ms(9000)), []);
        edit(&mut state, "c", ms(10_000));
        assert_eq!(tick(&mut state, ms(11_000)), save("c\n"));
        assert!(matches!(state.save_state(), SaveState::Failed { .. }));
        state.update(Message::Saved);
        assert_eq!(state.save_state(), SaveState::Saved);
    }

    #[test]
    fn results_without_a_save_under_way_change_nothing() {
        let mut state = opened();
        edit(&mut state, "b", ms(0));
        assert_eq!(state.update(Message::Saved), []);
        assert_eq!(state.update(failed("stray")), []);
        assert_eq!(state.save_state(), SaveState::Unsaved);
        assert_eq!(tick(&mut state, ms(1000)), save("b\n"));
    }

    // F-FILE-05
    #[test]
    fn quitting_saves_first_and_then_quits() {
        let mut state = opened();
        assert_eq!(state.update(Message::Quit), [Effect::Quit]);
        let mut state = opened();
        edit(&mut state, "b", ms(0));
        assert_eq!(
            state.update(Message::Quit),
            save("b\n"),
            "without the delay"
        );
        assert_eq!(state.update(Message::Quit), [], "the save is running");
        assert_eq!(state.update(Message::Saved), [Effect::Quit]);
    }

    #[test]
    fn quitting_while_an_autosave_runs_waits_for_it() {
        let mut state = opened();
        edit(&mut state, "b", ms(0));
        tick(&mut state, ms(1000));
        assert_eq!(state.update(Message::Quit), []);
        assert_eq!(state.update(Message::Saved), [Effect::Quit]);
    }

    // FR-DOC-02
    #[test]
    fn a_failed_save_on_quit_asks_before_quitting_without_saving() {
        let mut state = opened();
        edit(&mut state, "b", ms(0));
        tick(&mut state, ms(1000));
        state.update(failed("disk full"));
        assert_eq!(
            state.update(Message::Quit),
            save("b\n"),
            "quitting retries the failed save"
        );
        assert_eq!(state.update(failed("disk full")), []);
        assert!(state.confirming_quit());
        assert_eq!(tick(&mut state, ms(5000)), []);
        assert_eq!(state.update(Message::Quit), [Effect::Quit]);
    }

    #[test]
    fn a_change_or_a_save_withdraws_the_quit_question() {
        let mut state = opened();
        edit(&mut state, "b", ms(0));
        state.update(Message::Quit);
        state.update(failed("disk full"));
        edit(&mut state, "c", ms(100));
        assert!(!state.confirming_quit());
        assert_eq!(state.update(Message::Quit), save("c\n"));
        state.update(failed("disk full"));
        assert!(state.confirming_quit());
        edit(&mut state, "d", ms(200));
        assert_eq!(tick(&mut state, ms(1200)), save("d\n"));
        state.update(Message::Saved);
        assert!(!state.confirming_quit());
        assert_eq!(state.update(Message::Quit), [Effect::Quit]);
    }

    // F-FILE-05
    #[test]
    fn terminating_saves_and_ends_even_when_the_save_fails() {
        let mut state = opened();
        assert_eq!(state.update(Message::Terminate), [Effect::Quit]);
        let mut state = opened();
        edit(&mut state, "b", ms(0));
        assert_eq!(state.update(Message::Terminate), save("b\n"));
        assert_eq!(state.update(Message::Saved), [Effect::Quit]);
        let mut state = opened();
        edit(&mut state, "b", ms(0));
        state.update(Message::Terminate);
        assert_eq!(
            state.update(Message::Quit),
            [],
            "quitting does not soften the end"
        );
        assert_eq!(state.update(failed("disk full")), [Effect::Quit]);
    }

    #[test]
    fn terminating_after_the_quit_question_saves_once_more() {
        let mut state = opened();
        edit(&mut state, "b", ms(0));
        state.update(Message::Quit);
        state.update(failed("disk full"));
        assert_eq!(state.update(Message::Terminate), save("b\n"));
        assert_eq!(state.update(failed("disk full")), [Effect::Quit]);
    }

    // file-io.md §1
    #[test]
    fn a_read_only_or_unnamed_document_is_never_saved() {
        for mut state in [
            opened().with_access(Access::ReadOnly),
            State::new(format::parse("a\n"), None),
        ] {
            edit(&mut state, "b", ms(0));
            assert_eq!(state.save_state(), SaveState::Unsaved);
            assert_eq!(state.deadline(), None);
            assert_eq!(tick(&mut state, ms(5000)), []);
            assert_eq!(state.update(Message::Quit), [Effect::Quit]);
        }
    }

    // N-REL-01
    #[test]
    fn the_text_to_recover_is_the_document_while_the_file_lacks_changes() {
        let mut state = opened();
        assert_eq!(state.unsaved_text(), None);
        edit(&mut state, "b", ms(0));
        assert_eq!(state.unsaved_text().as_deref(), Some("b\n"));
        tick(&mut state, ms(1000));
        assert_eq!(
            state.unsaved_text().as_deref(),
            Some("b\n"),
            "the save may never finish"
        );
        state.update(failed("disk full"));
        assert_eq!(state.unsaved_text().as_deref(), Some("b\n"));
        edit(&mut state, "c", ms(2000));
        tick(&mut state, ms(3000));
        state.update(Message::Saved);
        assert_eq!(state.unsaved_text(), None);
        let mut unnamed = State::new(format::parse("a\n"), None);
        edit(&mut unnamed, "d", ms(0));
        assert_eq!(unnamed.unsaved_text().as_deref(), Some("d\n"));
    }

    // N-REL-01, file-io.md §4
    #[test]
    fn a_recovered_document_is_saved_like_a_change() {
        let recovered = || State::recovered(format::parse("b\n"), Some(PathBuf::from(FILE)));
        let mut state = recovered();
        assert_eq!(state.save_state(), SaveState::Unsaved);
        assert_eq!(state.unsaved_text().as_deref(), Some("b\n"));
        assert_eq!(state.deadline(), Some(DELAY));
        assert_eq!(tick(&mut state, ms(999)), []);
        assert_eq!(tick(&mut state, ms(1000)), save("b\n"));
        state.update(Message::Saved);
        assert_eq!(state.save_state(), SaveState::Saved);
        assert_eq!(state.unsaved_text(), None);
        let mut state = recovered();
        assert_eq!(state.update(Message::Quit), save("b\n"));
        assert_eq!(state.update(Message::Saved), [Effect::Quit]);
    }

    // FR-DOC-07
    #[test]
    fn backs_up_an_opened_file_on_start() {
        let backup = [Effect::Disk(Disk::Backup {
            path: PathBuf::from(FILE),
        })];
        assert_eq!(opened().update(Message::Started), backup);
        let read_only = opened().with_access(Access::ReadOnly);
        assert_eq!(read_only.clone().update(Message::Started), backup);
        assert_eq!(read_only.path(), Some(Path::new(FILE)));
        let unnamed = State::new(format::parse("a\n"), None);
        assert_eq!(unnamed.clone().update(Message::Started), []);
        let temporary = opened().with_origin(Origin::Temporary);
        assert_eq!(temporary.clone().update(Message::Started), []);
    }

    // FR-DOC-05, file-io.md §2
    #[test]
    fn the_temporary_file_is_saved_without_backups() {
        let mut state = opened().with_origin(Origin::Temporary);
        edit(&mut state, "b", ms(0));
        assert_eq!(
            tick(&mut state, ms(1000)),
            [Effect::Disk(Disk::Save {
                path: PathBuf::from(FILE),
                text: "b\n".to_owned(),
                expected: None,
            })]
        );
    }

    // FR-DOC-07
    #[test]
    fn a_failed_backup_is_shown_once_for_a_while() {
        let mut state = opened();
        tick(&mut state, ms(500));
        let failure = |reason: &str| Message::BackupFailed {
            reason: reason.to_owned(),
        };
        state.update(failure("no space"));
        let notice = Notice::BackupFailed {
            reason: "no space".to_owned(),
        };
        assert_eq!(state.notice(), Some(&notice));
        assert_eq!(state.deadline(), Some(ms(500) + NOTICE_TIME));
        edit(&mut state, "b", ms(600));
        assert_eq!(state.deadline(), Some(ms(1600)), "the sooner of the two");
        tick(&mut state, ms(499) + NOTICE_TIME);
        assert_eq!(state.notice(), Some(&notice));
        tick(&mut state, ms(500) + NOTICE_TIME);
        assert_eq!(state.notice(), None);
        state.update(failure("again"));
        assert_eq!(state.notice(), None, "once per session");
    }
}
