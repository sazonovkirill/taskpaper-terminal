//! A block's comments as lines: each starts with its local 24-hour timestamp,
//! `[YYYY-MM-DD HH:MM] text`, and a line without a valid one is a comment without a date, kept
//! as typed ([attachments.md §5](../../../docs/specs/attachments.md#5-comments-fr-att-05),
//! FR-ATT-05).

use jiff::civil::DateTime;

/// How a timestamp is written between its brackets.
const STAMP: &str = "%Y-%m-%d %H:%M";

/// A comment line read: its timestamp, if it starts with a valid one, and its text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Comment<'a> {
    /// The timestamp as written, without its brackets.
    pub stamp: Option<&'a str>,
    /// The text after the timestamp and its space, or the whole line without one.
    pub text: &'a str,
}

/// The comment the line `line` is.
#[must_use]
pub fn comment(line: &str) -> Comment<'_> {
    let stamped = line.strip_prefix('[').and_then(|rest| {
        let (stamp, text) = rest.split_once(']')?;
        DateTime::strptime(STAMP, stamp)
            .ok()
            .filter(|time| time.strftime(STAMP).to_string() == stamp)?;
        Some(Comment {
            stamp: Some(stamp),
            text: text.strip_prefix(' ').unwrap_or(text),
        })
    });
    stamped.unwrap_or(Comment {
        stamp: None,
        text: line,
    })
}

/// The line of a comment saying `text`, stamped `at`.
#[must_use]
pub fn stamped(at: DateTime, text: &str) -> String {
    format!("[{}] {text}", at.strftime(STAMP))
}

/// The comment line `line` saying `text` instead, its timestamp kept.
#[must_use]
pub fn reworded(line: &str, text: &str) -> String {
    match comment(line).stamp {
        Some(stamp) => format!("[{stamp}] {text}"),
        None => text.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;

    // FR-ATT-05, attachments.md §5
    #[test]
    fn a_comment_starts_with_a_valid_timestamp_or_has_none() {
        let read = |line| {
            let Comment { stamp, text } = comment(line);
            (stamp, text)
        };
        assert_eq!(
            read("[2026-10-08 09:05] Bought it"),
            (Some("2026-10-08 09:05"), "Bought it")
        );
        assert_eq!(read("[2026-10-08 23:59]"), (Some("2026-10-08 23:59"), ""));
        assert_eq!(read("[2026-10-08 23:59]x"), (Some("2026-10-08 23:59"), "x"));
        for line in [
            "Typed by hand",
            "[2026-10-08] no time",
            "[2026-13-08 09:05] no such month",
            "[2026-10-08 24:00] no such hour",
            "[2026-10-8 9:05] short",
            "[2026-10-08 09:05 unclosed",
            "",
        ] {
            assert_eq!(read(line), (None, line), "{line}");
        }
    }

    // FR-ATT-05, attachments.md §5
    #[test]
    fn a_new_comment_is_stamped_and_an_edit_keeps_the_stamp() {
        let at = date(2026, 10, 8).at(9, 5, 59, 0);
        assert_eq!(stamped(at, "Bought it"), "[2026-10-08 09:05] Bought it");
        assert_eq!(
            reworded("[2026-10-08 09:05] Bought it", "Sold it"),
            "[2026-10-08 09:05] Sold it"
        );
        assert_eq!(reworded("Typed by hand", "Retyped"), "Retyped");
    }
}
