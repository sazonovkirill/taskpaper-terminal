//! The date prompt that `ms` and `md` open: the text typed for a block's start or due date, the
//! date it resolves to as it is typed, and setting or removing that date
//! ([editing.md §5](../../../docs/specs/editing.md#5-block-commands), FR-TAG-03).

use std::{mem, ops::Range};

use jiff::civil::Date;

use super::{
    Access, Notice, State,
    text_field::{TextEdit, TextField},
};
use crate::document::{
    Block, DateTag, ItemId, blocks,
    dates::{self, DateError},
    format::breaks_line,
    metadata,
};

/// A key of the open date prompt, as an intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateInput {
    /// Change the text typed.
    Edit(TextEdit),
    /// Set the date the text resolves to, or remove it when the text is blank; a text that is
    /// not a date keeps the prompt open.
    Apply,
    /// Close the prompt, changing nothing.
    Cancel,
}

/// The open prompt as a frontend shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatePromptView<'a> {
    /// The date being set.
    pub tag: DateTag,
    /// The text typed.
    pub text: &'a str,
    /// The caret's byte offset into `text`.
    pub caret: usize,
    /// The selected bytes of `text`, if any.
    pub selection: Option<Range<usize>>,
    /// What applying would do: set this date, remove the date when `None`, or nothing, because
    /// the text is not a date for this reason.
    pub date: Result<Option<Date>, DateError>,
}

/// The open prompt: the blocks and date it sets, the local date it was opened on, which the text
/// is resolved against, and the text field.
#[derive(Debug, Clone)]
pub(super) struct DatePrompt {
    ids: Vec<ItemId>,
    tag: DateTag,
    today: Date,
    field: TextField,
}

impl DatePrompt {
    /// Types `text` as its characters would be typed, leaving out those that break lines, which
    /// the prompt's one line cannot hold.
    pub(super) fn paste(&mut self, text: &str) {
        for c in text.chars().filter(|&c| !breaks_line(c)) {
            self.field = self.field.edited(TextEdit::Insert(c));
        }
    }

    /// The date the text resolves to, or `None` for a blank text, which removes the date.
    fn date(&self) -> Result<Option<Date>, DateError> {
        match dates::parse(self.field.text(), self.today, self.tag) {
            Ok(date) => Ok(Some(date)),
            Err(DateError::Empty) => Ok(None),
            Err(error) => Err(error),
        }
    }
}

impl State {
    /// The date prompt, while it is open.
    #[must_use]
    pub fn date_prompt(&self) -> Option<DatePromptView<'_>> {
        let prompt = self.date_prompt.as_ref()?;
        Some(DatePromptView {
            tag: prompt.tag,
            text: prompt.field.text(),
            caret: prompt.field.caret(),
            selection: prompt.field.selection(),
            date: prompt.date(),
        })
    }

    /// Opens the prompt for the `tag` date of the selected blocks, else the focused block,
    /// resolving what is typed against `today`, with the focused block's date already typed and
    /// selected, so that typing replaces it and `Enter` keeps it (FR-OPS-07). A read-only file
    /// refuses with a notice.
    pub(super) fn open_date_prompt(&mut self, tag: DateTag, today: Date) {
        let Some((block, _)) = self.focused() else {
            return;
        };
        if self.access() == Access::ReadOnly {
            self.notify(Notice::ReadOnly);
            return;
        }
        let current = match tag {
            DateTag::Start => block.start(),
            DateTag::Due => block.due(),
        };
        let text = current.map(|date| date.to_string()).unwrap_or_default();
        self.date_prompt = Some(DatePrompt {
            ids: self.targets().iter().map(Block::id).collect(),
            tag,
            today,
            field: TextField::new(text).edited(TextEdit::SelectAll),
        });
    }

    /// Applies `input` to the open prompt; applying sets the date of the blocks the prompt was
    /// opened on that are still there as one step to undo (FR-OPS-08). A prompt the command menu
    /// opened returns to editing when it closes (D-50).
    pub(super) fn date_prompt_input(&mut self, input: DateInput) {
        let Some(prompt) = &mut self.date_prompt else {
            return;
        };
        match input {
            DateInput::Edit(edit) => prompt.field = prompt.field.edited(edit),
            DateInput::Cancel => self.date_prompt = None,
            DateInput::Apply => {
                let Ok(date) = prompt.date() else {
                    return;
                };
                let (ids, tag) = (mem::take(&mut prompt.ids), prompt.tag);
                self.date_prompt = None;
                let ops = ids
                    .into_iter()
                    .filter_map(|id| blocks::block(self.document(), id))
                    .filter_map(|block| metadata::set_date(self.document(), &block, tag, date))
                    .collect();
                self.commit_any(ops);
            }
        }
        self.resume_editing();
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;
    use crate::{
        app::{Message, Motion, SelectionChange},
        document::{Document, format},
    };

    const TODAY: Date = date(2026, 10, 5);

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

    fn prompt(tag: DateTag) -> Message {
        Message::OpenDatePrompt { tag, today: TODAY }
    }

    fn input(state: &mut State, input: DateInput) {
        assert_eq!(state.update(Message::DatePrompt(input)), []);
    }

    fn typed(state: &mut State, text: &str) {
        for c in text.chars() {
            input(state, DateInput::Edit(TextEdit::Insert(c)));
        }
    }

    /// The prompt's text, caret, selection and resolved date.
    type Shown = (
        String,
        usize,
        Option<Range<usize>>,
        Result<Option<Date>, DateError>,
    );

    fn shown(state: &State) -> Shown {
        let view = state.date_prompt().expect("the prompt is open");
        (view.text.to_owned(), view.caret, view.selection, view.date)
    }

    // FR-TAG-03, editing.md §5
    #[test]
    fn opens_for_the_focused_block_with_its_date_selected() {
        let mut state = open("- a\n- b @due(2026-10-20) @start(2026-10-01)", 1);
        assert_eq!(state.date_prompt(), None);
        assert_eq!(state.update(prompt(DateTag::Due)), []);
        let view = state.date_prompt().unwrap();
        assert_eq!(view.tag, DateTag::Due);
        assert_eq!(
            shown(&state),
            (
                "2026-10-20".to_owned(),
                10,
                Some(0..10),
                Ok(Some(date(2026, 10, 20)))
            )
        );
        input(&mut state, DateInput::Cancel);
        state.update(prompt(DateTag::Start));
        assert_eq!(state.date_prompt().unwrap().tag, DateTag::Start);
        assert_eq!(state.date_prompt().unwrap().text, "2026-10-01");
        input(&mut state, DateInput::Cancel);
        state.update(Message::FocusPrevious);
        state.update(prompt(DateTag::Start));
        assert_eq!(shown(&state), (String::new(), 0, None, Ok(None)));
    }

    // FR-TAG-03, search.md §4
    #[test]
    fn previews_the_date_as_it_is_typed() {
        let mut state = open("- a @due(2026-10-20)", 0);
        state.update(prompt(DateTag::Due));
        typed(&mut state, "next fri");
        assert_eq!(
            shown(&state),
            ("next fri".to_owned(), 8, None, Ok(Some(date(2026, 10, 16))))
        );
        typed(&mut state, "x");
        assert_eq!(shown(&state).3, Err(DateError::Unexpected { offset: 5 }));
        input(
            &mut state,
            DateInput::Edit(TextEdit::Move {
                motion: Motion::Start,
                select: true,
            }),
        );
        input(&mut state, DateInput::Edit(TextEdit::DeleteBackward));
        assert_eq!(shown(&state), (String::new(), 0, None, Ok(None)));
        assert_eq!(
            texts(&state),
            ["- a @due(2026-10-20)"],
            "nothing applied yet"
        );
    }

    // FR-TAG-03, D-6, FR-OPS-08
    #[test]
    fn enter_sets_the_date_as_one_step_to_undo() {
        let mut state = open("- a\n- b", 1);
        state.update(prompt(DateTag::Due));
        typed(&mut state, "tomorrow");
        input(&mut state, DateInput::Apply);
        assert_eq!(state.date_prompt(), None);
        assert_eq!(texts(&state), ["- a", "- b @due(2026-10-06)"]);
        state.update(prompt(DateTag::Start));
        typed(&mut state, "+1w");
        input(&mut state, DateInput::Apply);
        assert_eq!(
            texts(&state),
            ["- a", "- b @due(2026-10-06) @start(2026-10-12)"]
        );
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a", "- b @due(2026-10-06)"]);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a", "- b"]);
    }

    // FR-TAG-03, editing.md §5
    #[test]
    fn enter_on_an_empty_text_removes_the_date() {
        let mut state = open("- a @due(2026-10-20) @start(2026-10-01)", 0);
        state.update(prompt(DateTag::Due));
        input(&mut state, DateInput::Edit(TextEdit::DeleteBackward));
        input(&mut state, DateInput::Apply);
        assert_eq!(texts(&state), ["- a @start(2026-10-01)"]);
        assert_eq!(state.date_prompt(), None);
    }

    // FR-TAG-03, keybindings.md §4: a prompt takes pasted text as typed, on its one line.
    #[test]
    fn pasted_text_is_typed_into_the_prompt_without_line_breaks() {
        let paste = |state: &mut State, text: &str| {
            let at = crate::app::Paste::Below;
            let text = text.to_owned();
            assert_eq!(state.update(Message::Pasted { text, at }), []);
        };
        let mut state = open(
            "- a @due(2026-10-20)
- b",
            0,
        );
        state.update(prompt(DateTag::Due));
        paste(&mut state, "12-24\n");
        assert_eq!(
            shown(&state),
            ("12-24".to_owned(), 5, None, Ok(Some(date(2026, 12, 24))))
        );
        paste(&mut state, " +1\r\nd");
        assert_eq!(shown(&state).0, "12-24 +1d");
        assert_eq!(
            texts(&state),
            ["- a @due(2026-10-20)", "- b"],
            "no block pasted"
        );
        input(&mut state, DateInput::Apply);
        assert_eq!(texts(&state), ["- a @due(2026-12-25)", "- b"]);
        let mut starred = open("- a @star", 0);
        starred.update(Message::GoTo(crate::app::Location::View(
            crate::app::SpecialView::Starred,
        )));
        starred.update(prompt(DateTag::Due));
        paste(&mut starred, "fri");
        assert_eq!(shown(&starred).0, "fri", "in a special view too");
    }

    // search.md §4: the prompt rejects unparseable input.
    #[test]
    fn enter_on_a_text_that_is_no_date_keeps_the_prompt_open() {
        let mut state = open("- a", 0);
        state.update(prompt(DateTag::Due));
        typed(&mut state, "someday");
        input(&mut state, DateInput::Apply);
        assert_eq!(state.date_prompt().unwrap().text, "someday");
        assert_eq!(state.document().revision(), 0);
    }

    // FR-TAG-03
    #[test]
    fn esc_closes_the_prompt_and_keeps_the_date() {
        let mut state = open("- a @due(2026-10-20)", 0);
        state.update(prompt(DateTag::Due));
        typed(&mut state, "today");
        input(&mut state, DateInput::Cancel);
        assert_eq!(state.date_prompt(), None);
        assert_eq!(texts(&state), ["- a @due(2026-10-20)"]);
    }

    // FR-TAG-03
    #[test]
    fn keeping_the_date_as_it_was_records_no_step() {
        let mut state = open("- a @due(2026-10-20)", 0);
        state.update(prompt(DateTag::Due));
        input(&mut state, DateInput::Apply);
        assert_eq!(state.date_prompt(), None);
        assert_eq!(state.document().revision(), 0);
        state.update(prompt(DateTag::Start));
        input(&mut state, DateInput::Apply);
        assert_eq!(state.document().revision(), 0, "no date to remove");
    }

    // search.md §4, D-6: a month and day already past fall next year only for a due date.
    #[test]
    fn resolves_against_the_day_it_was_opened_for_its_tag() {
        let mut state = open("- a", 0);
        state.update(prompt(DateTag::Start));
        typed(&mut state, "01-15");
        assert_eq!(shown(&state).3, Ok(Some(date(2026, 1, 15))));
        input(&mut state, DateInput::Cancel);
        state.update(prompt(DateTag::Due));
        typed(&mut state, "01-15");
        input(&mut state, DateInput::Apply);
        assert_eq!(texts(&state), ["- a @due(2027-01-15)"]);
    }

    // FR-TAG-03, FR-MODE-01
    #[test]
    fn opening_ends_editing_and_works_on_the_zoom_title() {
        let mut state = open("P:\n\t- a", 1);
        state.update(Message::EditAtEnd);
        state.update(prompt(DateTag::Due));
        assert_eq!(state.mode(), crate::app::Mode::Navigate);
        assert!(state.date_prompt().is_some());
        input(&mut state, DateInput::Cancel);
        state.update(Message::ZoomOut);
        state.update(Message::FocusFirst);
        state.update(Message::ZoomIn);
        state.update(Message::FocusFirst);
        state.update(prompt(DateTag::Due));
        typed(&mut state, "today");
        input(&mut state, DateInput::Apply);
        assert_eq!(texts(&state), ["P: @due(2026-10-05)", "- a"]);
    }

    // file-io.md §1
    #[test]
    fn a_read_only_file_refuses_to_open_the_prompt() {
        let mut state = open("- a", 0).with_access(Access::ReadOnly);
        state.update(prompt(DateTag::Due));
        assert_eq!(state.date_prompt(), None);
        assert_eq!(state.notice(), Some(&Notice::ReadOnly));
    }

    #[test]
    fn nothing_opens_in_an_empty_document_and_input_without_a_prompt_does_nothing() {
        let mut state = State::new(Document::default(), None);
        state.update(prompt(DateTag::Due));
        assert_eq!(state.date_prompt(), None);
        let mut state = open("- a", 0);
        for each in [
            DateInput::Edit(TextEdit::Insert('x')),
            DateInput::Apply,
            DateInput::Cancel,
        ] {
            input(&mut state, each);
        }
        assert_eq!(texts(&state), ["- a"]);
        assert_eq!(state.date_prompt(), None);
    }

    #[test]
    fn a_block_gone_meanwhile_is_not_dated() {
        let mut state = open("- a\n- b", 1);
        state.update(prompt(DateTag::Due));
        state.update(Message::Delete);
        typed(&mut state, "today");
        input(&mut state, DateInput::Apply);
        assert_eq!(texts(&state), ["- a"]);
        assert_eq!(state.date_prompt(), None);
    }

    // FR-OPS-07, FR-TAG-03, FR-OPS-08
    #[test]
    fn enter_sets_the_date_of_every_selected_block_as_one_step() {
        let mut state = open("- a @due(2026-10-01)\n- b\n- c", 0);
        state.update(Message::Select(SelectionChange::ExtendDown));
        state.update(prompt(DateTag::Due));
        assert_eq!(shown(&state).0, "", "the focused b's date");
        typed(&mut state, "fri");
        input(&mut state, DateInput::Apply);
        assert_eq!(
            texts(&state),
            ["- a @due(2026-10-09)", "- b @due(2026-10-09)", "- c"]
        );
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a @due(2026-10-01)", "- b", "- c"]);
    }
}
