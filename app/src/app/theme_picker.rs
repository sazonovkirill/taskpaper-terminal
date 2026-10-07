//! The theme picker: a fuzzy finder over the themes the frontend names, which gives the chosen
//! one back to it. Themes are the frontend's: the core knows their names only, never their
//! colors (FR-UI-06, [ui.md §3](../../../docs/specs/ui.md#3-themes-fr-ui-0608)).

use super::{
    Effect, MenuInput, State,
    finder::{Candidate, Finder, FinderView},
    picker::Picked,
};

impl State {
    /// The theme picker, while it is open, each theme by its position in the names the frontend
    /// gave; the frontend previews the highlighted one (FR-UI-06).
    #[must_use]
    pub fn theme_picker(&self) -> Option<FinderView<'_, usize>> {
        self.themes.as_ref().map(Finder::view)
    }

    /// Opens the theme picker over `names`, in their order, the one at `current` highlighted.
    pub(super) fn open_theme_picker(&mut self, names: Vec<String>, current: usize) {
        let candidates = names
            .into_iter()
            .enumerate()
            .map(|(value, text)| Candidate {
                value,
                text,
                detail: String::new(),
                tier: 0,
            })
            .collect();
        let mut finder = Finder::new(candidates);
        finder.highlight(&current);
        self.themes = Some(finder);
    }

    /// Applies `input` to the theme picker, and once a theme is chosen, closes it and asks the
    /// frontend to use and keep that theme; dismissed, it closes without one.
    pub(super) fn theme_picker_input(&mut self, input: MenuInput) -> Option<Effect> {
        let closed = self.themes.as_mut()?.input(input)?;
        self.themes = None;
        match closed {
            Picked::Chosen(theme) => Some(Effect::SetTheme(theme)),
            Picked::Cancelled => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::{Menu, Message},
        document::format,
    };

    fn opened(current: usize) -> State {
        let mut state = State::new(format::parse("- a\n"), None);
        let names = ["Dark", "Light", "Nord"].map(str::to_owned).to_vec();
        assert_eq!(
            state.update(Message::OpenThemePicker { names, current }),
            []
        );
        state
    }

    fn key(state: &mut State, input: MenuInput) -> Vec<Effect> {
        state.update(Message::Menu(Menu::ThemePicker, input))
    }

    /// The names listed and the highlighted position.
    fn listed(state: &State) -> (Vec<String>, usize) {
        let view = state.theme_picker().unwrap();
        (
            view.entries
                .iter()
                .map(|entry| entry.text.clone())
                .collect(),
            view.selected,
        )
    }

    // FR-UI-06
    #[test]
    fn lists_the_themes_in_order_with_the_one_in_use_highlighted() {
        assert_eq!(State::new(format::parse(""), None).theme_picker(), None);
        let state = opened(2);
        assert_eq!(
            listed(&state),
            (vec!["Dark".into(), "Light".into(), "Nord".into()], 2)
        );
        assert_eq!(
            listed(&opened(9)).1,
            0,
            "an unknown theme highlights the first"
        );
    }

    // FR-UI-06
    #[test]
    fn typing_filters_and_choosing_asks_to_use_the_highlighted_theme() {
        let mut state = opened(0);
        for c in "nor".chars() {
            assert_eq!(key(&mut state, MenuInput::Type(c)), []);
        }
        assert_eq!(listed(&state), (vec!["Nord".into()], 0));
        assert_eq!(key(&mut state, MenuInput::Choose), [Effect::SetTheme(2)]);
        assert_eq!(state.theme_picker(), None);
        let mut state = opened(0);
        assert_eq!(key(&mut state, MenuInput::Next), []);
        assert_eq!(key(&mut state, MenuInput::Choose), [Effect::SetTheme(1)]);
    }

    // FR-UI-06
    #[test]
    fn dismissing_closes_it_without_a_theme() {
        let mut state = opened(1);
        assert_eq!(key(&mut state, MenuInput::Dismiss), []);
        assert_eq!(state.theme_picker(), None);
        assert_eq!(key(&mut state, MenuInput::Choose), [], "nothing open");
    }
}
