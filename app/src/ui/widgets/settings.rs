//! The settings window ([ui.md §8](../../../../docs/specs/ui.md#8-settings-window-fr-set-02-fr-set-03)):
//! a centered box at most 100×30 with the groups on the left and the highlighted group's
//! settings on the right, a rule that shows an error when a change failed, a line for the
//! prompt, the filter or the keys, and the settings file's path at the bottom. Its own labels are plain text, so it stays usable when
//! icons show as boxes (ui.md §7).

use std::path::Path;

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Widget},
};

use super::{Field, error_style, rule_style};
use crate::{
    app::{SettingsColumn, SettingsView, Typed},
    ui::{
        layout,
        theme::{Icon, IconSet, Theme},
    },
};

/// The largest the window gets.
const SIZE: (u16, u16) = (100, 30);

/// The narrowest the groups column gets, its rule included.
const GROUPS_WIDTH: u16 = 16;

/// What the status line says while nothing else takes it.
const KEYS: &str =
    " Tab: groups/settings · Enter: change · h/l: cycle · r: reset · /: filter · Esc: close";

/// The settings window.
#[derive(Debug)]
pub struct SettingsWindow<'a> {
    view: &'a SettingsView<'a>,
    file: Option<&'a Path>,
    icons: IconSet,
    icon: fn(usize) -> Icon,
    theme: &'a Theme,
}

impl<'a> SettingsWindow<'a> {
    /// The window showing `view` in `theme`, with the settings file `file` in its footer, each
    /// setting after the icon `icon` gives its position, drawn from `icons`.
    #[must_use]
    pub fn new(
        view: &'a SettingsView<'a>,
        file: Option<&'a Path>,
        icons: IconSet,
        icon: fn(usize) -> Icon,
        theme: &'a Theme,
    ) -> Self {
        Self {
            view,
            file,
            icons,
            icon,
            theme,
        }
    }

    /// Where the window goes in `area`: centered, as wide as it can be up to 100 columns and as
    /// tall as its largest group, two lines a setting, with its border, the status line and the
    /// footer, up to 30 rows.
    #[must_use]
    pub fn area(&self, area: Rect) -> Rect {
        let lines = self.view.groups.len().max(self.view.most_rows * 2).max(1);
        let height = u16::try_from(lines)
            .unwrap_or(u16::MAX)
            .saturating_add(5)
            .min(SIZE.1);
        area.centered(Constraint::Length(SIZE.0), Constraint::Length(height))
    }

    /// The window's inner rows over `area`: the groups and settings, the rule or error, the
    /// status line and the footer.
    fn parts(&self, area: Rect) -> [Rect; 4] {
        Block::bordered()
            .inner(self.area(area))
            .layout(&Layout::vertical([
                Constraint::Fill(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ]))
    }

    /// Where the caret goes when the window is drawn over `area`: in the prompt or the filter
    /// being typed, if any.
    #[must_use]
    pub fn caret(&self, area: Rect) -> Option<(u16, u16)> {
        let [_, _, status, _] = self.parts(area);
        let (_, column) = self.status(status.width);
        let x = u16::try_from(column?)
            .unwrap_or(u16::MAX)
            .min(status.width.saturating_sub(1));
        Some((status.x.saturating_add(x), status.y))
    }

    /// The status line, `width` columns wide, and the caret's column on it while text is typed:
    /// the prompt, else the filter, else the keys.
    fn status(&self, width: u16) -> (Line<'a>, Option<usize>) {
        let typed = |label: String, typed: &Typed<'a>| {
            let field = Field {
                text: typed.text,
                caret: typed.caret,
                selection: typed.selection.clone(),
            };
            let start = Span::raw(label.as_str()).width();
            let (spans, column) = field.spans(Style::new(), start, usize::from(width));
            let mut line = Line::from(label);
            line.spans.extend(spans);
            (line, column)
        };
        if let Some((name, prompt)) = &self.view.prompt {
            let (line, column) = typed(format!(" {name}: "), prompt);
            return (line, Some(column));
        }
        match &self.view.filter {
            Some((filter, typing)) => {
                let (line, column) = typed(" /".to_owned(), filter);
                (line, typing.then_some(column))
            }
            None => (Line::styled(KEYS, Style::new().dim()), None),
        }
    }

    /// The groups in `area`, the highlighted one reversed while the groups have the keys, else
    /// bold in the primary color, with a rule on their right.
    fn render_groups(&self, area: Rect, buf: &mut Buffer) {
        let rule = Block::new()
            .borders(Borders::RIGHT)
            .border_style(rule_style(self.theme));
        let inner = rule.inner(area);
        rule.render(area, buf);
        for (index, (group, row)) in self.view.groups.iter().zip(inner.rows()).enumerate() {
            let mut line = Line::from(format!(" {group}"));
            if index == self.view.group {
                if self.view.column == SettingsColumn::Groups {
                    buf.set_style(row, Style::new().reversed());
                    line = line.reversed();
                } else {
                    line = line.style(Style::new().fg(self.theme.primary).bold());
                }
            }
            line.render(row, buf);
        }
    }

    /// The highlighted group's settings in `area`, two lines each and scrolled to keep the
    /// highlighted one in view: a `*` when its value is not the default, its icon, its name and,
    /// at the right, its value, bold when changed; then its description, dimmed. The highlighted
    /// setting is reversed while the settings have the keys.
    fn render_settings(&self, area: Rect, buf: &mut Buffer) {
        let rows = &self.view.rows;
        if rows.is_empty() {
            Line::from(" No settings match").dim().render(area, buf);
            return;
        }
        let height = usize::from(area.height);
        let selected = self.view.selected * 2;
        let offset = layout::scroll_offset(0, selected..selected + 2, height, rows.len() * 2);
        let lines = rows.iter().enumerate().flat_map(|(index, row)| {
            let mark = if row.changed { " * " } else { "   " };
            let value = Span::raw(format!("{}  ", row.value));
            let value = if row.changed { value.bold() } else { value };
            let name = Line::from(vec![
                Span::raw(mark),
                Span::raw((self.icon)(row.at).glyph(self.icons)),
                Span::raw(format!(" {}", row.name)),
            ]);
            let description = Line::from(format!("     {}", row.description)).dim();
            [(index, name, Some(value)), (index, description, None)]
        });
        for ((index, line, value), area) in lines.skip(offset).zip(area.rows()) {
            line.render(area, buf);
            if let Some(value) = value {
                Line::from(value).right_aligned().render(area, buf);
            }
            if index == self.view.selected && self.view.column == SettingsColumn::Settings {
                buf.set_style(area, Style::new().reversed());
            }
        }
    }

    /// The footer: where the settings file is, so people know where to edit it by hand.
    fn footer(&self) -> Line<'static> {
        match self.file {
            Some(file) => Line::from(format!(" Settings file: {}", file.display())),
            None => {
                Line::from(" No settings file: neither $XDG_CONFIG_HOME nor a home folder is set")
            }
        }
        .dim()
    }
}

impl Widget for SettingsWindow<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [body, rule, status, footer] = self.parts(area);
        let area = self.area(area);
        Clear.render(area, buf);
        let accent = Style::new().fg(self.theme.accent);
        Block::bordered()
            .border_style(accent)
            .title_style(accent)
            .title(" Settings ")
            .render(area, buf);
        let widest = self.view.groups.iter().map(|group| group.len()).max();
        let groups_width = u16::try_from(widest.unwrap_or_default())
            .unwrap_or(u16::MAX)
            .saturating_add(3)
            .max(GROUPS_WIDTH);
        let [groups, settings] = body.layout(&Layout::horizontal([
            Constraint::Length(groups_width),
            Constraint::Fill(1),
        ]));
        self.render_groups(groups, buf);
        self.render_settings(settings, buf);
        match self.view.error {
            Some(error) => Line::styled(format!(" {error}"), error_style(self.theme)),
            None => Line::styled("─".repeat(usize::from(rule.width)), rule_style(self.theme)),
        }
        .render(rule, buf);
        self.status(status.width).0.render(status, buf);
        self.footer().render(footer, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::THEME;

    use super::*;
    use crate::{
        app::{Message, Setting, SettingKind, SettingValue, SettingsInput, State},
        document::format,
    };

    // ui.md §8
    #[test]
    fn a_long_group_scrolls_to_keep_the_highlighted_setting_in_view_with_a_margin() {
        let names = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
        let settings = names
            .map(|name| Setting {
                group: "G",
                name,
                description: "",
                kind: SettingKind::Switch,
                value: SettingValue::Switch(true),
                default: SettingValue::Switch(true),
            })
            .to_vec();
        let mut state = State::new(format::parse("- a\n"), None);
        state.update(Message::Settings(SettingsInput::Open(settings)));
        for _ in 0..4 {
            state.update(Message::Settings(SettingsInput::Next));
        }
        let view = state.settings().expect("open");
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 12));
        SettingsWindow::new(&view, None, IconSet::Unicode, |_| Icon::Command, &THEME)
            .render(buf.area, &mut buf);
        let names: Vec<String> = (1..8)
            .map(|y| {
                (17..40)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .trim()
                    .to_owned()
            })
            .collect();
        assert_eq!(
            names,
            [
                "│",
                "› 3            on  │",
                "│",
                "› 4            on  │",
                "│",
                "› 5            on  │",
                "│"
            ],
            "4 highlighted, with two lines of margin on each side"
        );
    }
}
