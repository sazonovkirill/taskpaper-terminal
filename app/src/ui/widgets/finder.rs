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
use super::{drawable, navigator::heading_style};
use crate::app::{Candidate, FinderView};

/// The largest the overlay gets.
const SIZE: (u16, u16) = (80, 20);

/// What stands before the query.
const PROMPT: &str = "› ";

/// A finder titled with what it finds, such as Go to Symbol, framed in the accent color like the
/// other menus of commands: the query with the caret after it,
/// then each matching entry's icon and its text, both in the style its value gives if any, and, dimmed
/// after it, its detail, the highlighted one reversed and kept in view, or a dimmed line saying
/// nothing matches; and a note in its bottom border, if it has one. With headings, each run of
/// entries of one tier comes under the heading of its tier, as the Navigator draws its headings
/// (D-153).
pub struct FinderPopup<'a, T> {
    title: &'a str,
    note: Option<&'a str>,
    headings: &'a [&'a str],
    view: &'a FinderView<'a, T>,
    icons: IconSet,
    icon: &'a dyn Fn(&T) -> (Icon, Style),
    text_style: Option<&'a dyn Fn(&T) -> Style>,
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
    /// The fewest rows that show the border, the query and one entry.
    pub const LEAST_HEIGHT: u16 = 4;

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
            note: None,
            headings: &[],
            view,
            icons,
            icon,
            text_style: None,
            theme,
        }
    }

    /// The finder with each entry's icon and text in the style `text_style` gives its value.
    #[must_use]
    pub fn text_style(mut self, text_style: &'a dyn Fn(&T) -> Style) -> Self {
        self.text_style = Some(text_style);
        self
    }

    /// The finder with `note`, if any, dimmed in its bottom border.
    #[must_use]
    pub fn note(self, note: Option<&'a str>) -> Self {
        Self { note, ..self }
    }

    /// The finder with `headings`, the heading of each tier by its number, over its entries.
    #[must_use]
    pub fn headings(self, headings: &'a [&'a str]) -> Self {
        Self { headings, ..self }
    }

    /// The lines under the query: each entry, by its position, after the heading of its tier
    /// where a run of that tier starts.
    fn lines(&self) -> Vec<Listed<'a, T>> {
        let mut lines = Vec::with_capacity(self.view.entries.len());
        let mut tier = None;
        for (index, entry) in self.view.entries.iter().enumerate() {
            if tier != Some(entry.tier)
                && let Some(heading) = self.headings.get(usize::from(entry.tier))
            {
                lines.push(Listed::Heading(heading));
            }
            tier = Some(entry.tier);
            lines.push(Listed::Entry(index, entry));
        }
        lines
    }

    /// Where the overlay goes in `area`: centered, as wide as it can be up to 80 columns and as
    /// tall as its entries and headings, with the query and border, up to 20 rows.
    #[must_use]
    pub fn area(&self, area: Rect) -> Rect {
        let entries = u16::try_from(self.lines().len().max(1)).unwrap_or(u16::MAX);
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

/// A line under a finder's query.
enum Listed<'a, T> {
    /// The heading of the entries below it.
    Heading(&'a str),
    /// An entry, by its position among the entries.
    Entry(usize, &'a Candidate<T>),
}

impl<T> Widget for FinderPopup<'_, T> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = self.area(area);
        Clear.render(area, buf);
        let accent = Style::new().fg(self.theme.accent);
        let mut frame = Block::bordered()
            .border_style(accent)
            .title_style(accent)
            .title(self.title);
        if let Some(note) = self.note {
            frame = frame.title_bottom(Line::from(note).dim());
        }
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
        let lines = self.lines();
        let selected = lines
            .iter()
            .position(
                |line| matches!(line, Listed::Entry(index, _) if *index == self.view.selected),
            )
            .unwrap_or_default();
        let offset = layout::scroll_offset(0, selected..selected + 1, rows.len(), lines.len());
        for (line, row) in lines.into_iter().skip(offset).zip(rows) {
            let (index, entry) = match line {
                Listed::Heading(heading) => {
                    Line::styled(format!(" {heading}"), heading_style()).render(row, buf);
                    continue;
                }
                Listed::Entry(index, entry) => (index, entry),
            };
            let (icon, style) = (self.icon)(&entry.value);
            let text_style = self
                .text_style
                .map_or_else(Style::new, |text_style| text_style(&entry.value));
            let mut line = Line::from(vec![
                Span::raw(" "),
                Span::styled(icon.glyph(self.icons), style.patch(text_style)),
                Span::raw(" "),
                Span::styled(drawable(&entry.text), text_style),
            ]);
            if !entry.detail.is_empty() {
                line.push_span(Span::styled(
                    format!("  {}", drawable(&entry.detail)),
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
    use ratatui::style::{Color, Modifier};

    use super::*;
    use crate::app::{Candidate, Entries};

    /// Every entry's icon in the tests: the generic one, in the style the value numbers.
    const ICON: fn(&u8) -> (Icon, Style) =
        |value| (Icon::Command, Style::new().fg(Color::Indexed(*value)));

    fn candidate(text: &str) -> Candidate<u8> {
        Candidate {
            value: 0,
            text: text.into(),
            detail: "".into(),
            tier: 0,
        }
    }

    #[test]
    fn fits_its_entries_in_at_most_80_by_20_centered() {
        let one = [candidate("One")];
        let view = FinderView {
            query: "o",
            entries: Entries::new(&one, &[0]),
            selected: 0,
        };
        let finder = FinderPopup::new("Find", &view, IconSet::Unicode, &ICON, &THEME);
        assert_eq!(
            finder.area(Rect::new(0, 0, 100, 30)),
            Rect::new(10, 13, 80, 4)
        );
        assert_eq!(finder.area(Rect::new(0, 0, 40, 3)), Rect::new(0, 0, 40, 3));
        let many: Vec<Candidate<u8>> = (0..40).map(|n| candidate(&n.to_string())).collect();
        let ranked: Vec<usize> = (0..many.len()).collect();
        let view = FinderView {
            query: "",
            entries: Entries::new(&many, &ranked),
            selected: 0,
        };
        let finder = FinderPopup::new("Find", &view, IconSet::Unicode, &ICON, &THEME);
        assert_eq!(finder.area(Rect::new(0, 0, 100, 30)).height, 20);
    }

    #[test]
    fn the_caret_follows_the_query_and_stays_inside_the_box() {
        let view = FinderView::<u8> {
            query: "abc",
            entries: Entries::new(&[], &[]),
            selected: 0,
        };
        let finder = FinderPopup::new("Find", &view, IconSet::Unicode, &ICON, &THEME);
        assert_eq!(finder.caret(Rect::new(0, 0, 100, 30)), (16, 14));
        let long = "x".repeat(200);
        let view = FinderView::<u8> {
            query: &long,
            entries: Entries::new(&[], &[]),
            selected: 0,
        };
        let finder = FinderPopup::new("Find", &view, IconSet::Unicode, &ICON, &THEME);
        assert_eq!(finder.caret(Rect::new(0, 0, 100, 30)), (88, 14));
    }

    #[test]
    fn a_note_shows_dimmed_in_the_bottom_border() {
        let view = FinderView::<u8> {
            query: "",
            entries: Entries::new(&[], &[]),
            selected: 0,
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 14, 4));
        FinderPopup::new("Find", &view, IconSet::Unicode, &ICON, &THEME)
            .note(Some("Why"))
            .render(buf.area, &mut buf);
        assert_eq!(
            (1..4).map(|x| buf[(x, 3)].symbol()).collect::<String>(),
            "Why"
        );
        assert!(buf[(1, 3)].modifier.contains(Modifier::DIM));
        assert_eq!(buf[(4, 3)].symbol(), "─");
    }

    #[test]
    fn draws_the_query_and_the_entries_after_their_icons_the_highlighted_one_reversed() {
        let (one, two) = (candidate("One"), candidate("Two"));
        let two = Candidate {
            value: 2,
            detail: "Path".into(),
            ..two
        };
        let candidates = [one, two];
        let view = FinderView {
            query: "",
            entries: Entries::new(&candidates, &[0, 1]),
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

    // FR-NAV-05
    #[test]
    fn a_tab_in_an_entrys_text_or_detail_is_drawn_as_a_space() {
        let candidates = [Candidate {
            detail: "c\td".into(),
            ..candidate("a\tb")
        }];
        let view = FinderView {
            query: "",
            entries: Entries::new(&candidates, &[0]),
            selected: 0,
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 14, 4));
        FinderPopup::new("Find", &view, IconSet::Unicode, &ICON, &THEME).render(buf.area, &mut buf);
        let row: String = (0..14).map(|x| buf[(x, 2)].symbol()).collect();
        assert_eq!(row, "│ › a b  c d │");
    }

    #[test]
    fn draws_each_entrys_icon_and_text_in_the_style_its_value_gives() {
        let (one, two) = (candidate("One"), candidate("Two"));
        let two = Candidate { value: 2, ..two };
        let candidates = [one, two];
        let view = FinderView {
            query: "",
            entries: Entries::new(&candidates, &[0, 1]),
            selected: 1,
        };
        let text = |value: &u8| Style::new().fg(Color::Indexed(value + 100));
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 5));
        FinderPopup::new("Find", &view, IconSet::Unicode, &ICON, &THEME)
            .text_style(&text)
            .render(buf.area, &mut buf);
        assert_eq!(buf[(2, 2)].fg, Color::Indexed(100), "the icon too");
        assert_eq!(buf[(4, 2)].fg, Color::Indexed(100));
        assert_eq!(buf[(6, 2)].fg, Color::Indexed(100));
        assert_eq!(buf[(3, 2)].fg, Color::Reset, "not the space before it");
        assert_eq!(buf[(4, 3)].fg, Color::Indexed(102));
        assert!(buf[(4, 3)].modifier.contains(Modifier::REVERSED));
    }

    /// The rows inside the frame of `finder` drawn over `area`, trimmed.
    fn drawn(finder: FinderPopup<'_, u8>, area: Rect) -> Vec<String> {
        let mut buf = Buffer::empty(area);
        finder.render(area, &mut buf);
        (1..area.height - 1)
            .map(|y| {
                (1..area.width - 1)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect()
    }

    // D-153
    #[test]
    fn headings_stand_over_each_run_of_a_tier_and_count_in_its_height() {
        let tiered = |text, tier| Candidate {
            tier,
            ..candidate(text)
        };
        let candidates = [
            tiered("A", 0),
            tiered("B", 0),
            tiered("C", 1),
            tiered("D", 5),
        ];
        let view = FinderView {
            query: "",
            entries: Entries::new(&candidates, &[0, 1, 2, 3]),
            selected: 0,
        };
        let finder = || {
            FinderPopup::new("Find", &view, IconSet::Unicode, &ICON, &THEME)
                .headings(&["Dark", "Light"])
        };
        assert_eq!(
            finder().area(Rect::new(0, 0, 100, 30)).height,
            9,
            "4 entries, 2 headings, the query and the frame"
        );
        assert_eq!(
            drawn(finder(), Rect::new(0, 0, 12, 9)),
            ["›", " Dark", " › A", " › B", " Light", " › C", " › D",],
            "a tier without a heading has none"
        );
        let mut buf = Buffer::empty(Rect::new(0, 0, 12, 9));
        finder().render(buf.area, &mut buf);
        assert_eq!(buf[(2, 2)].modifier, Modifier::BOLD | Modifier::DIM);
        assert!(buf[(1, 3)].modifier.contains(Modifier::REVERSED));
        let plain = FinderPopup::new("Find", &view, IconSet::Unicode, &ICON, &THEME);
        assert_eq!(plain.area(Rect::new(0, 0, 100, 30)).height, 7);
    }

    // D-153
    #[test]
    fn the_highlighted_entry_scrolls_into_view_with_its_heading_above_it() {
        let candidates: Vec<Candidate<u8>> = (0..12)
            .map(|n| Candidate {
                tier: u8::from(n >= 8),
                ..candidate(&n.to_string())
            })
            .collect();
        let ranked: Vec<usize> = (0..candidates.len()).collect();
        let view = |selected| FinderView {
            query: "",
            entries: Entries::new(&candidates, &ranked),
            selected,
        };
        let area = Rect::new(0, 0, 12, 8);
        let rows = |selected| {
            let view = view(selected);
            drawn(
                FinderPopup::new("Find", &view, IconSet::Unicode, &ICON, &THEME)
                    .headings(&["Dark", "Light"]),
                area,
            )
        };
        assert_eq!(rows(0)[1..3], [" Dark", " › 0"]);
        assert_eq!(rows(11)[5], " › 11", "the last entry at the bottom");
        let first_light = rows(8);
        let at = first_light.iter().position(|row| row == " › 8").unwrap();
        assert_eq!(first_light[at - 1], " Light");
    }

    #[test]
    fn debug_shows_the_title_and_icons() {
        let view = FinderView::<u8> {
            query: "",
            entries: Entries::new(&[], &[]),
            selected: 0,
        };
        let finder = FinderPopup::new("Find", &view, IconSet::Ascii, &ICON, &THEME);
        assert_eq!(
            format!("{finder:?}"),
            "FinderPopup { title: \"Find\", icons: Ascii, .. }"
        );
    }
}
