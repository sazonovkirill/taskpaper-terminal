//! The navigation sidebar on the left: bookmarks, Today and the project tree, with a rule on its
//! right edge ([ui.md §1](../../../../docs/specs/ui.md#1-layout), FR-UI-01).

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Widget},
};

use super::{
    FOCUS_MARKER, GUTTER_WIDTH, bookmark_style, current_line_style, rule_style, today_style,
};
use crate::{
    app::{Fold, SidebarEntry, SidebarView},
    ui::{
        layout,
        theme::{Icon, IconSet, Theme},
    },
};

/// The sidebar's lines: a heading over the bookmarks when there are any, the bookmarks, Today, a
/// blank line and a heading over the projects when there are any, and the projects, top-level ones in line with
/// the headings and 2 columns more per level, each right behind a marker of its fold, which for a top-level
/// project stands in the gutter. The focused entry is marked in a gutter like the outline's
/// focused block and gets the current-line band.
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
    /// to keep the focused entry visible and never past the last line.
    #[must_use]
    pub fn scroll(&self, offset: usize, height: usize) -> usize {
        let lines = self.lines().count();
        match self.focused_line() {
            Some(focused) => layout::scroll_offset(offset, focused..focused + 1, height, lines),
            None => offset.min(lines.saturating_sub(height)),
        }
    }

    /// The line of the focused entry, counted from the first, while the sidebar has the keys.
    #[must_use]
    pub fn focused_line(&self) -> Option<usize> {
        self.lines().position(|(_, focused)| focused)
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
            SidebarEntry::Bookmark { .. } | SidebarEntry::Today | SidebarEntry::Project { .. } => {
                None
            }
        };
        Some((entries, fold))
    }

    /// Each line with whether it shows the focused entry, built only when drawn since a large
    /// outline has many more than fit.
    fn lines(&self) -> impl Iterator<Item = (SidebarLine<'a>, bool)> {
        let entries = &self.view.entries[..];
        let focused = self.view.focused;
        entries.iter().enumerate().flat_map(move |(index, entry)| {
            let previous = index.checked_sub(1).and_then(|at| entries.get(at));
            let heading = match entry {
                SidebarEntry::Bookmark { .. } if previous.is_none() => Some("Bookmarks"),
                SidebarEntry::Project { .. } if previous == Some(&SidebarEntry::Today) => {
                    Some("Projects")
                }
                SidebarEntry::Bookmark { .. }
                | SidebarEntry::Today
                | SidebarEntry::Project { .. } => None,
            };
            let blank = heading.filter(|_| matches!(entry, SidebarEntry::Project { .. }));
            blank
                .map(|_| SidebarLine::Blank)
                .into_iter()
                .chain(heading.map(SidebarLine::Heading))
                .map(|line| (line, false))
                .chain([(SidebarLine::Entry(entry), Some(index) == focused)])
        })
    }
}

/// A line of the sidebar before it is drawn.
#[derive(Debug, Clone, Copy)]
enum SidebarLine<'a> {
    /// The blank line above the projects' heading.
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

/// An entry's line in `theme`: its icon or fold marker from `icons` and its text.
fn entry_line<'a>(entry: &'a SidebarEntry, icons: IconSet, theme: &Theme) -> Line<'a> {
    match entry {
        SidebarEntry::Bookmark { text, .. } => Line::from(vec![
            Span::styled(Icon::Bookmark.glyph(icons), bookmark_style(theme)),
            Span::raw(" "),
            Span::raw(text.as_str()),
        ]),
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
        for ((line, focused), (gutter_area, line_area)) in self
            .lines()
            .skip(self.offset)
            .zip(gutter.rows().zip(text.rows()))
        {
            if focused {
                buf.set_style(gutter_area.union(line_area), current_line_style(self.theme));
                Span::styled(FOCUS_MARKER, Style::new().fg(self.theme.focus))
                    .render(gutter_area, buf);
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
            line.line(self.icons, self.theme).render(line_area, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::THEME;
    use ratatui::style::Modifier;

    use super::*;
    use crate::{app::State, document::format};

    /// Draws the sidebar of `text` 20 columns wide and returns its rows, without the rule.
    fn draw(text: &str) -> Buffer {
        let view = State::new(format::parse(text), None)
            .sidebar()
            .expect("shown by default");
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 7));
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

    // FR-UI-01
    #[test]
    fn each_section_heading_comes_once_above_its_first_entry() {
        let bookmark = Icon::Bookmark.glyph(IconSet::Unicode);
        assert_eq!(
            rows(&draw("A: @bookmark\nB: @bookmark")),
            [
                "  Bookmarks".to_owned(),
                format!("  {bookmark} A"),
                format!("  {bookmark} B"),
                format!("  {} Today", Icon::Today.glyph(IconSet::Unicode)),
                String::new(),
                "  Projects".to_owned(),
                "  A".to_owned(),
            ]
        );
    }

    // FR-KEY-05
    #[test]
    fn a_line_names_its_entry_and_a_column_its_fold_marker() {
        let view = State::new(format::parse("A: @bookmark\n\tB:\nC:"), None)
            .sidebar()
            .expect("shown by default");
        let panel = SidebarPanel::new(&view, IconSet::Unicode, &THEME);
        // Bookmarks, A, Today, blank, Projects, ▾ A, B, C.
        assert_eq!(panel.entry_at(0, 0), None, "a heading");
        assert_eq!(panel.entry_at(1, 0), Some((0, None)));
        assert_eq!(panel.entry_at(2, 2), Some((1, None)));
        assert_eq!(panel.entry_at(3, 0), None, "the blank line");
        assert_eq!(panel.entry_at(5, 1), Some((2, Some(Fold::Expanded))));
        assert_eq!(panel.entry_at(5, 0), Some((2, None)));
        assert_eq!(panel.entry_at(5, 2), Some((2, None)));
        assert_eq!(
            panel.entry_at(6, 3),
            Some((3, None)),
            "a leaf has no marker"
        );
        assert_eq!(panel.entry_at(7, 0), Some((4, None)));
        assert_eq!(panel.entry_at(8, 0), None, "past the last entry");
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

    // FR-UI-01
    #[test]
    fn headings_are_bold_and_dim() {
        let buf = draw("A: @bookmark");
        for y in [0, 4] {
            let modifier = buf[(2, y)].modifier;
            assert!(modifier.contains(Modifier::BOLD | Modifier::DIM), "row {y}");
        }
        assert!(buf[(4, 1)].modifier.is_empty(), "entries are plain");
    }
}
