//! The fuzzy finder's overlay ([ui.md §1](../../../../docs/specs/ui.md#1-layout)): a centered box
//! at most 80×20 with the query on its first line and the matching entries below it.

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Clear, Widget},
};

use super::super::{
    layout,
    theme::{Icon, IconSet, Theme},
};
use crate::app::FinderView;

/// The largest the overlay gets.
const SIZE: (u16, u16) = (80, 20);

/// What stands before the query.
const PROMPT: &str = "› ";

/// A finder titled with what it finds, such as Go to Symbol, framed in the accent color like the
/// other menus of commands: the query with the caret after it,
/// then each matching entry's icon, its text and, dimmed after it, its detail, the highlighted
/// one reversed and kept in view, or a dimmed line saying nothing matches.
pub struct FinderPopup<'a, T> {
    title: &'a str,
    view: &'a FinderView<'a, T>,
    icons: IconSet,
    icon: &'a dyn Fn(&T) -> (Icon, Style),
    theme: &'a Theme,
}

impl<T> std::fmt::Debug for FinderPopup<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FinderPopup")
            .field("title", &self.title)
            .field("icons", &self.icons)
            .finish_non_exhaustive()
    }
}

impl<'a, T> FinderPopup<'a, T> {
    /// The overlay of `view` titled `title`, in `theme`, each entry after the icon `icon` gives
    /// its value, drawn from `icons` in the style it gives.
    #[must_use]
    pub fn new(
        title: &'a str,
        view: &'a FinderView<'a, T>,
        icons: IconSet,
        icon: &'a dyn Fn(&T) -> (Icon, Style),
        theme: &'a Theme,
    ) -> Self {
        Self {
            title,
            view,
            icons,
            icon,
            theme,
        }
    }

    /// Where the overlay goes in `area`: centered, as wide as it can be up to 80 columns and as
    /// tall as its entries, with the query and border, up to 20 rows.
    #[must_use]
    pub fn area(&self, area: Rect) -> Rect {
        let entries = u16::try_from(self.view.entries.len().max(1)).unwrap_or(u16::MAX);
        let height = entries.saturating_add(3).min(SIZE.1);
        area.centered(Constraint::Length(SIZE.0), Constraint::Length(height))
    }

    /// Where the caret goes when the overlay is drawn over `area`, as [`FinderPopup::area`]
    /// places it: after the query.
    #[must_use]
    pub fn caret(&self, area: Rect) -> (u16, u16) {
        let area = self.area(area);
        let typed = Line::from(format!("{PROMPT}{}", self.view.query)).width();
        let column = u16::try_from(typed).unwrap_or(u16::MAX);
        let right = area.right().saturating_sub(2);
        (
            area.x.saturating_add(1).saturating_add(column).min(right),
            area.y.saturating_add(1),
        )
    }
}

impl<T> Widget for FinderPopup<'_, T> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = self.area(area);
        Clear.render(area, buf);
        let accent = Style::new().fg(self.theme.accent);
        let frame = Block::bordered()
            .border_style(accent)
            .title_style(accent)
            .title(self.title);
        let inner = frame.inner(area);
        frame.render(area, buf);
        let mut rows = inner.rows();
        if let Some(row) = rows.next() {
            Line::from(format!("{PROMPT}{}", self.view.query)).render(row, buf);
        }
        let rows: Vec<Rect> = rows.collect();
        if self.view.entries.is_empty() {
            if let Some(&row) = rows.first() {
                Line::from(" No matches").dim().render(row, buf);
            }
            return;
        }
        let selected = self.view.selected;
        let offset = layout::scroll_offset(
            0,
            selected..selected + 1,
            rows.len(),
            self.view.entries.len(),
        );
        let entries = self.view.entries.iter().enumerate().skip(offset);
        for ((index, entry), row) in entries.zip(rows) {
            let (icon, style) = (self.icon)(&entry.value);
            let mut line = Line::from(vec![
                Span::raw(" "),
                Span::styled(icon.glyph(self.icons), style),
                Span::raw(format!(" {}", entry.text)),
            ]);
            if !entry.detail.is_empty() {
                line.push_span(Span::styled(
                    format!("  {}", entry.detail),
                    Style::new().dim(),
                ));
            }
            if index == self.view.selected {
                buf.set_style(row, Style::new().reversed());
                line = line.reversed();
            }
            line.render(row, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::THEME;
    use ratatui::style::Color;

    use super::*;
    use crate::app::Candidate;

    /// Every entry's icon in the tests: the generic one, in the style the value numbers.
    const ICON: fn(&u8) -> (Icon, Style) =
        |value| (Icon::Command, Style::new().fg(Color::Indexed(*value)));

    fn candidate(text: &str) -> Candidate<u8> {
        Candidate {
            value: 0,
            text: text.to_owned(),
            detail: String::new(),
            tier: 0,
        }
    }

    #[test]
    fn fits_its_entries_in_at_most_80_by_20_centered() {
        let one = candidate("One");
        let view = FinderView {
            query: "o",
            entries: vec![&one],
            selected: 0,
        };
        let finder = FinderPopup::new("Find", &view, IconSet::Unicode, &ICON, &THEME);
        assert_eq!(
            finder.area(Rect::new(0, 0, 100, 30)),
            Rect::new(10, 13, 80, 4)
        );
        assert_eq!(finder.area(Rect::new(0, 0, 40, 3)), Rect::new(0, 0, 40, 3));
        let many: Vec<Candidate<u8>> = (0..40).map(|n| candidate(&n.to_string())).collect();
        let view = FinderView {
            query: "",
            entries: many.iter().collect(),
            selected: 0,
        };
        let finder = FinderPopup::new("Find", &view, IconSet::Unicode, &ICON, &THEME);
        assert_eq!(finder.area(Rect::new(0, 0, 100, 30)).height, 20);
    }

    #[test]
    fn the_caret_follows_the_query_and_stays_inside_the_box() {
        let view = FinderView::<u8> {
            query: "abc",
            entries: Vec::new(),
            selected: 0,
        };
        let finder = FinderPopup::new("Find", &view, IconSet::Unicode, &ICON, &THEME);
        assert_eq!(finder.caret(Rect::new(0, 0, 100, 30)), (16, 14));
        let long = "x".repeat(200);
        let view = FinderView::<u8> {
            query: &long,
            entries: Vec::new(),
            selected: 0,
        };
        let finder = FinderPopup::new("Find", &view, IconSet::Unicode, &ICON, &THEME);
        assert_eq!(finder.caret(Rect::new(0, 0, 100, 30)), (88, 14));
    }

    #[test]
    fn draws_the_query_and_the_entries_after_their_icons_the_highlighted_one_reversed() {
        let (one, two) = (candidate("One"), candidate("Two"));
        let two = Candidate {
            value: 2,
            detail: "Path".to_owned(),
            ..two
        };
        let view = FinderView {
            query: "",
            entries: vec![&one, &two],
            selected: 1,
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 16, 5));
        FinderPopup::new("Find", &view, IconSet::Unicode, &ICON, &THEME).render(buf.area, &mut buf);
        let mut expected = Buffer::with_lines([
            "┌Find──────────┐",
            "│›             │",
            "│ › One        │",
            "│ › Two  Path  │",
            "└──────────────┘",
        ]);
        expected.set_style(expected.area, Style::new().fg(THEME.accent));
        expected.set_style(Rect::new(1, 1, 14, 3), Style::new().fg(Color::Reset));
        expected.set_style(Rect::new(2, 2, 1, 1), Style::new().fg(Color::Indexed(0)));
        expected.set_style(Rect::new(1, 3, 14, 1), Style::new().reversed());
        expected.set_style(Rect::new(2, 3, 1, 1), Style::new().fg(Color::Indexed(2)));
        expected.set_style(Rect::new(7, 3, 6, 1), Style::new().dim());
        assert_eq!(buf, expected);
    }

    #[test]
    fn debug_shows_the_title_and_icons() {
        let view = FinderView::<u8> {
            query: "",
            entries: Vec::new(),
            selected: 0,
        };
        let finder = FinderPopup::new("Find", &view, IconSet::Ascii, &ICON, &THEME);
        assert_eq!(
            format!("{finder:?}"),
            "FinderPopup { title: \"Find\", icons: Ascii, .. }"
        );
    }
}
