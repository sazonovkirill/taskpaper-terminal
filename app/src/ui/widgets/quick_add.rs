//! The Quick Add panel ([ui.md §1](../../../../docs/specs/ui.md#1-layout),
//! [editing.md §6](../../../../docs/specs/editing.md#6-quick-add-fr-mode-07-fr-mode-08)): a
//! panel across the bottom of the screen, placed as the date prompt's, titled with the
//! destination's path, holding the text behind a task's marker with the dates that count
//! highlighted and, below it, what `Enter` does.

use std::ops::Range;

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Clear, Widget},
};

use super::{Caret, Field, drawable, due_style, start_style};
use crate::{
    app::{Missing, QuickAddView},
    document::DateTag,
    ui::theme::{Icon, IconSet, Theme},
};

/// The Quick Add panel.
#[derive(Debug)]
pub struct QuickAddPanel<'a> {
    view: &'a QuickAddView<'a>,
    icons: IconSet,
    theme: &'a Theme,
    scroll: usize,
}

impl<'a> QuickAddPanel<'a> {
    /// The panel's height, the border included, the date prompt's.
    pub const HEIGHT: u16 = super::DatePromptPanel::HEIGHT;

    /// The panel showing `view` in `theme`, its task marker from `icons`.
    #[must_use]
    pub fn new(view: &'a QuickAddView<'a>, icons: IconSet, theme: &'a Theme) -> Self {
        Self {
            view,
            icons,
            theme,
            scroll: 0,
        }
    }

    /// The panel with its text scrolled on from `scroll` columns, as far as the last frame
    /// scrolled it (editing.md §2).
    #[must_use]
    pub fn text_scroll(self, scroll: usize) -> Self {
        Self { scroll, ..self }
    }

    /// The caret's cell when the panel is drawn over `area`, on the text's line, the first inside
    /// the border, and how far the text is scrolled.
    #[must_use]
    pub fn caret(&self, area: Rect) -> ((u16, u16), usize) {
        let inner = Block::bordered().inner(area);
        let (_, caret) = self.input(inner.width);
        let x = u16::try_from(caret.column)
            .unwrap_or(u16::MAX)
            .min(inner.width.saturating_sub(1));
        ((inner.x.saturating_add(x), inner.y), caret.scroll)
    }

    /// The text after a space and a task's marker, `width` columns wide, the dates that count in
    /// their chips' colors, and where the caret is on it.
    fn input(&self, width: u16) -> (Line<'a>, Caret) {
        let mut line = Line::from(" ");
        if self.view.task {
            line.spans
                .push(Span::raw(Icon::TaskMarker.glyph(self.icons)));
            line.spans.push(Span::raw(" "));
        }
        let field = Field {
            text: self.view.text,
            caret: self.view.caret,
            selection: self.view.selection.clone(),
            scroll: self.scroll,
        };
        let highlights: Vec<(Range<usize>, Style)> = self
            .view
            .dates
            .iter()
            .map(|date| {
                let style = match date.tag {
                    DateTag::Start => start_style(self.theme),
                    DateTag::Due => due_style(self.theme),
                };
                (date.span.clone(), style)
            })
            .collect();
        let (spans, caret) = field.spans(Style::new(), line.width(), usize::from(width));
        line.spans
            .extend(highlighted(spans, self.view.text.len(), &highlights));
        (line, caret)
    }

    /// What `Enter` does: says what is missing, or names the dates it sets.
    fn preview(&self) -> Line<'static> {
        let dim = |text: &str| Line::styled(format!(" {text}"), Style::new().dim());
        match self.view.missing {
            Some(Missing::Text) => return dim("Type the text"),
            Some(Missing::Project) => return dim("Choose a project with Tab"),
            None => {}
        }
        if self.view.dates.is_empty() {
            return dim("Enter adds it");
        }
        let mut line = Line::from(" ");
        for (at, date) in self.view.dates.iter().enumerate() {
            if at > 0 {
                line.spans.push(Span::raw(" · "));
            }
            let (name, style) = match date.tag {
                DateTag::Start => ("Start", start_style(self.theme)),
                DateTag::Due => ("Due", due_style(self.theme)),
            };
            line.spans.push(Span::styled(
                format!("{name} {}", date.date.strftime("%a %-d %b")),
                style,
            ));
        }
        line
    }
}

/// `spans`, which show the end of a text `len` bytes long, split where `highlights` start and
/// end, each part inside one patched with its style.
fn highlighted<'a>(
    spans: Vec<Span<'a>>,
    len: usize,
    highlights: &[(Range<usize>, Style)],
) -> Vec<Span<'a>> {
    let mut at = len - spans.iter().map(|span| span.content.len()).sum::<usize>();
    let mut out = Vec::new();
    for span in spans {
        let end = at + span.content.len();
        let mut cuts: Vec<usize> = highlights
            .iter()
            .flat_map(|(range, _)| [range.start, range.end])
            .filter(|cut| (at..=end).contains(cut))
            .chain([at, end])
            .collect();
        cuts.sort_unstable();
        cuts.dedup();
        for &[from, to] in cuts.array_windows() {
            let style = highlights
                .iter()
                .filter(|(range, _)| range.start <= from && to <= range.end)
                .fold(span.style, |style, (_, highlight)| style.patch(*highlight));
            out.push(Span::styled(
                span.content[from - at..to - at].to_owned(),
                style,
            ));
        }
        at = end;
    }
    out
}

impl Widget for QuickAddPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let title = match &self.view.destination {
            Some(path) => format!(" Quick Add → {} ", drawable(&path.join(" › "))),
            None => " Quick Add ".to_owned(),
        };
        let accent = Style::new().fg(self.theme.accent);
        let frame = Block::bordered()
            .border_style(accent)
            .title_style(accent)
            .title(title);
        let inner = frame.inner(area);
        frame.render(area, buf);
        let [input, preview] = inner.layout(&Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
        ]));
        self.input(input.width).0.render(input, buf);
        self.preview().render(preview, buf);
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;
    use ratatui::style::Color;

    use super::super::tests::THEME;
    use super::*;
    use crate::document::dates::Found;

    /// The panel showing `view`, drawn over 40 × 4 cells.
    fn render(view: &QuickAddView<'_>) -> Buffer {
        let area = Rect::new(0, 0, 40, QuickAddPanel::HEIGHT);
        let mut buf = Buffer::empty(area);
        QuickAddPanel::new(view, IconSet::Nerd, &THEME).render(area, &mut buf);
        buf
    }

    fn row(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
    }

    fn view(text: &str) -> QuickAddView<'_> {
        QuickAddView {
            task: true,
            text,
            caret: text.len(),
            selection: None,
            dates: Vec::new(),
            destination: Some(vec!["Work".to_owned(), "Launch".to_owned()]),
            missing: None,
        }
    }

    // FR-MODE-07, editing.md §6, ui.md §1
    #[test]
    fn shows_the_destination_the_task_text_and_the_dates_it_sets_in_their_colors() {
        let text = "Call Bob tomorrow due fri";
        let view = QuickAddView {
            dates: vec![
                Found {
                    span: 9..17,
                    tag: DateTag::Start,
                    date: date(2026, 10, 8),
                },
                Found {
                    span: 18..25,
                    tag: DateTag::Due,
                    date: date(2026, 10, 9),
                },
            ],
            ..view(text)
        };
        let buf = render(&view);
        assert_eq!(
            (0..4).map(|y| row(&buf, y)).collect::<Vec<_>>(),
            [
                "┌ Quick Add → Work › Launch ───────────┐",
                "│ - Call Bob tomorrow due fri          │",
                "│ Start Thu 8 Oct · Due Fri 9 Oct      │",
                "└──────────────────────────────────────┘",
            ]
        );
        assert_eq!(buf[(1, 0)].fg, THEME.accent, "border");
        assert_eq!(buf[(3, 0)].fg, THEME.accent, "title");
        let fg = |x| buf[(x, 1)].fg;
        assert_eq!(fg(10), Color::Reset, "plain text");
        assert_eq!((fg(13), fg(20)), (THEME.start, THEME.start), "start date");
        assert_eq!((fg(22), fg(28)), (THEME.due, THEME.due), "due date");
        assert_eq!(buf[(2, 2)].fg, THEME.start);
        assert_eq!(buf[(26, 2)].fg, THEME.due);
        assert_eq!(
            QuickAddPanel::new(&view, IconSet::Nerd, &THEME).caret(buf.area),
            ((29, 1), 0)
        );
    }

    // FR-MODE-07, ui.md §1
    #[test]
    fn a_tab_in_the_destinations_path_is_drawn_as_a_space() {
        let view = QuickAddView {
            destination: Some(vec!["Big\tWork".to_owned()]),
            ..view("")
        };
        assert_eq!(
            row(&render(&view), 0),
            "┌ Quick Add → Big Work ────────────────┐"
        );
    }

    // FR-MODE-07, editing.md §6
    #[test]
    fn says_what_enter_lacks_and_a_note_has_no_dash() {
        let preview = |view: &QuickAddView<'_>| row(&render(view), 2);
        assert_eq!(
            preview(&view("x")),
            "│ Enter adds it                        │"
        );
        assert_eq!(
            preview(&QuickAddView {
                missing: Some(Missing::Text),
                ..view("")
            }),
            "│ Type the text                        │"
        );
        let note = QuickAddView {
            task: false,
            destination: None,
            missing: Some(Missing::Project),
            ..view("Idea")
        };
        let buf = render(&note);
        assert_eq!(row(&buf, 0), "┌ Quick Add ───────────────────────────┐");
        assert_eq!(row(&buf, 1), "│ Idea                                 │");
        assert_eq!(row(&buf, 2), "│ Choose a project with Tab            │");
    }

    // ui.md §1: a text wider than the panel scrolls to keep the caret in view, and its dates
    // keep their colors where they are cut.
    #[test]
    fn a_scrolled_text_keeps_its_highlights_and_selection() {
        let spans = vec![
            Span::raw("cd"),
            Span::styled("ef", Style::new().reversed()),
            Span::raw("gh"),
        ];
        let start = Style::new().fg(Color::Green);
        let parts: Vec<(String, Style)> = highlighted(spans, 8, &[(1..5, start)])
            .into_iter()
            .map(|span| (span.content.into_owned(), span.style))
            .collect();
        assert_eq!(
            parts,
            [
                ("cd".to_owned(), start),
                ("e".to_owned(), Style::new().reversed().fg(Color::Green)),
                ("f".to_owned(), Style::new().reversed()),
                ("gh".to_owned(), Style::new()),
            ]
        );
    }
}
