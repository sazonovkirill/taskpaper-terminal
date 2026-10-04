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
}

impl<'a> OutlineView<'a> {
    /// A view of `rows`, drawn from the top of its area.
    #[must_use]
    pub fn new(rows: &'a [Row]) -> Self {
        Self { rows }
    }
}

impl Widget for OutlineView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines: Vec<Line> = self.rows.iter().map(row_line).collect();
        Paragraph::new(lines).render(area, buf);
    }
}

/// Every row reserves a marker column so notes line up with the text of sibling tasks.
fn row_line(row: &Row) -> Line<'_> {
    let (marker, style) = match row.kind {
        ItemKind::Project => ("▾", Style::new().bold()),
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

    #[test]
    fn clips_rows_that_do_not_fit() {
        let expected = Buffer::with_lines(["☐ abc", "☐ d  "]);
        assert_eq!(render("- abcdef\n- d\n- e", 5, 2), expected);
    }

    #[test]
    fn empty_outline_draws_nothing() {
        assert_eq!(render("", 4, 1), Buffer::with_lines(["    "]));
    }
}
