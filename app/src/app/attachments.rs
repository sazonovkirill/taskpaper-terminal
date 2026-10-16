//! The attachments panel: the focused block's note and then its comments, one below the other on
//! a panel that takes the keys while open, and the settings that turn each kind on
//! ([attachments.md §3, §6](../../../docs/specs/attachments.md#3-attachments-panel-fr-att-03),
//! FR-ATT-03, FR-ATT-06, D-94).

use std::{ops::Range, sync::Arc};

use jiff::civil::DateTime;

use super::{
    Access, Command, Notice, State,
    text_field::{TextEdit, TextField},
};
use crate::document::{Attachment, Document, ItemId, attachments, comments, format::breaks_line};

/// A key of the open panel, or opening it, as an intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentsInput {
    /// Open the panel on the focused block with its note selected, or its newest comment while
    /// notes are off (Open Attachments, D-134).
    Open,
    /// Close the panel.
    Close,
    /// Edit the selected note, as raw Markdown, or comment (attachments.md §4, §5).
    Edit,
    /// Change the line with the caret: the note's, or the comment being typed.
    Text(TextEdit),
    /// Break the note's line at the caret.
    Break,
    /// Move the caret to the line above.
    LineUp,
    /// Move the caret to the line below.
    LineDown,
    /// Save the edited note and show it rendered again.
    Save,
    /// Move the selection this many items down, or up when negative, no further than the first
    /// or the last: the note, then each comment.
    Select(isize),
    /// Start typing a new comment.
    AddComment,
    /// Delete the selected comment.
    DeleteComment,
    /// Add the comment being typed, stamped with this local time, or reword the one it edits.
    Accept(DateTime),
    /// Drop the comment being typed.
    Cancel,
}

/// The open panel: the block it is on, the note being edited, if any, the selected item, the
/// note while notes are on and then each comment, which may be past the last after they change,
/// and the comment being typed, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Panel {
    block: ItemId,
    editor: Option<NoteEditor>,
    selected: usize,
    draft: Option<Draft>,
}

/// A comment being typed: the comment it rewords, or `None` for a new one, and its text.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Draft {
    comment: Option<usize>,
    field: TextField,
}

/// A note being edited: its lines, the line the caret is on and that line as a text field, which
/// holds it while the caret is on it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct NoteEditor {
    lines: Vec<String>,
    row: usize,
    field: TextField,
}

impl NoteEditor {
    /// An editor over `lines`, the caret at the end of the last.
    fn new(mut lines: Vec<String>) -> Self {
        if lines.is_empty() {
            lines.push(String::new());
        }
        let row = lines.len() - 1;
        let field = TextField::new(lines.last().cloned().unwrap_or_default());
        Self { lines, row, field }
    }

    /// Every line, the caret's as the field has it.
    fn text(&self) -> Vec<String> {
        let mut lines = self.lines.clone();
        if let Some(line) = lines.get_mut(self.row) {
            self.field.text().clone_into(line);
        }
        lines
    }

    /// Moves the caret to line `row`, at the same column or the end of a shorter line.
    fn go_to(&mut self, row: usize) {
        let Some(line) = self.lines.get(row) else {
            return;
        };
        let column = self.field.column();
        let field = TextField::at_column(line.clone(), column);
        self.leave_row();
        self.row = row;
        self.field = field;
    }

    /// Puts the field's text back on its line.
    fn leave_row(&mut self) {
        if let Some(line) = self.lines.get_mut(self.row) {
            self.field.text().clone_into(line);
        }
    }

    /// Applies `edit`: deleting back from the start of a line joins it to the one above, and
    /// forward from the end joins the one below to it.
    fn edit(&mut self, edit: TextEdit) {
        let field = &self.field;
        let (start, end) = (field.caret() == 0, field.caret() == field.text().len());
        let nothing_selected = field.selection().is_none();
        match edit {
            TextEdit::DeleteBackward | TextEdit::DeleteWordBackward
                if start && nothing_selected && self.row > 0 =>
            {
                self.join(self.row - 1);
            }
            TextEdit::DeleteForward
                if end && nothing_selected && self.row + 1 < self.lines.len() =>
            {
                self.join(self.row);
            }
            _ => self.field = self.field.edited(edit),
        }
    }

    /// Joins line `row + 1` to line `row`, the caret between them.
    fn join(&mut self, row: usize) {
        self.leave_row();
        let next = self.lines.remove(row + 1);
        let line = self
            .lines
            .get_mut(row)
            .map(std::mem::take)
            .unwrap_or_default();
        let caret = line.len();
        self.row = row;
        self.field = TextField::with_caret(line + &next, caret);
    }

    /// Breaks the line at the caret, taking out the selection, the caret at the start of the
    /// new line.
    fn break_line(&mut self) {
        let field = if self.field.selection().is_some() {
            self.field.edited(TextEdit::DeleteBackward)
        } else {
            self.field.clone()
        };
        let (before, after) = field.text().split_at(field.caret());
        let after = after.to_owned();
        if let Some(line) = self.lines.get_mut(self.row) {
            before.clone_into(line);
        }
        self.row += 1;
        self.lines.insert(self.row, String::new());
        self.field = TextField::with_caret(after, 0);
    }

    /// Pastes `text`, each line break in it breaking the line.
    fn paste(&mut self, text: &str) {
        let text = text.replace("\r\n", "\n");
        for (at, piece) in text.split(breaks_line).enumerate() {
            if at > 0 {
                self.break_line();
            }
            self.field = self.field.pasted(piece);
        }
    }
}

/// The note being edited as a frontend shows it: the line with the caret and where on it the
/// caret and the selection are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteCaret {
    /// The line the caret is on.
    pub row: usize,
    /// The caret's byte offset into that line.
    pub caret: usize,
    /// The selected bytes of that line, if any.
    pub selection: Option<Range<usize>>,
}

/// The open panel as a frontend shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentsView<'a> {
    /// The block's text without its formatting, which the panel's title names.
    pub title: Arc<str>,
    /// Its sections, one per kind turned on, in order: the note, then the comments.
    pub sections: &'a [Attachment],
    /// The note's lines, as the user wrote them, or as the editor has them while editing.
    pub note: Vec<String>,
    /// The comments' lines, as the file has them.
    pub comments: Vec<String>,
    /// Where the caret is while the note is edited; `None` while it is shown rendered.
    pub editing: Option<NoteCaret>,
    /// The selected item, if there is any.
    pub selected: Option<PanelItem>,
    /// The comment being typed, if any.
    pub draft: Option<CommentDraft>,
}

/// An item of the panel that can be selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelItem {
    /// The note, whether or not the block has one yet.
    Note,
    /// The comment at this index, newest first.
    Comment(usize),
}

/// A comment being typed as a frontend shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentDraft {
    /// The comment it rewords, or `None` for a new one.
    pub comment: Option<usize>,
    /// Its text, without a timestamp.
    pub text: String,
    /// The caret's byte offset into the text.
    pub caret: usize,
    /// The selected bytes of the text, if any.
    pub selection: Option<Range<usize>>,
}

impl State {
    /// The kinds of attachment turned on, in order (`attachments_notes`,
    /// `attachments_comments`, attachments.md §6).
    #[must_use]
    pub fn attachments_shown(&self) -> &[Attachment] {
        &self.attached
    }

    /// Whether the app offers `command` in its lists of commands: an attachment's only while its
    /// kind is turned on (attachments.md §6).
    pub(super) fn offers(&self, command: Command) -> bool {
        match command {
            Command::OpenAttachments => !self.attached.is_empty(),
            _ => true,
        }
    }

    /// The attachments panel, while it is open (FR-ATT-03).
    #[must_use]
    pub fn attachments_panel(&self) -> Option<AttachmentsView<'_>> {
        let panel = self.panel.as_ref()?;
        let editing = panel.editor.as_ref().map(|editor| NoteCaret {
            row: editor.row,
            caret: editor.field.caret(),
            selection: editor.field.selection(),
        });
        let draft = panel.draft.as_ref().map(|draft| CommentDraft {
            comment: draft.comment,
            text: draft.field.text().to_owned(),
            caret: draft.field.caret(),
            selection: draft.field.selection(),
        });
        let content = |kind| {
            if self.attached.contains(&kind) {
                attachments::content(&self.document, panel.block, kind)
            } else {
                Vec::new()
            }
        };
        Some(AttachmentsView {
            title: self.document.get(panel.block)?.shared_plain_text(),
            sections: &self.attached,
            note: panel
                .editor
                .as_ref()
                .map_or_else(|| content(Attachment::Note), NoteEditor::text),
            comments: content(Attachment::Comments),
            editing,
            selected: self.selected_item(),
            draft,
        })
    }

    /// Whether the panel shows a note item before the comments.
    fn notes_on(&self) -> bool {
        self.attached.contains(&Attachment::Note)
    }

    /// The selected item, if the panel has any: the note while notes are on, then each comment
    /// while comments are.
    fn selected_item(&self) -> Option<PanelItem> {
        let panel = self.panel.as_ref()?;
        let note = usize::from(self.notes_on());
        let comments = if self.attached.contains(&Attachment::Comments) {
            attachments::count(&self.document, panel.block, Attachment::Comments)
        } else {
            0
        };
        let selected = panel.selected.min((note + comments).checked_sub(1)?);
        Some(match selected.checked_sub(note) {
            Some(comment) => PanelItem::Comment(comment),
            None => PanelItem::Note,
        })
    }

    /// The selected comment, if a comment is selected.
    fn selected_comment(&self) -> Option<usize> {
        match self.selected_item()? {
            PanelItem::Comment(comment) => Some(comment),
            PanelItem::Note => None,
        }
    }

    /// Selects the newest comment, the first after the note.
    fn select_newest_comment(&mut self) {
        let note = usize::from(self.notes_on());
        if let Some(panel) = &mut self.panel {
            panel.selected = note;
        }
    }

    /// Applies `input`: opens the panel on the focused block, or moves through, edits or closes the
    /// open one.
    pub(super) fn attachments_input(&mut self, input: AttachmentsInput) {
        match input {
            AttachmentsInput::Open => self.open_panel(),
            AttachmentsInput::Close => {
                self.save_note();
                self.panel = None;
                self.resume_editing();
            }
            AttachmentsInput::Edit => {
                self.edit_note();
                self.edit_comment();
            }
            AttachmentsInput::Save => self.save_note(),
            AttachmentsInput::Text(edit) => {
                self.note_editor(|editor| editor.edit(edit));
                if let Some(draft) = self.draft() {
                    draft.field = draft.field.edited(edit);
                }
            }
            AttachmentsInput::Break => self.note_editor(NoteEditor::break_line),
            AttachmentsInput::LineUp => self.note_editor(|editor| {
                if let Some(row) = editor.row.checked_sub(1) {
                    editor.go_to(row);
                }
            }),
            AttachmentsInput::LineDown => self.note_editor(|editor| editor.go_to(editor.row + 1)),
            AttachmentsInput::Select(by) => {
                let note = usize::from(self.notes_on());
                if let Some(selected) = self.selected_item()
                    && let Some(panel) = &mut self.panel
                    && panel.draft.is_none()
                    && panel.editor.is_none()
                {
                    let at = match selected {
                        PanelItem::Note => 0,
                        PanelItem::Comment(comment) => note + comment,
                    };
                    panel.selected = at.saturating_add_signed(by);
                }
            }
            AttachmentsInput::AddComment => self.start_comment(None),
            AttachmentsInput::DeleteComment => self.delete_comment(),
            AttachmentsInput::Accept(at) => self.accept_comment(at),
            AttachmentsInput::Cancel => {
                if let Some(panel) = &mut self.panel {
                    panel.draft = None;
                }
            }
        }
    }

    /// The comment being typed, if one is.
    fn draft(&mut self) -> Option<&mut Draft> {
        self.panel.as_mut()?.draft.as_mut()
    }

    /// Starts typing a comment while comments are on and nothing else is typed: a new one, or
    /// rewording comment `comment`, its text without its timestamp.
    fn start_comment(&mut self, comment: Option<usize>) {
        let Some(panel) = &self.panel else {
            return;
        };
        if !self.attached.contains(&Attachment::Comments)
            || panel.draft.is_some()
            || panel.editor.is_some()
        {
            return;
        }
        if self.access() == Access::ReadOnly {
            self.notify(Notice::ReadOnly);
            return;
        }
        let text = comment
            .and_then(|at| {
                attachments::content(&self.document, panel.block, Attachment::Comments)
                    .get(at)
                    .map(|line| comments::comment(line).text.to_owned())
            })
            .unwrap_or_default();
        if let Some(panel) = &mut self.panel {
            panel.draft = Some(Draft {
                comment,
                field: TextField::new(text),
            });
        }
    }

    /// Starts rewording the selected comment, if a comment is selected.
    fn edit_comment(&mut self) {
        if let Some(selected) = self.selected_comment() {
            self.start_comment(Some(selected));
        }
    }

    /// Adds the comment being typed at the top, stamped `at`, and selects it, or rewords the one
    /// it edits keeping its timestamp, as one step to undo; text of only spaces changes nothing
    /// (attachments.md §5).
    fn accept_comment(&mut self, at: DateTime) {
        let Some(panel) = &mut self.panel else {
            return;
        };
        let Some(draft) = panel.draft.take() else {
            return;
        };
        let text = draft.field.text();
        if text.trim().is_empty() {
            return;
        }
        let block = panel.block;
        let mut lines = attachments::content(&self.document, block, Attachment::Comments);
        match draft.comment {
            None => {
                lines.insert(0, comments::stamped(at, text));
                self.select_newest_comment();
            }
            Some(comment) => {
                if let Some(line) = lines.get_mut(comment) {
                    *line = comments::reworded(line, text);
                }
            }
        }
        self.write_attachment(block, Attachment::Comments, &lines);
    }

    /// Deletes the selected comment as one step to undo, and with the last one its node
    /// (attachments.md §5).
    fn delete_comment(&mut self) {
        let Some(panel) = &self.panel else {
            return;
        };
        if panel.draft.is_some() {
            return;
        }
        let block = panel.block;
        let Some(selected) = self.selected_comment() else {
            return;
        };
        let mut lines = attachments::content(&self.document, block, Attachment::Comments);
        lines.remove(selected);
        self.write_attachment(block, Attachment::Comments, &lines);
    }

    /// Makes `lines` the content of the block `block`'s attachment of kind `kind` as one step to
    /// undo.
    fn write_attachment(&mut self, block: ItemId, kind: Attachment, lines: &[String]) {
        if let Some(ops) = attachments::write(&mut self.document, block, kind, lines) {
            self.commit_any(ops);
        }
    }

    /// Starts editing the note, when it is selected, at the end of its last line.
    fn edit_note(&mut self) {
        let Some(panel) = &self.panel else {
            return;
        };
        if self.selected_item() != Some(PanelItem::Note)
            || panel.editor.is_some()
            || panel.draft.is_some()
        {
            return;
        }
        if self.access() == Access::ReadOnly {
            self.notify(Notice::ReadOnly);
            return;
        }
        let lines = attachments::content(&self.document, panel.block, Attachment::Note);
        if let Some(panel) = &mut self.panel {
            panel.editor = Some(NoteEditor::new(lines));
        }
    }

    /// Changes the note being edited with `change`, if one is.
    fn note_editor(&mut self, change: impl FnOnce(&mut NoteEditor)) {
        if let Some(editor) = self.panel.as_mut().and_then(|panel| panel.editor.as_mut()) {
            change(editor);
        }
    }

    /// Pastes `text` into the note being edited, as [`State::note_lines`] gives it, or the comment
    /// being typed, as [`State::one_line`] gives it, if one is; returns whether it took it.
    pub(super) fn attachments_paste(&mut self, text: &str) -> bool {
        let line = self.one_line(text);
        if let Some(draft) = self.draft() {
            draft.field = draft.field.pasted(&line);
            return true;
        }
        let editing = self
            .panel
            .as_ref()
            .is_some_and(|panel| panel.editor.is_some());
        let lines = self.note_lines(text);
        self.note_editor(|editor| editor.paste(&lines));
        editing
    }

    /// Writes the note being edited, if one is, to its block as one step to undo, deleting the
    /// node when it is empty, and shows it rendered again (attachments.md §4).
    pub(super) fn save_note(&mut self) {
        let Some(panel) = &mut self.panel else {
            return;
        };
        let Some(editor) = panel.editor.take() else {
            return;
        };
        let block = panel.block;
        self.write_attachment(block, Attachment::Note, &editor.text());
    }

    /// Opens the panel for the focused block, a divider aside, which has none, while some kind is
    /// turned on, with its first item selected: the note, or the newest comment while notes are
    /// off (attachments.md §3, §6).
    fn open_panel(&mut self) {
        let Some((block, _)) = self.focused() else {
            return;
        };
        if !self.attached.is_empty() && !block.divider() {
            self.panel = Some(Panel {
                block: block.id(),
                editor: None,
                selected: 0,
                draft: None,
            });
        }
    }

    /// Turns the attachments of kind `kind` on or off; the open panel loses that section, saving
    /// the note it edits or dropping the comment being typed, and closes with none left
    /// (attachments.md §6).
    pub(super) fn show_attachments(&mut self, kind: Attachment, on: bool) {
        self.attached = Attachment::ALL
            .into_iter()
            .filter(|&other| {
                if other == kind {
                    on
                } else {
                    self.attached.contains(&other)
                }
            })
            .collect();
        if on || self.panel.is_none() {
            return;
        }
        match kind {
            Attachment::Note => self.save_note(),
            Attachment::Comments => {
                if let Some(panel) = &mut self.panel {
                    panel.draft = None;
                }
            }
        }
        if self.attached.is_empty() {
            self.attachments_input(AttachmentsInput::Close);
        }
    }

    /// The panel on the block of a reloaded document that `exact` finds for its own, closed when
    /// it finds none (D-104); the selected and reworded comments are found again among the
    /// block's comments in `old`, the document before, as the same line, and a rewording of one
    /// that is gone ends, so it never lands on another (D-110).
    pub(super) fn rematch_panel(
        &mut self,
        old: &Document,
        exact: impl Fn(ItemId) -> Option<ItemId>,
    ) {
        let Some(mut panel) = self.panel.take() else {
            return;
        };
        let Some(block) = exact(panel.block) else {
            return;
        };
        let before = attachments::content(old, panel.block, Attachment::Comments);
        let after = attachments::content(&self.document, block, Attachment::Comments);
        let note = usize::from(self.notes_on());
        if let Some(comment) = panel.selected.checked_sub(note)
            && let Some(found) = same_comment(&before, &after, comment)
        {
            panel.selected = note + found;
        }
        if let Some(comment) = panel.draft.as_ref().and_then(|draft| draft.comment) {
            let found = same_comment(&before, &after, comment);
            panel.draft = (panel.draft.take())
                .filter(|_| found.is_some())
                .map(|draft| Draft {
                    comment: found,
                    ..draft
                });
        }
        self.panel = Some(Panel { block, ..panel });
    }
}

/// Where comment `at` of `before` is in `after`: the same line, counting the identical lines
/// before it, if it is still there.
fn same_comment(before: &[String], after: &[String], at: usize) -> Option<usize> {
    let line = before.get(at)?;
    let rank = before
        .iter()
        .take(at)
        .filter(|other| *other == line)
        .count();
    after
        .iter()
        .enumerate()
        .filter(|(_, other)| *other == line)
        .nth(rank)
        .map(|(found, _)| found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use super::super::text_field::Motion;
    use crate::{
        app::{
            Access, Configuration, Menu, MenuInput, Message, Mode, NodeInput, Notice, Paste,
            SelectionChange,
        },
        document::format,
    };

    const TEXT: &str = "- Milk\n\tNOTE @hidden\n\t\tFull fat\n\tCOMMENTS @hidden\n\t\t[2026-10-08 09:00] ok\n---\n";

    fn state() -> State {
        State::new(format::parse(TEXT), None)
    }

    fn input(state: &mut State, input: AttachmentsInput) {
        state.update(Message::Attachments(input));
    }

    fn configure(state: &mut State, kind: Attachment, on: bool) {
        state.update(Message::Configure(Configuration::Attachments(kind, on)));
    }

    /// The open panel's sections and selected item.
    fn sections(state: &State) -> Option<(Vec<Attachment>, Option<PanelItem>)> {
        state
            .attachments_panel()
            .map(|view| (view.sections.to_vec(), view.selected))
    }

    // FR-ATT-03, attachments.md §3
    #[test]
    fn the_panel_shows_the_note_then_the_comments_with_the_item_it_opened_on_selected() {
        let mut state = state();
        assert_eq!(state.attachments_panel(), None);
        input(&mut state, AttachmentsInput::Open);
        let view = state.attachments_panel().unwrap();
        assert_eq!(&*view.title, "Milk");
        assert_eq!(view.sections, Attachment::ALL);
        assert_eq!(view.note, ["Full fat"]);
        assert_eq!(view.comments, ["[2026-10-08 09:00] ok"]);
        assert_eq!(view.selected, Some(PanelItem::Note));
        input(&mut state, AttachmentsInput::Close);
        assert_eq!(state.attachments_panel(), None);
        input(&mut state, AttachmentsInput::Open);
        for (by, selected) in [
            (1, PanelItem::Comment(0)),
            (1, PanelItem::Comment(0)),
            (-1, PanelItem::Note),
            (-1, PanelItem::Note),
            (isize::MAX, PanelItem::Comment(0)),
        ] {
            input(&mut state, AttachmentsInput::Select(by));
            assert_eq!(sections(&state).unwrap().1, Some(selected), "{by}");
        }
    }

    // FR-ATT-03, FR-ATT-06, FR-BLK-07, attachments.md §1, §3, §6, D-134
    #[test]
    fn the_panel_opens_only_on_a_block_that_is_no_divider_while_a_kind_is_turned_on() {
        let mut state = state();
        configure(&mut state, Attachment::Note, false);
        input(&mut state, AttachmentsInput::Open);
        assert_eq!(
            sections(&state),
            Some((vec![Attachment::Comments], Some(PanelItem::Comment(0)))),
            "notes are off, so the newest comment is selected"
        );
        input(&mut state, AttachmentsInput::Close);
        configure(&mut state, Attachment::Comments, false);
        input(&mut state, AttachmentsInput::Open);
        assert_eq!(state.attachments_panel(), None, "both are off");
        configure(&mut state, Attachment::Note, true);
        state.update(Message::FocusLast);
        input(&mut state, AttachmentsInput::Open);
        assert_eq!(state.attachments_panel(), None, "a divider");
        let mut empty = State::new(format::parse(""), None);
        input(&mut empty, AttachmentsInput::Open);
        assert_eq!(empty.attachments_panel(), None, "no block");
    }

    // FR-ATT-06, attachments.md §6
    #[test]
    fn turning_a_kind_off_takes_its_section_away_and_both_close_the_panel() {
        let mut state = state();
        input(&mut state, AttachmentsInput::Open);
        configure(&mut state, Attachment::Comments, false);
        let view = state.attachments_panel().unwrap();
        assert_eq!(
            (view.sections, view.comments.len()),
            ([Attachment::Note].as_slice(), 0)
        );
        configure(&mut state, Attachment::Comments, true);
        configure(&mut state, Attachment::Note, false);
        let view = state.attachments_panel().unwrap();
        assert_eq!(
            (view.sections, view.note.len(), view.selected),
            (
                [Attachment::Comments].as_slice(),
                0,
                Some(PanelItem::Comment(0))
            )
        );
        configure(&mut state, Attachment::Note, false);
        assert!(state.attachments_panel().is_some(), "already off");
        configure(&mut state, Attachment::Comments, false);
        assert_eq!(state.attachments_panel(), None);
        assert_eq!(state.attachments_shown(), []);
        configure(&mut state, Attachment::Comments, true);
        configure(&mut state, Attachment::Note, true);
        assert_eq!(state.attachments_shown(), Attachment::ALL, "in order");
    }

    // FR-ATT-06, FR-NAV-06, FR-KEY-04, attachments.md §6
    #[test]
    fn open_attachments_leaves_go_to_command_and_the_shortcuts_panel_with_both_kinds_off() {
        let mut state = state();
        let open = Command::OpenAttachments;
        let listed = |state: &mut State| {
            state.update(Message::OpenGoToCommand(Vec::new()));
            let names: Vec<String> = state
                .go_to_command()
                .unwrap()
                .entries
                .iter()
                .map(|entry| entry.text.to_string())
                .collect();
            state.update(Message::Menu(Menu::GoToCommand, MenuInput::Dismiss));
            names
        };
        let shortcuts = |state: &State| -> Vec<Command> {
            state
                .shortcuts()
                .unwrap()
                .into_iter()
                .flat_map(|(_, entries)| entries)
                .map(|entry| entry.command)
                .collect()
        };
        state.update(Message::ToggleShortcuts);
        configure(&mut state, Attachment::Note, false);
        assert!(listed(&mut state).contains(&"Open Attachments".to_owned()));
        assert!(shortcuts(&state).contains(&open));
        configure(&mut state, Attachment::Comments, false);
        assert!(!listed(&mut state).contains(&"Open Attachments".to_owned()));
        assert!(!shortcuts(&state).contains(&open));
        assert!(state.offers(Command::EditNode));
    }

    // FR-ATT-03, D-50, editing.md §2.3
    #[test]
    fn the_panel_the_command_menu_opens_returns_to_editing_when_closed() {
        let mut state = state();
        state.update(Message::EditAtEnd);
        state.update(Message::OpenCommandMenu {
            today: jiff::civil::date(2026, 10, 5),
        });
        for c in "open attachments".chars() {
            state.update(Message::Menu(Menu::CommandMenu, MenuInput::Type(c)));
        }
        state.update(Message::Menu(Menu::CommandMenu, MenuInput::Choose));
        assert_eq!(sections(&state).unwrap().1, Some(PanelItem::Note));
        assert_eq!(state.mode(), Mode::Navigate, "the panel takes the keys");
        input(&mut state, AttachmentsInput::Close);
        assert_eq!(state.mode(), Mode::Edit);
    }

    /// The document as the file has it.
    fn file(state: &State) -> String {
        format::serialize(state.document())
    }

    fn keys(state: &mut State, inputs: &[AttachmentsInput]) {
        for &each in inputs {
            input(state, each);
        }
    }

    fn typed(state: &mut State, text: &str) {
        for c in text.chars() {
            input(state, AttachmentsInput::Text(TextEdit::Insert(c)));
        }
    }

    /// The note being edited and where its caret is.
    fn edited(state: &State) -> (Vec<String>, Option<(usize, usize)>) {
        let view = state.attachments_panel().unwrap();
        let caret = view.editing.map(|caret| (caret.row, caret.caret));
        (view.note, caret)
    }

    // FR-ATT-04, attachments.md §4
    #[test]
    fn the_note_editor_breaks_joins_and_moves_between_lines() {
        let mut state = state();
        keys(
            &mut state,
            &[AttachmentsInput::Open, AttachmentsInput::Edit],
        );
        assert_eq!(edited(&state), (vec!["Full fat".to_owned()], Some((0, 8))));
        keys(
            &mut state,
            &[AttachmentsInput::Select(1), AttachmentsInput::AddComment],
        );
        let view = state.attachments_panel().unwrap();
        assert_eq!(
            (view.selected, view.draft),
            (Some(PanelItem::Note), None),
            "editing keeps the selection"
        );
        typed(&mut state, "!");
        keys(&mut state, &[AttachmentsInput::Break]);
        typed(&mut state, "- x");
        assert_eq!(
            edited(&state),
            (vec!["Full fat!".to_owned(), "- x".to_owned()], Some((1, 3)))
        );
        keys(
            &mut state,
            &[AttachmentsInput::LineUp, AttachmentsInput::LineUp],
        );
        assert_eq!(edited(&state).1, Some((0, 3)), "the same column");
        keys(&mut state, &[AttachmentsInput::Break]);
        assert_eq!(edited(&state).0, ["Ful", "l fat!", "- x"]);
        keys(
            &mut state,
            &[AttachmentsInput::Text(TextEdit::DeleteBackward)],
        );
        assert_eq!(
            edited(&state),
            (vec!["Full fat!".to_owned(), "- x".to_owned()], Some((0, 3)))
        );
        let end = TextEdit::Move {
            motion: Motion::End,
            select: false,
        };
        keys(
            &mut state,
            &[
                AttachmentsInput::Text(end),
                AttachmentsInput::Text(TextEdit::DeleteForward),
            ],
        );
        assert_eq!(
            edited(&state),
            (vec!["Full fat!- x".to_owned()], Some((0, 9)))
        );
        keys(
            &mut state,
            &[
                AttachmentsInput::LineDown,
                AttachmentsInput::Text(end),
                AttachmentsInput::Text(TextEdit::DeleteForward),
            ],
        );
        assert_eq!(
            edited(&state).0,
            ["Full fat!- x"],
            "the last line joins nothing"
        );
        keys(
            &mut state,
            &[
                AttachmentsInput::Text(TextEdit::Move {
                    motion: Motion::Start,
                    select: false,
                }),
                AttachmentsInput::Text(TextEdit::DeleteBackward),
                AttachmentsInput::Text(TextEdit::Move {
                    motion: Motion::End,
                    select: true,
                }),
                AttachmentsInput::Break,
            ],
        );
        assert_eq!(
            edited(&state),
            (vec![String::new(), String::new()], Some((1, 0))),
            "the selection goes"
        );
        input(
            &mut state,
            AttachmentsInput::Text(TextEdit::DeleteWordBackward),
        );
        assert_eq!(edited(&state), (vec![String::new()], Some((0, 0))));
    }

    // FR-ATT-04, FR-OPS-08, attachments.md §3, §4
    #[test]
    fn saving_the_note_is_one_step_to_undo_and_an_empty_note_deletes_its_node() {
        let mut state = state();
        keys(
            &mut state,
            &[AttachmentsInput::Open, AttachmentsInput::Edit],
        );
        typed(&mut state, " milk");
        keys(&mut state, &[AttachmentsInput::Break]);
        typed(&mut state, "- not a task");
        input(&mut state, AttachmentsInput::Save);
        assert_eq!(
            file(&state),
            "- Milk\n\tNOTE @hidden\n\t\tFull fat milk\n\t\t\u{200B}- not a task\n\tCOMMENTS @hidden\n\t\t[2026-10-08 09:00] ok\n---\n"
        );
        let view = state.attachments_panel().unwrap();
        assert_eq!((view.editing, view.note.len()), (None, 2), "shown again");
        input(&mut state, AttachmentsInput::Close);
        state.update(Message::Undo);
        assert_eq!(file(&state), TEXT, "one step");
        keys(
            &mut state,
            &[AttachmentsInput::Open, AttachmentsInput::Edit],
        );
        keys(
            &mut state,
            &[
                AttachmentsInput::Text(TextEdit::SelectAll),
                AttachmentsInput::Text(TextEdit::DeleteBackward),
            ],
        );
        input(&mut state, AttachmentsInput::Close);
        assert_eq!(state.attachments_panel(), None, "closing saves");
        assert_eq!(
            file(&state),
            "- Milk\n\tCOMMENTS @hidden\n\t\t[2026-10-08 09:00] ok\n---\n"
        );
        let revision = state.document().revision();
        keys(
            &mut state,
            &[
                AttachmentsInput::Open,
                AttachmentsInput::Edit,
                AttachmentsInput::Save,
            ],
        );
        assert_eq!(state.document().revision(), revision, "nothing to save");
    }

    // FR-ATT-04, FR-OPS-06, attachments.md §4
    #[test]
    fn a_paste_breaks_the_note_where_the_text_does() {
        let mut state = state();
        keys(
            &mut state,
            &[AttachmentsInput::Open, AttachmentsInput::Edit],
        );
        state.update(Message::Pasted {
            text: "a\r\nb\nc".to_owned(),
            at: Paste::Below,
        });
        assert_eq!(
            edited(&state),
            (
                vec!["Full fata".to_owned(), "b".to_owned(), "c".to_owned()],
                Some((2, 1))
            )
        );
        input(&mut state, AttachmentsInput::Save);
        state.update(Message::Pasted {
            text: "x".to_owned(),
            at: Paste::Below,
        });
        assert_eq!(edited(&state).1, None, "the shown note takes no paste");
    }

    // FR-ATT-04, FR-OPS-06, attachments.md §4
    #[test]
    fn a_copied_block_pastes_into_the_note_as_its_text() {
        let mut state = State::new(
            format::parse("- Milk @due(2026-10-12)\n\tNOTE @hidden\n\t\tFull fat\n- Eggs\n"),
            None,
        );
        state.update(Message::Copy);
        keys(
            &mut state,
            &[AttachmentsInput::Open, AttachmentsInput::Edit],
        );
        state.update(Message::Pasted {
            text: "- Milk @due(2026-10-12)\r\n\tNOTE @hidden\r\n\t\tFull fat\r\n".to_owned(),
            at: Paste::Below,
        });
        assert_eq!(
            edited(&state),
            (vec!["Full fatMilk".to_owned()], Some((0, 12))),
            "the copied block's text, without its tags and attachments"
        );
    }

    // FR-ATT-04, FR-OPS-06, attachments.md §4
    #[test]
    fn several_copied_blocks_paste_into_the_note_one_line_each() {
        let mut state = State::new(format::parse("- Milk\n\t- Skim\n- Eggs\n- Bread\n"), None);
        state.update(Message::Select(SelectionChange::ExtendDown));
        state.update(Message::Select(SelectionChange::ExtendDown));
        state.update(Message::Copy);
        state.update(Message::FocusLast);
        keys(
            &mut state,
            &[AttachmentsInput::Open, AttachmentsInput::Edit],
        );
        state.update(Message::Pasted {
            text: "- Milk\n\t- Skim\n- Eggs\n".to_owned(),
            at: Paste::Below,
        });
        assert_eq!(
            edited(&state),
            (vec!["Milk".to_owned(), "Eggs".to_owned()], Some((1, 4)))
        );
    }

    // FR-ATT-04, attachments.md §4
    #[test]
    fn only_the_selected_note_edits_and_its_keys_need_an_editor() {
        let mut state = state();
        keys(
            &mut state,
            &[
                AttachmentsInput::Open,
                AttachmentsInput::Select(1),
                AttachmentsInput::Edit,
            ],
        );
        assert_eq!(edited(&state).1, None);
        typed(&mut state, "x");
        keys(
            &mut state,
            &[
                AttachmentsInput::Break,
                AttachmentsInput::LineUp,
                AttachmentsInput::LineDown,
                AttachmentsInput::Save,
            ],
        );
        assert_eq!(file(&state), TEXT);
        input(&mut state, AttachmentsInput::Edit);
        input(&mut state, AttachmentsInput::Close);
        input(&mut state, AttachmentsInput::Edit);
        assert_eq!(state.attachments_panel(), None);
        keys(
            &mut state,
            &[AttachmentsInput::Open, AttachmentsInput::Edit],
        );
        typed(&mut state, "!");
        input(&mut state, AttachmentsInput::Edit);
        assert_eq!(edited(&state).0, ["Full fat!"], "already editing");
    }

    // FR-ATT-04, FR-ATT-06, attachments.md §4, §6
    #[test]
    fn turning_notes_off_saves_the_note_being_edited() {
        let mut state = state();
        keys(
            &mut state,
            &[AttachmentsInput::Open, AttachmentsInput::Edit],
        );
        typed(&mut state, "!");
        configure(&mut state, Attachment::Note, false);
        assert!(file(&state).contains("Full fat!"));
        let view = state.attachments_panel().unwrap();
        assert_eq!(
            (view.sections, view.editing.is_some(), view.comments),
            (
                &[Attachment::Comments][..],
                false,
                vec!["[2026-10-08 09:00] ok".to_owned()]
            )
        );
    }

    // FR-ATT-04, file-io.md §1, §2
    #[test]
    fn quitting_saves_the_note_and_a_read_only_file_refuses_it() {
        for quit in [Message::Quit, Message::Terminate] {
            let mut state = state();
            keys(
                &mut state,
                &[AttachmentsInput::Open, AttachmentsInput::Edit],
            );
            typed(&mut state, "!");
            state.update(quit);
            assert!(file(&state).contains("Full fat!"));
        }
        let read_only = |selected: isize, start: AttachmentsInput| {
            let mut state = State::new(format::parse(TEXT), Some(PathBuf::from("/a.taskpaper")))
                .with_access(Access::ReadOnly);
            keys(
                &mut state,
                &[
                    AttachmentsInput::Open,
                    AttachmentsInput::Select(selected),
                    start,
                ],
            );
            state
        };
        for (selected, start) in [
            (0, AttachmentsInput::Edit),
            (1, AttachmentsInput::Edit),
            (0, AttachmentsInput::AddComment),
        ] {
            let state = read_only(selected, start);
            assert_eq!(state.notice(), Some(&Notice::ReadOnly), "{start:?}");
            let view = state.attachments_panel().unwrap();
            assert!(view.editing.is_none() && view.draft.is_none(), "{start:?}");
        }
        let mut state = read_only(0, AttachmentsInput::Select(0));
        assert_eq!(state.notice(), None, "moving is no edit");
        state.update(Message::Attachments(AttachmentsInput::Close));
        assert_eq!(file(&state), TEXT);
    }

    // FR-ATT-06, FR-SPC-01, editing.md §7
    #[test]
    fn the_edit_node_dialog_lists_attachments_only_while_their_kind_is_off() {
        let mut state = state();
        let hidden = |state: &mut State| {
            state.update(Message::NodeEditor(NodeInput::Open));
            let hidden = state.node_editor().unwrap().hidden.len();
            state.update(Message::NodeEditor(NodeInput::Cancel));
            hidden
        };
        assert_eq!(hidden(&mut state), 0);
        configure(&mut state, Attachment::Comments, false);
        assert_eq!(hidden(&mut state), 1);
    }

    const AT: DateTime = jiff::civil::date(2026, 10, 9).at(7, 45, 30, 0);

    /// The comments shown, the selected one and the one being typed: the comment it rewords and
    /// its text.
    type Listed = (Vec<String>, Option<usize>, Option<(Option<usize>, String)>);

    fn listed(state: &State) -> Listed {
        let view = state.attachments_panel().unwrap();
        let draft = view.draft.map(|draft| (draft.comment, draft.text));
        let selected = match view.selected {
            Some(PanelItem::Comment(comment)) => Some(comment),
            _ => None,
        };
        (view.comments, selected, draft)
    }

    const COMMENTS: &str = "- Milk\n\tCOMMENTS @hidden\n\t\t[2026-10-08 09:00] b\n\t\tby hand\n\t\t[2026-10-07 09:00] a\n";

    // FR-ATT-05, attachments.md §5
    #[test]
    fn the_selection_moves_within_the_comments() {
        let mut state = State::new(format::parse(COMMENTS), None);
        keys(
            &mut state,
            &[AttachmentsInput::Open, AttachmentsInput::Select(1)],
        );
        assert_eq!(listed(&state).1, Some(0));
        for (by, selected) in [
            (1, Some(1)),
            (isize::MAX, Some(2)),
            (-1, Some(1)),
            (isize::MIN, None),
            (1, Some(0)),
        ] {
            input(&mut state, AttachmentsInput::Select(by));
            assert_eq!(listed(&state).1, selected, "{by}");
        }
        let mut none = State::new(format::parse("- Milk"), None);
        keys(
            &mut none,
            &[
                AttachmentsInput::AddComment,
                AttachmentsInput::DeleteComment,
                AttachmentsInput::Accept(AT),
            ],
        );
        assert_eq!(none.attachments_panel(), None, "no panel");
        input(&mut none, AttachmentsInput::Open);
        input(&mut none, AttachmentsInput::Select(1));
        assert_eq!(
            sections(&none).unwrap().1,
            Some(PanelItem::Note),
            "no comments"
        );
    }

    // FR-ATT-05, FR-OPS-08, attachments.md §5
    #[test]
    fn a_new_comment_goes_on_top_stamped_and_selected() {
        let mut state = State::new(format::parse(COMMENTS), None);
        keys(
            &mut state,
            &[
                AttachmentsInput::Open,
                AttachmentsInput::Select(1),
                AttachmentsInput::Select(1),
                AttachmentsInput::AddComment,
            ],
        );
        typed(&mut state, "c");
        assert_eq!(listed(&state).2, Some((None, "c".to_owned())));
        keys(
            &mut state,
            &[
                AttachmentsInput::Select(1),
                AttachmentsInput::DeleteComment,
                AttachmentsInput::AddComment,
                AttachmentsInput::Edit,
            ],
        );
        assert_eq!(listed(&state).1, Some(1), "typing holds the selection");
        input(&mut state, AttachmentsInput::Accept(AT));
        assert_eq!(
            listed(&state),
            (
                vec![
                    "[2026-10-09 07:45] c".to_owned(),
                    "[2026-10-08 09:00] b".to_owned(),
                    "by hand".to_owned(),
                    "[2026-10-07 09:00] a".to_owned(),
                ],
                Some(0),
                None
            )
        );
        input(&mut state, AttachmentsInput::Close);
        state.update(Message::Undo);
        assert_eq!(file(&state), COMMENTS, "one step");
        let mut empty = State::new(format::parse("- Milk"), None);
        keys(
            &mut empty,
            &[
                AttachmentsInput::Open,
                AttachmentsInput::Select(1),
                AttachmentsInput::AddComment,
            ],
        );
        typed(&mut empty, "- first:");
        input(&mut empty, AttachmentsInput::Accept(AT));
        assert_eq!(
            file(&empty),
            "- Milk\n\tCOMMENTS @hidden\n\t\t[2026-10-09 07:45] - first:\u{200B}"
        );
    }

    // FR-ATT-05, attachments.md §5
    #[test]
    fn an_edit_keeps_the_timestamp_and_blank_or_cancelled_text_changes_nothing() {
        let mut state = State::new(format::parse(COMMENTS), None);
        keys(
            &mut state,
            &[
                AttachmentsInput::Open,
                AttachmentsInput::Select(1),
                AttachmentsInput::Edit,
            ],
        );
        assert_eq!(listed(&state).2, Some((Some(0), "b".to_owned())));
        typed(&mut state, "2");
        input(&mut state, AttachmentsInput::Accept(AT));
        keys(
            &mut state,
            &[AttachmentsInput::Select(1), AttachmentsInput::Edit],
        );
        typed(&mut state, "!");
        input(&mut state, AttachmentsInput::Accept(AT));
        assert_eq!(
            listed(&state).0[..2],
            ["[2026-10-08 09:00] b2", "by hand!"],
            "a comment without a date stays without one"
        );
        let revision = state.document().revision();
        keys(
            &mut state,
            &[
                AttachmentsInput::Edit,
                AttachmentsInput::Text(TextEdit::SelectAll),
                AttachmentsInput::Text(TextEdit::Insert(' ')),
                AttachmentsInput::Accept(AT),
                AttachmentsInput::AddComment,
            ],
        );
        typed(&mut state, "x");
        keys(
            &mut state,
            &[AttachmentsInput::Cancel, AttachmentsInput::Accept(AT)],
        );
        assert_eq!(state.document().revision(), revision);
        assert_eq!(listed(&state).2, None);
    }

    // FR-ATT-05, attachments.md §5
    #[test]
    fn deleting_takes_the_selected_comment_and_the_last_takes_the_node() {
        let mut state = State::new(format::parse(COMMENTS), None);
        keys(
            &mut state,
            &[
                AttachmentsInput::Open,
                AttachmentsInput::Select(1),
                AttachmentsInput::Select(isize::MAX),
                AttachmentsInput::DeleteComment,
            ],
        );
        assert_eq!(listed(&state).0, ["[2026-10-08 09:00] b", "by hand"]);
        assert_eq!(listed(&state).1, Some(1), "the selection stays on the last");
        keys(
            &mut state,
            &[
                AttachmentsInput::DeleteComment,
                AttachmentsInput::DeleteComment,
            ],
        );
        assert_eq!(file(&state), "- Milk\n");
        assert_eq!(listed(&state).1, None);
        input(&mut state, AttachmentsInput::DeleteComment);
        input(&mut state, AttachmentsInput::Edit);
        assert_eq!(listed(&state).2, None, "nothing to delete or edit");
        state.update(Message::Undo);
        assert_eq!(listed(&state).0, ["[2026-10-08 09:00] b"], "one step each");
        let mut note = super::tests::state();
        keys(
            &mut note,
            &[
                AttachmentsInput::Open,
                AttachmentsInput::DeleteComment,
                AttachmentsInput::AddComment,
            ],
        );
        assert_eq!(
            (file(&note), listed(&note).2),
            (TEXT.to_owned(), Some((None, String::new()))),
            "nothing deleted, and a comment is added from the note"
        );
    }

    // FR-ATT-06, attachments.md §6
    #[test]
    fn turning_on_a_kind_already_on_keeps_what_is_typed() {
        let mut state = state();
        keys(
            &mut state,
            &[
                AttachmentsInput::Open,
                AttachmentsInput::Select(1),
                AttachmentsInput::AddComment,
            ],
        );
        typed(&mut state, "new");
        configure(&mut state, Attachment::Comments, true);
        assert_eq!(listed(&state).2, Some((None, "new".to_owned())));
    }

    // FR-ATT-05, attachments.md §3, D-110
    #[test]
    fn a_comment_is_found_again_as_the_same_line_among_identical_ones() {
        let lines = |text: &[&str]| text.iter().map(|&line| line.to_owned()).collect::<Vec<_>>();
        let before = lines(&["a", "b", "a", "c"]);
        assert_eq!(
            same_comment(&before, &lines(&["n", "a", "b", "a", "c"]), 2),
            Some(3)
        );
        assert_eq!(
            same_comment(&before, &lines(&["b", "a", "c"]), 2),
            None,
            "one of two identical comments is gone, so the second is"
        );
        assert_eq!(same_comment(&before, &lines(&["b", "a", "c"]), 0), Some(1));
        assert_eq!(same_comment(&before, &before, 4), None, "past the end");
    }

    // FR-ATT-05, FR-OPS-06, attachments.md §5
    #[test]
    fn a_paste_into_a_comment_takes_its_line_breaks_as_spaces() {
        let mut state = state();
        state.update(Message::Copy);
        keys(
            &mut state,
            &[
                AttachmentsInput::Open,
                AttachmentsInput::Select(1),
                AttachmentsInput::AddComment,
            ],
        );
        state.update(Message::Pasted {
            text: "a\r\nb\nc\n".to_owned(),
            at: Paste::Below,
        });
        assert_eq!(listed(&state).2, Some((None, "a b c".to_owned())));
        state.update(Message::Pasted {
            text: TEXT.trim_end_matches("---\n").to_owned(),
            at: Paste::Below,
        });
        assert_eq!(
            listed(&state).2,
            Some((None, "a b cMilk".to_owned())),
            "the copied block's text, without its attachments"
        );
    }

    // FR-ATT-05, FR-ATT-06, attachments.md §5, §6
    #[test]
    fn turning_comments_off_drops_the_comment_being_typed() {
        let mut state = state();
        keys(
            &mut state,
            &[
                AttachmentsInput::Open,
                AttachmentsInput::Select(1),
                AttachmentsInput::AddComment,
            ],
        );
        typed(&mut state, "x");
        configure(&mut state, Attachment::Comments, false);
        assert_eq!(edited(&state), (vec!["Full fat".to_owned()], None));
        assert_eq!(state.attachments_panel().unwrap().draft, None);
        assert_eq!(file(&state), TEXT);
    }

    // FR-ATT-02, FR-ATT-06, attachments.md §2, §6
    #[test]
    fn a_row_counts_its_comments_while_they_are_on() {
        let mut state = State::new(format::parse(COMMENTS), None);
        assert_eq!(state.rows()[0].comments, 3);
        configure(&mut state, Attachment::Comments, false);
        assert_eq!(state.rows()[0].comments, 0);
    }
}
