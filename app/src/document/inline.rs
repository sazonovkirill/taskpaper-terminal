//! Inline Markdown in a line's text: `**bold**`, `` `code` `` and bare `http(s)://` URLs, derived
//! for display while the text stays as typed (FR-FMT-01, D-10,
//! [ui.md §2](../../../docs/specs/ui.md#2-blocks)). Hides the inline syntax: which delimiters
//! pair up, where a URL ends, and how a marker is added or removed.

use std::ops::Range;

/// A pair of delimiters that formats the text between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marker {
    /// `**bold**`.
    Bold,
    /// `` `code` ``.
    Code,
}

impl Marker {
    /// What stands on either side of the formatted text.
    fn delimiter(self) -> &'static str {
        match self {
            Self::Bold => "**",
            Self::Code => "`",
        }
    }
}

/// How a run of text is formatted; several apply at once, as for a URL in bold text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Format {
    /// Inside `**…**`.
    pub bold: bool,
    /// Inside `` `…` ``.
    pub code: bool,
    /// Part of a URL.
    pub link: bool,
}

/// A piece of the text as shown, with the delimiters hidden, all of it formatted one way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Run<'a> {
    /// The text shown.
    pub text: &'a str,
    /// How it is formatted.
    pub format: Format,
}

/// One formatted part of a text: a marker's span with its delimiters, or a URL, whose inner and
/// outer ranges are the same.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Element {
    /// The marker, or `None` for a URL.
    marker: Option<Marker>,
    /// The bytes with the delimiters.
    outer: Range<usize>,
    /// The bytes between the delimiters.
    inner: Range<usize>,
}

/// `text` as shown: runs in order with the delimiters of bold and code left out. A delimiter without a partner is shown as typed. Code shows its
/// content as typed; bold may hold code and URLs but not more bold.
#[must_use]
pub fn runs(text: &str) -> Vec<Run<'_>> {
    let elements = elements(text);
    let mut bounds: Vec<usize> = elements
        .iter()
        .flat_map(|e| [e.outer.start, e.inner.start, e.inner.end, e.outer.end])
        .chain([0, text.len()])
        .collect();
    bounds.sort_unstable();
    bounds.dedup();
    let mut runs: Vec<Run> = Vec::new();
    for &[start, end] in bounds.array_windows() {
        let inside = |range: &Range<usize>| range.start <= start && end <= range.end;
        if elements
            .iter()
            .any(|e| inside(&e.outer) && !inside(&e.inner))
        {
            continue;
        }
        let mut format = Format::default();
        for element in elements.iter().filter(|e| inside(&e.inner)) {
            match element.marker {
                Some(Marker::Bold) => format.bold = true,
                Some(Marker::Code) => format.code = true,
                None => format.link = true,
            }
        }
        runs.push(Run {
            text: text.get(start..end).unwrap_or_default(),
            format,
        });
    }
    runs
}

/// The bare URLs in `text`, in order (D-10).
#[must_use]
pub fn urls(text: &str) -> Vec<&str> {
    elements(text)
        .into_iter()
        .filter(|e| e.marker.is_none())
        .filter_map(|e| text.get(e.inner))
        .collect()
}

/// `text` with `marker` taken off the bytes `range`, or put on them, and where those bytes are
/// in the result (FR-FMT-02). The marker comes off when `range` lies inside a span of it, is such
/// a span with its delimiters, or stands right between two of its delimiters; otherwise it goes
/// around `range`. `range` lies on character boundaries of `text`.
#[must_use]
pub fn toggle(text: &str, range: Range<usize>, marker: Marker) -> (String, Range<usize>) {
    let delimiter = marker.delimiter();
    let width = delimiter.len();
    let piece = |bytes: Range<usize>| text.get(bytes).unwrap_or_default();
    let wrapped = range.start >= width
        && piece(range.start - width..range.start) == delimiter
        && piece(range.end..range.end + width) == delimiter;
    let span = (!wrapped)
        .then(|| {
            elements(text).into_iter().find(|e| {
                e.marker == Some(marker)
                    && (e.outer == range
                        || (e.inner.start <= range.start && range.end <= e.inner.end))
            })
        })
        .flatten();
    let unwrap = |outer: Range<usize>, inner: Range<usize>| {
        [
            piece(0..outer.start),
            piece(inner),
            piece(outer.end..text.len()),
        ]
        .concat()
    };
    if wrapped {
        let outer = range.start - width..range.end + width;
        return (
            unwrap(outer, range.clone()),
            range.start - width..range.end - width,
        );
    }
    if let Some(Element { outer, inner, .. }) = span {
        let moved = if outer == range {
            outer.start..outer.start + inner.len()
        } else {
            range.start - width..range.end - width
        };
        return (unwrap(outer, inner), moved);
    }
    let wrapped = [
        piece(0..range.start),
        delimiter,
        piece(range.clone()),
        delimiter,
        piece(range.end..text.len()),
    ]
    .concat();
    (wrapped, range.start + width..range.end + width)
}

/// Every element of `text`, ordered by where it starts; an element inside bold follows it.
fn elements(text: &str) -> Vec<Element> {
    let mut found = Vec::new();
    scan(text, 0..text.len(), true, &mut found);
    found
}

/// Adds the elements of the bytes `range` of `text` to `found`, bold ones only with `bold`.
fn scan(text: &str, range: Range<usize>, bold: bool, found: &mut Vec<Element>) {
    let mut at = range.start;
    while let Some(next) = text.get(at..range.end).and_then(|rest| rest.chars().next()) {
        let element = code_at(text, at, range.end)
            .or_else(|| bold.then(|| bold_at(text, at, range.end)).flatten())
            .or_else(|| url_at(text, at, range.end));
        if let Some(element) = element {
            at = element.outer.end;
            let inner = element.inner.clone();
            let is_bold = element.marker == Some(Marker::Bold);
            found.push(element);
            if is_bold {
                scan(text, inner, false, found);
            }
        } else {
            at += next.len_utf8();
        }
    }
}

/// The code span starting at byte `at` and ending by byte `end`: a backtick, at least one
/// character and the next backtick.
fn code_at(text: &str, at: usize, end: usize) -> Option<Element> {
    let rest = text.get(at..end)?.strip_prefix('`')?;
    let close = at + 1 + rest.find('`')?;
    (close > at + 1).then(|| Element {
        marker: Some(Marker::Code),
        outer: at..close + 1,
        inner: at + 1..close,
    })
}

/// The bold span starting at byte `at` and ending by byte `end`: `**`, text that neither starts
/// nor ends with whitespace, and the next `**` outside a code span after it.
fn bold_at(text: &str, at: usize, end: usize) -> Option<Element> {
    let content = at + 2;
    let first = text.get(at..end)?.strip_prefix("**")?.chars().next()?;
    if first.is_whitespace() {
        return None;
    }
    let mut close = content + first.len_utf8();
    while let Some(rest) = text.get(close..end).filter(|rest| !rest.is_empty()) {
        if let Some(code) = code_at(text, close, end) {
            close = code.outer.end;
            continue;
        }
        let before = text.get(..close)?.chars().next_back()?;
        if rest.starts_with("**") && !before.is_whitespace() {
            return Some(Element {
                marker: Some(Marker::Bold),
                outer: at..close + 2,
                inner: content..close,
            });
        }
        close += rest.chars().next()?.len_utf8();
    }
    None
}

/// The bare URL starting at byte `at` and ending by byte `end`: `http://` or `https://` not
/// right after a letter or digit, then everything up to whitespace or `` ` ``, `<`, `>` or `"`,
/// without trailing punctuation or a `)` that closes nothing in it.
fn url_at(text: &str, at: usize, end: usize) -> Option<Element> {
    let rest = text.get(at..end)?;
    let scheme = ["https://", "http://"]
        .into_iter()
        .find(|scheme| rest.starts_with(scheme))?;
    if text
        .get(..at)?
        .chars()
        .next_back()
        .is_some_and(char::is_alphanumeric)
    {
        return None;
    }
    let mut url = rest
        .split(|c: char| c.is_whitespace() || matches!(c, '`' | '<' | '>' | '"'))
        .next()?;
    loop {
        let trimmed = url.trim_end_matches(['.', ',', ':', ';', '!', '?', '\'', '*']);
        let unmatched =
            trimmed.ends_with(')') && trimmed.matches(')').count() > trimmed.matches('(').count();
        url = if unmatched {
            trimmed.strip_suffix(')')?
        } else {
            trimmed
        };
        if !unmatched {
            break;
        }
    }
    (url.len() > scheme.len()).then(|| Element {
        marker: None,
        outer: at..at + url.len(),
        inner: at..at + url.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAIN: Format = Format {
        bold: false,
        code: false,
        link: false,
    };
    const BOLD: Format = Format {
        bold: true,
        ..PLAIN
    };
    const CODE: Format = Format {
        code: true,
        ..PLAIN
    };
    const LINK: Format = Format {
        link: true,
        ..PLAIN
    };

    #[track_caller]
    fn check(text: &str, expected: &[(&str, Format)]) {
        let runs: Vec<(&str, Format)> = runs(text)
            .into_iter()
            .map(|run| (run.text, run.format))
            .collect();
        assert_eq!(runs, expected, "{text:?}");
    }

    // FR-FMT-01
    #[test]
    fn bold_and_code_hide_their_delimiters() {
        check(
            "a **b** c `d` e",
            &[
                ("a ", PLAIN),
                ("b", BOLD),
                (" c ", PLAIN),
                ("d", CODE),
                (" e", PLAIN),
            ],
        );
        check("**b**", &[("b", BOLD)]);
        check("`x`", &[("x", CODE)]);
        check("`a``b`c", &[("a", CODE), ("b", CODE), ("c", PLAIN)]);
        check("**日本**語", &[("日本", BOLD), ("語", PLAIN)]);
    }

    #[test]
    fn plain_and_empty_text_are_one_run_or_none() {
        check("plain", &[("plain", PLAIN)]);
        check("", &[]);
    }

    // FR-FMT-01
    #[test]
    fn unpaired_or_empty_delimiters_stay_as_typed() {
        for text in [
            "a ** b",
            "**a",
            "a`b",
            "``",
            "****",
            "** a**",
            "2 ** 3 ** 4",
            "*a*",
        ] {
            check(text, &[(text, PLAIN)]);
        }
        check("**a **b**", &[("a **b", BOLD)]);
    }

    // FR-FMT-01
    #[test]
    fn code_shows_its_content_as_typed_and_binds_before_bold() {
        check("`**a**`", &[("**a**", CODE)]);
        check("`https://a.b`", &[("https://a.b", CODE)]);
        check(
            "**a `b**` c**",
            &[
                ("a ", BOLD),
                (
                    "b**",
                    Format {
                        bold: true,
                        code: true,
                        link: false,
                    },
                ),
                (" c", BOLD),
            ],
        );
        check("**a `b** c", &[("a `b", BOLD), (" c", PLAIN)]);
    }

    // D-10
    #[test]
    fn bare_urls_are_links_without_trailing_punctuation() {
        check(
            "See https://a.b/c?d=1.",
            &[("See ", PLAIN), ("https://a.b/c?d=1", LINK), (".", PLAIN)],
        );
        check(
            "(http://a.b/x_(y)) z",
            &[("(", PLAIN), ("http://a.b/x_(y)", LINK), (") z", PLAIN)],
        );
        check(
            "**https://a.b**",
            &[(
                "https://a.b",
                Format {
                    bold: true,
                    link: true,
                    ..PLAIN
                },
            )],
        );
        check(
            "<https://a.b>",
            &[("<", PLAIN), ("https://a.b", LINK), (">", PLAIN)],
        );
        for text in [
            "https://",
            "https://.",
            "xhttps://a.b",
            "ftp://a.b",
            "http:/a",
        ] {
            check(text, &[(text, PLAIN)]);
        }
    }

    // D-10
    #[test]
    fn lists_the_bare_urls_in_order() {
        assert_eq!(
            urls("a https://x.y, `https://no.t` **http://b.c/d**"),
            ["https://x.y", "http://b.c/d"]
        );
        assert_eq!(urls("none"), Vec::<&str>::new());
    }

    /// The text and range after toggling `marker` on the bytes `[`…`]` mark in `before`, written
    /// the same way.
    #[track_caller]
    fn check_toggle(before: &str, marker: Marker, expected: &str) {
        let start = before.find('[').unwrap();
        let end = before.find(']').unwrap() - 1;
        let text = before.replace(['[', ']'], "");
        let (after, range) = toggle(&text, start..end, marker);
        let mut shown = after.clone();
        shown.insert(range.end, ']');
        shown.insert(range.start, '[');
        assert_eq!(shown, expected, "{marker:?} on {before:?}");
    }

    // FR-FMT-02
    #[test]
    fn toggling_wraps_a_range_in_the_marker() {
        check_toggle("a [bc] d", Marker::Bold, "a **[bc]** d");
        check_toggle("a [bc] d", Marker::Code, "a `[bc]` d");
        check_toggle("a []", Marker::Bold, "a **[]**");
        check_toggle("[日本]", Marker::Code, "`[日本]`");
    }

    // FR-FMT-02
    #[test]
    fn toggling_removes_the_marker_around_or_over_a_range() {
        check_toggle("a **[bc]** d", Marker::Bold, "a [bc] d");
        check_toggle("a [**bc**] d", Marker::Bold, "a [bc] d");
        check_toggle("**a [b] c**", Marker::Bold, "a [b] c");
        check_toggle("x **a[] c**", Marker::Bold, "x a[] c");
        check_toggle("`[ab]`", Marker::Code, "[ab]");
        check_toggle("**[]**", Marker::Bold, "[]");
    }

    // FR-FMT-02
    #[test]
    fn toggling_one_marker_leaves_the_other_alone() {
        check_toggle("**a [b] c**", Marker::Code, "**a `[b]` c**");
        check_toggle("`a [b] c`", Marker::Bold, "`a **[b]** c`");
    }

    proptest::proptest! {
        /// Any text shows as runs of itself in order, and a wrap comes off again.
        #[test]
        fn runs_are_pieces_of_any_text_and_a_wrap_toggles_back(
            text in "[a*` hps:/.()日]{0,24}",
        ) {
            let mut rest = text.as_str();
            for run in runs(&text) {
                let at = rest.find(run.text);
                proptest::prop_assert!(at.is_some(), "{:?} not in {:?}", run.text, rest);
                rest = &rest[at.unwrap() + run.text.len()..];
            }
            let _ = urls(&text);
            let end = text.len();
            for marker in [Marker::Bold, Marker::Code] {
                let (wrapped, range) = toggle(&text, end..end, marker);
                proptest::prop_assert_eq!(toggle(&wrapped, range, marker).0, text.clone());
            }
        }
    }
}
