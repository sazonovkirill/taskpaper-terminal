//! Widgets that draw the core's view state ([ui.md §2](../../../docs/specs/ui.md#2-blocks)).

mod wrap;

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

use super::{layout, theme::Icon};
use crate::{
    app::{
        Access, Command, CommandMenuView, Conversion, DatePromptView, DiffLine, Editor, MenuRow,
        Notice, PickerView, Row, SaveState,
    },
    document::{
        BlockKind, DateTag, Details, Flag, TagColor,
        dates::{self, DateError},
        format::Typed,
        inline::{self, Format, Syntax},
    },
};

/// The outline: a gutter that marks the focused block, whose lines get a brighter background, and
/// the selected blocks, then each block, 2 columns of indent per level, a marker and the text,
/// with its date chips at the right edge of its first line. Text too long for its line wraps
/// under where it starts, or with wrap off ends in `…` on the one line (FR-UI-05); it stops
/// before the chips. A block being edited shows the text as typed instead, without chips, on one
/// line.
#[derive(Debug)]
pub struct OutlineView<'a> {
    rows: &'a [Row],
    today: Date,
    offset: usize,
    focused: Option<usize>,
    editor: Option<&'a Editor<'a>>,
    wrap: bool,
}

impl<'a> OutlineView<'a> {
    /// A view of `rows` on `today`, the local date, drawn from the top of its area with no row
    /// focused and wrap off.
    #[must_use]
    pub fn new(rows: &'a [Row], today: Date) -> Self {
        Self {
            rows,
            today,
            offset: 0,
            focused: None,
            editor: None,
            wrap: false,
        }
    }

    /// Starts drawing at screen line `offset` of the outline instead of its first line.
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

    /// Wraps text too long for its line when `wrap` is on, else ends it in `…` (FR-UI-05).
    #[must_use]
    pub fn wrap(self, wrap: bool) -> Self {
        Self { wrap, ..self }
    }

    /// How many screen lines each row takes in an area `width` columns wide, the gutter
    /// included.
    #[must_use]
    pub fn heights(&self, width: u16) -> Vec<usize> {
        let width = usize::from(width.saturating_sub(GUTTER_WIDTH));
        self.blocks(width)
            .map(|(_, _, lines, _)| lines.len())
            .collect()
    }

    /// Each row with its index, its screen lines `width` columns wide and its chips.
    fn blocks(
        &self,
        width: usize,
    ) -> impl Iterator<Item = (usize, &'a Row, Vec<Line<'a>>, Line<'static>)> {
        self.rows.iter().enumerate().map(move |(index, row)| {
            if let Some(editor) = self.editor.filter(|editor| editor.id == row.id) {
                return (
                    index,
                    row,
                    vec![editor_line(row, editor, width).0],
                    Line::default(),
                );
            }
            let chips = chips(row, self.today);
            let room = width.saturating_sub(chips.width());
            let line = row_line(row, room);
            let lines = if self.wrap {
                wrap::wrap(&line, text_column(row), room, width)
            } else {
                vec![wrap::truncate(&line, room)]
            };
            (index, row, lines, chips)
        })
    }
}

impl Widget for OutlineView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [gutter, text] = area.layout(&Layout::horizontal([
            Constraint::Length(GUTTER_WIDTH),
            Constraint::Fill(1),
        ]));
        let lines = self
            .blocks(usize::from(text.width))
            .flat_map(|(index, row, lines, chips)| {
                let mut chips = Some(chips);
                lines
                    .into_iter()
                    .map(move |line| (index, row, line, chips.take().unwrap_or_default()))
            });
        for ((index, row, line, chips), (gutter_area, line_area)) in
            lines.skip(self.offset).zip(gutter.rows().zip(text.rows()))
        {
            let focused = Some(index) == self.focused;
            // The line patches its own styles over the band and the tint, so every foreground
            // color and modifier survives on the focused and colored rows.
            if focused {
                buf.set_style(gutter_area.union(line_area), current_line_style());
            }
            if let Some(color) = row.color {
                let indent = u16::try_from(row.depth.saturating_mul(2)).unwrap_or(u16::MAX);
                let tinted = Rect {
                    x: line_area.x.saturating_add(indent),
                    width: line_area.width.saturating_sub(indent),
                    ..line_area
                };
                buf.set_style(tinted, Style::new().bg(tag_tint(color, focused)));
            }
            line.render(line_area, buf);
            chips.render(line_area, buf);
            if focused {
                let color = if self.editor.is_some() {
                    ACCENT
                } else {
                    PRIMARY
                };
                Span::styled(FOCUS_MARKER, Style::new().fg(color)).render(gutter_area, buf);
            } else if row.selected {
                Span::styled(FOCUS_MARKER, selection_style()).render(gutter_area, buf);
            }
        }
    }
}

/// Columns left of the outline for the focus marker; the breadcrumbs start after it too, so the
/// root crumb lines up with top-level blocks (D-35).
pub const GUTTER_WIDTH: u16 = 2;

/// Marks the focused block in the gutter (D-30), in the primary color, or the accent while
/// editing, and dimmed, each selected one (ui.md §2); a half block, twice the width of the
/// quarter block it replaced (D-51).
const FOCUS_MARKER: &str = "▌";

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
        Notice::NotSections { skipped } => Span::styled(
            format!("Only text can be a section: {skipped} skipped"),
            warning_style(),
        ),
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

/// A popup over the outline ([ui.md §5](../../../docs/specs/ui.md#5-context-menu-fr-key-06)): a
/// box with a title listing lines, for a menu the entries the filter matches with the highlighted
/// one reversed and kept in view and the filter at the bottom once one is typed. It shows the
/// command menu (D-50), the color picker (FR-TAG-05), the type picker (FR-BLK-08) and a block's
/// details (D-3).
#[derive(Debug)]
pub struct Popup {
    title: &'static str,
    /// The style of the border and title.
    frame: Style,
    entries: Vec<Line<'static>>,
    selected: usize,
    filter: String,
}

impl Popup {
    /// The command menu, titled "Commands:" in the accent color like its border, each entry
    /// after its icon and padded by 2 columns on each side, with a line across it between groups
    /// and `/` before its filter (D-50, D-54).
    #[must_use]
    pub fn commands(menu: &CommandMenuView) -> Self {
        const PAD: &str = "  ";
        let title = "Commands:";
        let mut entries: Vec<Line<'static>> = menu
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| match row {
                MenuRow::Command { label, command } => {
                    let (icon, style) = command_icon(*command);
                    let line = Line::from(vec![
                        Span::raw(PAD),
                        Span::styled(icon.glyph(), style),
                        Span::raw(format!(" {label}{PAD}")),
                    ]);
                    if index == menu.selected {
                        line.reversed()
                    } else {
                        line
                    }
                }
                MenuRow::Divider => Line::default(),
            })
            .collect();
        let width = entries
            .iter()
            .map(Line::width)
            .chain([Line::from(title).width()])
            .max()
            .unwrap_or_default();
        for (line, row) in entries.iter_mut().zip(&menu.rows) {
            if *row == MenuRow::Divider {
                *line = Line::from("─".repeat(width)).dim();
            }
        }
        Self {
            title,
            frame: Style::new().fg(ACCENT),
            entries,
            selected: menu.selected,
            filter: if menu.filter.is_empty() {
                String::new()
            } else {
                format!("/{}", menu.filter)
            },
        }
    }

    /// The color picker, titled "Color:", each color's name in that color after its number,
    /// which chooses it (D-48).
    #[must_use]
    pub fn colors(picker: &PickerView<Option<TagColor>>) -> Self {
        let names: Vec<(String, Style)> = picker
            .entries
            .iter()
            .map(|&(color, name)| match color {
                Some(color) => (
                    format!("{} {name}", color.number()),
                    Style::new().fg(tag_color(color)),
                ),
                None => (format!("  {name}"), Style::new()),
            })
            .collect();
        let names = names.iter().map(|(name, style)| (name.as_str(), *style));
        Self {
            title: "Color:",
            frame: Style::new(),
            entries: entries(names, picker.selected),
            selected: picker.selected,
            filter: picker.filter.to_owned(),
        }
    }

    /// The type picker, titled "Node type:" (FR-BLK-08).
    #[must_use]
    pub fn block_types(picker: &PickerView<Conversion>) -> Self {
        let names = picker.entries.iter().map(|&(_, name)| (name, Style::new()));
        Self {
            title: "Node type:",
            frame: Style::new(),
            entries: entries(names, picker.selected),
            selected: picker.selected,
            filter: picker.filter.to_owned(),
        }
    }

    /// A block's details, titled "Details:": each tag as written, the unsupported ones marked,
    /// then each special node it owns as hidden data with its tags and line count (D-3).
    #[must_use]
    pub fn details(details: &Details) -> Self {
        let tags = details.tags.iter().map(|(tag, supported)| {
            let mut line = Line::from(format!(" {tag} "));
            if !supported {
                line.push_span(Span::styled("unsupported ", Style::new().dim()));
            }
            line
        });
        let hidden = details.hidden.iter().map(|node| {
            let lines = match node.lines {
                1 => "1 line".to_owned(),
                n => format!("{n} lines"),
            };
            Line::from(vec![
                Span::styled(" Hidden data ", Style::new().dim()),
                Span::raw(format!("{} · {lines} ", node.tags.join(" "))),
            ])
        });
        let mut entries: Vec<Line<'static>> = tags.chain(hidden).collect();
        if entries.is_empty() {
            entries.push(Line::from(" No tags ").dim());
        }
        Self {
            title: "Details:",
            frame: Style::new(),
            entries,
            selected: 0,
            filter: String::new(),
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

/// The icon of the command menu's entry for `command`, in its style: the colors of the matching
/// chips and icons of the outline (D-46, D-47), a color entry in its color, and a command without
/// an icon of its own the generic one (D-54).
fn command_icon(command: Command) -> (Icon, Style) {
    let plain = |icon| (icon, Style::new());
    let date = |tag| match tag {
        DateTag::Start => (Icon::Start, start_style()),
        DateTag::Due => (Icon::Due, due_style()),
    };
    match command {
        Command::Retype(Typed::Project) => plain(Icon::Project),
        Command::Retype(Typed::Task) => plain(Icon::Task),
        Command::Retype(Typed::Text) => plain(Icon::Note),
        Command::Retype(Typed::Section) => plain(Icon::Section),
        Command::ToggleFlag(Flag::Today) => (Icon::Today, today_style()),
        Command::ToggleFlag(Flag::Bookmark) => (Icon::Bookmark, bookmark_style()),
        Command::OpenDatePrompt(tag) | Command::ChangeDate(tag, _) | Command::ClearDate(tag) => {
            date(tag)
        }
        Command::ToggleDone => plain(Icon::Complete),
        Command::SetColor(Some(color)) => (Icon::Color, Style::new().fg(tag_color(color))),
        Command::SetColor(None) => plain(Icon::Color),
        Command::OpenDetails => plain(Icon::Details),
        Command::OpenUrl => plain(Icon::Link),
        Command::Duplicate => plain(Icon::Duplicate),
        Command::Delete => plain(Icon::Delete),
        _ => plain(Icon::Command),
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

impl Widget for Popup {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let frame = Block::bordered()
            .border_style(self.frame)
            .title_style(self.frame)
            .title(self.title)
            .title_bottom(self.filter);
        let inner = frame.inner(area);
        frame.render(area, buf);
        let height = usize::from(inner.height);
        let offset = layout::scroll_offset(
            0,
            self.selected..self.selected + 1,
            height,
            self.entries.len(),
        );
        for (line, row) in self.entries.into_iter().skip(offset).zip(inner.rows()) {
            buf.set_style(row, line.style);
            line.render(row, buf);
        }
    }
}

/// The date prompt ([editing.md §5](../../../docs/specs/editing.md#5-block-commands), ui.md §1):
/// a panel across the bottom of the screen, its border and title, the date it sets, in the
/// accent color, holding the text as typed with the selection reversed and, below it, what
/// `Enter` does: set the date the text resolves to, remove the date, or nothing, saying why the
/// text is not a date (D-54).
#[derive(Debug)]
pub struct DatePromptPanel<'a> {
    prompt: &'a DatePromptView<'a>,
}

impl<'a> DatePromptPanel<'a> {
    /// The panel's height, the border included.
    pub const HEIGHT: u16 = 4;

    /// The panel showing `prompt`.
    #[must_use]
    pub fn new(prompt: &'a DatePromptView<'a>) -> Self {
        Self { prompt }
    }

    /// The caret's cell when the panel is drawn over `area`, on the text's line, the first inside
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

impl Widget for DatePromptPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let title = match self.prompt.tag {
            DateTag::Start => " Start date ",
            DateTag::Due => " Due date ",
        };
        let accent = Style::new().fg(ACCENT);
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

/// The spans of `row`'s text in `style` with its formatting applied over it, the delimiters
/// hidden. Only links, the whole text of a row with `@url` included, take a color of their own,
/// underlined in the link color (D-45); the rest keeps the row's color and only adds modifiers
/// (D-49): a heading bold, level 1 also underlined; bold; and code dimmed (FR-FMT-01,
/// FR-FMT-03).
fn formatted(row: &Row, style: Style) -> impl Iterator<Item = Span<'_>> {
    inline::runs(&row.text, Syntax::of(row.kind))
        .into_iter()
        .map(move |run| {
            let Format {
                bold,
                code,
                link,
                heading,
            } = run.format;
            let mut style = style;
            if let Some(level) = heading {
                style = style.bold();
                if level == 1 {
                    style = style.underlined();
                }
            }
            if bold {
                style = style.bold();
            }
            if code {
                style = style.dim();
            }
            if link || row.url {
                style = style.underlined().fg(LINK);
            }
            Span::styled(run.text, style)
        })
}

/// The screen line of one block, `width` columns wide: indented 2 columns per level of depth
/// whatever its type, then the marker of a task, then the text with its inline formatting
/// (D-31, FR-FMT-01), then the `@today` star and the bookmark icon, each after a space (D-47),
/// dimmed with a done block or one inside it. A collapsed block with children shows `▸ n` after
/// them (ui.md §2). The zoom title is drawn like its block, in bold (D-34).
fn row_line(row: &Row, width: usize) -> Line<'_> {
    let (marker, marker_style, style) = row_style(row);
    let mut line = Line::from(vec![
        Span::raw("  ".repeat(row.depth)),
        Span::styled(marker, marker_style),
    ]);
    match row.kind {
        _ if row.divider => {}
        // TaskPaper marks a project with a trailing colon, which the display text drops.
        BlockKind::Project => {
            line.extend(formatted(row, style));
            line.push_span(Span::styled(":", style));
        }
        BlockKind::Task | BlockKind::Text => line.extend(formatted(row, style)),
    }
    let recede = |style: Style| {
        if row.done || row.inside_done {
            style.dim()
        } else {
            style
        }
    };
    for (marked, icon, style) in [
        (row.today, Icon::Today, today_style()),
        (row.bookmark, Icon::Bookmark, bookmark_style()),
    ] {
        if marked {
            line.push_span(" ");
            line.push_span(Span::styled(icon.glyph(), recede(style)));
        }
    }
    if row.hidden_children > 0 {
        line.push_span(Span::styled(
            format!(" ▸ {}", row.hidden_children),
            Style::new().dim(),
        ));
    }
    if row.divider {
        if row.today || row.bookmark || row.hidden_children > 0 {
            line.push_span(" ");
        }
        line.push_span(rule(&line, width));
    }
    line
}

/// The date chips of `row` on `today`, right-aligned, each after a space: its start date in green
/// and its due date in red, each as its icon and the date named relative to `today`, such as
/// `tomorrow` (D-46); an overdue date has its own icon and is reversed. All are dimmed with a done
/// block or one inside it (ui.md §2, FR-TAG-03).
fn chips(row: &Row, today: Date) -> Line<'static> {
    let receded = row.done || row.inside_done;
    let recede = |style: Style| if receded { style.dim() } else { style };
    let chip = |icon: Icon, date: Date, style: Style| {
        [
            Span::raw(" "),
            Span::styled(
                format!("{} {}", icon.glyph(), dates::relative(date, today)),
                recede(style),
            ),
        ]
    };
    let due = |date| {
        if row.overdue(today) {
            chip(Icon::Overdue, date, overdue_style())
        } else {
            chip(Icon::Due, date, due_style())
        }
    };
    let spans: Vec<Span> = [
        row.start.map(|date| chip(Icon::Start, date, start_style())),
        row.due.map(due),
    ]
    .into_iter()
    .flatten()
    .flatten()
    .collect();
    Line::from(spans).right_aligned()
}

/// The `@today` star (FR-TAG-04); themes arrive in step 6.2.
fn today_style() -> Style {
    Style::new().fg(Color::Yellow)
}

/// The bookmark icon (D-47), in the primary color; themes arrive in step 6.2.
fn bookmark_style() -> Style {
    Style::new().fg(PRIMARY)
}

/// Start date chips, always green (D-46); themes arrive in step 6.2.
fn start_style() -> Style {
    Style::new().fg(Color::Green)
}

/// Due date chips, always red (D-46); themes arrive in step 6.2.
fn due_style() -> Style {
    Style::new().fg(Color::Red)
}

/// Overdue due date chips: red and reversed, so they stand out from other due dates (FR-TAG-03,
/// D-46); themes arrive in step 6.2.
fn overdue_style() -> Style {
    due_style().bold().reversed()
}

/// The column of `row`'s line where its text starts, after its indent and marker.
fn text_column(row: &Row) -> usize {
    let (marker, _, _) = row_style(row);
    row.depth * 2 + Span::raw(marker).width()
}

/// The marker of a block with its style, and the style of its text, by its type and whether it
/// is done or inside a done block (ui.md §2).
fn row_style(row: &Row) -> (&'static str, Style, Style) {
    let (marker, style) = match row.kind {
        BlockKind::Project => ("", Style::new().bold().fg(PRIMARY)),
        BlockKind::Task if row.done => ("✔ ", Style::new()),
        BlockKind::Task => ("- ", Style::new()),
        BlockKind::Text if row.section => ("", Style::new().bold().italic().fg(SECONDARY)),
        BlockKind::Text => ("", Style::new().fg(TEXT)),
    };
    let style = if row.title { style.bold() } else { style };
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
/// on it: the row's indent and marker, then the text with the selection reversed, styled as the
/// row (FR-MODE-02). When the caret would be past the right edge,
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

/// A color tag's own color, in which the color picker names it (FR-TAG-05); themes arrive in
/// step 6.2, until when orange is the 256-color orange 208 and violet the terminal's magenta.
fn tag_color(color: TagColor) -> Color {
    match color {
        TagColor::Red => Color::Red,
        TagColor::Orange => Color::Indexed(208),
        TagColor::Yellow => Color::Yellow,
        TagColor::Green => Color::Green,
        TagColor::Blue => Color::Blue,
        TagColor::Violet => Color::Magenta,
    }
}

/// The background a color tag gives its block's row (D-48): a very dim shade of the color, a step
/// brighter on the focused row, as the current-line band is brighter than the background (D-39).
/// Themes arrive in step 6.2, until when these shades are picked for dark terminals.
fn tag_tint(color: TagColor, focused: bool) -> Color {
    let ((r, g, b), (focused_r, focused_g, focused_b)) = match color {
        TagColor::Red => ((0x3b, 0x22, 0x24), (0x4c, 0x2b, 0x2e)),
        TagColor::Orange => ((0x3b, 0x2a, 0x1c), (0x4c, 0x36, 0x24)),
        TagColor::Yellow => ((0x36, 0x32, 0x1c), (0x45, 0x40, 0x24)),
        TagColor::Green => ((0x21, 0x33, 0x22), (0x2a, 0x42, 0x2c)),
        TagColor::Blue => ((0x1f, 0x2a, 0x3d), (0x28, 0x36, 0x4e)),
        TagColor::Violet => ((0x30, 0x23, 0x3d), (0x3d, 0x2d, 0x4e)),
    };
    if focused {
        Color::Rgb(focused_r, focused_g, focused_b)
    } else {
        Color::Rgb(r, g, b)
    }
}

/// Divider blocks: the rule-lines color, dimmed further so they recede behind the header rule
/// (D-33).
fn divider_style() -> Style {
    rule_style().dim()
}

/// The theme's primary color (Textual's `$primary`), for projects and the focus marker in
/// navigate mode; themes arrive in step 6.2.
const PRIMARY: Color = Color::LightBlue;

/// The theme's accent (Textual's `$accent`), for the focus marker while editing (D-51); themes
/// arrive in step 6.2, until when it is Textual's default accent.
const ACCENT: Color = Color::Rgb(0xff, 0xa6, 0x2b);

/// Plain text: dimmed, but less than done tasks and the other dim blocks (D-37); themes arrive in
/// step 6.2. A fixed grey, `#a8a8a8`, because many terminals draw the ANSI gray as their default
/// foreground, which left text undimmed.
const TEXT: Color = Color::Indexed(248);

/// The theme's link color, for every link (ui.md §2, D-45); themes arrive in step 6.2.
const LINK: Color = Color::Cyan;

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

/// The marker of a selected block that is not focused: the focus marker, dimmer (ui.md §2);
/// themes arrive in step 6.2.
fn selection_style() -> Style {
    Style::new().fg(PRIMARY).dim()
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
        app::{Message, SelectionChange, State},
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

    /// A date chip as drawn: its icon, then the date in words.
    fn chip(icon: Icon, date: &str) -> String {
        format!("{} {date}", icon.glyph())
    }

    /// `text`, then `chips` ending at column `width`; every character is one cell wide.
    fn at_right(text: &str, chips: &str, width: usize) -> String {
        format!("{text}{chips:>pad$}", pad = width - text.chars().count())
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
            struck(Style::new().bold().italic().fg(SECONDARY)),
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
            Style::new().bold().italic().fg(SECONDARY).dim(),
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
    /// 30 × 4 cells.
    fn date_prompt(text: &str, date: Result<Option<Date>, DateError>) -> Buffer {
        let view = DatePromptView {
            tag: DateTag::Start,
            text,
            caret: text.len(),
            selection: Some(0..text.len()),
            date,
        };
        let panel = DatePromptPanel::new(&view);
        let area = Rect::new(0, 0, 30, DatePromptPanel::HEIGHT);
        let mut buf = Buffer::empty(area);
        assert_eq!(panel.caret(area), (2 + text_width(text), 1));
        panel.render(area, &mut buf);
        buf
    }

    fn text_width(text: &str) -> u16 {
        u16::try_from(text.len()).unwrap()
    }

    // FR-TAG-03, ui.md §1, D-54: its border and title in the accent color.
    #[test]
    fn the_date_prompt_shows_the_text_selected_and_the_date_it_resolves_to() {
        let mut expected = Buffer::with_lines([
            "┌ Start date ────────────────┐",
            "│ fri                        │",
            "│ Fri 9 Oct 2026             │",
            "└────────────────────────────┘",
        ]);
        let accent = Style::new().fg(ACCENT);
        expected.set_style(Rect::new(0, 0, 30, 1), accent);
        expected.set_style(Rect::new(0, 3, 30, 1), accent);
        expected.set_style(Rect::new(0, 1, 1, 2), accent);
        expected.set_style(Rect::new(29, 1, 1, 2), accent);
        expected.set_style(Rect::new(2, 1, 3, 1), Style::new().reversed());
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
                .map(|x| buf[(x, 2)].symbol().to_owned())
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

    /// The command menu of a session on `- a` after `keys`, drawn at its size but at most
    /// `height` rows tall.
    fn command_menu(keys: &[crate::app::MenuInput], height: u16) -> Buffer {
        let mut state = State::new(format::parse("- a"), None);
        state.update(Message::EditAtEnd);
        state.update(Message::OpenCommandMenu {
            today: jiff::civil::date(2026, 10, 6),
        });
        for key in keys {
            state.update(Message::CommandMenu(*key));
        }
        let menu = state.command_menu().unwrap();
        let popup = Popup::commands(&menu);
        let (width, full) = popup.size();
        let mut buf = Buffer::empty(Rect::new(0, 0, width, full.min(height)));
        popup.render(buf.area, &mut buf);
        buf
    }

    /// `expected` with the border and title of a `width` × `height` popup in the accent color.
    fn accent_frame(expected: &mut Buffer, width: u16, height: u16) {
        let accent = Style::new().fg(ACCENT);
        expected.set_style(Rect::new(0, 0, width, 1), accent);
        expected.set_style(Rect::new(0, height - 1, width, 1), accent);
        expected.set_style(Rect::new(0, 1, 1, height - 2), accent);
        expected.set_style(Rect::new(width - 1, 1, 1, height - 2), accent);
    }

    // D-50, D-54, ui.md §5
    #[test]
    fn the_command_menu_shows_the_matches_under_its_title_and_the_filter_at_the_bottom() {
        use crate::app::MenuInput::{Next, Type};
        let due = Icon::Due.glyph();
        let mut expected = Buffer::with_lines([
            "┌Commands:─────────┐".to_owned(),
            format!("│  {due} Set Due Date  │"),
            format!("│  {due} Due Today     │"),
            format!("│  {due} Due Tomorrow  │"),
            "└/due──────────────┘".to_owned(),
        ]);
        accent_frame(&mut expected, 20, 5);
        expected.set_style(Rect::new(1, 2, 18, 1), Style::new().reversed());
        for y in 1..4 {
            expected.set_style(Rect::new(3, y, 1, 1), due_style());
        }
        assert_eq!(
            command_menu(&[Type('d'), Type('u'), Type('e'), Next], 20),
            expected
        );
    }

    // D-50, D-54, ui.md §5
    #[test]
    fn a_command_menu_taller_than_its_room_scrolls_to_keep_the_highlight_in_view() {
        let [complete, today, bookmark] =
            [Icon::Complete, Icon::Today, Icon::Bookmark].map(Icon::glyph);
        let mut expected = Buffer::with_lines([
            "┌Commands:───────────┐".to_owned(),
            "│────────────────────│".to_owned(),
            format!("│  {complete} Complete        │"),
            format!("│  {today} Toggle Today    │"),
            format!("│  {bookmark} Bookmark        │"),
            "│────────────────────│".to_owned(),
            "└────────────────────┘".to_owned(),
        ]);
        accent_frame(&mut expected, 22, 7);
        for y in [1, 5] {
            expected.set_style(Rect::new(1, y, 20, 1), Style::new().dim());
        }
        expected.set_style(Rect::new(1, 3, 20, 1), Style::new().reversed());
        expected.set_style(Rect::new(3, 3, 1, 1), today_style());
        expected.set_style(Rect::new(3, 4, 1, 1), bookmark_style());
        assert_eq!(
            command_menu(&[crate::app::MenuInput::Next; 4], 7),
            expected,
            "groups apart, the highlight on Toggle Today"
        );
    }

    // D-54: every entry has its icon, a color entry in its color.
    #[test]
    fn each_command_has_its_icon() {
        let cases = [
            (Command::Retype(Typed::Project), Icon::Project, Style::new()),
            (Command::Retype(Typed::Task), Icon::Task, Style::new()),
            (Command::Retype(Typed::Text), Icon::Note, Style::new()),
            (Command::Retype(Typed::Section), Icon::Section, Style::new()),
            (Command::ToggleDone, Icon::Complete, Style::new()),
            (Command::ToggleFlag(Flag::Today), Icon::Today, today_style()),
            (
                Command::ToggleFlag(Flag::Bookmark),
                Icon::Bookmark,
                bookmark_style(),
            ),
            (
                Command::OpenDatePrompt(DateTag::Start),
                Icon::Start,
                start_style(),
            ),
            (
                Command::ChangeDate(DateTag::Due, crate::document::DateChange::Today),
                Icon::Due,
                due_style(),
            ),
            (
                Command::ClearDate(DateTag::Start),
                Icon::Start,
                start_style(),
            ),
            (
                Command::SetColor(Some(TagColor::Blue)),
                Icon::Color,
                Style::new().fg(Color::Blue),
            ),
            (Command::SetColor(None), Icon::Color, Style::new()),
            (Command::OpenDetails, Icon::Details, Style::new()),
            (Command::OpenUrl, Icon::Link, Style::new()),
            (Command::Duplicate, Icon::Duplicate, Style::new()),
            (Command::Delete, Icon::Delete, Style::new()),
            (Command::Undo, Icon::Command, Style::new()),
        ];
        for (command, icon, style) in cases {
            assert_eq!(command_icon(command), (icon, style), "{command:?}");
        }
    }

    // FR-BLK-08, keybindings.md §4
    #[test]
    fn the_type_picker_lists_the_types_and_the_section_entries() {
        let mut state = State::new(format::parse("a"), None);
        state.update(Message::OpenTypePicker);
        let popup = Popup::block_types(&state.type_picker().unwrap());
        assert_eq!(popup.size(), (18, 7));
        let mut buf = Buffer::empty(Rect::new(0, 0, 18, 7));
        popup.render(buf.area, &mut buf);
        let mut expected = Buffer::with_lines([
            "┌Node type:──────┐",
            "│ Project        │",
            "│ Task           │",
            "│ Note           │",
            "│ Make Section   │",
            "│ Remove Section │",
            "└────────────────┘",
        ]);
        expected.set_style(Rect::new(1, 3, 16, 1), Style::new().reversed());
        assert_eq!(buf, expected);
        for c in "rem".chars() {
            state.update(Message::TypePicker(crate::app::MenuInput::Type(c)));
        }
        let mut buf = Buffer::empty(Rect::new(0, 0, 18, 3));
        Popup::block_types(&state.type_picker().unwrap()).render(buf.area, &mut buf);
        let mut expected = Buffer::with_lines([
            "┌Node type:──────┐",
            "│ Remove Section │",
            "└rem─────────────┘",
        ]);
        expected.set_style(Rect::new(1, 1, 16, 1), Style::new().reversed());
        assert_eq!(buf, expected);
    }

    /// The details popup of the first block of `text`, drawn at its size.
    fn details(text: &str) -> Buffer {
        let mut state = State::new(format::parse(text), None);
        state.update(Message::OpenDetails);
        let popup = Popup::details(&state.details().unwrap());
        let (width, height) = popup.size();
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        popup.render(buf.area, &mut buf);
        buf
    }

    // D-3, FR-SPC-01
    #[test]
    fn the_details_list_tags_marking_the_unsupported_and_hidden_data() {
        let mut expected = Buffer::with_lines([
            "┌Details:────────────────────────────┐",
            "│ @due(2026-10-08)                   │",
            "│ @estimate(2h) unsupported          │",
            "│ Hidden data @hidden @log · 2 lines │",
            "│ Hidden data @hidden · 1 line       │",
            "└────────────────────────────────────┘",
        ]);
        expected.set_style(Rect::new(16, 2, 12, 1), Style::new().dim());
        expected.set_style(Rect::new(1, 3, 13, 1), Style::new().dim());
        expected.set_style(Rect::new(1, 4, 13, 1), Style::new().dim());
        assert_eq!(
            details("- a @due(2026-10-08) @estimate(2h)\n\tL @hidden @log\n\t\tx\n\tY @hidden"),
            expected
        );
    }

    // D-3
    #[test]
    fn the_details_of_a_block_without_tags_say_so() {
        let mut expected = Buffer::with_lines(["┌Details:─┐", "│ No tags │", "└─────────┘"]);
        expected.set_style(Rect::new(1, 1, 9, 1), Style::new().dim());
        assert_eq!(details("a"), expected);
    }

    // FR-TAG-05, keybindings.md §4, D-48
    #[test]
    fn the_color_picker_names_each_color_in_it_after_its_number() {
        let mut state = State::new(format::parse("- a @blue"), None);
        state.update(Message::OpenColorPicker);
        let picker = state.color_picker().unwrap();
        let popup = Popup::colors(&picker);
        assert_eq!(popup.size(), (12, 9));
        let mut buf = Buffer::empty(Rect::new(0, 0, 12, 9));
        popup.render(buf.area, &mut buf);
        let mut expected = Buffer::with_lines([
            "┌Color:────┐",
            "│ 1 Red    │",
            "│ 2 Orange │",
            "│ 3 Yellow │",
            "│ 4 Green  │",
            "│ 5 Blue   │",
            "│ 6 Violet │",
            "│   None   │",
            "└──────────┘",
        ]);
        for (y, color) in [
            Color::Red,
            Color::Indexed(208),
            Color::Yellow,
            Color::Green,
            Color::Blue,
            Color::Magenta,
        ]
        .into_iter()
        .enumerate()
        {
            let y = u16::try_from(y).unwrap() + 1;
            expected.set_style(Rect::new(1, y, 10, 1), Style::new().fg(color));
        }
        expected.set_style(Rect::new(1, 5, 10, 1), Style::new().reversed());
        assert_eq!(buf, expected);
        state.update(Message::ColorPicker(crate::app::MenuInput::Type('v')));
        let picker = state.color_picker().unwrap();
        let mut buf = Buffer::empty(Rect::new(0, 0, 12, 3));
        Popup::colors(&picker).render(buf.area, &mut buf);
        let mut expected = Buffer::with_lines(["┌Color:────┐", "│ 6 Violet │", "└v─────────┘"]);
        expected.set_style(
            Rect::new(1, 1, 10, 1),
            Style::new().fg(Color::Magenta).reversed(),
        );
        assert_eq!(buf, expected);
    }

    // FR-TAG-05, D-48
    #[test]
    fn a_color_tag_tints_the_background_of_any_block_after_its_indent() {
        let tint = |color| Style::new().bg(tag_tint(color, false));
        let mut expected = Buffer::with_lines(["- a   ", "P:    ", "  S   ", "n     ", "✔ d   "]);
        expected.set_style(Rect::new(0, 0, 6, 1), tint(TagColor::Red));
        expected.set_style(Rect::new(0, 1, 6, 1), tint(TagColor::Orange));
        expected.set_style(Rect::new(0, 1, 2, 1), Style::new().bold().fg(PRIMARY));
        expected.set_style(Rect::new(2, 2, 4, 1), tint(TagColor::Yellow));
        expected.set_style(
            Rect::new(2, 2, 1, 1),
            Style::new().bold().italic().fg(SECONDARY),
        );
        expected.set_style(Rect::new(0, 3, 6, 1), tint(TagColor::Green));
        expected.set_style(Rect::new(0, 3, 1, 1), Style::new().fg(TEXT));
        expected.set_style(Rect::new(0, 4, 6, 1), tint(TagColor::Blue));
        expected.set_style(Rect::new(0, 4, 2, 1), Style::new().dim());
        expected.set_style(Rect::new(2, 4, 1, 1), Style::new().dim().crossed_out());
        assert_eq!(
            render(
                "- a @red\nP: @orange\n\tS @section @yellow\nn @green\n- d @blue @done",
                6,
                5
            ),
            expected
        );
        let mut expected = Buffer::with_lines(["u     "]);
        expected.set_style(Rect::new(0, 0, 6, 1), tint(TagColor::Violet));
        expected.set_style(Rect::new(0, 0, 1, 1), Style::new().underlined().fg(LINK));
        assert_eq!(
            render("u @url(x) @violet", 6, 1),
            expected,
            "a link keeps the link color"
        );
    }

    // D-39, D-48
    #[test]
    fn a_focused_colored_row_gets_a_brighter_tint_after_the_band() {
        let rows = State::new(format::parse("A:\n\t- a @green\n\t- b @green"), None).rows();
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 3));
        OutlineView::new(&rows, TODAY)
            .focused(Some(1))
            .render(Rect::new(0, 0, 8, 3), &mut buf);
        let band = Color::Indexed(235);
        let bg = |x, y| buf[(x, y)].bg;
        assert_eq!(buf[(0, 1)].symbol(), FOCUS_MARKER);
        assert_eq!([bg(0, 1), bg(1, 1), bg(2, 1), bg(3, 1)], [band; 4]);
        assert_eq!(bg(4, 1), tag_tint(TagColor::Green, true));
        assert_eq!(bg(7, 1), tag_tint(TagColor::Green, true));
        assert_eq!(bg(4, 2), tag_tint(TagColor::Green, false));
        assert_eq!(bg(3, 2), Color::Reset, "no tint in the indent");
        assert_eq!(bg(8, 2), Color::Reset, "no tint past the pane");
        assert_ne!(
            tag_tint(TagColor::Green, true),
            tag_tint(TagColor::Green, false)
        );
    }

    // FR-TAG-04, D-47, ui.md §2
    #[test]
    fn today_and_bookmark_icons_follow_the_text() {
        let star = Icon::Today.glyph();
        let bookmark = Icon::Bookmark.glyph();
        let mut expected = Buffer::with_lines([
            at_right(&format!("- a {star}"), &chip(Icon::Due, "Fri"), 20),
            format!("{:<20}", format!("- b {star} {bookmark}")),
            format!("{:<20}", format!("P: {star}")),
            format!("{:<20}", format!("  - c {bookmark}")),
            format!(" {bookmark} {}", "─".repeat(17)),
        ]);
        expected.set_style(Rect::new(4, 0, 1, 1), today_style());
        expected.set_style(Rect::new(15, 0, 5, 1), due_style());
        expected.set_style(Rect::new(4, 1, 1, 1), today_style());
        expected.set_style(Rect::new(6, 1, 1, 1), bookmark_style());
        expected.set_style(
            Rect::new(0, 2, 2, 1),
            Style::new().bold().fg(PRIMARY).dim().crossed_out(),
        );
        expected.set_style(Rect::new(3, 2, 1, 1), today_style().dim());
        expected.set_style(Rect::new(2, 3, 3, 1), Style::new().dim());
        expected.set_style(Rect::new(6, 3, 1, 1), bookmark_style().dim());
        expected.set_style(Rect::new(1, 4, 1, 1), bookmark_style());
        expected.set_style(Rect::new(3, 4, 17, 1), divider_style());
        assert_eq!(
            render(
                "- a @due(2026-10-09) @today\n- b @bookmark @today(x)\nP: @today @done\n\t- c @bookmark\n--- @bookmark",
                20,
                5
            ),
            expected
        );
    }

    // D-47, ui.md §2
    #[test]
    fn a_collapsed_blocks_count_follows_its_icons() {
        let mut state = State::new(format::parse("Q: @bookmark\n\t- d"), None);
        state.update(Message::CollapseAll);
        let mut expected = Buffer::with_lines([format!("Q: {} ▸ 1  ", Icon::Bookmark.glyph())]);
        expected.set_style(Rect::new(0, 0, 2, 1), Style::new().bold().fg(PRIMARY));
        expected.set_style(Rect::new(3, 0, 1, 1), bookmark_style());
        expected.set_style(Rect::new(4, 0, 4, 1), Style::new().dim());
        assert_eq!(draw(&state.rows(), 10, 1), expected);
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
        expected.set_style(
            Rect::new(0, 3, 1, 1),
            Style::new().bold().italic().fg(SECONDARY),
        );
        expected.set_style(Rect::new(1, 3, 4, 1), dim);
        expected.set_style(Rect::new(0, 4, 4, 1), dim);
        expected.set_style(Rect::new(5, 4, 9, 1), divider_style());
        expected.set_style(Rect::new(0, 5, 3, 1), Style::new().fg(TEXT));
        expected.set_style(Rect::new(3, 5, 4, 1), dim);
        assert_eq!(buf, expected);
    }

    // FR-FMT-01, D-45, D-49
    #[test]
    fn inline_formatting_is_drawn_over_the_block_style_with_its_delimiters_hidden() {
        let mut expected = Buffer::with_lines([
            "- a b c https://x.y ",
            "P q:                ",
            "✔ d                 ",
            "n `                 ",
        ]);
        expected.set_style(Rect::new(2, 0, 1, 1), Style::new().bold());
        expected.set_style(Rect::new(4, 0, 1, 1), Style::new().dim());
        expected.set_style(Rect::new(8, 0, 11, 1), Style::new().underlined().fg(LINK));
        let project = Style::new().bold().fg(PRIMARY);
        expected.set_style(Rect::new(0, 1, 4, 1), project);
        expected.set_style(Rect::new(2, 1, 1, 1), project.dim());
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

    // FR-FMT-03, D-45, D-49
    #[test]
    fn text_blocks_draw_headings_and_links_with_their_markers_hidden() {
        let mut expected = Buffer::with_lines([
            "A           ",
            "B           ",
            "C d         ",
            "e f https:/…",
            "H mailto:g  ",
            "- # i       ",
        ]);
        let text = Style::new().fg(TEXT);
        expected.set_style(Rect::new(0, 0, 1, 1), text.bold().underlined());
        expected.set_style(Rect::new(0, 1, 1, 1), text.bold());
        expected.set_style(Rect::new(0, 2, 3, 1), text.bold());
        expected.set_style(Rect::new(2, 2, 1, 1), Style::new().dim());
        let link = text.underlined().fg(LINK);
        expected.set_style(Rect::new(0, 3, 4, 1), text);
        expected.set_style(Rect::new(0, 3, 1, 1), link);
        expected.set_style(Rect::new(4, 3, 8, 1), link);
        expected.set_style(
            Rect::new(0, 4, 12, 1),
            Style::new().bg(tag_tint(TagColor::Red, false)),
        );
        expected.set_style(Rect::new(0, 4, 10, 1), text.bold().underlined());
        expected.set_style(Rect::new(2, 4, 8, 1), Style::new().fg(LINK).underlined());
        expected.set_style(Rect::new(4, 5, 1, 1), Style::new().fg(LINK).underlined());
        assert_eq!(
            render(
                "# A\n## B\n### C `d`\n[e](f) f <https://g.h>\n# H <mailto:g> @red\n- # [i](j)",
                12,
                6
            ),
            expected
        );
    }

    // FR-BLK-06, FR-FMT-03, D-45
    #[test]
    fn every_block_draws_its_links_and_a_line_with_a_url_as_a_link() {
        let mut expected = Buffer::with_lines([
            "P a:        ",
            "S https://b ",
            "- c d       ",
            "- e         ",
            "f           ",
        ]);
        let link = Style::new().underlined().fg(LINK);
        let project = Style::new().bold().fg(PRIMARY);
        expected.set_style(Rect::new(0, 0, 4, 1), project);
        expected.set_style(Rect::new(2, 0, 1, 1), project.patch(link));
        let section = Style::new().bold().italic().fg(SECONDARY);
        expected.set_style(Rect::new(0, 1, 11, 1), section);
        expected.set_style(Rect::new(2, 1, 9, 1), section.patch(link));
        expected.set_style(Rect::new(4, 2, 1, 1), link);
        expected.set_style(Rect::new(2, 3, 1, 1), link);
        expected.set_style(Rect::new(0, 4, 1, 1), Style::new().fg(TEXT).patch(link));
        assert_eq!(
            render(
                "P [a](https://x.y):\nS <https://b> @section\n- c [d](u)\n- e @url(https://x.y)\nf @url",
                12,
                5
            ),
            expected
        );
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

    // FR-BLK-07, D-52
    #[test]
    fn the_editor_shows_a_dividers_dashes_as_text() {
        let (buf, _) = edited("--- @x", &editor("---", 3, None), 5);
        let mut expected = Buffer::with_lines(["---  "]);
        expected.set_style(Rect::new(0, 0, 3, 1), Style::new().fg(TEXT));
        assert_eq!(buf, expected);
    }

    // FR-MODE-02
    #[test]
    fn the_editor_shows_a_projects_colon_as_text_styled_as_the_project() {
        let (buf, caret) = edited("Inbox:", &editor("Inbox:", 5, None), 8);
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
            at_right(
                "- a",
                &format!("{} {}", chip(Icon::Start, "today"), chip(Icon::Due, "Fri")),
                34,
            ),
            at_right("- b", &chip(Icon::Overdue, "yesterday"), 34),
            at_right("- c", &chip(Icon::Start, "in 361 days"), 34),
            "- d                               ".to_owned(),
        ]);
        expected.set_style(Rect::new(21, 0, 7, 1), start_style());
        expected.set_style(Rect::new(29, 0, 5, 1), due_style());
        expected.set_style(Rect::new(23, 1, 11, 1), overdue_style());
        expected.set_style(Rect::new(21, 2, 13, 1), start_style());
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
        let mut expected = Buffer::with_lines([
            at_right("P:", &chip(Icon::Due, "yesterday"), 19),
            at_right("  - a", &chip(Icon::Due, "yesterday"), 19),
        ]);
        expected.set_style(
            Rect::new(0, 0, 2, 1),
            Style::new().bold().fg(PRIMARY).dim().crossed_out(),
        );
        expected.set_style(Rect::new(8, 0, 11, 1), due_style().dim());
        expected.set_style(Rect::new(2, 1, 3, 1), Style::new().dim());
        expected.set_style(Rect::new(8, 1, 11, 1), due_style().dim());
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
        let mut expected =
            Buffer::with_lines([at_right("▌ - a", &chip(Icon::Overdue, "yesterday"), 20)]);
        expected.set_style(expected.area, current_line_style());
        expected.set_style(Rect::new(0, 0, 1, 1), Style::new().fg(PRIMARY));
        expected.set_style(Rect::new(9, 0, 11, 1), overdue_style());
        assert_eq!(buf, expected);
    }

    // FR-TAG-03, FR-MODE-02, D-47
    #[test]
    fn the_edited_block_shows_no_chips_or_icons() {
        let mut state = State::new(
            format::parse("- a @due(2026-10-05) @today @bookmark\n- b @due(2026-10-05)"),
            None,
        );
        state.update(Message::EditAtEnd);
        let rows = state.rows();
        let editor = state.editor();
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 2));
        OutlineView::new(&rows, TODAY)
            .focused(Some(0))
            .editor(editor.as_ref())
            .render(buf.area, &mut buf);
        let mut expected = Buffer::with_lines([
            "▌ - a               ".to_owned(),
            at_right("  - b", &chip(Icon::Due, "today"), 20),
        ]);
        expected.set_style(Rect::new(0, 0, 20, 1), current_line_style());
        expected.set_style(Rect::new(0, 0, 1, 1), Style::new().fg(ACCENT));
        expected.set_style(Rect::new(13, 1, 7, 1), due_style());
        assert_eq!(
            buf, expected,
            "the marker is in the accent while editing (D-51)"
        );
    }

    // FR-UI-05
    #[test]
    fn with_wrap_off_text_that_does_not_fit_ends_in_an_ellipsis_and_rows_below_are_clipped() {
        let expected = Buffer::with_lines(["- ab…", "- d  "]);
        assert_eq!(render("- abcdef\n- d\n- e", 5, 2), expected);
    }

    // D-30, D-39
    #[test]
    fn marks_the_focused_row_in_the_gutter_on_a_band_without_changing_its_text() {
        let mut expected = Buffer::with_lines(["  - a", "▌ - b", "  - c"]);
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
        let mut expected = Buffer::with_lines(["  - c ", "▌ - d "]);
        expected.area = area;
        expected.set_style(Rect::new(2, 2, 6, 1), current_line_style());
        expected.set_style(Rect::new(2, 2, 1, 1), Style::new().fg(PRIMARY));
        assert_eq!(render_scrolled(2, Some(3), area), expected);
    }

    // FR-OPS-01, ui.md §2
    #[test]
    fn marks_each_selected_row_with_a_dimmer_marker_and_no_band() {
        let mut state = State::new(format::parse("- a\n- b\n- c"), None);
        state.update(Message::Select(SelectionChange::ExtendDown));
        let rows = state.rows();
        let area = Rect::new(0, 0, 5, 3);
        let mut buf = Buffer::empty(area);
        OutlineView::new(&rows, TODAY)
            .focused(Some(1))
            .render(area, &mut buf);
        let mut expected = Buffer::with_lines(["▌ - a", "▌ - b", "  - c"]);
        expected.set_style(Rect::new(0, 1, 5, 1), current_line_style());
        expected.set_style(Rect::new(0, 1, 1, 1), Style::new().fg(PRIMARY));
        expected.set_style(Rect::new(0, 0, 1, 1), selection_style());
        assert_eq!(buf, expected);
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

    /// The text of each line of `buf`, without trailing blanks.
    fn lines(buf: &Buffer) -> Vec<String> {
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

    /// Draws `text` with wrap on in a `width` × `height` area, the gutter included, focusing row
    /// `focused` and starting at line `offset`.
    fn wrapped(text: &str, width: u16, height: u16, focused: usize, offset: usize) -> Buffer {
        let rows = State::new(format::parse(text), None).rows();
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        OutlineView::new(&rows, TODAY)
            .wrap(true)
            .focused(Some(focused))
            .offset(offset)
            .render(buf.area, &mut buf);
        buf
    }

    // FR-UI-05, FR-TAG-03
    #[test]
    fn wrapped_text_goes_on_under_where_it_starts_and_stops_before_the_chips() {
        let buf = wrapped(
            "P:\n\t- buy milk and eggs @due(2026-10-05)\n\tnotes on the long trip\n- end",
            22,
            6,
            3,
            0,
        );
        assert_eq!(
            lines(&buf),
            [
                "  P:",
                &at_right("    - buy milk", &chip(Icon::Due, "today"), 22),
                "      and eggs",
                "    notes on the long",
                "    trip",
                "▌ - end",
            ]
        );
    }

    // FR-UI-05, D-30, D-39, D-48
    #[test]
    fn every_line_of_a_wrapped_block_gets_its_marker_band_and_tint() {
        let buf = wrapped("- aaa bbb @red\n- c", 9, 3, 0, 0);
        assert_eq!(lines(&buf), ["▌ - aaa", "▌   bbb", "  - c"]);
        for y in 0..2 {
            assert_eq!(buf[(0, y)].fg, PRIMARY, "line {y}");
            assert_eq!(buf[(0, y)].bg, Color::Indexed(235), "line {y}");
            assert_eq!(buf[(4, y)].bg, tag_tint(TagColor::Red, true), "line {y}");
        }
        assert_eq!(buf[(0, 2)].bg, Color::Reset);
    }

    // FR-UI-05
    #[test]
    fn a_wrapped_view_starts_at_its_offset_inside_a_block() {
        let buf = wrapped("- aaa bbb ccc\n- d", 9, 2, 1, 1);
        assert_eq!(lines(&buf), ["    bbb", "    ccc"]);
        assert_eq!(buf[(0, 0)].symbol(), " ", "the focused block starts below");
    }

    // FR-UI-05, FR-MODE-02
    #[test]
    fn heights_count_the_lines_of_each_row_and_one_for_the_edited_row() {
        let mut state = State::new(format::parse("- aaa bbb ccc\n- d\n- eee fff"), None);
        let rows = state.rows();
        let view = OutlineView::new(&rows, TODAY);
        assert_eq!(view.heights(9), [1, 1, 1], "wrap off");
        assert_eq!(view.wrap(true).heights(9), [3, 1, 2]);
        state.update(Message::EditAtEnd);
        let editor = state.editor();
        let rows = state.rows();
        let view = OutlineView::new(&rows, TODAY)
            .wrap(true)
            .editor(editor.as_ref());
        assert_eq!(view.heights(9), [1, 1, 2]);
        assert_eq!(view.heights(0), [1, 1, 1], "no room for text");
    }

    // FR-UI-05, FR-TAG-03
    #[test]
    fn with_wrap_off_text_ends_in_an_ellipsis_before_the_chips() {
        assert_eq!(
            lines(&render("- buy milk and eggs @due(2026-10-05)", 20, 1)),
            [at_right("- buy milk …", &chip(Icon::Due, "today"), 20)]
        );
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
                Notice::NotSections { skipped: 2 },
                (
                    "Only text can be a section: 2 skipped".to_owned(),
                    warning_style(),
                ),
            ),
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
