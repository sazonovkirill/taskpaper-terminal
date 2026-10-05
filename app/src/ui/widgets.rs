//! Widgets that draw the core's view state ([ui.md §2](../../../docs/specs/ui.md#2-blocks)).

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Clear, Widget},
};

use unicode_segmentation::UnicodeSegmentation;

use crate::{
    app::{Access, DiffLine, Editor, Notice, Row, SaveState},
    document::BlockKind,
};

/// The outline: a gutter that marks the focused block, then one row per block, 2 columns of
/// indent per level, a marker and the text; a block with several lines takes several screen
/// lines. A block being edited shows the text as typed instead.
#[derive(Debug)]
pub struct OutlineView<'a> {
    rows: &'a [Row],
    offset: usize,
    focused: Option<usize>,
    editor: Option<&'a Editor<'a>>,
}

impl<'a> OutlineView<'a> {
    /// A view of `rows`, drawn from the top of its area with no row focused.
    #[must_use]
    pub fn new(rows: &'a [Row]) -> Self {
        Self {
            rows,
            offset: 0,
            focused: None,
            editor: None,
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
        let lines = self.rows.iter().enumerate().flat_map(|(index, row)| {
            let lines = match self.editor.filter(|editor| editor.id == row.id) {
                Some(editor) => vec![editor_line(row, editor, width).0],
                None => row_lines(row, width),
            };
            lines.into_iter().map(move |line| (index, line))
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
        Notice::NotesMerged => Span::raw("Notes merged"),
        Notice::BlanksLifted => Span::raw("Empty lines at the end moved to the top level"),
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

/// The screen lines of one block, `width` columns wide: indented 2 columns per level of depth
/// whatever its type, then the marker of a task or URL, then the text (D-31). A collapsed block
/// with children shows `▸ n` after its text (ui.md §2). The zoom title is drawn like its block,
/// in bold (D-34).
fn row_lines(row: &Row, width: usize) -> Vec<Line<'_>> {
    let indent = "  ".repeat(row.depth);
    let (marker, marker_style, style) = row_style(row);
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
            let marker = if index == 0 {
                Span::styled(marker, marker_style)
            } else {
                Span::raw(" ".repeat(marker.chars().count()))
            };
            let mut line = Line::from(vec![Span::raw(indent.clone()), marker]);
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

/// The marker of a block's first line with its style, and the style of its text, by its type
/// (ui.md §2).
fn row_style(row: &Row) -> (&'static str, Style, Style) {
    let (marker, style) = match row.kind {
        BlockKind::Project => ("", Style::new().bold().fg(PRIMARY)),
        BlockKind::Task if row.done => ("✔ ", Style::new().dim().crossed_out()),
        BlockKind::Task => ("- ", Style::new()),
        BlockKind::Url => ("↗ ", Style::new().dim().underlined().cyan()),
        BlockKind::Section => ("", Style::new().bold().fg(SECONDARY)),
        BlockKind::Code => ("", code_style().dim()),
        BlockKind::Note => ("", Style::new().fg(NOTE)),
        BlockKind::Divider => ("", divider_style()),
    };
    let style = if row.title { style.bold() } else { style };
    // A done task's check mark recedes with its text (D-36).
    let marker_style = if row.kind == BlockKind::Task && row.done {
        Style::new().dim()
    } else {
        Style::new()
    };
    (marker, marker_style, style)
}

/// The screen line of the one-line block `row` while `editor` edits it, `width` columns wide,
/// and the caret's column in it: the row's indent and marker, the text as typed with the
/// selection reversed, then a project's `:`, all styled as the row (FR-MODE-02). When the caret
/// would be past the right edge, the text scrolls left by whole graphemes to keep it in view.
#[must_use]
pub fn editor_line<'a>(row: &Row, editor: &'a Editor, width: usize) -> (Line<'a>, usize) {
    let (marker, marker_style, style) = row_style(row);
    let mut line = Line::from(vec![
        Span::raw("  ".repeat(row.depth)),
        Span::styled(marker, marker_style),
    ]);
    let before_caret = editor.text.get(..editor.caret).unwrap_or_default();
    let caret = line.width() + Span::raw(before_caret).width();
    // The caret takes a column of its own after the text before it.
    let overflow = (caret + 1).saturating_sub(width);
    let (mut start, mut scrolled) = (0, 0);
    for (at, grapheme) in editor.text.grapheme_indices(true) {
        if scrolled >= overflow || at >= editor.caret {
            break;
        }
        start = at + grapheme.len();
        scrolled += Span::raw(grapheme).width();
    }
    let selection = editor
        .selection
        .clone()
        .map(|selection| selection.start.max(start)..selection.end.max(start))
        .unwrap_or(start..start);
    let piece = |range: std::ops::Range<usize>| editor.text.get(range).unwrap_or_default();
    line.push_span(Span::styled(piece(start..selection.start), style));
    line.push_span(Span::styled(piece(selection.clone()), style.reversed()));
    line.push_span(Span::styled(piece(selection.end..editor.text.len()), style));
    line.push_span(Span::styled(editor.suffix, style));
    if row.kind == BlockKind::Code {
        let fill = width.saturating_sub(line.width());
        line.push_span(Span::styled(" ".repeat(fill), code_style()));
    }
    (line, caret - scrolled)
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

/// Note text: dimmed, but less than done tasks and the other dim blocks (D-37); themes arrive in
/// step 6.2.
const NOTE: Color = Color::Gray;

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
        expected.set_style(Rect::new(4, 2, 7, 1), Style::new().fg(NOTE));
        expected.set_style(Rect::new(0, 3, 4, 1), Style::new().fg(NOTE));
        assert_eq!(
            render("Home:\n\t- Milk\n\t\ton sale\nnote", 13, 4),
            expected
        );
    }

    // FR-DONE-01
    #[test]
    fn done_tasks_are_checked_dimmed_and_struck_through() {
        let mut expected = Buffer::with_lines(["✔ a ", "- b ", "P:  ", "n   "]);
        expected.set_style(Rect::new(0, 0, 2, 1), Style::new().dim());
        expected.set_style(Rect::new(2, 0, 1, 1), Style::new().dim().crossed_out());
        expected.set_style(Rect::new(0, 2, 2, 1), Style::new().bold().fg(PRIMARY));
        expected.set_style(Rect::new(0, 3, 1, 1), Style::new().fg(NOTE));
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
        expected.set_style(Rect::new(0, 1, 1, 2), Style::new().fg(NOTE));
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
    fn the_title_row_is_styled_by_its_type_in_bold_over_its_indented_children() {
        let title = |text: &str| {
            let mut state = State::new(format::parse(text), None);
            state.update(Message::FocusNext);
            state.update(Message::ZoomIn);
            draw(&state.rows(), 6, 3)
        };
        let mut expected = Buffer::with_lines(["n     ", "m     ", "  - t "]);
        expected.set_style(Rect::new(0, 0, 1, 2), Style::new().bold().fg(NOTE));
        assert_eq!(title("P:\n\tn\n\tm\n\t\t- t"), expected, "a note is dim");
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

    fn editor(text: &str, caret: usize, selection: Option<std::ops::Range<usize>>) -> Editor<'_> {
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
    fn the_editor_of_a_code_block_fills_its_band() {
        let (buf, _) = edited("f() @code", &editor("f()", 3, None), 5);
        let mut expected = Buffer::with_lines(["f()  "]);
        expected.set_style(Rect::new(0, 0, 5, 1), code_style());
        expected.set_style(Rect::new(0, 0, 3, 1), code_style().dim());
        assert_eq!(buf, expected);
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
        OutlineView::new(&rows)
            .editor(editor.as_ref())
            .render(buf.area, &mut buf);
        let mut expected = Buffer::with_lines(["  - a  ", "  - bc "]);
        expected.set_style(Rect::new(4, 1, 2, 1), Style::new().reversed());
        assert_eq!(buf, expected);
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
        expected.set_style(Rect::new(2, 1, 1, 2), Style::new().fg(NOTE));
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
            (Notice::NotesMerged, plain("Notes merged")),
            (
                Notice::BlanksLifted,
                plain("Empty lines at the end moved to the top level"),
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
