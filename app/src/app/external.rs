//! The open file as someone else may change it: which version of it the core last read or wrote,
//! and what happens when the file on disk is no longer that version: a silent reload, the
//! conflict prompt with its conflict copies, or a note that the file is gone
//! ([file-io.md §3](../../../docs/specs/file-io.md#3-external-changes), F-FILE-06, OQ-16).

use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hasher as _},
    path::{Path, PathBuf},
};

use similar::{ChangeTag, TextDiff};

use super::{
    Effect, History, Notice, SaveState, State, context_menu::MenuOn, finder::Finder,
    saving::Ending, text_path, update::Disk,
};
use crate::document::{Block, Document, ItemId, format};

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
    /// The document as the save under way writes it, `None` when no save is under way.
    saving: Option<Document>,
    /// The document as the file on disk last held it, as far as the core knows, which keeps
    /// the ids of the open document: what Take theirs finds blocks again by.
    on_disk: Option<Document>,
    /// Whether a check found the file deleted or moved away; the next save creates it again.
    gone: bool,
    conflict: Option<Conflict>,
    /// The conflict copy holding the local version once a quit left the conflict unresolved.
    left_in: Option<PathBuf>,
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
    /// Starts a save of `document`, returning the version the file must still be for it to go
    /// ahead.
    pub(super) fn start_save(&mut self, document: &Document) -> Option<Version> {
        self.saving = Some(document.clone());
        self.known
    }

    /// The save under way left the file at `written`, or was not done when `None`.
    pub(super) fn save_ended(&mut self, written: Option<Version>) {
        if let Some(document) = self.saving.take()
            && let Some(written) = written
        {
            self.known = Some(written);
            self.gone = false;
            self.on_disk = Some(document);
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
        self.external.on_disk = Some(self.document.clone());
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
            Choice::Leave => {
                self.external.left_in = Some(copy);
                self.autosave.end(Ending::Discard);
            }
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
    /// blocks they were on; a focus that finds no block moves as after any change. Editing and
    /// what is open over the outline carry on with the blocks they were on (D-104): editing goes
    /// on with the reloaded block's text, the caret at the same column, as does an edit a command
    /// of the command menu would resume (D-50), and the date prompt stays on its blocks;
    /// each ends or closes once none of its blocks is found. Go to Symbol and Move to list the
    /// reloaded blocks with their query kept, and the context menu its commands for them. Quick
    /// Add stays open, its destination kept on the same project. The Edit Node dialog, a draft of
    /// the block as it was, closes.
    fn reload(&mut self, document: Document) {
        let menu = self.set_editing_aside();
        self.node_editor = None;
        let targets: Vec<ItemId> = if self.destinations.is_some() || self.context_menu.is_some() {
            self.targets().iter().map(Block::id).collect()
        } else {
            Vec::new()
        };
        let old = std::mem::replace(&mut self.document, document);
        let mut matches = text_path::matches(&old, &self.document);
        // Blocks whose unsaved changes theirs discards are found again as they were on disk, so
        // a block whose text was being typed is still found (D-123).
        if let Some(disk) =
            (self.external.on_disk.as_ref()).filter(|disk| disk.revision() != old.revision())
        {
            for (id, on_disk) in text_path::matches(disk, &self.document) {
                matches
                    .entry(id)
                    .and_modify(|local| {
                        if on_disk.1 || !local.1 {
                            *local = on_disk;
                        }
                    })
                    .or_insert(on_disk);
            }
        }
        self.external.on_disk = Some(self.document.clone());
        let found = |id: ItemId| matches.get(&id).copied();
        let nearest = |id: ItemId| found(id).map(|(id, _)| id);
        let exact = |id: ItemId| found(id).and_then(|(id, exact)| exact.then_some(id));
        for pane in std::iter::once(&mut self.pane).chain(&mut self.others) {
            pane.rematch(nearest, exact);
        }
        self.folds = self.folds.iter().filter_map(|&id| exact(id)).collect();
        self.navigator.rematch_folds(exact);
        self.today_order.rematch(exact);
        if let Some(quick_add) = &mut self.quick_add {
            quick_add.rematch(exact);
        }
        self.resume = self
            .resume
            .take()
            .and_then(|resume| resume.rematched(exact));
        self.date_prompt = (self.date_prompt.take()).and_then(|prompt| prompt.rematched(exact));
        self.rematch_panel(&old, exact);
        let symbol = (self.symbols.as_ref())
            .and_then(|(_, finder)| finder.highlighted())
            .and_then(|&id| exact(id));
        // Home, when listed and matched, always ranks first, where the highlight falls back to.
        let destination = (self.destinations.as_ref())
            .and_then(Finder::highlighted)
            .and_then(|&id| exact(id));
        // Move to and a context menu on blocks act on the selected blocks, else the focused block,
        // so with none of those found they close rather than act on where the focus fell back to.
        let targets_found = targets.iter().any(|&id| exact(id).is_some());
        self.changed = changed_lines(&old, &self.document);
        self.edits = History::default();
        self.autosave.reloaded(self.document.revision());
        if !targets_found {
            self.destinations = None;
            let on_blocks = !self.navigator_focused();
            self.context_menu
                .take_if(|(on, _)| on_blocks && *on == MenuOn::Focus);
        }
        self.relist_symbols(symbol);
        self.relist_destinations(destination);
        self.relist_context_menu();
        self.resume_after_reload(menu);
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

    /// The conflict copy holding the local version when a quit left the file on disk as it was
    /// with the conflict prompt open, the only place the user's changes are then (file-io.md §3).
    #[must_use]
    pub fn left_in(&self) -> Option<&Path> {
        self.external.left_in.as_deref()
    }
}

/// The lines of `new` that `old` has no line like, at the same depth with the same text, that no
/// earlier line of `new` was already matched with: what a reload changed, found in one pass
/// however large the files.
fn changed_lines(old: &Document, new: &Document) -> Vec<ItemId> {
    let mut unmatched: HashMap<(usize, &str), usize> = HashMap::new();
    for (id, depth) in old.descendants(old.root()) {
        if let Some(item) = old.get(id) {
            *unmatched.entry((depth, item.text())).or_default() += 1;
        }
    }
    new.descendants(new.root())
        .filter(|&(id, depth)| {
            let count = new
                .get(id)
                .and_then(|item| unmatched.get_mut(&(depth, item.text())))
                .filter(|count| **count > 0);
            count.map(|count| *count -= 1).is_none()
        })
        .map(|(id, _)| id)
        .collect()
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
    use std::{fmt::Write as _, time::Duration};

    use super::*;
    use crate::{
        app::{
            DateInput, DocumentText, Menu, MenuInput, MenuRow, Message, QuickAddInput, QuickAddTo,
            SelectionChange, SpecialView, Symbols, TextEdit, state::NOTICE_TIME, walks::Walks,
        },
        document::{Op, Transaction},
    };

    const FILE: &str = "/notes/todo.taskpaper";

    // UI-ANIM-02
    #[test]
    fn a_reload_changed_the_lines_its_old_outline_has_fewer_of() {
        let old = format::parse("- a\n- a\nB:\n\t- a\n");
        let new = format::parse("- a\n- a\n- a\nB:\n\t- a\n\t- c\n");
        let texts: Vec<&str> = changed_lines(&old, &new)
            .into_iter()
            .filter_map(|id| new.get(id).map(crate::document::Item::text))
            .collect();
        assert_eq!(texts, ["- a", "- c"]);
    }

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

    /// The report of a save that wrote `text`.
    fn saved(text: &str) -> Message {
        Message::Saved {
            version: Version::of(text.as_bytes()),
        }
    }

    fn save_over(text: &str, expected: Option<Version>) -> Vec<Effect> {
        let path = PathBuf::from(FILE);
        vec![
            Effect::Disk(Disk::Backup { path: path.clone() }),
            Effect::Disk(Disk::Save {
                path,
                text: DocumentText::of(&format::parse(text)),
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
        state.update(saved("b\n"));
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
        let changed: Vec<_> = state
            .changed()
            .iter()
            .map(|&id| text_of(&state, Some(id)))
            .collect();
        assert_eq!(
            changed,
            [
                Some("X".to_owned()),
                Some("A".to_owned()),
                Some("new".to_owned())
            ],
            "the reload marks what is new on disk (FR-UI-11)"
        );
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

    // F-FILE-06, D-96
    #[test]
    fn a_reload_keeps_quick_adds_destination_on_its_project_and_closes_the_picker() {
        let mut state = opened("A:\nB:\n");
        let quick_add = |state: &mut State, input| state.update(Message::QuickAdd(input));
        quick_add(&mut state, QuickAddInput::Open(QuickAddTo::Chosen));
        for c in ['x', ' ', '/'] {
            quick_add(&mut state, QuickAddInput::Edit(TextEdit::Insert(c)));
        }
        state.update(Message::Menu(Menu::QuickAdd, MenuInput::Next));
        state.update(Message::Menu(Menu::QuickAdd, MenuInput::Choose));
        quick_add(&mut state, QuickAddInput::Edit(TextEdit::Insert('/')));
        assert!(state.quick_add_picker().is_some());
        state.update(found("New:\nA:\nB:\n"));
        assert_eq!(state.quick_add_picker(), None);
        let destination = |state: &State| state.quick_add().unwrap().destination;
        assert_eq!(destination(&state), Some(vec!["B".to_owned()]));
        quick_add(&mut state, QuickAddInput::Add);
        assert_eq!(format::serialize(state.document()), "New:\nA:\nB:\n\t- x\n");
        let mut state = opened("A:\nB:\n");
        quick_add(&mut state, QuickAddInput::Open(QuickAddTo::Chosen));
        quick_add(&mut state, QuickAddInput::Edit(TextEdit::Insert('/')));
        state.update(Message::Menu(Menu::QuickAdd, MenuInput::Choose));
        assert_eq!(destination(&state), Some(vec!["A".to_owned()]));
        state.update(found("B:\n"));
        assert_eq!(destination(&state), None, "A is gone");
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

    /// Each project entry of the Navigator with its fold.
    fn project_folds(state: &State) -> Vec<(String, crate::app::Fold)> {
        let view = state.navigator().expect("open");
        view.entries
            .iter()
            .filter_map(|entry| match entry {
                crate::app::NavigatorEntry::Project { text, fold, .. } => {
                    Some((text.clone(), *fold))
                }
                crate::app::NavigatorEntry::Inbox { .. }
                | crate::app::NavigatorEntry::Bookmark { .. }
                | crate::app::NavigatorEntry::View { .. } => None,
            })
            .collect()
    }

    // F-FILE-06, FR-UI-01
    #[test]
    fn a_reload_keeps_folded_project_entries_on_their_projects() {
        use crate::app::{Fold, NavigatorInput};
        let mut state = opened("A:\n\tA1:\nB:\n\tB1:\nC:\n\tC1:\n");
        state.update(Message::ToggleNavigator);
        // From Today, past the other special views, A and A1, to B.
        for _ in 0..SpecialView::ALL.len() + 2 {
            state.update(Message::Navigator(NavigatorInput::Next));
        }
        state.update(Message::Navigator(NavigatorInput::Collapse));
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
        state.update(saved("b\n"));
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
        state.update(saved("mine\n"));
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

    /// Types `text` in edit mode.
    fn typed(state: &mut State, text: &str) {
        for c in text.chars() {
            state.update(Message::Text(TextEdit::Insert(c)));
        }
    }

    /// Lets the autosave write the document, as holding `text`, at time `now`.
    fn autosaved(state: &mut State, text: &str, now: Duration) {
        state.update(Message::Tick {
            now,
            today: jiff::civil::Date::default(),
        });
        state.update(saved(text));
        assert_eq!(state.save_state(), SaveState::Saved);
    }

    /// The text being edited with `|` at the caret.
    fn shown(state: &State) -> String {
        let editor = state.editor().expect("editing");
        let mut text = editor.text.to_owned();
        text.insert(editor.caret, '|');
        text
    }

    // file-io.md §3, FR-MODE-01, D-104
    #[test]
    fn a_reload_keeps_editing_the_block_with_the_caret_where_it_was() {
        let mut state = opened("- a\n- b\n");
        state.update(Message::EditAtEnd);
        typed(&mut state, "xy");
        state.update(Message::Text(TextEdit::Move {
            motion: crate::app::Motion::Left,
            select: false,
        }));
        autosaved(&mut state, "- axy\n- b\n", ms(1000));
        state.update(found("- new\n- axy @x\n- b\n"));
        assert_eq!(state.notice(), Some(&Notice::Reloaded));
        assert_eq!(state.editor().map(|editor| editor.id), state.focus());
        assert_eq!(text_of(&state, state.focus()).as_deref(), Some("axy"));
        assert_eq!(shown(&state), "ax|y");
        typed(&mut state, "z");
        assert_eq!(
            format::serialize(state.document()),
            "- new\n- axzy @x\n- b\n",
            "the reloaded tag is kept"
        );
    }

    // file-io.md §3, D-104
    #[test]
    fn keeping_mine_carries_on_editing_with_the_caret_where_it_was() {
        let mut state = opened("- a\n- b\n");
        state.update(Message::EditAtEnd);
        typed(&mut state, "xy");
        state.update(Message::Text(TextEdit::Move {
            motion: crate::app::Motion::Left,
            select: false,
        }));
        state.update(found("- theirs\n"));
        assert!(state.in_conflict());
        state.update(Message::KeepMine);
        state.update(copied());
        assert!(!state.in_conflict());
        assert_eq!(state.mode(), crate::app::Mode::Edit);
        assert_eq!(shown(&state), "ax|y");
        typed(&mut state, "z");
        assert_eq!(format::serialize(state.document()), "- axzy\n- b\n");
    }

    // file-io.md §3, D-104, D-123
    #[test]
    fn taking_theirs_carries_on_editing_in_the_block_found_again() {
        let mut state = opened("- a\n- bc\n");
        state.update(Message::EditAtEnd);
        typed(&mut state, "x");
        state.update(Message::EditNext);
        state.update(Message::Text(TextEdit::Move {
            motion: crate::app::Motion::Left,
            select: false,
        }));
        let caret = shown(&state);
        state.update(found("- a\n- bc @x\n- d\n"));
        assert!(state.in_conflict());
        state.update(Message::TakeTheirs);
        assert_eq!(
            state.mode(),
            crate::app::Mode::Edit,
            "editing goes on while the copy is written"
        );
        state.update(copied());
        assert!(!state.in_conflict());
        assert_eq!(state.mode(), crate::app::Mode::Edit);
        assert_eq!(shown(&state), caret);
        typed(&mut state, "z");
        assert_eq!(format::serialize(state.document()), "- a\n- bzc @x\n- d\n");
    }

    // file-io.md §3, D-123
    #[test]
    fn taking_theirs_carries_on_editing_the_block_whose_text_was_the_unsaved_change() {
        let mut state = opened("A:\n\t- x\n\t- y\n");
        state.update(Message::FocusNext);
        state.update(Message::EditAtEnd);
        typed(&mut state, " mine");
        autosaved(&mut state, "A:\n\t- x mine\n\t- y\n", ms(1000));
        typed(&mut state, "s");
        state.update(found("A:\n\t- x mine\n\t- y\n\t- from phone\n"));
        state.update(Message::TakeTheirs);
        state.update(copied());
        assert_eq!(state.mode(), crate::app::Mode::Edit);
        assert_eq!(shown(&state), "x mine|");
        typed(&mut state, "Q");
        assert_eq!(
            format::serialize(state.document()),
            "A:\n\t- x mineQ\n\t- y\n\t- from phone\n"
        );
    }

    // file-io.md §3, D-123
    #[test]
    fn taking_theirs_prefers_where_a_block_was_on_disk_unless_only_its_local_place_is_whole() {
        // The second `a` is the first `a` locally, but was the second on disk.
        let mut state = opened("- a\n- a\n");
        edit(&mut state, "- b", ms(0));
        state.update(Message::FocusNext);
        state.update(found("- a\n- a\n- c\n"));
        state.update(Message::TakeTheirs);
        state.update(copied());
        assert_eq!(state.focus(), state.rows().get(1).map(|row| row.id));
        // `Q2:` is whole only locally: on disk its path breaks off at `R:`.
        let mut state = opened("R:\n\tQ:\n\t\t- x\n");
        state.update(Message::FocusNext);
        state.update(Message::EditAtEnd);
        typed(&mut state, "2");
        state.update(found("R:\n\tQ2:\n\t\t- x\n- new\n"));
        state.update(Message::TakeTheirs);
        state.update(copied());
        assert_eq!(state.mode(), crate::app::Mode::Edit);
        assert_eq!(text_of(&state, state.focus()).as_deref(), Some("Q2"));
    }

    // file-io.md §3, D-104
    #[test]
    fn a_reload_that_changed_the_block_being_edited_ends_editing() {
        let mut state = opened("- a\n- b\n");
        state.update(Message::FocusNext);
        state.update(Message::EditAtEnd);
        typed(&mut state, "x");
        autosaved(&mut state, "- a\n- bx\n", ms(1000));
        state.update(found("- a\n- by\n"));
        assert_eq!(state.mode(), crate::app::Mode::Navigate);
        assert_eq!(format::serialize(state.document()), "- a\n- by\n");
    }

    // file-io.md §3, D-50, D-104
    #[test]
    fn a_reload_keeps_the_command_menu_open_listing_the_reloaded_blocks_commands() {
        let mut state = opened("- a\n");
        state.update(Message::EditAtEnd);
        state.update(Message::OpenCommandMenu {
            today: jiff::civil::date(2026, 10, 5),
        });
        state.update(Message::Menu(Menu::CommandMenu, MenuInput::Type('c')));
        let labels = |state: &State| -> Vec<String> {
            let menu = state.command_menu().expect("open");
            menu.rows
                .iter()
                .filter_map(|row| match row {
                    MenuRow::Command { label, .. } => Some((*label).to_owned()),
                    MenuRow::Divider => None,
                })
                .collect()
        };
        assert!(labels(&state).contains(&"Complete".to_owned()));
        state.update(found("- a @done\n"));
        assert_eq!(state.command_menu().map(|menu| menu.filter), Some("c"));
        assert!(!labels(&state).contains(&"Complete".to_owned()));
        state.update(Message::Menu(Menu::CommandMenu, MenuInput::Dismiss));
        assert_eq!(shown(&state), "a/c|", "the menu's text is typed");
    }

    // file-io.md §3, FR-TAG-03, D-104
    #[test]
    fn a_reload_keeps_the_date_prompt_on_its_block_while_it_is_there() {
        let mut state = opened("- a\n");
        let open = |state: &mut State| {
            state.update(Message::OpenDatePrompt {
                tag: crate::document::DateTag::Due,
                today: jiff::civil::date(2026, 10, 5),
            });
        };
        open(&mut state);
        state.update(found("- b\n- a\n"));
        for c in "2026-10-09".chars() {
            state.update(Message::DatePrompt(DateInput::Edit(TextEdit::Insert(c))));
        }
        state.update(Message::DatePrompt(DateInput::Apply));
        assert_eq!(
            format::serialize(state.document()),
            "- b\n- a @due(2026-10-09)\n"
        );
        open(&mut state);
        autosaved(&mut state, "- b\n- a @due(2026-10-09)\n", ms(1000));
        state.update(found("- b\n"));
        assert_eq!(state.date_prompt(), None, "a is gone");
    }

    // file-io.md §3, FR-NAV-05, D-104
    #[test]
    fn a_reload_lists_the_reloaded_blocks_in_go_to_symbol_keeping_the_query_and_highlight() {
        let mut state = opened("A\nB\nAB\n");
        state.update(Message::OpenGoToSymbol(Symbols::All));
        state.update(Message::Menu(Menu::GoToSymbol, MenuInput::Type('b')));
        state.update(Message::Menu(Menu::GoToSymbol, MenuInput::Next));
        state.update(found("X\nA\nB\nAB\nBX\n"));
        let view = state.go_to_symbol().expect("open");
        assert_eq!(view.query, "b");
        assert_eq!(view.entries.len(), 3, "BX is listed");
        state.update(Message::Menu(Menu::GoToSymbol, MenuInput::Choose));
        assert_eq!(text_of(&state, state.focus()).as_deref(), Some("AB"));
    }

    // file-io.md §3, FR-NAV-05, D-80, D-104, N-PERF-02
    #[test]
    fn a_reload_lists_the_reloaded_outline_in_finders_and_views_as_walked_anew() {
        let outline = |top: &str| {
            let mut text = format!("{top}:\n");
            for project in 0..40 {
                writeln!(text, "\tP{project}:").unwrap();
                for task in 0..50 {
                    writeln!(text, "\t\t- {project} {task} @star").unwrap();
                }
            }
            text
        };
        let lists = |state: &State| {
            (
                state.block_candidates(&[]),
                state.matching_rows(&state.pane, SpecialView::Starred),
            )
        };
        let mut state = opened(&outline("Top"));
        let _ = lists(&state);
        state.update(Message::OpenGoToSymbol(Symbols::All));
        state.update(found(&outline("Renamed")));
        assert_eq!(state.go_to_symbol().expect("open").entries.len(), 2041);
        let (candidates, entries) = lists(&state);
        assert_eq!(entries.len(), 2000);
        assert!(
            entries
                .iter()
                .all(|row| row.path.first().map(|top| &**top) == Some("Renamed")),
            "every path starts from the reloaded top block"
        );
        state.candidates_walks = Walks::default();
        state.entries_walks = Walks::default();
        assert_eq!((candidates, entries), lists(&state));
    }

    // file-io.md §3, FR-OPS-05, D-104
    #[test]
    fn a_reload_lists_the_reloaded_blocks_in_move_to_and_closes_it_once_nothing_moves() {
        let mut state = opened("A\nB\nC\n");
        state.update(Message::OpenMoveTo);
        state.update(Message::Menu(Menu::MoveTo, MenuInput::Type('c')));
        state.update(found("X\nA\nB\nC\nCX\n"));
        assert_eq!(state.move_to().expect("open").entries.len(), 2);
        state.update(Message::Menu(Menu::MoveTo, MenuInput::Choose));
        assert_eq!(format::serialize(state.document()), "X\nB\nC\n\tA\nCX\n");
        let mut state = opened("A:\nB:\nC:\n");
        state.update(Message::OpenMoveTo);
        state.update(Message::Menu(Menu::MoveTo, MenuInput::Next));
        state.update(Message::Menu(Menu::MoveTo, MenuInput::Next));
        state.update(found("X:\nA:\nB:\nC:\n"));
        state.update(Message::Menu(Menu::MoveTo, MenuInput::Choose));
        assert_eq!(
            format::serialize(state.document()),
            "X:\nB:\nC:\n\tA:\n",
            "C stays highlighted"
        );
        let mut state = opened("A:\nB:\n");
        state.update(Message::OpenMoveTo);
        state.update(found("X:\nA:\nB:\n"));
        state.update(Message::Menu(Menu::MoveTo, MenuInput::Choose));
        assert_eq!(
            format::serialize(state.document()),
            "X:\nB:\nA:\n",
            "Home stays highlighted"
        );
        let mut state = opened("Inbox:\n\t- Milk\nShop:\n");
        state.update(Message::FocusNext);
        state.update(Message::OpenMoveTo);
        state.update(found("Inbox:\n\t- Oat milk\nShop:\n"));
        assert_eq!(state.move_to(), None, "Milk is gone, so Inbox is not moved");
        let mut state = opened("- a\n- b\n- c\n");
        state.update(Message::Select(SelectionChange::Toggle));
        state.update(Message::FocusNext);
        state.update(Message::Select(SelectionChange::Toggle));
        state.update(Message::OpenMoveTo);
        state.update(found("- a\n- x\n- c\n"));
        state.update(Message::Menu(Menu::MoveTo, MenuInput::Type('c')));
        state.update(Message::Menu(Menu::MoveTo, MenuInput::Choose));
        assert_eq!(
            format::serialize(state.document()),
            "- x\n- c\n\t- a\n",
            "a is still selected"
        );
    }

    // file-io.md §3, FR-KEY-06, D-104
    #[test]
    fn a_reload_lists_the_context_menus_commands_for_the_reloaded_blocks() {
        let mut state = opened("- A\n");
        state.update(Message::OpenContextMenu);
        let completes =
            |state: &State| {
                state.context_menu().expect("open").rows.iter().any(
                    |row| matches!(row, MenuRow::Command { label, .. } if *label == "Complete"),
                )
            };
        assert!(completes(&state));
        state.update(found("- A @done\n"));
        assert!(!completes(&state), "A is done");
        let mut state = opened("Inbox:\n\t- Milk\n");
        state.update(Message::FocusNext);
        state.update(Message::OpenContextMenu);
        state.update(found("Inbox:\n\t- Oat milk\n"));
        assert_eq!(
            state.context_menu(),
            None,
            "Milk is gone, so Inbox is not offered"
        );
        state.update(Message::FocusNext);
        state.update(Message::OpenPaneMenu(None));
        state.update(found("Inbox:\n\t- Milk\n"));
        assert!(state.context_menu().is_some(), "the pane is still there");
    }

    // file-io.md §3, D-50, D-104
    #[test]
    fn a_reload_resumes_the_edit_a_menu_command_would_resume_on_its_block() {
        let mut state = opened("- a\n");
        state.update(Message::EditAtEnd);
        state.update(Message::OpenCommandMenu {
            today: jiff::civil::date(2026, 10, 5),
        });
        for c in "edit node".chars() {
            state.update(Message::Menu(Menu::CommandMenu, MenuInput::Type(c)));
        }
        state.update(Message::Menu(Menu::CommandMenu, MenuInput::Choose));
        assert!(state.node_editor().is_some());
        state.update(found("- b\n- a\n"));
        assert_eq!(state.node_editor(), None, "its draft is of the old block");
        assert_eq!(shown(&state), "a|");
        assert_eq!(text_of(&state, state.focus()).as_deref(), Some("a"));
    }

    // file-io.md §3, editing.md §2.3, D-104, D-105
    #[test]
    fn a_reload_keeps_holding_the_edited_block_a_menu_command_hid() {
        let mut state = opened("- a\n- b\n");
        state.update(Message::ToggleShowDone);
        state.update(Message::EditAtEnd);
        state.update(Message::OpenCommandMenu {
            today: jiff::civil::date(2026, 10, 5),
        });
        for c in "complete".chars() {
            state.update(Message::Menu(Menu::CommandMenu, MenuInput::Type(c)));
        }
        state.update(Message::Menu(Menu::CommandMenu, MenuInput::Choose));
        autosaved(&mut state, "- a @done(2026-10-05)\n- b\n", ms(1000));
        state.update(found("- z\n- a @done(2026-10-05)\n- b\n"));
        let rows = |state: &State| -> Vec<String> {
            state
                .rows()
                .iter()
                .map(|row| row.text.to_string())
                .collect()
        };
        assert_eq!(rows(&state), ["a", "z", "b"], "still held where it was");
        assert_eq!(shown(&state), "a|");
        typed(&mut state, "x");
        assert_eq!(
            format::serialize(state.document()),
            "- z\n- ax @done(2026-10-05)\n- b\n"
        );
        state.update(Message::StopEditing);
        assert_eq!(rows(&state), ["z", "b"], "it leaves once editing ends");
    }

    // file-io.md §3, FR-ATT-03, D-104
    #[test]
    fn a_reload_keeps_the_attachments_panel_on_its_block_or_closes_it() {
        use crate::{app::AttachmentsInput, document::Attachment};
        let mut state = opened("- a\n\tNOTE @hidden\n\t\told\n");
        state.update(Message::Attachments(AttachmentsInput::Open(
            Attachment::Note,
        )));
        state.update(found("- z\n- a\n\tNOTE @hidden\n\t\tnew\n"));
        let view = state.attachments_panel().unwrap();
        assert_eq!((&*view.title, view.note), ("a", vec!["new".to_owned()]));
        state.update(found("- y\n"));
        assert_eq!(state.attachments_panel(), None, "its block is gone");
    }

    // FR-ATT-05, attachments.md §3, D-110
    #[test]
    fn a_reload_keeps_the_selected_and_reworded_comment_on_the_same_comment() {
        use crate::{
            app::{AttachmentsInput, PanelItem},
            document::Attachment,
        };
        const BEFORE: &str =
            "- a\n\tCOMMENTS @hidden\n\t\t[2026-10-09 09:00] x\n\t\t[2026-10-08 09:00] y\n";
        const NEWER: &str = "- a\n\tCOMMENTS @hidden\n\t\t[2026-10-09 10:00] new\n\t\t[2026-10-09 09:00] x\n\t\t[2026-10-08 09:00] y\n";
        let mut state = opened(BEFORE);
        for input in [
            AttachmentsInput::Open(Attachment::Comments),
            AttachmentsInput::Select(1),
            AttachmentsInput::Edit,
            AttachmentsInput::Text(TextEdit::Insert('!')),
        ] {
            state.update(Message::Attachments(input));
        }
        state.update(found(NEWER));
        assert_eq!(
            state.attachments_panel().unwrap().selected,
            Some(PanelItem::Comment(2)),
            "the selection stays on y"
        );
        state.update(Message::Attachments(AttachmentsInput::Accept(
            jiff::civil::DateTime::default(),
        )));
        assert_eq!(
            state.attachments_panel().unwrap().comments,
            [
                "[2026-10-09 10:00] new",
                "[2026-10-09 09:00] x",
                "[2026-10-08 09:00] y!"
            ],
            "the rewording lands on y"
        );
        let mut state = opened(BEFORE);
        for input in [
            AttachmentsInput::Open(Attachment::Comments),
            AttachmentsInput::Select(1),
            AttachmentsInput::Edit,
        ] {
            state.update(Message::Attachments(input));
        }
        let gone = "- a\n\tCOMMENTS @hidden\n\t\t[2026-10-08 09:00] z\n\t\t[2026-10-09 09:00] x\n";
        state.update(found(gone));
        let view = state.attachments_panel().unwrap();
        assert_eq!(
            view.draft, None,
            "a rewording of a comment that is gone ends"
        );
        assert_eq!(view.selected, Some(PanelItem::Comment(1)), "where it was");
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
        state.update(Message::NodeEditor(crate::app::NodeInput::Open));
        state.update(Message::NodeEditor(crate::app::NodeInput::Cancel));
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
        state.update(saved("- a @done(2026-10-05)\n"));
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
        state.update(saved("mine\n"));
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
        assert_eq!(state.left_in(), None, "until the copy is written");
        assert_eq!(state.update(copied()), [Effect::Quit]);
        assert!(!state.in_conflict());
        assert_eq!(
            state.left_in(),
            Some(Path::new("/backups/copy.taskpaper")),
            "the quit says where the local version went"
        );
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
