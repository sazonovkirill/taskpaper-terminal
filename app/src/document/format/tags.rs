//! Tag scanning and tag edits with TaskPaper's grammar
//! ([file-format.md §5](../../../../docs/specs/file-format.md#5-tags)).

use std::ops::Range;

/// One tag occurrence in a line body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    /// From the whitespace before `@` (or the line start) to the end of the tag.
    pub span: Range<usize>,
    /// The tag name, without `@`.
    pub name: String,
    /// The unescaped value, or `None` when the tag has no parentheses.
    pub value: Option<String>,
}

/// Scans `text` for tags, left to right, like TaskPaper's global tag regex.
///
/// Empty names and the reserved names `id`, `text` and `type` are plain text, as file-format.md §5 says.
/// Duplicates are returned too; deciding that the first occurrence wins is up to the caller.
#[must_use]
pub fn scan(text: &str) -> Vec<Tag> {
    let mut tags = Vec::new();
    let mut resume = 0;
    for (at, _) in text.match_indices('@') {
        let start = text
            .get(..at)
            .map_or(0, |before| before.trim_end_matches(is_whitespace).len());
        if at < resume || (start == at && at > 0) {
            continue;
        }
        let Some((end, name, value)) = tag_at(text, at) else {
            continue;
        };
        resume = end;
        if !name.is_empty() && !matches!(name.as_str(), "id" | "text" | "type") {
            tags.push(Tag {
                span: start..end,
                name,
                value,
            });
        }
    }
    tags
}

/// Where the trailing tag run of `text`, whose tags [`scan`] found as `scanned`, starts: a
/// sequence of tags followed only by whitespace up to the end. Returns `text.len()` when the line
/// does not end in a tag.
#[must_use]
pub fn trailing_run_start(text: &str, scanned: &[Tag]) -> usize {
    let mut run_start = text.trim_end_matches(is_whitespace).len();
    for tag in scanned.iter().rev() {
        if tag.span.end != run_start {
            break;
        }
        run_start = tag.span.start;
    }
    if run_start == text.trim_end_matches(is_whitespace).len() {
        text.len()
    } else {
        run_start
    }
}

/// `text` with the tag `name` set to `value`: the first occurrence's value replaced in place,
/// else ` @name(value)` appended, without the space when `text` is empty or already ends in
/// whitespace, so no line starts with whitespace. An empty value writes a bare `@name`.
#[must_use]
pub fn set(text: &str, name: &str, value: &str) -> String {
    let value = value.replace('(', r"\(").replace(')', r"\)");
    let tag = if value.is_empty() {
        format!("@{name}")
    } else {
        format!("@{name}({value})")
    };
    let mut out = text.to_owned();
    if let Some(found) = scan(text).into_iter().find(|found| found.name == name) {
        // The span starts with the whitespace before `@`, which stays.
        let occurrence = text.get(found.span.clone()).unwrap_or_default();
        let at = found.span.end - occurrence.trim_start_matches(is_whitespace).len();
        out.replace_range(at..found.span.end, &tag);
    } else {
        if !out.is_empty() && !out.ends_with(is_whitespace) {
            out.push(' ');
        }
        out.push_str(&tag);
    }
    out
}

/// `text` without any tag `name`: each occurrence is deleted with its leading whitespace, or with
/// the whitespace after it at the start of the line, so no line starts with whitespace. Every
/// occurrence goes, because a later duplicate would take the first one's place.
#[must_use]
pub fn remove(text: &str, name: &str) -> String {
    let mut out = text.to_owned();
    while let Some(found) = scan(&out).into_iter().find(|found| found.name == name) {
        let end = if found.span.start == 0 {
            let after = out.get(found.span.end..).unwrap_or_default();
            out.len() - after.trim_start_matches(is_whitespace).len()
        } else {
            found.span.end
        };
        out.replace_range(found.span.start..end, "");
    }
    out
}

/// Whether changing the text `before` into `after` puts an `@` at the start of a word or makes a
/// tag out of text. TaskPaper has no escape for `@`, so such text would be read back as a hidden
/// tag; edit mode refuses it (OQ-25). An `@` inside a word, as in an email address, is text.
#[must_use]
pub fn adds_tag(before: &str, after: &str) -> bool {
    word_start_ats(after) > word_start_ats(before) || scan(after).len() > scan(before).len()
}

/// How many `@` start a word: at the start of `text` or after whitespace, where a tag can start.
fn word_start_ats(text: &str) -> usize {
    text.match_indices('@')
        .filter(|&(at, _)| {
            text.get(..at)
                .and_then(|before| before.chars().next_back())
                .is_none_or(is_whitespace)
        })
        .count()
}

/// Parses the tag whose `@` is at byte `at`; returns its end, name and value.
fn tag_at(text: &str, at: usize) -> Option<(usize, String, Option<String>)> {
    let rest = text.get(at + 1..)?;
    let name_len = rest
        .char_indices()
        .find(|&(_, c)| !is_name_char(c))
        .map_or(rest.len(), |(index, _)| index);
    let name = rest.get(..name_len)?.to_owned();
    let name_end = at + 1 + name_len;
    if let Some((value_end, value)) = value_at(text, name_end) {
        return Some((value_end, name, Some(value)));
    }
    ends_tag(text, name_end).then_some((name_end, name, None))
}

/// Parses a `(value)` starting at byte `open` like TaskPaper's backtracking regex: the longest
/// value whose parentheses are all escaped and whose closing `)` ends the tag.
fn value_at(text: &str, open: usize) -> Option<(usize, String)> {
    let inner = text.get(open..)?.strip_prefix('(')?;
    let mut close = None;
    let mut previous = None;
    for (index, c) in inner.char_indices() {
        if c == ')' && ends_tag(inner, index + 1) {
            close = Some(index);
        }
        if matches!(c, '(' | ')') && previous != Some('\\') {
            break;
        }
        previous = Some(c);
    }
    let close = close?;
    Some((open + 1 + close + 1, unescape(inner.get(..close)?)))
}

/// Replaces `\(` and `\)` with the parenthesis; other backslashes stay literal.
fn unescape(raw: &str) -> String {
    raw.replace(r"\(", "(").replace(r"\)", ")")
}

/// A tag must be followed by whitespace or the end of the line.
fn ends_tag(text: &str, end: usize) -> bool {
    text.get(end..)
        .and_then(|rest| rest.chars().next())
        .is_none_or(is_whitespace)
}

/// The XML name characters without `:`, as TaskPaper's `TagWord` (BMP only, like its regex).
fn is_name_char(c: char) -> bool {
    matches!(c,
        'A'..='Z' | '_' | 'a'..='z' | '-' | '.' | '0'..='9' | '\u{B7}'
        | '\u{C0}'..='\u{D6}' | '\u{D8}'..='\u{F6}' | '\u{F8}'..='\u{2FF}'
        | '\u{300}'..='\u{36F}' | '\u{370}'..='\u{37D}' | '\u{37F}'..='\u{1FFF}'
        | '\u{200C}'..='\u{200D}' | '\u{203F}'..='\u{2040}' | '\u{2070}'..='\u{218F}'
        | '\u{2C00}'..='\u{2FEF}' | '\u{3001}'..='\u{D7FF}' | '\u{F900}'..='\u{FDCF}'
        | '\u{FDF0}'..='\u{FFFD}')
}

/// JavaScript's `\s`, which TaskPaper's regexes use.
pub(crate) fn is_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{B}' | '\u{C}' | '\r' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(text: &str) -> Vec<(String, Option<String>)> {
        scan(text).into_iter().map(|t| (t.name, t.value)).collect()
    }

    fn tag(name: &str, value: Option<&str>) -> (String, Option<String>) {
        (name.to_owned(), value.map(str::to_owned))
    }

    #[test]
    fn finds_tags_with_and_without_values() {
        assert_eq!(
            names("- Buy milk @due(2026-10-05) @home"),
            [tag("due", Some("2026-10-05")), tag("home", None)]
        );
    }

    #[test]
    fn span_includes_the_whitespace_before_the_tag() {
        let tags = scan("a  @x(1) b");
        assert_eq!(tags[0].span, 1..8);
    }

    #[test]
    fn tag_at_line_start_spans_from_zero() {
        assert_eq!(scan("@today")[0].span, 0..6);
    }

    #[test]
    fn needs_whitespace_or_line_start_before_the_at_sign() {
        assert_eq!(scan("foo@bar.com"), Vec::new());
        assert_eq!(scan("Proj:@x"), Vec::new());
    }

    #[test]
    fn needs_whitespace_or_line_end_after_the_tag() {
        assert_eq!(scan("@due(2026)x"), Vec::new());
        assert_eq!(scan("a @tag, b"), Vec::new());
        assert_eq!(names("a @x\tb"), [tag("x", None)]);
    }

    #[test]
    fn empty_and_reserved_names_are_not_tags() {
        assert_eq!(scan("a @ b @"), Vec::new());
        assert_eq!(scan("@id @text @type(x)"), Vec::new());
    }

    #[test]
    fn names_allow_digits_dashes_dots_and_unicode() {
        assert_eq!(
            names("@2 @a.b-c @été @x·y @a\u{301}"),
            [
                tag("2", None),
                tag("a.b-c", None),
                tag("été", None),
                tag("x·y", None),
                tag("a\u{301}", None)
            ]
        );
    }

    #[test]
    fn name_stops_at_a_character_outside_the_name_classes() {
        assert_eq!(scan("@a:b"), Vec::new());
        assert_eq!(scan("@a/b"), Vec::new());
        assert_eq!(scan("@😀"), Vec::new());
    }

    #[test]
    fn value_unescapes_only_parentheses() {
        assert_eq!(names(r"@a(\(hi\) c:\d)"), [tag("a", Some(r"(hi) c:\d"))]);
    }

    #[test]
    fn backslash_before_the_closing_parenthesis_is_literal() {
        assert_eq!(names(r"@a(x\)"), [tag("a", Some(r"x\"))]);
        assert_eq!(names(r"@a(x\ )"), [tag("a", Some(r"x\ "))]);
    }

    #[test]
    fn value_takes_the_longest_match_that_ends_the_tag() {
        assert_eq!(names(r"@a(x\) y)"), [tag("a", Some("x) y"))]);
        assert_eq!(names(r"@a(x) y)"), [tag("a", Some("x"))]);
        assert_eq!(names(r"@a(x)y)"), Vec::new());
    }

    #[test]
    fn empty_value_is_an_empty_string() {
        assert_eq!(names("@a()"), [tag("a", Some(""))]);
    }

    #[test]
    fn values_may_contain_spaces_and_commas() {
        assert_eq!(
            names("@b(c d) @l(1, 2)"),
            [tag("b", Some("c d")), tag("l", Some("1, 2"))]
        );
    }

    #[test]
    fn unbalanced_parentheses_break_the_tag() {
        assert_eq!(names("@a(b @c(d)"), [tag("c", Some("d"))]);
        assert_eq!(scan("@a(b(c)d)"), Vec::new());
        assert_eq!(scan("@a(open"), Vec::new());
    }

    #[test]
    fn duplicates_are_all_returned() {
        assert_eq!(names("@a @a(2)"), [tag("a", None), tag("a", Some("2"))]);
    }

    #[test]
    fn javascript_whitespace_counts_as_separator() {
        assert_eq!(names("a\u{A0}@x"), [tag("x", None)]);
        assert_eq!(names("a\u{3000}@x\u{3000}b"), [tag("x", None)]);
    }

    #[test]
    fn empty_text_has_no_tags() {
        assert_eq!(scan(""), Vec::new());
    }

    #[test]
    fn trailing_run_starts_at_the_first_trailing_tag() {
        assert_eq!(
            trailing_run_start(
                "Groceries: @home @due(1)  ",
                &scan("Groceries: @home @due(1)  ")
            ),
            10
        );
    }

    #[test]
    fn mid_line_tag_is_not_part_of_the_trailing_run() {
        assert_eq!(trailing_run_start("a @x(1) b:", &scan("a @x(1) b:")), 10);
        assert_eq!(trailing_run_start("a @x b @y", &scan("a @x b @y")), 6);
    }

    #[test]
    fn line_without_trailing_tags_has_no_run() {
        assert_eq!(trailing_run_start("Inbox:  ", &scan("Inbox:  ")), 8);
        assert_eq!(trailing_run_start("", &scan("")), 0);
    }

    #[test]
    fn whole_line_of_tags_is_a_run_from_zero() {
        assert_eq!(trailing_run_start("@a @b", &scan("@a @b")), 0);
    }

    #[test]
    fn duplicate_tags_still_count_in_the_trailing_run() {
        assert_eq!(trailing_run_start("x @a @a", &scan("x @a @a")), 1);
    }

    #[track_caller]
    fn check_set(text: &str, name: &str, value: &str, expected: &str) {
        assert_eq!(
            set(text, name, value),
            expected,
            "set @{name}({value}) on {text:?}"
        );
    }

    #[test]
    fn set_appends_a_missing_tag() {
        check_set("- Milk", "done", "2026-10-05", "- Milk @done(2026-10-05)");
        check_set("- Milk @due(1)", "done", "x", "- Milk @due(1) @done(x)");
        check_set("Inbox:", "done", "x", "Inbox: @done(x)");
        check_set("- 日本語", "a", "語", "- 日本語 @a(語)");
    }

    #[test]
    fn set_adds_no_space_after_whitespace_or_on_an_empty_line() {
        check_set("- Milk ", "done", "x", "- Milk @done(x)");
        check_set("a\t", "done", "x", "a\t@done(x)");
        check_set("", "done", "x", "@done(x)");
    }

    #[test]
    fn set_replaces_the_first_occurrence_in_place() {
        check_set("- a @due(1) b", "due", "2", "- a @due(2) b");
        check_set("- a  @due b", "due", "2", "- a  @due(2) b");
        check_set("@due(1) @due(3)", "due", "2", "@due(2) @due(3)");
        check_set("- a @x @due(1)", "due", "", "- a @x @due");
    }

    #[test]
    fn set_writes_an_empty_value_bare_and_escapes_parentheses() {
        check_set("a", "today", "", "a @today");
        check_set("a", "x", "f(1)", r"a @x(f\(1\))");
        assert_eq!(
            scan(&set("a", "x", "f(1)"))[0].value.as_deref(),
            Some("f(1)")
        );
    }

    #[test]
    fn set_ignores_text_that_only_looks_like_the_tag() {
        check_set("me@done.com", "done", "x", "me@done.com @done(x)");
        check_set("@done(1)x", "done", "x", "@done(1)x @done(x)");
    }

    #[track_caller]
    fn check_remove(text: &str, name: &str, expected: &str) {
        assert_eq!(remove(text, name), expected, "remove @{name} from {text:?}");
    }

    #[test]
    fn remove_deletes_the_tag_with_its_leading_whitespace() {
        check_remove("- Milk @done(2026-10-05)", "done", "- Milk");
        check_remove("- a  @done b", "done", "- a b");
        check_remove("- a @x @done @y", "done", "- a @x @y");
        check_remove("Inbox: @done", "done", "Inbox:");
    }

    #[test]
    fn remove_deletes_every_occurrence() {
        check_remove("- a @done @done(1) @x @done", "done", "- a @x");
    }

    #[test]
    fn remove_at_line_start_deletes_the_whitespace_after_it() {
        check_remove("@done  note", "done", "note");
        check_remove("@done", "done", "");
        check_remove("@done @x", "done", "@x");
    }

    #[test]
    fn remove_without_the_tag_keeps_the_text() {
        for text in [
            "",
            "- a",
            "- a @donex",
            "me@done.com",
            "@done(1)x",
            "- a @x",
        ] {
            check_remove(text, "done", text);
        }
    }

    #[test]
    fn whitespace_matches_javascript() {
        for c in [
            '\t', '\n', '\u{B}', '\u{C}', '\r', ' ', '\u{A0}', '\u{1680}', '\u{2000}', '\u{200A}',
            '\u{2028}', '\u{2029}', '\u{202F}', '\u{205F}', '\u{3000}', '\u{FEFF}',
        ] {
            assert!(is_whitespace(c), "{c:?}");
        }
        for c in ['a', '\u{85}', '\u{200B}', '\u{1F}'] {
            assert!(!is_whitespace(c), "{c:?}");
        }
    }

    #[test]
    fn name_characters_match_taskpaper() {
        for c in [
            'A', 'Z', 'a', 'z', '_', '-', '.', '0', '9', '\u{B7}', '\u{C0}', '\u{D6}', '\u{D8}',
            '\u{F6}', '\u{F8}', '\u{2FF}', '\u{300}', '\u{36F}', '\u{370}', '\u{37D}', '\u{37F}',
            '\u{1FFF}', '\u{200C}', '\u{200D}', '\u{203F}', '\u{2040}', '\u{2070}', '\u{218F}',
            '\u{2C00}', '\u{2FEF}', '\u{3001}', '\u{D7FF}', '\u{F900}', '\u{FDCF}', '\u{FDF0}',
            '\u{FFFD}',
        ] {
            assert!(is_name_char(c), "{c:?}");
        }
        for c in [
            ':',
            '/',
            '@',
            '#',
            '!',
            ',',
            '(',
            ' ',
            '\u{D7}',
            '\u{F7}',
            '\u{37E}',
            '\u{2000}',
            '\u{2041}',
            '\u{2190}',
            '\u{3000}',
            '\u{FDD0}',
            '\u{FFFE}',
            '\u{10000}',
        ] {
            assert!(!is_name_char(c), "{c:?}");
        }
    }

    // OQ-25
    #[test]
    fn an_at_sign_starting_a_word_adds_a_tag() {
        assert!(adds_tag("", "@"));
        assert!(adds_tag("a ", "a @"));
        assert!(adds_tag("a\tb", "a\t@b"));
        assert!(adds_tag("ab", "a @b"), "a space before an @ inside a word");
        assert!(adds_tag("a@b", "@b"), "deleting what stood before it");
    }

    // OQ-25
    #[test]
    fn text_turning_into_a_tag_adds_one() {
        assert!(adds_tag("a @ b", "a @x b"), "a lone @ gets a name");
        assert!(
            adds_tag("a @id", "a @idx"),
            "a reserved name becomes another"
        );
    }

    // OQ-25
    #[test]
    fn an_at_sign_inside_a_word_or_an_edit_elsewhere_adds_none() {
        assert!(!adds_tag("me", "me@"));
        assert!(!adds_tag("me@example.com", "me@example.co"));
        assert!(!adds_tag("a @ b", "a @ bc"));
        assert!(!adds_tag("a @id", "b @id"));
        assert!(!adds_tag("@ a", "a"), "removing one");
        assert!(!adds_tag("", ""));
    }
}
