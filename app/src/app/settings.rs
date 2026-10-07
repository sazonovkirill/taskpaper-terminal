//! The settings window: the frontend's settings in their groups, which group and setting have
//! the keys, the filter typed, and the picker or prompt that changes a setting (FR-SET-02,
//! [ui.md §8](../../../docs/specs/ui.md#8-settings-window-fr-set-02-fr-set-03)). The settings
//! are the frontend's: the core knows their names, kinds and values, never what they do, and asks
//! the frontend to apply and keep each change.

use std::ops::Range;

use super::{
    Effect, MenuInput, Notice, State,
    finder::{Candidate, Finder, FinderView},
    picker::Picked,
    text_field::{TextEdit, TextField},
};

/// A setting as the frontend lists it in the settings window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setting {
    /// The group it is listed in, such as `Appearance`; groups show in the order of their first
    /// setting, so a group without settings does not show.
    pub group: &'static str,
    /// Its name, plain text.
    pub name: &'static str,
    /// What it does, in one line.
    pub description: &'static str,
    /// What values it takes.
    pub kind: SettingKind,
    /// The value it has.
    pub value: SettingValue,
    /// The value it has unless the user sets another.
    pub default: SettingValue,
}

/// What values a setting takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingKind {
    /// On or off.
    Switch,
    /// One of these choices, by name.
    Choice(Vec<String>),
    /// A whole number, at least `least`, counted in `unit`.
    Number {
        /// The least it may be.
        least: u32,
        /// What it counts, such as `ms`; empty for a plain count.
        unit: &'static str,
    },
    /// Text, such as a folder, where empty text stands for the default, shown as `unset`.
    Text {
        /// What empty text is shown as.
        unset: &'static str,
    },
}

/// A setting's value, of its [`SettingKind`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingValue {
    /// A switch, on when `true`.
    Switch(bool),
    /// The choice at this position.
    Choice(usize),
    /// A number.
    Number(u32),
    /// A text, empty for the default.
    Text(String),
}

/// What happens to the settings window: the frontend opens it, and its keys (ui.md §8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsInput {
    /// Open the window over these settings of the frontend's.
    Open(Vec<Setting>),
    /// Highlight the next group or setting.
    Next,
    /// Highlight the previous group or setting.
    Previous,
    /// Give the keys to the settings when the groups have them, and the other way round.
    SwitchColumn,
    /// Toggle the highlighted setting, or open its picker or prompt; on a group, go to its
    /// settings.
    Change,
    /// Give the highlighted choice setting its next choice, or the previous one when `false`,
    /// or toggle a switch.
    Cycle(bool),
    /// Give the highlighted setting its default value.
    Reset,
    /// Start typing a filter for the settings' names.
    Filter,
    /// Close the window.
    Close,
    /// Change the text of the prompt or the filter being typed.
    Edit(TextEdit),
    /// Take the prompt's value, or end typing the filter.
    Accept,
    /// Close the prompt, or clear the filter being typed.
    Cancel,
    /// A key of the open value picker.
    Pick(MenuInput),
}

/// Which column of the settings window has the keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsColumn {
    /// The groups, on the left.
    Groups,
    /// The settings of the highlighted group, on the right.
    Settings,
}

/// A text being typed, as a frontend shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Typed<'a> {
    /// The text typed.
    pub text: &'a str,
    /// The caret's byte offset into `text`.
    pub caret: usize,
    /// The selected bytes of `text`, if any.
    pub selection: Option<Range<usize>>,
}

impl<'a> From<&'a TextField> for Typed<'a> {
    fn from(field: &'a TextField) -> Self {
        Self {
            text: field.text(),
            caret: field.caret(),
            selection: field.selection(),
        }
    }
}

/// A setting as the settings window shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingRow<'a> {
    /// Its position in the settings the frontend listed.
    pub at: usize,
    /// Its name.
    pub name: &'a str,
    /// What it does.
    pub description: &'a str,
    /// Its value in words: `on` or `off`, a choice's name, a number with its unit, or a text.
    pub value: String,
    /// Whether the value differs from the default.
    pub changed: bool,
}

/// The open settings window as a frontend shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsView<'a> {
    /// The groups with a setting the filter matches, in order.
    pub groups: Vec<&'a str>,
    /// The position of the highlighted group in `groups`.
    pub group: usize,
    /// The settings of the highlighted group that the filter matches, in order.
    pub rows: Vec<SettingRow<'a>>,
    /// The position of the highlighted setting in `rows`.
    pub selected: usize,
    /// The most settings any group has, filter or not, so the window can keep one size.
    pub most_rows: usize,
    /// Which column has the keys.
    pub column: SettingsColumn,
    /// The filter, while there is one, and whether it is being typed.
    pub filter: Option<(Typed<'a>, bool)>,
    /// The value picker of the highlighted setting, named, while it is open.
    pub picker: Option<(&'a str, FinderView<'a, usize>)>,
    /// The prompt for the highlighted setting's value, named, while it is open.
    pub prompt: Option<(&'a str, Typed<'a>)>,
    /// Why the last change failed: a value the prompt cannot take, or a write that failed.
    pub error: Option<&'a str>,
}

/// The open settings window.
#[derive(Debug, Clone)]
pub(super) struct Window {
    settings: Vec<Setting>,
    column: SettingsColumn,
    /// The highlighted group's position among those shown.
    group: usize,
    /// The highlighted setting's position among those shown.
    selected: usize,
    filter: Option<TextField>,
    /// Whether the keys type the filter.
    filtering: bool,
    editor: Option<Editor>,
    error: Option<String>,
}

/// What an input leaves of the window.
#[derive(Debug)]
enum Outcome {
    /// It stays open, having asked for this change, if any.
    Open(Option<Effect>),
    /// It closed.
    Closed,
}

/// How the highlighted setting is being changed.
#[derive(Debug, Clone)]
enum Editor {
    /// Picking one of its choices, each by its position.
    Picking(Finder<usize>),
    /// Typing its value.
    Typing(TextField),
}

impl Window {
    /// The window over `settings`, the first group's settings highlighted.
    fn new(settings: Vec<Setting>) -> Self {
        Self {
            settings,
            column: SettingsColumn::Settings,
            group: 0,
            selected: 0,
            filter: None,
            filtering: false,
            editor: None,
            error: None,
        }
    }

    /// Whether the filter matches the setting `setting`'s name, ignoring case.
    fn matches(&self, setting: &Setting) -> bool {
        self.filter.as_ref().is_none_or(|filter| {
            setting
                .name
                .to_lowercase()
                .contains(&filter.text().to_lowercase())
        })
    }

    /// The groups with a setting the filter matches, in order.
    fn groups(&self) -> Vec<&'static str> {
        groups(self.settings.iter().filter(|setting| self.matches(setting)))
    }

    /// Every group, in order.
    fn groups_all(&self) -> Vec<&'static str> {
        groups(self.settings.iter())
    }

    /// The positions of the highlighted group's settings that the filter matches.
    fn rows(&self) -> Vec<usize> {
        let group = self.groups().get(self.group).copied();
        self.settings
            .iter()
            .enumerate()
            .filter(|(_, setting)| Some(setting.group) == group && self.matches(setting))
            .map(|(at, _)| at)
            .collect()
    }

    /// The position of the highlighted setting among those listed.
    fn highlighted(&self) -> Option<usize> {
        self.rows().get(self.selected).copied()
    }

    /// The window as a frontend shows it.
    fn view(&self) -> SettingsView<'_> {
        let rows = self
            .rows()
            .into_iter()
            .filter_map(|at| self.settings.get(at).map(|setting| (at, setting)))
            .map(|(at, setting)| SettingRow {
                at,
                name: setting.name,
                description: setting.description,
                value: shown_value(setting),
                changed: setting.value != setting.default,
            })
            .collect();
        let name = self
            .highlighted()
            .and_then(|at| self.settings.get(at))
            .map_or("", |setting| setting.name);
        SettingsView {
            groups: self.groups(),
            group: self.group,
            rows,
            selected: self.selected,
            most_rows: self
                .groups_all()
                .iter()
                .map(|group| self.settings.iter().filter(|s| s.group == *group).count())
                .max()
                .unwrap_or_default(),
            column: self.column,
            filter: self
                .filter
                .as_ref()
                .map(|filter| (filter.into(), self.filtering)),
            picker: match &self.editor {
                Some(Editor::Picking(finder)) => Some((name, finder.view())),
                _ => None,
            },
            prompt: match &self.editor {
                Some(Editor::Typing(field)) => Some((name, field.into())),
                _ => None,
            },
            error: self.error.as_deref(),
        }
    }

    /// Applies `input` and returns what it leaves of the window.
    fn input(&mut self, input: SettingsInput) -> Outcome {
        match (&mut self.editor, input) {
            (Some(Editor::Picking(finder)), SettingsInput::Pick(input)) => {
                let picked = finder.input(input);
                return Outcome::Open(picked.and_then(|picked| self.pick(picked)));
            }
            (Some(Editor::Typing(field)), SettingsInput::Edit(edit)) => *field = field.edited(edit),
            (Some(Editor::Typing(field)), SettingsInput::Accept) => {
                let typed = field.text().trim().to_owned();
                return Outcome::Open(self.accept(typed));
            }
            (Some(Editor::Typing(_)), SettingsInput::Cancel) => {
                self.editor = None;
                self.error = None;
            }
            (Some(_), _) => {}
            (None, input) if self.filtering => self.filter_input(&input),
            (None, input) => return self.browse(&input),
        }
        Outcome::Open(None)
    }

    /// Applies `input` while the filter is being typed.
    fn filter_input(&mut self, input: &SettingsInput) {
        match input {
            SettingsInput::Edit(edit) => {
                self.filter = self.filter.as_ref().map(|filter| filter.edited(*edit));
                self.group = 0;
                self.selected = 0;
            }
            SettingsInput::Accept => {
                self.filtering = false;
                if self.filter.as_ref().is_some_and(|f| f.text().is_empty()) {
                    self.filter = None;
                }
            }
            SettingsInput::Cancel => {
                self.filtering = false;
                self.filter = None;
                self.group = 0;
                self.selected = 0;
            }
            SettingsInput::Next | SettingsInput::Previous => {
                self.browse(input);
            }
            _ => {}
        }
    }

    /// Applies `input` while neither a picker, a prompt nor the filter takes the keys.
    fn browse(&mut self, input: &SettingsInput) -> Outcome {
        let settings = self.column == SettingsColumn::Settings;
        let effect = match input {
            SettingsInput::Next | SettingsInput::Previous => {
                let next = *input == SettingsInput::Next;
                if settings {
                    self.selected = step(self.selected, next, self.rows().len());
                } else {
                    self.group = step(self.group, next, self.groups().len());
                    self.selected = 0;
                }
                None
            }
            SettingsInput::SwitchColumn => {
                self.column = if settings {
                    SettingsColumn::Groups
                } else {
                    SettingsColumn::Settings
                };
                None
            }
            SettingsInput::Change if !settings => {
                self.column = SettingsColumn::Settings;
                None
            }
            SettingsInput::Change => self.change(),
            SettingsInput::Cycle(forward) if settings => self.cycle(*forward),
            SettingsInput::Reset if settings => self.reset(),
            SettingsInput::Filter => {
                self.filtering = true;
                self.column = SettingsColumn::Settings;
                self.filter
                    .get_or_insert_with(|| TextField::new(String::new()));
                None
            }
            SettingsInput::Close => return Outcome::Closed,
            _ => None,
        };
        Outcome::Open(effect)
    }

    /// `r` on the highlighted setting: gives it its default value.
    fn reset(&mut self) -> Option<Effect> {
        let at = self.highlighted()?;
        let default = self.settings.get(at)?.default.clone();
        self.set(at, default)
    }

    /// `Enter` on the highlighted setting: toggles a switch, or opens the picker of a choice or
    /// the prompt of a number or text, holding its value.
    fn change(&mut self) -> Option<Effect> {
        let at = self.highlighted()?;
        let setting = self.settings.get(at)?;
        match (&setting.kind, &setting.value) {
            (SettingKind::Switch, &SettingValue::Switch(on)) => {
                return self.set(at, SettingValue::Switch(!on));
            }
            (SettingKind::Choice(names), &SettingValue::Choice(chosen)) => {
                let candidates = names
                    .iter()
                    .enumerate()
                    .map(|(value, name)| Candidate {
                        value,
                        text: name.clone(),
                        detail: String::new(),
                        tier: 0,
                    })
                    .collect();
                let mut finder = Finder::new(candidates);
                finder.highlight(&chosen);
                self.editor = Some(Editor::Picking(finder));
            }
            (_, SettingValue::Number(number)) => {
                self.editor = Some(Editor::Typing(TextField::new(number.to_string())));
            }
            (_, SettingValue::Text(text)) => {
                self.editor = Some(Editor::Typing(TextField::new(text.clone())));
            }
            _ => {}
        }
        self.error = None;
        None
    }

    /// `h`/`l` on the highlighted setting: the next choice, or the previous one unless
    /// `forward`, after the last coming the first; a switch toggles.
    fn cycle(&mut self, forward: bool) -> Option<Effect> {
        let at = self.highlighted()?;
        let setting = self.settings.get(at)?;
        let value = match (&setting.kind, &setting.value) {
            (SettingKind::Switch, &SettingValue::Switch(on)) => SettingValue::Switch(!on),
            (SettingKind::Choice(names), &SettingValue::Choice(chosen)) => {
                let count = names.len().max(1);
                let next = if forward {
                    chosen + 1
                } else {
                    chosen + count - 1
                };
                SettingValue::Choice(next % count)
            }
            _ => return None,
        };
        self.set(at, value)
    }

    /// The value picker closed with `picked`: a choice is given to the setting.
    fn pick(&mut self, picked: Picked<usize>) -> Option<Effect> {
        self.editor = None;
        let at = self.highlighted()?;
        match picked {
            Picked::Chosen(choice) => self.set(at, SettingValue::Choice(choice)),
            Picked::Cancelled => None,
        }
    }

    /// `Enter` in the prompt holding `typed`: gives the setting that value, or says why it
    /// cannot take it and keeps the prompt open.
    fn accept(&mut self, typed: String) -> Option<Effect> {
        let at = self.highlighted()?;
        let setting = self.settings.get(at)?;
        let value = match &setting.kind {
            SettingKind::Number { least, .. } => match typed.parse::<u32>() {
                Ok(number) if number >= *least => SettingValue::Number(number),
                _ => {
                    self.error = Some(format!(
                        "{} must be a whole number, at least {least}",
                        setting.name
                    ));
                    return None;
                }
            },
            _ => SettingValue::Text(typed),
        };
        self.editor = None;
        self.set(at, value)
    }

    /// Gives the setting at `at` `value` and asks the frontend to apply and keep it, unless it
    /// has that value already.
    fn set(&mut self, at: usize, value: SettingValue) -> Option<Effect> {
        let setting = self.settings.get_mut(at)?;
        self.error = None;
        if setting.value == value {
            return None;
        }
        setting.value = value.clone();
        Some(Effect::ChangeSetting { at, value })
    }
}

/// The groups of `settings`, in the order of their first setting.
fn groups<'a>(settings: impl Iterator<Item = &'a Setting>) -> Vec<&'static str> {
    let mut groups: Vec<&'static str> = Vec::new();
    for setting in settings {
        if !groups.contains(&setting.group) {
            groups.push(setting.group);
        }
    }
    groups
}

/// `at` moved one step forward when `next`, else back, staying within `count` positions.
fn step(at: usize, next: bool, count: usize) -> usize {
    if next {
        (at + 1).min(count.saturating_sub(1))
    } else {
        at.saturating_sub(1)
    }
}

/// `setting`'s value in words.
fn shown_value(setting: &Setting) -> String {
    match (&setting.kind, &setting.value) {
        (_, SettingValue::Switch(true)) => "on".to_owned(),
        (_, SettingValue::Switch(false)) => "off".to_owned(),
        (SettingKind::Choice(names), SettingValue::Choice(chosen)) => {
            names.get(*chosen).cloned().unwrap_or_default()
        }
        (SettingKind::Number { unit, .. }, SettingValue::Number(number)) if !unit.is_empty() => {
            format!("{number} {unit}")
        }
        (_, SettingValue::Number(number)) => number.to_string(),
        (SettingKind::Text { unset }, SettingValue::Text(text)) if text.is_empty() => {
            (*unset).to_owned()
        }
        (_, SettingValue::Text(text)) => text.clone(),
        (_, SettingValue::Choice(chosen)) => chosen.to_string(),
    }
}

impl State {
    /// The settings window, while it is open (FR-SET-02).
    #[must_use]
    pub fn settings(&self) -> Option<SettingsView<'_>> {
        self.settings.as_ref().map(Window::view)
    }

    /// The value the open value picker highlights, with the position of its setting, which the
    /// frontend previews (ui.md §3, §8).
    #[must_use]
    pub fn settings_preview(&self) -> Option<(usize, SettingValue)> {
        let window = self.settings.as_ref()?;
        let Some(Editor::Picking(finder)) = &window.editor else {
            return None;
        };
        let choice = *finder.highlighted()?;
        Some((window.highlighted()?, SettingValue::Choice(choice)))
    }

    /// Applies `input` to the settings window, opening it over the frontend's settings, and
    /// returns the change it asks the frontend to apply and keep, if any.
    pub(super) fn settings_input(&mut self, input: SettingsInput) -> Option<Effect> {
        if let SettingsInput::Open(settings) = input {
            self.settings = Some(Window::new(settings));
            return None;
        }
        match self.settings.as_mut()?.input(input) {
            Outcome::Open(effect) => effect,
            Outcome::Closed => {
                self.settings = None;
                None
            }
        }
    }

    /// Shows the problems of the frontend's settings file, `problems`, in the status line and,
    /// while it is open, in the settings window, as a change made there could not be kept
    /// (FR-SET-04).
    pub(super) fn config_problems(&mut self, problems: Vec<String>) {
        if let Some(window) = &mut self.settings {
            window.error = Some(problems.join("; "));
        }
        self.notify(Notice::ConfigProblems(problems));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::{Message, Motion},
        document::format,
    };

    /// Three settings in two groups, as a frontend might list them: a choice and a switch, then
    /// a number and a text.
    fn settings() -> Vec<Setting> {
        let setting = |group, name, kind, value: SettingValue| Setting {
            group,
            name,
            description: "What it does",
            kind,
            default: value.clone(),
            value,
        };
        vec![
            setting(
                "Looks",
                "Theme",
                SettingKind::Choice(vec!["Dark".into(), "Light".into(), "Nord".into()]),
                SettingValue::Choice(0),
            ),
            setting(
                "Looks",
                "Wrap",
                SettingKind::Switch,
                SettingValue::Switch(true),
            ),
            setting(
                "Files",
                "Delay",
                SettingKind::Number {
                    least: 100,
                    unit: "ms",
                },
                SettingValue::Number(1000),
            ),
            setting(
                "Files",
                "Folder",
                SettingKind::Text { unset: "default" },
                SettingValue::Text(String::new()),
            ),
        ]
    }

    fn opened() -> State {
        let mut state = State::new(format::parse("- a\n"), None);
        assert_eq!(key(&mut state, SettingsInput::Open(settings())), []);
        state
    }

    fn key(state: &mut State, input: SettingsInput) -> Vec<Effect> {
        state.update(Message::Settings(input))
    }

    fn typed(state: &mut State, text: &str) {
        for c in text.chars() {
            key(state, SettingsInput::Edit(TextEdit::Insert(c)));
        }
    }

    fn change(at: usize, value: SettingValue) -> Vec<Effect> {
        vec![Effect::ChangeSetting { at, value }]
    }

    /// The groups, the highlighted group's settings with their values and whether they changed,
    /// and the highlighted setting.
    fn shown(state: &State) -> (Vec<&str>, Vec<(String, bool)>, usize) {
        let view = state.settings().unwrap();
        let rows = view
            .rows
            .iter()
            .map(|row| (format!("{}: {}", row.name, row.value), row.changed))
            .collect();
        (view.groups, rows, view.selected)
    }

    // FR-SET-02, ui.md §8
    #[test]
    fn lists_the_first_groups_settings_with_their_values() {
        assert_eq!(State::new(format::parse(""), None).settings(), None);
        let state = opened();
        assert_eq!(
            shown(&state),
            (
                vec!["Looks", "Files"],
                vec![("Theme: Dark".into(), false), ("Wrap: on".into(), false)],
                0
            )
        );
        let view = state.settings().unwrap();
        assert_eq!(view.column, SettingsColumn::Settings);
        assert_eq!(view.most_rows, 2);
        assert_eq!(view.rows[1].at, 1);
        assert_eq!(view.rows[0].description, "What it does");
        assert_eq!(
            (view.filter, view.picker, view.prompt, view.error),
            (None, None, None, None)
        );
    }

    // ui.md §8
    #[test]
    fn keys_move_within_a_column_and_tab_switches_columns() {
        let mut state = opened();
        assert_eq!(key(&mut state, SettingsInput::Next), []);
        key(&mut state, SettingsInput::Next);
        assert_eq!(shown(&state).2, 1, "the last setting stays highlighted");
        key(&mut state, SettingsInput::SwitchColumn);
        assert_eq!(state.settings().unwrap().column, SettingsColumn::Groups);
        key(&mut state, SettingsInput::Next);
        key(&mut state, SettingsInput::Next);
        let (_, rows, selected) = shown(&state);
        assert_eq!(
            (rows[0].0.as_str(), selected),
            ("Delay: 1000 ms", 0),
            "the next group's settings, the first highlighted"
        );
        assert_eq!(state.settings().unwrap().group, 1);
        key(&mut state, SettingsInput::Cycle(true));
        key(&mut state, SettingsInput::Reset);
        assert_eq!(shown(&state).2, 0, "h, l and r act on settings only");
        key(&mut state, SettingsInput::Previous);
        key(&mut state, SettingsInput::Previous);
        assert_eq!(state.settings().unwrap().group, 0);
        assert_eq!(key(&mut state, SettingsInput::Change), []);
        assert_eq!(
            state.settings().unwrap().column,
            SettingsColumn::Settings,
            "Enter on a group goes to its settings"
        );
        key(&mut state, SettingsInput::Next);
        key(&mut state, SettingsInput::Previous);
        assert_eq!(shown(&state).2, 0);
        key(&mut state, SettingsInput::SwitchColumn);
        key(&mut state, SettingsInput::SwitchColumn);
        assert_eq!(state.settings().unwrap().column, SettingsColumn::Settings);
    }

    // ui.md §8
    #[test]
    fn h_l_and_r_leave_the_settings_alone_while_the_groups_have_the_keys() {
        let mut state = opened();
        key(&mut state, SettingsInput::Cycle(true));
        key(&mut state, SettingsInput::SwitchColumn);
        assert_eq!(key(&mut state, SettingsInput::Cycle(true)), []);
        assert_eq!(key(&mut state, SettingsInput::Reset), []);
        assert_eq!(shown(&state).1[0], ("Theme: Light".into(), true));
    }

    // ui.md §8
    #[test]
    fn the_arrows_move_among_the_matches_while_the_filter_is_typed() {
        let mut state = opened();
        key(&mut state, SettingsInput::Filter);
        typed(&mut state, "l");
        key(&mut state, SettingsInput::Next);
        assert_eq!(
            shown(&state),
            (
                vec!["Files"],
                vec![
                    ("Delay: 1000 ms".into(), false),
                    ("Folder: default".into(), false)
                ],
                1
            )
        );
        assert!(
            state
                .settings()
                .unwrap()
                .filter
                .is_some_and(|(_, typing)| typing)
        );
    }

    // ui.md §8
    #[test]
    fn the_window_is_as_tall_as_its_largest_group() {
        let setting = |group| Setting {
            group,
            name: "N",
            description: "",
            kind: SettingKind::Switch,
            default: SettingValue::Switch(true),
            value: SettingValue::Switch(true),
        };
        let mut state = State::new(format::parse("- a\n"), None);
        key(
            &mut state,
            SettingsInput::Open(vec![setting("A"), setting("B"), setting("C"), setting("C")]),
        );
        assert_eq!(state.settings().unwrap().most_rows, 2);
    }

    // FR-SET-02, ui.md §8
    #[test]
    fn enter_and_cycling_toggle_a_switch_and_mark_it_changed() {
        let mut state = opened();
        key(&mut state, SettingsInput::Next);
        assert_eq!(
            key(&mut state, SettingsInput::Change),
            change(1, SettingValue::Switch(false))
        );
        assert_eq!(shown(&state).1[1], ("Wrap: off".into(), true));
        assert_eq!(
            key(&mut state, SettingsInput::Cycle(false)),
            change(1, SettingValue::Switch(true))
        );
        assert_eq!(shown(&state).1[1], ("Wrap: on".into(), false));
    }

    // ui.md §8
    #[test]
    fn cycling_gives_a_choice_the_next_or_previous_one_round_the_ends() {
        let mut state = opened();
        assert_eq!(
            key(&mut state, SettingsInput::Cycle(false)),
            change(0, SettingValue::Choice(2))
        );
        assert_eq!(shown(&state).1[0], ("Theme: Nord".into(), true));
        assert_eq!(
            key(&mut state, SettingsInput::Cycle(true)),
            change(0, SettingValue::Choice(0))
        );
        assert_eq!(
            key(&mut state, SettingsInput::Cycle(true)),
            change(0, SettingValue::Choice(1))
        );
    }

    // ui.md §8
    #[test]
    fn cycling_leaves_a_number_or_text_alone() {
        let mut state = opened();
        key(&mut state, SettingsInput::SwitchColumn);
        key(&mut state, SettingsInput::Next);
        key(&mut state, SettingsInput::SwitchColumn);
        for at in [2, 3] {
            assert_eq!(state.settings().unwrap().rows[at - 2].at, at);
            assert_eq!(key(&mut state, SettingsInput::Cycle(true)), []);
            key(&mut state, SettingsInput::Next);
        }
        assert_eq!(
            shown(&state).1,
            [
                ("Delay: 1000 ms".into(), false),
                ("Folder: default".into(), false)
            ]
        );
    }

    // FR-SET-02, ui.md §8
    #[test]
    fn resetting_gives_the_default_back_and_does_nothing_at_the_default() {
        let mut state = opened();
        key(&mut state, SettingsInput::Cycle(true));
        assert_eq!(
            key(&mut state, SettingsInput::Reset),
            change(0, SettingValue::Choice(0))
        );
        assert_eq!(shown(&state).1[0], ("Theme: Dark".into(), false));
        assert_eq!(key(&mut state, SettingsInput::Reset), []);
    }

    // FR-UI-06, ui.md §3, §8
    #[test]
    fn a_choice_opens_a_picker_that_previews_and_chooses_its_value() {
        let mut state = opened();
        assert_eq!(state.settings_preview(), None);
        assert_eq!(key(&mut state, SettingsInput::Change), []);
        let view = state.settings().unwrap();
        let (name, picker) = view.picker.clone().unwrap();
        assert_eq!(
            (name, picker.selected, picker.entries.len()),
            ("Theme", 0, 3)
        );
        assert_eq!(state.settings_preview(), Some((0, SettingValue::Choice(0))));
        assert_eq!(key(&mut state, SettingsInput::Pick(MenuInput::Next)), []);
        assert_eq!(state.settings_preview(), Some((0, SettingValue::Choice(1))));
        key(&mut state, SettingsInput::Next);
        key(&mut state, SettingsInput::Reset);
        assert_eq!(
            state.settings_preview(),
            Some((0, SettingValue::Choice(1))),
            "other keys wait for the picker"
        );
        assert_eq!(
            key(&mut state, SettingsInput::Pick(MenuInput::Choose)),
            change(0, SettingValue::Choice(1))
        );
        assert_eq!(state.settings().unwrap().picker, None);
        assert_eq!(state.settings_preview(), None);
        key(&mut state, SettingsInput::Change);
        for c in "zzz".chars() {
            key(&mut state, SettingsInput::Pick(MenuInput::Type(c)));
        }
        assert_eq!(state.settings_preview(), None, "nothing highlighted");
        assert_eq!(key(&mut state, SettingsInput::Pick(MenuInput::Choose)), []);
        assert_eq!(key(&mut state, SettingsInput::Pick(MenuInput::Dismiss)), []);
        assert_eq!(shown(&state).1[0], ("Theme: Light".into(), true));
        key(&mut state, SettingsInput::Change);
        assert_eq!(
            key(&mut state, SettingsInput::Pick(MenuInput::Choose)),
            [],
            "the value it has"
        );
    }

    // ui.md §8
    #[test]
    fn a_number_opens_a_prompt_that_takes_a_whole_number_no_less_than_its_least() {
        let mut state = opened();
        key(&mut state, SettingsInput::SwitchColumn);
        key(&mut state, SettingsInput::Next);
        key(&mut state, SettingsInput::SwitchColumn);
        assert_eq!(key(&mut state, SettingsInput::Change), []);
        let prompt = state.settings().unwrap().prompt.unwrap();
        assert_eq!(
            (prompt.0, prompt.1.text, prompt.1.caret),
            ("Delay", "1000", 4)
        );
        key(
            &mut state,
            SettingsInput::Edit(TextEdit::Move {
                motion: Motion::Start,
                select: true,
            }),
        );
        typed(&mut state, "99");
        assert_eq!(key(&mut state, SettingsInput::Accept), []);
        assert_eq!(
            state.settings().unwrap().error,
            Some("Delay must be a whole number, at least 100")
        );
        assert!(
            state.settings().unwrap().prompt.is_some(),
            "the prompt stays open"
        );
        typed(&mut state, "x");
        assert_eq!(key(&mut state, SettingsInput::Accept), []);
        key(&mut state, SettingsInput::Edit(TextEdit::DeleteBackward));
        typed(&mut state, "0 ");
        assert_eq!(
            key(&mut state, SettingsInput::Accept),
            change(2, SettingValue::Number(990))
        );
        let view = state.settings().unwrap();
        assert_eq!((view.prompt, view.error), (None, None));
        assert_eq!(shown(&state).1[0], ("Delay: 990 ms".into(), true));
        key(&mut state, SettingsInput::Change);
        assert_eq!(
            key(&mut state, SettingsInput::Accept),
            [],
            "the value it has"
        );
    }

    // ui.md §8
    #[test]
    fn a_text_opens_a_prompt_whose_empty_text_is_the_default() {
        let mut state = opened();
        key(&mut state, SettingsInput::SwitchColumn);
        key(&mut state, SettingsInput::Next);
        key(&mut state, SettingsInput::SwitchColumn);
        key(&mut state, SettingsInput::Next);
        assert_eq!(shown(&state).1[1], ("Folder: default".into(), false));
        key(&mut state, SettingsInput::Change);
        typed(&mut state, "~/copies");
        key(&mut state, SettingsInput::Next);
        key(&mut state, SettingsInput::Previous);
        assert_eq!(
            shown(&state).2,
            1,
            "the prompt keeps the setting highlighted"
        );
        assert_eq!(
            key(&mut state, SettingsInput::Accept),
            change(3, SettingValue::Text("~/copies".into()))
        );
        assert_eq!(shown(&state).1[1], ("Folder: ~/copies".into(), true));
        key(&mut state, SettingsInput::Change);
        typed(&mut state, "x");
        assert_eq!(key(&mut state, SettingsInput::Cancel), []);
        let view = state.settings().unwrap();
        assert_eq!(view.prompt, None);
        assert_eq!(
            shown(&state).1[1],
            ("Folder: ~/copies".into(), true),
            "unchanged"
        );
    }

    // ui.md §8
    #[test]
    fn slash_filters_the_settings_by_name_and_esc_clears_the_filter() {
        let mut state = opened();
        key(&mut state, SettingsInput::SwitchColumn);
        assert_eq!(key(&mut state, SettingsInput::Filter), []);
        let view = state.settings().unwrap();
        assert_eq!(view.column, SettingsColumn::Settings);
        assert_eq!(
            view.filter.map(|(filter, typing)| (filter.text, typing)),
            Some(("", true))
        );
        typed(&mut state, "DEL");
        assert_eq!(
            shown(&state),
            (vec!["Files"], vec![("Delay: 1000 ms".into(), false)], 0)
        );
        key(&mut state, SettingsInput::Edit(TextEdit::DeleteBackward));
        key(&mut state, SettingsInput::Edit(TextEdit::DeleteBackward));
        key(&mut state, SettingsInput::Edit(TextEdit::DeleteBackward));
        typed(&mut state, "e");
        assert_eq!(shown(&state).0, ["Looks", "Files"], "Theme and Delay");
        key(&mut state, SettingsInput::Next);
        assert_eq!(shown(&state).2, 0, "one match in Looks");
        typed(&mut state, "zz");
        assert_eq!(shown(&state), (vec![], vec![], 0));
        assert_eq!(
            key(&mut state, SettingsInput::Change),
            [],
            "nothing to change"
        );
        key(&mut state, SettingsInput::Cancel);
        let view = state.settings().unwrap();
        assert_eq!(view.filter, None);
        assert_eq!(shown(&state).1.len(), 2);
        key(&mut state, SettingsInput::Filter);
        typed(&mut state, "f");
        key(&mut state, SettingsInput::Reset);
        assert_eq!(key(&mut state, SettingsInput::Accept), []);
        let view = state.settings().unwrap();
        assert_eq!(
            view.filter.map(|(filter, typing)| (filter.text, typing)),
            Some(("f", false))
        );
        assert_eq!(shown(&state).1[0].0, "Folder: default");
        assert_eq!(
            key(&mut state, SettingsInput::Reset),
            [],
            "the keys act on the settings again"
        );
        key(&mut state, SettingsInput::Filter);
        key(&mut state, SettingsInput::Edit(TextEdit::DeleteBackward));
        key(&mut state, SettingsInput::Accept);
        assert_eq!(
            state.settings().unwrap().filter,
            None,
            "an empty filter is none"
        );
    }

    // FR-SET-04, ui.md §8
    #[test]
    fn a_change_that_cannot_be_kept_shows_in_the_window_and_the_status_line() {
        let mut state = opened();
        let problems = vec!["`theme` not saved: denied".to_owned()];
        state.update(Message::ConfigProblems(problems.clone()));
        assert_eq!(
            state.settings().unwrap().error,
            Some("`theme` not saved: denied")
        );
        assert_eq!(state.notice(), Some(&Notice::ConfigProblems(problems)));
        key(&mut state, SettingsInput::Cycle(true));
        assert_eq!(
            state.settings().unwrap().error,
            None,
            "the next change clears it"
        );
        let mut closed = State::new(format::parse("- a\n"), None);
        closed.update(Message::ConfigProblems(vec!["x".into()]));
        assert_eq!(closed.settings(), None);
    }

    // ui.md §8
    #[test]
    fn esc_closes_the_window_and_other_keys_then_do_nothing() {
        let mut state = opened();
        key(&mut state, SettingsInput::Accept);
        key(&mut state, SettingsInput::Edit(TextEdit::Insert('x')));
        assert!(
            state.settings().is_some(),
            "only the filter and prompt take text"
        );
        assert_eq!(key(&mut state, SettingsInput::Close), []);
        assert_eq!(state.settings(), None);
        assert_eq!(key(&mut state, SettingsInput::Change), []);
    }

    // ui.md §8
    #[test]
    fn picker_and_prompt_keys_do_nothing_while_neither_is_open() {
        let mut state = opened();
        assert_eq!(key(&mut state, SettingsInput::Pick(MenuInput::Choose)), []);
        assert_eq!(key(&mut state, SettingsInput::Accept), []);
        let view = state.settings().unwrap();
        assert_eq!((view.picker, view.prompt), (None, None));
    }

    #[test]
    fn a_choice_without_choices_opens_nothing() {
        let mut state = State::new(format::parse("- a\n"), None);
        let odd = Setting {
            group: "G",
            name: "N",
            description: "",
            kind: SettingKind::Switch,
            default: SettingValue::Choice(0),
            value: SettingValue::Choice(0),
        };
        key(&mut state, SettingsInput::Open(vec![odd]));
        assert_eq!(key(&mut state, SettingsInput::Change), []);
        assert_eq!(state.settings().unwrap().picker, None);
    }

    #[test]
    fn values_read_as_the_window_shows_them() {
        let setting = |kind, value| Setting {
            group: "G",
            name: "N",
            description: "",
            kind,
            default: SettingValue::Switch(false),
            value,
        };
        let number = |unit| SettingKind::Number { least: 0, unit };
        for (kind, value, words) in [
            (number("ms"), SettingValue::Number(5), "5 ms"),
            (number(""), SettingValue::Number(5), "5"),
            (
                SettingKind::Text { unset: "default" },
                SettingValue::Text("a".into()),
                "a",
            ),
            (
                SettingKind::Choice(vec!["a".into()]),
                SettingValue::Choice(3),
                "",
            ),
            (SettingKind::Switch, SettingValue::Choice(3), "3"),
        ] {
            assert_eq!(shown_value(&setting(kind, value)), words);
        }
    }
}
