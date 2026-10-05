//! Widgets that draw the core's view state ([ui.md §2](../../../docs/specs/ui.md#2-blocks)).

use std::ops::Range;

use jiff::civil::Date;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Clear, Widget},
};

use unicode_segmentation::UnicodeSegmentation;

use crate::{
    app::{
        Access, ColorPickerView, DatePromptView, DiffLine, Editor, Notice, Row, SaveState,
        TypeMenuView,
    },
    document::{
        BlockKind, DateTag, TagColor,
        dates::DateError,
        inline::{self, Format},
    },
};

/// The outline: a gutter that marks the focused block, whose line gets a brighter background, then
/// one line per block, 2 columns of indent per level, a marker and the text, with its date chips
/// at the right edge. A block being edited shows the text as typed instead, without chips.
#[derive(Debug)]
pub struct OutlineView<'a> {
    rows: &'a [Row],
    today: Date,
    offset: usize,
    focused: Option<usize>,
    editor: Option<&'a Editor<'a>>,
}

impl<'a> OutlineView<'a> {
    /// A view of `rows` on `today`, the local date, drawn from the top of its area with no row
    /// focused.
    #[must_use]
    pub fn new(rows: &'a [Row], today: Date) -> Self {
        Self {
            rows,
            today,
            offset: 0,
            focused: None,
            editor: None,
        }
    }

    /// Starts drawing at row `offset` of the outline instead of its first row.
    #[must_use]
    pub fn offset(self, offset: usize) -> Self {
        Self { offset, ..self }
    }

    /// Marks row `index` in the gutter and gives it the current-line band (ui.md §2).
    #[must_use]
    pub fn focused(self, index: Option<usize>) -> Self {
        Self {
            focused: index,
            ..self
        }
    }

    /// Draws the block `editor` edits as its editor (FR-MODE-02).
    #[must_use]
    pub fn editor(self, editor: Option<&'a Editor<'a>>) -> Self {
        Self { editor, ..self }
    }
}

impl Widget for OutlineView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [gutter, text] = area.layout(&Layout::horizontal([
            Constraint::Length(GUTTER_WIDTH),
            Constraint::Fill(1),
        ]));
        let width = usize::from(text.width);
        let lines = self.rows.iter().enumerate().map(|(index, row)| {
            let (line, chips) = match self.editor.filter(|editor| editor.id == row.id) {
                Some(editor) => (editor_line(row, editor, width).0, Line::default()),
                None => (row_line(row, width), chips(row, self.today)),
            };
            (index, line, chips)
        });
        for ((index, line, chips), (gutter_area, line_area)) in
            lines.skip(self.offset).zip(gutter.rows().zip(text.rows()))
        {
            let focused = Some(index) == self.focused;
            // The line patches its own styles over the band, so every foreground color and
            // modifier survives on the focused row.
            if focused {
                buf.set_style(gutter_area.union(line_area), current_line_style());
            }
            line.render(line_area, buf);
            chips.render(line_area, buf);
            if focused {
                Span::styled(FOCUS_MARKER, Style::new().fg(PRIMARY)).render(gutter_area, buf);
            }
        }
    }
}

/// Columns left of the outline for the focus marker; the breadcrumbs start after it too, so the
/// root crumb lines up with top-level blocks (D-35).
pub const GUTTER_WIDTH: u16 = 2;

/// Marks the focused block in the gutter (D-30).
const FOCUS_MARKER: &str = "▎";

/// Stands for the document root in the breadcrumbs (D-32).
const HOME_ICON: &str = "⌂";

/// The breadcrumb header for a view zoomed into the last of `crumbs`: `⌂`, then each crumb,
/// dropping crumbs from the left behind `…` until it fits in `width` columns or only the last
/// one is left (navigation §4). Bold, as the focused pane's header (ui.md §1).
#[must_use]
pub fn breadcrumbs(crumbs: &[String], width: usize) -> Line<'static> {
    let shown: Vec<String> = crumbs.iter().map(|crumb| shown_text(crumb)).collect();
    let path: Vec<&str> = std::iter::once(HOME_ICON)
        .chain(shown.iter().map(String::as_str))
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

/// What the status line says after the file's path (ui.md §1, file-io.md §1–3): `●` while changes
/// are unsaved, an error badge when the last save failed, `read-only`, `not on disk` when the file
/// was deleted or moved away, the question a failed save on quit asks, and the current notice.
#[must_use]
pub fn file_status(
    save: SaveState,
    access: Access,
    gone: bool,
    confirming_quit: bool,
    notice: Option<&Notice>,
) -> Vec<Span<'static>> {
    let mut spans = match save {
        SaveState::Saved => Vec::new(),
        SaveState::Unsaved => vec![Span::raw(" ●")],
        SaveState::Failed { reason } => vec![
            Span::raw(" ● "),
            Span::styled(format!("save failed: {reason}"), error_style()),
        ],
    };
    if access == Access::ReadOnly {
        spans.push(Span::raw(" · read-only"));
    }
    if gone {
        spans.push(Span::raw(" · not on disk"));
    }
    if confirming_quit {
        spans.push(Span::raw(" · "));
        spans.push(Span::styled(
            "Ctrl+Q again quits without saving",
            Style::new().bold(),
        ));
    }
    if let Some(notice) = notice {
        spans.push(Span::raw(" · "));
        spans.push(notice_span(notice));
    }
    spans
}

/// The status line's words for `notice`.
fn notice_span(notice: &Notice) -> Span<'static> {
    match notice {
        Notice::BackupFailed { reason } => {
            Span::styled(format!("backup failed: {reason}"), warning_style())
        }
        Notice::ReadOnly => Span::styled("cannot edit a read-only file", warning_style()),
        Notice::TypedTag => Span::styled("Tags are set with commands", warning_style()),
        Notice::EditFailed { reason } => {
            Span::styled(format!("edit failed: {reason}"), error_style())
        }
        Notice::Reloaded => Span::raw("Reloaded (changed on disk)"),
        Notice::KeptMine { copy } => Span::raw(format!(
            "Kept your version; the one on disk is in {}",
            copy.display()
        )),
        Notice::TookTheirs { copy } => Span::raw(format!(
            "Took the version on disk; yours is in {}",
            copy.display()
        )),
        Notice::ConflictCopyFailed { reason } => {
            Span::styled(format!("conflict copy failed: {reason}"), error_style())
        }
        Notice::CannotTakeTheirs { reason } => Span::styled(
            format!("cannot take the version on disk: {reason}"),
            error_style(),
        ),
        Notice::BlanksLifted => Span::raw("Empty lines at the end moved to the top level"),
        Notice::Bookmark { added: true } => Span::raw("Bookmarked"),
        Notice::Bookmark { added: false } => Span::raw("Bookmark removed"),
        Notice::OpenUrlFailed { url, reason } => {
            Span::styled(format!("cannot open {url}: {reason}"), error_style())
        }
    }
}

/// The widest and, with the diff, tallest the conflict prompt gets (ui.md §1).
const PROMPT_SIZE: (u16, u16) = (80, 20);

/// The conflict prompt (file-io.md §3): a box over the middle of the screen saying the file
/// changed on disk while there were unsaved changes, with the keys of the three choices. With the
/// diff, the box is as large as prompts get and lists the diff from line `scroll` on.
#[derive(Debug)]
pub struct ConflictPrompt<'a> {
    diff: Option<&'a [DiffLine]>,
    scroll: usize,
}

impl<'a> ConflictPrompt<'a> {
    /// The prompt, showing `diff` from line `scroll` on when there is one.
    #[must_use]
    pub fn new(diff: Option<&'a [DiffLine]>, scroll: usize) -> Self {
        Self { diff, scroll }
    }

    /// How many diff lines fit at once when the prompt is drawn over `area`.
    #[must_use]
    pub fn diff_rows(area: Rect) -> usize {
        usize::from(area.height.min(PROMPT_SIZE.1).saturating_sub(4))
    }
}

impl Widget for ConflictPrompt<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let height = if self.diff.is_some() {
            PROMPT_SIZE.1
        } else {
            4
        };
        let area = area.centered(
            Constraint::Length(PROMPT_SIZE.0),
            Constraint::Length(height),
        );
        Clear.render(area, buf);
        let frame = Block::bordered().title(" The file changed on disk ");
        let inner = frame.inner(area);
        frame.render(area, buf);
        let [top, body, keys] = inner.layout(&Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ]));
        let toggle = match self.diff {
            None => {
                Line::from("Your changes are not saved yet.").render(top, buf);
                "show diff"
            }
            Some(diff) => {
                Line::from(vec![
                    Span::styled("- on disk", disk_style()),
                    Span::raw("  "),
                    Span::styled("+ yours", mine_style()),
                ])
                .render(top, buf);
                for (line, row) in diff.iter().skip(self.scroll).zip(body.rows()) {
                    diff_line(line).render(row, buf);
                }
                "hide diff"
            }
        };
        Line::from(vec![
            "m".bold(),
            " keep mine · ".into(),
            "t".bold(),
            " take theirs · ".into(),
            "d".bold(),
            format!(" {toggle}").into(),
        ])
        .render(keys, buf);
    }
}

/// A menu over the outline ([ui.md §5](../../../docs/specs/ui.md#5-context-menu-fr-key-06)): a
/// box with a title listing the entries the filter matches, the highlighted one reversed, with
/// the filter at the bottom once one is typed. It shows the type menu (D-44) and the color picker
/// (FR-TAG-05).
#[derive(Debug)]
pub struct MenuPopup {
    title: &'static str,
    entries: Vec<Line<'static>>,
    filter: String,
}

impl MenuPopup {
    /// The type menu, titled "Node type:", with `/` before its filter.
    #[must_use]
    pub fn types(menu: &TypeMenuView) -> Self {
        let names = menu.entries.iter().map(|entry| (entry.name, Style::new()));
        Self {
            title: "Node type:",
            entries: entries(names, menu.selected),
            filter: if menu.filter.is_empty() {
                String::new()
            } else {
                format!("/{}", menu.filter)
            },
        }
    }

    /// The color picker, titled "Color:", each color's name in its tint.
    #[must_use]
    pub fn colors(picker: &ColorPickerView) -> Self {
        let names = picker
            .entries
            .iter()
            .map(|&(color, name)| (name, tint(Style::new(), color)));
        Self {
            title: "Color:",
            entries: entries(names, picker.selected),
            filter: picker.filter.to_owned(),
        }
    }

    /// The popup's width and height: its widest entry or its title, which is wider than any
    /// filter that matches an entry, and a line per entry, with the border.
    #[must_use]
    pub fn size(&self) -> (u16, u16) {
        let widest = self
            .entries
            .iter()
            .map(Line::width)
            .chain([Line::from(self.title).width()])
            .max()
            .unwrap_or_default();
        let width = u16::try_from(widest).unwrap_or(u16::MAX).saturating_add(2);
        let height = u16::try_from(self.entries.len())
            .unwrap_or(u16::MAX)
            .saturating_add(2);
        (width, height)
    }
}

/// A menu's entry lines: each name padded by a space and in its style, the one at `selected`
/// reversed.
fn entries<'a>(
    names: impl Iterator<Item = (&'a str, Style)>,
    selected: usize,
) -> Vec<Line<'static>> {
    names
        .enumerate()
        .map(|(index, (name, style))| {
            let line = Line::styled(format!(" {name} "), style);
            if index == selected {
                line.reversed()
            } else {
                line
            }
        })
        .collect()
}

impl Widget for MenuPopup {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let frame = Block::bordered()
            .title(self.title)
            .title_bottom(self.filter);
        let inner = frame.inner(area);
        frame.render(area, buf);
        for (line, row) in self.entries.into_iter().zip(inner.rows()) {
            buf.set_style(row, line.style);
            line.render(row, buf);
        }
    }
}

/// The date prompt ([editing.md §5](../../../docs/specs/editing.md#5-block-commands), ui.md §1):
/// a box over the middle of the screen titled with the date it sets, holding the text as typed
/// with the selection reversed and, below it, what `Enter` does: set the date the text resolves
/// to, remove the date, or nothing, saying why the text is not a date.
#[derive(Debug)]
pub struct DatePromptPopup<'a> {
    prompt: &'a DatePromptView<'a>,
}

impl<'a> DatePromptPopup<'a> {
    /// The box's width and height, the border included.
    const SIZE: (u16, u16) = (40, 4);

    /// The popup showing `prompt`.
    #[must_use]
    pub fn new(prompt: &'a DatePromptView<'a>) -> Self {
        Self { prompt }
    }

    /// Where the box goes over `area`: in its middle, and no larger than it.
    #[must_use]
    pub fn area(area: Rect) -> Rect {
        area.centered(
            Constraint::Length(Self::SIZE.0),
            Constraint::Length(Self::SIZE.1),
        )
    }

    /// The caret's cell when the box is drawn over `area`, on the text's line, the first inside
    /// the border.
    #[must_use]
    pub fn caret(&self, area: Rect) -> (u16, u16) {
        let inner = Block::bordered().inner(area);
        let (_, column) = self.input(inner.width);
        let x = u16::try_from(column)
            .unwrap_or(u16::MAX)
            .min(inner.width.saturating_sub(1));
        (inner.x.saturating_add(x), inner.y)
    }

    /// The text after a space, `width` columns wide, and the caret's column on it.
    fn input(&self, width: u16) -> (Line<'a>, usize) {
        let field = Field {
            text: self.prompt.text,
            caret: self.prompt.caret,
            selection: self.prompt.selection.clone(),
        };
        let (spans, column) = field.spans(Style::new(), 1, usize::from(width));
        let mut line = Line::from(" ");
        line.spans.extend(spans);
        (line, column)
    }

    /// What `Enter` does with the text: set the date, remove it, or nothing, quoting where the
    /// text stops being a date.
    fn preview(&self) -> Line<'static> {
        let from = |offset: usize| self.prompt.text.get(offset..).unwrap_or_default();
        let error = |why: String| Line::styled(format!(" {why}"), error_style());
        match self.prompt.date {
            Ok(Some(date)) => Line::from(format!(" {}", date.strftime("%a %-d %b %Y"))),
            Ok(None) | Err(DateError::Empty) => {
                Line::styled(" No date: Enter removes it", Style::new().dim())
            }
            Err(DateError::Unexpected { offset }) if from(offset).is_empty() => {
                error("Not a date: something is missing".to_owned())
            }
            Err(DateError::Unexpected { offset }) => {
                error(format!("Not a date from “{}”", from(offset)))
            }
            Err(DateError::NoSuchDay { offset }) => {
                error(format!("No such day: “{}”", from(offset)))
            }
            Err(DateError::OutOfRange { .. }) => error("Out of range".to_owned()),
        }
    }
}

impl Widget for DatePromptPopup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let title = match self.prompt.tag {
            DateTag::Start => " Start date ",
            DateTag::Due => " Due date ",
        };
        let frame = Block::bordered().title(title);
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

/// One line of the diff, marked and colored by the version it is in, with each tab of its
/// indentation drawn as 2 columns like a level of the outline.
fn diff_line(line: &DiffLine) -> Line<'static> {
    let shown = |text: &str| text.replace('\t', "  ");
    match line {
        DiffLine::Same(text) => Line::from(format!("  {}", shown(text))),
        DiffLine::Disk(text) => Line::styled(format!("- {}", shown(text)), disk_style()),
        DiffLine::Mine(text) => Line::styled(format!("+ {}", shown(text)), mine_style()),
        DiffLine::Gap => Line::styled("⋯", rule_style()),
    }
}

/// Lines only the file on disk has; themes arrive in step 6.2.
fn disk_style() -> Style {
    Style::new().fg(Color::Red)
}

/// Lines only the local version has; themes arrive in step 6.2.
fn mine_style() -> Style {
    Style::new().fg(Color::Green)
}

/// `text` as navigate mode shows it, without the delimiters of its inline formatting
/// (FR-FMT-01).
fn shown_text(text: &str) -> String {
    inline::runs(text).iter().map(|run| run.text).collect()
}

/// The spans of `text` in `style` with its inline formatting applied over it: bold, code in the
/// code color and URLs underlined, with the delimiters hidden (FR-FMT-01, D-10).
fn formatted(text: &str, style: Style) -> impl Iterator<Item = Span<'_>> {
    inline::runs(text).into_iter().map(move |run| {
        let Format { bold, code, link } = run.format;
        let mut style = style;
        if bold {
            style = style.bold();
        }
        if code {
            style = style.fg(CODE);
        }
        if link {
            style = style.underlined();
        }
        Span::styled(run.text, style)
    })
}

/// The screen line of one block, `width` columns wide: indented 2 columns per level of depth
/// whatever its type, then the marker of a task, then the text with its inline formatting
/// (D-31, FR-FMT-01). A collapsed block
/// with children shows `▸ n` after its text (ui.md §2). The zoom title is drawn like its block,
/// in bold (D-34).
fn row_line(row: &Row, width: usize) -> Line<'_> {
    let (marker, marker_style, style) = row_style(row);
    let mut line = Line::from(vec![
        Span::raw("  ".repeat(row.depth)),
        Span::styled(marker, marker_style),
    ]);
    match row.kind {
        BlockKind::Divider => {}
        // TaskPaper marks a project with a trailing colon, which the display text drops.
        BlockKind::Project => {
            line.extend(formatted(&row.text, style));
            line.push_span(Span::styled(":", style));
        }
        _ => line.extend(formatted(&row.text, style)),
    }
    if row.hidden_children > 0 {
        line.push_span(Span::styled(
            format!(" ▸ {}", row.hidden_children),
            Style::new().dim(),
        ));
        if row.kind == BlockKind::Divider {
            line.push_span(" ");
        }
    }
    if row.kind == BlockKind::Divider {
        line.push_span(rule(&line, width));
    }
    line
}

/// The chips of `row` on `today`, right-aligned, each after a space: `★` when it has `@today`
/// (FR-TAG-04), then its start and due dates as `start Fri 8 Oct` and `due Fri 8 Oct`, with the year when it is not this year's, an overdue
/// date in the error color, and dimmed with a done block or one inside it (ui.md §2, FR-TAG-03).
fn chips(row: &Row, today: Date) -> Line<'static> {
    let receded = row.done || row.inside_done;
    let recede = |style: Style| if receded { style.dim() } else { style };
    let chip = |label: &str, date: Date, style: Style| {
        let format = if date.year() == today.year() {
            "%a %-d %b"
        } else {
            "%a %-d %b %Y"
        };
        Span::styled(format!(" {label} {}", date.strftime(format)), recede(style))
    };
    let due_style = if row.overdue(today) {
        error_style()
    } else {
        chip_style()
    };
    let spans: Vec<Span> = [
        row.today
            .then(|| Span::styled(format!(" {TODAY_MARKER}"), recede(today_style()))),
        row.start.map(|date| chip("start", date, chip_style())),
        row.due.map(|date| chip("due", date, due_style)),
    ]
    .into_iter()
    .flatten()
    .collect();
    Line::from(spans).right_aligned()
}

/// Marks a block that has `@today` (ui.md §2).
const TODAY_MARKER: &str = "★";

/// The `@today` marker chip; themes arrive in step 6.2.
fn today_style() -> Style {
    Style::new().fg(Color::Yellow)
}

/// Date chips that are not overdue; themes arrive in step 6.2.
fn chip_style() -> Style {
    Style::new().fg(Color::Cyan)
}

/// The marker of a block with its style, and the style of its text, by its type, its color and
/// whether it is done or inside a done block (ui.md §2).
fn row_style(row: &Row) -> (&'static str, Style, Style) {
    let (marker, style) = match row.kind {
        BlockKind::Project => ("", Style::new().bold().fg(PRIMARY)),
        BlockKind::Task if row.done => ("✔ ", Style::new()),
        BlockKind::Task => ("- ", Style::new()),
        BlockKind::Url => ("", Style::new().dim().underlined().cyan()),
        BlockKind::Section => ("", Style::new().bold().fg(SECONDARY)),
        BlockKind::Text => ("", Style::new().fg(TEXT)),
        BlockKind::Divider => ("", divider_style()),
    };
    let style = tint(if row.title { style.bold() } else { style }, row.color);
    // A done block recedes and is struck through, and everything inside it recedes; markers
    // recede with their text (D-36, D-40).
    if row.done {
        (marker, Style::new().dim(), style.dim().crossed_out())
    } else if row.inside_done {
        (marker, Style::new().dim(), style.dim())
    } else {
        (marker, Style::new(), style)
    }
}

/// The screen line of `row` while `editor` edits it, `width` columns wide, and the caret's column
/// on it: the row's indent and marker, then the text with the selection reversed and a project's
/// `:` after it, all styled as the row (FR-MODE-02). When the caret would be past the right edge,
/// the text scrolls left by whole graphemes to keep it in view.
#[must_use]
pub fn editor_line<'a>(row: &Row, editor: &'a Editor, width: usize) -> (Line<'a>, usize) {
    let (marker, marker_style, style) = row_style(row);
    let mut line = Line::from(vec![
        Span::raw("  ".repeat(row.depth)),
        Span::styled(marker, marker_style),
    ]);
    let field = Field {
        text: editor.text,
        caret: editor.caret,
        selection: editor.selection.clone(),
    };
    let (spans, column) = field.spans(style, line.width(), width);
    line.spans.extend(spans);
    line.push_span(Span::styled(editor.suffix, style));
    (line, column)
}

/// A text being typed, with its caret and selection as byte offsets.
struct Field<'a> {
    text: &'a str,
    caret: usize,
    selection: Option<Range<usize>>,
}

impl<'a> Field<'a> {
    /// The text styled `style` with the selection reversed, for a line `width` columns wide on
    /// which it starts at column `start`, and the caret's column on that line. When the caret
    /// would be past the right edge, the text scrolls left by whole graphemes to keep it in view.
    fn spans(&self, style: Style, start: usize, width: usize) -> (Vec<Span<'a>>, usize) {
        let text = self.text;
        let selection = self.selection.clone().unwrap_or(self.caret..self.caret);
        let before_caret = text.get(..self.caret).unwrap_or_default();
        let column = start + Span::raw(before_caret).width();
        // The caret takes a column of its own after the text before it.
        let overflow = (column + 1).saturating_sub(width);
        let (mut from, mut scrolled) = (0, 0);
        for (at, grapheme) in text.grapheme_indices(true) {
            if scrolled >= overflow || at >= before_caret.len() {
                break;
            }
            from = at + grapheme.len();
            scrolled += Span::raw(grapheme).width();
        }
        let local = |at: usize| at.min(text.len()).max(from);
        let selected = local(selection.start)..local(selection.end);
        let piece = |range: Range<usize>| text.get(range).unwrap_or_default();
        let spans = vec![
            Span::styled(piece(from..selected.start), style),
            Span::styled(piece(selected.clone()), style.reversed()),
            Span::styled(piece(selected.end..text.len()), style),
        ];
        (spans, column - scrolled)
    }
}

/// A horizontal rule from the end of `line` to the right edge, very dim.
fn rule(line: &Line, width: usize) -> Span<'static> {
    Span::styled(
        "─".repeat(width.saturating_sub(line.width())),
        divider_style(),
    )
}

/// `style` with the text in `color`'s tint, if there is one (FR-TAG-05); themes arrive in step
/// 6.2, until when orange is the 256-color orange 208 and violet the terminal's magenta.
fn tint(style: Style, color: Option<TagColor>) -> Style {
    let Some(color) = color else {
        return style;
    };
    style.fg(match color {
        TagColor::Red => Color::Red,
        TagColor::Orange => Color::Indexed(208),
        TagColor::Yellow => Color::Yellow,
        TagColor::Green => Color::Green,
        TagColor::Blue => Color::Blue,
        TagColor::Violet => Color::Magenta,
    })
}

/// Divider blocks: the rule-lines color, dimmed further so they recede behind the header rule
/// (D-33).
fn divider_style() -> Style {
    rule_style().dim()
}

/// The theme's accent (Textual's `$primary`), for projects; themes arrive in step 6.2.
const PRIMARY: Color = Color::LightBlue;

/// Plain text: dimmed, but less than done tasks and the other dim blocks (D-37); themes arrive in
/// step 6.2.
const TEXT: Color = Color::Gray;

/// Inline code (FR-FMT-01); themes arrive in step 6.2.
const CODE: Color = Color::LightYellow;

/// The theme's headers color (Textual's `$secondary`), for sections; themes arrive in step 6.2.
const SECONDARY: Color = Color::LightMagenta;

/// Rule lines: dividers and the line under the header; themes arrive in step 6.2.
#[must_use]
pub fn rule_style() -> Style {
    Style::new().fg(Color::DarkGray)
}

/// Errors, such as a failed save; themes arrive in step 6.2.
fn error_style() -> Style {
    Style::new().fg(Color::Red).bold()
}

/// Warnings, such as a failed backup; themes arrive in step 6.2.
fn warning_style() -> Style {
    Style::new().fg(Color::Yellow)
}

/// The band behind every line of the focused block, the full width of the pane, a step brighter
/// than a dark terminal's background (D-39); themes arrive in step 6.2.
fn current_line_style() -> Style {
    Style::new().bg(Color::Indexed(235))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::{Message, State},
        document::format,
    };

    const TODAY: Date = jiff::civil::date(2026, 10, 5);

    /// Draws `rows` with nothing focused and returns the `width` × `height` columns right of
    /// the gutter.
    fn draw(rows: &[Row], width: u16, height: u16) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, width + GUTTER_WIDTH, height));
        OutlineView::new(rows, TODAY).render(buf.area, &mut buf);
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
        OutlineView::new(&rows, TODAY)
            .offset(offset)
            .focused(focused)
            .render(area, &mut buf);
        buf
    }

    #[test]
    fn draws_markers_indents_projects_and_dim_text() {
        let mut expected = Buffer::with_lines([
            "Home:        ",
            "  - Milk     ",
            "    on sale  ",
            "note         ",
        ]);
        expected.set_style(Rect::new(0, 0, 5, 1), Style::new().bold().fg(PRIMARY));
        expected.set_style(Rect::new(4, 2, 7, 1), Style::new().fg(TEXT));
        expected.set_style(Rect::new(0, 3, 4, 1), Style::new().fg(TEXT));
        assert_eq!(
            render("Home:\n\t- Milk\n\t\ton sale\nnote", 13, 4),
            expected
        );
    }

    // FR-DONE-01, D-40
    #[test]
    fn done_blocks_are_dimmed_and_struck_through_and_done_tasks_checked() {
        let mut expected = Buffer::with_lines(["✔ a ", "- b ", "P:  ", "n   "]);
        expected.set_style(Rect::new(0, 0, 2, 1), Style::new().dim());
        expected.set_style(Rect::new(2, 0, 1, 1), Style::new().dim().crossed_out());
        let struck = |style: Style| style.dim().crossed_out();
        expected.set_style(
            Rect::new(0, 2, 2, 1),
            struck(Style::new().bold().fg(PRIMARY)),
        );
        expected.set_style(Rect::new(0, 3, 1, 1), struck(Style::new().fg(TEXT)));
        assert_eq!(
            render("- a @done\n- b\nP: @done\nn @done(2026-10-01)", 4, 4),
            expected
        );
        let mut expected = Buffer::with_lines(["S     ", "u     ", "──────", "f()   "]);
        expected.set_style(
            Rect::new(0, 0, 1, 1),
            struck(Style::new().bold().fg(SECONDARY)),
        );
        expected.set_style(
            Rect::new(0, 1, 1, 1),
            struck(Style::new().underlined().cyan()),
        );
        expected.set_style(Rect::new(0, 2, 6, 1), divider_style());
        expected.set_style(Rect::new(0, 3, 3, 1), struck(Style::new().fg(TEXT)));
        assert_eq!(
            render(
                "S @section @done\nu @url(x) @done\n--- @done\nf() @code @done",
                6,
                4
            ),
            expected
        );
    }

    // FR-DONE-01, D-40
    #[test]
    fn every_block_inside_a_done_block_is_dimmed_but_not_struck_through() {
        let text = "P: @done\n\t- a\n\t\tn\n\tS @section\n\tu @url(x)\n\t---\n\tf() @code\n- b";
        let mut expected = Buffer::with_lines([
            "P:      ",
            "  - a   ",
            "    n   ",
            "  S     ",
            "  u     ",
            "  ──────",
            "  f()   ",
            "- b     ",
        ]);
        let dim = Style::new().dim();
        expected.set_style(
            Rect::new(0, 0, 2, 1),
            Style::new().bold().fg(PRIMARY).dim().crossed_out(),
        );
        expected.set_style(Rect::new(2, 1, 3, 1), dim);
        expected.set_style(Rect::new(4, 2, 1, 1), Style::new().fg(TEXT).dim());
        expected.set_style(
            Rect::new(2, 3, 1, 1),
            Style::new().bold().fg(SECONDARY).dim(),
        );
        expected.set_style(
            Rect::new(2, 4, 1, 1),
            Style::new().dim().underlined().cyan(),
        );
        expected.set_style(Rect::new(2, 5, 6, 1), divider_style());
        expected.set_style(Rect::new(2, 6, 3, 1), Style::new().fg(TEXT).dim());
        assert_eq!(render(text, 8, 8), expected);
    }

    /// The date prompt showing `text`, selected, with `date` as what it resolves to, drawn over
    /// 30 × 6 cells.
    fn date_prompt(text: &str, date: Result<Option<Date>, DateError>) -> Buffer {
        let view = DatePromptView {
            tag: DateTag::Start,
            text,
            caret: text.len(),
            selection: Some(0..text.len()),
            date,
        };
        let popup = DatePromptPopup::new(&view);
        let area = Rect::new(0, 0, 30, 6);
        let mut buf = Buffer::empty(area);
        assert_eq!(
            popup.caret(DatePromptPopup::area(area)),
            (2 + text_width(text), 2)
        );
        popup.render(DatePromptPopup::area(area), &mut buf);
        buf
    }

    fn text_width(text: &str) -> u16 {
        u16::try_from(text.len()).unwrap()
    }

    // FR-TAG-03, ui.md §1: as wide as the area allows and in its middle.
    #[test]
    fn the_date_prompt_shows_the_text_selected_and_the_date_it_resolves_to() {
        let mut expected = Buffer::with_lines([
            "",
            "┌ Start date ────────────────┐",
            "│ fri                        │",
            "│ Fri 9 Oct 2026             │",
            "└────────────────────────────┘",
            "",
        ]);
        expected.set_style(Rect::new(2, 2, 3, 1), Style::new().reversed());
        assert_eq!(
            date_prompt("fri", Ok(Some(jiff::civil::date(2026, 10, 9)))),
            expected
        );
    }

    // FR-TAG-03, search.md §4
    #[test]
    fn the_date_prompt_says_why_a_text_is_no_date() {
        let preview = |text: &str, date| {
            let buf = date_prompt(text, date);
            (0..30)
                .map(|x| buf[(x, 3)].symbol().to_owned())
                .collect::<String>()
        };
        assert_eq!(
            preview("", Err(DateError::Empty)),
            "│ No date: Enter removes it  │",
            "a blank text removes the date"
        );
        assert_eq!(
            preview("9999-12-31 +1d", Err(DateError::OutOfRange { offset: 11 })),
            "│ Out of range               │"
        );
        assert_eq!(
            preview("next xyz", Err(DateError::Unexpected { offset: 5 })),
            "│ Not a date from “xyz”      │"
        );
    }

    /// The type menu of a session on `a` after `keys`, drawn at its size.
    fn type_menu(keys: &[crate::app::MenuInput]) -> Buffer {
        let mut state = State::new(format::parse("a"), None);
        state.update(Message::EditAtEnd);
        state.update(Message::OpenTypeMenu);
        for key in keys {
            state.update(Message::TypeMenu(*key));
        }
        let menu = state.type_menu().unwrap();
        let popup = MenuPopup::types(&menu);
        let (width, height) = popup.size();
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        popup.render(buf.area, &mut buf);
        buf
    }

    // D-44, ui.md §5
    #[test]
    fn the_type_menu_lists_the_types_under_its_title_with_the_highlight_reversed() {
        let mut expected = Buffer::with_lines([
            "┌Node type:┐",
            "│ Project  │",
            "│ Section  │",
            "│ Note     │",
            "│ Task     │",
            "│ Divider  │",
            "└──────────┘",
        ]);
        expected.set_style(Rect::new(1, 2, 10, 1), Style::new().reversed());
        assert_eq!(type_menu(&[crate::app::MenuInput::Next]), expected);
    }

    // D-44, ui.md §5
    #[test]
    fn the_type_menu_shows_the_filter_at_the_bottom() {
        use crate::app::MenuInput::Type;
        let mut expected = Buffer::with_lines([
            "┌Node type:┐",
            "│ Project  │",
            "│ Section  │",
            "└/ect──────┘",
        ]);
        expected.set_style(Rect::new(1, 1, 10, 1), Style::new().reversed());
        assert_eq!(type_menu(&[Type('e'), Type('c'), Type('t')]), expected);
    }

    // FR-TAG-05, keybindings.md §4
    #[test]
    fn the_color_picker_names_each_color_in_its_tint() {
        let mut state = State::new(format::parse("- a @blue"), None);
        state.update(Message::OpenColorPicker);
        let picker = state.color_picker().unwrap();
        let popup = MenuPopup::colors(&picker);
        assert_eq!(popup.size(), (10, 9));
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 9));
        popup.render(buf.area, &mut buf);
        let mut expected = Buffer::with_lines([
            "┌Color:──┐",
            "│ Red    │",
            "│ Orange │",
            "│ Yellow │",
            "│ Green  │",
            "│ Blue   │",
            "│ Violet │",
            "│ None   │",
            "└────────┘",
        ]);
        for (y, color) in (1..).zip(TagColor::ALL) {
            expected.set_style(Rect::new(1, y, 8, 1), tint(Style::new(), Some(color)));
        }
        expected.set_style(Rect::new(1, 5, 8, 1), Style::new().reversed());
        assert_eq!(buf, expected);
        state.update(Message::ColorPicker(crate::app::MenuInput::Type('v')));
        let picker = state.color_picker().unwrap();
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 3));
        MenuPopup::colors(&picker).render(buf.area, &mut buf);
        let mut expected = Buffer::with_lines(["┌Color:──┐", "│ Violet │", "└v───────┘"]);
        expected.set_style(
            Rect::new(1, 1, 8, 1),
            Style::new().fg(Color::Magenta).reversed(),
        );
        assert_eq!(buf, expected);
    }

    // FR-TAG-05, D-5
    #[test]
    fn a_color_tag_tints_the_text_of_any_block_but_not_its_marker() {
        let mut expected = Buffer::with_lines(["- a   ", "P:    ", "S     ", "n     ", "✔ d   "]);
        expected.set_style(Rect::new(2, 0, 1, 1), Style::new().fg(Color::Red));
        expected.set_style(
            Rect::new(0, 1, 2, 1),
            Style::new().bold().fg(Color::Indexed(208)),
        );
        expected.set_style(Rect::new(0, 2, 1, 1), Style::new().bold().fg(Color::Yellow));
        expected.set_style(Rect::new(0, 3, 1, 1), Style::new().fg(Color::Green));
        expected.set_style(Rect::new(0, 4, 2, 1), Style::new().dim());
        expected.set_style(
            Rect::new(2, 4, 1, 1),
            Style::new().fg(Color::Blue).dim().crossed_out(),
        );
        assert_eq!(
            render(
                "- a @red\nP: @orange\nS @section @yellow\nn @green\n- d @blue @done",
                6,
                5
            ),
            expected
        );
        let mut expected = Buffer::with_lines(["u     "]);
        expected.set_style(
            Rect::new(0, 0, 1, 1),
            Style::new().dim().underlined().fg(Color::Magenta),
        );
        assert_eq!(render("u @url(x) @violet", 6, 1), expected);
    }

    // FR-TAG-04, ui.md §2
    #[test]
    fn today_shows_as_a_star_chip_before_the_dates() {
        let mut expected = Buffer::with_lines([
            "- a  ★ due Fri 9 Oct",
            "- b                ★",
            "P:                 ★",
            "  - c              ★",
        ]);
        expected.set_style(Rect::new(4, 0, 2, 1), today_style());
        expected.set_style(Rect::new(6, 0, 14, 1), chip_style());
        expected.set_style(Rect::new(18, 1, 2, 1), today_style());
        expected.set_style(
            Rect::new(0, 2, 2, 1),
            Style::new().bold().fg(PRIMARY).dim().crossed_out(),
        );
        expected.set_style(Rect::new(18, 2, 2, 1), today_style().dim());
        expected.set_style(Rect::new(2, 3, 3, 1), Style::new().dim());
        expected.set_style(Rect::new(18, 3, 2, 1), today_style().dim());
        assert_eq!(
            render(
                "- a @due(2026-10-09) @today\n- b @today(x)\nP: @today @done\n\t- c @today",
                20,
                4
            ),
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
        expected.set_style(Rect::new(0, 1, 1, 2), Style::new().fg(TEXT));
        expected.set_style(Rect::new(1, 2, 4, 1), dim);
        expected.set_style(Rect::new(0, 3, 1, 1), Style::new().bold().fg(SECONDARY));
        expected.set_style(Rect::new(1, 3, 4, 1), dim);
        expected.set_style(Rect::new(0, 4, 4, 1), dim);
        expected.set_style(Rect::new(5, 4, 9, 1), divider_style());
        expected.set_style(Rect::new(0, 5, 3, 1), Style::new().fg(TEXT));
        expected.set_style(Rect::new(3, 5, 4, 1), dim);
        assert_eq!(buf, expected);
    }

    // FR-FMT-01, D-10
    #[test]
    fn inline_formatting_is_drawn_over_the_block_style_with_its_delimiters_hidden() {
        let mut expected = Buffer::with_lines([
            "- a b c https://x.y ",
            "P q:                ",
            "✔ d                 ",
            "n `                 ",
        ]);
        expected.set_style(Rect::new(2, 0, 1, 1), Style::new().bold());
        expected.set_style(Rect::new(4, 0, 1, 1), Style::new().fg(CODE));
        expected.set_style(Rect::new(8, 0, 11, 1), Style::new().underlined());
        let project = Style::new().bold().fg(PRIMARY);
        expected.set_style(Rect::new(0, 1, 4, 1), project);
        expected.set_style(Rect::new(2, 1, 1, 1), project.fg(CODE));
        expected.set_style(Rect::new(0, 2, 2, 1), Style::new().dim());
        expected.set_style(
            Rect::new(2, 2, 1, 1),
            Style::new().dim().crossed_out().bold(),
        );
        expected.set_style(Rect::new(0, 3, 3, 1), Style::new().fg(TEXT));
        assert_eq!(
            render(
                "- **a** `b` c https://x.y\nP `q`:\n- **d** @done\nn `",
                20,
                4
            ),
            expected
        );
    }

    // FR-FMT-01, FR-NAV-03
    #[test]
    fn breadcrumbs_hide_the_delimiters_of_inline_formatting() {
        let crumbs = ["**Work**".to_owned(), "`x` y".to_owned()];
        assert_eq!(breadcrumbs(&crumbs, 40).to_string(), "⌂ › Work › x y");
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
    fn the_title_row_is_styled_by_its_type_in_bold_over_its_indented_children() {
        let title = |text: &str| {
            let mut state = State::new(format::parse(text), None);
            state.update(Message::FocusNext);
            state.update(Message::ZoomIn);
            draw(&state.rows(), 6, 3)
        };
        let mut expected = Buffer::with_lines(["m     ", "  - t ", "      "]);
        expected.set_style(Rect::new(0, 0, 1, 1), Style::new().bold().fg(TEXT));
        assert_eq!(title("P:\n\tm\n\t\t- t"), expected, "text is dim");
        let mut expected = Buffer::with_lines(["Q:    ", "  - t ", "      "]);
        expected.set_style(Rect::new(0, 0, 2, 1), Style::new().bold().fg(PRIMARY));
        assert_eq!(title("P:\n\tQ:\n\t\t- t"), expected, "a project is colored");
        let mut expected = Buffer::with_lines(["- t   ", "  - u ", "      "]);
        expected.set_style(Rect::new(2, 0, 1, 1), Style::new().bold());
        assert_eq!(
            title("P:\n\t- t\n\t\t- u"),
            expected,
            "a task has its marker"
        );
    }

    /// The first row of `text` drawn as `editor` edits it, `width` columns wide, with the caret's
    /// column.
    fn edited(text: &str, editor: &Editor, width: u16) -> (Buffer, usize) {
        let rows = State::new(format::parse(text), None).rows();
        let mut buf = Buffer::empty(Rect::new(0, 0, width, 1));
        let (line, caret) = editor_line(&rows[0], editor, usize::from(width));
        line.render(buf.area, &mut buf);
        (buf, caret)
    }

    fn editor(text: &str, caret: usize, selection: Option<Range<usize>>) -> Editor<'_> {
        Editor {
            id: crate::document::ItemId::default(),
            text,
            caret,
            selection,
            suffix: "",
        }
    }

    // FR-MODE-02
    #[test]
    fn the_editor_draws_the_text_as_typed_after_the_marker_with_the_selection_reversed() {
        let (buf, caret) = edited("- a", &editor("ab:cd", 3, Some(1..3)), 9);
        let mut expected = Buffer::with_lines(["- ab:cd  "]);
        expected.set_style(Rect::new(3, 0, 2, 1), Style::new().reversed());
        assert_eq!(buf, expected);
        assert_eq!(caret, 5, "after the marker and the text before the caret");
    }

    // FR-MODE-02
    #[test]
    fn the_editor_shows_a_projects_colon_after_the_text() {
        let mut project = editor("Inbox", 5, None);
        project.suffix = ":";
        let (buf, caret) = edited("Inbox:", &project, 8);
        let mut expected = Buffer::with_lines(["Inbox:  "]);
        expected.set_style(Rect::new(0, 0, 6, 1), Style::new().bold().fg(PRIMARY));
        assert_eq!(buf, expected);
        assert_eq!(caret, 5);
    }

    // FR-MODE-02
    #[test]
    fn the_editor_scrolls_the_text_by_graphemes_to_keep_the_caret_in_view() {
        let (buf, caret) = edited("- a", &editor("日本語ab", 11, None), 6);
        assert_eq!(buf, Buffer::with_lines(["- ab  "]));
        assert_eq!(caret, 4);
        let (buf, caret) = edited("- a", &editor("日本語ab", 6, Some(0..9)), 6);
        let mut expected = Buffer::with_lines(["- 本語"]);
        for x in [2, 4] {
            expected.set_style(Rect::new(x, 0, 1, 1), Style::new().reversed());
        }
        assert_eq!(
            buf, expected,
            "the scrolled-off part of the selection is not drawn"
        );
        assert_eq!(caret, 4);
        let (buf, caret) = edited("- a", &editor("abc", 0, None), 3);
        assert_eq!(
            buf,
            Buffer::with_lines(["- a"]),
            "no scrolling while the caret fits"
        );
        assert_eq!(caret, 2);
    }

    // FR-MODE-02
    #[test]
    fn the_outline_draws_the_edited_block_as_its_editor() {
        let mut state = State::new(format::parse("- a\n- bc"), None);
        state.update(Message::FocusNext);
        state.update(Message::EditAtEnd);
        state.update(Message::Text(crate::app::TextEdit::SelectAll));
        let rows = state.rows();
        let editor = state.editor();
        let mut buf = Buffer::empty(Rect::new(0, 0, 7, 2));
        OutlineView::new(&rows, TODAY)
            .editor(editor.as_ref())
            .render(buf.area, &mut buf);
        let mut expected = Buffer::with_lines(["  - a  ", "  - bc "]);
        expected.set_style(Rect::new(4, 1, 2, 1), Style::new().reversed());
        assert_eq!(buf, expected);
    }

    // FR-TAG-03
    #[test]
    fn dates_show_as_chips_at_the_right_edge() {
        let mut expected = Buffer::with_lines([
            "- a  start Mon 5 Oct due Fri 9 Oct",
            "- b                  due Sun 4 Oct",
            "- c           start Fri 1 Oct 2027",
            "- d                               ",
        ]);
        expected.set_style(Rect::new(4, 0, 30, 1), chip_style());
        expected.set_style(Rect::new(20, 1, 14, 1), error_style());
        expected.set_style(Rect::new(13, 2, 21, 1), chip_style());
        assert_eq!(
            render(
                "- a @due(2026-10-09) @start(2026-10-05)\n- b @due(2026-10-04)\n- c @start(2027-10-01) @due(x)\n- d @start",
                34,
                4
            ),
            expected
        );
    }

    // FR-TAG-03, D-40
    #[test]
    fn chips_of_done_blocks_recede_and_are_not_overdue() {
        let mut expected = Buffer::with_lines(["P:    due Sun 4 Oct", "  - a due Sun 4 Oct"]);
        expected.set_style(
            Rect::new(0, 0, 2, 1),
            Style::new().bold().fg(PRIMARY).dim().crossed_out(),
        );
        expected.set_style(Rect::new(5, 0, 14, 1), chip_style().dim());
        expected.set_style(Rect::new(2, 1, 3, 1), Style::new().dim());
        expected.set_style(Rect::new(5, 1, 14, 1), chip_style().dim());
        assert_eq!(
            render("P: @done @due(2026-10-04)\n\t- a @due(2026-10-04)", 19, 2),
            expected
        );
    }

    // FR-TAG-03, D-39
    #[test]
    fn chips_keep_the_band_of_the_focused_row() {
        let rows = State::new(format::parse("- a @due(2026-10-04)"), None).rows();
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        OutlineView::new(&rows, TODAY)
            .focused(Some(0))
            .render(buf.area, &mut buf);
        let mut expected = Buffer::with_lines(["▎ - a  due Sun 4 Oct"]);
        expected.set_style(expected.area, current_line_style());
        expected.set_style(Rect::new(0, 0, 1, 1), Style::new().fg(PRIMARY));
        expected.set_style(Rect::new(6, 0, 14, 1), error_style());
        assert_eq!(buf, expected);
    }

    // FR-TAG-03, FR-MODE-02
    #[test]
    fn the_edited_block_shows_no_chips() {
        let mut state = State::new(
            format::parse("- a @due(2026-10-05)\n- b @due(2026-10-05)"),
            None,
        );
        state.update(Message::EditAtEnd);
        let rows = state.rows();
        let editor = state.editor();
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 2));
        OutlineView::new(&rows, TODAY)
            .editor(editor.as_ref())
            .render(buf.area, &mut buf);
        let mut expected = Buffer::with_lines(["  - a               ", "  - b  due Mon 5 Oct"]);
        expected.set_style(Rect::new(6, 1, 14, 1), chip_style());
        assert_eq!(buf, expected);
    }

    #[test]
    fn clips_rows_that_do_not_fit() {
        let expected = Buffer::with_lines(["- abc", "- d  "]);
        assert_eq!(render("- abcdef\n- d\n- e", 5, 2), expected);
    }

    // D-30, D-39
    #[test]
    fn marks_the_focused_row_in_the_gutter_on_a_band_without_changing_its_text() {
        let mut expected = Buffer::with_lines(["  - a", "▎ - b", "  - c"]);
        expected.set_style(Rect::new(0, 1, 5, 1), current_line_style());
        expected.set_style(Rect::new(0, 1, 1, 1), Style::new().fg(PRIMARY));
        assert_eq!(render_scrolled(0, Some(1), Rect::new(0, 0, 5, 3)), expected);
    }

    // D-39
    #[test]
    fn the_band_keeps_every_text_style_of_each_block_kind() {
        let rows = State::new(
            format::parse("P:\n- a @done\nn\nS @section\n---\nhttps://x.y @url"),
            None,
        )
        .rows();
        let area = Rect::new(0, 0, 12, 6);
        let mut plain = Buffer::empty(area);
        OutlineView::new(&rows, TODAY).render(area, &mut plain);
        for index in 0..rows.len() {
            let mut focused = Buffer::empty(area);
            OutlineView::new(&rows, TODAY)
                .focused(Some(index))
                .render(area, &mut focused);
            let y = u16::try_from(index).unwrap();
            for x in GUTTER_WIDTH..area.width {
                let (cell, before) = (&focused[(x, y)], &plain[(x, y)]);
                assert_eq!(
                    (cell.symbol(), cell.fg, cell.modifier, cell.bg),
                    (
                        before.symbol(),
                        before.fg,
                        before.modifier,
                        Color::Indexed(235)
                    ),
                    "row {index}, column {x}"
                );
            }
        }
    }

    #[test]
    fn starts_at_the_offset_and_marks_relative_to_it() {
        let area = Rect::new(2, 1, 6, 2);
        let mut expected = Buffer::with_lines(["  - c ", "▎ - d "]);
        expected.area = area;
        expected.set_style(Rect::new(2, 2, 6, 1), current_line_style());
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

    /// The spans `file_status` draws, as text and style.
    fn status(
        save: SaveState,
        access: Access,
        confirming_quit: bool,
        notice: Option<&Notice>,
    ) -> Vec<(String, Style)> {
        file_status(save, access, false, confirming_quit, notice)
            .into_iter()
            .map(|span| (span.content.into_owned(), span.style))
            .collect()
    }

    fn plain(text: &str) -> (String, Style) {
        (text.to_owned(), Style::new())
    }

    // FR-DOC-02
    #[test]
    fn the_file_status_marks_unsaved_changes_and_a_failed_save() {
        let rw = Access::ReadWrite;
        assert_eq!(status(SaveState::Saved, rw, false, None), []);
        assert_eq!(status(SaveState::Unsaved, rw, false, None), [plain(" ●")]);
        let failed = SaveState::Failed {
            reason: "disk full",
        };
        assert_eq!(
            status(failed, rw, false, None),
            [
                plain(" ● "),
                ("save failed: disk full".to_owned(), error_style())
            ]
        );
        assert_eq!(
            status(failed, rw, true, None)[2..],
            [
                plain(" · "),
                (
                    "Ctrl+Q again quits without saving".to_owned(),
                    Style::new().bold()
                )
            ]
        );
    }

    // FR-DOC-07
    #[test]
    fn the_file_status_shows_read_only_and_a_failed_backup() {
        let notice = Notice::BackupFailed {
            reason: "no space".to_owned(),
        };
        assert_eq!(
            status(SaveState::Saved, Access::ReadOnly, false, Some(&notice)),
            [
                plain(" · read-only"),
                plain(" · "),
                ("backup failed: no space".to_owned(), warning_style())
            ]
        );
        assert_eq!(error_style(), Style::new().red().bold());
        assert_eq!(warning_style(), Style::new().yellow());
    }

    // OQ-25
    #[test]
    fn the_file_status_hints_that_tags_are_set_with_commands() {
        assert_eq!(
            status(
                SaveState::Saved,
                Access::ReadWrite,
                false,
                Some(&Notice::TypedTag)
            ),
            [
                plain(" · "),
                ("Tags are set with commands".to_owned(), warning_style())
            ]
        );
    }

    // file-io.md §1, FR-OPS-08
    #[test]
    fn the_file_status_shows_a_refused_or_failed_edit() {
        let rw = Access::ReadWrite;
        assert_eq!(
            status(SaveState::Saved, rw, false, Some(&Notice::ReadOnly)),
            [
                plain(" · "),
                ("cannot edit a read-only file".to_owned(), warning_style())
            ]
        );
        let failed = Notice::EditFailed {
            reason: "it changed".to_owned(),
        };
        assert_eq!(
            status(SaveState::Saved, rw, false, Some(&failed)),
            [
                plain(" · "),
                ("edit failed: it changed".to_owned(), error_style())
            ]
        );
    }

    // F-FILE-06
    #[test]
    fn the_file_status_marks_a_file_gone_from_disk_and_external_change_notices() {
        let rw = Access::ReadWrite;
        assert_eq!(
            file_status(SaveState::Saved, rw, true, false, None)
                .into_iter()
                .map(|span| span.content.into_owned())
                .collect::<Vec<_>>(),
            [" · not on disk"]
        );
        let copy = std::path::PathBuf::from("/b/c.taskpaper");
        for (notice, expected) in [
            (Notice::Reloaded, plain("Reloaded (changed on disk)")),
            (
                Notice::BlanksLifted,
                plain("Empty lines at the end moved to the top level"),
            ),
            (Notice::Bookmark { added: true }, plain("Bookmarked")),
            (Notice::Bookmark { added: false }, plain("Bookmark removed")),
            (
                Notice::KeptMine { copy: copy.clone() },
                plain("Kept your version; the one on disk is in /b/c.taskpaper"),
            ),
            (
                Notice::TookTheirs { copy },
                plain("Took the version on disk; yours is in /b/c.taskpaper"),
            ),
            (
                Notice::ConflictCopyFailed {
                    reason: "disk full".to_owned(),
                },
                ("conflict copy failed: disk full".to_owned(), error_style()),
            ),
            (
                Notice::OpenUrlFailed {
                    url: "https://a.b".to_owned(),
                    reason: "not found".to_owned(),
                },
                (
                    "cannot open https://a.b: not found".to_owned(),
                    error_style(),
                ),
            ),
            (
                Notice::CannotTakeTheirs {
                    reason: "not UTF-8".to_owned(),
                },
                (
                    "cannot take the version on disk: not UTF-8".to_owned(),
                    error_style(),
                ),
            ),
        ] {
            assert_eq!(
                status(SaveState::Saved, rw, false, Some(&notice)),
                [plain(" · "), expected]
            );
        }
    }

    /// Draws the conflict prompt over a blank `width` × `height` screen.
    fn prompt(diff: Option<&[DiffLine]>, scroll: usize, width: u16, height: u16) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        ConflictPrompt::new(diff, scroll).render(buf.area, &mut buf);
        buf
    }

    // OQ-16
    #[test]
    fn the_conflict_prompt_names_the_choices_in_a_box_in_the_middle() {
        let mut expected = Buffer::with_lines([
            "                                                                                  ",
            " ┌ The file changed on disk ────────────────────────────────────────────────────┐ ",
            " │Your changes are not saved yet.                                               │ ",
            " │m keep mine · t take theirs · d show diff                                     │ ",
            " └──────────────────────────────────────────────────────────────────────────────┘ ",
            "                                                                                  ",
        ]);
        for x in [2, 16, 32] {
            expected.set_style(Rect::new(x, 3, 1, 1), Style::new().bold());
        }
        assert_eq!(prompt(None, 0, 82, 6), expected);
        let mut covered = Buffer::with_lines(vec!["x".repeat(82); 6]);
        ConflictPrompt::new(None, 0).render(covered.area, &mut covered);
        assert_eq!(covered[(0, 0)].symbol(), "x");
        assert_eq!(
            covered[(50, 2)].symbol(),
            " ",
            "the box hides what is under it"
        );
    }

    // OQ-16
    #[test]
    fn the_conflict_prompt_lists_the_diff_from_the_scrolled_line() {
        let diff = [
            DiffLine::Same("one".to_owned()),
            DiffLine::Disk("two".to_owned()),
            DiffLine::Mine("TWO".to_owned()),
            DiffLine::Gap,
            DiffLine::Same("nine".to_owned()),
        ];
        let buf = prompt(Some(&diff), 1, 30, 7);
        let row = |y: u16| -> String { (0..30).map(|x| buf[(x, y)].symbol()).collect::<String>() };
        assert_eq!(row(0), "┌ The file changed on disk ──┐");
        assert_eq!(row(1), "│- on disk  + yours          │");
        assert_eq!(row(2), "│- two                       │");
        assert_eq!(row(3), "│+ TWO                       │");
        assert_eq!(row(4), "│⋯                           │");
        assert_eq!(row(5), "│m keep mine · t take theirs │");
        assert_eq!(buf[(1, 2)].fg, Color::Red);
        assert_eq!(buf[(1, 3)].fg, Color::Green);
        assert_eq!(buf[(1, 4)].fg, Color::DarkGray);
        assert_eq!(ConflictPrompt::diff_rows(Rect::new(0, 0, 30, 7)), 3);
        assert_eq!(ConflictPrompt::diff_rows(Rect::new(0, 0, 100, 50)), 16);
        assert_eq!(ConflictPrompt::diff_rows(Rect::new(0, 0, 30, 3)), 0);
        let same = prompt(Some(&diff), 0, 30, 7);
        assert_eq!(same[(1, 2)].symbol(), " ");
        assert_eq!(same[(3, 2)].symbol(), "o");
    }
}
