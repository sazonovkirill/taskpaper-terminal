//! Quick Add: a one-line prompt that adds a task or note, with the dates its words name, as the
//! last child of a project chosen in it or the INBOX or TODAY project it opens with
//! ([editing.md §6](../../../docs/specs/editing.md#6-quick-add-fr-mode-07-fr-mode-08),
//! FR-MODE-07, FR-MODE-08, D-96, D-97, D-98).

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

/// The project Quick Add opens with as its destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickAddTo {
    /// None: one is chosen with `/` (`qq`).
    Chosen,
    /// The INBOX project the Navigator lists apart (`qi`, D-75).
    Inbox,
    /// The TODAY project (`qt`, D-98).
    Today,
}

/// Opening Quick Add, or a key of the open panel, as an intent; the project picker's keys are
/// menu keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickAddInput {
    /// Open the panel with the destination this names (`qq`, `qi`, `qt`, D-96).
    Open(QuickAddTo),
    /// Change the text typed; `/` at the start of a word opens the project picker.
    Edit(TextEdit),
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
    /// A destination.
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
/// picker while `/` has it open.
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

    /// Whether `/` typed now opens the project picker: nothing is selected and the caret is at
    /// the start of the text or after a space.
    fn at_word_start(&self) -> bool {
        let before = &self.field.text()[..self.field.caret()];
        self.field.selection().is_none()
            && (before.is_empty() || before.ends_with(char::is_whitespace))
    }

    /// The line the text adds with the dates found in it on `today`: the text without them,
    /// as a task or a note that colons ending it do not make a project, then `@start` and
    /// `@due`; `None` when no text is left (D-96, D-97).
    fn line(&self, today: Date) -> Option<String> {
        let found = dates::find(self.field.text(), today);
        let text = without(self.field.text(), &found);
        if text.is_empty() {
            return None;
        }
        let typed = if self.task { Typed::Task } else { Typed::Text };
        let (parts, text) = LineParts::default().retyped(typed, &text);
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

    /// Quick Add's project picker, while `/` has it open.
    #[must_use]
    pub fn quick_add_picker(&self) -> Option<FinderView<'_, ItemId>> {
        self.quick_add.as_ref()?.picker.as_ref().map(Finder::view)
    }

    /// Whether `qt` adds to the `TODAY` project and the Today view lists its tasks
    /// (`special_today`, D-98, D-102).
    #[must_use]
    pub fn special_today(&self) -> bool {
        self.special_today
    }

    /// The destination of `quick_add` while it is in the document.
    fn quick_add_destination(&self, quick_add: &QuickAdd) -> Option<blocks::Block> {
        quick_add
            .destination
            .and_then(|id| blocks::block(&self.document, id))
    }

    /// `qq`, `qi` and `qt`: opens Quick Add with the destination `to` names, or, when that
    /// project is missing, says so and opens nothing. A read-only file refuses with a notice.
    fn open_quick_add(&mut self, to: QuickAddTo) {
        if self.access() == Access::ReadOnly {
            self.notify(Notice::ReadOnly);
            return;
        }
        let destination = match to {
            QuickAddTo::Chosen => None,
            QuickAddTo::Inbox => {
                let Some(inbox) = self.inbox_project() else {
                    self.notify(Notice::NoInbox);
                    return;
                };
                Some(inbox)
            }
            QuickAddTo::Today => {
                let Some(today) = self.today_project(&self.pane) else {
                    self.notify(Notice::NoTodayProject);
                    return;
                };
                Some(today)
            }
        };
        self.quick_add = Some(QuickAdd {
            task: true,
            field: TextField::new(String::new()),
            destination,
            picker: None,
        });
    }

    /// Applies `input`: opens the panel, or changes, adds or closes the open one; adding puts
    /// the block in as one step to undo and closes the panel, saying where the block went.
    pub(super) fn quick_add_input(&mut self, input: QuickAddInput) {
        match input {
            QuickAddInput::Open(to) => self.open_quick_add(to),
            QuickAddInput::Edit(edit) => self.quick_add_edit(edit),
            QuickAddInput::Cancel => self.quick_add = None,
            QuickAddInput::Add => self.add_quickly(),
        }
    }

    /// Applies `edit` to the open panel's text: `/` at the start of a word opens the project
    /// picker, and `Backspace` at the start of a task's text makes it a note.
    fn quick_add_edit(&mut self, edit: TextEdit) {
        let Some(mut quick_add) = self.quick_add.take() else {
            return;
        };
        match edit {
            TextEdit::Insert('/') if quick_add.at_word_start() => {
                let candidates = self.project_candidates();
                if candidates.is_empty() {
                    quick_add.edit(edit);
                } else {
                    quick_add.picker = Some(Finder::new(candidates));
                }
            }
            TextEdit::DeleteBackward
                if quick_add.task
                    && quick_add.field.caret() == 0
                    && quick_add.field.selection().is_none() =>
            {
                quick_add.task = false;
            }
            _ => {
                if !quick_add.edit(edit) {
                    self.notify(Notice::TypedTag);
                }
            }
        }
        self.quick_add = Some(quick_add);
    }

    /// Types `text` into the open panel's text, as [`QuickAdd::paste`] does, unless its project
    /// picker is open, which takes no text pasted; `false` when no panel is open.
    pub(super) fn quick_add_paste(&mut self, text: &str) -> bool {
        let Some(quick_add) = &mut self.quick_add else {
            return false;
        };
        if quick_add.picker.is_none() && !quick_add.paste(text) {
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

    /// Applies `input` to Quick Add's open project picker: choosing makes the highlighted
    /// project the destination; `Esc`, or a character after which nothing matches, closes it and
    /// types `/` and the filter, and `Backspace` on an empty filter closes it (editing.md §6).
    pub(super) fn quick_add_picker_input(&mut self, input: MenuInput) {
        let Some(quick_add) = &mut self.quick_add else {
            return;
        };
        let Some(picker) = &mut quick_add.picker else {
            return;
        };
        let typed = format!("/{}", picker.view().query);
        let literal = match input {
            MenuInput::Erase if picker.view().query.is_empty() => None,
            MenuInput::Dismiss => Some(typed),
            MenuInput::Type(c) => {
                picker.input(input);
                if !picker.view().entries.is_empty() {
                    return;
                }
                Some(format!("{typed}{c}"))
            }
            MenuInput::Choose => {
                if let Some(Picked::Chosen(id)) = picker.input(input) {
                    quick_add.destination = Some(id);
                }
                None
            }
            MenuInput::Erase | MenuInput::Previous | MenuInput::Next => {
                picker.input(input);
                return;
            }
        };
        quick_add.picker = None;
        if let Some(text) = literal
            && !quick_add.paste(&text)
        {
            self.notify(Notice::TypedTag);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use jiff::civil::date;

    use super::*;
    use crate::{
        app::{Configuration, Menu, Message, Motion, Notice},
        document::format,
    };

    /// A Wednesday.
    const TODAY: Date = date(2026, 10, 7);

    const OUTLINE: &str = "INBOX:\n\t- old\nWork:\n\tLaunch:\n\t\t- plan\nTODAY:\n- loose";

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

    fn pick(state: &mut State, input: MenuInput) {
        assert_eq!(state.update(Message::Menu(Menu::QuickAdd, input)), []);
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

    // FR-MODE-07, D-96
    #[test]
    fn qq_opens_an_empty_task_without_a_destination() {
        let mut state = open(OUTLINE);
        input(&mut state, QuickAddInput::Edit(TextEdit::Insert('x')));
        input(&mut state, QuickAddInput::Add);
        assert_eq!(state.quick_add(), None, "keys without a panel do nothing");
        assert_eq!(lines(&state).len(), 7);
        state.update(Message::QuickAdd(QuickAddInput::Open(QuickAddTo::Chosen)));
        let view = state.quick_add().unwrap();
        assert_eq!(
            view,
            QuickAddView {
                task: true,
                text: "",
                caret: 0,
                selection: None,
                dates: Vec::new(),
                destination: None,
                missing: Some(Missing::Text),
            }
        );
        assert_eq!(state.quick_add_picker(), None);
    }

    // D-96, D-98
    #[test]
    fn qi_and_qt_open_on_the_inbox_and_today_projects_or_say_there_is_none() {
        let mut state = open(OUTLINE);
        state.update(Message::QuickAdd(QuickAddInput::Open(QuickAddTo::Inbox)));
        assert_eq!(shown(&state).2, Some(path(&["INBOX"])));
        input(&mut state, QuickAddInput::Cancel);
        assert_eq!(state.quick_add(), None);
        state.update(Message::QuickAdd(QuickAddInput::Open(QuickAddTo::Today)));
        assert_eq!(shown(&state).2, Some(path(&["TODAY"])));
        input(&mut state, QuickAddInput::Cancel);
        assert!(state.special_today(), "on by default");
        state.update(Message::Configure(Configuration::SpecialToday(false)));
        assert!(!state.special_today());
        state.update(Message::QuickAdd(QuickAddInput::Open(QuickAddTo::Today)));
        assert_eq!(state.quick_add(), None);
        assert_eq!(state.notice(), Some(&Notice::NoTodayProject));
        state.update(Message::Configure(Configuration::SpecialInbox(false)));
        state.update(Message::QuickAdd(QuickAddInput::Open(QuickAddTo::Inbox)));
        assert_eq!(state.quick_add(), None);
        assert_eq!(state.notice(), Some(&Notice::NoInbox));
        let mut state = open("Work:\n\tTODAY:\nToday:\n- TODAY");
        state.update(Message::QuickAdd(QuickAddInput::Open(QuickAddTo::Today)));
        assert_eq!(
            state.quick_add(),
            None,
            "only a top-level project named TODAY counts"
        );
        assert_eq!(state.notice(), Some(&Notice::NoTodayProject));
    }

    // FR-MODE-07, file-io.md §1
    #[test]
    fn a_read_only_file_refuses() {
        let mut state = open(OUTLINE).with_access(Access::ReadOnly);
        state.update(Message::QuickAdd(QuickAddInput::Open(QuickAddTo::Inbox)));
        assert_eq!(state.quick_add(), None);
        assert_eq!(state.notice(), Some(&Notice::ReadOnly));
    }

    // D-96, OQ-27
    #[test]
    fn backspace_at_the_start_makes_a_note_and_a_typed_dash_a_task_again() {
        let mut state = open(OUTLINE);
        state.update(Message::QuickAdd(QuickAddInput::Open(QuickAddTo::Inbox)));
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

    // editing.md §6: the panel takes pasted text as typed, and its project picker none.
    #[test]
    fn pasted_text_is_typed_into_the_panel_but_not_its_picker() {
        let mut state = open(OUTLINE);
        let paste = |state: &mut State, text: &str| {
            let text = text.to_owned();
            let at = crate::app::Paste::Below;
            assert_eq!(state.update(Message::Pasted { text, at }), []);
        };
        state.update(Message::QuickAdd(QuickAddInput::Open(QuickAddTo::Inbox)));
        paste(&mut state, "Renew\r\n passport");
        assert_eq!(shown(&state).1, "Renew passport");
        assert_eq!(state.notice(), None);
        paste(&mut state, " for @ann");
        assert_eq!(shown(&state).1, "Renew passport for ann", "as typed");
        assert_eq!(state.notice(), Some(&Notice::TypedTag));
        typed(&mut state, " /");
        let all = listed(&state);
        paste(&mut state, "Wo");
        assert_eq!(shown(&state).1, "Renew passport for ann ");
        assert_eq!(listed(&state), all, "the picker's filter is unchanged");
        assert_eq!(lines(&state), lines(&open(OUTLINE)), "no block pasted");
    }

    // OQ-25
    #[test]
    fn tags_and_line_breaks_are_not_typed() {
        let mut state = open(OUTLINE);
        state.update(Message::QuickAdd(QuickAddInput::Open(QuickAddTo::Inbox)));
        typed(&mut state, "a @x\nb ann@example.com");
        assert_eq!(shown(&state).1, "a xb ann@example.com");
        assert_eq!(state.notice(), Some(&Notice::TypedTag));
    }

    // D-96
    #[test]
    fn enter_needs_text_besides_the_dates_and_a_destination() {
        let mut state = open(OUTLINE);
        state.update(Message::QuickAdd(QuickAddInput::Open(QuickAddTo::Chosen)));
        typed(&mut state, "tomorrow");
        assert_eq!(state.quick_add().unwrap().missing, Some(Missing::Text));
        input(&mut state, QuickAddInput::Add);
        typed(&mut state, " milk");
        assert_eq!(state.quick_add().unwrap().missing, Some(Missing::Project));
        input(&mut state, QuickAddInput::Add);
        assert!(state.quick_add().is_some(), "nothing added yet");
        assert_eq!(lines(&state), lines(&open(OUTLINE)));
    }

    // FR-MODE-07, FR-MODE-08, D-96, D-97
    #[test]
    fn enter_adds_the_task_with_its_dates_as_the_last_child_in_one_step() {
        let mut state = open(OUTLINE);
        state.update(Message::FocusLast);
        let focus = state.focused().map(|(block, _)| block.id());
        state.update(Message::QuickAdd(QuickAddInput::Open(QuickAddTo::Inbox)));
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
                "INBOX:",
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
        state.update(Message::QuickAdd(QuickAddInput::Open(QuickAddTo::Today)));
        input(&mut state, QuickAddInput::Edit(TextEdit::DeleteBackward));
        typed(&mut state, "  Groceries:  today ");
        input(&mut state, QuickAddInput::Add);
        assert_eq!(
            lines(&state)[5..7],
            ["TODAY:", "\tGroceries @start(2026-10-07)"]
        );
    }

    // D-96
    #[test]
    fn a_destination_deleted_meanwhile_counts_as_none() {
        let mut state = open("TODAY:\n- a");
        state.update(Message::QuickAdd(QuickAddInput::Open(QuickAddTo::Today)));
        typed(&mut state, "b");
        let today = state.top_project("TODAY").unwrap();
        let ops = structure::delete(state.document(), &today)
            .into_iter()
            .collect();
        state.commit(ops);
        assert_eq!(state.quick_add().unwrap().destination, None);
        assert_eq!(state.quick_add().unwrap().missing, Some(Missing::Project));
        input(&mut state, QuickAddInput::Add);
        assert_eq!(lines(&state), ["- a"]);
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

    // D-96, D-65
    #[test]
    fn slash_at_a_word_start_picks_a_project_and_elsewhere_is_typed() {
        let mut state = open(OUTLINE);
        state.update(Message::QuickAdd(QuickAddInput::Open(QuickAddTo::Chosen)));
        typed(&mut state, "a/b ");
        assert_eq!(shown(&state).1, "a/b ");
        assert_eq!(state.quick_add_picker(), None);
        typed(&mut state, "/");
        assert_eq!(
            listed(&state),
            (
                ["INBOX", "Work", "Launch", "TODAY"]
                    .map(str::to_owned)
                    .to_vec(),
                0
            )
        );
        pick(&mut state, MenuInput::Type('l'));
        pick(&mut state, MenuInput::Type('a'));
        assert_eq!(listed(&state).0, ["Launch"]);
        pick(&mut state, MenuInput::Previous);
        pick(&mut state, MenuInput::Next);
        pick(&mut state, MenuInput::Erase);
        assert_eq!(listed(&state).0, ["Launch"]);
        pick(&mut state, MenuInput::Erase);
        assert_eq!(listed(&state).0.len(), 4);
        pick(&mut state, MenuInput::Next);
        pick(&mut state, MenuInput::Next);
        assert_eq!(listed(&state).1, 2);
        pick(&mut state, MenuInput::Previous);
        pick(&mut state, MenuInput::Type('l'));
        pick(&mut state, MenuInput::Choose);
        assert_eq!(state.quick_add_picker(), None);
        assert_eq!(
            shown(&state),
            (true, "a/b ".to_owned(), Some(path(&["Work", "Launch"])))
        );
        input(&mut state, QuickAddInput::Add);
        assert_eq!(lines(&state)[5], "\t\t- a/b");
        let mut state = open(OUTLINE);
        state.update(Message::QuickAdd(QuickAddInput::Open(QuickAddTo::Inbox)));
        input(&mut state, QuickAddInput::Edit(TextEdit::Insert('/')));
        assert!(
            state.quick_add_picker().is_some(),
            "at the start of the text"
        );
        pick(&mut state, MenuInput::Choose);
        assert_eq!(shown(&state).2, Some(path(&["INBOX"])));
        typed(&mut state, "x");
        input(&mut state, QuickAddInput::Edit(TextEdit::SelectAll));
        input(&mut state, QuickAddInput::Edit(TextEdit::Insert('/')));
        assert_eq!(state.quick_add_picker(), None, "not over a selection");
        assert_eq!(shown(&state).1, "/");
    }

    // D-96, D-44
    #[test]
    fn esc_or_a_filter_matching_nothing_types_the_slash_and_the_filter() {
        let mut state = open(OUTLINE);
        state.update(Message::QuickAdd(QuickAddInput::Open(QuickAddTo::Chosen)));
        typed(&mut state, "/");
        pick(&mut state, MenuInput::Type('w'));
        pick(&mut state, MenuInput::Type('o'));
        pick(&mut state, MenuInput::Dismiss);
        assert_eq!(state.quick_add_picker(), None);
        assert_eq!(shown(&state), (true, "/wo".to_owned(), None));
        typed(&mut state, " /");
        pick(&mut state, MenuInput::Type('z'));
        assert_eq!(state.quick_add_picker(), None);
        assert_eq!(shown(&state).1, "/wo /z");
        typed(&mut state, " /");
        pick(&mut state, MenuInput::Type('w'));
        pick(&mut state, MenuInput::Type('@'));
        assert_eq!(
            shown(&state).1,
            "/wo /z /w@",
            "an @ inside a word is no tag"
        );
        assert_eq!(state.notice(), None);
        typed(&mut state, " /");
        pick(&mut state, MenuInput::Erase);
        assert_eq!(
            state.quick_add_picker(),
            None,
            "Backspace on an empty filter closes it"
        );
        assert_eq!(shown(&state).1, "/wo /z /w@ ");
        let mut state = open("- no projects");
        state.update(Message::QuickAdd(QuickAddInput::Open(QuickAddTo::Chosen)));
        typed(&mut state, "/");
        assert_eq!(state.quick_add_picker(), None);
        assert_eq!(shown(&state).1, "/");
    }

    // OQ-25
    #[test]
    fn a_filter_typed_back_as_text_leaves_out_an_at_that_would_start_a_tag() {
        let mut state = open("Work:");
        state.update(Message::QuickAdd(QuickAddInput::Open(QuickAddTo::Chosen)));
        typed(&mut state, "/");
        pick(&mut state, MenuInput::Type(' '));
        assert!(
            state.quick_add_picker().is_some(),
            "a space filters nothing out"
        );
        pick(&mut state, MenuInput::Type('@'));
        assert_eq!(state.quick_add_picker(), None);
        assert_eq!(shown(&state).1, "/ ");
        assert_eq!(state.notice(), Some(&Notice::TypedTag));
    }

    #[test]
    fn keys_without_an_open_panel_or_picker_do_nothing() {
        let mut state = open(OUTLINE);
        input(&mut state, QuickAddInput::Add);
        pick(&mut state, MenuInput::Choose);
        state.update(Message::QuickAdd(QuickAddInput::Open(QuickAddTo::Chosen)));
        pick(&mut state, MenuInput::Choose);
        assert_eq!(state.quick_add_picker(), None);
        assert!(state.quick_add().is_some());
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
