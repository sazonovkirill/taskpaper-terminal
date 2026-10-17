//! Quick Add: a one-line prompt that adds a task or note, with the dates its words name, as the
//! last child of a project, the inbox project unless another is chosen in it
//! ([editing.md §6](../../../docs/specs/editing.md#6-quick-add-fr-mode-07-fr-mode-08),
//! FR-MODE-07, FR-MODE-08, D-96, D-97, D-133, D-141).

use std::ops::Range;

use jiff::civil::Date;

use super::{
    Access, MenuInput, Notice, State,
    finder::{Finder, FinderView},
    picker::Picked,
    text_field::{TextEdit, TextField},
};
use crate::document::{
    DateTag, ItemId, blocks,
    dates::{self, Found},
    format::{self, LineParts, Typed, tags},
    structure::{self, Place},
};

/// Opening Quick Add, or a key of the open panel, as an intent; the project picker's keys are
/// menu keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickAddInput {
    /// Open the panel (`q`, D-133).
    Open,
    /// Change the text typed.
    Edit(TextEdit),
    /// Open the project picker to choose the destination (`Tab`, D-141).
    ChooseProject,
    /// Add the block, when there is a destination and text.
    Add,
    /// Close the panel, adding nothing.
    Cancel,
}

/// What `Enter` lacks to add the block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Missing {
    /// Text besides the dates.
    Text,
    /// A destination (D-141).
    Project,
}

/// The open panel as a frontend shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickAddView<'a> {
    /// Whether it adds a task, whose dash stands before the text, rather than a note.
    pub task: bool,
    /// The text typed.
    pub text: &'a str,
    /// The caret's byte offset into `text`.
    pub caret: usize,
    /// The selected bytes of `text`, if any.
    pub selection: Option<Range<usize>>,
    /// The dates that count in `text`, in order (D-97).
    pub dates: Vec<Found>,
    /// The destination's path, its own text last, if there is one.
    pub destination: Option<Vec<String>>,
    /// What `Enter` lacks, or `None` when it adds the block.
    pub missing: Option<Missing>,
}

/// The open panel: whether it adds a task, the text field, the destination and the project
/// picker while `Tab` has it open.
#[derive(Debug, Clone)]
pub(super) struct QuickAdd {
    task: bool,
    field: TextField,
    destination: Option<ItemId>,
    picker: Option<Finder<ItemId>>,
}

impl QuickAdd {
    /// Keeps the destination on the project `exact` finds for it in a reloaded document, none
    /// when it finds none, and closes the project picker, whose candidates name blocks of the
    /// old document (file-io.md §3).
    pub(super) fn rematch(&mut self, exact: impl Fn(ItemId) -> Option<ItemId>) {
        self.destination = self.destination.and_then(exact);
        self.picker = None;
    }

    /// Applies `edit` to the text, or returns `false` and changes nothing when it would type a
    /// tag (OQ-25). A character that breaks a line is not typed. Text that starts with a task's
    /// dash and space makes a note a task, the dash moving before the text (OQ-27).
    fn edit(&mut self, edit: TextEdit) -> bool {
        if let TextEdit::Insert(c) = edit
            && format::breaks_line(c)
        {
            return true;
        }
        let field = self.field.edited(edit);
        if tags::adds_tag(self.field.text(), field.text()) {
            return false;
        }
        self.field = match LineParts::default()
            .as_task(field.text())
            .filter(|_| !self.task)
        {
            // The edit that made the dash and space was inside them, so the caret was too.
            Some((_, text)) => {
                self.task = true;
                TextField::with_caret(text, 0)
            }
            None => field,
        };
        true
    }

    /// Types `text` into the text as its characters would be typed, leaving out those that
    /// break lines; `false` when a character was left out because it would type a tag (OQ-25).
    fn paste(&mut self, text: &str) -> bool {
        let mut typed = true;
        for c in text.chars() {
            typed &= self.edit(TextEdit::Insert(c));
        }
        typed
    }

    /// The line the text adds with the dates found in it on `today`: the text without them,
    /// as a task, whose text keeps the colons ending it, or as a note without the colons that
    /// would make it a project, then `@start` and `@due`; `None` when no text is left
    /// (editing.md §6, D-96, D-97).
    fn line(&self, today: Date) -> Option<String> {
        let found = dates::find(self.field.text(), today);
        let typed = if self.task { Typed::Task } else { Typed::Text };
        // The text is typed into a block of that type, so its colons are never a project's.
        let (parts, _) = LineParts::default().retyped(typed, "");
        let (parts, text) = parts.retyped(typed, &without(self.field.text(), &found));
        if text.is_empty() {
            return None;
        }
        let mut line = parts.line(&text);
        for date in found {
            let name = match date.tag {
                DateTag::Start => "start",
                DateTag::Due => "due",
            };
            line = tags::set(&line, name, &date.date.to_string());
        }
        Some(line)
    }
}

/// `text` without the spans of `found` and the whitespace before each, trimmed.
fn without(text: &str, found: &[Found]) -> String {
    let mut out = String::new();
    let mut from = 0;
    for date in found {
        out.push_str(text[from..date.span.start].trim_end());
        from = date.span.end;
    }
    out.push_str(&text[from..]);
    out.trim().to_owned()
}

impl State {
    /// Quick Add, while it is open.
    #[must_use]
    pub fn quick_add(&self) -> Option<QuickAddView<'_>> {
        let quick_add = self.quick_add.as_ref()?;
        let destination = self.quick_add_destination(quick_add);
        let missing = if quick_add.line(self.today).is_none() {
            Some(Missing::Text)
        } else if destination.is_none() {
            Some(Missing::Project)
        } else {
            None
        };
        Some(QuickAddView {
            task: quick_add.task,
            text: quick_add.field.text(),
            caret: quick_add.field.caret(),
            selection: quick_add.field.selection(),
            dates: dates::find(quick_add.field.text(), self.today),
            destination: destination.map(|block| self.path_to(&block)),
            missing,
        })
    }

    /// Quick Add's project picker, while `Tab` has it open (D-141).
    #[must_use]
    pub fn quick_add_picker(&self) -> Option<FinderView<'_, ItemId>> {
        self.quick_add.as_ref()?.picker.as_ref().map(Finder::view)
    }

    /// The destination of `quick_add` while it is in the document.
    fn quick_add_destination(&self, quick_add: &QuickAdd) -> Option<blocks::Block> {
        quick_add
            .destination
            .and_then(|id| blocks::block(&self.document, id))
    }

    /// `q`: opens Quick Add with the inbox project as its destination, or, without one, with the
    /// project picker open; says so and opens nothing when there is no project. A read-only file
    /// refuses with a notice.
    fn open_quick_add(&mut self) {
        if self.access() == Access::ReadOnly {
            self.notify(Notice::ReadOnly);
            return;
        }
        let candidates = self.project_candidates();
        if candidates.is_empty() {
            self.notify(Notice::NoProjects);
            return;
        }
        let destination = self.inbox_project();
        self.quick_add = Some(QuickAdd {
            task: true,
            field: TextField::new(String::new()),
            destination,
            picker: destination.is_none().then(|| Finder::new(candidates)),
        });
    }

    /// `Tab`: opens the project picker over the projects as Go to Project lists them, the
    /// destination highlighted.
    fn open_quick_add_picker(&mut self) {
        let candidates = self.project_candidates();
        let Some(quick_add) = &mut self.quick_add else {
            return;
        };
        let mut picker = Finder::new(candidates);
        if let Some(destination) = &quick_add.destination {
            picker.highlight(destination);
        }
        quick_add.picker = Some(picker);
    }

    /// Applies `input` to Quick Add's open project picker: choosing makes the highlighted
    /// project the destination, and choosing or `Esc` closes the picker (editing.md §6).
    pub(super) fn quick_add_picker_input(&mut self, input: MenuInput) {
        let Some(quick_add) = &mut self.quick_add else {
            return;
        };
        let Some(closed) = quick_add
            .picker
            .as_mut()
            .and_then(|picker| picker.input(input))
        else {
            return;
        };
        quick_add.picker = None;
        if let Picked::Chosen(id) = closed {
            quick_add.destination = Some(id);
        }
    }

    /// Applies `input`: opens the panel, or changes, adds or closes the open one; adding puts
    /// the block in as one step to undo and closes the panel, saying where the block went.
    pub(super) fn quick_add_input(&mut self, input: QuickAddInput) {
        match input {
            QuickAddInput::Open => self.open_quick_add(),
            QuickAddInput::Edit(edit) => self.quick_add_edit(edit),
            QuickAddInput::ChooseProject => self.open_quick_add_picker(),
            QuickAddInput::Cancel => self.quick_add = None,
            QuickAddInput::Add => self.add_quickly(),
        }
    }

    /// Applies `edit` to the open panel's text: `Backspace` at the start of a task's text makes
    /// it a note.
    fn quick_add_edit(&mut self, edit: TextEdit) {
        let Some(quick_add) = &mut self.quick_add else {
            return;
        };
        if edit == TextEdit::DeleteBackward
            && quick_add.task
            && quick_add.field.caret() == 0
            && quick_add.field.selection().is_none()
        {
            quick_add.task = false;
        } else if !quick_add.edit(edit) {
            self.notify(Notice::TypedTag);
        }
    }

    /// Types `text` into the open panel's text, as [`State::one_line`] gives it,
    /// as [`QuickAdd::paste`] does, unless its project picker is open, which takes no text
    /// pasted; `false` when no panel is open.
    pub(super) fn quick_add_paste(&mut self, text: &str) -> bool {
        let line = self.one_line(text);
        let Some(quick_add) = &mut self.quick_add else {
            return false;
        };
        if quick_add.picker.is_none() && !quick_add.paste(&line) {
            self.notify(Notice::TypedTag);
        }
        true
    }

    /// Adds the block the open panel holds as its destination's last child and closes the
    /// panel; does nothing while it lacks a destination or text.
    fn add_quickly(&mut self) {
        let Some(quick_add) = &self.quick_add else {
            return;
        };
        let (Some(destination), Some(line)) = (
            self.quick_add_destination(quick_add),
            quick_add.line(self.today),
        ) else {
            return;
        };
        self.quick_add = None;
        let (_, ops) =
            structure::new_line(&mut self.document, Place::LastChild(&destination), line)
                .expect("the destination was just found in the document");
        self.commit(ops);
        self.notify(Notice::QuickAdded {
            project: self.shown_text(destination.id()),
        });
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use jiff::civil::date;

    use super::*;
    use crate::{
        app::{Menu, Message, Motion, Notice},
        document::format,
    };

    /// A Wednesday.
    const TODAY: Date = date(2026, 10, 7);

    const OUTLINE: &str = "INBOX: @inbox\n\t- old\nWork:\n\tLaunch:\n\t\t- plan\nTODAY:\n- loose";

    fn open(text: &str) -> State {
        let mut state = State::new(format::parse(text), None);
        state.update(Message::Tick {
            now: Duration::ZERO,
            today: TODAY,
        });
        state
    }

    fn input(state: &mut State, input: QuickAddInput) {
        assert_eq!(state.update(Message::QuickAdd(input)), []);
    }

    fn typed(state: &mut State, text: &str) {
        for c in text.chars() {
            input(state, QuickAddInput::Edit(TextEdit::Insert(c)));
        }
    }

    /// Each line as one tab per depth and its text.
    fn lines(state: &State) -> Vec<String> {
        let document = state.document();
        document
            .descendants(document.root())
            .map(|(id, depth)| {
                format!("{}{}", "\t".repeat(depth), document.get(id).unwrap().text())
            })
            .collect()
    }

    /// Whether it adds a task, its text and its destination's path.
    fn shown(state: &State) -> (bool, String, Option<Vec<String>>) {
        let view = state.quick_add().expect("Quick Add is open");
        (view.task, view.text.to_owned(), view.destination)
    }

    fn path(path: &[&str]) -> Vec<String> {
        path.iter().map(|&crumb| crumb.to_owned()).collect()
    }

    // FR-MODE-07, D-96, D-133
    #[test]
    fn q_opens_an_empty_task_for_the_inbox_project() {
        let mut state = open(OUTLINE);
        input(&mut state, QuickAddInput::Edit(TextEdit::Insert('x')));
        input(&mut state, QuickAddInput::Add);
        assert_eq!(state.quick_add(), None, "keys without a panel do nothing");
        assert_eq!(lines(&state).len(), 7);
        state.update(Message::QuickAdd(QuickAddInput::Open));
        let view = state.quick_add().unwrap();
        assert_eq!(
            view,
            QuickAddView {
                task: true,
                text: "",
                caret: 0,
                selection: None,
                dates: Vec::new(),
                destination: Some(path(&["INBOX"])),
                missing: Some(Missing::Text),
            }
        );
        input(&mut state, QuickAddInput::Cancel);
        assert_eq!(state.quick_add(), None);
    }

    /// The texts of the projects the picker lists and the highlighted position.
    fn listed(state: &State) -> (Vec<String>, usize) {
        let view = state.quick_add_picker().expect("the picker is open");
        (
            view.entries
                .iter()
                .map(|entry| entry.text.to_string())
                .collect(),
            view.selected,
        )
    }

    fn pick(state: &mut State, input: MenuInput) {
        assert_eq!(state.update(Message::Menu(Menu::QuickAdd, input)), []);
    }

    // D-141
    #[test]
    fn tab_picks_another_project_and_esc_keeps_the_one_there_was() {
        let mut state = open(OUTLINE);
        state.update(Message::QuickAdd(QuickAddInput::Open));
        assert_eq!(state.quick_add_picker(), None, "the inbox is chosen");
        typed(&mut state, "x");
        input(&mut state, QuickAddInput::ChooseProject);
        assert_eq!(
            listed(&state),
            (
                ["INBOX", "Work", "Launch", "TODAY"]
                    .map(str::to_owned)
                    .to_vec(),
                0
            ),
            "the inbox first and highlighted"
        );
        pick(&mut state, MenuInput::Next);
        pick(&mut state, MenuInput::Dismiss);
        assert_eq!(state.quick_add_picker(), None);
        assert_eq!(
            shown(&state),
            (true, "x".to_owned(), Some(path(&["INBOX"])))
        );
        input(&mut state, QuickAddInput::ChooseProject);
        pick(&mut state, MenuInput::Type('l'));
        pick(&mut state, MenuInput::Type('a'));
        assert_eq!(listed(&state).0, ["Launch"]);
        assert!(state.quick_add_paste("typed"), "the panel is open");
        assert_eq!(listed(&state).0, ["Launch"], "the filter takes no paste");
        pick(&mut state, MenuInput::Choose);
        assert_eq!(state.quick_add_picker(), None);
        assert_eq!(
            shown(&state),
            (true, "x".to_owned(), Some(path(&["Work", "Launch"])))
        );
        input(&mut state, QuickAddInput::ChooseProject);
        assert_eq!(listed(&state).1, 2, "the destination highlighted");
        pick(&mut state, MenuInput::Dismiss);
        input(&mut state, QuickAddInput::Add);
        assert_eq!(lines(&state)[5], "\t\t- x");
        assert_eq!(
            state.notice(),
            Some(&Notice::QuickAdded {
                project: "Launch".to_owned()
            })
        );
    }

    // D-141
    #[test]
    fn without_an_inbox_project_q_opens_the_picker_and_without_projects_nothing() {
        let mut state = open("Work:\n\tINBOX:\n- loose");
        state.update(Message::QuickAdd(QuickAddInput::Open));
        assert_eq!(listed(&state), (path(&["Work", "INBOX"]), 0));
        pick(&mut state, MenuInput::Dismiss);
        assert_eq!(state.quick_add().unwrap().destination, None);
        typed(&mut state, "a");
        assert_eq!(state.quick_add().unwrap().missing, Some(Missing::Project));
        input(&mut state, QuickAddInput::Add);
        assert!(state.quick_add().is_some(), "nothing added");
        let mut state = open("- loose\nnote");
        state.update(Message::QuickAdd(QuickAddInput::Open));
        assert_eq!(state.quick_add(), None);
        assert_eq!(state.notice(), Some(&Notice::NoProjects));
    }

    // D-141
    #[test]
    fn picker_keys_with_quick_add_closed_change_nothing() {
        let mut state = open(OUTLINE);
        input(&mut state, QuickAddInput::ChooseProject);
        pick(&mut state, MenuInput::Choose);
        assert_eq!(state.quick_add(), None);
        assert_eq!(format::serialize(state.document()), OUTLINE);
    }

    // FR-MODE-07, file-io.md §1
    #[test]
    fn a_read_only_file_refuses() {
        let mut state = open(OUTLINE).with_access(Access::ReadOnly);
        state.update(Message::QuickAdd(QuickAddInput::Open));
        assert_eq!(state.quick_add(), None);
        assert_eq!(state.notice(), Some(&Notice::ReadOnly));
    }

    // D-96, OQ-27
    #[test]
    fn backspace_at_the_start_makes_a_note_and_a_typed_dash_a_task_again() {
        let mut state = open(OUTLINE);
        state.update(Message::QuickAdd(QuickAddInput::Open));
        typed(&mut state, "ab");
        input(&mut state, QuickAddInput::Edit(TextEdit::DeleteBackward));
        assert_eq!(
            shown(&state).1,
            "a",
            "Backspace deletes text before the caret"
        );
        let start = TextEdit::Move {
            motion: Motion::Start,
            select: false,
        };
        input(&mut state, QuickAddInput::Edit(start));
        input(&mut state, QuickAddInput::Edit(TextEdit::DeleteBackward));
        assert!(!shown(&state).0);
        assert_eq!(shown(&state).1, "a");
        input(&mut state, QuickAddInput::Edit(TextEdit::DeleteBackward));
        assert_eq!((shown(&state).0, shown(&state).1.as_str()), (false, "a"));
        typed(&mut state, "- ");
        assert_eq!((shown(&state).0, shown(&state).1.as_str()), (true, "a"));
        assert_eq!(state.quick_add().unwrap().caret, 0);
        input(&mut state, QuickAddInput::Edit(TextEdit::SelectAll));
        input(&mut state, QuickAddInput::Edit(TextEdit::DeleteBackward));
        assert_eq!(
            (shown(&state).0, shown(&state).1.as_str()),
            (true, ""),
            "a selection is deleted"
        );
    }

    // editing.md §6
    #[test]
    fn pasted_text_is_typed_into_the_panel() {
        let mut state = open(OUTLINE);
        let paste = |state: &mut State, text: &str| {
            let text = text.to_owned();
            let at = crate::app::Paste::Below;
            assert_eq!(state.update(Message::Pasted { text, at }), []);
        };
        state.update(Message::QuickAdd(QuickAddInput::Open));
        paste(&mut state, "Renew\r\npassport\n");
        assert_eq!(
            shown(&state).1,
            "Renew passport",
            "its lines joined with spaces"
        );
        assert_eq!(state.notice(), None);
        paste(&mut state, " for @ann");
        assert_eq!(shown(&state).1, "Renew passport for ann", "as typed");
        assert_eq!(state.notice(), Some(&Notice::TypedTag));
        assert_eq!(lines(&state), lines(&open(OUTLINE)), "no block pasted");
    }

    // editing.md §6
    #[test]
    fn blocks_copied_by_the_app_are_typed_as_the_text_they_show() {
        let mut state =
            open("INBOX: @inbox\n- buy milk @due(2026-10-12) @star\nWork:\n\t- plan\n- ");
        state.update(Message::FocusNext);
        for _ in 0..3 {
            state.update(Message::Select(crate::app::SelectionChange::ExtendDown));
        }
        state.update(Message::Copy);
        let paste = |state: &mut State, text: &str| {
            let text = text.to_owned();
            let at = crate::app::Paste::Below;
            state.update(Message::Pasted { text, at });
        };
        let copy = "- buy milk @due(2026-10-12) @star\r\nWork:\r\n\t- plan\r\n- \r\n";
        input(&mut state, QuickAddInput::Open);
        paste(&mut state, copy);
        assert_eq!(
            shown(&state).1,
            "buy milk Work",
            "the copied blocks' text, joined with spaces, without their children"
        );
        assert_eq!(state.notice(), None);
        paste(&mut state, " - buy milk @star");
        assert_eq!(
            shown(&state).1,
            "buy milk Work - buy milk star",
            "a line from elsewhere"
        );
    }

    // OQ-25
    #[test]
    fn tags_and_line_breaks_are_not_typed() {
        let mut state = open(OUTLINE);
        state.update(Message::QuickAdd(QuickAddInput::Open));
        typed(&mut state, "a ");
        assert_eq!(state.notice(), None, "typed text has no notice");
        typed(&mut state, "@x\nb ann@example.com");
        assert_eq!(shown(&state).1, "a xb ann@example.com");
        assert_eq!(state.notice(), Some(&Notice::TypedTag));
    }

    // D-96
    #[test]
    fn enter_needs_text_besides_the_dates() {
        let mut state = open(OUTLINE);
        state.update(Message::QuickAdd(QuickAddInput::Open));
        typed(&mut state, "tomorrow");
        assert_eq!(state.quick_add().unwrap().missing, Some(Missing::Text));
        input(&mut state, QuickAddInput::Add);
        assert!(state.quick_add().is_some(), "nothing added yet");
        assert_eq!(lines(&state), lines(&open(OUTLINE)));
    }

    // D-112
    #[test]
    fn a_weekday_abbreviation_in_the_text_stays_text() {
        let mut state = open(OUTLINE);
        state.update(Message::QuickAdd(QuickAddInput::Open));
        typed(&mut state, "Buy sun screen this fri");
        input(&mut state, QuickAddInput::Add);
        assert_eq!(lines(&state)[2], "\t- Buy sun screen @start(2026-10-09)");
    }

    // FR-MODE-07, FR-MODE-08, D-96, D-97
    #[test]
    fn enter_adds_the_task_with_its_dates_as_the_last_child_in_one_step() {
        let mut state = open(OUTLINE);
        state.update(Message::FocusLast);
        let focus = state.focused().map(|(block, _)| block.id());
        state.update(Message::QuickAdd(QuickAddInput::Open));
        typed(&mut state, "Buy milk tomorrow due next Mon.");
        let view = state.quick_add().unwrap();
        assert_eq!(view.missing, None);
        assert_eq!(
            view.dates
                .iter()
                .map(|date| (date.tag, date.date))
                .collect::<Vec<_>>(),
            [
                (DateTag::Start, date(2026, 10, 8)),
                (DateTag::Due, date(2026, 10, 19))
            ]
        );
        input(&mut state, QuickAddInput::Add);
        assert_eq!(state.quick_add(), None);
        assert_eq!(
            lines(&state)[..3],
            [
                "INBOX: @inbox",
                "\t- old",
                "\t- Buy milk. @start(2026-10-08) @due(2026-10-19)"
            ]
        );
        assert_eq!(
            state.notice(),
            Some(&Notice::QuickAdded {
                project: "INBOX".to_owned()
            })
        );
        assert_eq!(
            state.focused().map(|(block, _)| block.id()),
            focus,
            "the focus stays"
        );
        state.update(Message::Undo);
        assert_eq!(lines(&state), lines(&open(OUTLINE)));
    }

    // D-96
    #[test]
    fn a_note_keeps_no_dash_and_loses_the_colons_that_would_make_it_a_project() {
        let mut state = open(OUTLINE);
        state.update(Message::QuickAdd(QuickAddInput::Open));
        input(&mut state, QuickAddInput::Edit(TextEdit::DeleteBackward));
        typed(&mut state, "  Groceries:  today ");
        input(&mut state, QuickAddInput::Add);
        assert_eq!(
            lines(&state)[..3],
            ["INBOX: @inbox", "\t- old", "\tGroceries @start(2026-10-07)"]
        );
    }

    // D-96
    #[test]
    fn a_task_keeps_the_colon_ending_its_text() {
        let mut state = open(OUTLINE);
        state.update(Message::QuickAdd(QuickAddInput::Open));
        typed(&mut state, "Questions for Ana: tomorrow");
        input(&mut state, QuickAddInput::Add);
        assert_eq!(
            lines(&state)[2],
            "\t- Questions for Ana: @start(2026-10-08)"
        );
    }

    // D-96
    #[test]
    fn a_note_of_only_colons_and_dates_has_no_text() {
        for text in [":", " :: ", ":: tomorrow"] {
            let mut state = open(OUTLINE);
            state.update(Message::QuickAdd(QuickAddInput::Open));
            input(&mut state, QuickAddInput::Edit(TextEdit::DeleteBackward));
            typed(&mut state, text);
            assert_eq!(
                state.quick_add().unwrap().missing,
                Some(Missing::Text),
                "{text}"
            );
            input(&mut state, QuickAddInput::Add);
            assert!(state.quick_add().is_some(), "{text}: nothing added");
            assert_eq!(lines(&state), lines(&open(OUTLINE)), "{text}");
        }
    }

    // D-96, D-141
    #[test]
    fn a_destination_deleted_meanwhile_is_missing() {
        let mut state = open("INBOX: @inbox\n- a");
        state.update(Message::QuickAdd(QuickAddInput::Open));
        typed(&mut state, "b");
        let inbox = blocks::block(state.document(), state.inbox_project().unwrap()).unwrap();
        let ops = structure::delete(state.document(), &inbox)
            .into_iter()
            .collect();
        state.commit(ops);
        assert_eq!(state.quick_add().unwrap().destination, None);
        assert_eq!(state.quick_add().unwrap().missing, Some(Missing::Project));
        input(&mut state, QuickAddInput::Add);
        assert!(state.quick_add().is_some(), "nothing added");
        assert_eq!(lines(&state), ["- a"]);
    }

    // D-133
    #[test]
    fn a_slash_is_typed_as_text() {
        let mut state = open(OUTLINE);
        state.update(Message::QuickAdd(QuickAddInput::Open));
        typed(&mut state, "/a b/ /");
        input(&mut state, QuickAddInput::Add);
        assert_eq!(lines(&state)[2], "\t- /a b/ /");
    }

    // D-97
    #[test]
    fn without_takes_out_the_dates_and_the_space_before_each() {
        let text = "tomorrow buy milk due fri, then go today";
        let found = dates::find(text, TODAY);
        assert_eq!(
            without(text, &found),
            "tomorrow buy milk, then go",
            "a date that does not count stays"
        );
        assert_eq!(
            without("today  buy", &dates::find("today  buy", TODAY)),
            "buy"
        );
    }
}
