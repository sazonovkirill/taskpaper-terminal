//! The attachments panel: the focused block's note and comments, each on a tab of a panel that
//! takes the keys while open, and the settings that turn each kind on
//! ([attachments.md §3, §6](../../../docs/specs/attachments.md#3-attachments-panel-fr-att-03),
//! FR-ATT-03, FR-ATT-06, D-94).

use std::{ops::Range, sync::Arc};

use super::{
    Command, State,
    text_field::{TextEdit, TextField},
};
use crate::document::{Attachment, ItemId, attachments, format::breaks_line};

/// A key of the open panel, or opening it, as an intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentsInput {
    /// Open the panel on the focused block, on this tab (Open Note, Open Comments).
    Open(Attachment),
    /// Show the other tab.
    SwitchTab,
    /// Close the panel.
    Close,
    /// Edit the note shown, as raw Markdown (attachments.md §4).
    Edit,
    /// Change the note's line with the caret.
    Text(TextEdit),
    /// Break the note's line at the caret.
    Break,
    /// Move the caret to the line above.
    LineUp,
    /// Move the caret to the line below.
    LineDown,
    /// Save the edited note and show it rendered again.
    Save,
}

/// The open panel: the block it is on, the tab it shows and the note being edited, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Panel {
    block: ItemId,
    tab: Attachment,
    editor: Option<NoteEditor>,
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
    /// The tabs, one per kind turned on, in order.
    pub tabs: &'a [Attachment],
    /// The tab shown.
    pub tab: Attachment,
    /// The lines of the shown tab's attachment, as the user wrote them, or as the editor has
    /// them while editing.
    pub lines: Vec<String>,
    /// Where the caret is while the note is edited; `None` while it is shown rendered.
    pub editing: Option<NoteCaret>,
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
            Command::OpenAttachment(kind) => self.attached.contains(&kind),
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
        Some(AttachmentsView {
            title: self.document.get(panel.block)?.shared_plain_text(),
            tabs: &self.attached,
            tab: panel.tab,
            lines: panel.editor.as_ref().map_or_else(
                || attachments::content(&self.document, panel.block, panel.tab),
                NoteEditor::text,
            ),
            editing,
        })
    }

    /// Applies `input`: opens the panel on the focused block, or switches or closes the open one.
    pub(super) fn attachments_input(&mut self, input: AttachmentsInput) {
        match input {
            AttachmentsInput::Open(tab) => self.open_panel(tab),
            AttachmentsInput::SwitchTab => {
                let tabs = self.attached.clone();
                if let Some(panel) = &mut self.panel
                    && panel.editor.is_none()
                    && let Some(&next) = tabs.iter().find(|&&tab| tab != panel.tab)
                {
                    panel.tab = next;
                }
            }
            AttachmentsInput::Close => {
                self.save_note();
                self.panel = None;
                self.resume_editing();
            }
            AttachmentsInput::Edit => self.edit_note(),
            AttachmentsInput::Save => self.save_note(),
            AttachmentsInput::Text(edit) => self.note_editor(|editor| editor.edit(edit)),
            AttachmentsInput::Break => self.note_editor(NoteEditor::break_line),
            AttachmentsInput::LineUp => self.note_editor(|editor| {
                if let Some(row) = editor.row.checked_sub(1) {
                    editor.go_to(row);
                }
            }),
            AttachmentsInput::LineDown => self.note_editor(|editor| editor.go_to(editor.row + 1)),
        }
    }

    /// Starts editing the note shown, at the end of its last line.
    fn edit_note(&mut self) {
        let Some(panel) = &self.panel else {
            return;
        };
        if panel.tab != Attachment::Note || panel.editor.is_some() {
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

    /// Pastes `text` into the note being edited, if one is; returns whether it took it.
    pub(super) fn note_editor_paste(&mut self, text: &str) -> bool {
        let editing = self
            .panel
            .as_ref()
            .is_some_and(|panel| panel.editor.is_some());
        self.note_editor(|editor| editor.paste(text));
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
        if let Some(ops) =
            attachments::write(&mut self.document, block, Attachment::Note, &editor.text())
        {
            self.commit_any(ops);
        }
    }

    /// Opens the panel on `tab` for the focused block, a divider aside, which has none, while
    /// that kind is turned on (attachments.md §3, §6).
    fn open_panel(&mut self, tab: Attachment) {
        let Some((block, _)) = self.focused() else {
            return;
        };
        if self.attached.contains(&tab) && !block.divider() {
            self.panel = Some(Panel {
                block: block.id(),
                tab,
                editor: None,
            });
        }
    }

    /// Turns the attachments of kind `kind` on or off; the open panel leaves a tab turned off,
    /// saving the note it edits, for the other, or closes without one (attachments.md §6).
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
        let Some(panel) = &self.panel else {
            return;
        };
        if !self.attached.contains(&panel.tab) {
            self.save_note();
            match (self.attached.first(), &mut self.panel) {
                (Some(&other), Some(panel)) => panel.tab = other,
                _ => self.attachments_input(AttachmentsInput::Close),
            }
        }
    }

    /// The panel on the block of a reloaded document that `exact` finds for its own, closed when
    /// it finds none (D-104).
    pub(super) fn rematch_panel(&mut self, exact: impl Fn(ItemId) -> Option<ItemId>) {
        self.panel = self.panel.take().and_then(|panel| {
            Some(Panel {
                block: exact(panel.block)?,
                ..panel
            })
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use super::super::text_field::Motion;
    use crate::{
        app::{Access, Configuration, Menu, MenuInput, Message, Mode, NodeInput, Notice, Paste},
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

    /// The open panel's tabs and shown tab.
    fn tabs(state: &State) -> Option<(Vec<Attachment>, Attachment)> {
        state
            .attachments_panel()
            .map(|view| (view.tabs.to_vec(), view.tab))
    }

    // FR-ATT-03, attachments.md §3
    #[test]
    fn the_panel_shows_the_focused_blocks_attachment_on_the_tab_it_opened_on() {
        let mut state = state();
        assert_eq!(state.attachments_panel(), None);
        input(&mut state, AttachmentsInput::Open(Attachment::Comments));
        let view = state.attachments_panel().unwrap();
        assert_eq!(&*view.title, "Milk");
        assert_eq!(view.tabs, Attachment::ALL);
        assert_eq!(view.tab, Attachment::Comments);
        assert_eq!(view.lines, ["[2026-10-08 09:00] ok"]);
        input(&mut state, AttachmentsInput::SwitchTab);
        let view = state.attachments_panel().unwrap();
        assert_eq!(
            (view.tab, view.lines),
            (Attachment::Note, vec!["Full fat".to_owned()])
        );
        input(&mut state, AttachmentsInput::SwitchTab);
        assert_eq!(tabs(&state).unwrap().1, Attachment::Comments);
        input(&mut state, AttachmentsInput::Close);
        assert_eq!(state.attachments_panel(), None);
        input(&mut state, AttachmentsInput::SwitchTab);
        assert_eq!(state.attachments_panel(), None, "nothing to switch");
    }

    // FR-ATT-03, FR-ATT-06, FR-BLK-07, attachments.md §1, §3, §6
    #[test]
    fn the_panel_opens_only_on_a_block_that_is_no_divider_for_a_kind_turned_on() {
        let mut state = state();
        configure(&mut state, Attachment::Note, false);
        input(&mut state, AttachmentsInput::Open(Attachment::Note));
        assert_eq!(state.attachments_panel(), None, "notes are off");
        input(&mut state, AttachmentsInput::Open(Attachment::Comments));
        assert_eq!(
            tabs(&state),
            Some((vec![Attachment::Comments], Attachment::Comments))
        );
        input(&mut state, AttachmentsInput::SwitchTab);
        assert_eq!(tabs(&state).unwrap().1, Attachment::Comments, "one tab");
        input(&mut state, AttachmentsInput::Close);
        state.update(Message::FocusLast);
        input(&mut state, AttachmentsInput::Open(Attachment::Comments));
        assert_eq!(state.attachments_panel(), None, "a divider");
        let mut empty = State::new(format::parse(""), None);
        input(&mut empty, AttachmentsInput::Open(Attachment::Note));
        assert_eq!(empty.attachments_panel(), None, "no block");
    }

    // FR-ATT-06, attachments.md §6
    #[test]
    fn turning_the_shown_kind_off_moves_to_the_other_tab_or_closes() {
        let mut state = state();
        input(&mut state, AttachmentsInput::Open(Attachment::Note));
        configure(&mut state, Attachment::Comments, false);
        assert_eq!(
            tabs(&state),
            Some((vec![Attachment::Note], Attachment::Note))
        );
        configure(&mut state, Attachment::Comments, true);
        assert_eq!(
            tabs(&state),
            Some((Attachment::ALL.to_vec(), Attachment::Note))
        );
        configure(&mut state, Attachment::Note, false);
        assert_eq!(
            tabs(&state),
            Some((vec![Attachment::Comments], Attachment::Comments))
        );
        configure(&mut state, Attachment::Comments, false);
        assert_eq!(state.attachments_panel(), None);
        assert_eq!(state.attachments_shown(), []);
        configure(&mut state, Attachment::Comments, true);
        configure(&mut state, Attachment::Note, true);
        assert_eq!(state.attachments_shown(), Attachment::ALL, "in order");
    }

    // FR-ATT-06, FR-NAV-06, FR-KEY-04, attachments.md §6
    #[test]
    fn a_kind_turned_off_leaves_go_to_command_and_the_shortcuts_panel() {
        let mut state = state();
        let open = |kind| Command::OpenAttachment(kind);
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
        assert!(listed(&mut state).contains(&"Open Note".to_owned()));
        assert!(shortcuts(&state).contains(&open(Attachment::Note)));
        configure(&mut state, Attachment::Note, false);
        assert!(!listed(&mut state).contains(&"Open Note".to_owned()));
        assert!(listed(&mut state).contains(&"Open Comments".to_owned()));
        assert!(!shortcuts(&state).contains(&open(Attachment::Note)));
        assert!(shortcuts(&state).contains(&open(Attachment::Comments)));
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
        for c in "open note".chars() {
            state.update(Message::Menu(Menu::CommandMenu, MenuInput::Type(c)));
        }
        state.update(Message::Menu(Menu::CommandMenu, MenuInput::Choose));
        assert_eq!(tabs(&state).unwrap().1, Attachment::Note);
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
        (view.lines, caret)
    }

    // FR-ATT-04, attachments.md §4
    #[test]
    fn the_note_editor_breaks_joins_and_moves_between_lines() {
        let mut state = state();
        keys(
            &mut state,
            &[
                AttachmentsInput::Open(Attachment::Note),
                AttachmentsInput::Edit,
            ],
        );
        assert_eq!(edited(&state), (vec!["Full fat".to_owned()], Some((0, 8))));
        keys(&mut state, &[AttachmentsInput::SwitchTab]);
        assert_eq!(
            tabs(&state).unwrap().1,
            Attachment::Note,
            "editing keeps the tab"
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
            &[
                AttachmentsInput::Open(Attachment::Note),
                AttachmentsInput::Edit,
            ],
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
        assert_eq!((view.editing, view.lines.len()), (None, 2), "shown again");
        input(&mut state, AttachmentsInput::Close);
        state.update(Message::Undo);
        assert_eq!(file(&state), TEXT, "one step");
        keys(
            &mut state,
            &[
                AttachmentsInput::Open(Attachment::Note),
                AttachmentsInput::Edit,
            ],
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
                AttachmentsInput::Open(Attachment::Note),
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
            &[
                AttachmentsInput::Open(Attachment::Note),
                AttachmentsInput::Edit,
            ],
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

    // FR-ATT-04, attachments.md §4
    #[test]
    fn only_the_note_tab_edits_and_its_keys_need_an_editor() {
        let mut state = state();
        keys(
            &mut state,
            &[
                AttachmentsInput::Open(Attachment::Comments),
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
            &[
                AttachmentsInput::Open(Attachment::Note),
                AttachmentsInput::Edit,
            ],
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
            &[
                AttachmentsInput::Open(Attachment::Note),
                AttachmentsInput::Edit,
            ],
        );
        typed(&mut state, "!");
        configure(&mut state, Attachment::Note, false);
        assert!(file(&state).contains("Full fat!"));
        assert_eq!(
            edited(&state),
            (vec!["[2026-10-08 09:00] ok".to_owned()], None)
        );
    }

    // FR-ATT-04, file-io.md §1, §2
    #[test]
    fn quitting_saves_the_note_and_a_read_only_file_refuses_it() {
        for quit in [Message::Quit, Message::Terminate] {
            let mut state = state();
            keys(
                &mut state,
                &[
                    AttachmentsInput::Open(Attachment::Note),
                    AttachmentsInput::Edit,
                ],
            );
            typed(&mut state, "!");
            state.update(quit);
            assert!(file(&state).contains("Full fat!"));
        }
        let mut state = State::new(format::parse(TEXT), Some(PathBuf::from("/a.taskpaper")))
            .with_access(Access::ReadOnly);
        keys(
            &mut state,
            &[
                AttachmentsInput::Open(Attachment::Note),
                AttachmentsInput::Edit,
            ],
        );
        typed(&mut state, "!");
        input(&mut state, AttachmentsInput::Save);
        assert_eq!(file(&state), TEXT);
        assert_eq!(state.notice(), Some(&Notice::ReadOnly));
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
}
