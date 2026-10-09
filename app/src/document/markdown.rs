//! A note's lines read as Markdown blocks: headings, list items, block quotes, fenced code and
//! text, each line on its own, for a frontend to show with its inline formatting
//! ([attachments.md §4](../../../docs/specs/attachments.md#4-notes-fr-att-04), FR-ATT-04).

use super::inline::{self, Syntax};

/// One line of a note as Markdown shows it, its syntax taken off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteLine<'a> {
    /// A heading of this level, 1 to 6, and its title.
    Heading(u8, &'a str),
    /// A list item `depth` levels in, every two leading spaces one level, with its marker as
    /// typed (`-`, `*`, `+`, `1.` or `1)`) and its text.
    Item {
        /// How many levels the item is nested.
        depth: usize,
        /// Its marker.
        marker: &'a str,
        /// Its text.
        text: &'a str,
    },
    /// A line of a block quote.
    Quote(&'a str),
    /// A line inside a fenced code block, as typed.
    Code(&'a str),
    /// A line of text without its leading whitespace; an empty one separates paragraphs.
    Text(&'a str),
}

/// The lines of the note `lines` as Markdown, the lines of code fences left out: a fence opens
/// with three backticks or tildes and closes with the same three, or at the end of the note.
#[must_use]
pub fn note_lines(lines: &[String]) -> Vec<NoteLine<'_>> {
    let mut fence: Option<&str> = None;
    let mut shown = Vec::new();
    for line in lines {
        let fenced = ["```", "~~~"]
            .into_iter()
            .find(|mark| line.trim_start().starts_with(mark));
        match (fence, fenced) {
            (None, Some(mark)) => fence = Some(mark),
            (Some(open), Some(mark)) if open == mark => fence = None,
            (Some(_), _) => shown.push(NoteLine::Code(line)),
            (None, None) => shown.push(block_line(line)),
        }
    }
    shown
}

/// The line `line` outside code.
fn block_line(line: &str) -> NoteLine<'_> {
    let text = line.trim_start();
    let indent = line.len() - text.len();
    heading(text)
        .or_else(|| {
            let quote = text.strip_prefix('>')?;
            Some(NoteLine::Quote(quote.strip_prefix(' ').unwrap_or(quote)))
        })
        .or_else(|| {
            let (marker, text) = text.split_once(' ')?;
            let number = marker.strip_suffix(['.', ')']).is_some_and(|digits| {
                !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
            });
            (matches!(marker, "-" | "*" | "+") || number).then_some(NoteLine::Item {
                depth: indent / 2,
                marker,
                text: text.trim_start(),
            })
        })
        .unwrap_or(NoteLine::Text(text))
}

/// The heading `text` is, its title without trailing whitespace (FR-FMT-03).
fn heading(text: &str) -> Option<NoteLine<'_>> {
    let (level, title) = inline::heading(text, Syntax::Markdown)?;
    Some(NoteLine::Heading(level, text.get(title..)?.trim_end()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Vec<NoteLine<'static>> {
        let lines: Vec<String> = text.lines().map(str::to_owned).collect();
        note_lines(Vec::leak(lines))
    }

    // FR-ATT-04, attachments.md §4
    #[test]
    fn headings_items_quotes_and_text_lose_their_syntax() {
        assert_eq!(
            read(
                "# Title\n###### Six\n####### Seven\n#No\n# \n  text  \n\n> said\n>bare\n- a\n  * b\n    + c\n1. one\n12) twelve\n-no\n1.no\nx. no"
            ),
            [
                NoteLine::Heading(1, "Title"),
                NoteLine::Heading(6, "Six"),
                NoteLine::Text("####### Seven"),
                NoteLine::Text("#No"),
                NoteLine::Text("# "),
                NoteLine::Text("text  "),
                NoteLine::Text(""),
                NoteLine::Quote("said"),
                NoteLine::Quote("bare"),
                NoteLine::Item {
                    depth: 0,
                    marker: "-",
                    text: "a"
                },
                NoteLine::Item {
                    depth: 1,
                    marker: "*",
                    text: "b"
                },
                NoteLine::Item {
                    depth: 2,
                    marker: "+",
                    text: "c"
                },
                NoteLine::Item {
                    depth: 0,
                    marker: "1.",
                    text: "one"
                },
                NoteLine::Item {
                    depth: 0,
                    marker: "12)",
                    text: "twelve"
                },
                NoteLine::Text("-no"),
                NoteLine::Text("1.no"),
                NoteLine::Text("x. no"),
            ]
        );
    }

    // FR-ATT-04, attachments.md §4
    #[test]
    fn fenced_code_shows_as_typed_without_its_fences() {
        assert_eq!(
            read("a\n```rust\n  # not a heading\n~~~\n```\n- b\n  ~~~\n```\nopen to the end"),
            [
                NoteLine::Text("a"),
                NoteLine::Code("  # not a heading"),
                NoteLine::Code("~~~"),
                NoteLine::Item {
                    depth: 0,
                    marker: "-",
                    text: "b"
                },
                NoteLine::Code("```"),
                NoteLine::Code("open to the end"),
            ]
        );
    }
}
