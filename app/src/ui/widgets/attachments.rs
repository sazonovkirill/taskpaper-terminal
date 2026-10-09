//! The attachments panel ([attachments.md §3](../../../../docs/specs/attachments.md#3-attachments-panel-fr-att-03),
//! FR-ATT-03): a box over the right side of the screen whose title names the block, a row of
//! tabs, one per kind of attachment turned on, the shown tab's content and a hint naming the
//! panel's keys.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Clear, Widget},
};

use super::attachment_icon;
use crate::{
    app::AttachmentsView,
    document::Attachment,
    ui::theme::{IconSet, Theme},
};

/// The narrowest and widest the panel is drawn, screen width allowing.
const WIDTH: (u16, u16) = (40, 72);

/// The attachments panel.
#[derive(Debug)]
pub struct AttachmentsPanel<'a> {
    view: &'a AttachmentsView<'a>,
    icons: IconSet,
    theme: &'a Theme,
}

impl<'a> AttachmentsPanel<'a> {
    /// The panel showing `view` in `theme` with `icons`.
    #[must_use]
    pub fn new(view: &'a AttachmentsView<'a>, icons: IconSet, theme: &'a Theme) -> Self {
        Self { view, icons, theme }
    }

    /// Where the panel goes over `body`, the screen above the status line: at its right, half
    /// as wide within 40 to 72 columns, from the panes' rule under their headers down.
    #[must_use]
    pub fn area(body: Rect) -> Rect {
        let width = (body.width / 2).clamp(WIDTH.0, WIDTH.1).min(body.width);
        Rect::new(
            body.right().saturating_sub(width),
            body.y.saturating_add(1),
            width,
            body.height.saturating_sub(1),
        )
    }

    /// The row of tabs, the shown one stood out in the accent color.
    fn tabs(&self) -> Line<'static> {
        let mut line = Line::default();
        for &tab in self.view.tabs {
            let label = format!(" {} {} ", attachment_icon(tab).glyph(self.icons), name(tab));
            line.push_span(if tab == self.view.tab {
                Span::styled(label, Style::new().fg(self.theme.accent).reversed())
            } else {
                Span::styled(label, Style::new().dim())
            });
            line.push_span(" ");
        }
        line
    }

    /// The hint naming the panel's keys.
    fn hint(&self) -> Line<'static> {
        let mut keys = Vec::new();
        if self.view.tabs.len() > 1 {
            keys.push("Tab other tab");
        }
        keys.push("Esc close");
        Line::from(keys.join(" · ")).dim()
    }
}

/// The name of the tab of kind `kind`.
fn name(kind: Attachment) -> &'static str {
    match kind {
        Attachment::Note => "Note",
        Attachment::Comments => "Comments",
    }
}

impl Widget for AttachmentsPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let accent = Style::new().fg(self.theme.accent);
        let frame = Block::bordered()
            .border_style(accent)
            .title_style(accent)
            .title(format!(" {} ", self.view.title));
        let inner = frame.inner(area);
        frame.render(area, buf);
        if inner.height == 0 {
            return;
        }
        self.tabs()
            .render(Rect::new(inner.x, inner.y, inner.width, 1), buf);
        let body = Rect::new(
            inner.x.saturating_add(1),
            inner.y.saturating_add(2),
            inner.width.saturating_sub(2),
            inner.height.saturating_sub(3),
        );
        if self.view.lines.is_empty() {
            let empty = match self.view.tab {
                Attachment::Note => "No note",
                Attachment::Comments => "No comments",
            };
            Line::from(empty).dim().render(body, buf);
        }
        for (y, line) in (body.y..body.bottom()).zip(&self.view.lines) {
            Line::from(line.as_str()).render(Rect::new(body.x, y, body.width, 1), buf);
        }
        if inner.height >= 3 {
            self.hint().render(
                Rect::new(body.x, inner.bottom().saturating_sub(1), body.width, 1),
                buf,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::super::tests::THEME;
    use super::*;

    /// Every row of the panel showing `view`, drawn over `width` × `height` cells.
    fn render(view: &AttachmentsView<'_>, width: u16, height: u16) -> Vec<String> {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        AttachmentsPanel::new(view, IconSet::Unicode, &THEME).render(area, &mut buf);
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect()
    }

    fn view(tabs: &[Attachment], tab: Attachment, lines: &[&str]) -> AttachmentsView<'static> {
        AttachmentsView {
            title: Arc::from("Milk"),
            tabs: Vec::leak(tabs.to_vec()),
            tab,
            lines: lines.iter().map(|&line| line.to_owned()).collect(),
        }
    }

    // FR-ATT-03, attachments.md §3
    #[test]
    fn the_panel_names_the_block_and_shows_its_tabs_content_and_keys() {
        let shown = view(&Attachment::ALL, Attachment::Note, &["# Title", "text"]);
        assert_eq!(
            render(&shown, 30, 8),
            [
                "┌ Milk ──────────────────────┐",
                "│ ≡ Note   ❞ Comments        │",
                "│                            │",
                "│ # Title                    │",
                "│ text                       │",
                "│                            │",
                "│ Tab other tab · Esc close  │",
                "└────────────────────────────┘",
            ]
        );
    }

    // FR-ATT-03, FR-ATT-06, attachments.md §3, §6
    #[test]
    fn an_empty_tab_says_so_and_one_tab_needs_no_tab_key() {
        let rows = render(
            &view(&[Attachment::Comments], Attachment::Comments, &[]),
            24,
            6,
        );
        assert_eq!(
            rows,
            [
                "┌ Milk ────────────────┐",
                "│ ❞ Comments           │",
                "│                      │",
                "│ No comments          │",
                "│ Esc close            │",
                "└──────────────────────┘",
            ]
        );
        assert_eq!(
            render(&view(&[Attachment::Note], Attachment::Note, &[]), 24, 6)[3],
            "│ No note              │"
        );
    }

    // FR-ATT-03
    #[test]
    fn the_shown_tab_stands_out() {
        let area = Rect::new(0, 0, 30, 6);
        let mut buf = Buffer::empty(area);
        let shown = view(&Attachment::ALL, Attachment::Comments, &[]);
        AttachmentsPanel::new(&shown, IconSet::Unicode, &THEME).render(area, &mut buf);
        assert!(
            !buf[(2, 1)]
                .modifier
                .contains(ratatui::style::Modifier::REVERSED)
        );
        assert!(buf[(2, 1)].modifier.contains(ratatui::style::Modifier::DIM));
        assert!(
            buf[(12, 1)]
                .modifier
                .contains(ratatui::style::Modifier::REVERSED)
        );
        assert_eq!(buf[(12, 1)].fg, THEME.accent);
    }

    // FR-ATT-03, attachments.md §3
    #[test]
    fn the_panel_takes_the_right_of_the_screen_below_the_headers() {
        assert_eq!(
            AttachmentsPanel::area(Rect::new(0, 0, 120, 30)),
            Rect::new(60, 1, 60, 29)
        );
        assert_eq!(
            AttachmentsPanel::area(Rect::new(0, 0, 200, 30)),
            Rect::new(128, 1, 72, 29)
        );
        assert_eq!(
            AttachmentsPanel::area(Rect::new(0, 0, 60, 30)),
            Rect::new(20, 1, 40, 29)
        );
        assert_eq!(
            AttachmentsPanel::area(Rect::new(0, 0, 30, 30)),
            Rect::new(0, 1, 30, 29)
        );
    }
}
