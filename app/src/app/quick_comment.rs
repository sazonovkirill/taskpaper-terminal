//! Quick Comment: a one-line prompt that adds the comment typed in it to the focused block
//! without opening the attachments panel
//! ([attachments.md §5](../../../docs/specs/attachments.md#5-comments-fr-att-05), FR-ATT-07).

use std::{ops::Range, sync::Arc};

use jiff::civil::DateTime;

use super::{
    Access, Notice, State,
    text_field::{TextEdit, TextField},
};
use crate::document::{Attachment, ItemId};

/// Opening Quick Comment, or a key of the open prompt, as an intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickCommentInput {
    /// Open the prompt on the focused block.
    Open,
    /// Change the text typed.
    Edit(TextEdit),
    /// Add the text typed as the block's newest comment, stamped with this local time, and close.
    Add(DateTime),
    /// Close the prompt, adding nothing.
    Cancel,
}

/// The open prompt as a frontend shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickCommentView<'a> {
    /// The block's text without its formatting, which the prompt's title names.
    pub title: Arc<str>,
    /// The text typed.
    pub text: &'a str,
    /// The caret's byte offset into `text`.
    pub caret: usize,
    /// The selected bytes of `text`, if any.
    pub selection: Option<Range<usize>>,
}

/// The open prompt: the block the comment goes to and the text typed.
#[derive(Debug, Clone)]
pub(super) struct QuickComment {
    block: ItemId,
    field: TextField,
}

impl State {
    /// Quick Comment, while it is open.
    #[must_use]
    pub fn quick_comment(&self) -> Option<QuickCommentView<'_>> {
        let quick_comment = self.quick_comment.as_ref()?;
        Some(QuickCommentView {
            title: self.document.get(quick_comment.block)?.shared_plain_text(),
            text: quick_comment.field.text(),
            caret: quick_comment.field.caret(),
            selection: quick_comment.field.selection(),
        })
    }

    /// Applies `input`: opens the prompt, or changes, adds or closes the open one.
    pub(super) fn quick_comment_input(&mut self, input: QuickCommentInput) {
        match input {
            QuickCommentInput::Open => self.open_quick_comment(),
            QuickCommentInput::Edit(edit) => {
                if let Some(quick_comment) = &mut self.quick_comment {
                    quick_comment.field = quick_comment.field.edited(edit);
                }
            }
            QuickCommentInput::Add(at) => {
                if let Some(QuickComment { block, field }) = self.quick_comment.take()
                    && self.add_comment(block, at, field.text())
                {
                    self.notify(Notice::CommentAdded);
                }
            }
            QuickCommentInput::Cancel => self.quick_comment = None,
        }
    }

    /// Opens the prompt on the focused block, a divider aside, which has no comments, while
    /// comments are turned on; a read-only file refuses with a notice (attachments.md §5, §6).
    fn open_quick_comment(&mut self) {
        let Some((block, _)) = self.focused() else {
            return;
        };
        if block.divider() || !self.attached.contains(&Attachment::Comments) {
            return;
        }
        if self.access() == Access::ReadOnly {
            self.notify(Notice::ReadOnly);
            return;
        }
        self.quick_comment = Some(QuickComment {
            block: block.id(),
            field: TextField::new(String::new()),
        });
    }

    /// Types `text` into the open prompt as one line, as [`State::one_line`] gives it; `false`
    /// when no prompt is open.
    pub(super) fn quick_comment_paste(&mut self, text: &str) -> bool {
        let line = self.one_line(text);
        let Some(quick_comment) = &mut self.quick_comment else {
            return false;
        };
        quick_comment.field = quick_comment.field.pasted(&line);
        true
    }

    /// Keeps the open prompt on the block `exact` finds for its own in a reloaded document, and
    /// closes it when it finds none (file-io.md §3).
    pub(super) fn rematch_quick_comment(&mut self, exact: impl Fn(ItemId) -> Option<ItemId>) {
        self.quick_comment = self.quick_comment.take().and_then(|quick_comment| {
            Some(QuickComment {
                block: exact(quick_comment.block)?,
                ..quick_comment
            })
        });
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use jiff::civil::date;

    use super::*;
    use crate::{
        app::{AttachmentsInput, Configuration, Location, Message, Paste, SpecialView},
        document::format,
    };

    const TEXT: &str =
        "- Call the bank\n\tCOMMENTS @hidden\n\t\t[2026-10-08 09:00] busy\n---\n- Pay\n";

    const NOW: DateTime = date(2026, 10, 30).at(15, 20, 0, 0);

    fn state(text: &str) -> State {
        State::new(format::parse(text), None)
    }

    fn input(state: &mut State, input: QuickCommentInput) {
        state.update(Message::Attachments(AttachmentsInput::QuickComment(input)));
    }

    fn typed(state: &mut State, text: &str) {
        for c in text.chars() {
            input(state, QuickCommentInput::Edit(TextEdit::Insert(c)));
        }
    }

    fn file(state: &State) -> String {
        format::serialize(state.document())
    }

    // FR-ATT-07, attachments.md §5
    #[test]
    fn enter_adds_the_text_as_the_newest_comment_stamped_now_as_one_step_to_undo() {
        let mut state = state(TEXT);
        typed(&mut state, "x");
        input(&mut state, QuickCommentInput::Add(NOW));
        assert_eq!(
            state.quick_comment(),
            None,
            "keys without a prompt do nothing"
        );
        assert_eq!(file(&state), TEXT);
        input(&mut state, QuickCommentInput::Open);
        typed(&mut state, "Called back");
        assert_eq!(
            state.quick_comment(),
            Some(QuickCommentView {
                title: "Call the bank".into(),
                text: "Called back",
                caret: 11,
                selection: None,
            })
        );
        input(&mut state, QuickCommentInput::Add(NOW));
        assert_eq!(state.quick_comment(), None);
        assert_eq!(state.notice(), Some(&Notice::CommentAdded));
        assert_eq!(
            file(&state),
            "- Call the bank\n\tCOMMENTS @hidden\n\t\t[2026-10-30 15:20] Called back\n\t\t\
             [2026-10-08 09:00] busy\n---\n- Pay\n"
        );
        state.update(Message::Undo);
        assert_eq!(file(&state), TEXT);
    }

    // FR-ATT-07, attachments.md §5
    #[test]
    fn the_first_comment_makes_the_comments_node_and_esc_or_only_spaces_add_nothing() {
        let mut state = state(TEXT);
        state.update(Message::FocusLast);
        input(&mut state, QuickCommentInput::Open);
        typed(&mut state, "  ");
        input(&mut state, QuickCommentInput::Add(NOW));
        assert_eq!(state.quick_comment(), None, "Enter closes it");
        assert_eq!(state.notice(), None);
        input(&mut state, QuickCommentInput::Open);
        typed(&mut state, "soon");
        input(&mut state, QuickCommentInput::Cancel);
        assert_eq!(state.quick_comment(), None);
        assert_eq!(file(&state), TEXT);
        input(&mut state, QuickCommentInput::Open);
        typed(&mut state, "soon");
        input(&mut state, QuickCommentInput::Add(NOW));
        assert_eq!(
            file(&state),
            TEXT.to_owned() + "\tCOMMENTS @hidden\n\t\t[2026-10-30 15:20] soon\n"
        );
    }

    // FR-ATT-07, attachments.md §1, §6, file-io.md §1
    #[test]
    fn a_divider_comments_turned_off_or_a_read_only_file_open_nothing() {
        let mut state = state(TEXT);
        state.update(Message::FocusNext);
        input(&mut state, QuickCommentInput::Open);
        assert_eq!(state.quick_comment(), None, "a divider");
        state.update(Message::FocusFirst);
        state.update(Message::Configure(Configuration::Attachments(
            Attachment::Comments,
            false,
        )));
        input(&mut state, QuickCommentInput::Open);
        assert_eq!(state.quick_comment(), None, "comments are off");
        state.update(Message::Configure(Configuration::Attachments(
            Attachment::Note,
            false,
        )));
        state.update(Message::Configure(Configuration::Attachments(
            Attachment::Comments,
            true,
        )));
        input(&mut state, QuickCommentInput::Open);
        state.update(Message::Configure(Configuration::Attachments(
            Attachment::Note,
            false,
        )));
        state.update(Message::Configure(Configuration::Attachments(
            Attachment::Comments,
            true,
        )));
        assert!(state.quick_comment().is_some(), "notes need not be on");
        state.update(Message::Configure(Configuration::Attachments(
            Attachment::Comments,
            false,
        )));
        assert_eq!(
            state.quick_comment(),
            None,
            "turning comments off closes it"
        );
        let mut empty = self::state("");
        input(&mut empty, QuickCommentInput::Open);
        assert_eq!(empty.quick_comment(), None, "nothing is focused");
        let mut read_only = State::new(format::parse(TEXT), Some(PathBuf::from("/a.taskpaper")))
            .with_access(Access::ReadOnly);
        input(&mut read_only, QuickCommentInput::Open);
        assert_eq!(read_only.quick_comment(), None);
        assert_eq!(read_only.notice(), Some(&Notice::ReadOnly));
    }

    // FR-ATT-07, FR-OPS-06, attachments.md §5
    #[test]
    fn a_paste_goes_in_as_one_line_even_in_a_special_view() {
        let mut state = state("- Call the bank @star\n");
        state.update(Message::GoTo(Location::View(SpecialView::Starred)));
        input(&mut state, QuickCommentInput::Open);
        typed(&mut state, "a");
        state.update(Message::Pasted {
            text: "b\nc\n".to_owned(),
            at: Paste::Below,
        });
        assert_eq!(state.quick_comment().unwrap().text, "ab c");
        input(&mut state, QuickCommentInput::Add(NOW));
        assert_eq!(
            file(&state),
            "- Call the bank @star\n\tCOMMENTS @hidden\n\t\t[2026-10-30 15:20] ab c\n"
        );
    }
}
