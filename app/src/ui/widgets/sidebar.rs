//! The navigation sidebar on the left: the inbox, Today, bookmarks and the project tree, with a
//! rule on its right edge ([ui.md §1](../../../../docs/specs/ui.md#1-layout), FR-UI-01).

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Widget},
};

use super::{
    FOCUS_MARKER, GUTTER_WIDTH, current_line_style, rule_style, selection_style, today_style,
};
use crate::{
    app::{Fold, SidebarEntry, SidebarView},
    ui::{
        layout,
        theme::{Icon, IconSet, Theme},
    },
};

/// The sidebar's lines: the inbox and Today, then for each of the bookmarks and the projects that
/// has entries, a blank line, a heading and its entries (D-68). Bookmarks are their text alone;
/// projects are 2 columns more per level, each right behind a marker of its fold, which for a
/// top-level project stands in the gutter. The focused entry is marked in a gutter like the
/// outline's focused block and gets the current-line band; while the pane has the keys, the entry
/// of where it is gets a selected block's marker instead (D-68). A project or the inbox with open
/// tasks has their count dimmed at the right, its text cut off a space before it (D-73).
#[derive(Debug)]
pub struct SidebarPanel<'a> {
    view: &'a SidebarView,
    offset: usize,
    icons: IconSet,
    theme: &'a Theme,
}

impl<'a> SidebarPanel<'a> {
    /// A panel of `view` in `theme` drawn from its first line, its icons and fold markers from
    /// `icons`.
    #[must_use]
    pub fn new(view: &'a SidebarView, icons: IconSet, theme: &'a Theme) -> Self {
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

    /// The first line to draw in `height` rows from `offset` on, scrolling as little as possible
    /// to keep the focused or current entry visible and never past the last line.
    #[must_use]
    pub fn scroll(&self, offset: usize, height: usize) -> usize {
        let lines = self.lines().count();
        match self.lines().position(|(_, mark)| mark != Mark::None) {
            Some(marked) => layout::scroll_offset(offset, marked..marked + 1, height, lines),
            None => offset.min(lines.saturating_sub(height)),
        }
    }

    /// The line of the focused entry, counted from the first, while the sidebar has the keys.
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
            entries += usize::from(matches!(shown, SidebarLine::Entry(_)));
        }
        let (SidebarLine::Entry(entry), _) = self.lines().nth(line)? else {
            return None;
        };
        let fold = match *entry {
            SidebarEntry::Project { depth, fold, .. }
                if fold != Fold::Leaf
                    && usize::from(column) == usize::from(GUTTER_WIDTH) - 1 + depth * 2 =>
            {
                Some(fold)
            }
            SidebarEntry::Inbox { .. }
            | SidebarEntry::Bookmark { .. }
            | SidebarEntry::Today
            | SidebarEntry::Project { .. } => None,
        };
        Some((entries, fold))
    }

    /// Each line with how it is marked, built only when drawn since a large
    /// outline has many more than fit.
    fn lines(&self) -> impl Iterator<Item = (SidebarLine<'a>, Mark)> {
        let entries = &self.view.entries[..];
        let SidebarView {
            focused, current, ..
        } = *self.view;
        entries.iter().enumerate().flat_map(move |(index, entry)| {
            let previous = index.checked_sub(1).and_then(|at| entries.get(at));
            let heading = match (entry, previous) {
                (SidebarEntry::Bookmark { .. }, Some(SidebarEntry::Today)) => Some("Bookmarks"),
                (
                    SidebarEntry::Project { .. },
                    Some(SidebarEntry::Today | SidebarEntry::Bookmark { .. }),
                ) => Some("Projects"),
                (
                    SidebarEntry::Inbox { .. }
                    | SidebarEntry::Today
                    | SidebarEntry::Bookmark { .. }
                    | SidebarEntry::Project { .. },
                    _,
                ) => None,
            };
            heading
                .into_iter()
                .flat_map(|heading| [SidebarLine::Blank, SidebarLine::Heading(heading)])
                .map(|line| (line, Mark::None))
                .chain([(SidebarLine::Entry(entry), Mark::of(index, focused, current))])
        })
    }
}

/// How a line of the sidebar is marked in its gutter.
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

/// A line of the sidebar before it is drawn.
#[derive(Debug, Clone, Copy)]
enum SidebarLine<'a> {
    /// The blank line above a heading.
    Blank,
    /// A section heading.
    Heading(&'static str),
    /// An entry.
    Entry(&'a SidebarEntry),
}

impl<'a> SidebarLine<'a> {
    /// The line as drawn in `theme`, its icons from `icons`.
    fn line(self, icons: IconSet, theme: &Theme) -> Line<'a> {
        match self {
            Self::Blank => Line::default(),
            Self::Heading(heading) => Line::styled(heading, heading_style()),
            Self::Entry(entry) => entry_line(entry, icons, theme),
        }
    }
}

/// An entry's line in `theme`: the inbox's or Today's icon or a project's fold marker from
/// `icons`, and its text.
fn entry_line<'a>(entry: &'a SidebarEntry, icons: IconSet, theme: &Theme) -> Line<'a> {
    match entry {
        SidebarEntry::Inbox { text, .. } => Line::from(vec![
            Span::raw(Icon::Inbox.glyph(icons)),
            Span::raw(" "),
            Span::raw(text.as_str()),
        ]),
        SidebarEntry::Bookmark { text, .. } => Line::raw(text.as_str()),
        SidebarEntry::Today => Line::from(vec![
            Span::styled(Icon::Today.glyph(icons), today_style(theme)),
            Span::raw(" Today"),
        ]),
        SidebarEntry::Project {
            text, depth, fold, ..
        } => {
            let marker = match fold {
                Fold::Leaf => " ",
                Fold::Expanded => Icon::Expanded.glyph(icons),
                Fold::Collapsed => Icon::Collapsed.glyph(icons),
            };
            Line::from(vec![
                Span::raw(format!("{:indent$}{marker}", "", indent = depth * 2)),
                Span::raw(text.as_str()),
            ])
        }
    }
}

/// The section headings: bold and dimmed, in no color of their own.
fn heading_style() -> Style {
    Style::new().bold().dim()
}

impl Widget for SidebarPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border = Block::new()
            .borders(Borders::RIGHT)
            .border_style(rule_style(self.theme));
        let inner = border.inner(area);
        border.render(area, buf);
        let [gutter, text] = inner.layout(&Layout::horizontal([
            Constraint::Length(GUTTER_WIDTH),
            Constraint::Fill(1),
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
            let line_area = if matches!(line, SidebarLine::Entry(SidebarEntry::Project { .. })) {
                Rect {
                    x: line_area.x - 1,
                    width: line_area.width + 1,
                    ..line_area
                }
            } else {
                line_area
            };
            let line_area = match line {
                SidebarLine::Entry(
                    &(SidebarEntry::Project { open_tasks, .. }
                    | SidebarEntry::Inbox { open_tasks, .. }),
                ) if open_tasks > 0 => render_count(open_tasks, line_area, buf),
                SidebarLine::Blank | SidebarLine::Heading(_) | SidebarLine::Entry(_) => line_area,
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
    use ratatui::style::{Color, Modifier};

    use super::*;
    use crate::{
        app::{Message, State},
        document::format,
    };

    /// Draws the sidebar of `text` 20 columns wide and returns its rows, without the rule.
    fn draw(text: &str) -> Buffer {
        let view = State::new(format::parse(text), None)
            .sidebar()
            .expect("shown by default");
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 8));
        SidebarPanel::new(&view, IconSet::Unicode, &THEME).render(buf.area, &mut buf);
        buf
    }

    fn rows(buf: &Buffer) -> Vec<String> {
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width - 1)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect()
    }

    // FR-UI-01, D-68
    #[test]
    fn today_comes_first_and_each_section_heading_once_above_its_first_entry() {
        assert_eq!(
            rows(&draw("A: @bookmark\nB: @bookmark")),
            [
                format!("  {} Today", Icon::Today.glyph(IconSet::Unicode)),
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
                format!("  {} INBOX         1", Icon::Inbox.glyph(IconSet::Unicode)),
                format!("  {} Today", Icon::Today.glyph(IconSet::Unicode)),
                String::new(),
                "  Projects".to_owned(),
                "  A".to_owned(),
            ]
        );
        let view = State::new(format::parse("INBOX:\n\tA:"), None)
            .sidebar()
            .expect("shown by default");
        let panel = SidebarPanel::new(&view, IconSet::Unicode, &THEME);
        assert_eq!(panel.entry_at(0, 1), Some((0, None)), "no fold marker");
    }

    // FR-KEY-05
    #[test]
    fn a_line_names_its_entry_and_a_column_its_fold_marker() {
        let view = State::new(format::parse("A: @bookmark\n\tB:\nC:"), None)
            .sidebar()
            .expect("shown by default");
        let panel = SidebarPanel::new(&view, IconSet::Unicode, &THEME);
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
        let view = State::new(format::parse("A:\n\tB:\n\t\tC:"), None)
            .sidebar()
            .expect("shown by default");
        let panel = SidebarPanel::new(&view, IconSet::Unicode, &THEME);
        // Today, blank, Projects, ▾ A, ▾ B, C.
        assert_eq!(panel.entry_at(4, 3), Some((2, Some(Fold::Expanded))));
        assert_eq!(panel.entry_at(4, 1), Some((2, None)));
    }

    // D-68
    #[test]
    fn the_current_entry_gets_a_selected_blocks_marker_and_is_scrolled_to() {
        let mut state = State::new(format::parse("A:\nB:\nC:\nD:"), None);
        state.update(Message::FocusLast);
        state.update(Message::ZoomIn);
        let view = state.sidebar().expect("shown by default");
        assert_eq!(view.current, Some(4), "D");
        let panel = SidebarPanel::new(&view, IconSet::Unicode, &THEME);
        // Today, blank, Projects, A, B, C, D.
        assert_eq!(panel.scroll(0, 4), 3, "D on the last row");
        assert_eq!(panel.scroll(0, 7), 0);
        assert_eq!(panel.focused_line(), None, "nothing focused");
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 7));
        panel.render(buf.area, &mut buf);
        assert_eq!(rows(&buf)[6], "▌ D");
        assert_eq!(
            Style::new()
                .fg(buf[(0, 6)].fg)
                .add_modifier(buf[(0, 6)].modifier),
            selection_style(&THEME)
        );
        assert_eq!(buf[(2, 6)].bg, Color::Reset, "no band");
        state.update(Message::FocusSidebar);
        let view = state.sidebar().expect("shown");
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 7));
        SidebarPanel::new(&view, IconSet::Unicode, &THEME).render(buf.area, &mut buf);
        assert_eq!(buf[(0, 6)].fg, THEME.focus, "the focus marker instead");
    }

    // D-73
    #[test]
    fn a_projects_open_task_count_is_dim_against_the_rule_and_cuts_its_text_off() {
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
        assert_eq!(buf[(18, 3)].modifier, Modifier::DIM);
        assert_eq!(buf[(17, 3)].symbol(), " ", "a space before the count");
        assert!(buf[(2, 3)].modifier.is_empty(), "the text is plain");
    }

    // D-73, D-68
    #[test]
    fn the_focused_band_covers_the_count() {
        let mut state = State::new(format::parse("Work:\n\t- a"), None);
        state.update(Message::FocusSidebar);
        state.update(Message::Sidebar(crate::app::SidebarInput::Focus(1)));
        let view = state.sidebar().expect("shown");
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 4));
        SidebarPanel::new(&view, IconSet::Unicode, &THEME).render(buf.area, &mut buf);
        assert_eq!(rows(&buf)[3], "▌ Work            1");
        let band = current_line_style(&THEME).bg;
        assert_eq!(Some(buf[(18, 3)].bg), band);
        assert_eq!(buf[(18, 3)].modifier, Modifier::DIM);
    }

    // FR-UI-01
    #[test]
    fn headings_are_bold_and_dim() {
        let buf = draw("A: @bookmark");
        for y in [2, 5] {
            let modifier = buf[(18, y)].modifier;
            assert!(modifier.contains(Modifier::BOLD | Modifier::DIM), "row {y}");
        }
        assert!(buf[(2, 3)].modifier.is_empty(), "entries are plain");
    }
}
