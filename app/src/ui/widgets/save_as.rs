//! The Save As prompt ([ui.md §1](../../../../docs/specs/ui.md#1-layout), FR-DOC-06): a centered
//! box at most 80 columns wide with the path typed and, below it, what `Enter` does or why
//! saving failed.

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Line,
    widgets::{Block, Clear, Widget},
};

use super::{Field, error_style, warning_style};
use crate::app::{SaveAsStatus, SaveAsView};

/// The prompt's size, the border included.
const SIZE: (u16, u16) = (80, 4);

/// The Save As prompt.
#[derive(Debug)]
pub struct SaveAsPrompt<'a> {
    view: &'a SaveAsView<'a>,
}

impl<'a> SaveAsPrompt<'a> {
    /// The prompt showing `view`.
    #[must_use]
    pub fn new(view: &'a SaveAsView<'a>) -> Self {
        Self { view }
    }

    /// Where the prompt goes in `area`: centered, as wide as it can be up to 80 columns.
    #[must_use]
    pub fn area(area: Rect) -> Rect {
        area.centered(Constraint::Length(SIZE.0), Constraint::Length(SIZE.1))
    }

    /// The caret's cell when the prompt is drawn over `area`: on the path's line.
    #[must_use]
    pub fn caret(&self, area: Rect) -> (u16, u16) {
        let inner = Block::bordered().inner(Self::area(area));
        let (_, column) = self.input(inner.width);
        let x = u16::try_from(column)
            .unwrap_or(u16::MAX)
            .min(inner.width.saturating_sub(1));
        (inner.x.saturating_add(x), inner.y)
    }

    /// The path after a space, `width` columns wide, and the caret's column on it.
    fn input(&self, width: u16) -> (Line<'a>, usize) {
        let field = Field {
            text: self.view.text,
            caret: self.view.caret,
            selection: self.view.selection.clone(),
        };
        let (spans, column) = field.spans(Style::new(), 1, usize::from(width));
        let mut line = Line::from(" ");
        line.spans.extend(spans);
        (line, column)
    }

    /// What the prompt is doing, in words.
    fn status(&self) -> Line<'static> {
        match self.view.status {
            SaveAsStatus::Typing => {
                Line::styled(" Tab completes a folder, Enter saves", Style::new().dim())
            }
            SaveAsStatus::Failed(reason) => {
                Line::styled(format!(" Cannot save: {reason}"), error_style())
            }
            SaveAsStatus::Replacing => Line::styled(
                " The file exists: Enter replaces it, Esc goes back",
                warning_style(),
            ),
            SaveAsStatus::Saving => Line::styled(" Saving…", Style::new().dim()),
        }
    }
}

impl Widget for SaveAsPrompt<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = Self::area(area);
        Clear.render(area, buf);
        let frame = Block::bordered().title(" Save As ");
        let inner = frame.inner(area);
        frame.render(area, buf);
        let [input, status] = inner.layout(&Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
        ]));
        self.input(input.width).0.render(input, buf);
        self.status().render(status, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The prompt's rows for `status` over a 40×6 area, with the path `~/a`.
    fn rows(status: SaveAsStatus<'_>) -> Vec<String> {
        let view = SaveAsView {
            text: "~/a",
            caret: 3,
            selection: None,
            status,
        };
        let area = Rect::new(0, 0, 40, 6);
        let mut buf = Buffer::empty(area);
        SaveAsPrompt::new(&view).render(area, &mut buf);
        (1..5)
            .map(|y| (0..40).map(|x| buf[(x, y)].symbol()).collect())
            .collect()
    }

    // FR-DOC-06, ui.md §1
    #[test]
    fn shows_the_path_over_what_the_prompt_is_doing() {
        assert_eq!(
            rows(SaveAsStatus::Typing),
            [
                "┌ Save As ─────────────────────────────┐",
                "│ ~/a                                  │",
                "│ Tab completes a folder, Enter saves  │",
                "└──────────────────────────────────────┘",
            ]
        );
        assert_eq!(
            rows(SaveAsStatus::Saving)[2],
            "│ Saving…                              │"
        );
        assert_eq!(
            rows(SaveAsStatus::Failed("denied"))[2],
            "│ Cannot save: denied                  │"
        );
        assert_eq!(
            rows(SaveAsStatus::Replacing)[2],
            "│ The file exists: Enter replaces it, E│"
        );
        let view = SaveAsView {
            text: "~/a",
            caret: 3,
            selection: None,
            status: SaveAsStatus::Typing,
        };
        assert_eq!(
            SaveAsPrompt::new(&view).caret(Rect::new(0, 0, 40, 6)),
            (5, 2)
        );
    }
}
