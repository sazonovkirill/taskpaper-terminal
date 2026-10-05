//! The canonical-outline rules that one line's own text can break, and their repairs
//! ([file-format.md §3.1](../../../../docs/specs/file-format.md#31-canonical-outlines)): an empty
//! line with children (rule 2) and a line starting with whitespace (rule 4) would be read back
//! elsewhere.

use super::tags::is_whitespace;

/// Stands in for the text of a line that would otherwise be empty where an empty line would be
/// read back elsewhere (rules 2 and 3).
pub const PLACEHOLDER: &str = "…";

/// `text` repaired for a line that has children or not and is the document's only line or not:
/// without leading whitespace unless it is the only line, which keeps it (rule 4, file-format.md
/// §2), and the placeholder in place of an empty text when it has children (rule 2). A line
/// breaking neither rule comes back unchanged.
#[must_use]
pub fn repaired(text: &str, has_children: bool, only_line: bool) -> String {
    let trimmed = if only_line {
        text
    } else {
        text.trim_start_matches(is_whitespace)
    };
    if trimmed.is_empty() && has_children {
        PLACEHOLDER.to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// Whether a line with `text`, with children or not and the only line or not, breaks rule 2 or
/// rule 4.
#[must_use]
pub fn needs_repair(text: &str, has_children: bool, only_line: bool) -> bool {
    repaired(text, has_children, only_line) != text
}

#[cfg(test)]
mod tests {
    use super::*;

    // file-format §3.1
    #[test]
    fn leading_whitespace_is_trimmed() {
        assert_eq!(repaired("  a b ", false, false), "a b ");
        assert_eq!(repaired("\t\u{3000}- a", true, false), "- a");
        assert_eq!(repaired(" ", false, false), "");
    }

    // file-format §2, §3.1
    #[test]
    fn the_only_line_keeps_its_leading_whitespace() {
        assert_eq!(repaired("  - a", false, true), "  - a");
        assert!(!needs_repair(" a", false, true));
    }

    // file-format §3.1
    #[test]
    fn an_empty_line_with_children_gets_the_placeholder() {
        assert_eq!(repaired("", true, false), "…");
        assert_eq!(repaired("  ", true, false), "…");
        assert_eq!(repaired("", false, false), "", "an empty leaf stays empty");
    }

    #[test]
    fn a_canonical_line_is_kept() {
        for (text, has_children) in [("a", true), ("- a", false), ("a  ", true), ("@x", true)] {
            assert_eq!(repaired(text, has_children, false), text);
            assert!(!needs_repair(text, has_children, false), "{text:?}");
        }
    }

    #[test]
    fn lines_breaking_a_rule_need_repair() {
        assert!(needs_repair(" a", false, false));
        assert!(needs_repair("", true, false));
        assert!(!needs_repair("", false, false));
    }
}
