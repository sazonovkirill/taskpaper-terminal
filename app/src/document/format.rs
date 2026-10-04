//! Parsing `.taskpaper` text into a [`Document`] ([file-format.md §1–4](../../../docs/specs/file-format.md)).
//! Pure: it takes bytes or strings and never touches files.

pub mod tags;

use thiserror::Error;

use super::model::{Document, FileFormat, IndentUnit, ItemId, ItemKind, LineEnding};
use tags::is_whitespace;

const BOM: &str = "\u{FEFF}";

/// Why bytes could not be read as a document.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DecodeError {
    /// The bytes are not UTF-8; opening the file would risk corrupting it (F-FILE-07).
    #[error(
        "the file is not valid UTF-8 (first invalid byte on line {line}, at byte offset {offset})"
    )]
    NotUtf8 {
        /// 1-based line of the first invalid byte.
        line: usize,
        /// Byte offset of the first invalid byte, counting a BOM.
        offset: usize,
    },
}

/// Decodes file bytes, remembering a UTF-8 BOM, and parses them.
///
/// # Errors
///
/// [`DecodeError::NotUtf8`] when the bytes are not valid UTF-8.
pub fn parse_bytes(bytes: &[u8]) -> Result<Document, DecodeError> {
    let stripped = bytes.strip_prefix(BOM.as_bytes());
    let bom = stripped.is_some();
    let body = stripped.unwrap_or(bytes);
    let text = str::from_utf8(body).map_err(|error| {
        let valid = body.get(..error.valid_up_to()).unwrap_or_default();
        DecodeError::NotUtf8 {
            line: valid.split(|&byte| byte == b'\n').count(),
            offset: error.valid_up_to() + if bom { BOM.len() } else { 0 },
        }
    })?;
    Ok(parse_text(text, bom))
}

/// Parses text: line breaks, indentation, hierarchy and blank lines as TaskPaper does.
#[must_use]
pub fn parse(text: &str) -> Document {
    parse_text(text, false)
}

fn parse_text(text: &str, bom: bool) -> Document {
    let (mut lines, line_ending) = split_lines(text);
    let trailing_newline = lines.last() == Some(&"") && lines.len() > 1;
    if trailing_newline || text.is_empty() {
        lines.pop();
    }
    let (indented, indent) = indent_lines(&lines);
    let mut document = Document::new(FileFormat {
        indent,
        line_ending,
        bom,
        trailing_newline,
    });
    build_tree(&mut document, &blank_line_depths(indented));
    document
}

/// The TaskPaper type of a line body.
#[must_use]
pub fn kind(text: &str) -> ItemKind {
    let content = content(text);
    let mut chars = content.chars();
    if chars.next() == Some('-') && chars.next().is_some_and(is_whitespace) {
        ItemKind::Task
    } else if content.ends_with(':') {
        ItemKind::Project
    } else {
        ItemKind::Note
    }
}

/// The body without a task's `- `, a project's `:` and every tag (data-model.md §2).
#[must_use]
pub fn display_text(text: &str) -> String {
    let shown = without_tags(text);
    match kind(text) {
        ItemKind::Task => shown.chars().skip(2).collect(),
        ItemKind::Project => shown.strip_suffix(':').unwrap_or(&shown).to_owned(),
        ItemKind::Note => shown,
    }
}

/// The line's tags, keeping only the first occurrence of each name (file-format.md §5).
#[must_use]
pub fn tags(text: &str) -> Vec<tags::Tag> {
    let mut found: Vec<tags::Tag> = Vec::new();
    for tag in tags::scan(text) {
        if found.iter().all(|seen| seen.name != tag.name) {
            found.push(tag);
        }
    }
    found
}

/// Removes every tag occurrence. A tag's span starts at the whitespace before it, so the text
/// after a tag starts with the whitespace that followed it; that collapses to one space between
/// the remaining text on either side, or to nothing at either end of the line.
fn without_tags(text: &str) -> String {
    let mut shown = String::with_capacity(text.len());
    let mut rest = 0;
    let line_end = text.len()..text.len();
    for span in tags::scan(text)
        .into_iter()
        .map(|tag| tag.span)
        .chain([line_end])
    {
        let piece = text.get(rest..span.start).unwrap_or_default();
        let piece = if rest == 0 {
            piece
        } else {
            piece.trim_start_matches(is_whitespace)
        };
        if !piece.is_empty() {
            if !shown.is_empty() {
                shown.push(' ');
            }
            shown.push_str(piece);
        }
        rest = span.end;
    }
    shown
}

/// The body without its trailing tag run, which decides the item type.
fn content(text: &str) -> &str {
    text.get(..tags::trailing_run_start(text)).unwrap_or(text)
}

/// Splits on every TaskPaper paragraph break and finds the dominant line ending.
fn split_lines(text: &str) -> (Vec<&str>, LineEnding) {
    let mut lines = Vec::new();
    let (mut lf, mut crlf, mut cr) = (0, 0, 0);
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((index, c)) = chars.next() {
        let end = match c {
            '\r' if chars.peek().is_some_and(|&(_, next)| next == '\n') => {
                chars.next();
                crlf += 1;
                index + 2
            }
            '\r' => {
                cr += 1;
                index + 1
            }
            '\n' => {
                lf += 1;
                index + 1
            }
            '\u{C}' | '\u{2029}' | '\u{2B7F}' => index + c.len_utf8(),
            _ => continue,
        };
        lines.push(text.get(start..index).unwrap_or_default());
        start = end;
    }
    lines.push(text.get(start..).unwrap_or_default());
    let line_ending = if crlf > lf && crlf >= cr {
        LineEnding::CrLf
    } else if cr > lf {
        LineEnding::Cr
    } else {
        LineEnding::Lf
    };
    (lines, line_ending)
}

/// A line split into its absolute depth and its body.
struct Line<'a> {
    depth: usize,
    body: &'a str,
}

/// Turns leading whitespace into depths, normalizing space indentation in multi-line input.
fn indent_lines<'a>(lines: &[&'a str]) -> (Vec<Line<'a>>, IndentUnit) {
    let spaces_per_level = (lines.len() > 1)
        .then(|| lines.iter().flat_map(|line| leading_space_runs(line)).min())
        .flatten();
    let indented = lines
        .iter()
        .map(|line| {
            if lines.len() > 1 {
                normalize(line, spaces_per_level)
            } else {
                tabs_only(line)
            }
        })
        .collect();
    let unit = spaces_per_level.map_or(IndentUnit::Tab, IndentUnit::Spaces);
    (indented, unit)
}

/// Lengths of the runs of spaces in a line's leading whitespace.
fn leading_space_runs(line: &str) -> impl Iterator<Item = usize> + '_ {
    let indent_len = line.len() - line.trim_start_matches([' ', '\t']).len();
    line.get(..indent_len)
        .unwrap_or_default()
        .split('\t')
        .map(str::len)
        .filter(|&run| run > 0)
}

/// Converts leading groups of `per_level` spaces to levels, then drops leftover leading whitespace
/// from the first space on, as TaskPaper does.
fn normalize(line: &str, spaces_per_level: Option<usize>) -> Line<'_> {
    let leading_spaces = line.len() - line.trim_start_matches(' ').len();
    let (groups, grouped) = spaces_per_level.map_or((0, 0), |per_level| {
        let groups = leading_spaces / per_level;
        (groups, groups * per_level)
    });
    let after_groups = line.get(grouped..).unwrap_or_default();
    let tabs = after_groups.len() - after_groups.trim_start_matches('\t').len();
    Line {
        depth: groups + tabs,
        body: line.trim_start_matches([' ', '\t']),
    }
}

/// Single-line input is not normalized: only leading tabs indent, other whitespace is text.
fn tabs_only(line: &str) -> Line<'_> {
    let body = line.trim_start_matches('\t');
    Line {
        depth: line.len() - body.len(),
        body,
    }
}

/// Blank lines take the depth of the next non-blank line when it is deeper; trailing ones are top-level.
fn blank_line_depths(mut lines: Vec<Line<'_>>) -> Vec<Line<'_>> {
    let mut next_depth = None;
    for line in lines.iter_mut().rev() {
        if line.body.chars().all(is_whitespace) {
            line.body = "";
            line.depth = next_depth.map_or(0, |next| line.depth.max(next));
        } else {
            next_depth = Some(line.depth);
        }
    }
    lines
}

/// A deeper line is a child of the previous one; a shallower one pops to the nearest shallower ancestor.
fn build_tree(document: &mut Document, lines: &[Line<'_>]) {
    let mut ancestors: Vec<(ItemId, usize)> = Vec::new();
    for line in lines {
        while ancestors
            .last()
            .is_some_and(|&(_, depth)| depth >= line.depth)
        {
            ancestors.pop();
        }
        let (parent, expected) = ancestors
            .last()
            .map_or((document.root(), 0), |&(id, depth)| (id, depth + 1));
        let id = document.push_child(parent, line.body.to_owned(), line.depth - expected);
        ancestors.push((id, line.depth));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each item as `depth extra_indent "text"`, in document order.
    fn outline(document: &Document) -> Vec<String> {
        document
            .descendants(document.root())
            .map(|(id, depth)| {
                let item = document.get(id).unwrap();
                format!("{depth} {} {:?}", item.extra_indent(), item.text())
            })
            .collect()
    }

    #[track_caller]
    fn check(text: &str, expected: &[&str]) {
        assert_eq!(outline(&parse(text)), expected, "parsing {text:?}");
    }

    #[test]
    fn builds_hierarchy_from_tabs() {
        check(
            "A:\n\t- a1\n\t\tnote\n\t- a2\nB:\n",
            &[
                r#"0 0 "A:""#,
                r#"1 0 "- a1""#,
                r#"2 0 "note""#,
                r#"1 0 "- a2""#,
                r#"0 0 "B:""#,
            ],
        );
    }

    #[test]
    fn indent_jumps_keep_their_extra_indent() {
        check(
            "A\n\t\t\tB\n\tC",
            &[r#"0 0 "A""#, r#"1 2 "B""#, r#"1 0 "C""#],
        );
    }

    #[test]
    fn shallower_line_pops_to_the_nearest_shallower_ancestor() {
        check("A\n\t\tB\n\tC", &[r#"0 0 "A""#, r#"1 1 "B""#, r#"1 0 "C""#]);
    }

    #[test]
    fn over_indented_first_line_is_a_root_with_extra_indent() {
        check("\t\tA\nB", &[r#"0 2 "A""#, r#"0 0 "B""#]);
    }

    #[test]
    fn spaces_are_normalized_by_the_smallest_run() {
        check(
            "A\n    B\n        C\n    D",
            &[r#"0 0 "A""#, r#"1 0 "B""#, r#"2 0 "C""#, r#"1 0 "D""#],
        );
        assert_eq!(parse("A\n    B").format().indent, IndentUnit::Spaces(4));
    }

    #[test]
    fn one_stray_short_run_skews_every_depth() {
        check("A\n    B\n  C", &[r#"0 0 "A""#, r#"1 1 "B""#, r#"1 0 "C""#]);
    }

    #[test]
    fn leftover_spaces_and_tabs_after_them_are_dropped() {
        check("A\n  B\n   C", &[r#"0 0 "A""#, r#"1 0 "B""#, r#"1 0 "C""#]);
        check(
            "A\n  B\n\t   \tC",
            &[r#"0 0 "A""#, r#"1 0 "B""#, r#"1 0 "C""#],
        );
    }

    #[test]
    fn space_runs_before_tabs_count() {
        check(
            "A\n \tB\n    C",
            &[r#"0 0 "A""#, r#"1 1 "B""#, r#"2 1 "C""#],
        );
    }

    #[test]
    fn tabs_after_space_groups_add_levels() {
        check("A\n  \tB", &[r#"0 0 "A""#, r#"1 1 "B""#]);
    }

    #[test]
    fn tab_indented_files_use_tabs() {
        assert_eq!(parse("A\n\tB").format().indent, IndentUnit::Tab);
    }

    #[test]
    fn single_line_input_is_not_normalized() {
        check("  - Milk", &[r#"0 0 "  - Milk""#]);
        check("  - Milk\n", &[r#"0 0 "  - Milk""#]);
        check("\t- Milk", &[r#"0 1 "- Milk""#]);
        assert_eq!(kind("  - Milk"), ItemKind::Note);
        assert_eq!(parse("  - Milk").format().indent, IndentUnit::Tab);
    }

    #[test]
    fn blank_lines_take_the_next_deeper_depth() {
        check(
            "A:\n\t- a\n\n\t- b\n\nB:",
            &[
                r#"0 0 "A:""#,
                r#"1 0 "- a""#,
                r#"1 0 """#,
                r#"1 0 "- b""#,
                r#"0 0 """#,
                r#"0 0 "B:""#,
            ],
        );
    }

    #[test]
    fn deeper_blank_line_keeps_its_own_depth() {
        check("A\n\t\t\nB", &[r#"0 0 "A""#, r#"1 1 """#, r#"0 0 "B""#]);
    }

    #[test]
    fn whitespace_only_lines_are_blank() {
        check(
            "A\n\t \u{A0}\n\tB",
            &[r#"0 0 "A""#, r#"1 0 """#, r#"1 0 "B""#],
        );
    }

    #[test]
    fn trailing_blank_lines_are_top_level() {
        check(
            "A\n\tB\n\t\n\t\t\n",
            &[r#"0 0 "A""#, r#"1 0 "B""#, r#"0 0 """#, r#"0 0 """#],
        );
    }

    #[test]
    fn only_blank_lines_are_all_top_level() {
        check("\t\n\t\t", &[r#"0 0 """#, r#"0 0 """#]);
    }

    #[test]
    fn empty_text_has_no_items() {
        check("", &[]);
        assert!(!parse("").format().trailing_newline);
    }

    #[test]
    fn lone_newline_is_one_blank_line() {
        check("\n", &[r#"0 0 """#]);
        assert!(parse("\n").format().trailing_newline);
    }

    #[test]
    fn remembers_the_trailing_newline() {
        assert!(parse("A\n").format().trailing_newline);
        assert!(!parse("A").format().trailing_newline);
        check("A\n\n", &[r#"0 0 "A""#, r#"0 0 """#]);
    }

    #[test]
    fn splits_on_every_paragraph_break() {
        check(
            "a\r\nb\rc\nd\u{C}e\u{2029}f\u{2B7F}g\u{2028}h",
            &[
                r#"0 0 "a""#,
                r#"0 0 "b""#,
                r#"0 0 "c""#,
                r#"0 0 "d""#,
                r#"0 0 "e""#,
                r#"0 0 "f""#,
                r#"0 0 "g\u{2028}h""#,
            ],
        );
    }

    #[test]
    fn detects_the_dominant_line_ending() {
        assert_eq!(
            parse("a\r\nb\r\nc\nd").format().line_ending,
            LineEnding::CrLf
        );
        assert_eq!(parse("a\rb\rc\nd").format().line_ending, LineEnding::Cr);
        assert_eq!(parse("a\nb\nc\r\nd").format().line_ending, LineEnding::Lf);
        assert_eq!(parse("a\nb\r\n").format().line_ending, LineEnding::Lf);
        assert_eq!(parse("a\rb\r\n").format().line_ending, LineEnding::CrLf);
        assert_eq!(parse("a\nb\r").format().line_ending, LineEnding::Lf);
        assert_eq!(parse("a").format().line_ending, LineEnding::Lf);
    }

    #[test]
    fn parse_bytes_strips_and_remembers_the_bom() {
        let document = parse_bytes("\u{FEFF}A:\n\t- b".as_bytes()).unwrap();
        assert!(document.format().bom);
        assert_eq!(outline(&document), [r#"0 0 "A:""#, r#"1 0 "- b""#]);
        assert!(!parse_bytes(b"A").unwrap().format().bom);
        assert!(!parse("A").format().bom);
    }

    #[test]
    fn parse_bytes_refuses_invalid_utf8_with_its_position() {
        assert_eq!(
            parse_bytes(b"ok\nalso ok\nbad \xff").unwrap_err(),
            DecodeError::NotUtf8 {
                line: 3,
                offset: 15
            }
        );
        assert_eq!(
            parse_bytes(b"\xEF\xBB\xBFx\xC3").unwrap_err(),
            DecodeError::NotUtf8 { line: 1, offset: 4 }
        );
    }

    #[test]
    fn decode_error_names_line_and_offset() {
        assert_eq!(
            DecodeError::NotUtf8 {
                line: 3,
                offset: 14
            }
            .to_string(),
            "the file is not valid UTF-8 (first invalid byte on line 3, at byte offset 14)"
        );
    }

    #[test]
    fn item_types_follow_taskpaper() {
        let cases = [
            ("- Milk", ItemKind::Task),
            ("-\tMilk", ItemKind::Task),
            ("- ", ItemKind::Task),
            ("-", ItemKind::Note),
            ("-Milk", ItemKind::Note),
            ("* Milk", ItemKind::Note),
            ("+ Milk", ItemKind::Note),
            ("- Buy: milk:", ItemKind::Task),
            ("Groceries:", ItemKind::Project),
            ("Groceries: @home @due(2016-01-01)", ItemKind::Project),
            ("Groceries:@home", ItemKind::Note),
            ("Groceries:   ", ItemKind::Note),
            ("Groceries: @home   ", ItemKind::Project),
            ("a @x(1) b:", ItemKind::Project),
            ("Proj: @a:", ItemKind::Project),
            ("- @done", ItemKind::Note),
            ("@done", ItemKind::Note),
            ("", ItemKind::Note),
        ];
        for (text, expected) in cases {
            assert_eq!(kind(text), expected, "{text:?}");
        }
    }

    #[test]
    fn display_text_drops_task_dash_and_project_colon() {
        let cases = [
            ("- Milk", "Milk"),
            ("-\tMilk @x", "Milk"),
            ("- ", ""),
            ("Groceries:", "Groceries"),
            ("Groceries: @home @a", "Groceries"),
            ("a @x(1) b:", "a b"),
            ("Café:", "Café"),
            ("- é", "é"),
            ("note: text", "note: text"),
        ];
        for (text, expected) in cases {
            assert_eq!(display_text(text), expected, "{text:?}");
        }
    }

    // FR-TAG-01
    #[test]
    fn display_text_hides_every_tag() {
        let cases = [
            ("- Buy milk @due(2026-10-05) @home", "Buy milk"),
            ("Inbox: @a(1)  ", "Inbox"),
            ("@today", ""),
            ("@a @b", ""),
            ("@a  rest", "rest"),
            ("- @a x", "x"),
            ("-\t@a x", "x"),
            ("a @x b", "a b"),
            ("a  @x   b", "a b"),
            ("a @x @y b @z c", "a b c"),
            ("a\u{3000}@x\tb", "a b"),
            ("x @a @a", "x"),
            ("@a x @a(2) y", "x y"),
            ("@a :", ""),
            ("keep  inner   spaces @x", "keep  inner   spaces"),
            ("trailing  ", "trailing  "),
            ("a @x b  ", "a b  "),
        ];
        for (text, expected) in cases {
            assert_eq!(display_text(text), expected, "{text:?}");
        }
    }

    #[test]
    fn display_text_keeps_what_is_not_a_tag() {
        let cases = [
            ("mail foo@bar.com", "mail foo@bar.com"),
            ("Proj:@x", "Proj:@x"),
            ("a @due(2026)x", "a @due(2026)x"),
            ("a @ b", "a @ b"),
            ("@id @text @type(x)", "@id @text @type(x)"),
            ("a @tag, b", "a @tag, b"),
        ];
        for (text, expected) in cases {
            assert_eq!(display_text(text), expected, "{text:?}");
        }
    }

    // FR-TAG-02
    #[test]
    fn tags_keep_the_first_occurrence_per_name() {
        let found: Vec<_> = tags("- a @done @due(1) b @done(2026-10-01) @due(2) @x")
            .into_iter()
            .map(|tag| (tag.name, tag.value, tag.span))
            .collect();
        assert_eq!(
            found,
            [
                ("done".to_owned(), None, 3..9),
                ("due".to_owned(), Some("1".to_owned()), 9..17),
                ("x".to_owned(), None, 45..48),
            ]
        );
    }

    #[test]
    fn tag_names_are_case_sensitive() {
        let names: Vec<_> = tags("@Done @done")
            .into_iter()
            .map(|tag| tag.name)
            .collect();
        assert_eq!(names, ["Done", "done"]);
    }

    #[test]
    fn line_without_tags_has_none() {
        assert_eq!(tags("plain text"), Vec::new());
        assert_eq!(tags(""), Vec::new());
    }
}
