//! The type picker that `my` opens: the three block types and making sections, and what choosing
//! one does to the selected or focused blocks, with `S` and `s`, which make and unmake sections
//! ([editing.md §5](../../../docs/specs/editing.md#5-block-commands), FR-BLK-04, FR-BLK-08,
//! D-52).

use super::{
    Access, MenuInput, Notice, State,
    picker::{Picked, Picker, PickerView},
};
use crate::document::{BlockKind, format::Typed, metadata};

/// What the type picker gives the blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conversion {
    /// This type.
    To(Typed),
    /// `@section` to a text block, or with `false` taken off it (D-52).
    Section(bool),
}

/// Every entry of the picker with its name, in the order it lists them: the command menu's
/// types (D-52).
const ENTRIES: [(Conversion, &str); 5] = [
    (Conversion::To(Typed::Project), "Project"),
    (Conversion::To(Typed::Task), "Task"),
    (Conversion::To(Typed::Text), "Note"),
    (Conversion::Section(true), "Make Section"),
    (Conversion::Section(false), "Remove Section"),
];

impl State {
    /// The type picker, while it is open.
    #[must_use]
    pub fn type_picker(&self) -> Option<PickerView<'_, Conversion>> {
        self.type_picker.as_ref().map(Picker::view)
    }

    /// `my`: opens the type picker on the focused block, its type highlighted, Make Section for a
    /// section (FR-BLK-08).
    pub(super) fn open_type_picker(&mut self) {
        if let Some((block, _)) = self.focused() {
            let current = match block.kind() {
                BlockKind::Project => Conversion::To(Typed::Project),
                BlockKind::Task => Conversion::To(Typed::Task),
                BlockKind::Text if block.section() => Conversion::Section(true),
                BlockKind::Text => Conversion::To(Typed::Text),
            };
            self.type_picker = Some(Picker::new(&ENTRIES, current));
        }
    }

    /// Applies `input` to the open type picker, and once an entry is chosen, applies it to the
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
        match closed {
            Picked::Chosen(Conversion::To(typed)) => self.convert_type(typed),
            Picked::Chosen(Conversion::Section(on)) => self.set_section(on),
            Picked::Cancelled => {}
        }
    }

    /// Gives the selected blocks, else the focused block, the type `typed` as one step
    /// (FR-BLK-08, FR-OPS-07).
    fn convert_type(&mut self, typed: Typed) {
        let ops = self
            .targets()
            .iter()
            .filter_map(|block| metadata::convert_type(self.document(), block, typed))
            .collect();
        self.commit_any(ops);
    }

    /// `S`, or `s` with `on` false: makes the selected text blocks, else the focused block,
    /// sections, or plain text, as one step, and says how many tasks and projects it skipped,
    /// which cannot be sections (FR-BLK-04, FR-OPS-07, D-52).
    pub(super) fn set_section(&mut self, on: bool) {
        if self.access() == Access::ReadOnly {
            self.notify(Notice::ReadOnly);
            return;
        }
        let (text, skipped): (Vec<_>, Vec<_>) = self
            .targets()
            .into_iter()
            .partition(|block| block.kind() == BlockKind::Text);
        let ops = text
            .iter()
            .filter_map(|block| metadata::set_section(self.document(), block, on))
            .collect();
        self.commit_any(ops);
        if !skipped.is_empty() {
            self.notify(Notice::NotSections {
                skipped: skipped.len(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::{
        app::{Access, Message, Notice, SelectionChange},
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
                vec!["Project", "Task", "Note", "Make Section", "Remove Section"],
                1
            )
        );
        pick(&mut state, MenuInput::Next);
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
        assert_eq!(listed(&state), (vec!["Make Section", "Remove Section"], 0));
        pick(&mut state, MenuInput::Choose);
        assert_eq!(texts(&state), ["- a @x", "b @section"]);
        assert!(state.rows()[1].section);
        state.update(Message::OpenTypePicker);
        assert_eq!(listed(&state).1, 3, "a section highlights Make Section");
        pick(&mut state, MenuInput::Next);
        pick(&mut state, MenuInput::Choose);
        assert_eq!(texts(&state), ["- a @x", "b"]);
    }

    // FR-BLK-08, D-45
    #[test]
    fn a_linked_block_keeps_its_url_under_any_type() {
        let mut state = State::new(format::parse("Docs @url(https://a.b)"), None);
        state.update(Message::OpenTypePicker);
        pick(&mut state, MenuInput::Type('t'));
        pick(&mut state, MenuInput::Type('a'));
        pick(&mut state, MenuInput::Choose);
        assert_eq!(texts(&state), ["- Docs @url(https://a.b)"]);
        assert!(state.rows()[0].url);
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

    // FR-BLK-08, FR-OPS-07
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
        pick(&mut state, MenuInput::Next);
        pick(&mut state, MenuInput::Choose);
        assert_eq!(texts(&state), ["- a", "- b https://b.c", "- P", "- d"]);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a", "b https://b.c", "P:", "- d"]);
    }

    /// The lines of `text` after `J` `extend` times from its first block, then `S`, or `s` with
    /// `on` false, with the notice it left.
    fn sectioned(text: &str, extend: usize, on: bool) -> (State, Option<Notice>) {
        let mut state = State::new(format::parse(text), None);
        for _ in 0..extend {
            state.update(Message::Select(SelectionChange::ExtendDown));
        }
        state.update(Message::SetSection(on));
        let notice = state.notice().cloned();
        (state, notice)
    }

    // FR-BLK-04, D-52
    #[test]
    fn s_keys_make_the_focused_text_block_a_section_and_plain_text_again() {
        let (mut state, notice) = sectioned("a @x\n- t", 0, true);
        assert_eq!(texts(&state), ["a @x @section", "- t"]);
        assert_eq!(notice, None);
        state.update(Message::SetSection(true));
        assert_eq!(texts(&state), ["a @x @section", "- t"], "already one");
        state.update(Message::SetSection(false));
        assert_eq!(texts(&state), ["a @x", "- t"]);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["a @x @section", "- t"]);
    }

    // FR-BLK-04, D-52
    #[test]
    fn a_task_or_project_is_never_made_a_section_and_says_so() {
        for (text, on) in [("- t", true), ("P:", true), ("- t @section", false)] {
            let (state, notice) = sectioned(text, 0, on);
            assert_eq!(texts(&state), [text]);
            assert_eq!(notice, Some(Notice::NotSections { skipped: 1 }), "{text}");
            assert_eq!(state.document().revision(), 0);
        }
    }

    // FR-BLK-04, FR-OPS-07, D-52
    #[test]
    fn a_selection_makes_its_text_blocks_sections_as_one_step_and_counts_the_rest() {
        let (mut state, notice) = sectioned("a\n- t\nb @section\nP:\nc\nd", 4, true);
        assert_eq!(
            texts(&state),
            ["a @section", "- t", "b @section", "P:", "c @section", "d"]
        );
        assert_eq!(notice, Some(Notice::NotSections { skipped: 2 }));
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["a", "- t", "b @section", "P:", "c", "d"]);
        let (state, notice) = sectioned("a @section\nb @section\nc", 2, false);
        assert_eq!(texts(&state), ["a", "b", "c"]);
        assert_eq!(notice, None);
    }

    // FR-BLK-08, D-52
    #[test]
    fn the_pickers_section_entries_act_as_the_s_keys() {
        let mut state = State::new(format::parse("a\n- t"), None);
        state.update(Message::Select(SelectionChange::ExtendDown));
        state.update(Message::OpenTypePicker);
        for c in "make".chars() {
            pick(&mut state, MenuInput::Type(c));
        }
        pick(&mut state, MenuInput::Choose);
        assert_eq!(texts(&state), ["a @section", "- t"]);
        assert_eq!(state.notice(), Some(&Notice::NotSections { skipped: 1 }));
    }

    // file-io.md §1
    #[test]
    fn a_read_only_file_makes_no_section() {
        let mut state = State::new(format::parse("a"), Some(PathBuf::from("/a.taskpaper")))
            .with_access(Access::ReadOnly);
        state.update(Message::SetSection(true));
        assert_eq!(texts(&state), ["a"]);
        assert_eq!(state.notice(), Some(&Notice::ReadOnly));
    }
}
