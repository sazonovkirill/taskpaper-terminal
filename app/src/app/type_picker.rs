//! The type picker that `my` opens: the six block types and what choosing one does to the
//! selected or focused blocks
//! ([editing.md §5](../../../docs/specs/editing.md#5-block-commands), FR-BLK-08).

use super::{
    MenuInput, Notice, State,
    picker::{Picked, Picker, PickerView},
};
use crate::document::{BlockKind, ConvertError, metadata};

/// Every entry of the picker with its name, in the order it lists them: the type menu's (D-44),
/// with URL before Divider.
const ENTRIES: [(BlockKind, &str); 6] = [
    (BlockKind::Project, "Project"),
    (BlockKind::Section, "Section"),
    (BlockKind::Text, "Note"),
    (BlockKind::Task, "Task"),
    (BlockKind::Url, "URL"),
    (BlockKind::Divider, "Divider"),
];

impl State {
    /// The type picker, while it is open.
    #[must_use]
    pub fn type_picker(&self) -> Option<PickerView<'_, BlockKind>> {
        self.type_picker.as_ref().map(Picker::view)
    }

    /// `my`: opens the type picker on the focused block, its type highlighted (FR-BLK-08).
    pub(super) fn open_type_picker(&mut self) {
        if let Some((block, _)) = self.focused() {
            self.type_picker = Some(Picker::new(&ENTRIES, block.kind()));
        }
    }

    /// Applies `input` to the open type picker, and once a type is chosen, gives it to the
    /// selected blocks, else the focused block.
    pub(super) fn type_picker_input(&mut self, input: MenuInput) {
        let Some(closed) = self
            .type_picker
            .as_mut()
            .and_then(|picker| picker.input(input))
        else {
            return;
        };
        self.type_picker = None;
        if let Picked::Chosen(kind) = closed {
            self.convert_type(kind);
        }
    }

    /// Gives the selected blocks, else the focused block, the type `kind` as one step, and says
    /// when a block cannot take it (FR-BLK-08, FR-OPS-07).
    fn convert_type(&mut self, kind: BlockKind) {
        let mut no_address = false;
        let ops = self
            .targets()
            .iter()
            .filter_map(|block| {
                metadata::convert_type(self.document(), block, kind).unwrap_or_else(
                    |ConvertError::NoAddress| {
                        no_address = true;
                        None
                    },
                )
            })
            .collect();
        if no_address {
            self.notify(Notice::NoAddress);
        }
        self.commit_any(ops);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::{
        app::{Access, Message, SelectionChange},
        document::{Document, format},
    };

    /// Each line's text.
    fn texts(state: &State) -> Vec<String> {
        let document = state.document();
        document
            .descendants(document.root())
            .map(|(id, _)| document.get(id).unwrap().text().to_owned())
            .collect()
    }

    fn pick(state: &mut State, input: MenuInput) {
        state.update(Message::TypePicker(input));
    }

    /// The names the open picker lists and the highlighted one.
    fn listed(state: &State) -> (Vec<&'static str>, usize) {
        let view = state.type_picker().unwrap();
        (
            view.entries.iter().map(|&(_, name)| name).collect(),
            view.selected,
        )
    }

    // FR-BLK-08, D-44
    #[test]
    fn the_picker_opens_on_the_blocks_type_and_converts_to_the_chosen_one() {
        let mut state = State::new(format::parse("- a @x\nb"), None);
        state.update(Message::OpenTypePicker);
        assert_eq!(
            listed(&state),
            (
                vec!["Project", "Section", "Note", "Task", "URL", "Divider"],
                3
            )
        );
        pick(&mut state, MenuInput::Previous);
        pick(&mut state, MenuInput::Choose);
        assert_eq!(state.type_picker(), None);
        assert_eq!(texts(&state), ["a @x", "b"]);
        assert_eq!(state.rows()[0].kind, BlockKind::Text);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a @x", "b"]);
        state.update(Message::FocusNext);
        state.update(Message::OpenTypePicker);
        assert_eq!(listed(&state).1, 2, "a text block highlights Note");
        for c in "sec".chars() {
            pick(&mut state, MenuInput::Type(c));
        }
        assert_eq!(listed(&state), (vec!["Section"], 0));
        pick(&mut state, MenuInput::Choose);
        assert_eq!(texts(&state), ["- a @x", "b @section"]);
        assert_eq!(state.rows()[1].kind, BlockKind::Section);
    }

    // FR-BLK-08, OQ-30
    #[test]
    fn a_url_block_needs_a_url_in_the_text() {
        let mut state = State::new(format::parse("Docs https://a.b\n- none"), None);
        state.update(Message::OpenTypePicker);
        pick(&mut state, MenuInput::Type('u'));
        pick(&mut state, MenuInput::Type('r'));
        pick(&mut state, MenuInput::Choose);
        assert_eq!(texts(&state), ["Docs @url(https://a.b)", "- none"]);
        assert_eq!(state.rows()[0].kind, BlockKind::Url);
        state.update(Message::FocusNext);
        state.update(Message::OpenTypePicker);
        pick(&mut state, MenuInput::Type('U'));
        pick(&mut state, MenuInput::Choose);
        assert_eq!(state.notice(), Some(&Notice::NoAddress));
        assert_eq!(texts(&state), ["Docs @url(https://a.b)", "- none"]);
        assert_eq!(state.document().revision(), 1);
    }

    // FR-BLK-08
    #[test]
    fn choosing_the_blocks_own_type_or_dismissing_changes_nothing() {
        let mut state = State::new(format::parse("P: @x"), None);
        state.update(Message::OpenTypePicker);
        pick(&mut state, MenuInput::Choose);
        assert_eq!(state.type_picker(), None);
        state.update(Message::OpenTypePicker);
        pick(&mut state, MenuInput::Next);
        pick(&mut state, MenuInput::Dismiss);
        assert_eq!(state.type_picker(), None);
        assert_eq!(texts(&state), ["P: @x"]);
        assert_eq!(state.document().revision(), 0);
        pick(&mut state, MenuInput::Choose);
        assert_eq!(state.document().revision(), 0, "no picker, no change");
    }

    #[test]
    fn the_picker_needs_a_focused_block_and_a_writable_file() {
        let mut empty = State::new(Document::default(), None);
        empty.update(Message::OpenTypePicker);
        assert_eq!(empty.type_picker(), None);
        let mut state = State::new(
            format::parse("- a"),
            Some(PathBuf::from("/notes/todo.taskpaper")),
        )
        .with_access(Access::ReadOnly);
        state.update(Message::OpenTypePicker);
        pick(&mut state, MenuInput::Type('n'));
        pick(&mut state, MenuInput::Type('o'));
        pick(&mut state, MenuInput::Choose);
        assert_eq!(texts(&state), ["- a"]);
        assert_eq!(state.notice(), Some(&Notice::ReadOnly));
    }

    // FR-BLK-08, FR-OPS-07, OQ-30
    #[test]
    fn every_selected_block_takes_the_type_as_one_step() {
        let mut state = State::new(
            format::parse(
                "- a
b https://b.c
P:
- d",
            ),
            None,
        );
        for _ in 0..2 {
            state.update(Message::Select(SelectionChange::ExtendDown));
        }
        state.update(Message::OpenTypePicker);
        assert_eq!(listed(&state).1, 0, "the focused block's Project");
        pick(&mut state, MenuInput::Type('u'));
        pick(&mut state, MenuInput::Choose);
        assert_eq!(
            texts(&state),
            ["- a", "b @url(https://b.c)", "P:", "- d"],
            "the blocks without a URL keep their type"
        );
        assert_eq!(state.notice(), Some(&Notice::NoAddress));
        state.update(Message::OpenTypePicker);
        pick(&mut state, MenuInput::Next);
        pick(&mut state, MenuInput::Next);
        pick(&mut state, MenuInput::Next);
        pick(&mut state, MenuInput::Choose);
        assert_eq!(texts(&state), ["- a", "- b", "- P", "- d"]);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a", "b @url(https://b.c)", "P:", "- d"]);
    }
}
