//! The details popup that `mi` opens over the focused block: its tags, supported or not, and the
//! special nodes it owns ([editing.md §5](../../../docs/specs/editing.md#5-block-commands), D-3).

use super::State;
use crate::document::{Details, blocks};

impl State {
    /// The focused block's details, while the popup is open (D-3).
    #[must_use]
    pub fn details(&self) -> Option<Details> {
        self.details
            .then_some(self.pane.focus?)
            .and_then(|id| blocks::details(self.document(), id))
    }

    /// `mi`: opens the details popup on the focused block, if there is one.
    pub(super) fn open_details(&mut self) {
        self.details = self.focused().is_some();
    }

    /// Closes the details popup, returning to editing when the command menu opened it (D-50).
    pub(super) fn close_details(&mut self) {
        self.details = false;
        self.resume_editing();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::Message,
        document::{Document, format},
    };

    // D-3
    #[test]
    fn mi_shows_the_focused_blocks_details_until_closed() {
        let mut state = State::new(format::parse("- a @x\n- b @due(2026-10-08)"), None);
        assert_eq!(state.details(), None);
        state.update(Message::OpenDetails);
        let tags = |state: &State| state.details().map(|details| details.tags);
        assert_eq!(tags(&state), Some(vec![("@x".to_owned(), false)]));
        state.update(Message::FocusNext);
        assert_eq!(
            tags(&state),
            Some(vec![("@due(2026-10-08)".to_owned(), true)]),
            "the details follow the focus"
        );
        state.update(Message::CloseDetails);
        assert_eq!(state.details(), None);
        assert_eq!(state.document().revision(), 0, "read-only");
    }

    #[test]
    fn details_need_a_focused_block() {
        let mut empty = State::new(Document::default(), None);
        empty.update(Message::OpenDetails);
        assert_eq!(empty.details(), None);
        empty.document = format::parse("a");
        empty.pane.focus = empty.rows().first().map(|row| row.id);
        assert_eq!(empty.details(), None, "opening found nothing to show");
    }
}
