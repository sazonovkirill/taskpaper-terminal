//! The color picker that `mc` opens: its entries, the filter typed into it, the highlighted
//! entry, and what each key does to them
//! ([editing.md §5](../../../docs/specs/editing.md#5-block-commands), FR-TAG-05).

use super::{MenuInput, State};
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

/// The open picker as a frontend shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorPickerView<'a> {
    /// What was typed to filter the entries.
    pub filter: &'a str,
    /// The entries whose name the filter matches, in order, each a color or none with its name.
    pub entries: Vec<(Option<TagColor>, &'static str)>,
    /// The position of the highlighted entry in `entries`.
    pub selected: usize,
}

/// The open picker: the filter and the highlighted entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ColorPicker {
    filter: String,
    selected: usize,
}

/// How an input closed the picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Closed {
    /// The block takes this color, or none.
    Chosen(Option<TagColor>),
    /// The block keeps its color.
    Cancelled,
}

impl ColorPicker {
    /// A picker with every entry listed and `current`, the block's color, highlighted.
    fn new(current: Option<TagColor>) -> Self {
        Self {
            filter: String::new(),
            selected: ENTRIES
                .iter()
                .position(|&(color, _)| color == current)
                .unwrap_or_default(),
        }
    }

    /// The entries whose name holds the filter, ignoring case.
    fn entries(&self) -> Vec<(Option<TagColor>, &'static str)> {
        let filter = self.filter.to_lowercase();
        ENTRIES
            .into_iter()
            .filter(|(_, name)| name.to_lowercase().contains(&filter))
            .collect()
    }

    /// Applies `input`, and returns how it closed the picker, or `None` while it stays open.
    fn input(&mut self, input: MenuInput) -> Option<Closed> {
        match input {
            MenuInput::Type(c) => {
                self.filter.push(c);
                self.selected = 0;
            }
            MenuInput::Erase => {
                if self.filter.pop().is_some() {
                    self.selected = 0;
                }
            }
            MenuInput::Previous => self.selected = self.selected.saturating_sub(1),
            MenuInput::Next => {
                self.selected = (self.selected + 1).min(self.entries().len().saturating_sub(1));
            }
            MenuInput::Choose => {
                return self
                    .entries()
                    .get(self.selected)
                    .map(|&(color, _)| Closed::Chosen(color));
            }
            MenuInput::Dismiss => return Some(Closed::Cancelled),
        }
        None
    }
}

impl State {
    /// The color picker, while it is open.
    #[must_use]
    pub fn color_picker(&self) -> Option<ColorPickerView<'_>> {
        let picker = self.color_picker.as_ref()?;
        Some(ColorPickerView {
            filter: &picker.filter,
            entries: picker.entries(),
            selected: picker.selected,
        })
    }

    /// `mc`: opens the color picker on the focused block, its color highlighted (FR-TAG-05).
    pub(super) fn open_color_picker(&mut self) {
        if let Some((block, _)) = self.focused() {
            self.color_picker = Some(ColorPicker::new(block.color()));
        }
    }

    /// Applies `input` to the open color picker, and once a color or none is chosen, gives it to
    /// the focused block.
    pub(super) fn color_picker_input(&mut self, input: MenuInput) {
        let Some(closed) = self
            .color_picker
            .as_mut()
            .and_then(|picker| picker.input(input))
        else {
            return;
        };
        self.color_picker = None;
        if let Closed::Chosen(color) = closed {
            self.set_color(color);
        }
    }

    /// Gives the focused block `color`, or none (FR-TAG-05).
    fn set_color(&mut self, color: Option<TagColor>) {
        if let Some((block, _)) = self.focused()
            && let Some(op) = metadata::set_color(self.document(), &block, color)
        {
            self.commit(vec![op]);
        }
    }

    /// `c`: gives the focused block the color after its own in the cycle (D-26).
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
        app::{Access, Message, Notice},
        document::{Document, format},
    };

    /// The first line's text.
    fn text(state: &State) -> String {
        let document = state.document();
        let (id, _) = document.descendants(document.root()).next().unwrap();
        document.get(id).unwrap().text().to_owned()
    }

    fn pick(state: &mut State, input: MenuInput) {
        state.update(Message::ColorPicker(input));
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
}
