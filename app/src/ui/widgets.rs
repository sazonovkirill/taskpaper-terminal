//! Widgets that draw the core's view state ([ui.md §2](../../../docs/specs/ui.md#2-blocks)).

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use crate::{app::Row, document::ItemKind};

/// The outline: one row per line, 2 columns of indent per level, then a marker and the text.
#[derive(Debug)]
pub struct OutlineView<'a> {
    rows: &'a [Row],
    offset: usize,
    focused: Option<usize>,
}

impl<'a> OutlineView<'a> {
    /// A view of `rows`, drawn from the top of its area with no row focused.
    #[must_use]
    pub fn new(rows: &'a [Row]) -> Self {
        Self {
            rows,
            offset: 0,
            focused: None,
        }
    }

    /// Starts drawing at row `offset` instead of the first row.
    #[must_use]
    pub fn offset(self, offset: usize) -> Self {
        Self { offset, ..self }
    }

    /// Highlights row `index` across the full width (ui.md §2).
    #[must_use]
    pub fn focused(self, index: Option<usize>) -> Self {
        Self {
            focused: index,
            ..self
        }
    }
}

impl Widget for OutlineView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines: Vec<Line> = self
            .rows
            .iter()
            .skip(self.offset)
            .take(usize::from(area.height))
            .map(row_line)
            .collect();
        Paragraph::new(lines).render(area, buf);
        let focused_area = self
            .focused
            .and_then(|index| index.checked_sub(self.offset))
            .and_then(|row| area.rows().nth(row));
        if let Some(focused_area) = focused_area {
            buf.set_style(focused_area, Style::new().reversed());
        }
    }
}

/// Every row reserves a marker column so notes line up with the text of sibling tasks.
fn row_line(row: &Row) -> Line<'_> {
    let (marker, style) = match row.kind {
        ItemKind::Project => ("▾", Style::new().bold()),
        ItemKind::Task if row.done => ("☑", Style::new().dim().crossed_out()),
        ItemKind::Task => ("☐", Style::new()),
        ItemKind::Note => (" ", Style::new()),
    };
    Line::from(vec![
        Span::raw(format!("{}{marker} ", "  ".repeat(row.depth))),
        Span::styled(row.text.as_str(), style),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app::State, document::format};

    #[track_caller]
    fn render(text: &str, width: u16, height: u16) -> Buffer {
        let rows = State::new(format::parse(text), None).rows();
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        OutlineView::new(&rows).render(buf.area, &mut buf);
        buf
    }

    #[track_caller]
    fn render_scrolled(offset: usize, focused: Option<usize>, area: Rect) -> Buffer {
        let rows = State::new(format::parse("- a\n- b\n- c\n- d"), None).rows();
        let mut buf = Buffer::empty(area);
        OutlineView::new(&rows)
            .offset(offset)
            .focused(focused)
            .render(area, &mut buf);
        buf
    }

    #[test]
    fn draws_markers_indents_and_bold_projects() {
        let mut expected = Buffer::with_lines([
            "▾ Home       ",
            "  ☐ Milk     ",
            "      on sale",
            "  note       ",
        ]);
        expected.set_style(Rect::new(2, 0, 4, 1), Style::new().bold());
        assert_eq!(
            render("Home:\n\t- Milk\n\t\ton sale\nnote", 13, 4),
            expected
        );
    }

    // FR-DONE-01
    #[test]
    fn done_tasks_are_checked_dimmed_and_struck_through() {
        let mut expected = Buffer::with_lines(["☑ a ", "☐ b ", "▾ P ", "  n "]);
        expected.set_style(Rect::new(2, 0, 1, 1), Style::new().dim().crossed_out());
        expected.set_style(Rect::new(2, 2, 1, 1), Style::new().bold());
        assert_eq!(
            render("- a @done\n- b\nP: @done\nn @done(2026-10-01)", 4, 4),
            expected
        );
    }

    #[test]
    fn clips_rows_that_do_not_fit() {
        let expected = Buffer::with_lines(["☐ abc", "☐ d  "]);
        assert_eq!(render("- abcdef\n- d\n- e", 5, 2), expected);
    }

    #[test]
    fn highlights_the_focused_row_across_the_full_width() {
        let mut expected = Buffer::with_lines(["☐ a  ", "☐ b  ", "☐ c  "]);
        expected.set_style(Rect::new(0, 1, 5, 1), Style::new().reversed());
        assert_eq!(render_scrolled(0, Some(1), Rect::new(0, 0, 5, 3)), expected);
    }

    #[test]
    fn starts_at_the_offset_and_highlights_relative_to_it() {
        let area = Rect::new(2, 1, 4, 2);
        let mut expected = Buffer::with_lines(["☐ c ", "☐ d "]);
        expected.area = area;
        expected.set_style(Rect::new(2, 2, 4, 1), Style::new().reversed());
        assert_eq!(render_scrolled(2, Some(3), area), expected);
    }

    #[test]
    fn focus_outside_the_view_is_not_highlighted() {
        let expected = Buffer::with_lines(["☐ b ", "☐ c "]);
        assert_eq!(render_scrolled(1, Some(0), Rect::new(0, 0, 4, 2)), expected);
        assert_eq!(render_scrolled(1, Some(3), Rect::new(0, 0, 4, 2)), expected);
        assert_eq!(render_scrolled(1, None, Rect::new(0, 0, 4, 2)), expected);
    }

    #[test]
    fn empty_outline_draws_nothing() {
        assert_eq!(render("", 4, 1), Buffer::with_lines(["    "]));
    }
}
