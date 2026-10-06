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
    FOCUS_MARKER, GUTTER_WIDTH, PRIMARY, bookmark_style, current_line_style, rule_style,
    today_style,
};
use crate::{
    app::{Fold, SidebarEntry, SidebarView},
    ui::{layout, theme::Icon},
};

/// The sidebar's lines: a heading over the bookmarks when there are any, the bookmarks, Today, a
/// blank line and a heading over the projects when there are any, and the projects, indented under it and 2 columns
/// more per level, behind a marker of their fold. The focused entry is marked in a gutter like the outline's
/// focused block and gets the current-line band.
#[derive(Debug)]
pub struct SidebarPanel<'a> {
    view: &'a SidebarView,
    offset: usize,
}

impl<'a> SidebarPanel<'a> {
    /// A panel of `view` drawn from its first line.
    #[must_use]
    pub fn new(view: &'a SidebarView) -> Self {
        Self { view, offset: 0 }
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
    /// The line as drawn.
    fn line(self) -> Line<'a> {
        match self {
            Self::Blank => Line::default(),
            Self::Heading(heading) => Line::styled(heading, heading_style()),
            Self::Entry(entry) => entry_line(entry),
        }
    }
}

/// An entry's line: its icon or fold marker and its text.
fn entry_line(entry: &SidebarEntry) -> Line<'_> {
    match entry {
        SidebarEntry::Bookmark { text, .. } => Line::from(vec![
            Span::styled(Icon::Bookmark.glyph(), bookmark_style()),
            Span::raw(" "),
            Span::raw(text.as_str()),
        ]),
        SidebarEntry::Today => Line::from(vec![
            Span::styled(Icon::Today.glyph(), today_style()),
            Span::raw(" Today"),
        ]),
        SidebarEntry::Project {
            text, depth, fold, ..
        } => {
            let marker = match fold {
                Fold::Leaf => " ",
                Fold::Expanded => "▾",
                Fold::Collapsed => "▸",
            };
            Line::from(vec![
                Span::raw(format!("{:indent$}{marker} ", "", indent = (depth + 1) * 2)),
                Span::raw(text.as_str()),
            ])
        }
    }
}

/// The section headings; themes arrive in step 6.2.
fn heading_style() -> Style {
    Style::new().bold().dim()
}

impl Widget for SidebarPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border = Block::new()
            .borders(Borders::RIGHT)
            .border_style(rule_style());
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
                buf.set_style(gutter_area.union(line_area), current_line_style());
                Span::styled(FOCUS_MARKER, Style::new().fg(PRIMARY)).render(gutter_area, buf);
            }
            line.line().render(line_area, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::style::Modifier;

    use super::*;
    use crate::{app::State, document::format};

    /// Draws the sidebar of `text` 20 columns wide and returns its rows, without the rule.
    fn draw(text: &str) -> Buffer {
        let view = State::new(format::parse(text), None)
            .sidebar()
            .expect("shown by default");
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 7));
        SidebarPanel::new(&view).render(buf.area, &mut buf);
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
        let bookmark = Icon::Bookmark.glyph();
        assert_eq!(
            rows(&draw("A: @bookmark\nB: @bookmark")),
            [
                "  Bookmarks".to_owned(),
                format!("  {bookmark} A"),
                format!("  {bookmark} B"),
                format!("  {} Today", Icon::Today.glyph()),
                String::new(),
                "  Projects".to_owned(),
                "      A".to_owned(),
            ]
        );
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
