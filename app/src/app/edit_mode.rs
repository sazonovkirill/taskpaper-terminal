//! Edit mode: the focused block's text edited in place with the text keys, its lines written to
//! the document as they are typed, refused where it would type a tag, kept together until editing
//! ends, undone as one step and repaired when editing ends
//! ([editing.md §1–2](../../../docs/specs/editing.md#1-modes), FR-MODE-01–03, FR-BLK-03, FR-BLK-05,
//! OQ-23, OQ-24, OQ-25, OQ-27).

use std::{mem, ops::Range};

use super::{
    Access, Notice, Restore, State,
    structure::NewBlock,
    text_field::{TextEdit, TextField},
};
use crate::document::{
    Block, BlockKind, Document, Item, ItemId, Op, Transaction, blocks,
    format::{self, BlockLines, LineParts, canonical, tags},
    structure,
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
    /// The text as typed so far: the block's text without its type syntax and tags, with `\n`
    /// between its lines.
    pub text: &'a str,
    /// The caret's byte offset into `text`.
    pub caret: usize,
    /// The selected bytes of `text`, if any.
    pub selection: Option<Range<usize>>,
    /// What follows the text as part of the block's type and cannot be edited: a project's `:`.
    pub suffix: &'static str,
    /// Whether a line of the text reads as another type, so that the block splits there when
    /// editing ends (OQ-27).
    pub splits: bool,
}

/// One edit session: the block's lines as they started and as they are written now, the parts of
/// its first line and the text field, with the session's own undo history of the text.
#[derive(Debug, Clone)]
pub(super) struct Session {
    /// The block's first line, which stays its first line.
    id: ItemId,
    /// The applied ops that created the block for this session, undone with it as one step; empty
    /// for a block that was there before.
    created: Vec<Op>,
    /// The focus before the session's step.
    before: Option<ItemId>,
    /// The block's type when the session started, which it keeps on screen while its text has
    /// several lines.
    kind: BlockKind,
    /// The block's lines when the session started, which the text rewrites.
    block: BlockLines,
    /// The ids of those lines, in order.
    originals: Vec<ItemId>,
    /// The ids of the lines the text added, in the order it added them, kept for adding again.
    added: Vec<ItemId>,
    /// The lines as the document has them without `applied`.
    base: Vec<ItemId>,
    /// The lines the text is written to, in order.
    lines: Vec<ItemId>,
    /// The ops that wrote the text over `base`, applied outside the history.
    applied: Vec<Op>,
    /// The first line's parts, which change as the text gives a one-line block a task's type.
    parts: LineParts,
    field: TextField,
    undo: Vec<(LineParts, TextField)>,
    redo: Vec<(LineParts, TextField)>,
    /// Whether the latest change typed a character, so that the next one typed joins its undo
    /// step.
    typing: bool,
}

impl Session {
    /// A session editing `block` of `document` with the caret at the end of its text, or at its
    /// start without `at_end`.
    fn new(document: &Document, block: &Block, at_end: bool) -> Self {
        let originals = block.lines().to_vec();
        let (lines, text) = BlockLines::split(
            originals
                .iter()
                .filter_map(|&id| document.get(id))
                .map(Item::text),
        );
        Self {
            id: block.id(),
            created: Vec::new(),
            before: Some(block.id()),
            kind: block.kind(),
            parts: lines.first(),
            block: lines,
            base: originals.clone(),
            lines: originals.clone(),
            originals,
            added: Vec::new(),
            applied: Vec::new(),
            field: TextField::new(text, at_end),
            undo: Vec::new(),
            redo: Vec::new(),
            typing: false,
        }
    }

    /// Applies `edit` to the text, or returns `false` and changes nothing when it would type a
    /// tag (OQ-25). A character that breaks a line is not typed, since it would split the line
    /// when the file is read back; [`TextEdit::LineBreak`] breaks it. Text that makes a one-line
    /// text a task takes the task's syntax, so that its dash shows as the marker (OQ-27).
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
        let as_task = Some(field.text())
            .filter(|text| !text.contains('\n'))
            .and_then(|text| self.parts.as_task(text));
        self.field = match as_task {
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

    /// The line holding the caret.
    fn caret_line(&self) -> Option<ItemId> {
        let before = self.field.text().get(..self.field.caret())?;
        self.lines.get(before.matches('\n').count()).copied()
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
            splits: blocks::block(&self.document, session.id)
                .is_none_or(|block| block.lines() != session.lines),
        })
    }

    /// Starts editing the focused block's text with the caret at its end, or at its start
    /// without `at_end`. A divider, whose text is its type, is not edited; a read-only file
    /// refuses with a notice.
    pub(super) fn start_editing(&mut self, at_end: bool) {
        let Some((block, _)) = self.focused() else {
            return;
        };
        if self.edit.is_some() || block.kind() == BlockKind::Divider {
            return;
        }
        if self.access() == Access::ReadOnly {
            self.notify(Notice::ReadOnly);
            return;
        }
        self.edit = Some(Session::new(&self.document, &block, at_end));
    }

    /// Starts editing the new line `id`, which the applied `created` ops put in while the focus
    /// was on `before`, with an empty text (FR-MODE-04).
    pub(super) fn edit_new_block(&mut self, id: ItemId, created: Vec<Op>, before: Option<ItemId>) {
        if let Some(block) = blocks::block(&self.document, id) {
            self.edit = Some(Session {
                created,
                before,
                ..Session::new(&self.document, &block, true)
            });
        }
    }

    /// Applies `edit` to the text being edited and writes it to the document, or shows why a
    /// tag cannot be typed (OQ-25). `Backspace` in an empty text deletes the block instead, and
    /// only note and code blocks take line breaks (editing.md §2).
    pub(super) fn edit_text(&mut self, edit: TextEdit) {
        let takes_line_breaks =
            matches!(self.edited_kind(), Some(BlockKind::Note | BlockKind::Code));
        let Some(session) = &mut self.edit else {
            return;
        };
        if edit == TextEdit::DeleteBackward && session.field.text().is_empty() {
            self.delete_empty_block();
            return;
        }
        if edit == TextEdit::LineBreak && !takes_line_breaks {
            return;
        }
        if session.edit(edit) {
            self.write(false);
        } else {
            self.notify(Notice::TypedTag);
        }
    }

    /// `Enter` in edit mode: breaks the line in a code block, else ends editing and adds the next
    /// block (OQ-23, editing.md §2–3).
    pub(super) fn new_next(&mut self) {
        if self.edited_kind() == Some(BlockKind::Code) {
            self.edit_text(TextEdit::LineBreak);
        } else {
            self.stop_editing();
            self.add_block(NewBlock::Next, None);
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

    /// Ends editing: writes the lines repaired where they break canonical-outline rule 2 or 4,
    /// records the whole session, with the block's creation, as one step to undo, and focuses the
    /// block holding the caret, which is another one when the text split the block
    /// (file-format.md §3.1, FR-OPS-08, OQ-27).
    pub(super) fn stop_editing(&mut self) {
        self.write(true);
        let Some(session) = self.edit.take() else {
            return;
        };
        let focus = session
            .caret_line()
            .and_then(|line| blocks::holding(&self.document, line))
            .map_or(session.id, |block| block.id());
        let before = Restore {
            focus: session.before,
        };
        let after = Restore { focus: Some(focus) };
        let ops = session.created.into_iter().chain(session.applied).collect();
        self.edits.record(Transaction::new(ops, before, after));
        self.focus = Some(focus);
    }

    /// Goes on editing the block being edited in the block it is now part of, after a step put it
    /// next to note lines that it joined (OQ-26): what the session wrote so far becomes a step of
    /// its own, and a new session edits the merged block with the caret where it was.
    pub(super) fn edit_merged(&mut self) {
        let Some(session) = self.edit.take() else {
            return;
        };
        let Some(block) = blocks::holding(&self.document, session.id).filter(|block| {
            block.lines().len() > session.lines.len()
                && session
                    .lines
                    .iter()
                    .all(|line| block.lines().contains(line))
        }) else {
            self.edit = Some(session);
            return;
        };
        let before = Restore {
            focus: session.before,
        };
        let after = Restore {
            focus: Some(session.id),
        };
        let ops = session.created.into_iter().chain(session.applied).collect();
        self.edits.record(Transaction::new(ops, before, after));
        let mut merged = Session::new(&self.document, &block, true);
        let above: usize = block
            .lines()
            .iter()
            .take_while(|&&line| line != session.id)
            .filter_map(|&line| self.document.get(line))
            .map(|item| item.display_text().len() + 1)
            .sum();
        merged.field = TextField::with_caret(
            merged.field.text().to_owned(),
            above + session.field.caret(),
        );
        self.focus = Some(block.id());
        self.edit = Some(merged);
    }

    /// Records the creation of the block being edited and, once its text has added or removed
    /// lines, what it wrote so far, as a step of its own, so that a step committed while it is
    /// edited, which may count or move its lines, comes after it in the history and is undone
    /// before it. Text written to lines that stay in place is recorded when editing ends.
    pub(super) fn record_edit_so_far(&mut self) {
        let Some(session) = self.edit.as_mut() else {
            return;
        };
        let mut ops = mem::take(&mut session.created);
        if session
            .applied
            .iter()
            .any(|op| matches!(op, Op::Insert { .. } | Op::Remove { .. } | Op::Move { .. }))
        {
            ops.append(&mut session.applied);
            session.base.clone_from(&session.lines);
        }
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
        let ops = session.created.iter().chain(&session.applied).cloned();
        let mut undo = Transaction::new(ops.collect(), (), ()).inverse();
        if self.document.apply(&mut undo).is_err() {
            return false;
        }
        self.edit = None;
        true
    }

    /// Whether saving waits because a line being edited breaks rule 2 or 4 for now, or is one of
    /// the empty lines ending the document that break rule 3: typing may repair it, and ending
    /// the session or the save after it does (file-format.md §3.1).
    pub(super) fn editing_holds_save(&self) -> bool {
        let only_line = only_line(&self.document);
        self.edit
            .iter()
            .flat_map(|session| &session.lines)
            .any(|&id| {
                self.document.get(id).is_some_and(|item| {
                    canonical::needs_repair(item.text(), !item.children().is_empty(), only_line)
                        || item.text().is_empty()
                            && canonical::trailing_blanks(&self.document).contains(&id)
                })
            })
    }

    /// What `block` shows as while its lines are edited: the block being edited, kept as one
    /// block of the type it started with while its text has several lines, in place of the
    /// block of its first line, and nothing in place of the blocks of its other lines, which its
    /// text may have split off (OQ-27). Any other block shows as itself.
    pub(super) fn shown_block(&self, block: Block) -> Option<Block> {
        match &self.edit {
            Some(session) if session.lines.len() > 1 && block.id() == session.id => {
                blocks::joined(&self.document, &session.lines, session.kind)
            }
            Some(session) if block.id() != session.id && session.lines.contains(&block.id()) => {
                None
            }
            _ => Some(block),
        }
    }

    /// The type of the block being edited: the type it started with while its text has several
    /// lines, else the type its line has now (OQ-27).
    fn edited_kind(&self) -> Option<BlockKind> {
        let session = self.edit.as_ref()?;
        if session.lines.len() > 1 {
            Some(session.kind)
        } else {
            blocks::block(&self.document, session.id).map(|block| block.kind())
        }
    }

    /// Writes the text being edited to the block's lines in place of what it wrote before,
    /// outside the undo history, so that saving and every view see it at once; with `repair`,
    /// with the lines repaired where they break canonical-outline rule 2 or 4. Lines that already
    /// read so are left alone.
    fn write(&mut self, repair: bool) {
        let Some(session) = self.edit.as_mut() else {
            return;
        };
        let lines = session.block.lines(&session.parts, session.field.text());
        let only_line = lines.len() == 1
            && self.document.descendants(self.document.root()).count() == session.lines.len();
        let last = session.lines.last().and_then(|&id| self.document.get(id));
        let has_children = last.is_some_and(|item| !item.children().is_empty());
        let new_lines = lines.iter().filter(|(from, _)| from.is_none()).count();
        let missing = new_lines.saturating_sub(session.added.len());
        session
            .added
            .extend(std::iter::repeat_with(|| self.document.new_id()).take(missing));
        let mut added = session.added.iter();
        let mut target = Vec::new();
        for (index, (from, text)) in lines.iter().enumerate() {
            let id = match from {
                Some(from) => session.originals.get(*from),
                None => added.next(),
            };
            let id = *id.expect("every line has an id: the one it rewrites or one made for it");
            let text = if repair {
                canonical::repaired(text, has_children && index + 1 == lines.len(), only_line)
            } else {
                text.clone()
            };
            target.push((id, text));
        }
        let written = target.iter().map(|(id, _)| id).eq(&session.lines)
            && target.iter().all(|(id, text)| {
                self.document
                    .get(*id)
                    .is_some_and(|item| item.text() == text)
            });
        if written {
            return;
        }
        let mut revert = Transaction::new(mem::take(&mut session.applied), (), ()).inverse();
        self.document
            .apply(&mut revert)
            .expect("a session's own ops are the latest change to the document");
        let mut transaction = Transaction::new(
            structure::rewrite(&self.document, &session.base, &target),
            (),
            (),
        );
        self.document
            .apply(&mut transaction)
            .expect("the ops are built from the document as it is, with fresh ids for new lines");
        session.applied = transaction.ops().to_vec();
        session.lines = target.into_iter().map(|(id, _)| id).collect();
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
    fn dividers_and_an_empty_outline_are_not_edited() {
        for (text, index) in [("- a\n---", 1), ("", 0)] {
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

    /// Each line as a tab per depth and its text.
    fn lines(state: &State) -> Vec<String> {
        let document = state.document();
        document
            .descendants(document.root())
            .map(|(id, depth)| {
                format!("{}{}", "\t".repeat(depth), document.get(id).unwrap().text())
            })
            .collect()
    }

    fn go(state: &mut State, motion: Motion) {
        state.update(Message::Text(TextEdit::Move {
            motion,
            select: false,
        }));
    }

    fn line_break(state: &mut State) {
        state.update(Message::Text(TextEdit::LineBreak));
    }

    // FR-BLK-03, OQ-23, editing.md §2
    #[test]
    fn a_line_break_adds_a_note_line_written_as_typed() {
        let mut state = open("Plan @x\n- t", 0);
        state.update(Message::EditAtEnd);
        line_break(&mut state);
        assert_eq!(texts(&state), ["Plan @x", "", "- t"]);
        typed(&mut state, "Step two");
        line_break(&mut state);
        typed(&mut state, "Step three");
        assert_eq!(shown(&state), "Plan\nStep two\nStep three|");
        assert_eq!(texts(&state), ["Plan @x", "Step two", "Step three", "- t"]);
        assert_eq!(state.rows()[0].lines, ["Plan", "Step two", "Step three"]);
        assert!(!state.editor().unwrap().splits);
        state.update(Message::StopEditing);
        assert_eq!(state.rows()[0].kind, BlockKind::Note);
        assert_eq!(state.rows()[0].lines.len(), 3);
        assert_eq!(state.rows()[1].lines, ["t"]);
    }

    // OQ-23
    #[test]
    fn only_note_and_code_blocks_take_line_breaks() {
        for text in ["- t", "P:", "S @section", "L @url(https://x.y)"] {
            let mut state = open(text, 0);
            state.update(Message::EditAtEnd);
            let before = shown(&state);
            line_break(&mut state);
            assert_eq!(shown(&state), before, "{text:?}");
            assert_eq!(state.document().revision(), 0, "{text:?}");
        }
        let mut state = open("note", 0);
        state.update(Message::EditAtStart);
        typed(&mut state, "- ");
        line_break(&mut state);
        assert_eq!(shown(&state), "|note", "a note that became a task");
    }

    // FR-BLK-05, OQ-23
    #[test]
    fn enter_breaks_the_line_in_a_code_block_and_esc_ends_it() {
        let mut state = open("f() @code\n- t", 0);
        state.update(Message::EditAtEnd);
        state.update(Message::NewNext);
        typed(&mut state, "g()");
        assert_eq!(state.mode(), Mode::Edit);
        assert_eq!(texts(&state), ["f() @code", "g()", "- t"]);
        state.update(Message::StopEditing);
        assert_eq!(state.rows()[0].kind, BlockKind::Code);
        assert_eq!(state.rows()[0].lines, ["f()", "g()"]);
        assert_eq!(state.rows().len(), 2);
    }

    // FR-BLK-03, editing.md §2
    #[test]
    fn a_block_of_several_lines_is_edited_as_one_text() {
        let mut state = open("one @a\ntwo\n- t", 0);
        state.update(Message::EditAtStart);
        assert_eq!(shown(&state), "|one\ntwo");
        go(&mut state, Motion::Down);
        go(&mut state, Motion::End);
        typed(&mut state, "!");
        assert_eq!(texts(&state), ["one @a", "two!", "- t"]);
        state.update(Message::StopEditing);
        assert_eq!(state.focus(), Some(state.rows()[0].id));
    }

    // editing.md §2, FR-DOC-03
    #[test]
    fn lines_keep_their_tags_and_the_tags_of_removed_lines_move_to_the_first() {
        let mut state = open("a @x\nb @y\nc @z", 0);
        state.update(Message::EditAtEnd);
        go(&mut state, Motion::Up);
        go(&mut state, Motion::Up);
        line_break(&mut state);
        typed(&mut state, "new");
        assert_eq!(texts(&state), ["a @x", "new", "b @y", "c @z"]);
        press(&mut state, TextEdit::DeleteBackward, 4);
        go(&mut state, Motion::Down);
        go(&mut state, Motion::Down);
        go(&mut state, Motion::Start);
        press(&mut state, TextEdit::DeleteBackward, 1);
        assert_eq!(shown(&state), "a\nb|c");
        assert_eq!(texts(&state), ["a @x @z", "bc @y"]);
        state.update(Message::Text(TextEdit::SelectAll));
        typed(&mut state, "z");
        assert_eq!(texts(&state), ["z @x @y @z"]);
    }

    // FR-BLK-03, special-nodes.md §1
    #[test]
    fn the_children_and_special_nodes_follow_the_last_line() {
        let mut state = open("n\nm\n\t- c\n\tC @hidden @comments\n\t\tx\n- t", 0);
        state.update(Message::EditAtEnd);
        line_break(&mut state);
        typed(&mut state, "o");
        assert_eq!(
            lines(&state),
            [
                "n",
                "m",
                "o",
                "\t- c",
                "\tC @hidden @comments",
                "\t\tx",
                "- t"
            ]
        );
        assert_eq!(
            state.rows().len(),
            3,
            "the child stays shown under the block"
        );
        state.update(Message::Text(TextEdit::SelectAll));
        typed(&mut state, "k");
        assert_eq!(
            lines(&state),
            ["k", "\t- c", "\tC @hidden @comments", "\t\tx", "- t"]
        );
        state.update(Message::StopEditing);
        state.update(Message::Undo);
        assert_eq!(
            lines(&state),
            ["n", "m", "\t- c", "\tC @hidden @comments", "\t\tx", "- t"],
            "one step"
        );
        state.update(Message::Redo);
        assert_eq!(lines(&state)[0..2], ["k", "\t- c"]);
    }

    // OQ-27
    #[test]
    fn a_line_of_another_type_splits_the_block_when_editing_ends() {
        let mut state = open("note\nmore\nlast\n\t- c", 0);
        state.update(Message::EditAtEnd);
        go(&mut state, Motion::Up);
        go(&mut state, Motion::Start);
        typed(&mut state, "- ");
        assert_eq!(texts(&state), ["note", "- more", "last", "- c"]);
        assert!(state.editor().unwrap().splits);
        let rows = state.rows();
        assert_eq!(rows.len(), 2, "one edit area until editing ends");
        assert_eq!((rows[0].kind, rows[0].lines.len()), (BlockKind::Note, 3));
        assert_eq!(rows[1].depth, 1);
        state.update(Message::StopEditing);
        let rows = state.rows();
        let kinds: Vec<BlockKind> = rows.iter().map(|row| row.kind).collect();
        assert_eq!(
            kinds,
            [
                BlockKind::Note,
                BlockKind::Task,
                BlockKind::Note,
                BlockKind::Task
            ]
        );
        assert_eq!(state.focus(), Some(rows[1].id), "the block with the caret");
        assert_eq!(rows[3].depth, 1, "the children stay under the last line");
        state.update(Message::Undo);
        assert_eq!(state.rows().len(), 2);
        assert_eq!(state.focus(), Some(state.rows()[0].id));
    }

    // OQ-27
    #[test]
    fn a_first_line_that_becomes_a_project_splits_off_too() {
        let mut state = open("Questions\nwhy", 0);
        state.update(Message::EditAtStart);
        go(&mut state, Motion::End);
        typed(&mut state, ":");
        assert!(state.editor().unwrap().splits);
        assert_eq!(state.rows()[0].kind, BlockKind::Note, "shown as it started");
        line_break(&mut state);
        press(&mut state, TextEdit::DeleteBackward, 1);
        assert_eq!(
            shown(&state),
            "Questions:|\nwhy",
            "still a note while edited"
        );
        assert_eq!(texts(&state), ["Questions:", "why"]);
        state.update(Message::StopEditing);
        let rows = state.rows();
        assert_eq!(
            (rows[0].kind, rows[1].kind),
            (BlockKind::Project, BlockKind::Note)
        );
        assert_eq!(state.focus(), Some(rows[0].id));
    }

    // OQ-27, FR-OPS-02, editing.md §2
    #[test]
    fn tab_on_a_block_that_splits_splits_it_and_moves_the_block_with_the_caret() {
        let mut state = open("- a\nnote\nmore", 1);
        state.update(Message::EditAtStart);
        go(&mut state, Motion::Down);
        typed(&mut state, "- ");
        state.update(Message::Indent);
        assert_eq!(state.mode(), Mode::Edit);
        assert_eq!(shown(&state), "more|");
        line_break(&mut state);
        typed(&mut state, "!");
        state.update(Message::StopEditing);
        assert_eq!(lines(&state), ["- a", "note", "\t- more!"]);
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["- a", "note", "\t- more"]);
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["- a", "note", "- more"]);
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["- a", "note", "more"]);
    }

    // OQ-26, OQ-27, editing.md §2
    #[test]
    fn a_block_split_off_by_tab_that_joins_a_note_is_edited_whole() {
        let mut state = open("- a\n\tn1\nx\ny", 2);
        state.update(Message::EditAtStart);
        go(&mut state, Motion::Down);
        typed(&mut state, "- ");
        go(&mut state, Motion::Up);
        state.update(Message::Indent);
        assert_eq!(lines(&state), ["- a", "\tn1", "\tx", "- y"]);
        assert_eq!(state.mode(), Mode::Edit);
        assert_eq!(shown(&state), "n1\nx|");
    }

    // special-nodes.md §1, FR-DOC-03
    #[test]
    fn a_special_node_between_the_lines_stays_put() {
        let mut state = open("n\nX @hidden\nm", 0);
        state.update(Message::EditAtEnd);
        assert_eq!(shown(&state), "n\nm|");
        typed(&mut state, "!");
        assert_eq!(texts(&state), ["n", "X @hidden", "m!"]);
    }

    // FR-OPS-08, editing.md §2
    #[test]
    fn a_session_of_line_breaks_taken_back_is_no_undo_step() {
        let mut state = open("- a\nn", 0);
        state.update(Message::ToggleDone {
            today: jiff::civil::date(2026, 10, 5),
        });
        state.update(Message::FocusNext);
        state.update(Message::EditAtEnd);
        line_break(&mut state);
        typed(&mut state, "x");
        state.update(Message::Undo);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a @done(2026-10-05)", "n"]);
        state.update(Message::Redo);
        assert_eq!(texts(&state), ["- a @done(2026-10-05)", "n", ""]);
        press(&mut state, TextEdit::DeleteBackward, 1);
        state.update(Message::StopEditing);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a", "n"], "undo skips the empty session");
    }

    // FR-MODE-04, editing.md §2
    #[test]
    fn a_new_note_can_take_lines_and_backspace_still_discards_it() {
        let mut state = open("P:", 0);
        state.update(Message::NewBelow);
        line_break(&mut state);
        assert_eq!(texts(&state), ["P:", "", ""]);
        press(&mut state, TextEdit::DeleteBackward, 2);
        assert_eq!(texts(&state), ["P:"]);
        assert_eq!(state.mode(), Mode::Edit, "back on P");
        state.update(Message::StopEditing);
        let revision = state.document().revision();
        state.update(Message::Undo);
        assert_eq!(state.document().revision(), revision, "nothing to undo");
    }

    // file-format §3.1
    #[test]
    fn ending_the_session_repairs_every_line() {
        let mut state = open("a\nb\n\t- c", 0);
        state.update(Message::EditAtStart);
        go(&mut state, Motion::Down);
        typed(&mut state, " ");
        go(&mut state, Motion::End);
        press(&mut state, TextEdit::DeleteBackward, 2);
        assert_eq!(texts(&state), ["a", "", "- c"]);
        assert!(state.editing_holds_save());
        state.update(Message::StopEditing);
        assert_eq!(texts(&state), ["a", "…", "- c"]);
        let mut state = open("a\nb", 0);
        state.update(Message::EditAtEnd);
        go(&mut state, Motion::Start);
        typed(&mut state, "  ");
        assert!(state.editing_holds_save());
        state.update(Message::StopEditing);
        assert_eq!(texts(&state), ["a", "b"]);
    }

    // FR-OPS-08
    #[test]
    fn a_step_committed_while_lines_are_added_comes_after_them() {
        let mut state = open("n\n- t", 0);
        state.update(Message::EditAtEnd);
        line_break(&mut state);
        typed(&mut state, "m");
        let task = state.rows()[1].id;
        state.commit(vec![Op::SetText {
            id: task,
            before: "- t".to_owned(),
            after: "- u".to_owned(),
        }]);
        typed(&mut state, "o");
        state.update(Message::StopEditing);
        assert_eq!(texts(&state), ["n", "mo", "- u"]);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["n", "m", "- u"]);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["n", "m", "- t"]);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["n", "- t"]);
    }

    // FR-NAV-03, FR-BLK-03
    #[test]
    fn the_zoom_title_keeps_its_children_while_it_splits() {
        let mut state = open("n\nm\n\t- c", 0);
        state.update(Message::ZoomIn);
        state.update(Message::FocusFirst);
        state.update(Message::EditAtEnd);
        go(&mut state, Motion::Start);
        typed(&mut state, "- ");
        let rows = state.rows();
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[0].title, rows[0].lines.len()), (true, 2));
    }
}
