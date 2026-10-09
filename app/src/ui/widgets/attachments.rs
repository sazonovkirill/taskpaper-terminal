//! The attachments panel ([attachments.md §3](../../../../docs/specs/attachments.md#3-attachments-panel-fr-att-03),
//! FR-ATT-03): a box over the right side of the screen whose title names the block, a row of
//! tabs, one per kind of attachment turned on, the shown tab's content and a hint naming the
//! panel's keys.

use std::ops::Range;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Clear, Widget},
};

use super::{Field, attachment_icon, drawable, run_style, wrap};
use crate::{
    app::{AttachmentsView, CommentDraft, NoteCaret},
    document::{
        Attachment, comments,
        inline::{self, Format, Syntax},
        markdown::{self, NoteLine},
    },
    ui::theme::{IconSet, Theme},
};

/// The narrowest and widest the panel is drawn, screen width allowing.
const WIDTH: (u16, u16) = (40, 72);

/// The tab's content laid out in lines: the lines, those of the caret's line or the selected
/// comment, which scrolling keeps in view, and the caret's line and column, while text is typed.
#[derive(Debug, Default)]
struct Layout<'a> {
    lines: Vec<Line<'a>>,
    current: Option<Range<usize>>,
    caret: Option<(usize, usize)>,
}

/// The attachments panel.
#[derive(Debug)]
pub struct AttachmentsPanel<'a> {
    view: &'a AttachmentsView<'a>,
    icons: IconSet,
    theme: &'a Theme,
    scroll: usize,
}

impl<'a> AttachmentsPanel<'a> {
    /// The panel showing `view` in `theme` with `icons`, at the top of its tab.
    #[must_use]
    pub fn new(view: &'a AttachmentsView<'a>, icons: IconSet, theme: &'a Theme) -> Self {
        Self {
            view,
            icons,
            theme,
            scroll: 0,
        }
    }

    /// The panel with its tab scrolled `scroll` lines down.
    #[must_use]
    pub fn scrolled(self, scroll: usize) -> Self {
        Self { scroll, ..self }
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

    /// How far the tab scrolls when the panel is drawn over `area` after scrolling `scroll`
    /// lines down: no further than its last line at the bottom, and far enough to show the
    /// caret's line or the start of the selected comment; and how many lines a page scrolls.
    #[must_use]
    pub fn scroll(&self, scroll: usize, area: Rect) -> (usize, usize) {
        let body = Self::body(area);
        let height = usize::from(body.height).max(1);
        let layout = self.layout(body.width);
        let mut scroll = scroll.min(layout.lines.len().saturating_sub(height));
        if let Some(current) = layout.current {
            scroll = scroll
                .max(current.end.saturating_sub(height))
                .min(current.start);
        }
        (scroll, height)
    }

    /// The caret's cell when the panel is drawn over `area`, while text is typed and its line is
    /// in view.
    #[must_use]
    pub fn caret(&self, area: Rect) -> Option<(u16, u16)> {
        let body = Self::body(area);
        let (row, column) = self.layout(body.width).caret?;
        let y = row.checked_sub(self.scroll)?;
        (y < usize::from(body.height)).then(|| {
            (
                body.x
                    .saturating_add(u16::try_from(column).unwrap_or(u16::MAX))
                    .min(body.right().saturating_sub(1)),
                body.y.saturating_add(u16::try_from(y).unwrap_or(u16::MAX)),
            )
        })
    }

    /// The area of the tab's content when the panel is drawn over `area`: inside the frame,
    /// below the tabs and a blank line, above the hint, a column in from each side.
    fn body(area: Rect) -> Rect {
        let inner = Block::bordered().inner(area);
        Rect::new(
            inner.x.saturating_add(1),
            inner.y.saturating_add(2),
            inner.width.saturating_sub(2),
            inner.height.saturating_sub(3),
        )
    }

    /// The caret's line as a field.
    fn field(&self, caret: &NoteCaret) -> Field<'a> {
        Field {
            text: self.view.lines.get(caret.row).map_or("", String::as_str),
            caret: caret.caret,
            selection: caret.selection.clone(),
        }
    }

    /// The tab's content laid out `width` columns wide: the note rendered and wrapped, the
    /// comments, or the note being edited as written, its lines cut at the edge but the
    /// caret's, which scrolls to keep the caret in view.
    fn layout(&self, width: u16) -> Layout<'a> {
        let width = usize::from(width);
        if let Some(caret) = &self.view.editing {
            let (field, column) = self.field(caret).spans(Style::new(), 0, width);
            let mut lines: Vec<Line<'a>> = self
                .view
                .lines
                .iter()
                .map(|line| wrap::truncate(&Line::from(drawable(line)), width))
                .collect();
            if let Some(line) = lines.get_mut(caret.row) {
                *line = Line::from(field);
            }
            return Layout {
                lines,
                current: Some(caret.row..caret.row + 1),
                caret: Some((caret.row, column)),
            };
        }
        match self.view.tab {
            Attachment::Note => Layout {
                lines: markdown::note_lines(&self.view.lines)
                    .into_iter()
                    .flat_map(|line| self.note_line(line, width))
                    .collect(),
                ..Layout::default()
            },
            Attachment::Comments => self.comments(width),
        }
    }

    /// The comments laid out `width` columns wide, newest first as the file has them: the
    /// selected one, or the one being typed, on the focused block's band, a new one above the
    /// others (attachments.md §5).
    fn comments(&self, width: usize) -> Layout<'a> {
        let draft = self.view.draft.as_ref();
        let mut layout = Layout::default();
        let typed = |layout: &mut Layout<'a>, draft: &'a CommentDraft| {
            let start = layout.lines.len();
            let (line, column) = self.draft_line(draft, width);
            layout.lines.push(line);
            (layout.current, layout.caret) = (Some(start..start + 1), Some((start, column)));
        };
        if let Some(draft) = draft.filter(|draft| draft.comment.is_none()) {
            typed(&mut layout, draft);
        }
        for (at, line) in self.view.lines.iter().enumerate() {
            match draft {
                Some(draft) if draft.comment == Some(at) => typed(&mut layout, draft),
                _ => {
                    let start = layout.lines.len();
                    layout.lines.extend(self.comment_line(line, width));
                    if draft.is_none() && self.view.selected == Some(at) {
                        layout.current = Some(start..layout.lines.len());
                    }
                }
            }
        }
        let band = Style::new().bg(self.theme.current_line);
        for line in layout
            .current
            .clone()
            .and_then(|current| layout.lines.get_mut(current))
            .into_iter()
            .flatten()
        {
            line.style = line.style.patch(band);
        }
        layout
    }

    /// A comment wrapped to `width` columns: its timestamp dimmed, without its brackets, and its
    /// text with its inline formatting, the lines after the first under the text.
    fn comment_line(&self, line: &str, width: usize) -> Vec<Line<'static>> {
        let comment = comments::comment(line);
        let mut shown = Line::default();
        if let Some(stamp) = comment.stamp {
            shown.push_span(Span::styled(format!("{stamp} "), Style::new().dim()));
        }
        let start = shown.width();
        for run in inline::runs(comment.text, Syntax::Inline) {
            shown.push_span(Span::styled(
                drawable(run.text).into_owned(),
                run_style(run.format, Style::new(), self.theme.link),
            ));
        }
        wrap::wrap(&shown, start, width, width)
    }

    /// The comment being typed on a line `width` columns wide after a dimmed marker, and the
    /// caret's column.
    fn draft_line(&self, draft: &'a CommentDraft, width: usize) -> (Line<'a>, usize) {
        let marker = if self.icons == IconSet::Ascii {
            "> "
        } else {
            "› "
        };
        let field = Field {
            text: &draft.text,
            caret: draft.caret,
            selection: draft.selection.clone(),
        };
        let start = Span::raw(marker).width();
        let (spans, column) = field.spans(Style::new(), start, width);
        let mut line = Line::from(Span::styled(marker, Style::new().dim()));
        line.spans.extend(spans);
        (line, column)
    }

    /// A line of a note as Markdown shows it, wrapped to `width` columns: a heading bold, the
    /// first level also underlined; a list item after its marker, a bullet for `-`, `*` or `+`;
    /// a quote dimmed after a bar; code dimmed as typed; each with its inline formatting.
    fn note_line(&self, line: NoteLine<'_>, width: usize) -> Vec<Line<'static>> {
        let link_color = self.theme.link;
        let formatted = |text: &str, format: Format| -> Vec<Span<'static>> {
            inline::runs(text, Syntax::Note)
                .into_iter()
                .map(|run| {
                    let format = Format {
                        heading: format.heading,
                        ..run.format
                    };
                    Span::styled(
                        drawable(run.text).into_owned(),
                        run_style(format, Style::new(), link_color),
                    )
                })
                .collect()
        };
        let (prefix, spans) = match line {
            NoteLine::Heading(level, text) => (
                String::new(),
                formatted(
                    text,
                    Format {
                        heading: Some(level),
                        ..Format::default()
                    },
                ),
            ),
            NoteLine::Item {
                depth,
                marker,
                text,
            } => {
                let bullet = matches!(marker, "-" | "*" | "+") && self.icons != IconSet::Ascii;
                let marker = if bullet { "•" } else { marker };
                (
                    format!("{}{marker} ", "  ".repeat(depth)),
                    formatted(text, Format::default()),
                )
            }
            NoteLine::Quote(text) => (
                "│ ".to_owned(),
                formatted(text, Format::default())
                    .into_iter()
                    .map(Stylize::dim)
                    .collect(),
            ),
            NoteLine::Code(text) => (
                String::new(),
                vec![Span::styled(
                    drawable(text).into_owned(),
                    Style::new().dim(),
                )],
            ),
            NoteLine::Text(text) => (String::new(), formatted(text, Format::default())),
        };
        let start = Span::raw(&prefix).width();
        let mut line = Line::from(Span::styled(prefix, Style::new().dim()));
        line.spans.extend(spans);
        wrap::wrap(&line, start, width, width)
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
        if self.view.editing.is_some() {
            return Line::from("Enter new line · Esc save").dim();
        }
        if let Some(draft) = &self.view.draft {
            let accept = if draft.comment.is_some() {
                "Enter save"
            } else {
                "Enter add"
            };
            return Line::from(format!("{accept} · Esc cancel")).dim();
        }
        let mut keys = Vec::new();
        let selected = self.view.selected.is_some();
        if self.view.tabs.len() > 1 && !selected {
            keys.push("Tab other tab");
        }
        match self.view.tab {
            Attachment::Note => keys.push("e edit"),
            Attachment::Comments if selected => keys.extend(["a add", "e edit", "d del"]),
            Attachment::Comments => keys.push("a add"),
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
        let body = Self::body(area);
        let lines = self.layout(body.width).lines;
        if lines.is_empty() {
            let empty = match self.view.tab {
                Attachment::Note => "No note",
                Attachment::Comments => "No comments",
            };
            Line::from(empty).dim().render(body, buf);
        }
        for (y, line) in (body.y..body.bottom()).zip(lines.iter().skip(self.scroll)) {
            line.render(Rect::new(body.x, y, body.width, 1), buf);
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

    use ratatui::style::Modifier;

    use super::super::tests::THEME;
    use super::*;

    /// The panel showing `view` drawn over `width` × `height` cells with `icons`, scrolled
    /// `scroll` lines as the frontend would, and the caret's cell.
    fn draw(
        view: &AttachmentsView<'_>,
        (width, height): (u16, u16),
        icons: IconSet,
        scroll: usize,
    ) -> (Buffer, Option<(u16, u16)>) {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        let panel = AttachmentsPanel::new(view, icons, &THEME);
        let (scroll, _) = panel.scroll(scroll, area);
        let panel = panel.scrolled(scroll);
        let caret = panel.caret(area);
        panel.render(area, &mut buf);
        (buf, caret)
    }

    /// Every row of `buf`, without trailing spaces.
    fn rows(buf: &Buffer) -> Vec<String> {
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect()
    }

    /// Every row of the panel showing `view`, drawn over `width` × `height` cells.
    fn render(view: &AttachmentsView<'_>, width: u16, height: u16) -> Vec<String> {
        rows(&draw(view, (width, height), IconSet::Unicode, 0).0)
    }

    fn view(tabs: &[Attachment], tab: Attachment, lines: &[&str]) -> AttachmentsView<'static> {
        AttachmentsView {
            title: Arc::from("Milk"),
            tabs: Vec::leak(tabs.to_vec()),
            tab,
            lines: lines.iter().map(|&line| line.to_owned()).collect(),
            editing: None,
            selected: None,
            draft: None,
        }
    }

    /// The note `lines` being edited with the caret at byte `caret` of line `row`.
    fn editing(lines: &[&str], row: usize, caret: usize) -> AttachmentsView<'static> {
        AttachmentsView {
            editing: Some(NoteCaret {
                row,
                caret,
                selection: None,
            }),
            ..view(&Attachment::ALL, Attachment::Note, lines)
        }
    }

    // FR-ATT-03, attachments.md §3
    #[test]
    fn the_panel_names_the_block_and_shows_its_tabs_content_and_keys() {
        let shown = view(&Attachment::ALL, Attachment::Comments, &["[1] a", "b"]);
        assert_eq!(
            render(&shown, 30, 8),
            [
                "┌ Milk ──────────────────────┐",
                "│ ≡ Note   ❞ Comments        │",
                "│                            │",
                "│ [1] a                      │",
                "│ b                          │",
                "│                            │",
                "│ Tab other tab · a add · Es │",
                "└────────────────────────────┘",
            ]
        );
    }

    // FR-ATT-04, attachments.md §4
    #[test]
    fn the_note_tab_renders_markdown_wrapped_to_the_panel() {
        let note = view(
            &Attachment::ALL,
            Attachment::Note,
            &[
                "# Title",
                "Some *soft* and **bold** words to wrap",
                "",
                "- one",
                "  * two",
                "3. three",
                "> quoted",
                "```",
                "  code *x*",
                "```",
            ],
        );
        let (buf, caret) = draw(&note, (30, 16), IconSet::Unicode, 0);
        assert_eq!(
            rows(&buf),
            [
                "┌ Milk ──────────────────────┐",
                "│ ≡ Note   ❞ Comments        │",
                "│                            │",
                "│ Title                      │",
                "│ Some soft and bold words   │",
                "│ to wrap                    │",
                "│                            │",
                "│ • one                      │",
                "│   • two                    │",
                "│ 3. three                   │",
                "│ │ quoted                   │",
                "│   code *x*                 │",
                "│                            │",
                "│                            │",
                "│ Tab other tab · e edit · E │",
                "└────────────────────────────┘",
            ]
        );
        assert_eq!(caret, None);
        let has = |x: u16, y: u16, modifier| buf[(x, y)].modifier.contains(modifier);
        assert!(
            has(2, 3, Modifier::BOLD) && has(2, 3, Modifier::UNDERLINED),
            "the heading"
        );
        assert!(has(7, 4, Modifier::ITALIC) && !has(2, 4, Modifier::ITALIC));
        assert!(has(16, 4, Modifier::BOLD));
        assert!(
            has(4, 10, Modifier::DIM) && has(2, 10, Modifier::DIM),
            "the quote"
        );
        assert!(has(4, 11, Modifier::DIM), "the code");
        assert!(has(2, 7, Modifier::DIM), "the bullet");
        let ascii = rows(&draw(&note, (30, 16), IconSet::Ascii, 0).0);
        assert_eq!(ascii[7], "│ - one                      │");
        assert_eq!(ascii[8], "│   * two                    │");
    }

    // FR-ATT-04, attachments.md §4, D-107
    #[test]
    fn the_tab_scrolls_no_further_than_its_end() {
        let lines: Vec<String> = (1..=9).map(|n| format!("line {n}")).collect();
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        let note = view(&[Attachment::Note], Attachment::Note, &lines);
        let area = Rect::new(0, 0, 24, 8);
        let panel = AttachmentsPanel::new(&note, IconSet::Unicode, &THEME);
        assert_eq!(panel.scroll(2, area), (2, 3));
        assert_eq!(panel.scroll(usize::MAX, area), (6, 3));
        let shown = rows(&draw(&note, (24, 8), IconSet::Unicode, usize::MAX).0);
        assert_eq!(
            shown[3..6],
            [
                "│ line 7               │",
                "│ line 8               │",
                "│ line 9               │"
            ]
        );
        let short = view(&[Attachment::Note], Attachment::Note, &["a"]);
        assert_eq!(
            AttachmentsPanel::new(&short, IconSet::Unicode, &THEME).scroll(5, area),
            (0, 3)
        );
        let flat = Rect::new(0, 0, 24, 4);
        assert_eq!(
            AttachmentsPanel::new(&short, IconSet::Unicode, &THEME)
                .scroll(0, flat)
                .1,
            1,
            "a page is a line at least"
        );
    }

    // FR-ATT-04, attachments.md §4
    #[test]
    fn an_edited_note_shows_as_written_with_its_caret_in_view() {
        let text = [
            "# Title",
            "a long line that goes past the edge",
            "b",
            "c",
            "d",
        ];
        let (buf, caret) = draw(&editing(&text, 4, 1), (24, 8), IconSet::Unicode, 0);
        assert_eq!(
            rows(&buf)[3..7],
            [
                "│ b                    │",
                "│ c                    │",
                "│ d                    │",
                "│ Enter new line · Esc │",
            ]
        );
        assert_eq!(caret, Some((3, 5)));
        let (buf, caret) = draw(&editing(&text, 1, 35), (24, 8), IconSet::Unicode, 0);
        assert_eq!(rows(&buf)[4], "│  goes past the edge  │");
        assert_eq!(caret, Some((21, 4)), "the caret's line scrolls to it");
        let (buf, caret) = draw(&editing(&text, 0, 0), (24, 8), IconSet::Unicode, 4);
        assert_eq!(
            rows(&buf)[3..5],
            ["│ # Title              │", "│ a long line that go… │"]
        );
        assert_eq!(caret, Some((2, 3)));
        let area = Rect::new(0, 0, 24, 8);
        let view = editing(&text, 0, 0);
        let panel = AttachmentsPanel::new(&view, IconSet::Unicode, &THEME).scrolled(1);
        assert_eq!(panel.caret(area), None, "the caret's line is out of view");
        let (_, caret) = draw(&editing(&text, 0, 0), (24, 3), IconSet::Unicode, 0);
        assert_eq!(caret, None, "no room for the note");
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
                "│ a add · Esc close    │",
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

    /// The comments `lines` with comment `selected` selected and `draft` being typed.
    fn comments(
        lines: &[&str],
        selected: Option<usize>,
        draft: Option<CommentDraft>,
    ) -> AttachmentsView<'static> {
        AttachmentsView {
            selected,
            draft,
            ..view(&Attachment::ALL, Attachment::Comments, lines)
        }
    }

    fn draft(comment: Option<usize>, text: &str) -> CommentDraft {
        CommentDraft {
            comment,
            text: text.to_owned(),
            caret: text.len(),
            selection: None,
        }
    }

    const COMMENTS: [&str; 3] = [
        "[2026-10-08 09:05] Bought **it** at the corner shop",
        "by hand",
        "[2026-10-07 18:02] a",
    ];

    // FR-ATT-05, attachments.md §5
    #[test]
    fn comments_show_their_dimmed_timestamps_and_the_selected_one_on_a_band() {
        let (buf, caret) = draw(
            &comments(&COMMENTS, Some(1), None),
            (32, 9),
            IconSet::Unicode,
            0,
        );
        assert_eq!(
            rows(&buf)[3..8],
            [
                "│ 2026-10-08 09:05 Bought it   │",
                "│                  at the      │",
                "│                  corner shop │",
                "│ by hand                      │",
                "│ a add · e edit · d del · Esc │",
            ]
        );
        assert_eq!(caret, None);
        assert!(buf[(2, 3)].modifier.contains(Modifier::DIM), "the stamp");
        assert!(!buf[(19, 3)].modifier.contains(Modifier::DIM));
        assert!(
            buf[(26, 3)].modifier.contains(Modifier::BOLD),
            "inline Markdown"
        );
        assert_eq!(buf[(2, 6)].bg, THEME.current_line, "the selected comment");
        assert_eq!(buf[(29, 6)].bg, THEME.current_line, "the whole width");
        assert_ne!(buf[(2, 3)].bg, THEME.current_line);
        let (buf, _) = draw(
            &comments(&COMMENTS, Some(2), None),
            (32, 9),
            IconSet::Unicode,
            0,
        );
        assert_eq!(
            rows(&buf)[3..7],
            [
                "│                  at the      │",
                "│                  corner shop │",
                "│ by hand                      │",
                "│ 2026-10-07 18:02 a           │",
            ],
            "the selected comment is kept in view"
        );
        let (buf, _) = draw(
            &comments(&COMMENTS, Some(0), None),
            (32, 9),
            IconSet::Unicode,
            usize::MAX,
        );
        assert_eq!(
            rows(&buf)[3],
            "│ 2026-10-08 09:05 Bought it   │",
            "from its start"
        );
    }

    // FR-ATT-05, attachments.md §5
    #[test]
    fn a_comment_being_typed_shows_with_its_caret_where_it_goes() {
        let (buf, caret) = draw(
            &comments(&COMMENTS[1..], Some(0), Some(draft(None, "new"))),
            (32, 8),
            IconSet::Unicode,
            0,
        );
        assert_eq!(
            rows(&buf)[3..7],
            [
                "│ › new                        │",
                "│ by hand                      │",
                "│ 2026-10-07 18:02 a           │",
                "│ Enter add · Esc cancel       │",
            ]
        );
        assert_eq!(caret, Some((7, 3)));
        assert_eq!(buf[(2, 3)].bg, THEME.current_line);
        assert_ne!(buf[(2, 4)].bg, THEME.current_line, "not the selected one");
        let (buf, caret) = draw(
            &comments(&COMMENTS[1..], Some(0), Some(draft(Some(1), "b"))),
            (32, 8),
            IconSet::Ascii,
            0,
        );
        assert_eq!(
            rows(&buf)[3..7],
            [
                "│ by hand                      │",
                "│ > b                          │",
                "│                              │",
                "│ Enter save · Esc cancel      │",
            ]
        );
        assert_eq!(caret, Some((5, 4)));
        let (_, caret) = draw(
            &comments(&COMMENTS[1..], Some(0), Some(draft(Some(1), "b"))),
            (32, 4),
            IconSet::Ascii,
            0,
        );
        assert_eq!(caret, None, "no room for the comments");
    }
}
