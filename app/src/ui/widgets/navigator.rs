//! The Navigator, a window over the panes: the inbox, the special views, bookmarks and the
//! project tree in a centered box ([ui.md §1](../../../../docs/specs/ui.md#1-layout), FR-UI-01,
//! D-79).

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Clear, Widget},
};

use super::{
    FOCUS_MARKER, GUTTER_WIDTH, current_line_style, drawable, selection_style, view_title,
};
use crate::{
    app::{Fold, NavigatorEntry, NavigatorView},
    ui::{
        layout,
        theme::{Icon, IconSet, Theme},
    },
};

/// The widest the window gets.
const WIDTH: u16 = 48;

/// The window's title.
const TITLE: &str = "Navigator";

/// The Navigator, framed and titled in the accent color like the finders, with its filter
/// in the bottom border as the context menu has it (D-86), and its lines:
/// the inbox and the special views, then for each of the bookmarks and the projects that
/// has entries, a blank line unless it comes first, a heading and its entries (D-68, D-77, D-80). Bookmarks are their text alone;
/// projects are 2 columns more per level, each right behind a marker of its fold, which for a
/// top-level project stands in the gutter. The focused entry is marked in a gutter like the
/// outline's focused block and gets the current-line band; any other entry of where the focused
/// pane is gets a selected block's marker (D-68). An entry with open tasks has their count dimmed
/// at the right, its text cut off a space before it (D-73, D-85).
#[derive(Debug)]
pub struct NavigatorPanel<'a> {
    view: &'a NavigatorView,
    offset: usize,
    icons: IconSet,
    theme: &'a Theme,
}

impl<'a> NavigatorPanel<'a> {
    /// A panel of `view` in `theme` drawn from its first line, its icons and fold markers from
    /// `icons`.
    #[must_use]
    pub fn new(view: &'a NavigatorView, icons: IconSet, theme: &'a Theme) -> Self {
        Self {
            view,
            offset: 0,
            icons,
            theme,
        }
    }

    /// Starts drawing at line `offset` instead of the first.
    #[must_use]
    pub fn offset(self, offset: usize) -> Self {
        Self { offset, ..self }
    }

    /// Where the window goes in `area`: centered, as wide as it can be up to 48 columns and as
    /// tall as its lines with the border, up to the height of `area`.
    #[must_use]
    pub fn area(&self, area: Rect) -> Rect {
        let lines = u16::try_from(self.lines().count().max(1)).unwrap_or(u16::MAX);
        let height = lines.saturating_add(2).min(area.height);
        area.centered(Constraint::Length(WIDTH), Constraint::Length(height))
    }

    /// The first line to draw in `height` rows from `offset` on, scrolling as little as possible
    /// to keep the focused entry visible and never past the last line.
    #[must_use]
    pub fn scroll(&self, offset: usize, height: usize) -> usize {
        let lines = self.lines().count();
        match self.focused_line() {
            Some(focused) => layout::scroll_offset(offset, focused..focused + 1, height, lines),
            None => 0,
        }
    }

    /// The line of the focused entry, counted from the first, if it lists any.
    #[must_use]
    pub fn focused_line(&self) -> Option<usize> {
        self.lines().position(|(_, mark)| mark == Mark::Focused)
    }

    /// The index of the entry on line `line`, counted from the first, with its fold when column
    /// `column`, counted from the gutter's start, shows a project entry's fold marker (FR-KEY-05).
    #[must_use]
    pub fn entry_at(&self, line: usize, column: u16) -> Option<(usize, Option<Fold>)> {
        let mut entries = 0;
        for (shown, _) in self.lines().take(line) {
            entries += usize::from(matches!(shown, NavigatorLine::Entry(_)));
        }
        let (NavigatorLine::Entry(entry), _) = self.lines().nth(line)? else {
            return None;
        };
        let fold = match *entry {
            NavigatorEntry::Project { depth, fold, .. }
                if fold != Fold::Leaf
                    && usize::from(column) == usize::from(GUTTER_WIDTH) - 1 + depth * 2 =>
            {
                Some(fold)
            }
            NavigatorEntry::Inbox { .. }
            | NavigatorEntry::Bookmark { .. }
            | NavigatorEntry::View { .. }
            | NavigatorEntry::Project { .. } => None,
        };
        Some((entries, fold))
    }

    /// Each line with how it is marked, built only when drawn since a large
    /// outline has many more than fit.
    fn lines(&self) -> impl Iterator<Item = (NavigatorLine<'a>, Mark)> {
        let entries = &self.view.entries[..];
        let NavigatorView {
            focused, current, ..
        } = *self.view;
        entries.iter().enumerate().flat_map(move |(index, entry)| {
            let previous = index.checked_sub(1).and_then(|at| entries.get(at));
            let heading = match (entry, previous) {
                (NavigatorEntry::Bookmark { .. }, Some(NavigatorEntry::Bookmark { .. }))
                | (NavigatorEntry::Project { .. }, Some(NavigatorEntry::Project { .. }))
                | (NavigatorEntry::Inbox { .. } | NavigatorEntry::View { .. }, _) => None,
                (NavigatorEntry::Bookmark { .. }, _) => Some("Bookmarks"),
                (NavigatorEntry::Project { .. }, _) => Some("Projects"),
            };
            // The Navigator's first line is never blank (D-77).
            let blank = previous.map(|_| NavigatorLine::Blank);
            heading
                .into_iter()
                .flat_map(move |heading| blank.into_iter().chain([NavigatorLine::Heading(heading)]))
                .map(|line| (line, Mark::None))
                .chain([(
                    NavigatorLine::Entry(entry),
                    Mark::of(index, focused, current),
                )])
        })
    }
}

/// How a line of the Navigator is marked in its gutter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    /// Not at all.
    None,
    /// As the focused entry.
    Focused,
    /// As the entry of where the focused pane is.
    Current,
}

impl Mark {
    /// The mark of the entry at `index` when `focused` and `current` are those entries' indices.
    fn of(index: usize, focused: Option<usize>, current: Option<usize>) -> Self {
        if Some(index) == focused {
            Self::Focused
        } else if Some(index) == current {
            Self::Current
        } else {
            Self::None
        }
    }
}

/// A line of the Navigator before it is drawn.
#[derive(Debug, Clone, Copy)]
enum NavigatorLine<'a> {
    /// The blank line above a heading.
    Blank,
    /// A section heading.
    Heading(&'static str),
    /// An entry.
    Entry(&'a NavigatorEntry),
}

impl<'a> NavigatorLine<'a> {
    /// The line as drawn in `theme`, its icons from `icons`.
    fn line(self, icons: IconSet, theme: &Theme) -> Line<'a> {
        match self {
            Self::Blank => Line::default(),
            Self::Heading(heading) => Line::styled(heading, heading_style()),
            Self::Entry(entry) => entry_line(entry, icons, theme),
        }
    }
}

/// An entry's line in `theme`: the inbox's or a special view's icon or a project's fold marker from
/// `icons`, and its text.
fn entry_line<'a>(entry: &'a NavigatorEntry, icons: IconSet, theme: &Theme) -> Line<'a> {
    match entry {
        NavigatorEntry::Inbox { text, .. } => Line::from(vec![
            Span::raw(Icon::Inbox.glyph(icons)),
            Span::raw(" "),
            Span::raw(text.as_str()),
        ]),
        NavigatorEntry::Bookmark { text, .. } => Line::raw(drawable(text)),
        NavigatorEntry::View { view, .. } => view_title(*view, icons, theme),
        NavigatorEntry::Project {
            text, depth, fold, ..
        } => {
            let marker = match fold {
                Fold::Leaf => " ",
                Fold::Expanded => Icon::Expanded.glyph(icons),
                Fold::Collapsed => Icon::Collapsed.glyph(icons),
            };
            Line::from(vec![
                Span::raw(format!("{:indent$}{marker}", "", indent = depth * 2)),
                Span::raw(drawable(text)),
            ])
        }
    }
}

/// The section headings: bold and dimmed, in no color of their own.
fn heading_style() -> Style {
    Style::new().bold().dim()
}

impl Widget for NavigatorPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let accent = Style::new().fg(self.theme.accent);
        let frame = Block::bordered()
            .border_style(accent)
            .title_style(accent)
            .title(TITLE)
            .title_bottom(self.view.filter.as_str());
        let inner = frame.inner(area);
        frame.render(area, buf);
        // Today is always listed, so only a filter leaves no entries (D-102).
        if self.view.entries.is_empty() {
            Line::from(" Nothing matches").dim().render(inner, buf);
            return;
        }
        // The text keeps a column clear of the right border, as the gutter does of the left.
        let [gutter, text, _] = inner.layout(&Layout::horizontal([
            Constraint::Length(GUTTER_WIDTH),
            Constraint::Fill(1),
            Constraint::Length(1),
        ]));
        for ((line, mark), (gutter_area, line_area)) in self
            .lines()
            .skip(self.offset)
            .zip(gutter.rows().zip(text.rows()))
        {
            match mark {
                Mark::Focused => {
                    buf.set_style(gutter_area.union(line_area), current_line_style(self.theme));
                    Span::styled(FOCUS_MARKER, Style::new().fg(self.theme.focus))
                        .render(gutter_area, buf);
                }
                Mark::Current => {
                    Span::styled(FOCUS_MARKER, selection_style(self.theme))
                        .render(gutter_area, buf);
                }
                Mark::None => {}
            }
            // A project's line starts a column early, so its fold marker stands right before its
            // text (D-61).
            let line_area = if matches!(line, NavigatorLine::Entry(NavigatorEntry::Project { .. }))
            {
                Rect {
                    x: line_area.x - 1,
                    width: line_area.width + 1,
                    ..line_area
                }
            } else {
                line_area
            };
            let line_area = match line {
                NavigatorLine::Entry(entry) if entry.open_tasks() > 0 => {
                    render_count(entry.open_tasks(), line_area, buf)
                }
                NavigatorLine::Blank | NavigatorLine::Heading(_) | NavigatorLine::Entry(_) => {
                    line_area
                }
            };
            line.line(self.icons, self.theme).render(line_area, buf);
        }
    }
}

/// Draws `count` dimmed at the right of `area` and returns what is left of `area` for the text, a
/// space short of the count (D-73).
fn render_count(count: usize, area: Rect, buf: &mut Buffer) -> Rect {
    let count = count.to_string();
    let [text, _, count_area] = area.layout(&Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(u16::try_from(count.len()).expect("a usize has at most 20 digits")),
    ]));
    Span::styled(count, Style::new().dim()).render(count_area, buf);
    text
}

#[cfg(test)]
mod tests {
    use super::super::tests::THEME;
    use ratatui::{
        buffer::Cell,
        style::{Color, Modifier},
    };

    use super::*;
    use crate::{
        app::{Message, NavigatorInput, NavigatorView, SpecialView, State},
        document::format,
    };

    /// The state of `text` with the Navigator closed, Today the only special view it lists, as
    /// these tests were written for before the other views (D-80).
    fn only_today(text: &str) -> State {
        let mut state = State::new(format::parse(text), None);
        for view in SpecialView::ALL {
            if view != SpecialView::Today {
                state.update(Message::Configure(
                    crate::app::Configuration::NavigatorView(view, false),
                ));
            }
        }
        state
    }

    /// The state of `text` with the Navigator open, on its first entry, as [`only_today`] lists it.
    fn opened(text: &str) -> State {
        let mut state = only_today(text);
        state.update(Message::ToggleNavigator);
        state
    }

    fn view(state: &State) -> NavigatorView {
        state.navigator().expect("open")
    }

    /// Draws `view` in a window 22 columns wide whose inside is `height` rows tall.
    fn render(view: &NavigatorView, height: u16) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, 22, height + 2));
        NavigatorPanel::new(view, IconSet::Unicode, &THEME).render(buf.area, &mut buf);
        buf
    }

    /// Draws the open Navigator of `text` with 8 rows inside the window.
    fn draw(text: &str) -> Buffer {
        render(&view(&opened(text)), 8)
    }

    /// The cell at `x`, `y` inside the window's border.
    fn cell(buf: &Buffer, x: u16, y: u16) -> &Cell {
        &buf[(x + 1, y + 1)]
    }

    /// The rows inside the window's border, without the column it keeps clear of the right one.
    fn rows(buf: &Buffer) -> Vec<String> {
        (0..buf.area.height - 2)
            .map(|y| {
                (0..buf.area.width - 3)
                    .map(|x| cell(buf, x, y).symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect()
    }

    // ui.md §1, D-79
    #[test]
    fn the_window_is_framed_and_titled_in_the_accent_color_and_centered() {
        let buf = render(&view(&opened("A:")), 4);
        let top: String = (0..22).map(|x| buf[(x, 0)].symbol()).collect();
        assert_eq!(top, format!("┌{TITLE}{}┐", "─".repeat(20 - TITLE.len())));
        assert_eq!(buf[(0, 0)].fg, THEME.accent);
        assert_eq!(buf[(1, 0)].fg, THEME.accent, "the title");
        let state = opened("A:");
        let view = view(&state);
        let panel = NavigatorPanel::new(&view, IconSet::Unicode, &THEME);
        // Today, blank, Projects, A, with the border.
        assert_eq!(
            panel.area(Rect::new(0, 0, 100, 20)),
            Rect::new(26, 7, 48, 6)
        );
        assert_eq!(
            panel.area(Rect::new(2, 1, 30, 4)),
            Rect::new(2, 1, 30, 4),
            "no larger than where it goes"
        );
    }

    // D-86
    #[test]
    fn a_filter_matching_nothing_says_so_and_shows_in_the_bottom_border() {
        let mut state = opened("- a");
        state.update(Message::Navigator(NavigatorInput::Type('z')));
        let view = view(&state);
        assert_eq!(view.focused, None);
        let buf = render(&view, 1);
        assert_eq!(rows(&buf), [" Nothing matches"]);
        assert_eq!(cell(&buf, 1, 0).modifier, Modifier::DIM);
        assert_eq!(
            (0..4).map(|x| buf[(x, 2)].symbol()).collect::<String>(),
            "└z──"
        );
        let panel = NavigatorPanel::new(&view, IconSet::Unicode, &THEME);
        assert_eq!(panel.focused_line(), None);
        assert_eq!(panel.scroll(3, 2), 0);
        assert_eq!(panel.area(Rect::new(0, 0, 60, 20)).height, 3, "one line");
    }

    // FR-UI-01, D-68
    #[test]
    fn today_comes_first_and_each_section_heading_once_above_its_first_entry() {
        assert_eq!(
            rows(&draw("A: @bookmark\nB: @bookmark")),
            [
                format!("▌ {} Today", Icon::Today.glyph(IconSet::Unicode)),
                String::new(),
                "  Bookmarks".to_owned(),
                "  A".to_owned(),
                "  B".to_owned(),
                String::new(),
                "  Projects".to_owned(),
                "  A".to_owned(),
            ],
            "bookmarks without an icon, headings in line with the entries"
        );
        assert_eq!(
            rows(&draw("- a @bookmark"))[1..],
            ["", "  Bookmarks", "  a", "", "", "", ""],
            "no projects"
        );
    }

    // D-75
    #[test]
    fn the_inbox_comes_above_today_behind_its_icon() {
        assert_eq!(
            rows(&draw("INBOX:\n\t- a\nA:"))[..5],
            [
                format!("▌ {} INBOX         1", Icon::Inbox.glyph(IconSet::Unicode)),
                format!("  {} Today", Icon::Today.glyph(IconSet::Unicode)),
                String::new(),
                "  Projects".to_owned(),
                "  A".to_owned(),
            ]
        );
        let view = view(&opened("INBOX:\n\tA:"));
        let panel = NavigatorPanel::new(&view, IconSet::Unicode, &THEME);
        assert_eq!(panel.entry_at(0, 1), Some((0, None)), "no fold marker");
    }

    // FR-UI-01, D-79
    #[test]
    fn a_tab_in_a_bookmarks_or_projects_text_is_drawn_as_a_space() {
        assert_eq!(
            rows(&draw("a\tb @bookmark\nc\td:"))[2..7],
            ["  Bookmarks", "  a b", "", "  Projects", "  c d"]
        );
    }

    // D-77, D-86, D-102
    #[test]
    fn without_today_the_first_section_starts_at_the_top_or_below_the_inbox() {
        // Only a filter leaves Today out; none of the views' names holds a `q` or an `x`.
        let filtered = |filter: char, text: &str| {
            let mut state = opened(text);
            state.update(Message::Navigator(NavigatorInput::Type(filter)));
            rows(&render(&view(&state), 5))
        };
        // D-80, D-92, D-93: the special views stand together, a blank line after the last one
        // listed; Flagged's flag is red.
        let buf = {
            let mut state = State::new(format::parse("A:"), None);
            for view in [SpecialView::Overdue, SpecialView::Colors] {
                state.update(Message::Configure(
                    crate::app::Configuration::NavigatorView(view, false),
                ));
            }
            state.update(Message::ToggleNavigator);
            render(&view(&state), 7)
        };
        assert_eq!(
            rows(&buf),
            [
                format!("▌ {} Today", Icon::Today.glyph(IconSet::Unicode)),
                format!("  {} Upcoming", Icon::Upcoming.glyph(IconSet::Unicode)),
                format!("  {} Logbook", Icon::Logbook.glyph(IconSet::Unicode)),
                format!("  {} Starred", Icon::Star.glyph(IconSet::Unicode)),
                format!("  {} Flagged", Icon::Flag.glyph(IconSet::Unicode)),
                String::new(),
                "  Projects".to_owned(),
            ]
        );
        assert_eq!(
            cell(&buf, 2, 4).fg,
            THEME.tag(crate::document::TagColor::Red),
            "the flag's red"
        );
        assert_eq!(
            filtered('q', "Q: @bookmark"),
            ["  Bookmarks", "▌ Q", "", "  Projects", "  Q"]
        );
        assert_eq!(filtered('q', "Q:"), ["  Projects", "▌ Q", "", "", ""]);
        assert_eq!(
            filtered('x', "INBOX:\nX:"),
            [
                format!("▌ {} INBOX", Icon::Inbox.glyph(IconSet::Unicode)),
                String::new(),
                "  Projects".to_owned(),
                "  X".to_owned(),
                String::new(),
            ]
        );
    }

    // FR-KEY-05
    #[test]
    fn a_line_names_its_entry_and_a_column_its_fold_marker() {
        let view = view(&opened("A: @bookmark\n\tB:\nC:"));
        let panel = NavigatorPanel::new(&view, IconSet::Unicode, &THEME);
        // Today, blank, Bookmarks, A, blank, Projects, ▾ A, B, C.
        assert_eq!(panel.entry_at(0, 2), Some((0, None)));
        assert_eq!(panel.entry_at(1, 0), None, "the blank line");
        assert_eq!(panel.entry_at(2, 0), None, "a heading");
        assert_eq!(panel.entry_at(3, 2), Some((1, None)));
        assert_eq!(panel.entry_at(4, 0), None, "the blank line");
        assert_eq!(panel.entry_at(6, 1), Some((2, Some(Fold::Expanded))));
        assert_eq!(panel.entry_at(6, 0), Some((2, None)));
        assert_eq!(panel.entry_at(6, 2), Some((2, None)));
        assert_eq!(
            panel.entry_at(7, 3),
            Some((3, None)),
            "a leaf has no marker"
        );
        assert_eq!(panel.entry_at(8, 0), Some((4, None)));
        assert_eq!(panel.entry_at(9, 0), None, "past the last entry");
    }

    // D-61
    #[test]
    fn a_top_level_projects_marker_stands_in_the_gutter_and_its_text_reaches_the_edge() {
        assert_eq!(
            rows(&draw("ABCDEFGHIJKLMNOPQ:\n\tB:"))[2..5],
            [
                "  Projects".to_owned(),
                " ▾ABCDEFGHIJKLMNOPQ".to_owned(),
                "    B".to_owned(),
            ]
        );
    }

    // FR-KEY-05, D-60
    #[test]
    fn a_nested_projects_fold_marker_is_2_columns_in_per_level() {
        let view = view(&opened("A:\n\tB:\n\t\tC:"));
        let panel = NavigatorPanel::new(&view, IconSet::Unicode, &THEME);
        // Today, blank, Projects, ▾ A, ▾ B, C.
        assert_eq!(panel.entry_at(4, 3), Some((2, Some(Fold::Expanded))));
        assert_eq!(panel.entry_at(4, 1), Some((2, None)));
    }

    // D-68
    #[test]
    fn the_focused_entry_is_scrolled_to_and_the_current_one_gets_a_selected_blocks_marker() {
        let mut state = only_today("A:\nB:\nC:\nD:");
        state.update(Message::FocusLast);
        state.update(Message::ZoomIn);
        state.update(Message::ToggleNavigator);
        let opened = view(&state);
        assert_eq!((opened.focused, opened.current), (Some(4), Some(4)), "D");
        let panel = NavigatorPanel::new(&opened, IconSet::Unicode, &THEME);
        // Today, blank, Projects, A, B, C, D.
        assert_eq!(panel.focused_line(), Some(6));
        assert_eq!(panel.scroll(0, 4), 3, "D on the last row");
        assert_eq!(panel.scroll(0, 7), 0);
        let buf = render(&opened, 7);
        assert_eq!(rows(&buf)[6], "▌ D");
        assert_eq!(cell(&buf, 0, 6).fg, THEME.focus, "the focus marker");
        state.update(Message::Navigator(NavigatorInput::Previous));
        let buf = render(&view(&state), 7);
        assert_eq!(rows(&buf)[5..], ["▌ C", "▌ D"]);
        assert_eq!(cell(&buf, 0, 5).fg, THEME.focus);
        assert_eq!(
            Style::new()
                .fg(cell(&buf, 0, 6).fg)
                .add_modifier(cell(&buf, 0, 6).modifier),
            selection_style(&THEME)
        );
        assert_eq!(cell(&buf, 2, 6).bg, Color::Reset, "no band");
    }

    // D-85
    #[test]
    fn bookmarks_and_today_show_their_counts_as_projects_do() {
        assert_eq!(
            rows(&draw("- a @star\nNote @bookmark\n\t- b\nEmpty @bookmark"))[..5],
            [
                format!("▌ {} Today         1", Icon::Today.glyph(IconSet::Unicode)),
                String::new(),
                "  Bookmarks".to_owned(),
                "  Note            1".to_owned(),
                "  Empty".to_owned(),
            ]
        );
    }

    // D-73
    #[test]
    fn a_projects_open_task_count_is_dim_at_the_right_and_cuts_its_text_off() {
        let buf =
            draw("Work:\n\t- a\n\tSub:\n\t\t- b\nABCDEFGHIJKLMNOPQ:\n\t- c\nNone:\n\t- d @done");
        assert_eq!(
            rows(&buf)[2..7],
            [
                "  Projects",
                " ▾Work            2",
                "    Sub           1",
                "  ABCDEFGHIJKLMNO 1",
                "  None",
            ]
        );
        assert_eq!(cell(&buf, 18, 3).modifier, Modifier::DIM);
        assert_eq!(cell(&buf, 17, 3).symbol(), " ", "a space before the count");
        assert_eq!(
            cell(&buf, 19, 3).symbol(),
            " ",
            "a column clear of the border"
        );
        assert!(cell(&buf, 2, 3).modifier.is_empty(), "the text is plain");
    }

    // D-73, D-68
    #[test]
    fn the_focused_band_covers_the_count() {
        let mut state = opened("Work:\n\t- a");
        state.update(Message::Navigator(NavigatorInput::Focus(1)));
        let buf = render(&view(&state), 4);
        assert_eq!(rows(&buf)[3], "▌ Work            1");
        let band = current_line_style(&THEME).bg;
        assert_eq!(Some(cell(&buf, 18, 3).bg), band);
        assert_eq!(cell(&buf, 18, 3).modifier, Modifier::DIM);
    }

    // FR-UI-01
    #[test]
    fn headings_are_bold_and_dim() {
        let buf = draw("A: @bookmark");
        for y in [2, 5] {
            let modifier = cell(&buf, 18, y).modifier;
            assert!(modifier.contains(Modifier::BOLD | Modifier::DIM), "row {y}");
        }
        assert!(cell(&buf, 2, 3).modifier.is_empty(), "entries are plain");
    }
}
