//! The color picker that `mc` opens: its entries and what choosing one does to the focused block
//! ([editing.md §5](../../../docs/specs/editing.md#5-block-commands), FR-TAG-05).

use super::{
    MenuInput, State,
    picker::{Picked, Picker, PickerView},
};
use crate::document::{TagColor, metadata};

/// Every entry of the picker with its name, in the order it lists them: the six colors, then
/// none.
const ENTRIES: [(Option<TagColor>, &str); 7] = [
    (Some(TagColor::Red), "Red"),
    (Some(TagColor::Orange), "Orange"),
    (Some(TagColor::Yellow), "Yellow"),
    (Some(TagColor::Green), "Green"),
    (Some(TagColor::Blue), "Blue"),
    (Some(TagColor::Violet), "Violet"),
    (None, "None"),
];

/// The color whose number `typed` is (D-48).
fn numbered(typed: char) -> Option<TagColor> {
    let number = typed.to_digit(10)?;
    TagColor::ALL
        .into_iter()
        .find(|color| color.number() == number)
}

impl State {
    /// The color picker, while it is open.
    #[must_use]
    pub fn color_picker(&self) -> Option<PickerView<'_, Option<TagColor>>> {
        self.color_picker.as_ref().map(Picker::view)
    }

    /// `mc`: opens the color picker on the focused block, its color highlighted (FR-TAG-05).
    pub(super) fn open_color_picker(&mut self) {
        if let Some((block, _)) = self.focused() {
            self.color_picker = Some(Picker::new(&ENTRIES, block.color()));
        }
    }

    /// Applies `input` to the open color picker, and once a color or none is chosen, gives it to
    /// the selected blocks, else the focused block. Typing a color's number, 1 to 6 in the
    /// picker's order, chooses it at once (D-48).
    pub(super) fn color_picker_input(&mut self, input: MenuInput) {
        let Some(picker) = self.color_picker.as_mut() else {
            return;
        };
        let closed = match input {
            MenuInput::Type(typed) => numbered(typed).map(|color| Picked::Chosen(Some(color))),
            _ => None,
        }
        .or_else(|| picker.input(input));
        let Some(closed) = closed else {
            return;
        };
        self.color_picker = None;
        if let Picked::Chosen(color) = closed {
            self.set_color(color);
        }
    }

    /// Gives the selected blocks, else the focused block, `color`, or none (FR-TAG-05,
    /// FR-OPS-07).
    pub(super) fn set_color(&mut self, color: Option<TagColor>) {
        let ops = self
            .targets()
            .iter()
            .filter_map(|block| metadata::set_color(self.document(), block, color))
            .collect();
        self.commit_any(ops);
    }

    /// `c`: gives the selected blocks, else the focused block, the color after the focused
    /// block's own in the cycle (D-26).
    pub(super) fn cycle_color(&mut self) {
        if let Some((block, _)) = self.focused() {
            self.set_color(TagColor::cycle(block.color()));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::{
        app::{Access, Menu, Message, Notice, SelectionChange},
        document::{Document, format},
    };

    /// The first line's text.
    fn text(state: &State) -> String {
        let document = state.document();
        let (id, _) = document.descendants(document.root()).next().unwrap();
        document.get(id).unwrap().text().to_owned()
    }

    fn pick(state: &mut State, input: MenuInput) {
        state.update(Message::Menu(Menu::ColorPicker, input));
    }

    /// The names the open picker lists and the highlighted one.
    fn listed(state: &State) -> (Vec<&'static str>, usize) {
        let view = state.color_picker().unwrap();
        (
            view.entries.iter().map(|&(_, name)| name).collect(),
            view.selected,
        )
    }

    // D-26, editing.md §5
    #[test]
    fn c_cycles_through_the_colors_and_back_to_none() {
        let mut state = State::new(format::parse("- a @x"), None);
        let mut seen = Vec::new();
        for _ in 0..7 {
            state.update(Message::CycleColor);
            seen.push(text(&state));
        }
        assert_eq!(
            seen,
            [
                "- a @x @red",
                "- a @x @orange",
                "- a @x @yellow",
                "- a @x @green",
                "- a @x @blue",
                "- a @x @violet",
                "- a @x",
            ]
        );
        assert_eq!(state.rows()[0].color, None);
        state.update(Message::Undo);
        assert_eq!(state.rows()[0].color, Some(TagColor::Violet));
        let mut empty = State::new(Document::default(), None);
        empty.update(Message::CycleColor);
        assert_eq!(empty.document().revision(), 0);
    }

    // FR-TAG-05
    #[test]
    fn the_picker_opens_on_the_blocks_color_and_applies_the_chosen_one() {
        let mut state = State::new(format::parse("- a @blue"), None);
        state.update(Message::OpenColorPicker);
        assert_eq!(
            listed(&state),
            (
                vec!["Red", "Orange", "Yellow", "Green", "Blue", "Violet", "None"],
                4
            )
        );
        assert_eq!(state.color_picker().unwrap().filter, "");
        pick(&mut state, MenuInput::Previous);
        pick(&mut state, MenuInput::Choose);
        assert_eq!(state.color_picker(), None);
        assert_eq!(text(&state), "- a @green");
        state.update(Message::OpenColorPicker);
        for _ in 0..9 {
            pick(&mut state, MenuInput::Next);
        }
        assert_eq!(listed(&state).1, 6, "the highlight stops at the last");
        pick(&mut state, MenuInput::Choose);
        assert_eq!(text(&state), "- a");
        state.update(Message::OpenColorPicker);
        assert_eq!(listed(&state).1, 6, "none is highlighted without a color");
        for _ in 0..9 {
            pick(&mut state, MenuInput::Previous);
        }
        pick(&mut state, MenuInput::Choose);
        assert_eq!(text(&state), "- a @red");
    }

    // FR-TAG-05, FR-OPS-07, D-48
    #[test]
    fn set_color_gives_every_target_the_color_or_none_in_one_step() {
        let mut state = State::new(format::parse("- a @red\n- b\n- c @x @blue"), None);
        state.update(Message::Select(SelectionChange::ExtendDown));
        state.update(Message::SetColor(Some(TagColor::Green)));
        assert_eq!(texts(&state), ["- a @green", "- b @green", "- c @x @blue"]);
        state.update(Message::Select(SelectionChange::ExtendDown));
        state.update(Message::SetColor(None));
        assert_eq!(texts(&state), ["- a", "- b", "- c @x"]);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a @green", "- b @green", "- c @x @blue"]);
        let revision = state.document().revision();
        state.update(Message::Select(SelectionChange::Clear));
        state.update(Message::SetColor(Some(TagColor::Blue)));
        assert_eq!(state.document().revision(), revision, "c is blue already");
    }

    // FR-TAG-05, D-48
    #[test]
    fn a_digit_in_the_picker_chooses_its_color_at_once() {
        for (digit, color) in ('1'..='6').zip(TagColor::ALL) {
            let mut state = State::new(format::parse("- a @x"), None);
            state.update(Message::OpenColorPicker);
            pick(&mut state, MenuInput::Type(digit));
            assert_eq!(state.color_picker(), None, "{digit}");
            assert_eq!(text(&state), format!("- a @x @{}", color.tag()));
        }
        for other in ['0', '7', '9'] {
            let mut state = State::new(format::parse("- a @red"), None);
            state.update(Message::OpenColorPicker);
            pick(&mut state, MenuInput::Type(other));
            assert_eq!(listed(&state), (vec![], 0), "{other} filters");
            assert_eq!(text(&state), "- a @red");
        }
    }

    // FR-TAG-05, keybindings.md §4
    #[test]
    fn typing_filters_the_picker_and_erasing_widens_it() {
        let mut state = State::new(format::parse("- a"), None);
        state.update(Message::OpenColorPicker);
        pick(&mut state, MenuInput::Type('E'));
        assert_eq!(
            listed(&state),
            (
                vec!["Red", "Orange", "Yellow", "Green", "Blue", "Violet", "None"],
                0
            )
        );
        pick(&mut state, MenuInput::Type('n'));
        assert_eq!(listed(&state), (vec!["Green"], 0));
        assert_eq!(state.color_picker().unwrap().filter, "En");
        pick(&mut state, MenuInput::Erase);
        pick(&mut state, MenuInput::Erase);
        pick(&mut state, MenuInput::Next);
        pick(&mut state, MenuInput::Type('l'));
        assert_eq!(listed(&state), (vec!["Yellow", "Blue", "Violet"], 0));
        pick(&mut state, MenuInput::Next);
        pick(&mut state, MenuInput::Erase);
        assert_eq!(listed(&state).1, 0, "erasing highlights the first");
        pick(&mut state, MenuInput::Erase);
        pick(&mut state, MenuInput::Next);
        pick(&mut state, MenuInput::Erase);
        assert_eq!(listed(&state).1, 1, "nothing to erase keeps the highlight");
        pick(&mut state, MenuInput::Type('x'));
        assert_eq!(listed(&state), (vec![], 0));
        pick(&mut state, MenuInput::Next);
        pick(&mut state, MenuInput::Choose);
        assert!(state.color_picker().is_some(), "nothing to choose");
        assert_eq!(text(&state), "- a");
    }

    // FR-TAG-05
    #[test]
    fn dismissing_the_picker_keeps_the_color() {
        let mut state = State::new(format::parse("- a @red"), None);
        state.update(Message::OpenColorPicker);
        pick(&mut state, MenuInput::Next);
        pick(&mut state, MenuInput::Dismiss);
        assert_eq!(state.color_picker(), None);
        assert_eq!(text(&state), "- a @red");
        assert_eq!(state.document().revision(), 0);
        pick(&mut state, MenuInput::Choose);
        assert_eq!(state.document().revision(), 0, "no picker, no change");
    }

    #[test]
    fn the_picker_needs_a_focused_block_and_a_writable_file() {
        let mut empty = State::new(Document::default(), None);
        empty.update(Message::OpenColorPicker);
        assert_eq!(empty.color_picker(), None);
        let mut state = State::new(
            format::parse("- a"),
            Some(PathBuf::from("/notes/todo.taskpaper")),
        )
        .with_access(Access::ReadOnly);
        state.update(Message::OpenColorPicker);
        pick(&mut state, MenuInput::Type('r'));
        pick(&mut state, MenuInput::Choose);
        assert_eq!(text(&state), "- a");
        assert_eq!(state.notice(), Some(&Notice::ReadOnly));
    }

    /// Each line's text.
    fn texts(state: &State) -> Vec<String> {
        let document = state.document();
        document
            .descendants(document.root())
            .map(|(id, _)| document.get(id).unwrap().text().to_owned())
            .collect()
    }

    // FR-OPS-07, FR-TAG-05, D-26
    #[test]
    fn every_selected_block_takes_the_color_after_the_focused_ones_or_the_one_picked() {
        let mut state = State::new(format::parse("- a @red\n- b @blue\n- c"), None);
        state.update(Message::Select(SelectionChange::ExtendDown));
        state.update(Message::CycleColor);
        assert_eq!(
            texts(&state),
            ["- a @violet", "- b @violet", "- c"],
            "after the focused b's blue"
        );
        state.update(Message::OpenColorPicker);
        assert_eq!(listed(&state).1, 5, "the focused block's violet");
        pick(&mut state, MenuInput::Previous);
        pick(&mut state, MenuInput::Choose);
        assert_eq!(texts(&state), ["- a @blue", "- b @blue", "- c"]);
        state.update(Message::Undo);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a @red", "- b @blue", "- c"]);
    }
}
