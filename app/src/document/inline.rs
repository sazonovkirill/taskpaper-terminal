//! Inline Markdown in a line's text: `**bold**`, `` `code` `` and links (bare `http(s)://` URLs,
//! `<url>` and `[title](url)`) in every block, and headings in text blocks, derived for display
//! while the text stays as typed (FR-FMT-01, FR-FMT-03, D-45,
//! [ui.md §2](../../../docs/specs/ui.md#2-blocks)). Hides the inline syntax: which delimiters pair
//! up, where a URL ends, and how a marker is added or removed.

use std::ops::Range;

use super::BlockKind;

/// Which inline syntax a line's text has ([editing.md §2.2](../../../docs/specs/editing.md#22-markdown-in-text)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Syntax {
    /// Bold, code and links: bare URLs, `<url>` and `[title](url)` (FR-FMT-01, FR-FMT-03, D-45).
    Inline,
    /// Those and headings (FR-FMT-03).
    Markdown,
}

impl Syntax {
    /// The syntax of the text of a block of type `kind`: headings in text blocks only.
    #[must_use]
    pub fn of(kind: BlockKind) -> Self {
        match kind {
            BlockKind::Text => Self::Markdown,
            BlockKind::Project | BlockKind::Task | BlockKind::Section | BlockKind::Divider => {
                Self::Inline
            }
        }
    }
}

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
    /// Part of a URL or the title of a link.
    pub link: bool,
    /// The level, 1 to 6, of the heading the text belongs to (FR-FMT-03).
    pub heading: Option<u8>,
}

/// A piece of the text as shown, with the delimiters hidden, all of it formatted one way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Run<'a> {
    /// The text shown.
    pub text: &'a str,
    /// How it is formatted.
    pub format: Format,
}

/// What a formatted part of a text is.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    /// A marker's span.
    Marker(Marker),
    /// A bare URL or a `<url>` link, which shows its address.
    Url,
    /// A `[title](url)` link, which shows its title, to the address at these bytes.
    Link(Range<usize>),
}

/// One formatted part of a text: the bytes with its delimiters (outer) and the bytes shown
/// (inner); a bare URL's are the same.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Element {
    /// What it is.
    kind: Kind,
    /// The bytes with the delimiters.
    outer: Range<usize>,
    /// The bytes between the delimiters.
    inner: Range<usize>,
}

/// What a scan may find besides code spans: bold except inside bold, and links except inside a
/// link's title.
#[derive(Debug, Clone, Copy)]
struct Scope {
    bold: bool,
    links: bool,
}

/// `text` as shown: runs in order with the delimiters of bold, code, links and a heading left out.
/// A delimiter without a partner is shown as typed. Code shows its content as typed; bold may
/// hold code and links but not more bold, and a link's title may hold bold and code but no link.
#[must_use]
pub fn runs(text: &str, syntax: Syntax) -> Vec<Run<'_>> {
    let (heading, body) = heading(text, syntax).unzip();
    let body = body.unwrap_or_default();
    let elements = elements(text, syntax);
    let mut bounds: Vec<usize> = elements
        .iter()
        .flat_map(|e| [e.outer.start, e.inner.start, e.inner.end, e.outer.end])
        .chain([body, text.len()])
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
        let mut format = Format {
            heading,
            ..Format::default()
        };
        for element in elements.iter().filter(|e| inside(&e.inner)) {
            match element.kind {
                Kind::Marker(Marker::Bold) => format.bold = true,
                Kind::Marker(Marker::Code) => format.code = true,
                Kind::Url | Kind::Link(_) => format.link = true,
            }
        }
        runs.push(Run {
            text: text.get(start..end).unwrap_or_default(),
            format,
        });
    }
    runs
}

/// The addresses of the links in `text`, in order: bare URLs and the addresses of `[title](url)`
/// and `<url>` links (FR-FMT-03, D-45).
#[must_use]
pub fn urls(text: &str, syntax: Syntax) -> Vec<&str> {
    elements(text, syntax)
        .into_iter()
        .filter_map(|e| match e.kind {
            Kind::Marker(_) => None,
            Kind::Url => text.get(e.inner),
            Kind::Link(address) => text.get(address),
        })
        .collect()
}

/// Whether `address` starts with a URL scheme, such as `https:` or `mailto:`, so that an opener
/// never reads it as an option or a file name.
#[must_use]
pub fn has_scheme(address: &str) -> bool {
    address.split_once(':').is_some_and(|(scheme, _)| {
        let mut chars = scheme.chars();
        chars
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic())
            && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    })
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
            elements(text, Syntax::Inline).into_iter().find(|e| {
                e.kind == Kind::Marker(marker)
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

/// Every element of `text` in `syntax`, ordered by where it starts; an element inside bold or a
/// link's title follows it.
fn elements(text: &str, syntax: Syntax) -> Vec<Element> {
    let body = heading(text, syntax).map_or(0, |(_, body)| body);
    let scope = Scope {
        bold: true,
        links: true,
    };
    let mut found = Vec::new();
    scan(text, body..text.len(), scope, &mut found);
    found
}

/// Adds the elements of the bytes `range` of `text` that `scope` allows to `found`.
fn scan(text: &str, range: Range<usize>, scope: Scope, found: &mut Vec<Element>) {
    let mut at = range.start;
    while let Some(next) = text.get(at..range.end).and_then(|rest| rest.chars().next()) {
        let end = range.end;
        let element = code_at(text, at, end)
            .or_else(|| scope.bold.then(|| bold_at(text, at, end)).flatten())
            .or_else(|| scope.links.then(|| link_at(text, at, end)).flatten())
            .or_else(|| scope.links.then(|| autolink_at(text, at, end)).flatten())
            .or_else(|| scope.links.then(|| url_at(text, at, end)).flatten());
        let Some(element) = element else {
            at += next.len_utf8();
            continue;
        };
        at = element.outer.end;
        let inner = match element.kind {
            Kind::Marker(Marker::Bold) => Some(Scope {
                bold: false,
                ..scope
            }),
            Kind::Link(_) => Some(Scope {
                links: false,
                ..scope
            }),
            Kind::Marker(Marker::Code) | Kind::Url => None,
        }
        .map(|inner| (element.inner.clone(), inner));
        found.push(element);
        if let Some((range, inner)) = inner {
            scan(text, range, inner, found);
        }
    }
}

/// The level of the Markdown heading `text` is and the byte its title starts at: one to six `#`,
/// a space and text that is not all whitespace (FR-FMT-03).
fn heading(text: &str, syntax: Syntax) -> Option<(u8, usize)> {
    if syntax != Syntax::Markdown {
        return None;
    }
    let level = text.bytes().take_while(|&byte| byte == b'#').count();
    let title = text.get(level..)?.strip_prefix(' ')?.trim_start();
    let level = u8::try_from(level)
        .ok()
        .filter(|level| (1..=6).contains(level))?;
    (!title.is_empty()).then_some((level, text.len() - title.len()))
}

/// The Markdown link starting at byte `at` and ending by byte `end`: `[`, a title of at least one
/// character without brackets outside code spans, `](`, an address of at least one character
/// without whitespace whose parentheses pair up, and `)`.
fn link_at(text: &str, at: usize, end: usize) -> Option<Element> {
    text.get(at..end)?.strip_prefix('[')?;
    let title = at + 1;
    let mut close = title;
    loop {
        if let Some(code) = code_at(text, close, end) {
            close = code.outer.end;
            continue;
        }
        match text.get(close..end)?.chars().next()? {
            ']' => break,
            '[' => return None,
            other => close += other.len_utf8(),
        }
    }
    let address = close + 2;
    if close == title || !text.get(close + 1..end)?.starts_with('(') {
        return None;
    }
    let mut depth = 0_usize;
    let mut stop = address;
    for c in text.get(address..end)?.chars() {
        match c {
            ')' if depth == 0 => break,
            ')' => depth -= 1,
            '(' => depth += 1,
            c if c.is_whitespace() => return None,
            _ => {}
        }
        stop += c.len_utf8();
    }
    (stop > address && text.get(stop..end)?.starts_with(')')).then(|| Element {
        kind: Kind::Link(address..stop),
        outer: at..stop + 1,
        inner: title..close,
    })
}

/// The `<url>` link starting at byte `at` and ending by byte `end`: `<`, an address with a
/// scheme, something after its `:` and no whitespace, and `>`.
fn autolink_at(text: &str, at: usize, end: usize) -> Option<Element> {
    let rest = text.get(at..end)?.strip_prefix('<')?;
    let address = rest.get(..rest.find('>')?)?;
    let valid = has_scheme(address)
        && address
            .split_once(':')
            .is_some_and(|(_, after)| !after.is_empty())
        && !address.contains(|c: char| c.is_whitespace() || c == '<');
    valid.then(|| Element {
        kind: Kind::Url,
        outer: at..at + address.len() + 2,
        inner: at + 1..at + 1 + address.len(),
    })
}

/// The code span starting at byte `at` and ending by byte `end`: a backtick, at least one
/// character and the next backtick.
fn code_at(text: &str, at: usize, end: usize) -> Option<Element> {
    let rest = text.get(at..end)?.strip_prefix('`')?;
    let close = at + 1 + rest.find('`')?;
    (close > at + 1).then(|| Element {
        kind: Kind::Marker(Marker::Code),
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
                kind: Kind::Marker(Marker::Bold),
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
        kind: Kind::Url,
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
        heading: None,
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
        check_in(Syntax::Inline, text, expected);
    }

    #[track_caller]
    fn check_in(syntax: Syntax, text: &str, expected: &[(&str, Format)]) {
        let runs: Vec<(&str, Format)> = runs(text, syntax)
            .into_iter()
            .map(|run| (run.text, run.format))
            .collect();
        assert_eq!(runs, expected, "{syntax:?} {text:?}");
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
                        ..PLAIN
                    },
                ),
                (" c", BOLD),
            ],
        );
        check("**a `b** c", &[("a `b", BOLD), (" c", PLAIN)]);
    }

    // D-45
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

    // D-45
    #[test]
    fn lists_the_bare_urls_in_order() {
        assert_eq!(
            urls(
                "a https://x.y, `https://no.t` **http://b.c/d**",
                Syntax::Inline
            ),
            ["https://x.y", "http://b.c/d"]
        );
        assert_eq!(urls("none", Syntax::Inline), Vec::<&str>::new());
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

    // FR-FMT-03
    #[test]
    fn markdown_is_the_syntax_of_text_blocks_only() {
        assert_eq!(Syntax::of(BlockKind::Text), Syntax::Markdown);
        for kind in [
            BlockKind::Project,
            BlockKind::Task,
            BlockKind::Section,
            BlockKind::Divider,
        ] {
            assert_eq!(Syntax::of(kind), Syntax::Inline, "{kind:?}");
        }
    }

    const fn heading(level: u8) -> Format {
        Format {
            heading: Some(level),
            ..PLAIN
        }
    }

    // FR-FMT-03
    #[test]
    fn a_heading_hides_its_hashes_and_marks_every_run_with_its_level() {
        let markdown =
            |text, expected: &[(&str, Format)]| check_in(Syntax::Markdown, text, expected);
        markdown("# Title", &[("Title", heading(1))]);
        markdown("###### Six", &[("Six", heading(6))]);
        markdown("##   Spaced out ", &[("Spaced out ", heading(2))]);
        markdown(
            "### **Big** news",
            &[
                (
                    "Big",
                    Format {
                        bold: true,
                        ..heading(3)
                    },
                ),
                (" news", heading(3)),
            ],
        );
        markdown(
            "# [a](https://b.c)",
            &[(
                "a",
                Format {
                    link: true,
                    ..heading(1)
                },
            )],
        );
        for text in [
            "#tag",
            "####### Seven",
            "# ",
            "#   ",
            " # x",
            "#\tx",
            "a # b",
            "#",
        ] {
            markdown(text, &[(text, PLAIN)]);
        }
        check("# Title", &[("# Title", PLAIN)]);
    }

    // FR-FMT-03, D-45
    #[test]
    fn markdown_links_show_their_title_or_address_in_every_syntax() {
        check(
            "See [the site](https://a.b/c) or <https://d.e>",
            &[
                ("See ", PLAIN),
                ("the site", LINK),
                (" or ", PLAIN),
                ("https://d.e", LINK),
            ],
        );
        let markdown =
            |text, expected: &[(&str, Format)]| check_in(Syntax::Markdown, text, expected);
        markdown(
            "See [the site](https://a.b/c) now",
            &[("See ", PLAIN), ("the site", LINK), (" now", PLAIN)],
        );
        markdown(
            "see <https://a.b>.",
            &[("see ", PLAIN), ("https://a.b", LINK), (".", PLAIN)],
        );
        markdown("<mailto:x@y.z>", &[("mailto:x@y.z", LINK)]);
        markdown("<a<b:c>", &[("<a", PLAIN), ("b:c", LINK)]);
        markdown("[w](https://x.y/a_(b))", &[("w", LINK)]);
        markdown("[notes](notes.txt)", &[("notes", LINK)]);
        markdown("[https://a.b](https://a.b)", &[("https://a.b", LINK)]);
        markdown("[日本](https://a.b)語", &[("日本", LINK), ("語", PLAIN)]);
        markdown("[a[b](https://c.d)", &[("[a", PLAIN), ("b", LINK)]);
    }

    // FR-FMT-03
    #[test]
    fn a_link_title_holds_bold_and_code_and_bold_holds_links() {
        let markdown =
            |text, expected: &[(&str, Format)]| check_in(Syntax::Markdown, text, expected);
        let both = |format: Format| Format {
            link: true,
            ..format
        };
        markdown(
            "[**a** `]`](https://b.c)",
            &[("a", both(BOLD)), (" ", LINK), ("]", both(CODE))],
        );
        markdown("**[a](https://b.c)**", &[("a", both(BOLD))]);
        markdown("**<https://b.c>**", &[("https://b.c", both(BOLD))]);
    }

    // FR-FMT-03
    #[test]
    fn malformed_markdown_links_stay_as_typed() {
        for text in [
            "[](x.y)",
            "[a]()",
            "[a] (x.y)",
            "[a](b c)",
            "[a](x.y",
            "[a](x(y)",
            "[a",
            "<a.b>",
            "<https:>",
            "<x:a b>",
            "<>",
            "<-x:y>",
        ] {
            check_in(Syntax::Markdown, text, &[(text, PLAIN)]);
        }
        check(
            "[a](https://u.v w)",
            &[("[a](", PLAIN), ("https://u.v", LINK), (" w)", PLAIN)],
        );
    }

    // FR-FMT-03, D-45
    #[test]
    fn lists_markdown_link_addresses_with_the_bare_urls_in_order() {
        for syntax in [Syntax::Inline, Syntax::Markdown] {
            assert_eq!(
                urls(
                    "[a](rel) <mailto:x@y.z> https://b.c **[d](https://e.f)** `[g](h)`",
                    syntax
                ),
                ["rel", "mailto:x@y.z", "https://b.c", "https://e.f"]
            );
            assert_eq!(urls("[https://a.b](https://c.d)", syntax), ["https://c.d"]);
        }
    }

    #[test]
    fn a_scheme_is_a_letter_then_letters_digits_or_plus_minus_dot_before_a_colon() {
        for address in ["https://a", "mailto:x", "a+b-c.d:x", "x:"] {
            assert!(has_scheme(address), "{address:?}");
        }
        for address in ["-help", "/etc/x", "1x:y", "a_b:c", "", ":x", "www.x"] {
            assert!(!has_scheme(address), "{address:?}");
        }
    }

    proptest::proptest! {
        /// Any text shows as runs of itself in order, and a wrap comes off again.
        #[test]
        fn runs_are_pieces_of_any_text_and_a_wrap_toggles_back(
            text in "[a*` hps:/.()#\\[\\]<>日]{0,24}",
        ) {
            for syntax in [Syntax::Inline, Syntax::Markdown] {
                let mut rest = text.as_str();
                for run in runs(&text, syntax) {
                    let at = rest.find(run.text);
                    proptest::prop_assert!(at.is_some(), "{:?} not in {:?}", run.text, rest);
                    rest = &rest[at.unwrap() + run.text.len()..];
                }
                let _ = urls(&text, syntax);
            }
            let end = text.len();
            for marker in [Marker::Bold, Marker::Code] {
                let (wrapped, range) = toggle(&text, end..end, marker);
                proptest::prop_assert_eq!(toggle(&wrapped, range, marker).0, text.clone());
            }
        }
    }
}
