//! The open file as someone else may change it: which version of it the core last read or wrote,
//! and what happens when the file on disk is no longer that version: a silent reload, the
//! conflict prompt with its conflict copies, or a note that the file is gone
//! ([file-io.md §3](../../../docs/specs/file-io.md#3-external-changes), F-FILE-06, OQ-16).

use std::{
    collections::HashSet,
    hash::{DefaultHasher, Hasher as _},
    path::PathBuf,
};

use similar::{ChangeTag, TextDiff};

use super::{
    Effect, History, Notice, SaveState, State, saving::Ending, text_path::TextPath, update::Disk,
};
use crate::document::{Document, ItemId, format};

/// Unchanged lines the diff shows around each change.
const CONTEXT: usize = 3;

/// A fingerprint of a file's bytes, which tells whether the file on disk is still the version the
/// core knows. It is only compared within one session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    len: usize,
    hash: u64,
}

impl Version {
    /// The version of a file holding `bytes`.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        let mut hasher = DefaultHasher::new();
        hasher.write(bytes);
        Self {
            len: bytes.len(),
            hash: hasher.finish(),
        }
    }
}

/// One line of the diff between the file on disk and the local version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLine {
    /// A line both versions have.
    Same(String),
    /// A line only the file on disk has.
    Disk(String),
    /// A line only the local version has.
    Mine(String),
    /// Unchanged lines left out between two changes.
    Gap,
}

/// What the core knows about the file on disk.
#[derive(Debug, Clone, Default)]
pub(super) struct External {
    /// The version the core last read or wrote, `None` when there is no file.
    known: Option<Version>,
    /// The version the save under way writes.
    writing: Option<Version>,
    /// Whether a check found the file deleted or moved away; the next save creates it again.
    gone: bool,
    conflict: Option<Conflict>,
}

/// The file changed on disk while the local version had unsaved changes.
#[derive(Debug, Clone)]
struct Conflict {
    /// The file as it is on disk.
    theirs: Vec<u8>,
    /// Whether the diff is shown.
    diff: bool,
    /// The choice waiting for its conflict copy to be written.
    choosing: Option<Choice>,
}

/// How a conflict is resolved, each after a conflict copy of the version it discards.
#[derive(Debug, Clone)]
enum Choice {
    /// Overwrite the file on disk with the local version.
    KeepMine,
    /// Reload the file on disk, read as this document, discarding the local version.
    TakeTheirs(Document),
    /// Quit, leaving the file on disk as it is.
    Leave,
}

impl External {
    /// Starts a save of `text`, returning the version the file must still be for it to go ahead.
    pub(super) fn start_save(&mut self, text: &str) -> Option<Version> {
        self.writing = Some(Version::of(text.as_bytes()));
        self.known
    }

    /// The save under way is done, or was not done.
    pub(super) fn save_ended(&mut self, saved: bool) {
        let writing = self.writing.take();
        if saved && writing.is_some() {
            self.known = writing;
            self.gone = false;
        }
    }

    /// The open file is now another one, which was just written at `version`.
    pub(super) fn moved(&mut self, version: Version) {
        *self = Self {
            known: Some(version),
            ..Self::default()
        };
    }

    /// Whether the conflict prompt is open, which pauses saving.
    pub(super) fn in_conflict(&self) -> bool {
        self.conflict.is_some()
    }
}

impl State {
    /// This state with the file on disk at `version`, or with no file there yet when `None`.
    #[must_use]
    pub fn with_version(mut self, version: Option<Version>) -> Self {
        self.external.known = version;
        self
    }

    /// Watching the open file, when there is one, for changes made elsewhere (F-FILE-06).
    pub(super) fn watch(&self) -> Vec<Effect> {
        self.path()
            .map(|path| Effect::Watch(path.into()))
            .into_iter()
            .collect()
    }

    /// A check of the file on disk, when there is one to check. None while Save As writes the
    /// document elsewhere: the result could only arrive once that file is the open one.
    pub(super) fn check_file(&self) -> Vec<Effect> {
        if self.saving_as() {
            return Vec::new();
        }
        self.path()
            .map(|path| Effect::Disk(Disk::Check { path: path.into() }))
            .into_iter()
            .collect()
    }

    /// Takes in what is on disk now, `None` when there is no file: nothing when it is the known
    /// version; else a file that is gone is marked so, and a changed one is reloaded when nothing
    /// local is unsaved and it can be read, or opens the conflict prompt.
    pub(super) fn found(&mut self, contents: Option<Vec<u8>>) {
        let version = contents.as_deref().map(Version::of);
        if version == self.external.known {
            return;
        }
        self.external.known = version;
        self.external.gone = contents.is_none();
        if let Some(conflict) = &mut self.external.conflict {
            // The version a choice would discard changed, so the choice no longer stands.
            conflict
                .choosing
                .take_if(|choice| !matches!(choice, Choice::Leave));
            if let Some(theirs) = contents {
                conflict.theirs = theirs;
            }
            return;
        }
        let Some(theirs) = contents else { return };
        match format::parse_bytes(&theirs) {
            Ok(document) if self.save_state() == SaveState::Saved => {
                self.reload(document);
                self.notify(Notice::Reloaded);
            }
            Ok(_) | Err(_) => {
                self.external.conflict = Some(Conflict {
                    theirs,
                    diff: false,
                    choosing: None,
                });
            }
        }
    }

    /// Keeps the local version: once the file on disk is written to a conflict copy, the local
    /// version overwrites it.
    pub(super) fn keep_mine(&mut self) -> Vec<Effect> {
        let theirs = self
            .external
            .conflict
            .as_ref()
            .map(|conflict| conflict.theirs.clone())
            .unwrap_or_default();
        self.choose(Choice::KeepMine, theirs)
    }

    /// Takes the file on disk: once the local version is written to a conflict copy, the file is
    /// reloaded. A file that is not UTF-8 cannot be taken.
    pub(super) fn take_theirs(&mut self) -> Vec<Effect> {
        let Some(conflict) = &self.external.conflict else {
            return Vec::new();
        };
        match format::parse_bytes(&conflict.theirs) {
            Ok(theirs) => {
                let mine = format::serialize(&self.document).into_bytes();
                self.choose(Choice::TakeTheirs(theirs), mine)
            }
            Err(error) => {
                self.notify(Notice::CannotTakeTheirs {
                    reason: error.to_string(),
                });
                Vec::new()
            }
        }
    }

    /// Starts `choice` with a conflict copy of `discarded`, the version it discards, unless no
    /// conflict is open or a choice is under way.
    fn choose(&mut self, choice: Choice, discarded: Vec<u8>) -> Vec<Effect> {
        let Some(conflict) =
            (self.external.conflict.as_mut()).filter(|conflict| conflict.choosing.is_none())
        else {
            return Vec::new();
        };
        conflict.choosing = Some(choice);
        self.conflict_copy(discarded)
    }

    /// Writes `contents` to a conflict copy beside the open file's backups.
    fn conflict_copy(&self, contents: Vec<u8>) -> Vec<Effect> {
        self.path()
            .map(|path| {
                Effect::Disk(Disk::ConflictCopy {
                    path: path.into(),
                    contents,
                })
            })
            .into_iter()
            .collect()
    }

    /// Shows or hides the diff between the file on disk and the local version.
    pub(super) fn toggle_diff(&mut self) {
        if let Some(conflict) = &mut self.external.conflict {
            conflict.diff = !conflict.diff;
        }
    }

    /// The conflict copy of the choice under way was written to `copy`, so the choice goes ahead.
    pub(super) fn conflict_copied(&mut self, copy: PathBuf) {
        let Some(Conflict {
            choosing: Some(choice),
            ..
        }) = self
            .external
            .conflict
            .take_if(|conflict| conflict.choosing.is_some())
        else {
            return;
        };
        match choice {
            Choice::KeepMine => {
                self.autosave.overwrite();
                self.notify(Notice::KeptMine { copy });
            }
            Choice::TakeTheirs(theirs) => {
                self.reload(theirs);
                self.notify(Notice::TookTheirs { copy });
            }
            Choice::Leave => self.autosave.end(Ending::Discard),
        }
    }

    /// The conflict copy of the choice under way could not be written, so the choice is
    /// cancelled and the prompt stays; a quit waiting for it asks instead, as after a failed save.
    pub(super) fn conflict_copy_failed(&mut self, reason: String) {
        let Some(choice) = (self.external.conflict.as_mut()).and_then(|c| c.choosing.take()) else {
            return;
        };
        if matches!(choice, Choice::Leave) {
            self.autosave.ending_failed();
        }
        self.notify(Notice::ConflictCopyFailed { reason });
    }

    /// Ending the session while the prompt is open leaves the file on disk as it is and writes
    /// the local version to a conflict copy instead, so neither is lost: a quit waits for the
    /// copy, while a termination, or a quit confirmed after the copy failed, ends at once.
    pub(super) fn leave_conflict(&mut self) -> Vec<Effect> {
        let mine = format::serialize(&self.document).into_bytes();
        match self.autosave.ending() {
            None => Vec::new(),
            Some(Ending::Quit) => self.choose(Choice::Leave, mine),
            Some(Ending::Terminate) => {
                self.external.conflict = None;
                self.autosave.end(Ending::Discard);
                let mut effects = self.conflict_copy(mine);
                effects.push(Effect::Quit);
                effects
            }
            Some(Ending::Discard) => vec![Effect::Quit],
        }
    }

    /// Replaces the document with `document`, keeping each pane's zoom, focus, history and
    /// selection, the collapsed blocks and project entries and the Today view's order on the
    /// blocks they were on; a focus that finds no block moves as after any change. An edit
    /// session ends with the document it edited, whose text is saved or in a conflict copy, and
    /// the date prompt closes with it, as does an edit a command of the command menu would resume
    /// (D-50), and so do Go to Symbol and Move to, whose candidates name blocks of the old
    /// document, and the context menu, whose labels tell the old blocks' state.
    fn reload(&mut self, document: Document) {
        self.edit = None;
        self.date_prompt = None;
        self.symbols = None;
        self.destinations = None;
        self.context_menu = None;
        self.resume = None;
        let old = std::mem::replace(&mut self.document, document);
        let found = |id: ItemId| TextPath::of(&old, id)?.find(&self.document);
        let nearest = |id: ItemId| found(id).map(|(id, _)| id);
        let exact = |id: ItemId| found(id).and_then(|(id, exact)| exact.then_some(id));
        for pane in std::iter::once(&mut self.pane).chain(&mut self.others) {
            pane.rematch(nearest, exact);
        }
        self.folds = self
            .folds
            .iter()
            .filter_map(|&id| exact(id))
            .collect::<HashSet<_>>();
        self.sidebar.rematch_folds(exact);
        self.today_order.rematch(exact);
        self.edits = History::default();
        self.autosave.reloaded(self.document.revision());
    }

    /// Whether the conflict prompt is open: the file changed on disk while there were unsaved
    /// changes (file-io.md §3).
    #[must_use]
    pub fn in_conflict(&self) -> bool {
        self.external.in_conflict()
    }

    /// The diff from the file on disk to the local version while the prompt shows it: the
    /// changed lines with a few unchanged ones around each change.
    #[must_use]
    pub fn diff(&self) -> Option<Vec<DiffLine>> {
        let conflict = self
            .external
            .conflict
            .as_ref()
            .filter(|conflict| conflict.diff)?;
        Some(diff(
            &String::from_utf8_lossy(&conflict.theirs),
            &format::serialize(&self.document),
        ))
    }

    /// Whether the file was deleted or moved away on disk; the next save creates it again.
    #[must_use]
    pub fn gone_from_disk(&self) -> bool {
        self.external.gone
    }
}

/// The lines that differ from `disk` to `mine`, with [`CONTEXT`] unchanged lines around each
/// change and a [`DiffLine::Gap`] between changes further apart.
fn diff(disk: &str, mine: &str) -> Vec<DiffLine> {
    let diff = TextDiff::from_lines(disk, mine);
    let mut lines = Vec::new();
    for (index, group) in diff.grouped_ops(CONTEXT).iter().enumerate() {
        if index > 0 {
            lines.push(DiffLine::Gap);
        }
        for change in group.iter().flat_map(|op| diff.iter_changes(op)) {
            let text = change.value().trim_end_matches(['\n', '\r']).to_owned();
            lines.push(match change.tag() {
                ChangeTag::Equal => DiffLine::Same(text),
                ChangeTag::Delete => DiffLine::Disk(text),
                ChangeTag::Insert => DiffLine::Mine(text),
            });
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        app::{
            Menu, MenuInput, Message, SelectionChange, SpecialView, Symbols, state::NOTICE_TIME,
        },
        document::{Op, Transaction},
    };

    const FILE: &str = "/notes/todo.taskpaper";

    fn ms(millis: u64) -> Duration {
        Duration::from_millis(millis)
    }

    fn version(text: &str) -> Version {
        Version::of(text.as_bytes())
    }

    /// The file holding `text`, opened at time 0.
    fn opened(text: &str) -> State {
        let mut state = State::new(format::parse(text), Some(PathBuf::from(FILE)))
            .with_version(Some(version(text)));
        state.update(Message::Tick {
            now: ms(0),
            today: jiff::civil::Date::default(),
        });
        state
    }

    /// Changes the first line's text to `text` at time `now`, as a command would.
    fn edit(state: &mut State, text: &str, now: Duration) -> Vec<Effect> {
        state.update(Message::Tick {
            now,
            today: jiff::civil::Date::default(),
        });
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
        state.update(Message::Tick {
            now,
            today: jiff::civil::Date::default(),
        })
    }

    fn found(text: &str) -> Message {
        Message::FileChecked {
            contents: Some(text.as_bytes().to_vec()),
        }
    }

    fn texts(state: &State) -> Vec<String> {
        state
            .rows()
            .iter()
            .map(|row| row.text.to_string())
            .collect()
    }

    /// The text of the block with id `id`.
    fn text_of(state: &State, id: Option<ItemId>) -> Option<String> {
        state
            .document()
            .get(id?)
            .map(crate::document::Item::display_text)
            .map(str::to_owned)
    }

    fn copy(contents: &str) -> Vec<Effect> {
        vec![Effect::Disk(Disk::ConflictCopy {
            path: PathBuf::from(FILE),
            contents: contents.as_bytes().to_vec(),
        })]
    }

    fn copied() -> Message {
        Message::ConflictCopied {
            copy: PathBuf::from("/backups/copy.taskpaper"),
        }
    }

    fn save_over(text: &str, expected: Option<Version>) -> Vec<Effect> {
        let path = PathBuf::from(FILE);
        vec![
            Effect::Disk(Disk::Backup { path: path.clone() }),
            Effect::Disk(Disk::Save {
                path,
                text: text.to_owned(),
                expected,
            }),
        ]
    }

    /// The file holding `a`, with the local change to `mine` unsaved and the file changed on
    /// disk to `theirs`, so the prompt is open.
    fn conflicted(theirs: &str) -> State {
        let mut state = opened("a\n");
        edit(&mut state, "mine", ms(0));
        state.update(found(theirs));
        assert!(state.in_conflict());
        state
    }

    #[test]
    fn versions_tell_contents_apart() {
        assert_eq!(Version::of(b"- a\n"), Version::of(b"- a\n"));
        assert_ne!(Version::of(b"- a\n"), Version::of(b"- b\n"));
        assert_ne!(Version::of(b""), Version::of(b"\0"));
    }

    // F-FILE-06
    #[test]
    fn checks_the_open_file_when_asked() {
        assert_eq!(
            opened("a\n").update(Message::CheckFile),
            [Effect::Disk(Disk::Check {
                path: PathBuf::from(FILE)
            })]
        );
        assert_eq!(
            State::new(Document::default(), None).update(Message::CheckFile),
            []
        );
    }

    #[test]
    fn the_known_version_changes_nothing() {
        let mut state = opened("a\n");
        state.update(found("a\n"));
        assert_eq!(state.notice(), None);
        assert!(!state.gone_from_disk());
        let mut state = opened("a\n");
        edit(&mut state, "b", ms(0));
        assert_eq!(
            state.update(Message::Tick {
                now: ms(1000),
                today: jiff::civil::Date::default()
            }),
            save_over("b\n", Some(version("a\n")))
        );
        state.update(Message::Saved);
        state.update(found("b\n"));
        assert_eq!(state.notice(), None, "the echo of its own save");
        assert!(!state.in_conflict());
    }

    // F-FILE-06, OQ-16
    #[test]
    fn a_change_without_unsaved_changes_reloads_keeping_zoom_focus_and_folds() {
        let mut state = opened("A:\n\t- b\n\t\t- c\n\t- d\nE:\n");
        state.update(Message::ZoomIn);
        state.update(Message::Collapse);
        state.update(Message::FocusLast);
        assert_eq!(texts(&state), ["A", "b", "d"]);
        let effects = state.update(found("X:\nA: @x\n\t- new\n\t- b\n\t\t- c\n\t- d\n"));
        assert_eq!(effects, []);
        assert_eq!(state.notice(), Some(&Notice::Reloaded));
        assert_eq!(state.save_state(), SaveState::Saved);
        assert_eq!(text_of(&state, state.zoom()).as_deref(), Some("A"));
        assert_eq!(text_of(&state, state.focus()).as_deref(), Some("d"));
        assert_eq!(texts(&state), ["A", "new", "b", "d"]);
        assert_eq!(
            state
                .rows()
                .iter()
                .map(|row| row.hidden_tasks)
                .collect::<Vec<_>>(),
            [None, None, Some(1), None],
            "b stays collapsed"
        );
        state.update(Message::Back);
        assert_eq!(state.zoom(), None, "the history is kept");
        assert_eq!(
            state.update(Message::Tick {
                now: ms(9000),
                today: jiff::civil::Date::default()
            }),
            [],
            "a reload is not a change to save"
        );
    }

    // F-FILE-06, D-7
    #[test]
    fn a_reload_keeps_the_today_view_and_its_focus_and_lists_the_new_blocks() {
        let mut state = opened("- a @star\n- b @star\n");
        state.update(Message::FocusNext);
        state.update(Message::ZoomIn);
        state.go_to(crate::app::Location::View(SpecialView::Today));
        state.update(found("- new @star\n- a @star\n- b @star @x\n"));
        assert_eq!(
            state.location(),
            crate::app::Location::View(SpecialView::Today)
        );
        assert_eq!(texts(&state), ["new", "a", "b"]);
        assert_eq!(text_of(&state, state.focus()).as_deref(), Some("b"));
        state.update(Message::Back);
        assert_eq!(
            text_of(&state, state.zoom()).as_deref(),
            Some("b"),
            "the history is kept"
        );
    }

    // F-FILE-06, D-57
    #[test]
    fn a_reload_keeps_the_today_views_order() {
        let mut state = opened("- a @star\n- b @star\n");
        state.go_to(crate::app::Location::View(SpecialView::Today));
        state.update(Message::FocusLast);
        state.update(Message::MoveUp);
        assert_eq!(texts(&state), ["b", "a"]);
        state.update(found("- new @star\n- a @star\n- b @star @x\n"));
        assert_eq!(texts(&state), ["b", "a", "new"]);
    }

    /// Each project entry of the sidebar with its fold.
    fn project_folds(state: &State) -> Vec<(String, crate::app::Fold)> {
        let view = state.sidebar().expect("open");
        view.entries
            .iter()
            .filter_map(|entry| match entry {
                crate::app::SidebarEntry::Project { text, fold, .. } => Some((text.clone(), *fold)),
                crate::app::SidebarEntry::Inbox { .. }
                | crate::app::SidebarEntry::Bookmark { .. }
                | crate::app::SidebarEntry::View { .. } => None,
            })
            .collect()
    }

    // F-FILE-06, FR-UI-01
    #[test]
    fn a_reload_keeps_folded_project_entries_on_their_projects() {
        use crate::app::{Fold, SidebarInput};
        let mut state = opened("A:\n\tA1:\nB:\n\tB1:\nC:\n\tC1:\n");
        state.update(Message::ToggleSidebar);
        for input in [SidebarInput::Next; 3] {
            state.update(Message::Sidebar(input));
        }
        state.update(Message::Sidebar(SidebarInput::Collapse));
        state.update(found("B:\n\tB1:\nA:\n\tA1:\nC2:\n\tC1:\n"));
        let folds = project_folds(&state);
        let fold = |name: &str| {
            folds
                .iter()
                .find(|(text, _)| text == name)
                .map(|&(_, fold)| fold)
        };
        assert_eq!(fold("B"), Some(Fold::Collapsed), "{folds:?}");
        assert_eq!(fold("A"), Some(Fold::Expanded));
        assert_eq!(fold("C2"), Some(Fold::Expanded));
    }

    #[test]
    fn a_block_that_is_gone_falls_back_to_its_nearest_surviving_ancestor() {
        let mut state = opened("A:\n\t- b\n\t\t- c\n");
        state.update(Message::FocusNext);
        state.update(Message::ZoomIn);
        state.update(Message::FocusLast);
        assert_eq!(text_of(&state, state.focus()).as_deref(), Some("c"));
        state.update(found("A:\n\t- x\n"));
        assert_eq!(text_of(&state, state.zoom()).as_deref(), Some("A"));
        assert_eq!(text_of(&state, state.focus()).as_deref(), Some("A"));
        state.update(found("Z:\n"));
        assert_eq!(state.zoom(), None);
        assert_eq!(text_of(&state, state.focus()).as_deref(), Some("Z"));
        state.update(found(""));
        assert_eq!(state.focus(), None);
    }

    #[test]
    fn blocks_with_equal_texts_are_told_apart_by_their_order() {
        let mut state = opened("- x\n- x\n");
        state.update(Message::FocusLast);
        let before = state.focus();
        state.update(found("- y\n- x\n- x\n"));
        assert_eq!(texts(&state), ["y", "x", "x"]);
        assert_eq!(state.focus(), state.rows().get(2).map(|row| row.id));
        assert_ne!(state.focus(), before);
    }

    // F-FILE-06
    #[test]
    fn a_deleted_file_is_marked_and_created_again_by_the_next_save() {
        let mut state = opened("a\n");
        state.update(Message::FileChecked { contents: None });
        assert!(state.gone_from_disk());
        assert_eq!(texts(&state), ["a"], "the buffer is kept");
        assert_eq!(state.update(Message::FileChecked { contents: None }), []);
        edit(&mut state, "b", ms(0));
        assert_eq!(
            state.update(Message::Tick {
                now: ms(1000),
                today: jiff::civil::Date::default()
            }),
            save_over("b\n", None)
        );
        state.update(Message::Saved);
        assert!(!state.gone_from_disk());
        state.update(Message::FileChecked { contents: None });
        state.update(found("c\n"));
        assert!(!state.gone_from_disk(), "a file that is back is not gone");
        assert_eq!(texts(&state), ["c"]);
    }

    // OQ-16
    #[test]
    fn a_change_with_unsaved_changes_opens_the_prompt_and_pauses_saving() {
        let mut state = conflicted("theirs\n");
        assert_eq!(texts(&state), ["mine"]);
        assert_eq!(state.notice(), None);
        assert_eq!(state.deadline(), None);
        assert_eq!(
            state.update(Message::Tick {
                now: ms(9000),
                today: jiff::civil::Date::default()
            }),
            []
        );
        assert_eq!(state.save_state(), SaveState::Unsaved);
    }

    // OQ-16, D-22
    #[test]
    fn keeping_mine_copies_theirs_then_overwrites_the_file_at_once() {
        let mut state = conflicted("theirs\n");
        assert_eq!(state.update(Message::KeepMine), copy("theirs\n"));
        assert_eq!(state.update(Message::KeepMine), [], "one choice at a time");
        assert_eq!(state.update(Message::TakeTheirs), []);
        assert!(state.in_conflict(), "until the copy is written");
        assert_eq!(
            state.update(copied()),
            save_over("mine\n", Some(version("theirs\n")))
        );
        assert!(!state.in_conflict());
        assert_eq!(
            state.notice(),
            Some(&Notice::KeptMine {
                copy: PathBuf::from("/backups/copy.taskpaper")
            })
        );
        state.update(Message::Saved);
        assert_eq!(state.save_state(), SaveState::Saved);
    }

    // OQ-16, D-22
    #[test]
    fn taking_theirs_copies_mine_then_reloads() {
        let mut state = conflicted("- theirs\n");
        assert_eq!(state.update(Message::TakeTheirs), copy("mine\n"));
        assert_eq!(texts(&state), ["mine"]);
        assert_eq!(state.update(copied()), []);
        assert!(!state.in_conflict());
        assert_eq!(texts(&state), ["theirs"]);
        assert_eq!(state.save_state(), SaveState::Saved);
        assert_eq!(
            state.notice(),
            Some(&Notice::TookTheirs {
                copy: PathBuf::from("/backups/copy.taskpaper")
            })
        );
        assert_eq!(
            state.update(Message::Tick {
                now: ms(9000),
                today: jiff::civil::Date::default()
            }),
            []
        );
    }

    // file-io.md §3, FR-TAG-03
    #[test]
    fn a_reload_closes_the_date_prompt() {
        let mut state = opened("- a\n");
        state.update(Message::OpenDatePrompt {
            tag: crate::document::DateTag::Due,
            today: jiff::civil::date(2026, 10, 5),
        });
        assert!(state.date_prompt().is_some());
        state.update(found("- b\n"));
        assert_eq!(state.date_prompt(), None);
    }

    // file-io.md §3, FR-NAV-05
    #[test]
    fn a_reload_closes_go_to_symbol() {
        let mut state = opened("A\nB\nC\n");
        state.update(Message::OpenGoToSymbol(Symbols::All));
        state.update(Message::Menu(Menu::GoToSymbol, MenuInput::Type('c')));
        state.update(found("X\nA\nB\nC\n"));
        assert_eq!(state.go_to_symbol(), None);
        state.update(Message::Menu(Menu::GoToSymbol, MenuInput::Choose));
        assert_eq!(
            state.focus(),
            Some(state.rows()[1].id),
            "still on A, not moved to B"
        );
    }

    // file-io.md §3, FR-OPS-05
    #[test]
    fn a_reload_closes_move_to() {
        let mut state = opened("A\nB\nC\n");
        state.update(Message::OpenMoveTo);
        state.update(Message::Menu(Menu::MoveTo, MenuInput::Type('c')));
        state.update(found("X\nA\nB\nC\n"));
        assert_eq!(state.move_to(), None);
        state.update(Message::Menu(Menu::MoveTo, MenuInput::Choose));
        assert_eq!(format::serialize(state.document()), "X\nA\nB\nC\n");
    }

    // file-io.md §3, FR-KEY-06
    #[test]
    fn a_reload_closes_the_context_menu() {
        let mut state = opened("- A\n");
        state.update(Message::OpenContextMenu);
        assert!(state.context_menu().is_some());
        state.update(found("- A @done\n"));
        assert_eq!(state.context_menu(), None);
    }

    // file-io.md §3, D-50
    #[test]
    fn a_reload_forgets_the_edit_a_menu_command_would_resume() {
        let mut state = opened("- a\n- b\n");
        state.update(Message::EditAtEnd);
        state.update(Message::OpenCommandMenu {
            today: jiff::civil::date(2026, 10, 5),
        });
        for c in "set due".chars() {
            state.update(Message::Menu(Menu::CommandMenu, MenuInput::Type(c)));
        }
        state.update(Message::Menu(Menu::CommandMenu, MenuInput::Choose));
        assert!(state.date_prompt().is_some());
        state.update(found("- x\n- y\n"));
        state.update(Message::FocusLast);
        state.update(Message::OpenDetails);
        state.update(Message::CloseDetails);
        assert_eq!(state.mode(), crate::app::Mode::Navigate);
        assert_eq!(state.focus(), Some(state.rows()[1].id));
    }

    // file-io.md §3, FR-OPS-08
    #[test]
    fn a_reload_forgets_the_edits_to_undo() {
        let mut state = opened("- a\n");
        state.update(Message::ToggleDone {
            today: jiff::civil::date(2026, 10, 5),
        });
        state.update(Message::Tick {
            now: ms(1000),
            today: jiff::civil::Date::default(),
        });
        state.update(Message::Saved);
        state.update(found("- b\n"));
        state.update(Message::Undo);
        assert_eq!(format::serialize(state.document()), "- b\n");
        assert_eq!(state.notice(), Some(&Notice::Reloaded));
    }

    // file-io.md §3
    #[test]
    fn taking_theirs_ends_a_failed_save() {
        let mut state = opened("a\n");
        edit(&mut state, "mine", ms(0));
        state.update(Message::Tick {
            now: ms(1000),
            today: jiff::civil::Date::default(),
        });
        state.update(Message::SaveFailed {
            reason: "disk full".to_owned(),
        });
        assert_eq!(
            state.save_state(),
            SaveState::Failed {
                reason: "disk full"
            }
        );
        state.update(found("theirs\n"));
        state.update(Message::TakeTheirs);
        state.update(copied());
        assert_eq!(state.save_state(), SaveState::Saved);
    }

    // file-io.md §3
    #[test]
    fn a_failed_conflict_copy_cancels_the_choice() {
        let mut state = conflicted("theirs\n");
        state.update(Message::ConflictCopyFailed {
            reason: "stray".to_owned(),
        });
        assert_eq!(state.notice(), None, "no choice was under way");
        state.update(Message::TakeTheirs);
        state.update(Message::ConflictCopyFailed {
            reason: "disk full".to_owned(),
        });
        assert_eq!(
            state.notice(),
            Some(&Notice::ConflictCopyFailed {
                reason: "disk full".to_owned()
            })
        );
        assert!(state.in_conflict());
        assert_eq!(texts(&state), ["mine"]);
        assert_eq!(state.update(copied()), [], "a late copy chooses nothing");
        assert!(state.in_conflict());
        assert_eq!(state.update(Message::KeepMine), copy("theirs\n"));
    }

    #[test]
    fn a_newer_change_on_disk_cancels_a_choice_under_way() {
        let mut state = conflicted("theirs\n");
        state.update(Message::KeepMine);
        state.update(found("newer\n"));
        assert_eq!(state.update(copied()), []);
        assert!(state.in_conflict());
        assert_eq!(state.update(Message::KeepMine), copy("newer\n"));
        state.update(Message::FileChecked { contents: None });
        assert!(state.gone_from_disk());
        assert_eq!(state.update(copied()), []);
        assert!(state.in_conflict(), "the prompt stays for a deleted file");
        assert_eq!(state.update(Message::KeepMine), copy("newer\n"));
        assert_eq!(state.update(copied()), save_over("mine\n", None));
    }

    // F-FILE-07, OQ-16
    #[test]
    fn a_file_changed_to_bytes_that_are_not_utf8_cannot_be_reloaded_or_taken() {
        let mut state = opened("a\n");
        state.update(Message::FileChecked {
            contents: Some(b"caf\xe9\n".to_vec()),
        });
        assert!(state.in_conflict(), "even without unsaved changes");
        assert_eq!(texts(&state), ["a"]);
        assert_eq!(state.update(Message::TakeTheirs), []);
        assert!(matches!(
            state.notice(),
            Some(Notice::CannotTakeTheirs { reason }) if reason.contains("not valid UTF-8")
        ));
        assert_eq!(
            state.update(Message::KeepMine),
            [Effect::Disk(Disk::ConflictCopy {
                path: PathBuf::from(FILE),
                contents: b"caf\xe9\n".to_vec(),
            })]
        );
        assert_eq!(
            state.update(copied()),
            save_over("a\n", Some(Version::of(b"caf\xe9\n"))),
            "keeping an unchanged version still overwrites the file"
        );
        assert_eq!(state.save_state(), SaveState::Unsaved);
    }

    // file-io.md §3
    #[test]
    fn a_refused_save_opens_the_prompt_or_creates_a_deleted_file_again() {
        let mut state = opened("a\n");
        edit(&mut state, "mine", ms(0));
        assert_eq!(
            state.update(Message::Tick {
                now: ms(1000),
                today: jiff::civil::Date::default()
            }),
            save_over("mine\n", Some(version("a\n")))
        );
        state.update(Message::SaveRefused {
            contents: Some(b"theirs\n".to_vec()),
        });
        assert!(state.in_conflict());
        assert_eq!(state.save_state(), SaveState::Unsaved, "not a failure");
        let mut state = opened("a\n");
        edit(&mut state, "mine", ms(0));
        state.update(Message::Tick {
            now: ms(1000),
            today: jiff::civil::Date::default(),
        });
        assert_eq!(
            state.update(Message::SaveRefused { contents: None }),
            save_over("mine\n", None)
        );
        assert!(state.gone_from_disk());
        state.update(Message::Saved);
        assert!(!state.gone_from_disk());
        assert_eq!(state.save_state(), SaveState::Saved);
    }

    #[test]
    fn a_change_found_while_a_save_runs_opens_the_prompt() {
        let mut state = opened("a\n");
        edit(&mut state, "mine", ms(0));
        state.update(Message::Tick {
            now: ms(1000),
            today: jiff::civil::Date::default(),
        });
        state.update(found("mine\n"));
        assert!(state.in_conflict());
        state.update(Message::SaveRefused {
            contents: Some(b"mine\n".to_vec()),
        });
        assert!(state.in_conflict());
        assert_eq!(state.update(Message::KeepMine), copy("mine\n"));
    }

    // F-FILE-05, file-io.md §3
    #[test]
    fn terminating_in_the_prompt_copies_mine_and_leaves_the_file() {
        let mut state = conflicted("theirs\n");
        let mut effects = copy("mine\n");
        effects.push(Effect::Quit);
        assert_eq!(state.update(Message::Terminate), effects);
        assert!(!state.in_conflict());
        assert_eq!(state.update(Message::Terminate), [Effect::Quit], "one copy");
    }

    // F-FILE-05, file-io.md §3
    #[test]
    fn quitting_in_the_prompt_copies_mine_then_quits_leaving_the_file() {
        let mut state = conflicted("theirs\n");
        assert_eq!(state.update(Message::Quit), copy("mine\n"));
        assert_eq!(state.update(Message::Quit), [], "one copy");
        assert_eq!(state.update(Message::KeepMine), [], "the quit goes ahead");
        assert_eq!(state.update(found("newer\n")), [], "whatever is on disk");
        assert!(state.in_conflict());
        assert_eq!(state.update(copied()), [Effect::Quit]);
        assert!(!state.in_conflict());
    }

    // file-io.md §2–3
    #[test]
    fn a_quit_whose_copy_failed_asks_before_quitting_without_it() {
        let mut state = conflicted("theirs\n");
        state.update(Message::Quit);
        assert_eq!(
            state.update(Message::ConflictCopyFailed {
                reason: "disk full".to_owned(),
            }),
            []
        );
        assert!(state.in_conflict());
        assert!(state.confirming_quit());
        assert_eq!(
            state.notice(),
            Some(&Notice::ConflictCopyFailed {
                reason: "disk full".to_owned()
            })
        );
        assert_eq!(state.update(Message::Quit), [Effect::Quit]);
    }

    // OQ-16
    #[test]
    fn the_diff_is_shown_on_request() {
        let mut state = conflicted("theirs\n");
        assert_eq!(state.diff(), None);
        state.update(Message::ToggleDiff);
        assert_eq!(
            state.diff(),
            Some(vec![
                DiffLine::Disk("theirs".to_owned()),
                DiffLine::Mine("mine".to_owned()),
            ])
        );
        state.update(Message::ToggleDiff);
        assert_eq!(state.diff(), None);
    }

    #[test]
    fn the_diff_shows_changes_with_three_lines_around_each() {
        let disk = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n";
        let mine = "1\nTWO\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n";
        let same = |text: &str| DiffLine::Same(text.to_owned());
        assert_eq!(
            diff(disk, mine),
            [
                same("1"),
                DiffLine::Disk("2".to_owned()),
                DiffLine::Mine("TWO".to_owned()),
                same("3"),
                same("4"),
                same("5"),
                DiffLine::Gap,
                same("10"),
                same("11"),
                same("12"),
                DiffLine::Mine("13".to_owned()),
            ]
        );
        assert_eq!(diff("a\r\nb", "a\r\nb"), []);
        assert_eq!(diff("", "x"), [DiffLine::Mine("x".to_owned())]);
    }

    #[test]
    fn prompt_messages_without_a_conflict_change_nothing() {
        let mut state = opened("a\n");
        for message in [
            Message::KeepMine,
            Message::TakeTheirs,
            Message::ToggleDiff,
            copied(),
            Message::ConflictCopyFailed {
                reason: "x".to_owned(),
            },
        ] {
            assert_eq!(state.update(message), []);
        }
        assert_eq!(state.notice(), None);
        assert_eq!(state.diff(), None);
        assert!(!state.in_conflict());
    }

    #[test]
    fn notices_of_a_reload_expire() {
        let mut state = opened("a\n");
        state.update(found("b\n"));
        assert_eq!(state.deadline(), Some(NOTICE_TIME));
    }

    // FR-OPS-01, F-FILE-06
    #[test]
    fn a_reload_keeps_the_selection_on_the_blocks_it_was_on() {
        let mut state = opened("- a\n- b\n- c\n");
        state.update(Message::FocusNext);
        state.update(Message::Select(SelectionChange::Toggle));
        state.update(found("- z\n- a\n- b\n- c\n"));
        let selected: Vec<String> = state
            .rows()
            .iter()
            .filter(|row| row.selected)
            .map(|row| row.text.to_string())
            .collect();
        assert_eq!(selected, ["b"]);
        state.update(Message::Delete);
        assert_eq!(texts(&state), ["z", "a", "c"]);
    }
}
