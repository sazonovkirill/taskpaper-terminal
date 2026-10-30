//! The Quick Comment prompt ([ui.md §1](../../../../docs/specs/ui.md#1-layout),
//! [attachments.md §5](../../../../docs/specs/attachments.md#5-comments-fr-att-05), FR-ATT-07):
//! one line across the bottom of the screen, placed as Quick Add's panel, titled with the block
//! the comment goes to.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{Block, Clear, Widget},
};

use super::{Caret, Field, drawable};
use crate::{app::QuickCommentView, ui::theme::Theme};

/// The Quick Comment prompt.
#[derive(Debug)]
pub struct QuickCommentPanel<'a> {
    view: &'a QuickCommentView<'a>,
    theme: &'a Theme,
    scroll: usize,
}

impl<'a> QuickCommentPanel<'a> {
    /// The prompt's height, the border included.
    pub const HEIGHT: u16 = 3;

    /// The prompt showing `view` in `theme`.
    #[must_use]
    pub fn new(view: &'a QuickCommentView<'a>, theme: &'a Theme) -> Self {
        Self {
            view,
            theme,
            scroll: 0,
        }
    }

    /// The prompt with its text scrolled on from `scroll` columns, as far as the last frame
    /// scrolled it (editing.md §2).
    #[must_use]
    pub fn text_scroll(self, scroll: usize) -> Self {
        Self { scroll, ..self }
    }

    /// The caret's cell when the prompt is drawn over `area`, on the text's line, the first
    /// inside the border, and how far the text is scrolled.
    #[must_use]
    pub fn caret(&self, area: Rect) -> ((u16, u16), usize) {
        let inner = Block::bordered().inner(area);
        let (_, caret) = self.input(inner.width);
        let x = u16::try_from(caret.column)
            .unwrap_or(u16::MAX)
            .min(inner.width.saturating_sub(1));
        ((inner.x.saturating_add(x), inner.y), caret.scroll)
    }

    /// The text after a space, `width` columns wide, and where the caret is on it.
    fn input(&self, width: u16) -> (Line<'a>, Caret) {
        let field = Field {
            text: self.view.text,
            caret: self.view.caret,
            selection: self.view.selection.clone(),
            scroll: self.scroll,
        };
        let (spans, caret) = field.spans(Style::new(), 1, usize::from(width));
        let mut line = Line::from(" ");
        line.spans.extend(spans);
        (line, caret)
    }
}

impl Widget for QuickCommentPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let accent = Style::new().fg(self.theme.accent);
        let frame = Block::bordered()
            .border_style(accent)
            .title_style(accent)
            .title(format!(" Comment on {} ", drawable(&self.view.title)));
        let inner = frame.inner(area);
        frame.render(area, buf);
        self.input(inner.width).0.render(inner, buf);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::style::Modifier;

    use super::super::tests::THEME;
    use super::*;

    fn view(text: &str) -> QuickCommentView<'_> {
        QuickCommentView {
            title: "Call\tthe bank".into(),
            text,
            caret: text.len(),
            selection: None,
        }
    }

    fn render(view: &QuickCommentView<'_>) -> Buffer {
        let area = Rect::new(0, 0, 30, QuickCommentPanel::HEIGHT);
        let mut buf = Buffer::empty(area);
        QuickCommentPanel::new(view, &THEME).render(area, &mut buf);
        buf
    }

    fn rows(buf: &Buffer) -> Vec<String> {
        (0..buf.area.height)
            .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect())
            .collect()
    }

    // FR-ATT-07
    #[test]
    fn the_prompt_is_one_line_titled_with_the_block_in_the_accent_color() {
        let buf = render(&view("Called back"));
        assert_eq!(
            rows(&buf),
            [
                "┌ Comment on Call the bank ──┐",
                "│ Called back                │",
                "└────────────────────────────┘",
            ]
        );
        assert_eq!(buf[(2, 0)].fg, THEME.accent, "the title");
        assert_eq!(buf[(0, 1)].fg, THEME.accent, "the border");
    }

    // FR-ATT-07, editing.md §2
    #[test]
    fn the_selection_is_reversed_and_the_caret_scrolls_the_text_into_view() {
        let selected = QuickCommentView {
            selection: Some(0..6),
            ..view("Called back")
        };
        let buf = render(&selected);
        assert!(buf[(2, 1)].modifier.contains(Modifier::REVERSED));
        assert!(!buf[(9, 1)].modifier.contains(Modifier::REVERSED));
        let prompt = QuickCommentPanel::new(&selected, &THEME);
        assert_eq!(prompt.caret(Rect::new(0, 10, 30, 3)), ((13, 11), 0));
        let long = "x".repeat(40);
        let long = view(&long);
        let ((x, y), scroll) = QuickCommentPanel::new(&long, &THEME).caret(Rect::new(0, 0, 30, 3));
        assert_eq!((x, y), (28, 1), "the last cell inside the border");
        assert!(scroll > 0, "the text scrolls to keep the caret in view");
        let scrolled = QuickCommentPanel::new(&long, &THEME).text_scroll(scroll);
        assert_eq!(scrolled.caret(Rect::new(0, 0, 30, 3)), ((28, 1), scroll));
        let middle = "x".repeat(40);
        let middle = QuickCommentView {
            caret: 20,
            ..view(&middle)
        };
        let kept = QuickCommentPanel::new(&middle, &THEME).text_scroll(10);
        assert_eq!(
            kept.caret(Rect::new(0, 0, 30, 3)),
            ((12, 1), 10),
            "the last frame's scroll stays while the caret is in view"
        );
    }
}
