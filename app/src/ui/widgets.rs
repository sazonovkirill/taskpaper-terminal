//! Widgets that draw the core's view state ([ui.md §2](../../../docs/specs/ui.md#2-blocks)).

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::{app::Row, document::BlockKind};

/// The outline: one row per block, 2 columns of indent per level, then a marker and the text; a
/// block with several lines takes several screen lines.
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

    /// Starts drawing at screen line `offset` of the outline instead of its first line.
    #[must_use]
    pub fn offset(self, offset: usize) -> Self {
        Self { offset, ..self }
    }

    /// Highlights every line of row `index` across the full width (ui.md §2).
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
        let width = usize::from(area.width);
        let lines = self.rows.iter().enumerate().flat_map(|(index, row)| {
            row_lines(row, width)
                .into_iter()
                .map(move |line| (index, line))
        });
        for ((index, line), line_area) in lines.skip(self.offset).zip(area.rows()) {
            line.render(line_area, buf);
            if Some(index) == self.focused {
                buf.set_style(line_area, Style::new().reversed());
            }
        }
    }
}

/// The screen lines of one block, `width` columns wide. Every block reserves a marker column so
/// text lines up with the text of sibling tasks.
fn row_lines(row: &Row, width: usize) -> Vec<Line<'_>> {
    let indent = "  ".repeat(row.depth);
    let (marker, style) = match row.kind {
        BlockKind::Project => ("▾", Style::new().bold()),
        BlockKind::Task if row.done => ("☑", Style::new().dim().crossed_out()),
        BlockKind::Task => ("☐", Style::new()),
        BlockKind::Url => ("↗", Style::new().underlined().cyan()),
        BlockKind::Section => (" ", Style::new().bold()),
        BlockKind::Code => (" ", code_style()),
        BlockKind::Note | BlockKind::Divider => (" ", Style::new()),
    };
    row.lines
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let prefix = if index == 0 {
                format!("{indent}{marker} ")
            } else {
                format!("{indent}  ")
            };
            let mut line = Line::from(vec![Span::raw(prefix)]);
            match row.kind {
                BlockKind::Divider => line.push_span(rule(&line, width)),
                BlockKind::Section => {
                    line.push_span(Span::styled(text.as_str(), style));
                    line.push_span(" ");
                    line.push_span(rule(&line, width));
                }
                BlockKind::Code => {
                    line.push_span(Span::styled(text.as_str(), style));
                    let fill = width.saturating_sub(line.width());
                    line.push_span(Span::styled(" ".repeat(fill), style));
                }
                _ => line.push_span(Span::styled(text.as_str(), style)),
            }
            line
        })
        .collect()
}

/// A horizontal rule from the end of `line` to the right edge.
fn rule(line: &Line, width: usize) -> Span<'static> {
    Span::raw("─".repeat(width.saturating_sub(line.width())))
}

/// The background band of code blocks; themes arrive in step 6.2.
fn code_style() -> Style {
    Style::new().bg(Color::DarkGray)
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
