//! Edit mode: the focused block's text edited in place with the text keys, written to the
//! document as it is typed, refused where it would type a tag, undone as one step and repaired
//! when editing ends, with the command menu that `/` opens
//! ([editing.md §1–2](../../../docs/specs/editing.md#1-modes), FR-MODE-01–03, OQ-24, OQ-25,
//! OQ-27, D-44, D-50).

mod command_menu;

use std::{mem, ops::Range};

use command_menu::{Closed, CommandMenu};
pub use command_menu::{CommandMenuView, MenuInput, MenuRow};
use jiff::civil::Date;

use super::{
    Access, Effect, Notice, Restore, State,
    commands::{Command, Inputs, node_commands},
    structure::NewBlock,
    text_field::{TextEdit, TextField},
};
use crate::document::{
    Block, BlockKind, Document, ItemId, ItemKind, Op, Transaction, blocks,
    format::{self, LineParts, Typed, canonical, tags},
};

/// Whether keys move between blocks or edit one block's text (FR-MODE-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Keys move the focus and run block commands.
    Navigate,
    /// Keys edit the focused block's text.
    Edit,
}

/// The text being edited, for a frontend to draw in place of the block's text (FR-MODE-02).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Editor<'a> {
    /// The block being edited, which is the focused block.
    pub id: ItemId,
    /// The text as typed so far: the block's text without a task's dash and the tags.
    pub text: &'a str,
    /// The caret's byte offset into `text`.
    pub caret: usize,
    /// The selected bytes of `text`, if any.
    pub selection: Option<Range<usize>>,
}

/// Where editing resumes after a command of the command menu: the block and the caret's column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Resume {
    id: ItemId,
    column: usize,
}

/// One edit session: the block's line as it started and as it is written now, its parts and the
/// text field, with the session's own undo history of the text.
#[derive(Debug, Clone)]
pub(super) struct Session {
    /// The block's line.
    id: ItemId,
    /// The applied ops that created the block for this session, undone with it as one step; empty
    /// for a block that was there before.
    created: Vec<Op>,
    /// The focus before the session's step.
    before: Option<ItemId>,
    /// The line's text when the session started, written back as it was while the text and
    /// parts are as they were, so its mid-line tags stay where they are.
    original: String,
    /// The text the session has written over `original`, if any.
    written: Option<String>,
    /// The line's parts, which change as the text gives the line a task's type.
    parts: LineParts,
    field: TextField,
    undo: Vec<(LineParts, TextField)>,
    redo: Vec<(LineParts, TextField)>,
    /// Whether the latest change typed a character, so that the next one typed joins its undo
    /// step.
    typing: bool,
    /// The command menu, while `/` has it open, with the local date its date commands write
    /// (D-50).
    menu: Option<(CommandMenu, Date)>,
}

impl Session {
    /// A session editing the line `id`, which reads `line`, with the caret at the end of its
    /// text, before a project's `:` so that typing there adds to the name.
    fn new(id: ItemId, line: &str) -> Self {
        let (parts, text) = LineParts::split(line);
        let caret = text.len() - usize::from(format::kind(line) == ItemKind::Project);
        Self {
            id,
            created: Vec::new(),
            before: Some(id),
            original: line.to_owned(),
            written: None,
            parts,
            field: TextField::with_caret(text, caret),
            undo: Vec::new(),
            redo: Vec::new(),
            typing: false,
            menu: None,
        }
    }

    /// Applies `edit` to the text, or returns `false` and changes nothing when it would type a
    /// tag (OQ-25). A character that breaks a line is not typed, since it would split the line
    /// when the file is read back. Text that makes the line a task takes the task's syntax, so
    /// that its dash shows as the marker (OQ-27).
    fn edit(&mut self, edit: TextEdit) -> bool {
        if let TextEdit::Insert(c) = edit
            && format::breaks_line(c)
        {
            return true;
        }
        let field = self.field.edited(edit);
        if field.text() == self.field.text() {
            self.field = field;
            self.typing = false;
            return true;
        }
        if tags::adds_tag(self.field.text(), field.text()) {
            return false;
        }
        let typing = matches!(edit, TextEdit::Insert(_));
        if !(typing && self.typing) {
            self.undo.push((self.parts.clone(), self.field.clone()));
        }
        self.redo.clear();
        self.typing = typing;
        self.field = match self.parts.as_task(field.text()) {
            Some((parts, text)) => {
                let cut = field.text().len() - text.len();
                self.parts = parts;
                TextField::with_caret(text, field.caret().saturating_sub(cut))
            }
            None => field,
        };
        true
    }

    /// Whether `Backspace` meets nothing but the start of the text, nothing selected.
    fn at_start(&self) -> bool {
        self.field.caret() == 0 && self.field.selection().is_none()
    }

    /// Takes off the task's dash that `Backspace` at the start of the text deletes, as one change
    /// to undo, and returns whether it did (OQ-27).
    fn unmark(&mut self) -> bool {
        if !self.at_start() {
            return false;
        }
        let Some((parts, text)) = self.parts.unmarked(self.field.text()) else {
            return false;
        };
        self.change(parts, TextField::with_caret(text, 0));
        true
    }

    /// Whether the block has no name for `Backspace` at the start of the text to keep: its text
    /// is empty, or a project's lone `:`.
    fn nameless(&self) -> bool {
        self.at_start() && matches!(self.field.text(), "" | ":")
    }

    /// Puts `line`, which holds no line break, in place of the selection or at the caret as one
    /// change to undo, leaving out each `@` that would start a tag, as typing it would (OQ-25);
    /// returns whether it left one out.
    fn paste(&mut self, line: &str) -> bool {
        let steps = self.undo.len();
        self.typing = false;
        let mut refused = false;
        for c in line.chars() {
            refused |= !self.edit(TextEdit::Insert(c));
        }
        self.undo.truncate(steps + 1);
        self.typing = false;
        refused
    }

    /// Takes the text back to before its latest change, or with `undo` false, replays the
    /// latest change undone.
    fn step(&mut self, undo: bool) {
        let (from, to) = if undo {
            (&mut self.undo, &mut self.redo)
        } else {
            (&mut self.redo, &mut self.undo)
        };
        if let Some((parts, field)) = from.pop() {
            to.push((
                mem::replace(&mut self.parts, parts),
                mem::replace(&mut self.field, field),
            ));
            self.typing = false;
        }
    }

    /// Gives the text the type `typed` as one change to undo, keeping the caret where it was
    /// (editing.md §2.3).
    fn retype(&mut self, typed: Typed) {
        let (parts, text) = self.parts.retyped(typed, self.field.text());
        let field = TextField::with_caret(text, self.field.caret());
        self.change(parts, field);
    }

    /// Makes `parts` and `field` the session's as one change to undo.
    fn change(&mut self, parts: LineParts, field: TextField) {
        self.undo.push((
            mem::replace(&mut self.parts, parts),
            mem::replace(&mut self.field, field),
        ));
        self.redo.clear();
        self.typing = false;
    }

    /// The line the text and parts make: the line as it was while they are as they were.
    fn line(&self) -> String {
        let (parts, text) = LineParts::split(&self.original);
        if parts == self.parts && text == self.field.text() {
            self.original.clone()
        } else {
            self.parts.line(self.field.text())
        }
    }

    /// The op that wrote the session's text over the line, if it changed it.
    fn applied(&self) -> Option<Op> {
        self.written
            .clone()
            .filter(|after| *after != self.original)
            .map(|after| Op::SetText {
                id: self.id,
                before: self.original.clone(),
                after,
            })
    }
}

impl State {
    /// Whether keys navigate or edit the focused block.
    #[must_use]
    pub fn mode(&self) -> Mode {
        if self.edit.is_some() {
            Mode::Edit
        } else {
            Mode::Navigate
        }
    }

    /// The text being edited, or `None` in navigate mode.
    #[must_use]
    pub fn editor(&self) -> Option<Editor<'_>> {
        let session = self.edit.as_ref()?;
        Some(Editor {
            id: session.id,
            text: session.field.text(),
            caret: session.field.caret(),
            selection: session.field.selection(),
        })
    }

    /// Starts editing the focused block's text with the caret at its end, selecting nothing; a
    /// read-only file refuses with a notice.
    pub(super) fn start_editing(&mut self) {
        let Some((block, _)) = self.focused() else {
            return;
        };
        if self.edit.is_some() {
            return;
        }
        if self.access() == Access::ReadOnly {
            self.notify(Notice::ReadOnly);
            return;
        }
        self.edit = self.session(&block);
        self.clear_selection();
    }

    /// A session editing `block` with the caret at the end of its text.
    fn session(&self, block: &Block) -> Option<Session> {
        let line = self.document.get(block.id())?.text();
        Some(Session::new(block.id(), line))
    }

    /// Starts editing the new line `id`, which the applied `created` ops put in while the focus
    /// was on `before`, with an empty text (FR-MODE-04).
    pub(super) fn edit_new_block(&mut self, id: ItemId, created: Vec<Op>, before: Option<ItemId>) {
        if let Some(session) =
            blocks::block(&self.document, id).and_then(|block| self.session(&block))
        {
            self.edit = Some(Session {
                created,
                before,
                ..session
            });
            self.clear_selection();
        }
    }

    /// Applies `edit` to the text being edited and writes it to the document, or shows why a
    /// tag cannot be typed (OQ-25). `Backspace` at the start of the text takes a task's dash off,
    /// and in a block without a name deletes the block instead (editing.md §2, OQ-27).
    pub(super) fn edit_text(&mut self, edit: TextEdit) {
        let Some(session) = &mut self.edit else {
            return;
        };
        let backspace = edit == TextEdit::DeleteBackward;
        if backspace && session.unmark() {
            self.write(false);
        } else if backspace && session.nameless() {
            self.delete_empty_block();
        } else if session.edit(edit) {
            self.write(false);
        } else {
            self.notify(Notice::TypedTag);
        }
    }

    /// Pastes `line`, which holds no line break, into the text being edited and writes it to the
    /// document, saying so when it left out an `@` that would start a tag (editing.md §2, OQ-25).
    pub(super) fn paste_text(&mut self, line: &str) {
        if self
            .edit
            .as_mut()
            .is_some_and(|session| session.paste(line))
        {
            self.notify(Notice::TypedTag);
        }
        self.write(false);
    }

    /// The command menu, while it is open (D-50).
    #[must_use]
    pub fn command_menu(&self) -> Option<CommandMenuView<'_>> {
        self.edit
            .as_ref()?
            .menu
            .as_ref()
            .map(|(menu, _)| menu.view())
    }

    /// `/` in edit mode: opens the command menu with the commands for the edited block, `today`
    /// being the local date they write (editing.md §2.3).
    pub(super) fn open_command_menu(&mut self, today: Date) {
        let link = !self.open_url().is_empty();
        let Some((session, block)) = self.edit.as_mut().and_then(|session| {
            let block = blocks::block(&self.document, session.id)?;
            Some((session, block))
        }) else {
            return;
        };
        session.menu = Some((CommandMenu::new(node_commands(&block, link)), today));
    }

    /// Applies `input` to the open command menu, and once it closes, runs the chosen command or
    /// types what the menu took as text. A type change is a change of the session. Any other
    /// command ends the session, runs as a
    /// step of its own and editing resumes on the block at the caret's column, once a prompt or
    /// popup it opened closes (editing.md §2.3).
    pub(super) fn command_menu_input(&mut self, input: MenuInput) -> Vec<Effect> {
        let Some(session) = &mut self.edit else {
            return Vec::new();
        };
        let Some((closed, today)) = session
            .menu
            .as_mut()
            .and_then(|(menu, today)| Some((menu.input(input)?, *today)))
        else {
            return Vec::new();
        };
        session.menu = None;
        match closed {
            Closed::Run(Command::Retype(typed)) => {
                session.retype(typed);
                self.write(false);
            }
            Closed::Run(command) => {
                // No command of the menu pages, so a page needs no height.
                let inputs = Inputs {
                    today,
                    page_rows: 0,
                };
                self.suspend_editing();
                let effects = command
                    .message(&inputs)
                    .map(|message| self.update(message))
                    .unwrap_or_default();
                self.resume_editing();
                return effects;
            }
            Closed::Literal(literal) => {
                for c in literal.chars() {
                    self.edit_text(TextEdit::Insert(c));
                }
            }
            Closed::Erased => {}
        }
        Vec::new()
    }

    /// `Ctrl+T` in edit mode: gives the block the next type of project, section, task and note,
    /// and a note the first, as a change of the session with the caret kept (D-50).
    pub(super) fn cycle_type(&mut self) {
        let Some((session, block)) = self.edit.as_mut().and_then(|session| {
            let block = blocks::block(&self.document, session.id)?;
            Some((session, block))
        }) else {
            return;
        };
        let next = match block.kind() {
            BlockKind::Project => Typed::Section,
            BlockKind::Text if block.section() => Typed::Task,
            BlockKind::Task => Typed::Text,
            BlockKind::Text => Typed::Project,
        };
        session.retype(next);
        self.write(false);
    }

    /// Ends the edit session, if any, so that a command runs as a step of its own, keeping where
    /// editing resumes once the command is done (D-50).
    pub(super) fn suspend_editing(&mut self) {
        if let Some(session) = &self.edit {
            self.resume = Some(Resume {
                id: session.id,
                column: session.field.column(),
            });
            self.stop_editing();
        }
    }

    /// Edits the block that [`State::suspend_editing`] left again, with the caret at the column
    /// it had, once no prompt or popup is open; a block that is gone is not edited.
    pub(super) fn resume_editing(&mut self) {
        if self.date_prompt.is_some() || self.details {
            return;
        }
        let Some(resume) = self.resume.take() else {
            return;
        };
        if blocks::block(&self.document, resume.id).is_none() {
            return;
        }
        self.focus = Some(resume.id);
        self.start_editing();
        if let Some(session) = &mut self.edit {
            session.field = TextField::at_column(session.field.text().to_owned(), resume.column);
        }
    }

    /// `Enter` in edit mode: ends editing and adds the next block (editing.md §2–3, D-43).
    pub(super) fn new_next(&mut self) {
        self.stop_editing();
        self.add_block(NewBlock::Next, None);
    }

    /// `↑`/`↓` in edit mode: ends editing and edits the visible block above or below, with the
    /// caret as many graphemes from the start as it was, or where editing starts on a shorter
    /// text, before a project's `:`; nothing happens without such a block (editing.md §2, D-43).
    pub(super) fn edit_adjacent(&mut self, down: bool) {
        let Some(session) = &self.edit else {
            return;
        };
        let column = session.field.column();
        let rows = self.rows();
        let at = rows.iter().position(|row| row.id == session.id);
        let target = if down {
            at.and_then(|at| rows.get(at + 1))
        } else {
            at.and_then(|at| at.checked_sub(1))
                .and_then(|above| rows.get(above))
        };
        let Some(target) = target.map(|row| row.id) else {
            return;
        };
        self.stop_editing();
        self.focus = Some(target);
        self.start_editing();
        if let Some(session) = &mut self.edit {
            let column = column.min(session.field.column());
            session.field = TextField::at_column(session.field.text().to_owned(), column);
        }
    }

    /// Takes the text being edited back to before its latest change.
    pub(super) fn undo_typing(&mut self) {
        if let Some(session) = &mut self.edit {
            session.step(true);
        }
        self.write(false);
    }

    /// Replays the latest change to the text being edited that was undone.
    pub(super) fn redo_typing(&mut self) {
        if let Some(session) = &mut self.edit {
            session.step(false);
        }
        self.write(false);
    }

    /// Ends editing: writes the line repaired where it breaks canonical-outline rule 2 or 4 and
    /// records the whole session, with the block's creation, as one step to undo
    /// (file-format.md §3.1, FR-OPS-08).
    pub(super) fn stop_editing(&mut self) {
        self.write(true);
        let Some(session) = self.edit.take() else {
            return;
        };
        let before = Restore {
            focus: session.before,
        };
        let after = Restore {
            focus: Some(session.id),
        };
        let ops = session
            .created
            .iter()
            .cloned()
            .chain(session.applied())
            .collect();
        self.edits.record(Transaction::new(ops, before, after));
        self.focus = Some(session.id);
    }

    /// Records the creation of the block being edited as a step of its own, so that a step
    /// committed while it is edited, which may move it, comes after it in the history and is
    /// undone before it. The text is recorded when editing ends.
    pub(super) fn record_edit_so_far(&mut self) {
        let Some(session) = self.edit.as_mut() else {
            return;
        };
        let ops = mem::take(&mut session.created);
        if !ops.is_empty() {
            let before = Restore {
                focus: session.before.replace(session.id),
            };
            let after = Restore {
                focus: Some(session.id),
            };
            self.edits.record(Transaction::new(ops, before, after));
        }
    }

    /// Ends a session that created its block by taking the block out again, leaving no step to
    /// undo, and returns whether it did. A session on a block that was there before, or whose
    /// block can no longer be taken out that way, is left going.
    pub(super) fn discard_new_block(&mut self) -> bool {
        let Some(session) = self
            .edit
            .as_ref()
            .filter(|session| !session.created.is_empty())
        else {
            return false;
        };
        let ops = session.created.iter().cloned().chain(session.applied());
        let mut undo = Transaction::new(ops.collect(), (), ()).inverse();
        if self.document.apply(&mut undo).is_err() {
            return false;
        }
        self.edit = None;
        true
    }

    /// Whether saving waits because the line being edited breaks rule 2 or 4 for now, or is one
    /// of the empty lines ending the document that break rule 3: typing may repair it, and ending
    /// the session or the save after it does (file-format.md §3.1).
    pub(super) fn editing_holds_save(&self) -> bool {
        let only_line = only_line(&self.document);
        self.edit.as_ref().is_some_and(|session| {
            self.document.get(session.id).is_some_and(|item| {
                canonical::needs_repair(item.text(), !item.children().is_empty(), only_line)
                    || item.text().is_empty()
                        && canonical::trailing_blanks(&self.document).contains(&session.id)
            })
        })
    }

    /// Writes the text being edited to the block's line, outside the undo history, so that
    /// saving and every view see it at once; with `repair`, repaired where it breaks
    /// canonical-outline rule 2 or 4. A line that already reads so is left alone.
    fn write(&mut self, repair: bool) {
        let only_line = only_line(&self.document);
        let Some((session, item)) = self.edit.as_mut().and_then(|session| {
            let item = self.document.get(session.id)?;
            Some((session, item))
        }) else {
            return;
        };
        let line = session.line();
        let after = if repair {
            canonical::repaired(&line, !item.children().is_empty(), only_line)
        } else {
            line
        };
        if item.text() == after {
            return;
        }
        let op = Op::SetText {
            id: session.id,
            before: item.text().to_owned(),
            after: after.clone(),
        };
        self.document
            .apply(&mut Transaction::new(vec![op], (), ()))
            .expect("the op is built from the line as it is");
        session.written = Some(after);
    }
}

/// Whether `document` holds a single line, which keeps its leading whitespace (file-format.md §2).
fn only_line(document: &Document) -> bool {
    document.descendants(document.root()).nth(1).is_none()
}

#[cfg(test)]
mod tests {
    use std::{
        path::{Path, PathBuf},
        time::Duration,
    };

    use super::*;
    use crate::{
        app::{DateInput, Disk, Effect, Message, Motion, Version},
        document::format,
    };

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

    fn typed(state: &mut State, text: &str) {
        for c in text.chars() {
            state.update(Message::Text(TextEdit::Insert(c)));
        }
    }

    fn press(state: &mut State, edit: TextEdit, times: usize) {
        for _ in 0..times {
            state.update(Message::Text(edit));
        }
    }

    /// The text being edited with `|` at the caret.
    fn shown(state: &State) -> String {
        let editor = state.editor().unwrap();
        let mut text = editor.text.to_owned();
        text.insert(editor.caret, '|');
        text
    }

    // FR-MODE-01, FR-MODE-02
    #[test]
    fn enter_and_i_edit_the_focused_block_with_the_caret_at_either_end() {
        let mut state = open("- a\n- bc @x", 1);
        assert_eq!(state.mode(), Mode::Navigate);
        assert_eq!(state.editor(), None);
        state.update(Message::EditAtEnd);
        assert_eq!(state.mode(), Mode::Edit);
        assert_eq!(shown(&state), "bc|", "tags are not edited");
        assert_eq!(state.editor().unwrap().id, state.focus().unwrap());
        go(&mut state, Motion::Start);
        state.update(Message::EditAtEnd);
        assert_eq!(shown(&state), "|bc", "already editing");
    }

    // FR-MODE-01
    #[test]
    fn esc_returns_to_navigate_mode_on_the_same_block() {
        let mut state = open("- a\n- b", 1);
        let focus = state.focus();
        state.update(Message::EditAtEnd);
        state.update(Message::StopEditing);
        assert_eq!(state.mode(), Mode::Navigate);
        assert_eq!(state.focus(), focus);
        assert_eq!(
            state.document().revision(),
            0,
            "nothing typed, nothing changed"
        );
    }

    // FR-MODE-03, editing.md §2
    #[test]
    fn typing_changes_the_document_at_once() {
        let mut state = open("- Milk", 0);
        state.update(Message::EditAtEnd);
        typed(&mut state, "s");
        assert_eq!(texts(&state), ["- Milks"]);
        assert_eq!(state.rows()[0].text, "Milks");
        press(&mut state, TextEdit::DeleteBackward, 2);
        assert_eq!(texts(&state), ["- Mil"]);
        state.update(Message::Text(TextEdit::Move {
            motion: Motion::Start,
            select: false,
        }));
        press(&mut state, TextEdit::DeleteForward, 1);
        assert_eq!(shown(&state), "|il");
        assert_eq!(texts(&state), ["- il"]);
    }

    // editing.md §2, OQ-24
    #[test]
    fn tags_stay_on_the_line_and_move_after_the_text() {
        let mut state = open("- Call @due(2026-10-09) Ana @home", 0);
        state.update(Message::EditAtEnd);
        assert_eq!(shown(&state), "Call Ana|");
        typed(&mut state, "!");
        assert_eq!(texts(&state), ["- Call Ana! @due(2026-10-09) @home"]);
        press(&mut state, TextEdit::DeleteBackward, 1);
        assert_eq!(
            texts(&state),
            ["- Call @due(2026-10-09) Ana @home"],
            "the text as it started gives the line back"
        );
    }

    // OQ-25
    #[test]
    fn an_at_sign_starting_a_word_is_refused_with_a_hint() {
        let mut state = open("note", 0);
        state.update(Message::EditAtEnd);
        typed(&mut state, " @");
        assert_eq!(shown(&state), "note |");
        assert_eq!(state.notice(), Some(&Notice::TypedTag));
        typed(&mut state, "me@example.com");
        assert_eq!(texts(&state), ["note me@example.com"]);
        state.update(Message::Text(TextEdit::Move {
            motion: Motion::WordLeft,
            select: false,
        }));
        assert_eq!(shown(&state), "note me@|example.com");
        state.update(Message::Text(TextEdit::Move {
            motion: Motion::Start,
            select: false,
        }));
        state.update(Message::Text(TextEdit::Move {
            motion: Motion::WordRight,
            select: true,
        }));
        press(&mut state, TextEdit::DeleteForward, 1);
        assert_eq!(shown(&state), "| me@example.com");
        typed(&mut state, "a");
        assert_eq!(texts(&state), ["a me@example.com"]);
    }

    // OQ-27
    #[test]
    fn the_type_follows_the_text_live() {
        let mut state = open("Questions @x", 0);
        state.update(Message::EditAtEnd);
        typed(&mut state, ":");
        assert_eq!(state.rows()[0].kind, BlockKind::Project);
        assert_eq!(shown(&state), "Questions:|", "the colon stays text");
        typed(&mut state, " why");
        assert_eq!(state.rows()[0].kind, BlockKind::Text);
        state.update(Message::Text(TextEdit::SelectAll));
        typed(&mut state, "- buy");
        assert_eq!(state.rows()[0].kind, BlockKind::Task);
        assert_eq!(shown(&state), "buy|", "the dash becomes the marker");
        assert_eq!(texts(&state), ["- buy @x"]);
    }

    // OQ-27
    #[test]
    fn a_projects_colon_is_text_with_the_caret_starting_before_it() {
        let mut state = open("Inbox: @x", 0);
        state.update(Message::EditAtEnd);
        assert_eq!(shown(&state), "Inbox|:");
        typed(&mut state, "es");
        assert_eq!(texts(&state), ["Inboxes: @x"]);
        go(&mut state, Motion::End);
        typed(&mut state, ":");
        assert_eq!(texts(&state), ["Inboxes:: @x"], "a colon typed is text");
        let mut state = open("Inbox:", 0);
        edit_from_start(&mut state);
        typed(&mut state, "My ");
        assert_eq!(texts(&state), ["My Inbox:"]);
        go(&mut state, Motion::Start);
        typed(&mut state, "- ");
        assert_eq!(shown(&state), "|My Inbox:");
        assert_eq!(state.rows()[0].kind, BlockKind::Task);
        assert_eq!(texts(&state), ["- My Inbox:"]);
    }

    // OQ-27
    #[test]
    fn deleting_the_dash_or_the_colon_takes_the_type_off() {
        let mut state = open("- buy @x", 0);
        edit_from_start(&mut state);
        press(&mut state, TextEdit::DeleteBackward, 1);
        assert_eq!(texts(&state), ["buy @x"]);
        assert_eq!(state.rows()[0].kind, BlockKind::Text);
        assert_eq!(shown(&state), "|buy");
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- buy @x"], "a change of the session");
        let mut state = open("Inbox: @x", 0);
        state.update(Message::EditAtEnd);
        press(&mut state, TextEdit::DeleteForward, 1);
        assert_eq!(texts(&state), ["Inbox @x"], "the colon is text");
        assert_eq!(state.rows()[0].kind, BlockKind::Text);
        assert_eq!(shown(&state), "Inbox|");
        let mut state = open("- Inbox:", 0);
        edit_from_start(&mut state);
        press(&mut state, TextEdit::DeleteBackward, 1);
        assert_eq!(state.rows()[0].kind, BlockKind::Project, "the text decides");
        assert_eq!(shown(&state), "|Inbox:");
    }

    // OQ-27, editing.md §2
    #[test]
    fn backspace_in_an_empty_task_takes_the_dash_off_before_deleting_the_block() {
        let mut state = open(
            "- a
- ", 1,
        );
        state.update(Message::EditAtEnd);
        press(&mut state, TextEdit::DeleteBackward, 1);
        assert_eq!(texts(&state), ["- a", ""]);
        assert_eq!(state.rows()[1].kind, BlockKind::Text);
        press(&mut state, TextEdit::DeleteBackward, 1);
        assert_eq!(texts(&state), ["- a"]);
        assert_eq!(shown(&state), "a|", "the block above is edited");
    }

    // editing.md §2
    #[test]
    fn backspace_before_the_lone_colon_of_a_project_deletes_the_block() {
        let mut state = open(
            "- a
:", 1,
        );
        state.update(Message::EditAtEnd);
        assert_eq!(shown(&state), "|:");
        press(&mut state, TextEdit::DeleteBackward, 1);
        assert_eq!(texts(&state), ["- a"]);
        let mut state = open(
            "- a
:", 1,
        );
        state.update(Message::EditAtEnd);
        go(&mut state, Motion::End);
        press(&mut state, TextEdit::DeleteBackward, 1);
        assert_eq!(texts(&state), ["- a", ""], "after it, the colon is deleted");
        let mut state = open(
            "- a
:", 1,
        );
        state.update(Message::EditAtEnd);
        state.update(Message::Text(TextEdit::SelectAll));
        press(&mut state, TextEdit::DeleteBackward, 1);
        assert_eq!(texts(&state), ["- a", ""], "a selection is deleted");
    }

    // OQ-27
    #[test]
    fn only_a_backspace_at_the_dash_with_nothing_selected_takes_it_off() {
        for (line, edit, expected) in [
            ("- ab", TextEdit::DeleteBackward, "- a"),
            ("- ab", TextEdit::DeleteForward, "- ab"),
            ("Ab:", TextEdit::DeleteBackward, "A:"),
            ("Ab:", TextEdit::DeleteForward, "Ab"),
            ("ab", TextEdit::DeleteForward, "ab"),
        ] {
            let mut state = open(line, 0);
            state.update(Message::EditAtEnd);
            press(&mut state, edit, 1);
            assert_eq!(texts(&state), [expected], "{line:?} {edit:?}");
        }
        let mut state = open("- ab", 0);
        state.update(Message::EditAtEnd);
        state.update(Message::Text(TextEdit::Move {
            motion: Motion::Start,
            select: true,
        }));
        press(&mut state, TextEdit::DeleteBackward, 1);
        assert_eq!(
            state.rows()[0].kind,
            BlockKind::Task,
            "a selection is deleted"
        );
    }

    // OQ-27, D-52
    #[test]
    fn a_section_that_becomes_a_task_is_a_section_again_as_text() {
        let mut state = open("Ideas @section", 0);
        edit_from_start(&mut state);
        typed(&mut state, "- ");
        assert_eq!(texts(&state), ["- Ideas @section"]);
        assert_eq!(state.rows()[0].kind, BlockKind::Task);
        assert!(!state.rows()[0].section, "the tag has no effect on a task");
        press(&mut state, TextEdit::DeleteBackward, 1);
        assert_eq!(texts(&state), ["Ideas @section"]);
        assert!(state.rows()[0].section);
    }

    // FR-OPS-08, editing.md §2
    #[test]
    fn a_session_is_one_undo_step_that_restores_the_focus() {
        let mut state = open("- a\n- b", 1);
        state.update(Message::EditAtEnd);
        typed(&mut state, "cd");
        press(&mut state, TextEdit::DeleteBackward, 1);
        state.update(Message::StopEditing);
        assert_eq!(texts(&state), ["- a", "- bc"]);
        state.update(Message::FocusFirst);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a", "- b"]);
        assert_eq!(state.focus(), Some(state.rows()[1].id));
        state.update(Message::FocusFirst);
        state.update(Message::Redo);
        assert_eq!(texts(&state), ["- a", "- bc"]);
        assert_eq!(state.focus(), Some(state.rows()[1].id));
        state.update(Message::Undo);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a", "- b"], "nothing before the session");
    }

    // FR-OPS-08
    #[test]
    fn a_session_that_changes_nothing_is_no_undo_step() {
        let mut state = open("- a @x b", 0);
        state.update(Message::ToggleDone {
            today: jiff::civil::date(2026, 10, 5),
        });
        state.update(Message::EditAtEnd);
        typed(&mut state, "z");
        press(&mut state, TextEdit::DeleteBackward, 1);
        state.update(Message::StopEditing);
        assert_eq!(texts(&state), ["- a @x b @done(2026-10-05)"]);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a @x b"], "undo skips the empty session");
    }

    // keybindings.md §3
    #[test]
    fn undo_in_edit_mode_takes_back_runs_of_typing_and_other_changes() {
        let mut state = open("- a", 0);
        state.update(Message::EditAtEnd);
        typed(&mut state, "bc");
        press(&mut state, TextEdit::DeleteBackward, 1);
        state.update(Message::Text(TextEdit::Move {
            motion: Motion::Start,
            select: false,
        }));
        typed(&mut state, "x");
        assert_eq!(shown(&state), "x|ab");
        state.update(Message::Undo);
        assert_eq!(shown(&state), "|ab");
        state.update(Message::Undo);
        assert_eq!(shown(&state), "abc|");
        state.update(Message::Undo);
        assert_eq!(shown(&state), "a|");
        assert_eq!(texts(&state), ["- a"]);
        state.update(Message::Undo);
        assert_eq!(shown(&state), "a|", "nothing more in the session");
        assert_eq!(state.mode(), Mode::Edit);
        state.update(Message::Redo);
        assert_eq!(shown(&state), "abc|");
        assert_eq!(texts(&state), ["- abc"]);
        typed(&mut state, "d");
        state.update(Message::Redo);
        assert_eq!(shown(&state), "abcd|", "typing drops what was undone");
    }

    // OQ-27, keybindings.md §3
    #[test]
    fn undo_in_edit_mode_takes_back_a_type_change() {
        let mut state = open("a", 0);
        edit_from_start(&mut state);
        typed(&mut state, "- ");
        assert_eq!(texts(&state), ["- a"]);
        state.update(Message::Undo);
        assert_eq!(shown(&state), "|a");
        assert_eq!(texts(&state), ["a"]);
        assert_eq!(state.rows()[0].kind, BlockKind::Text);
    }

    // file-format §3.1
    #[test]
    fn ending_the_session_repairs_leading_whitespace_and_an_empty_parent() {
        let mut state = open("a\n\t- b\nc", 0);
        state.update(Message::EditAtEnd);
        state.update(Message::Text(TextEdit::SelectAll));
        press(&mut state, TextEdit::DeleteBackward, 1);
        assert_eq!(
            texts(&state),
            ["", "- b", "c"],
            "repaired only when editing ends"
        );
        state.update(Message::StopEditing);
        assert_eq!(texts(&state), ["…", "- b", "c"]);
        state.update(Message::FocusLast);
        edit_from_start(&mut state);
        typed(&mut state, "  ");
        assert_eq!(texts(&state), ["…", "- b", "  c"]);
        state.update(Message::StopEditing);
        assert_eq!(texts(&state), ["…", "- b", "c"]);
        state.update(Message::FocusFirst);
        state.update(Message::Undo);
        assert_eq!(
            texts(&state),
            ["a", "- b", "c"],
            "repairs undo with their session"
        );
    }

    // file-format §2, §3.1
    #[test]
    fn a_one_line_document_keeps_its_leading_whitespace() {
        let mut state = open("  - Milk", 0);
        state.update(Message::EditAtEnd);
        state.update(Message::StopEditing);
        assert_eq!(texts(&state), ["  - Milk"]);
        assert_eq!(
            state.document().revision(),
            0,
            "an unchanged session changes nothing"
        );
        let (mut state, path) = editing_file("  a\n");
        typed(&mut state, "b");
        assert_eq!(
            later(&mut state, 1),
            save(&path, "  ab\n", "  a\n"),
            "saving does not wait"
        );
        state.update(Message::StopEditing);
        assert_eq!(texts(&state), ["  ab"]);
    }

    // file-format §1
    #[test]
    fn characters_that_break_a_line_are_not_typed() {
        let mut state = open("A:\n\t- a\n\t\t- c", 1);
        state.update(Message::EditAtEnd);
        for c in ['\n', '\r', '\u{C}', '\u{2029}', '\u{2B7F}'] {
            state.update(Message::Text(TextEdit::Insert(c)));
        }
        typed(&mut state, "b\u{2028}");
        assert_eq!(texts(&state), ["A:", "- ab\u{2028}", "- c"]);
    }

    // file-format §4
    #[test]
    fn a_task_emptied_before_its_tags_stays_a_task() {
        let mut state = open("n\n- a @done\nm", 1);
        state.update(Message::EditAtEnd);
        press(&mut state, TextEdit::DeleteBackward, 1);
        assert_eq!(texts(&state), ["n", "- … @done", "m"]);
        assert_eq!(state.rows().len(), 3, "no notes merged");
        assert_eq!(shown(&state), "|");
    }

    #[test]
    fn an_empty_outline_is_not_edited() {
        let mut state = open("", 0);
        state.update(Message::EditAtEnd);
        assert_eq!(state.mode(), Mode::Navigate);
    }

    // FR-BLK-07, D-52
    #[test]
    fn a_divider_is_edited_as_its_raw_text() {
        let mut state = open("- a\n--- @x", 1);
        state.update(Message::EditAtEnd);
        assert_eq!(shown(&state), "---|");
        typed(&mut state, "-");
        assert_eq!(texts(&state), ["- a", "---- @x"]);
        assert!(!state.rows()[1].divider, "four dashes are text");
    }

    // FR-NAV-03
    #[test]
    fn the_zoom_title_is_edited_in_place() {
        let mut state = open("P:\n\t- a", 0);
        state.update(Message::ZoomIn);
        state.update(Message::FocusFirst);
        state.update(Message::EditAtEnd);
        typed(&mut state, "Q");
        state.update(Message::StopEditing);
        assert_eq!(texts(&state), ["PQ:", "- a"]);
        assert_eq!(state.breadcrumbs(), ["PQ"]);
    }

    // file-io.md §1
    #[test]
    fn a_read_only_file_refuses_edit_mode() {
        let mut state = State::new(format::parse("- a"), Some(PathBuf::from("/a.taskpaper")))
            .with_access(Access::ReadOnly);
        state.update(Message::EditAtEnd);
        assert_eq!(state.mode(), Mode::Navigate);
        assert_eq!(state.notice(), Some(&Notice::ReadOnly));
    }

    #[test]
    fn text_changes_outside_edit_mode_do_nothing() {
        let mut state = open("- a", 0);
        typed(&mut state, "x");
        state.update(Message::StopEditing);
        assert_eq!(texts(&state), ["- a"]);
        assert_eq!(state.document().revision(), 0);
    }

    // FR-MODE-01
    #[test]
    fn a_navigate_command_ends_the_session_first() {
        let mut state = open("- a\n- b", 0);
        state.update(Message::EditAtEnd);
        typed(&mut state, "x");
        state.update(Message::FocusNext);
        assert_eq!(state.mode(), Mode::Navigate);
        assert_eq!(state.focus(), Some(state.rows()[1].id));
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a", "- b"]);
        state.update(Message::EditAtEnd);
        state.update(Message::Tick {
            now: Duration::from_secs(1),
        });
        assert_eq!(state.mode(), Mode::Edit, "a tick leaves it alone");
    }

    // file-io.md §3
    #[test]
    fn a_reload_ends_the_session_and_a_conflict_choice_ends_it_first() {
        let path = PathBuf::from("/notes/todo.taskpaper");
        let opened = || {
            let mut state = State::new(format::parse("- a\n"), Some(path.clone()))
                .with_version(Some(Version::of(b"- a\n")));
            state.update(Message::EditAtEnd);
            state
        };
        let theirs = || Message::FileChecked {
            contents: Some(b"- b\n".to_vec()),
        };
        let mut state = opened();
        state.update(theirs());
        assert_eq!(state.mode(), Mode::Navigate);
        assert_eq!(texts(&state), ["- b"]);
        let mut state = opened();
        typed(&mut state, "x");
        state.update(theirs());
        assert!(state.in_conflict());
        state.update(Message::ToggleDiff);
        assert_eq!(
            state.mode(),
            Mode::Edit,
            "the prompt's diff leaves the session"
        );
        state.update(Message::TakeTheirs);
        assert_eq!(state.mode(), Mode::Navigate);
    }

    /// A file whose first line is being edited, opened at time 0.
    fn editing_file(text: &str) -> (State, PathBuf) {
        let path = PathBuf::from("/notes/todo.taskpaper");
        let mut state = State::new(format::parse(text), Some(path.clone()))
            .with_version(Some(Version::of(text.as_bytes())));
        state.update(Message::EditAtEnd);
        (state, path)
    }

    fn later(state: &mut State, seconds: u64) -> Vec<Effect> {
        state.update(Message::Tick {
            now: Duration::from_secs(seconds),
        })
    }

    /// A save of `text` over the file holding `on_disk`.
    fn save(path: &Path, text: &str, on_disk: &str) -> Vec<Effect> {
        vec![
            Effect::Disk(Disk::Backup { path: path.into() }),
            Effect::Disk(Disk::Save {
                path: path.into(),
                text: text.to_owned(),
                expected: Some(Version::of(on_disk.as_bytes())),
            }),
        ]
    }

    // FR-DOC-02
    #[test]
    fn typing_is_saved_while_editing() {
        let (mut state, path) = editing_file("- a\n");
        typed(&mut state, "b");
        assert_eq!(later(&mut state, 1), save(&path, "- ab\n", "- a\n"));
        assert_eq!(state.mode(), Mode::Edit);
    }

    // file-format §3.1
    #[test]
    fn saving_waits_while_the_line_being_edited_would_be_read_back_elsewhere() {
        let (mut state, path) = editing_file("a\n\tb\n");
        press(&mut state, TextEdit::DeleteBackward, 1);
        assert_eq!(state.deadline(), None, "no save is due");
        assert_eq!(later(&mut state, 5), []);
        typed(&mut state, "c");
        assert_eq!(state.deadline(), Some(Duration::from_secs(6)));
        assert_eq!(later(&mut state, 6), save(&path, "c\n\tb\n", "a\n\tb\n"));
        state.update(Message::Saved);
        state.update(Message::Text(TextEdit::Move {
            motion: Motion::Start,
            select: false,
        }));
        typed(&mut state, " ");
        assert_eq!(later(&mut state, 10), []);
        state.update(Message::StopEditing);
        assert_eq!(later(&mut state, 11), save(&path, "c\n\tb\n", "c\n\tb\n"));
    }

    // file-io.md §2, file-format §3.1
    #[test]
    fn quitting_ends_the_session_repaired_and_saves() {
        let (mut state, path) = editing_file("a\n\tb\n");
        press(&mut state, TextEdit::DeleteBackward, 1);
        let effects = state.update(Message::Quit);
        assert_eq!(state.mode(), Mode::Navigate);
        assert_eq!(effects, save(&path, "…\n\tb\n", "a\n\tb\n"));
    }

    /// Edits the focused block with the caret at the start of its text.
    fn edit_from_start(state: &mut State) {
        state.update(Message::EditAtEnd);
        go(state, Motion::Start);
    }

    fn go(state: &mut State, motion: Motion) {
        state.update(Message::Text(TextEdit::Move {
            motion,
            select: false,
        }));
    }

    // editing.md §2, D-43
    #[test]
    fn up_and_down_edit_the_block_above_and_below_at_the_same_column() {
        let mut state = open(
            "- abcdef
P:
	- x😀yz
- ab",
            2,
        );
        state.update(Message::EditAtEnd);
        go(&mut state, Motion::Start);
        go(&mut state, Motion::Right);
        go(&mut state, Motion::Right);
        state.update(Message::EditPrevious);
        assert_eq!(state.mode(), Mode::Edit);
        assert_eq!(state.focus(), Some(state.rows()[1].id));
        assert_eq!(shown(&state), "P|:", "at the end of a shorter name");
        state.update(Message::EditPrevious);
        assert_eq!(shown(&state), "a|bcdef", "the column it had on P");
        state.update(Message::EditNext);
        state.update(Message::EditNext);
        go(&mut state, Motion::Right);
        go(&mut state, Motion::Right);
        assert_eq!(shown(&state), "x😀y|z", "by grapheme");
        state.update(Message::EditNext);
        assert_eq!(shown(&state), "ab|");
        assert_eq!(state.focus(), Some(state.rows()[3].id));
    }

    // editing.md §2, D-43
    #[test]
    fn up_and_down_edit_dividers_too_and_stay_at_the_ends() {
        let mut state = open(
            "- a
---
- b",
            2,
        );
        state.update(Message::EditAtEnd);
        state.update(Message::EditNext);
        assert_eq!(shown(&state), "b|", "nothing below");
        assert_eq!(state.focus(), Some(state.rows()[2].id));
        state.update(Message::EditPrevious);
        assert_eq!(shown(&state), "-|--", "a divider is text");
        state.update(Message::EditPrevious);
        assert_eq!(shown(&state), "a|");
        assert_eq!(state.focus(), Some(state.rows()[0].id));
        state.update(Message::EditPrevious);
        assert_eq!(shown(&state), "a|", "nothing above");
        let mut state = open(
            "- a
	- b
- c",
            0,
        );
        state.update(Message::Collapse);
        state.update(Message::EditAtEnd);
        state.update(Message::EditNext);
        assert_eq!(shown(&state), "c|", "the next visible block");
        state.update(Message::StopEditing);
        state.update(Message::EditNext);
        assert_eq!(state.mode(), Mode::Navigate, "only in edit mode");
    }

    // FR-OPS-08, editing.md §2, D-43
    #[test]
    fn each_block_edited_on_with_up_and_down_is_its_own_undo_step() {
        let mut state = open(
            "- a
- b", 0,
        );
        state.update(Message::EditAtEnd);
        typed(&mut state, "1");
        state.update(Message::EditNext);
        typed(&mut state, "2");
        state.update(Message::StopEditing);
        assert_eq!(texts(&state), ["- a1", "- b2"]);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a1", "- b"]);
        assert_eq!(state.focus(), Some(state.rows()[1].id));
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a", "- b"]);
        assert_eq!(state.focus(), Some(state.rows()[0].id));
    }

    // FR-OPS-08
    #[test]
    fn a_step_committed_while_a_new_block_is_edited_comes_after_its_creation() {
        let mut state = open("n\n- t", 0);
        state.update(Message::New(NewBlock::Below, None));
        typed(&mut state, "m");
        let task = state.rows()[2].id;
        state.commit(vec![Op::SetText {
            id: task,
            before: "- t".to_owned(),
            after: "- u".to_owned(),
        }]);
        typed(&mut state, "o");
        state.update(Message::StopEditing);
        assert_eq!(texts(&state), ["n", "mo", "- u"]);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["n", "", "- u"]);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["n", "", "- t"]);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["n", "- t"]);
    }

    // D-43
    #[test]
    fn code_and_url_lines_are_edited_like_any_other_line() {
        let mut state = open(
            "run() @code
Site @url(https://x.y)",
            0,
        );
        state.update(Message::EditAtEnd);
        typed(&mut state, "!");
        state.update(Message::EditNext);
        typed(&mut state, "!");
        state.update(Message::StopEditing);
        assert_eq!(texts(&state), ["run()! @code", "Site! @url(https://x.y)"]);
    }

    const TODAY: Date = jiff::civil::date(2026, 10, 6);

    fn menu(state: &mut State, input: MenuInput) -> Vec<Effect> {
        state.update(Message::CommandMenu(input))
    }

    /// Opens the command menu and types `keys` into it.
    fn slash(state: &mut State, keys: &str) {
        state.update(Message::OpenCommandMenu { today: TODAY });
        for c in keys.chars() {
            menu(state, MenuInput::Type(c));
        }
    }

    /// The labels the open menu lists, with `|` for a divider.
    fn listed(state: &State) -> Vec<&str> {
        let rows = state.command_menu().unwrap().rows;
        rows.into_iter()
            .map(|row| match row {
                MenuRow::Command { label, .. } => label,
                MenuRow::Divider => "|",
            })
            .collect()
    }

    /// `text` with its first block edited and the caret before its last word.
    fn editing_before_last_word(text: &str) -> State {
        let mut state = open(text, 0);
        state.update(Message::EditAtEnd);
        go(&mut state, Motion::WordLeft);
        state
    }

    // D-44, D-50, editing.md §2.3
    #[test]
    fn a_type_chosen_after_the_slash_gives_the_block_its_type_with_the_caret_kept() {
        for (keys, line, kind, text) in [
            ("p", "Call Ana: @x", BlockKind::Project, "Call |Ana:"),
            ("n", "Call Ana @x", BlockKind::Text, "Call |Ana"),
            ("t", "- Call Ana @x", BlockKind::Task, "Call |Ana"),
        ] {
            let mut state = editing_before_last_word("- Call Ana @x\n- t");
            slash(&mut state, keys);
            assert!(state.command_menu().is_some(), "{keys} only filters");
            assert_eq!(texts(&state)[0], "- Call Ana @x", "opening changes nothing");
            menu(&mut state, MenuInput::Choose);
            assert_eq!(state.command_menu(), None, "{keys}");
            assert_eq!(texts(&state)[0], line, "{keys}");
            assert_eq!(state.rows()[0].kind, kind, "{keys}");
            assert_eq!(shown(&state), text, "{keys}");
            assert_eq!(state.mode(), Mode::Edit);
        }
    }

    // D-50, editing.md §2.3
    #[test]
    fn the_menu_lists_the_blocks_commands_and_filters_them_by_word() {
        let mut state = open("- a @due(2026-10-09) https://x.y", 0);
        state.update(Message::EditAtEnd);
        slash(&mut state, "");
        let all = listed(&state);
        assert_eq!(all.first(), Some(&"Project"));
        assert!(all.contains(&"Clear Due Date") && all.contains(&"Open Link"));
        assert!(!all.contains(&"Clear Start Date"), "no start date to clear");
        menu(&mut state, MenuInput::Dismiss);
        state.update(Message::Text(TextEdit::DeleteBackward));
        slash(&mut state, "date");
        assert_eq!(
            listed(&state),
            ["Set Start Date", "Set Due Date", "Clear Due Date"]
        );
        assert_eq!(state.command_menu().unwrap().filter, "date");
    }

    // D-50, editing.md §2.3
    #[test]
    fn a_command_runs_as_its_own_step_and_editing_resumes_at_the_caret() {
        let mut state = editing_before_last_word("- Call Ana\n- t");
        typed(&mut state, "x");
        slash(&mut state, "due");
        assert_eq!(
            listed(&state),
            ["Set Due Date", "Due Today", "Due Tomorrow"]
        );
        menu(&mut state, MenuInput::Next);
        menu(&mut state, MenuInput::Next);
        menu(&mut state, MenuInput::Previous);
        assert_eq!(menu(&mut state, MenuInput::Choose), []);
        assert_eq!(texts(&state), ["- Call xAna @due(2026-10-06)", "- t"]);
        assert_eq!(state.mode(), Mode::Edit);
        assert_eq!(state.command_menu(), None);
        assert_eq!(shown(&state), "Call x|Ana", "the caret where it was");
        typed(&mut state, "y");
        state.update(Message::StopEditing);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- Call xAna @due(2026-10-06)", "- t"]);
        state.update(Message::Undo);
        assert_eq!(
            texts(&state),
            ["- Call xAna", "- t"],
            "the command's own step"
        );
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- Call Ana", "- t"], "the typing before it");
    }

    // D-50, editing.md §2.3
    #[test]
    fn a_prompt_the_menu_opens_returns_to_editing_when_it_closes() {
        for (input, line) in [
            (DateInput::Apply, "- Call Ana @start(2026-10-07)"),
            (DateInput::Cancel, "- Call Ana"),
        ] {
            let mut state = editing_before_last_word("- Call Ana");
            slash(&mut state, "set s");
            menu(&mut state, MenuInput::Choose);
            assert_eq!(state.mode(), Mode::Navigate, "the prompt takes the keys");
            assert!(state.date_prompt().is_some());
            for c in "tomorrow".chars() {
                state.update(Message::DatePrompt(DateInput::Edit(TextEdit::Insert(c))));
            }
            assert_eq!(state.mode(), Mode::Navigate, "the prompt stays open");
            state.update(Message::DatePrompt(input));
            assert_eq!(texts(&state), [line]);
            assert_eq!(state.mode(), Mode::Edit, "{input:?}");
            assert_eq!(shown(&state), "Call |Ana", "{input:?}");
        }
    }

    // D-50, D-3
    #[test]
    fn the_details_the_menu_shows_return_to_editing_when_closed() {
        let mut state = editing_before_last_word("- Call Ana @x");
        slash(&mut state, "show");
        menu(&mut state, MenuInput::Choose);
        assert!(state.details().is_some());
        assert_eq!(state.mode(), Mode::Navigate);
        state.update(Message::CloseDetails);
        assert_eq!(shown(&state), "Call |Ana");
    }

    // D-50
    #[test]
    fn duplicate_and_open_link_resume_editing_on_the_block_and_delete_does_not() {
        let line = "- see https://x.y now";
        let mut state = editing_before_last_word(&format!("{line}\n- t"));
        slash(&mut state, "dup");
        menu(&mut state, MenuInput::Choose);
        assert_eq!(texts(&state), [line, line, "- t"]);
        assert_eq!(state.focus(), Some(state.rows()[0].id), "the original");
        assert_eq!(shown(&state), "see https://x.y |now");
        slash(&mut state, "open");
        assert_eq!(
            menu(&mut state, MenuInput::Choose),
            [Effect::OpenUrl("https://x.y".to_owned())]
        );
        assert_eq!(shown(&state), "see https://x.y |now");
        slash(&mut state, "del");
        menu(&mut state, MenuInput::Choose);
        assert_eq!(texts(&state), [line, "- t"]);
        assert_eq!(state.mode(), Mode::Navigate, "nothing to edit");
    }

    // D-44, D-50, editing.md §2.3
    #[test]
    fn esc_or_a_filter_matching_nothing_types_the_slash_and_the_filter() {
        let mut state = open("see", 0);
        state.update(Message::EditAtEnd);
        typed(&mut state, " ");
        slash(&mut state, "");
        menu(&mut state, MenuInput::Dismiss);
        assert_eq!(shown(&state), "see /|");
        slash(&mut state, "h");
        assert_eq!(state.command_menu(), None, "nothing starts with h");
        typed(&mut state, "ome");
        assert_eq!(texts(&state), ["see //home"]);
        slash(&mut state, "r");
        menu(&mut state, MenuInput::Dismiss);
        assert_eq!(shown(&state), "see //home/r|");
        slash(&mut state, "o");
        menu(&mut state, MenuInput::Erase);
        menu(&mut state, MenuInput::Erase);
        assert_eq!(state.command_menu(), None);
        assert_eq!(
            shown(&state),
            "see //home/r|",
            "erasing the slash types nothing"
        );
    }

    // D-44
    #[test]
    fn a_menu_input_without_an_open_menu_does_nothing() {
        let mut state = open("a", 0);
        menu(&mut state, MenuInput::Choose);
        state.update(Message::OpenCommandMenu { today: TODAY });
        assert_eq!(state.command_menu(), None, "no menu in navigate mode");
        state.update(Message::EditAtEnd);
        menu(&mut state, MenuInput::Type('t'));
        assert_eq!(texts(&state), ["a"]);
        state.update(Message::OpenCommandMenu { today: TODAY });
        state.update(Message::StopEditing);
        state.update(Message::EditAtEnd);
        assert_eq!(state.command_menu(), None, "the menu ends with the session");
    }

    // D-44, FR-OPS-08
    #[test]
    fn a_type_change_is_a_step_of_the_session() {
        let mut state = open("note\n- t", 0);
        state.update(Message::EditAtEnd);
        typed(&mut state, "s");
        slash(&mut state, "p");
        menu(&mut state, MenuInput::Choose);
        assert_eq!(texts(&state), ["notes:", "- t"]);
        assert_eq!(
            shown(&state),
            "notes|:",
            "the colon is text after the caret"
        );
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["notes", "- t"]);
        state.update(Message::Redo);
        assert_eq!(texts(&state), ["notes:", "- t"]);
        typed(&mut state, "!");
        state.update(Message::Undo);
        state.update(Message::Undo);
        assert_eq!(
            texts(&state),
            ["notes", "- t"],
            "typing after it is a change of its own"
        );
        state.update(Message::Redo);
        state.update(Message::StopEditing);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["note", "- t"], "one step with the typing");
    }

    // D-52
    #[test]
    fn make_section_and_remove_section_keep_editing_as_one_step() {
        let mut state = editing_before_last_word("Call Ana @x\n- t");
        slash(&mut state, "make");
        assert_eq!(listed(&state), ["Make Section"]);
        menu(&mut state, MenuInput::Choose);
        assert_eq!(texts(&state)[0], "Call Ana @x @section");
        assert!(state.rows()[0].section);
        assert_eq!(shown(&state), "Call |Ana");
        slash(&mut state, "sec");
        assert_eq!(listed(&state), ["Remove Section"]);
        menu(&mut state, MenuInput::Choose);
        assert_eq!(texts(&state)[0], "Call Ana @x");
        state.update(Message::StopEditing);
        state.update(Message::Undo);
        assert_eq!(
            texts(&state)[0],
            "Call Ana @x",
            "the session changed nothing"
        );
    }

    // D-50
    #[test]
    fn ctrl_t_cycles_project_section_task_and_note_keeping_text_and_caret() {
        let mut state = editing_before_last_word("Call Ana @x\n- t");
        let mut seen = Vec::new();
        for _ in 0..4 {
            state.update(Message::CycleType);
            let row = &state.rows()[0];
            seen.push((
                texts(&state)[0].clone(),
                row.kind,
                row.section,
                shown(&state),
            ));
        }
        assert_eq!(
            seen,
            [
                (
                    "Call Ana: @x".to_owned(),
                    BlockKind::Project,
                    false,
                    "Call |Ana:".to_owned()
                ),
                (
                    "Call Ana @x @section".to_owned(),
                    BlockKind::Text,
                    true,
                    "Call |Ana".to_owned()
                ),
                (
                    "- Call Ana @x".to_owned(),
                    BlockKind::Task,
                    false,
                    "Call |Ana".to_owned()
                ),
                (
                    "Call Ana @x".to_owned(),
                    BlockKind::Text,
                    false,
                    "Call |Ana".to_owned()
                ),
            ]
        );
        state.update(Message::Undo);
        assert_eq!(texts(&state)[0], "- Call Ana @x", "a change of the session");
        state.update(Message::StopEditing);
        state.update(Message::Undo);
        assert_eq!(texts(&state)[0], "Call Ana @x");
        state.update(Message::CycleType);
        assert_eq!(texts(&state)[0], "Call Ana @x", "only while editing");
    }
}
