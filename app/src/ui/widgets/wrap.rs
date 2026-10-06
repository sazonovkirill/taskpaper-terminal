//! Fitting a block's screen line into the outline's width: wrapped under the column its text
//! starts at, or cut short with `…` ([ui.md §2](../../../../docs/specs/ui.md#2-blocks),
//! FR-UI-05). Widths are display widths, and graphemes are never split.

use ratatui::{
    style::Style,
    text::{Line, Span},
};
use unicode_segmentation::UnicodeSegmentation;

/// One grapheme of a line with its style.
type Cell<'a> = (&'a str, Style);

/// `line` in lines `rest` columns wide, the first of them `first` columns wide, broken between
/// words where it can and inside a word wider than a line. Its first `start` columns, the indent
/// and the marker, stay on the first line; the lines after it start under the text, without the
/// spaces they were broken at. When no column is left after `start`, `line` stays one line.
pub(super) fn wrap(line: &Line, start: usize, first: usize, rest: usize) -> Vec<Line<'static>> {
    let cells = cells(line);
    let full = rest.saturating_sub(start);
    let at = split(&cells, start);
    let (prefix, body) = cells.split_at(at);
    if full == 0 {
        return vec![styled(&cells, line.style)];
    }
    let mut lines: Vec<Vec<Cell>> = vec![prefix.to_vec()];
    let (mut room, mut used) = (first.saturating_sub(start), 0);
    // The spaces before the next word, which a line breaks at instead of ending in them.
    let mut gap: &[Cell] = &[];
    for word in body.chunk_by(|a, b| is_blank(a.0) == is_blank(b.0)) {
        if word.iter().all(|cell| is_blank(cell.0)) {
            gap = word;
            continue;
        }
        let word_width = width(word);
        if used + width(gap) + word_width > room && word_width <= full {
            lines.push(Vec::new());
            (room, used, gap) = (full, 0, &[]);
        }
        let current = lines.last_mut().expect("lines start with one");
        current.extend_from_slice(gap);
        used += width(gap);
        gap = &[];
        for &cell in word {
            let cell_width = width(&[cell]);
            // An empty line as wide as any takes even a grapheme wider than it, which is cut.
            if used + cell_width > room && !(used == 0 && room == full) {
                lines.push(Vec::new());
                (room, used) = (full, 0);
            }
            lines.last_mut().expect("lines start with one").push(cell);
            used += cell_width;
        }
    }
    let indent = " ".repeat(start);
    lines
        .into_iter()
        .enumerate()
        .map(|(index, mut cells)| {
            if index > 0 {
                cells.insert(0, (&indent, Style::new()));
            }
            styled(&cells, line.style)
        })
        .collect()
}

/// `line` cut to `columns` columns, its last column a `…` in the style of the first grapheme cut,
/// when it is wider; as it is otherwise.
pub(super) fn truncate(line: &Line, columns: usize) -> Line<'static> {
    let cells = cells(line);
    if width(&cells) <= columns {
        return styled(&cells, line.style);
    }
    let (kept, cut) = cells.split_at(split(&cells, columns.saturating_sub(1)));
    let mut cells = kept.to_vec();
    if columns > 0
        && let Some(&(_, style)) = cut.first()
    {
        cells.push(("…", style));
    }
    styled(&cells, line.style)
}

/// The graphemes of `line`, each with the style of its span.
fn cells<'a>(line: &'a Line) -> Vec<Cell<'a>> {
    line.spans
        .iter()
        .flat_map(|span| {
            span.content
                .graphemes(true)
                .map(move |grapheme| (grapheme, span.style))
        })
        .collect()
}

/// How many of `cells`, from the first, fit in `columns` columns.
fn split(cells: &[Cell], columns: usize) -> usize {
    let mut used = 0;
    cells
        .iter()
        .take_while(|&&cell| {
            used += width(&[cell]);
            used <= columns
        })
        .count()
}

/// The display width of `cells`.
fn width(cells: &[Cell]) -> usize {
    cells
        .iter()
        .map(|(grapheme, _)| Span::raw(*grapheme).width())
        .sum()
}

/// Whether `grapheme` is white space, where a line may break.
fn is_blank(grapheme: &str) -> bool {
    grapheme.chars().all(char::is_whitespace)
}

/// A line of `cells`, each run in one style a span of its own, in `style` as a whole.
fn styled(cells: &[Cell], style: Style) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for &(grapheme, cell_style) in cells {
        match spans.last_mut() {
            Some(span) if span.style == cell_style => span.content.to_mut().push_str(grapheme),
            _ => spans.push(Span::styled(grapheme.to_owned(), cell_style)),
        }
    }
    Line::from(spans).style(style)
}

#[cfg(test)]
mod tests {
    use ratatui::style::Stylize;

    use super::*;

    /// The text of each line.
    fn texts(lines: &[Line]) -> Vec<String> {
        lines.iter().map(ToString::to_string).collect()
    }

    /// A task's line at depth 1: 2 columns of indent and a marker, then `text`.
    fn task(text: &str) -> Line<'_> {
        Line::from(vec![Span::raw("  "), Span::raw("- "), Span::raw(text)])
    }

    #[track_caller]
    fn check(line: &Line, start: usize, first: usize, rest: usize, expected: &[&str]) {
        assert_eq!(texts(&wrap(line, start, first, rest)), expected);
    }

    // FR-UI-05
    #[test]
    fn a_line_that_fits_stays_as_it_is() {
        check(&task("buy milk"), 4, 12, 12, &["  - buy milk"]);
        check(&task(""), 4, 12, 12, &["  - "]);
        check(&task(" a  b"), 4, 12, 12, &["  -  a  b"]);
    }

    // FR-UI-05
    #[test]
    fn long_text_breaks_between_words_under_where_it_starts() {
        check(
            &task("buy milk and eggs today"),
            4,
            14,
            14,
            &["  - buy milk", "    and eggs", "    today"],
        );
        check(&task("one  two"), 4, 8, 8, &["  - one", "    two"]);
    }

    // FR-UI-05
    #[test]
    fn the_first_line_can_be_narrower_for_the_chips() {
        check(&task("aa bb cc dd"), 4, 7, 12, &["  - aa", "    bb cc dd"]);
        check(&task("aa bb"), 4, 4, 12, &["  - ", "    aa bb"]);
    }

    // FR-UI-05
    #[test]
    fn a_word_wider_than_a_line_breaks_inside_it() {
        check(
            &task("abcdefgh ij"),
            4,
            7,
            7,
            &["  - abc", "    def", "    gh", "    ij"],
        );
        check(&Line::from("abcdefg"), 0, 3, 3, &["abc", "def", "g"]);
    }

    // FR-UI-05: widths are display widths and graphemes stay whole.
    #[test]
    fn wide_characters_and_graphemes_are_never_split() {
        check(&Line::from("日本語"), 0, 4, 4, &["日本", "語"]);
        check(
            &Line::from("e\u{301}e\u{301}e\u{301}"),
            0,
            2,
            2,
            &["e\u{301}e\u{301}", "e\u{301}"],
        );
        check(&Line::from("日x"), 0, 1, 1, &["日", "x"]);
    }

    // FR-UI-05
    #[test]
    fn without_room_after_the_indent_the_line_stays_whole() {
        check(&task("abc def"), 4, 4, 4, &["  - abc def"]);
    }

    // FR-UI-05
    #[test]
    fn styles_carry_over_to_the_next_line() {
        let line = Line::from(vec![Span::raw("- "), "ab cd".bold()]).dim();
        let lines = wrap(&line, 2, 4, 4);
        assert_eq!(
            lines,
            [
                Line::from(vec![Span::raw("- "), "ab".bold()]).dim(),
                Line::from(vec![Span::raw("  "), "cd".bold()]).dim(),
            ]
        );
    }

    // FR-UI-05
    #[test]
    fn a_line_too_wide_ends_in_an_ellipsis_in_the_style_of_what_it_cuts() {
        let line = Line::from(vec![Span::raw("- "), "abc".bold(), "def".italic()]);
        assert_eq!(
            truncate(&line, 8),
            Line::from(vec![Span::raw("- "), "abc".bold(), "def".italic()])
        );
        assert_eq!(
            truncate(&line, 7),
            Line::from(vec![Span::raw("- "), "abc".bold(), "d…".italic()])
        );
        assert_eq!(
            truncate(&line, 5),
            Line::from(vec![Span::raw("- "), "ab…".bold()])
        );
        assert_eq!(truncate(&line, 1), Line::from("…"));
        assert_eq!(truncate(&line, 0), Line::default());
    }

    // FR-UI-05
    #[test]
    fn an_ellipsis_never_splits_a_wide_character() {
        assert_eq!(truncate(&Line::from("日本語"), 4), Line::from("日…"));
        assert_eq!(truncate(&Line::from("日本語"), 6), Line::from("日本語"));
    }
}
