//! Edit mode: the focused one-line block's text edited in place with the text keys, written to
//! the document as it is typed, refused where it would type a tag, undone as one step and
//! repaired when editing ends ([editing.md §1–2](../../../docs/specs/editing.md#1-modes),
//! FR-MODE-01–03, OQ-24, OQ-25, OQ-27).

use std::{mem, ops::Range};

use super::{
    Access, Notice, Restore, State,
    text_field::{TextEdit, TextField},
};
use crate::document::{
    BlockKind, ItemId, Op, Transaction, blocks,
    format::{self, LineParts, canonical, tags},
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
    /// The text as typed so far: the block's text without its type syntax and tags.
    pub text: &'a str,
    /// The caret's byte offset into `text`.
    pub caret: usize,
    /// The selected bytes of `text`, if any.
    pub selection: Option<Range<usize>>,
    /// What follows the text as part of the block's type and cannot be edited: a project's `:`.
    pub suffix: &'static str,
}

/// One edit session: the block's line, its parts around the text and the text field, with the
/// session's own undo history of the text.
#[derive(Debug, Clone)]
pub(super) struct Session {
    /// The block's only line.
    id: ItemId,
    /// The applied ops that created the block for this session, undone with it as one step; empty
    /// for a block that was there before.
    created: Vec<Op>,
    /// The focus before the session's step.
    before: Option<ItemId>,
    /// The line's text when the session started.
    original: String,
    /// The parts and text the session started with, which stand for `original` as it was.
    start: (LineParts, String),
    parts: LineParts,
    field: TextField,
    undo: Vec<(LineParts, TextField)>,
    redo: Vec<(LineParts, TextField)>,
    /// Whether the latest change typed a character, so that the next one typed joins its undo
    /// step.
    typing: bool,
}

impl Session {
    /// A session editing line `id`, whose text is `original`, with the caret at the end of the
    /// text, or at its start without `at_end`.
    fn new(id: ItemId, original: String, at_end: bool) -> Self {
        let (parts, text) = LineParts::split(&original);
        Self {
            id,
            created: Vec::new(),
            before: Some(id),
            original,
            field: TextField::new(text.clone(), at_end),
            start: (parts.clone(), text),
            parts,
            undo: Vec::new(),
            redo: Vec::new(),
            typing: false,
        }
    }

    /// Applies `edit` to the text, or returns `false` and changes nothing when it would type a
    /// tag (OQ-25). A character that breaks a line is not typed, since it would split the line
    /// when the file is read back. Text that makes the line a task takes the task's syntax, so that its dash
    /// shows as the marker (OQ-27).
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
                let cut =
                    (field.text().len() + self.parts.suffix().len()).saturating_sub(text.len());
                self.parts = parts;
                TextField::with_caret(text, field.caret().saturating_sub(cut))
            }
            None => field,
        };
        true
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

    /// The line the text gives: the original line while the text is as it started, so that an
    /// unchanged text leaves mid-line tags where they are.
    fn line(&self) -> String {
        if self.start.0 == self.parts && self.start.1 == self.field.text() {
            self.original.clone()
        } else {
            self.parts.line(self.field.text())
        }
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
            suffix: session.parts.suffix(),
        })
    }

    /// Starts editing the focused block's text with the caret at its end, or at its start
    /// without `at_end`. Blocks of several lines and dividers, whose text is their type, are
    /// not edited here; a read-only file refuses with a notice.
    pub(super) fn start_editing(&mut self, at_end: bool) {
        let Some((block, original)) = self.focused().and_then(|(block, _)| {
            let text = self.document.get(block.id())?.text().to_owned();
            Some((block, text))
        }) else {
            return;
        };
        if self.edit.is_some() || block.lines().len() > 1 || block.kind() == BlockKind::Divider {
            return;
        }
        if self.access() == Access::ReadOnly {
            self.notify(Notice::ReadOnly);
            return;
        }
        self.edit = Some(Session::new(block.id(), original, at_end));
    }

    /// Starts editing the new line `id`, which the applied `created` ops put in while the focus
    /// was on `before`, with an empty text (FR-MODE-04).
    pub(super) fn edit_new_block(&mut self, id: ItemId, created: Vec<Op>, before: Option<ItemId>) {
        if let Some(original) = self.document.get(id).map(|item| item.text().to_owned()) {
            self.edit = Some(Session {
                created,
                before,
                ..Session::new(id, original, true)
            });
        }
    }

    /// Applies `edit` to the text being edited and writes it to the document, or shows why a
    /// tag cannot be typed (OQ-25). `Backspace` in an empty text deletes the block instead
    /// (editing.md §2).
    pub(super) fn edit_text(&mut self, edit: TextEdit) {
        let Some(session) = &mut self.edit else {
            return;
        };
        if edit == TextEdit::DeleteBackward && session.field.text().is_empty() {
            self.delete_empty_block();
            return;
        }
        if session.edit(edit) {
            self.write_line();
        } else {
            self.notify(Notice::TypedTag);
        }
    }

    /// Takes the text being edited back to before its latest change.
    pub(super) fn undo_typing(&mut self) {
        if let Some(session) = &mut self.edit {
            session.step(true);
        }
        self.write_line();
    }

    /// Replays the latest change to the text being edited that was undone.
    pub(super) fn redo_typing(&mut self) {
        if let Some(session) = &mut self.edit {
            session.step(false);
        }
        self.write_line();
    }

    /// Ends editing: repairs the line where it breaks canonical-outline rule 2 or 4, then records
    /// the whole session, with the block's creation, as one step to undo (file-format.md §3.1,
    /// FR-OPS-08).
    pub(super) fn stop_editing(&mut self) {
        let only_line = self.only_line();
        let Some((session, repaired)) = self.edit.take().and_then(|session| {
            let item = self.document.get(session.id)?;
            let repaired = canonical::repaired(item.text(), !item.children().is_empty(), only_line);
            Some((session, repaired))
        }) else {
            return;
        };
        self.set_line(session.id, repaired.clone());
        let mut ops = session.created;
        if repaired != session.original {
            ops.push(Op::SetText {
                id: session.id,
                before: session.original,
                after: repaired,
            });
        }
        let before = Restore {
            focus: session.before,
        };
        let after = Restore {
            focus: Some(session.id),
        };
        self.edits.record(Transaction::new(ops, before, after));
    }

    /// Ends editing when the block being edited has joined other note lines, since blocks of
    /// several lines are not edited here (OQ-26).
    pub(super) fn stop_editing_merged(&mut self) {
        if let Some(session) = &self.edit
            && blocks::block(&self.document, session.id).is_none_or(|block| block.lines().len() > 1)
        {
            self.stop_editing();
        }
    }

    /// Records the creation of the block being edited as a step of its own, so that a step
    /// committed while it is edited, which may count its line, comes after it in the history and
    /// is undone before it.
    pub(super) fn record_creation(&mut self) {
        if let Some(session) = self
            .edit
            .as_mut()
            .filter(|session| !session.created.is_empty())
        {
            let before = Restore {
                focus: session.before.replace(session.id),
            };
            let after = Restore {
                focus: Some(session.id),
            };
            let ops = mem::take(&mut session.created);
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
        let (id, original) = (session.id, session.original.clone());
        let mut undo = Transaction::new(session.created.clone(), (), ()).inverse();
        self.set_line(id, original);
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
        self.edit
            .as_ref()
            .and_then(|session| Some((session.id, self.document.get(session.id)?)))
            .is_some_and(|(id, item)| {
                canonical::needs_repair(item.text(), !item.children().is_empty(), self.only_line())
                    || item.text().is_empty()
                        && canonical::trailing_blanks(&self.document).contains(&id)
            })
    }

    /// Whether the document holds a single line, which keeps its leading whitespace
    /// (file-format.md §2).
    fn only_line(&self) -> bool {
        self.document
            .descendants(self.document.root())
            .nth(1)
            .is_none()
    }

    /// Writes the text being edited to its line.
    fn write_line(&mut self) {
        if let Some(session) = &self.edit {
            let (id, line) = (session.id, session.line());
            self.set_line(id, line);
        }
    }

    /// Sets the text of line `id` to `line` as a change of its own, outside the undo history,
    /// so that saving and every view see it at once.
    fn set_line(&mut self, id: ItemId, line: String) {
        if let Some(before) = self.document.get(id).map(|item| item.text().to_owned())
            && before != line
        {
            let op = Op::SetText {
                id,
                before,
                after: line,
            };
            self.document
                .apply(&mut Transaction::new(vec![op], (), ()))
                .expect("the op starts from the line's current text");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::{Path, PathBuf},
        time::Duration,
    };

    use super::*;
    use crate::{
        app::{Disk, Effect, Message, Motion, Version},
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
        state.update(Message::StopEditing);
        state.update(Message::EditAtStart);
        assert_eq!(shown(&state), "|bc");
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
        assert_eq!(state.rows()[0].lines, ["Milks"]);
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
        assert_eq!(state.editor().unwrap().suffix, "");
        typed(&mut state, " why");
        assert_eq!(state.rows()[0].kind, BlockKind::Note);
        state.update(Message::Text(TextEdit::SelectAll));
        typed(&mut state, "- buy");
        assert_eq!(state.rows()[0].kind, BlockKind::Task);
        assert_eq!(shown(&state), "buy|", "the dash becomes the marker");
        assert_eq!(texts(&state), ["- buy @x"]);
    }

    // OQ-27
    #[test]
    fn a_project_keeps_its_colon_after_the_text_until_it_becomes_a_task() {
        let mut state = open("Inbox:", 0);
        state.update(Message::EditAtStart);
        assert_eq!(state.editor().unwrap().suffix, ":");
        typed(&mut state, "My ");
        assert_eq!(texts(&state), ["My Inbox:"]);
        state.update(Message::Text(TextEdit::Move {
            motion: Motion::Start,
            select: false,
        }));
        typed(&mut state, "- ");
        assert_eq!(shown(&state), "|My Inbox:");
        assert_eq!(state.editor().unwrap().suffix, "");
        assert_eq!(state.rows()[0].kind, BlockKind::Task);
        assert_eq!(texts(&state), ["- My Inbox:"]);
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
        state.update(Message::EditAtStart);
        typed(&mut state, "- ");
        assert_eq!(texts(&state), ["- a"]);
        state.update(Message::Undo);
        assert_eq!(shown(&state), "|a");
        assert_eq!(texts(&state), ["a"]);
        assert_eq!(state.rows()[0].kind, BlockKind::Note);
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
        state.update(Message::EditAtStart);
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
    fn several_lines_dividers_and_an_empty_outline_are_not_edited() {
        for (text, index) in [("n\nm", 0), ("- a\n---", 1), ("", 0)] {
            let mut state = open(text, index);
            state.update(Message::EditAtEnd);
            assert_eq!(state.mode(), Mode::Navigate, "{text:?}");
        }
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
}
