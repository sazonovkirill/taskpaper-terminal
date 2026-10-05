//! Widgets that draw the core's view state ([ui.md §2](../../../docs/specs/ui.md#2-blocks)).

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::Widget,
};

use crate::{app::Row, document::BlockKind};

/// The outline: a gutter that marks the focused block, then one row per block, 2 columns of
/// indent per level, a marker and the text; a block with several lines takes several screen
/// lines.
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

    /// Marks every line of row `index` in the gutter (ui.md §2).
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
        let [gutter, text] = area.layout(&Layout::horizontal([
            Constraint::Length(GUTTER_WIDTH),
            Constraint::Fill(1),
        ]));
        let width = usize::from(text.width);
        let lines = self.rows.iter().enumerate().flat_map(|(index, row)| {
            row_lines(row, width)
                .into_iter()
                .map(move |line| (index, line))
        });
        for ((index, line), (gutter_area, line_area)) in
            lines.skip(self.offset).zip(gutter.rows().zip(text.rows()))
        {
            line.render(line_area, buf);
            if Some(index) == self.focused {
                Span::styled(FOCUS_MARKER, Style::new().fg(PRIMARY)).render(gutter_area, buf);
            }
        }
    }
}

/// Columns left of the outline for the focus marker; the breadcrumbs start after it too, so the
/// root crumb lines up with top-level blocks (D-35).
pub const GUTTER_WIDTH: u16 = 2;

/// Marks each line of the focused block in the gutter (D-30).
const FOCUS_MARKER: &str = "▎";

/// Stands for the document root in the breadcrumbs (D-32).
const HOME_ICON: &str = "⌂";

/// The breadcrumb header for a view zoomed into the last of `crumbs`: `⌂`, then each crumb,
/// dropping crumbs from the left behind `…` until it fits in `width` columns or only the last
/// one is left (navigation §4). Bold, as the focused pane's header (ui.md §1).
#[must_use]
pub fn breadcrumbs(crumbs: &[String], width: usize) -> Line<'static> {
    let path: Vec<&str> = std::iter::once(HOME_ICON)
        .chain(crumbs.iter().map(String::as_str))
        .collect();
    let mut line = Line::from(path.join(" › "));
    for start in 1..path.len() {
        if line.width() <= width {
            break;
        }
        let rest = path.get(start..).unwrap_or_default();
        line = Line::from(format!("… › {}", rest.join(" › ")));
    }
    line.bold()
}

/// The screen lines of one block, `width` columns wide, indented 2 columns per level as in
/// TaskPaper: tasks and URLs start with their marker, every other block with its text, so a
/// child's text lines up with its parent task's text (D-31). A collapsed block with children
/// shows `▸ n` after its text (ui.md §2). The zoom title is drawn like its block, in bold (D-34).
fn row_lines(row: &Row, width: usize) -> Vec<Line<'_>> {
    let indent = "  ".repeat(row.depth);
    let (marker, style) = match row.kind {
        BlockKind::Project => ("", Style::new().bold().fg(PRIMARY)),
        BlockKind::Task if row.done => ("✔ ", Style::new().dim().crossed_out()),
        BlockKind::Task => ("- ", Style::new()),
        BlockKind::Url => ("↗ ", Style::new().dim().underlined().cyan()),
        BlockKind::Section => ("", Style::new().bold().fg(SECONDARY)),
        BlockKind::Code => ("", code_style().dim()),
        BlockKind::Note => ("", Style::new().dim()),
        BlockKind::Divider => ("", divider_style()),
    };
    let style = if row.title { style.bold() } else { style };
    let count_style = if row.kind == BlockKind::Code {
        code_style()
    } else {
        Style::new()
    };
    let count = (row.hidden_children > 0)
        .then(|| Span::styled(format!(" ▸ {}", row.hidden_children), count_style.dim()));
    let last = row.lines.len().saturating_sub(1);
    row.lines
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let prefix = if index == 0 {
                format!("{indent}{marker}")
            } else {
                format!("{indent}{}", " ".repeat(marker.chars().count()))
            };
            let mut line = Line::from(vec![Span::raw(prefix)]);
            match row.kind {
                BlockKind::Divider => {}
                // TaskPaper marks a project with a trailing colon, which the display text drops.
                BlockKind::Project => line.push_span(Span::styled(format!("{text}:"), style)),
                _ => line.push_span(Span::styled(text.as_str(), style)),
            }
            if index == last
                && let Some(count) = count.clone()
            {
                line.push_span(count);
                if row.kind == BlockKind::Divider {
                    line.push_span(" ");
                }
            }
            match row.kind {
                BlockKind::Divider => line.push_span(rule(&line, width)),
                BlockKind::Code => {
                    let fill = width.saturating_sub(line.width());
                    line.push_span(Span::styled(" ".repeat(fill), code_style()));
                }
                _ => {}
            }
            line
        })
        .collect()
}

/// A horizontal rule from the end of `line` to the right edge, very dim.
fn rule(line: &Line, width: usize) -> Span<'static> {
    Span::styled(
        "─".repeat(width.saturating_sub(line.width())),
        divider_style(),
    )
}

/// Divider blocks: the rule-lines color, dimmed further so they recede behind the header rule
/// (D-33).
fn divider_style() -> Style {
    rule_style().dim()
}

/// The theme's accent (Textual's `$primary`), for projects; themes arrive in step 6.2.
const PRIMARY: Color = Color::LightBlue;

/// The theme's headers color (Textual's `$secondary`), for sections; themes arrive in step 6.2.
const SECONDARY: Color = Color::LightMagenta;

/// Rule lines: dividers and the line under the header; themes arrive in step 6.2.
#[must_use]
pub fn rule_style() -> Style {
    Style::new().fg(Color::DarkGray)
}

/// The background band of code blocks; themes arrive in step 6.2.
fn code_style() -> Style {
    Style::new().bg(Color::DarkGray)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::{Message, State},
        document::format,
    };

    /// Draws `rows` with nothing focused and returns the `width` × `height` columns right of
    /// the gutter.
    fn draw(rows: &[Row], width: u16, height: u16) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, width + GUTTER_WIDTH, height));
        OutlineView::new(rows).render(buf.area, &mut buf);
        let mut text = Buffer::empty(Rect::new(0, 0, width, height));
        for y in 0..height {
            for x in 0..width {
                text[(x, y)] = buf[(x + GUTTER_WIDTH, y)].clone();
            }
        }
        text
    }

    #[track_caller]
    fn render(text: &str, width: u16, height: u16) -> Buffer {
        draw(&State::new(format::parse(text), None).rows(), width, height)
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
    fn draws_markers_indents_projects_and_dim_notes() {
        let mut expected = Buffer::with_lines([
            "Home:        ",
            "  - Milk     ",
            "    on sale  ",
            "note         ",
        ]);
        expected.set_style(Rect::new(0, 0, 5, 1), Style::new().bold().fg(PRIMARY));
        expected.set_style(Rect::new(4, 2, 7, 1), Style::new().dim());
        expected.set_style(Rect::new(0, 3, 4, 1), Style::new().dim());
        assert_eq!(
            render("Home:\n\t- Milk\n\t\ton sale\nnote", 13, 4),
            expected
        );
    }

    // FR-DONE-01
    #[test]
    fn done_tasks_are_checked_dimmed_and_struck_through() {
        let mut expected = Buffer::with_lines(["✔ a ", "- b ", "P:  ", "n   "]);
        expected.set_style(Rect::new(2, 0, 1, 1), Style::new().dim().crossed_out());
        expected.set_style(Rect::new(0, 2, 2, 1), Style::new().bold().fg(PRIMARY));
        expected.set_style(Rect::new(0, 3, 1, 1), Style::new().dim());
        assert_eq!(
            render("- a @done\n- b\nP: @done\nn @done(2026-10-01)", 4, 4),
            expected
        );
    }

    // FR-NAV-02
    #[test]
    fn collapsed_blocks_show_a_dim_count_after_their_text() {
        let text = "P:\n\t- t\nn\nm\n\t- x\nS @section\n\t- y\n---\n\t- z\nf() @code\n\t- w\n- v";
        let mut state = State::new(format::parse(text), None);
        state.update(Message::CollapseAll);
        let buf = draw(&state.rows(), 14, 8);
        let mut expected = Buffer::with_lines([
            "P: ▸ 1        ",
            "n             ",
            "m ▸ 1         ",
            "S ▸ 1         ",
            " ▸ 1 ─────────",
            "f() ▸ 1       ",
            "- v           ",
            "              ",
        ]);
        let dim = Style::new().dim();
        expected.set_style(Rect::new(0, 0, 2, 1), Style::new().bold().fg(PRIMARY));
        expected.set_style(Rect::new(2, 0, 4, 1), dim);
        expected.set_style(Rect::new(0, 1, 1, 2), dim);
        expected.set_style(Rect::new(1, 2, 4, 1), dim);
        expected.set_style(Rect::new(0, 3, 1, 1), Style::new().bold().fg(SECONDARY));
        expected.set_style(Rect::new(1, 3, 4, 1), dim);
        expected.set_style(Rect::new(0, 4, 4, 1), dim);
        expected.set_style(Rect::new(5, 4, 9, 1), divider_style());
        expected.set_style(Rect::new(0, 5, 14, 1), code_style());
        expected.set_style(Rect::new(0, 5, 7, 1), code_style().dim());
        assert_eq!(buf, expected);
    }

    // FR-NAV-03
    #[test]
    fn breadcrumbs_start_at_home_and_drop_crumbs_from_the_left_to_fit() {
        let crumbs = ["Work".to_owned(), "Report".to_owned(), "Draft".to_owned()];
        let text = |width| breadcrumbs(&crumbs, width).to_string();
        assert_eq!(breadcrumbs(&[], 0).to_string(), "⌂");
        assert_eq!(text(25), "⌂ › Work › Report › Draft");
        assert_eq!(text(24), "… › Report › Draft");
        assert_eq!(text(17), "… › Draft");
        assert_eq!(text(3), "… › Draft", "the last crumb always stays");
        assert_eq!(breadcrumbs(&crumbs, 25).style, Style::new().bold());
    }

    // FR-NAV-03
    #[test]
    fn the_title_row_is_styled_by_its_type_in_bold_without_indent() {
        let title = |text: &str| {
            let mut state = State::new(format::parse(text), None);
            state.update(Message::FocusNext);
            state.update(Message::ZoomIn);
            draw(&state.rows(), 6, 3)
        };
        let mut expected = Buffer::with_lines(["n     ", "m     ", "- t   "]);
        expected.set_style(Rect::new(0, 0, 1, 2), Style::new().bold().dim());
        assert_eq!(title("P:\n\tn\n\tm\n\t\t- t"), expected, "a note is dim");
        let mut expected = Buffer::with_lines(["Q:    ", "- t   ", "      "]);
        expected.set_style(Rect::new(0, 0, 2, 1), Style::new().bold().fg(PRIMARY));
        assert_eq!(title("P:\n\tQ:\n\t\t- t"), expected, "a project is colored");
        let mut expected = Buffer::with_lines(["- t   ", "- u   ", "      "]);
        expected.set_style(Rect::new(2, 0, 1, 1), Style::new().bold());
        assert_eq!(
            title("P:\n\t- t\n\t\t- u"),
            expected,
            "a task has its marker"
        );
    }

    #[test]
    fn clips_rows_that_do_not_fit() {
        let expected = Buffer::with_lines(["- abc", "- d  "]);
        assert_eq!(render("- abcdef\n- d\n- e", 5, 2), expected);
    }

    // D-30
    #[test]
    fn marks_the_focused_row_in_the_gutter_without_highlighting_it() {
        let mut expected = Buffer::with_lines(["  - a", "▎ - b", "  - c"]);
        expected.set_style(Rect::new(0, 1, 1, 1), Style::new().fg(PRIMARY));
        assert_eq!(render_scrolled(0, Some(1), Rect::new(0, 0, 5, 3)), expected);
    }

    // D-30
    #[test]
    fn marks_every_line_of_a_multi_line_focused_block() {
        let rows = State::new(format::parse("- a\nn\nm"), None).rows();
        let mut buf = Buffer::empty(Rect::new(0, 0, 6, 3));
        OutlineView::new(&rows)
            .focused(Some(1))
            .render(buf.area, &mut buf);
        let mut expected = Buffer::with_lines(["  - a ", "▎ n   ", "▎ m   "]);
        expected.set_style(Rect::new(0, 1, 1, 2), Style::new().fg(PRIMARY));
        expected.set_style(Rect::new(2, 1, 1, 2), Style::new().dim());
        assert_eq!(buf, expected);
    }

    #[test]
    fn starts_at_the_offset_and_marks_relative_to_it() {
        let area = Rect::new(2, 1, 6, 2);
        let mut expected = Buffer::with_lines(["  - c ", "▎ - d "]);
        expected.area = area;
        expected.set_style(Rect::new(2, 2, 1, 1), Style::new().fg(PRIMARY));
        assert_eq!(render_scrolled(2, Some(3), area), expected);
    }

    #[test]
    fn focus_outside_the_view_is_not_marked() {
        let expected = Buffer::with_lines(["  - b ", "  - c "]);
        assert_eq!(render_scrolled(1, Some(0), Rect::new(0, 0, 6, 2)), expected);
        assert_eq!(render_scrolled(1, Some(3), Rect::new(0, 0, 6, 2)), expected);
        assert_eq!(render_scrolled(1, None, Rect::new(0, 0, 6, 2)), expected);
    }

    #[test]
    fn empty_outline_draws_nothing() {
        assert_eq!(render("", 4, 1), Buffer::with_lines(["    "]));
    }
}
