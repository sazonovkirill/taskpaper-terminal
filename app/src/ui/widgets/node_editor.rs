//! The Edit Node dialog ([editing.md §7](../../../../docs/specs/editing.md#7-edit-node-dialog-fr-mode-09),
//! D-103): a box centered over the panes, one row per property of a block, a label and a value,
//! the focused row marked as the outline marks its focused block, and a hint naming the keys of
//! the focused row, or what its open field would do. What does not fit ends in `…`, the Parent
//! path keeping its end, as the status line keeps the file's (ui.md §1).

use jiff::civil::Date;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Clear, Widget},
};

use super::{
    FOCUS_MARKER, Field, due_style, fitted_path, flag_style, overdue_style, start_style,
    today_style, truncate_before, warning_style, wrap::truncate,
};
use crate::{
    app::{DateValue, FieldOutcome, NodeEditorView, NodeRow},
    document::{dates, format::Typed},
    ui::theme::{Icon, IconSet, Theme},
};

/// The widest the dialog is drawn.
const WIDTH: u16 = 64;

/// The columns before a row's value: the focus marker, a space and the label.
const LABEL_WIDTH: usize = 11;

/// The Edit Node dialog.
#[derive(Debug)]
pub struct NodeEditorWindow<'a> {
    view: &'a NodeEditorView<'a>,
    today: Date,
    icons: IconSet,
    theme: &'a Theme,
}

/// A line of the dialog's body: the row it shows, if the focus can be on it, its label and its
/// value.
struct BodyLine<'a> {
    row: Option<NodeRow>,
    label: &'static str,
    value: Line<'a>,
    /// Whether the value is the text being typed, which [`NodeEditorWindow::input`] draws.
    typing: bool,
}

impl<'a> NodeEditorWindow<'a> {
    /// The dialog showing `view`, its dates named relative to `today`, in `theme` with `icons`.
    #[must_use]
    pub fn new(
        view: &'a NodeEditorView<'a>,
        today: Date,
        icons: IconSet,
        theme: &'a Theme,
    ) -> Self {
        Self {
            view,
            today,
            icons,
            theme,
        }
    }

    /// Where the dialog goes over `area`: centered, as wide as it can be up to 64 columns and as
    /// tall as its lines, with the border.
    #[must_use]
    pub fn area(&self, area: Rect) -> Rect {
        let height = u16::try_from(self.lines() + 4).unwrap_or(u16::MAX);
        area.centered(
            Constraint::Length(WIDTH.min(area.width)),
            Constraint::Length(height.min(area.height)),
        )
    }

    /// The caret's cell when the dialog is drawn over `area`: in the open field, else in the
    /// text while the Text row is focused, else none.
    #[must_use]
    pub fn caret(&self, area: Rect) -> Option<(u16, u16)> {
        let inner = Block::bordered().inner(self.area(area));
        let focused = self.view.rows.get(self.view.focus).copied();
        let editing = self.view.field.is_some() || focused == Some(NodeRow::Text);
        if !editing {
            return None;
        }
        let y = self.view.focus.checked_sub(self.scroll(inner.height))?;
        let (_, column) = self.input(usize::from(inner.width));
        let x = u16::try_from(column)
            .unwrap_or(u16::MAX)
            .min(inner.width.saturating_sub(1));
        Some((
            inner.x.saturating_add(x),
            inner.y.saturating_add(u16::try_from(y).unwrap_or(u16::MAX)),
        ))
    }

    /// The text being typed on the focused row, the field's or the Text row's, from the value
    /// column on a line `width` columns wide, and the caret's column on it.
    fn input(&self, width: usize) -> (Line<'a>, usize) {
        let (field, prefix) = match &self.view.field {
            Some(field) => {
                let tag = matches!(
                    self.view.rows.get(self.view.focus),
                    Some(NodeRow::Tag(_) | NodeRow::AddTag)
                );
                let field = Field {
                    text: field.text,
                    caret: field.caret,
                    selection: field.selection.clone(),
                };
                (field, if tag { "@" } else { "" })
            }
            None => (
                Field {
                    text: self.view.text,
                    caret: self.view.caret,
                    selection: self.view.selection.clone(),
                },
                "",
            ),
        };
        let start = LABEL_WIDTH + prefix.len();
        let (spans, column) = field.spans(Style::new(), start, width);
        let mut line = Line::from(prefix);
        line.spans.extend(spans);
        (line, column)
    }

    /// How many lines the body has: one per row, then one per special node.
    fn lines(&self) -> usize {
        self.view.rows.len() + self.view.hidden.len()
    }

    /// Each line of the body, in order: the rows, then the special nodes, their values cut to
    /// `columns` columns.
    fn body(&self, columns: usize) -> Vec<BodyLine<'a>> {
        let view = self.view;
        let mut lines: Vec<BodyLine<'a>> = view
            .rows
            .iter()
            .enumerate()
            .map(|(at, &row)| {
                let typing = at == view.focus && (view.field.is_some() || row == NodeRow::Text);
                BodyLine {
                    row: Some(row),
                    label: label(row, view.tags.is_empty()),
                    value: fit(&self.value(row, columns), columns),
                    typing,
                }
            })
            .collect();
        for (at, node) in view.hidden.iter().enumerate() {
            let lines_count = match node.lines {
                1 => "1 line".to_owned(),
                n => format!("{n} lines"),
            };
            lines.push(BodyLine {
                row: None,
                label: if at == 0 { "Hidden" } else { "" },
                value: truncate(
                    &Line::styled(
                        format!("{} · {lines_count}", node.tags.join(" ")),
                        Style::new().dim(),
                    ),
                    columns,
                ),
                typing: false,
            });
        }
        lines
    }

    /// The value of `row` as shown while no field is open over it, the Parent path fitted to
    /// `columns` columns.
    fn value(&self, row: NodeRow, columns: usize) -> Line<'a> {
        let view = self.view;
        let icon = |icon: Icon| icon.glyph(self.icons);
        let unset = |text: &'static str| Line::styled(text, Style::new().dim());
        match row {
            NodeRow::Text => Line::raw(view.text),
            NodeRow::Type => {
                let (glyph, name) = match view.typed {
                    Typed::Project => (Icon::Project, "Project"),
                    Typed::Task => (Icon::TaskMarker, "Task"),
                    Typed::Section | Typed::Text => (Icon::Note, "Note"),
                };
                Line::raw(format!("{} {name}", icon(glyph)))
            }
            NodeRow::Done => match view.done {
                Some(on) => Line::raw(match on {
                    Some(date) => format!("{} Done {date}", icon(Icon::DoneTask)),
                    None => format!("{} Done", icon(Icon::DoneTask)),
                }),
                None => unset("Not done"),
            },
            NodeRow::Parent => match &view.parent {
                Some(path) => Line::raw(fitted_path(&path.join(" › "), columns).into_owned()),
                None => Line::raw(format!("{} Home", icon(Icon::Home))),
            },
            NodeRow::Start | NodeRow::Due => {
                let (value, glyph, style) = if row == NodeRow::Start {
                    (view.start, Icon::Start, start_style(self.theme))
                } else {
                    (view.due, Icon::Due, due_style(self.theme))
                };
                match value {
                    Some(DateValue::Date(date))
                        if row == NodeRow::Due && view.done.is_none() && date < self.today =>
                    {
                        self.date(date, Icon::Overdue, overdue_style(self.theme))
                    }
                    Some(DateValue::Date(date)) => self.date(date, glyph, style),
                    Some(DateValue::Written(written)) => Line::from(vec![
                        Span::raw(written.unwrap_or_default().to_owned()),
                        Span::styled(" not a date", Style::new().dim()),
                    ]),
                    None => unset("None"),
                }
            }
            NodeRow::Starred if view.starred => Line::styled(
                format!("{} Starred", icon(Icon::Star)),
                today_style(self.theme),
            ),
            NodeRow::Starred => unset("Not starred"),
            NodeRow::Flagged if view.flagged => Line::styled(
                format!("{} Flagged", icon(Icon::Flag)),
                flag_style(self.theme),
            ),
            NodeRow::Flagged => unset("Not flagged"),
            NodeRow::Tag(at) => view.tags.get(at).map_or_else(Line::default, |tag| {
                let mut line = Line::raw(match tag.value {
                    Some(value) => format!("@{}({value})", tag.name),
                    None => format!("@{}", tag.name),
                });
                if !tag.supported {
                    line.push_span(Span::styled(" unsupported", Style::new().dim()));
                }
                line
            }),
            NodeRow::AddTag => unset("+ Add tag"),
        }
    }

    /// `date` after `glyph`, as `Thu 8 Oct 2026` and named relative to today, in `style`: as
    /// the outline's date chip shows it.
    fn date(&self, date: Date, glyph: Icon, style: Style) -> Line<'a> {
        Line::styled(
            format!(
                "{} {} · {}",
                glyph.glyph(self.icons),
                date.strftime("%a %-d %b %Y"),
                dates::relative(date, self.today)
            ),
            style,
        )
    }

    /// The last line: what the open field's `Enter` would do, or the focused row's keys.
    fn hint(&self) -> Line<'static> {
        let dim = |text: &str| Line::styled(text.to_owned(), Style::new().dim());
        if let Some(field) = &self.view.field {
            let warning = |error: String| Line::styled(error, warning_style(self.theme));
            return match &field.outcome {
                FieldOutcome::Date(Ok(Some(date))) => dim(&format!(
                    "{} · Enter sets it",
                    date.strftime("%a %-d %b %Y")
                )),
                FieldOutcome::Tag(Ok(false))
                    if self.view.rows.get(self.view.focus) == Some(&NodeRow::AddTag) =>
                {
                    warning("no tag entered".to_owned())
                }
                FieldOutcome::Date(Ok(None)) | FieldOutcome::Tag(Ok(false)) => {
                    dim("Enter removes it · Esc keeps it")
                }
                FieldOutcome::Tag(Ok(true)) => dim("Enter writes it · Esc keeps it as it was"),
                FieldOutcome::Date(Err(error)) => warning(error.to_string()),
                FieldOutcome::Tag(Err(error)) => warning(error.to_string()),
            };
        }
        let keys = match self.view.rows.get(self.view.focus) {
            Some(NodeRow::Text) => "Type the text · Tab next",
            Some(NodeRow::Type) => "Space or ←/→ change · p t n",
            Some(NodeRow::Parent) => "Space or type to choose",
            Some(NodeRow::Start | NodeRow::Due) => "Type or ←/→ a date · Del clears",
            Some(NodeRow::Tag(_)) => "Space or type to edit · Del removes",
            Some(NodeRow::AddTag) => "Type a tag: name or name(value)",
            Some(NodeRow::Done) => "Space or x toggles",
            Some(NodeRow::Starred) => "Space or * toggles",
            _ => "Space or ! toggles",
        };
        dim(&format!("{keys} · Enter save · Esc cancel"))
    }

    /// How many body lines are scrolled past to keep the focused row in view in `height` rows,
    /// and on the last row as many of the special nodes after it as fit, since the focus skips
    /// them.
    fn scroll(&self, height: u16) -> usize {
        let room = room(height).max(1);
        let focus = self.view.focus;
        if focus + 1 == self.view.rows.len() {
            self.lines().saturating_sub(room).min(focus)
        } else {
            (focus + 1).saturating_sub(room)
        }
    }
}

/// `value` cut to `columns` columns, its spans after the first, markers such as `unsupported`,
/// kept in view after the `…`.
fn fit(value: &Line, columns: usize) -> Line<'static> {
    truncate_before(value, 1..value.spans.len(), columns)
}

/// How many body lines fit in `height` rows: all but the blank line and the hint, unless there
/// is no room for those beside a line.
fn room(height: u16) -> usize {
    let height = usize::from(height);
    if height >= 3 { height - 2 } else { height }
}

/// The label of `row`; a tag's only on the first tag, and on Add tag when there is none.
fn label(row: NodeRow, no_tags: bool) -> &'static str {
    match row {
        NodeRow::Text => "Text",
        NodeRow::Type => "Type",
        NodeRow::Done => "Done",
        NodeRow::Parent => "Parent",
        NodeRow::Start => "Start",
        NodeRow::Due => "Due",
        NodeRow::Starred => "Starred",
        NodeRow::Flagged => "Flagged",
        NodeRow::Tag(0) => "Tags",
        NodeRow::AddTag if no_tags => "Tags",
        NodeRow::Tag(_) | NodeRow::AddTag => "",
    }
}

impl Widget for NodeEditorWindow<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let window = self.area(area);
        Clear.render(window, buf);
        let accent = Style::new().fg(self.theme.accent);
        let frame = Block::bordered()
            .border_style(accent)
            .title_style(accent)
            .title(" Edit Node ");
        let inner = frame.inner(window);
        frame.render(window, buf);
        let columns = usize::from(inner.width).saturating_sub(LABEL_WIDTH);
        let body = self.body(columns);
        let skip = self.scroll(inner.height);
        let room = room(inner.height);
        let focused = self.view.rows.get(self.view.focus).copied();
        for (y, line) in (inner.y..).zip(body.iter().skip(skip).take(room)) {
            let row = Rect::new(inner.x, y, inner.width, 1);
            let mut shown = Line::default();
            if line.row.is_some() && line.row == focused {
                shown.push_span(Span::styled(
                    FOCUS_MARKER,
                    Style::new().fg(self.theme.focus),
                ));
                shown.push_span(" ");
            } else {
                shown.push_span("  ");
            }
            shown.push_span(Span::styled(
                format!("{:<width$}", line.label, width = LABEL_WIDTH - 2),
                Style::new().dim(),
            ));
            let value = if line.typing {
                self.input(usize::from(inner.width)).0
            } else {
                line.value.clone()
            };
            // A value's style can be on the line rather than its spans, which moving the spans
            // onto this line would lose.
            shown.spans.extend(value.spans.into_iter().map(|span| Span {
                style: value.style.patch(span.style),
                ..span
            }));
            shown.render(row, buf);
        }
        let hint = Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1);
        if inner.height >= 3 {
            let mut line = self.hint();
            line.spans.insert(0, Span::raw(" "));
            truncate(&line, usize::from(inner.width)).render(hint, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use ratatui::style::Color;

    use super::super::tests::THEME;
    use super::*;
    use crate::{
        app::{FieldView, TagView},
        document::{HiddenData, dates::DateError, properties::TagError},
    };

    const TODAY: Date = date(2026, 10, 5);

    /// Every row of the dialog showing `view`, drawn over `width` × `height` cells, and the
    /// caret's cell.
    fn render(
        view: &NodeEditorView<'_>,
        width: u16,
        height: u16,
    ) -> (Vec<String>, Option<(u16, u16)>) {
        let (buf, caret) = draw(view, width, height);
        (rows(&buf), caret)
    }

    /// The dialog showing `view` drawn over `width` × `height` cells, and the caret's cell.
    fn draw(view: &NodeEditorView<'_>, width: u16, height: u16) -> (Buffer, Option<(u16, u16)>) {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        let window = NodeEditorWindow::new(view, TODAY, IconSet::Unicode, &THEME);
        let caret = window.caret(area);
        window.render(area, &mut buf);
        (buf, caret)
    }

    /// Every row of `buf` as text.
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

    /// The value on `view`'s row labelled `label` as text, and the style of its cells, which
    /// must all be the same, as a style that sets nothing but their color and modifiers.
    fn value_style(view: &NodeEditorView<'_>, label: &str) -> (String, Style) {
        let (buf, _) = draw(view, 64, 20);
        let rows = rows(&buf);
        let y = rows
            .iter()
            .position(|row| row.contains(&format!(" {label:<9}")))
            .unwrap();
        let value = rows[y].chars().skip(12).collect::<String>();
        let value = value.trim_end_matches(['│', ' ']).to_owned();
        let width = u16::try_from(value.chars().count()).unwrap();
        let styles: Vec<Style> = (12..12 + width)
            .map(|x| {
                let cell = &buf[(x, u16::try_from(y).unwrap())];
                let style = Style::new().add_modifier(cell.modifier);
                if cell.fg == Color::Reset {
                    style
                } else {
                    style.fg(cell.fg)
                }
            })
            .collect();
        assert!(
            styles.iter().all(|style| *style == styles[0]),
            "{label}: {styles:?}"
        );
        (value, styles[0])
    }

    /// The rows of a block with no tags, focused on `focus`.
    fn view(focus: usize) -> NodeEditorView<'static> {
        NodeEditorView {
            rows: vec![
                NodeRow::Text,
                NodeRow::Type,
                NodeRow::Done,
                NodeRow::Parent,
                NodeRow::Start,
                NodeRow::Due,
                NodeRow::Starred,
                NodeRow::Flagged,
                NodeRow::AddTag,
            ],
            focus,
            text: "Milk",
            caret: 2,
            selection: None,
            typed: Typed::Task,
            done: None,
            parent: None,
            start: None,
            due: None,
            starred: false,
            flagged: false,
            tags: Vec::new(),
            hidden: Vec::new(),
            field: None,
        }
    }

    /// The last row inside the border: the hint.
    fn hint(view: &NodeEditorView<'_>) -> String {
        let (rows, _) = render(view, 64, 30);
        let bottom = rows.iter().rposition(|row| row.starts_with('└')).unwrap();
        rows[bottom - 1].clone()
    }

    // editing.md §7, ui.md §1
    #[test]
    fn shows_each_row_as_a_label_and_its_value_the_focused_one_marked() {
        let view = NodeEditorView {
            typed: Typed::Project,
            done: Some(Some("2026-10-04")),
            parent: Some(vec!["Work".to_owned(), "Launch".to_owned()]),
            start: Some(DateValue::Date(TODAY)),
            due: Some(DateValue::Written(Some("soon"))),
            starred: true,
            flagged: true,
            rows: vec![
                NodeRow::Text,
                NodeRow::Type,
                NodeRow::Done,
                NodeRow::Parent,
                NodeRow::Start,
                NodeRow::Due,
                NodeRow::Starred,
                NodeRow::Flagged,
                NodeRow::Tag(0),
                NodeRow::Tag(1),
                NodeRow::AddTag,
            ],
            tags: vec![
                TagView {
                    name: "x",
                    value: Some("1"),
                    supported: true,
                },
                TagView {
                    name: "y",
                    value: None,
                    supported: false,
                },
            ],
            hidden: vec![
                HiddenData {
                    tags: vec!["@hidden".to_owned()],
                    lines: 1,
                },
                HiddenData {
                    tags: vec!["@hidden".to_owned(), "@log".to_owned()],
                    lines: 3,
                },
            ],
            ..view(5)
        };
        let (rows, caret) = render(&view, 50, 20);
        assert_eq!(
            rows[2..19],
            [
                "┌ Edit Node ─────────────────────────────────────┐",
                "│  Text     Milk                                 │",
                "│  Type     ▸ Project                            │",
                "│  Done     ✔ Done 2026-10-04                    │",
                "│  Parent   Work › Launch                        │",
                "│  Start    ↦ Mon 5 Oct 2026 · today             │",
                "│▌ Due      soon not a date                      │",
                "│  Starred  ★ Starred                            │",
                "│  Flagged  ⚐ Flagged                            │",
                "│  Tags     @x(1)                                │",
                "│           @y unsupported                       │",
                "│           + Add tag                            │",
                "│  Hidden   @hidden · 1 line                     │",
                "│           @hidden @log · 3 lines               │",
                "│                                                │",
                "│ Type or ←/→ a date · Del clears · Enter save ·…│",
                "└────────────────────────────────────────────────┘",
            ]
        );
        assert_eq!(caret, None, "no caret off the Text row");
    }

    // editing.md §7
    #[test]
    fn shows_unset_values_dimmed_and_a_note() {
        let view = NodeEditorView {
            typed: Typed::Text,
            done: Some(None),
            due: Some(DateValue::Written(None)),
            ..view(0)
        };
        let (rows, caret) = render(&view, 40, 14);
        assert_eq!(
            rows[1..13],
            [
                "┌ Edit Node ───────────────────────────┐",
                "│▌ Text     Milk                       │",
                "│  Type     ¶ Note                     │",
                "│  Done     ✔ Done                     │",
                "│  Parent   ⌂ Home                     │",
                "│  Start    None                       │",
                "│  Due       not a date                │",
                "│  Starred  Not starred                │",
                "│  Flagged  Not flagged                │",
                "│  Tags     + Add tag                  │",
                "│                                      │",
                "│ Type the text · Tab next · Enter sav…│",
            ]
        );
        assert_eq!(caret, Some((14, 2)), "in the text, after Mi");
    }

    // editing.md §7, FR-TAG-03, D-46
    #[test]
    fn draws_set_values_in_their_chip_colors_and_unset_ones_dimmed() {
        let set = NodeEditorView {
            start: Some(DateValue::Date(date(2026, 10, 8))),
            due: Some(DateValue::Date(date(2026, 10, 4))),
            starred: true,
            flagged: true,
            ..view(0)
        };
        let icon = |icon: Icon| icon.glyph(IconSet::Unicode);
        assert_eq!(
            value_style(&set, "Start"),
            (
                format!("{} Thu 8 Oct 2026 · Thu", icon(Icon::Start)),
                start_style(&THEME)
            )
        );
        assert_eq!(
            value_style(&set, "Due"),
            (
                format!("{} Sun 4 Oct 2026 · yesterday", icon(Icon::Overdue)),
                overdue_style(&THEME)
            ),
            "overdue"
        );
        assert_eq!(value_style(&set, "Starred").1, today_style(&THEME));
        assert_eq!(value_style(&set, "Flagged").1, flag_style(&THEME));
        let done = NodeEditorView {
            done: Some(None),
            ..set
        };
        assert_eq!(
            value_style(&done, "Due"),
            (
                format!("{} Sun 4 Oct 2026 · yesterday", icon(Icon::Due)),
                due_style(&THEME)
            ),
            "a done block is overdue no longer"
        );
        let due_today = NodeEditorView {
            due: Some(DateValue::Date(TODAY)),
            ..view(0)
        };
        assert_eq!(
            value_style(&due_today, "Due").1,
            due_style(&THEME),
            "due today is not overdue"
        );
        let unset = view(0);
        for (label, value) in [
            ("Done", "Not done"),
            ("Start", "None"),
            ("Due", "None"),
            ("Starred", "Not starred"),
            ("Flagged", "Not flagged"),
            ("Tags", "+ Add tag"),
        ] {
            assert_eq!(
                value_style(&unset, label),
                (value.to_owned(), Style::new().dim()),
                "{label}"
            );
        }
    }

    // editing.md §7
    #[test]
    fn names_the_keys_of_the_focused_row() {
        let hints: Vec<String> = (1..9).map(|focus| hint(&view(focus))).collect();
        assert_eq!(
            hints,
            [
                "│ Space or ←/→ change · p t n · Enter save · Esc cancel        │",
                "│ Space or x toggles · Enter save · Esc cancel                 │",
                "│ Space or type to choose · Enter save · Esc cancel            │",
                "│ Type or ←/→ a date · Del clears · Enter save · Esc cancel    │",
                "│ Type or ←/→ a date · Del clears · Enter save · Esc cancel    │",
                "│ Space or * toggles · Enter save · Esc cancel                 │",
                "│ Space or ! toggles · Enter save · Esc cancel                 │",
                "│ Type a tag: name or name(value) · Enter save · Esc cancel    │",
            ]
        );
        let tagged = NodeEditorView {
            rows: vec![NodeRow::Tag(0), NodeRow::AddTag],
            tags: vec![TagView {
                name: "x",
                value: None,
                supported: true,
            }],
            ..view(0)
        };
        assert_eq!(
            hint(&tagged),
            "│ Space or type to edit · Del removes · Enter save · Esc cancel│"
        );
    }

    // editing.md §7, §5
    #[test]
    fn an_open_field_shows_its_text_and_what_enter_would_do() {
        let field = |text: &'static str, outcome: FieldOutcome| FieldView {
            text,
            caret: text.len(),
            selection: None,
            outcome,
        };
        let cases = [
            (
                field("fri", FieldOutcome::Date(Ok(Some(date(2026, 10, 9))))),
                "│ Fri 9 Oct 2026 · Enter sets it",
            ),
            (
                field("", FieldOutcome::Date(Ok(None))),
                "│ Enter removes it · Esc keeps it",
            ),
            (
                field("x?", FieldOutcome::Date(Err(DateError::Empty))),
                "│ no date entered",
            ),
            (
                field(" ", FieldOutcome::Tag(Ok(false))),
                "│ Enter removes it · Esc keeps it",
            ),
            (
                field("x(1)", FieldOutcome::Tag(Ok(true))),
                "│ Enter writes it · Esc keeps it as it was",
            ),
            (
                field("a b", FieldOutcome::Tag(Err(TagError::Name))),
                "│ not a tag name",
            ),
        ];
        for (field, expected) in cases {
            let shown = NodeEditorView {
                field: Some(field),
                ..view(5)
            };
            assert!(hint(&shown).starts_with(expected), "{}", hint(&shown));
        }
        let nothing_to_add = NodeEditorView {
            field: Some(field(" ", FieldOutcome::Tag(Ok(false)))),
            ..view(8)
        };
        let shown = hint(&nothing_to_add);
        assert!(shown.starts_with("│ no tag entered "), "{shown}");
        let (buf, _) = draw(&nothing_to_add, 64, 30);
        let bottom = rows(&buf)
            .iter()
            .rposition(|row| row.starts_with('└'))
            .unwrap();
        assert_eq!(
            buf[(2, u16::try_from(bottom - 1).unwrap())].fg,
            THEME.warning(),
            "in the warning color"
        );
        let tag = NodeEditorView {
            field: Some(field("x(1)", FieldOutcome::Tag(Ok(true)))),
            ..view(8)
        };
        let (rows, caret) = render(&tag, 64, 16);
        assert_eq!(
            rows[11],
            "│▌ Tags     @x(1)                                              │"
        );
        assert_eq!(caret, Some((17, 11)), "after the @ and the text");
        let start = NodeEditorView {
            field: Some(field(
                "fri",
                FieldOutcome::Date(Ok(Some(date(2026, 10, 9)))),
            )),
            ..view(4)
        };
        let (rows, caret) = render(&start, 64, 16);
        assert_eq!(
            rows[7],
            "│▌ Start    fri                                                │"
        );
        assert_eq!(caret, Some((15, 7)));
    }

    // editing.md §7, ui.md §1
    #[test]
    fn a_value_too_wide_ends_in_an_ellipsis_keeping_the_parent_path_end_and_tag_markers() {
        let view = NodeEditorView {
            text: "A task whose text is far too long to fit",
            parent: Some(vec![
                "Alpha project with a long name".to_owned(),
                "Beta subproject also long".to_owned(),
                "Gamma".to_owned(),
            ]),
            due: Some(DateValue::Written(Some("some day when the moon is blue"))),
            rows: vec![
                NodeRow::Text,
                NodeRow::Parent,
                NodeRow::Due,
                NodeRow::Tag(0),
                NodeRow::AddTag,
            ],
            tags: vec![TagView {
                name: "tag",
                value: Some("a very long value that goes on and on"),
                supported: false,
            }],
            hidden: vec![HiddenData {
                tags: vec![
                    "@hidden".to_owned(),
                    "@log".to_owned(),
                    "@archive".to_owned(),
                ],
                lines: 12,
            }],
            ..view(4)
        };
        let (rows, _) = render(&view, 50, 12);
        assert_eq!(
            rows[1..11],
            [
                "┌ Edit Node ─────────────────────────────────────┐",
                "│  Text     A task whose text is far too long to…│",
                "│  Parent   … › Beta subproject also long › Gamma│",
                "│  Due      some day when the moon is… not a date│",
                "│  Tags     @tag(a very long value t… unsupported│",
                "│▌          + Add tag                            │",
                "│  Hidden   @hidden @log @archive · 12 lines     │",
                "│                                                │",
                "│ Type a tag: name or name(value) · Enter save ·…│",
                "└────────────────────────────────────────────────┘",
            ]
        );
    }

    // editing.md §7
    #[test]
    fn scrolls_to_keep_the_focused_row_in_view() {
        let (rows, caret) = render(&view(8), 40, 8);
        assert_eq!(
            rows,
            [
                "┌ Edit Node ───────────────────────────┐",
                "│  Due      None                       │",
                "│  Starred  Not starred                │",
                "│  Flagged  Not flagged                │",
                "│▌ Tags     + Add tag                  │",
                "│                                      │",
                "│ Type a tag: name or name(value) · En…│",
                "└──────────────────────────────────────┘",
            ]
        );
        assert_eq!(caret, None);
        let hidden = NodeEditorView {
            hidden: vec![
                HiddenData {
                    tags: vec!["@hidden".to_owned()],
                    lines: 1,
                },
                HiddenData {
                    tags: vec!["@log".to_owned()],
                    lines: 2,
                },
            ],
            ..view(8)
        };
        let (rows, _) = render(&hidden, 40, 8);
        assert_eq!(
            rows[1..5],
            [
                "│  Flagged  Not flagged                │",
                "│▌ Tags     + Add tag                  │",
                "│  Hidden   @hidden · 1 line           │",
                "│           @log · 2 lines             │",
            ],
            "the last row shows the special nodes after it"
        );
        let (rows, caret) = render(&hidden, 40, 4);
        assert_eq!(
            rows[1], "│▌ Tags     + Add tag                  │",
            "but stays in view itself"
        );
        assert_eq!(caret, None);
        let (rows, _) = render(&NodeEditorView { focus: 7, ..hidden }, 40, 8);
        assert_eq!(
            rows[4], "│▌ Flagged  Not flagged                │",
            "another row does not scroll past itself"
        );
        let (rows, caret) = render(&view(0), 40, 3);
        assert_eq!(
            rows[1], "│▌ Text     Milk                       │",
            "no room for the hint"
        );
        assert_eq!(caret, Some((14, 1)));
    }
}
