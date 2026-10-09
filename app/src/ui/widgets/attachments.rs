//! The attachments panel ([attachments.md §3](../../../../docs/specs/attachments.md#3-attachments-panel-fr-att-03),
//! FR-ATT-03): a box over the right side of the screen, from top to bottom, whose title names
//! the block, with a section per kind of attachment turned on, the note and then the comments,
//! and a hint naming the panel's keys.

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
    app::{AttachmentsView, CommentDraft, NoteCaret, PanelItem},
    document::{
        Attachment, comments,
        inline::{self, Format, Syntax},
        markdown::{self, NoteLine},
    },
    ui::theme::{IconSet, Theme},
};

/// The narrowest and widest the panel is drawn, screen width allowing.
const WIDTH: (u16, u16) = (40, 72);

/// The panel's content laid out in lines: the lines; those of the selected item, the caret's line
/// or the comment being typed, which scrolling keeps in view; those on the focused block's band;
/// and the caret's line and column, while text is typed.
#[derive(Debug, Default)]
struct Layout<'a> {
    lines: Vec<Line<'a>>,
    current: Option<Range<usize>>,
    band: Option<Range<usize>>,
    caret: Option<(usize, usize)>,
}

/// How the panel's content scrolls at a draw: the first line shown, the lines a page holds and
/// those of the selected item, the caret's line or the comment being typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scrolled {
    /// The first line shown.
    pub scroll: usize,
    /// How many lines a page holds, one at least.
    pub page: usize,
    /// The lines of what is selected or typed, if anything.
    pub current: Option<Range<usize>>,
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
    /// The panel showing `view` in `theme` with `icons`, at the top of its content.
    #[must_use]
    pub fn new(view: &'a AttachmentsView<'a>, icons: IconSet, theme: &'a Theme) -> Self {
        Self {
            view,
            icons,
            theme,
            scroll: 0,
        }
    }

    /// The panel with its content scrolled `scroll` lines down.
    #[must_use]
    pub fn scrolled(self, scroll: usize) -> Self {
        Self { scroll, ..self }
    }

    /// Where the panel goes over `body`, the screen above the status line: at its right, half
    /// as wide within 40 to 72 columns, its whole height.
    #[must_use]
    pub fn area(body: Rect) -> Rect {
        let width = (body.width / 2).clamp(WIDTH.0, WIDTH.1).min(body.width);
        Rect::new(
            body.right().saturating_sub(width),
            body.y,
            width,
            body.height,
        )
    }

    /// How the content scrolls when the panel is drawn over `area` after scrolling `scroll`
    /// lines down: no further than its last line at the bottom, and, when `follow` says the
    /// selection moved or while text is typed, far enough to show the start of the selected item
    /// and as much of it as fits, or the caret's line.
    #[must_use]
    pub fn scroll(&self, scroll: usize, area: Rect, follow: bool) -> Scrolled {
        let body = Self::body(area);
        let page = usize::from(body.height).max(1);
        let layout = self.layout(body.width);
        let mut scroll = scroll.min(layout.lines.len().saturating_sub(page));
        let typing = self.view.editing.is_some() || self.view.draft.is_some();
        if let Some(current) = &layout.current
            && (follow || typing)
        {
            scroll = scroll
                .max(current.end.saturating_sub(page))
                .min(current.start);
        }
        Scrolled {
            scroll,
            page,
            current: layout.current,
        }
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

    /// The area of the content when the panel is drawn over `area`: inside the frame, above a
    /// blank line and the hint, a column in from each side.
    fn body(area: Rect) -> Rect {
        let inner = Block::bordered().inner(area);
        Rect::new(
            inner.x.saturating_add(1),
            inner.y,
            inner.width.saturating_sub(2),
            inner.height.saturating_sub(2),
        )
    }

    /// The content laid out `width` columns wide: each section, a blank line between them, with
    /// the selected item on the focused block's band while nothing is typed.
    fn layout(&self, width: u16) -> Layout<'a> {
        let width = usize::from(width);
        let mut layout = Layout::default();
        for (at, &kind) in self.view.sections.iter().enumerate() {
            if at > 0 {
                layout.lines.push(Line::default());
            }
            layout.lines.push(self.heading(kind));
            match kind {
                Attachment::Note => self.note(&mut layout, width),
                Attachment::Comments => self.comments(&mut layout, width),
            }
        }
        let band = Style::new().bg(self.theme.current_line);
        for line in layout
            .band
            .clone()
            .and_then(|lines| layout.lines.get_mut(lines))
            .into_iter()
            .flatten()
        {
            line.style = line.style.patch(band);
        }
        layout
    }

    /// A section's heading: its icon and name in the headers color, bold, at the right (D-110).
    fn heading(&self, kind: Attachment) -> Line<'a> {
        let name = match kind {
            Attachment::Note => "Note",
            Attachment::Comments => "Comments",
        };
        Line::styled(
            format!("{} {name}", attachment_icon(kind).glyph(self.icons)),
            Style::new().fg(self.theme.headers).bold(),
        )
        .right_aligned()
    }

    /// Lays out the note below its heading: rendered and wrapped, or while it is edited as
    /// written, its lines cut at the edge but the caret's, which scrolls to keep the caret in
    /// view; selected, the heading and the note are one item.
    fn note(&self, layout: &mut Layout<'a>, width: usize) {
        let heading = layout.lines.len() - 1;
        let start = layout.lines.len();
        if let Some(caret) = &self.view.editing {
            let (field, column) = self.field(caret).spans(Style::new(), 0, width);
            layout
                .lines
                .extend(self.view.note.iter().enumerate().map(|(row, line)| {
                    if row == caret.row {
                        Line::from(field.clone())
                    } else {
                        wrap::truncate(&Line::from(drawable(line)), width)
                    }
                }));
            let row = start + caret.row;
            (layout.current, layout.caret) = (Some(row..row + 1), Some((row, column)));
            return;
        }
        layout.lines.extend(
            markdown::note_lines(&self.view.note)
                .into_iter()
                .flat_map(|line| self.note_line(line, width)),
        );
        if layout.lines.len() == start {
            layout.lines.push(Line::from("No note").dim());
        }
        if self.view.selected == Some(PanelItem::Note) {
            layout.current = Some(heading..layout.lines.len());
            layout.band.clone_from(&layout.current);
        }
    }

    /// Lays out the comments below their heading, newest first as the file has them: the
    /// selected one, or the one being typed, on the band, a new one above the others
    /// (attachments.md §5).
    fn comments(&self, layout: &mut Layout<'a>, width: usize) {
        let draft = self.view.draft.as_ref();
        let typed = |layout: &mut Layout<'a>, draft: &'a CommentDraft| {
            let start = layout.lines.len();
            let (line, column) = self.draft_line(draft, width);
            layout.lines.push(line);
            layout.current = Some(start..start + 1);
            layout.band.clone_from(&layout.current);
            layout.caret = Some((start, column));
        };
        if let Some(draft) = draft.filter(|draft| draft.comment.is_none()) {
            typed(layout, draft);
        } else if self.view.comments.is_empty() {
            layout.lines.push(Line::from("No comments").dim());
        }
        for (at, line) in self.view.comments.iter().enumerate() {
            match draft {
                Some(draft) if draft.comment == Some(at) => typed(layout, draft),
                _ => {
                    let start = layout.lines.len();
                    layout.lines.extend(self.comment_line(line, width));
                    if draft.is_none() && self.view.selected == Some(PanelItem::Comment(at)) {
                        layout.current = Some(start..layout.lines.len());
                        layout.band.clone_from(&layout.current);
                    }
                }
            }
        }
    }

    /// The caret's line of the note being edited as a field.
    fn field(&self, caret: &NoteCaret) -> Field<'a> {
        Field {
            text: self.view.note.get(caret.row).map_or("", String::as_str),
            caret: caret.caret,
            selection: caret.selection.clone(),
        }
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

    /// The hint naming the keys of what is selected or typed.
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
        let comments = self.view.sections.contains(&Attachment::Comments);
        let keys: &[&str] = match self.view.selected {
            Some(PanelItem::Note) if comments => &["e edit", "a comment", "Esc close"],
            Some(PanelItem::Note) => &["e edit", "Esc close"],
            Some(PanelItem::Comment(_)) => &["a add", "e edit", "d del", "Esc close"],
            None if comments => &["a add", "Esc close"],
            None => &["Esc close"],
        };
        Line::from(keys.join(" · ")).dim()
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
        let body = Self::body(area);
        let lines = self.layout(body.width).lines;
        for (y, line) in (body.y..body.bottom()).zip(lines.iter().skip(self.scroll)) {
            line.render(Rect::new(body.x, y, body.width, 1), buf);
        }
        if inner.height >= 2 {
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
    /// `scroll` lines as the frontend would on the first draw, and the caret's cell.
    fn draw(
        view: &AttachmentsView<'_>,
        (width, height): (u16, u16),
        icons: IconSet,
        scroll: usize,
    ) -> (Buffer, Option<(u16, u16)>) {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        let panel = AttachmentsPanel::new(view, icons, &THEME);
        let scrolled = panel.scroll(scroll, area, true);
        let panel = panel.scrolled(scrolled.scroll);
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

    /// Both sections, the note `note` and the comments `comments`, `selected` selected.
    fn view(
        note: &[&str],
        comments: &[&str],
        selected: Option<PanelItem>,
    ) -> AttachmentsView<'static> {
        let owned = |lines: &[&str]| lines.iter().map(|&line| line.to_owned()).collect();
        AttachmentsView {
            title: Arc::from("Milk"),
            sections: &Attachment::ALL,
            note: owned(note),
            comments: owned(comments),
            editing: None,
            selected,
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
            ..view(lines, &[], Some(PanelItem::Note))
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

    // FR-ATT-03, attachments.md §3
    #[test]
    fn the_panel_names_the_block_and_shows_the_note_then_the_comments_and_the_keys() {
        let shown = view(
            &["Full fat"],
            &["[2026-10-08 09:05] ok", "b"],
            Some(PanelItem::Note),
        );
        let (buf, _) = draw(&shown, (30, 11), IconSet::Unicode, 0);
        assert_eq!(
            rows(&buf),
            [
                "┌ Milk ──────────────────────┐",
                "│                     ≡ Note │",
                "│ Full fat                   │",
                "│                            │",
                "│                 ❞ Comments │",
                "│ 2026-10-08 09:05 ok        │",
                "│ b                          │",
                "│                            │",
                "│                            │",
                "│ e edit · a comment · Esc c │",
                "└────────────────────────────┘",
            ]
        );
        assert_eq!(buf[(2, 1)].fg, THEME.headers);
        assert!(buf[(2, 1)].modifier.contains(Modifier::BOLD), "a heading");
        assert_eq!(buf[(2, 2)].bg, THEME.current_line, "the selected note");
        assert_eq!(buf[(2, 1)].bg, THEME.current_line, "with its heading");
        assert_ne!(buf[(2, 3)].bg, THEME.current_line);
        let only = AttachmentsView {
            sections: &[Attachment::Note],
            ..view(&[], &[], Some(PanelItem::Note))
        };
        assert_eq!(
            render(&only, 24, 6)[1..5],
            [
                "│               ≡ Note │",
                "│ No note              │",
                "│                      │",
                "│ e edit · Esc close   │",
            ]
        );
        let none = AttachmentsView {
            sections: &[Attachment::Comments],
            ..view(&[], &[], None)
        };
        assert_eq!(
            render(&none, 24, 6)[1..5],
            [
                "│           ❞ Comments │",
                "│ No comments          │",
                "│                      │",
                "│ a add · Esc close    │",
            ]
        );
    }

    // FR-ATT-04, attachments.md §4
    #[test]
    fn the_note_renders_markdown_wrapped_to_the_panel() {
        let note = AttachmentsView {
            sections: &[Attachment::Note],
            ..view(
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
                &[],
                None,
            )
        };
        let (buf, caret) = draw(&note, (30, 15), IconSet::Unicode, 0);
        assert_eq!(
            rows(&buf)[1..14],
            [
                "│                     ≡ Note │",
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
                "│ Esc close                  │",
            ]
        );
        assert_eq!(caret, None);
        let has = |x: u16, y: u16, modifier| buf[(x, y)].modifier.contains(modifier);
        assert!(
            has(2, 2, Modifier::BOLD) && has(2, 2, Modifier::UNDERLINED),
            "the heading"
        );
        assert!(has(7, 3, Modifier::ITALIC) && !has(2, 3, Modifier::ITALIC));
        assert!(has(16, 3, Modifier::BOLD));
        assert!(
            has(4, 9, Modifier::DIM) && has(2, 9, Modifier::DIM),
            "the quote"
        );
        assert!(has(4, 10, Modifier::DIM), "the code");
        assert!(has(2, 6, Modifier::DIM), "the bullet");
        let ascii = rows(&draw(&note, (30, 15), IconSet::Ascii, 0).0);
        assert_eq!(ascii[6], "│ - one                      │");
        assert_eq!(ascii[7], "│   * two                    │");
    }

    // FR-ATT-03, attachments.md §3, D-109
    #[test]
    fn the_content_scrolls_no_further_than_its_end_and_to_a_new_selection() {
        let lines: Vec<String> = (1..=9).map(|n| format!("line {n}")).collect();
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        let note = view(&lines, &["c"], Some(PanelItem::Comment(0)));
        let area = Rect::new(0, 0, 24, 7);
        let panel = AttachmentsPanel::new(&note, IconSet::Unicode, &THEME);
        assert_eq!(
            panel.scroll(2, area, false),
            Scrolled {
                scroll: 2,
                page: 3,
                current: Some(12..13)
            }
        );
        assert_eq!(panel.scroll(usize::MAX, area, false).scroll, 10);
        assert_eq!(panel.scroll(2, area, true).scroll, 10, "to the comment");
        let typed = AttachmentsView {
            draft: Some(draft(Some(0), "c!")),
            ..view(&lines, &["c"], Some(PanelItem::Note))
        };
        assert_eq!(
            AttachmentsPanel::new(&typed, IconSet::Unicode, &THEME)
                .scroll(2, area, false)
                .scroll,
            10,
            "to the comment being typed"
        );
        let first = view(&lines, &["c"], Some(PanelItem::Note));
        let panel = AttachmentsPanel::new(&first, IconSet::Unicode, &THEME);
        assert_eq!(
            panel.scroll(5, area, true).scroll,
            0,
            "the start of a taller item"
        );
        let shown = rows(&draw(&first, (24, 7), IconSet::Unicode, 0).0);
        assert_eq!(
            shown[1..4],
            [
                "│               ≡ Note │",
                "│ line 1               │",
                "│ line 2               │"
            ]
        );
        let flat = Rect::new(0, 0, 24, 3);
        assert_eq!(
            AttachmentsPanel::new(&first, IconSet::Unicode, &THEME)
                .scroll(0, flat, false)
                .page,
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
        let (buf, caret) = draw(&editing(&text, 4, 1), (24, 7), IconSet::Unicode, 0);
        assert_eq!(
            rows(&buf)[1..6],
            [
                "│ b                    │",
                "│ c                    │",
                "│ d                    │",
                "│                      │",
                "│ Enter new line · Esc │",
            ]
        );
        assert_eq!(caret, Some((3, 3)));
        assert_ne!(buf[(2, 3)].bg, THEME.current_line, "no band while editing");
        let (buf, caret) = draw(&editing(&text, 1, 35), (24, 7), IconSet::Unicode, 0);
        assert_eq!(rows(&buf)[3], "│  goes past the edge  │");
        assert_eq!(caret, Some((21, 3)), "the caret's line scrolls to it");
        let (buf, caret) = draw(&editing(&text, 0, 0), (24, 7), IconSet::Unicode, 9);
        assert_eq!(
            rows(&buf)[1..4],
            [
                "│ # Title              │",
                "│ a long line that go… │",
                "│ b                    │"
            ]
        );
        assert_eq!(caret, Some((2, 1)));
        let area = Rect::new(0, 0, 24, 7);
        let view = editing(&text, 0, 0);
        let panel = AttachmentsPanel::new(&view, IconSet::Unicode, &THEME).scrolled(3);
        assert_eq!(panel.caret(area), None, "the caret's line is out of view");
        let (_, caret) = draw(&editing(&text, 0, 0), (24, 3), IconSet::Unicode, 0);
        assert_eq!(caret, None, "no room for the note");
    }

    const COMMENTS: [&str; 3] = [
        "[2026-10-08 09:05] Bought **it** at the corner shop",
        "by hand",
        "[2026-10-07 18:02] a",
    ];

    /// Only the comments section, with `comments`, comment `selected` selected and `draft` typed.
    fn comments(
        comments: &[&str],
        selected: Option<usize>,
        draft: Option<CommentDraft>,
    ) -> AttachmentsView<'static> {
        AttachmentsView {
            sections: &[Attachment::Comments],
            draft,
            ..view(&[], comments, selected.map(PanelItem::Comment))
        }
    }

    // FR-ATT-05, attachments.md §5
    #[test]
    fn comments_show_their_dimmed_timestamps_and_the_selected_one_on_a_band() {
        let (buf, caret) = draw(
            &comments(&COMMENTS, Some(1), None),
            (32, 8),
            IconSet::Unicode,
            0,
        );
        assert_eq!(
            rows(&buf)[1..7],
            [
                "│ 2026-10-08 09:05 Bought it   │",
                "│                  at the      │",
                "│                  corner shop │",
                "│ by hand                      │",
                "│                              │",
                "│ a add · e edit · d del · Esc │",
            ]
        );
        assert_eq!(caret, None);
        assert!(buf[(2, 1)].modifier.contains(Modifier::DIM), "the stamp");
        assert!(!buf[(19, 1)].modifier.contains(Modifier::DIM));
        assert!(
            buf[(26, 1)].modifier.contains(Modifier::BOLD),
            "inline Markdown"
        );
        let (buf, _) = draw(
            &comments(&COMMENTS, Some(1), None),
            (32, 9),
            IconSet::Unicode,
            0,
        );
        assert_eq!(rows(&buf)[1], "│                   ❞ Comments │");
        assert_eq!(rows(&buf)[5], "│ by hand                      │");
        assert_eq!(buf[(2, 5)].bg, THEME.current_line, "the selected comment");
        assert_eq!(buf[(29, 5)].bg, THEME.current_line, "the whole width");
        assert_ne!(buf[(2, 2)].bg, THEME.current_line);
        let (buf, _) = draw(
            &comments(&COMMENTS, Some(2), None),
            (32, 8),
            IconSet::Unicode,
            0,
        );
        assert_eq!(
            rows(&buf)[1..5],
            [
                "│                  at the      │",
                "│                  corner shop │",
                "│ by hand                      │",
                "│ 2026-10-07 18:02 a           │",
            ],
            "the selected comment is kept in view"
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
            rows(&buf)[1..7],
            [
                "│                   ❞ Comments │",
                "│ › new                        │",
                "│ by hand                      │",
                "│ 2026-10-07 18:02 a           │",
                "│                              │",
                "│ Enter add · Esc cancel       │",
            ]
        );
        assert_eq!(caret, Some((7, 2)));
        assert_eq!(buf[(2, 2)].bg, THEME.current_line);
        assert_ne!(buf[(2, 3)].bg, THEME.current_line, "not the selected one");
        let (buf, caret) = draw(
            &comments(&COMMENTS[1..], Some(0), Some(draft(Some(1), "b"))),
            (32, 8),
            IconSet::Ascii,
            0,
        );
        assert_eq!(
            rows(&buf)[2..4],
            [
                "│ by hand                      │",
                "│ > b                          │"
            ]
        );
        assert_eq!(rows(&buf)[6], "│ Enter save · Esc cancel      │");
        assert_eq!(caret, Some((5, 3)));
        let (_, caret) = draw(
            &comments(&COMMENTS[1..], Some(0), Some(draft(Some(1), "b"))),
            (32, 3),
            IconSet::Ascii,
            0,
        );
        assert_eq!(caret, None, "no room for the comments");
        let with_note = AttachmentsView {
            draft: Some(draft(None, "x")),
            ..view(&["n"], &[], Some(PanelItem::Note))
        };
        let (buf, _) = draw(&with_note, (32, 9), IconSet::Unicode, 0);
        assert_ne!(
            buf[(2, 2)].bg,
            THEME.current_line,
            "the note is not selected while typing"
        );
    }

    // FR-ATT-03, attachments.md §3
    #[test]
    fn the_panel_takes_the_right_of_the_screen_from_top_to_bottom() {
        assert_eq!(
            AttachmentsPanel::area(Rect::new(0, 0, 120, 30)),
            Rect::new(60, 0, 60, 30)
        );
        assert_eq!(
            AttachmentsPanel::area(Rect::new(0, 0, 200, 30)),
            Rect::new(128, 0, 72, 30)
        );
        assert_eq!(
            AttachmentsPanel::area(Rect::new(0, 0, 60, 30)),
            Rect::new(20, 0, 40, 30)
        );
        assert_eq!(
            AttachmentsPanel::area(Rect::new(0, 0, 30, 30)),
            Rect::new(0, 0, 30, 30)
        );
    }
}
