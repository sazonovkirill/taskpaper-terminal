//! The canonical-outline rules that editing can break, and their repairs
//! ([file-format.md §3.1](../../../../docs/specs/file-format.md#31-canonical-outlines)): an empty
//! line with children (rule 2), empty lines ending the document below the top level (rule 3) and
//! a line starting with whitespace (rule 4) would be read back elsewhere.

use super::{
    super::model::{Document, ItemId, Op, Transaction},
    tags::is_whitespace,
};

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

/// The empty lines that end `document`, in order, when one of them is below the top level and
/// would be read back at the top level (rule 3); none when the rule holds.
#[must_use]
pub fn trailing_blanks(document: &Document) -> Vec<ItemId> {
    let blank = |id: &ItemId| document.get(*id).is_some_and(|item| item.text().is_empty());
    let run: Vec<ItemId> = from_the_end(document).take_while(blank).collect();
    let below_top = run.iter().any(|&id| {
        document
            .get(id)
            .is_some_and(|item| item.parent() != Some(document.root()) || item.extra_indent() > 0)
    });
    if below_top {
        run.into_iter().rev().collect()
    } else {
        Vec::new()
    }
}

/// Every line of `document` from the last one up, one at a time, so that looking at the last
/// few does not go through the whole document.
fn from_the_end(document: &Document) -> impl Iterator<Item = ItemId> + '_ {
    let children = |id| {
        document
            .get(id)
            .map_or(&[][..], |item| item.children())
            .iter()
            .map(|&child| (child, false))
    };
    // Lines still to go, the last on top, each with whether its children are already on the
    // stack above it.
    let mut stack: Vec<(ItemId, bool)> = children(document.root()).collect();
    std::iter::from_fn(move || {
        loop {
            let (id, expanded) = stack.pop()?;
            if expanded {
                return Some(id);
            }
            stack.push((id, true));
            stack.extend(children(id));
        }
    })
}

/// The ops that move each of the [`trailing_blanks`] to the end of the top level, in order and
/// without extra indent, so that the file reads them back where they are.
#[must_use]
pub fn lift_trailing_blanks(document: &Document) -> Vec<Op> {
    let root = document.root();
    let blanks = trailing_blanks(document);
    if blanks.is_empty() {
        return Vec::new();
    }
    let mut lifted = document.clone();
    let mut ops = Vec::new();
    let blanks = blanks
        .into_iter()
        .filter_map(|id| Some((id, document.get(id)?)));
    for (id, item) in blanks {
        if item.extra_indent() > 0 {
            ops.push(Op::SetExtraIndent {
                id,
                before: item.extra_indent(),
                after: 0,
            });
        }
        let Some(from) = lifted.position(id).filter(|&(parent, _)| parent != root) else {
            continue;
        };
        let end = lifted.get(root).map_or(0, |root| root.children().len());
        let to_end = Op::Move {
            id,
            from,
            to: (root, end),
        };
        if lifted
            .apply(&mut Transaction::new(vec![to_end.clone()], (), ()))
            .is_ok()
        {
            ops.push(to_end);
        }
    }
    ops
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::format::parse;

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

    fn written(document: &Document) -> String {
        crate::document::format::serialize(document)
    }

    /// The texts of the trailing blanks of `text`.
    fn blanks(text: &str) -> usize {
        trailing_blanks(&parse(text)).len()
    }

    // N-PERF-02
    #[test]
    fn lines_from_the_end_are_the_document_order_backwards() {
        let document = parse("A:\n\t- a\n\t\t- b\n\tc\nB:\n- d\n\t\te\n\tf");
        let mut order: Vec<ItemId> = document
            .descendants(document.root())
            .map(|(id, _)| id)
            .collect();
        order.reverse();
        assert_eq!(from_the_end(&document).collect::<Vec<_>>(), order);
    }

    // file-format §3.1
    #[test]
    fn trailing_blanks_are_the_empty_run_ending_the_document_when_one_is_indented() {
        assert_eq!(blanks("A:\n\t- a\n\t\n"), 0, "read back as top-level");
        let mut document = parse("A:\n\t- a\n\tb\n\tc");
        let ids: Vec<ItemId> = document
            .descendants(document.root())
            .map(|(id, _)| id)
            .collect();
        for (id, text) in [(ids[2], "b"), (ids[3], "c")] {
            let op = Op::SetText {
                id,
                before: text.to_owned(),
                after: String::new(),
            };
            document
                .apply(&mut Transaction::new(vec![op], (), ()))
                .unwrap();
        }
        assert_eq!(trailing_blanks(&document), [ids[2], ids[3]]);
        assert_eq!(blanks("A:\n\tb\n"), 0, "the last line has text");
        assert_eq!(blanks(""), 0);
        assert_eq!(blanks("a\n\n\n"), 0, "all at the top level");
    }

    /// The file for `text` with the lines reading `blank` emptied, then lifted.
    #[track_caller]
    fn lifted(text: &str, blank: &[&str]) -> String {
        let mut document = parse(text);
        let ops: Vec<Op> = document
            .descendants(document.root())
            .filter_map(|(id, _)| {
                let text = document.get(id)?.text();
                blank.contains(&text).then(|| Op::SetText {
                    id,
                    before: text.to_owned(),
                    after: String::new(),
                })
            })
            .collect();
        document.apply(&mut Transaction::new(ops, (), ())).unwrap();
        let before = written(&document);
        let mut lift = Transaction::new(lift_trailing_blanks(&document), (), ());
        document.apply(&mut lift).unwrap();
        assert!(trailing_blanks(&document).is_empty(), "rule 3 holds");
        let after = written(&document);
        let reread = parse(&after);
        assert_eq!(written(&reread), after);
        assert_eq!(
            reread.descendants(reread.root()).count(),
            document.descendants(document.root()).count(),
            "every line is read back"
        );
        document.apply(&mut lift.inverse()).unwrap();
        assert_eq!(written(&document), before, "undo");
        after
    }

    // file-format §3.1
    #[test]
    fn lifting_moves_the_trailing_blanks_to_the_top_level_in_order() {
        assert_eq!(lifted("A:\n\t- a\n\tb\n", &["b"]), "A:\n\t- a\n\n");
        assert_eq!(
            lifted("A:\n\t- a\n\tb\n\t\tc\n\td\n", &["b", "c", "d"]),
            "A:\n\t- a\n\n\n\n",
            "a blank parent and its blank child"
        );
        assert_eq!(lifted("a\n\t\t\tb", &["b"]), "a\n\n", "an indent jump");
        assert_eq!(lifted("- a", &[]), "- a", "nothing to lift");
        let document = parse("A:\n\t");
        let blank = trailing_blanks(&document);
        assert_eq!(blank.len(), 0, "a blank last line is read at the top level");
        let mut document = parse("A:\n\tb");
        let b = document.descendants(document.root()).nth(1).unwrap().0;
        let op = Op::SetText {
            id: b,
            before: "b".to_owned(),
            after: String::new(),
        };
        document
            .apply(&mut Transaction::new(vec![op], (), ()))
            .unwrap();
        assert_eq!(
            lift_trailing_blanks(&document),
            [Op::Move {
                id: b,
                from: (document.descendants(document.root()).next().unwrap().0, 0),
                to: (document.root(), 1)
            }],
            "only a move when there is no extra indent"
        );
    }

    // file-format §3.1
    #[test]
    fn lifting_drops_the_extra_indent_of_a_top_level_blank() {
        let mut document = Document::default();
        let root = document.root();
        document.push_child(root, "\t\ta".to_owned(), 0);
        let blank = document.push_child(root, String::new(), 2);
        assert_eq!(trailing_blanks(&document), [blank]);
        assert_eq!(
            lift_trailing_blanks(&document),
            [Op::SetExtraIndent {
                id: blank,
                before: 2,
                after: 0
            }]
        );
    }
}
