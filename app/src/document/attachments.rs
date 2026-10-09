//! A block's attachments as lines of text: where their special nodes sit among the block's
//! children, and how their lines are guarded so that the file reads them back unchanged
//! ([attachments.md §1](../../../docs/specs/attachments.md#1-storage-fr-att-01), D-94).

use super::{
    blocks::{self, Attachment},
    format::{breaks_line, tags},
    model::{Document, ItemId, Op, Snapshot},
    structure,
};

/// The invisible character that guards a line (attachments.md §1.1).
const GUARD: char = '\u{200B}';

/// The node of the block `block`'s attachment of kind `kind`: the first of its children that is
/// one, or `None` when it has none or is no block.
#[must_use]
pub fn node(document: &Document, block: ItemId, kind: Attachment) -> Option<ItemId> {
    blocks::block(document, block)?;
    document
        .get(block)?
        .children()
        .iter()
        .copied()
        .find(|&child| {
            document
                .get(child)
                .is_some_and(|item| blocks::attachment(item) == Some(kind))
        })
}

/// The lines of the block `block`'s attachment of kind `kind` as the user wrote them, every line
/// of its node's subtree in order without its guards; none when it has no such attachment.
#[must_use]
pub fn content(document: &Document, block: ItemId, kind: Attachment) -> Vec<String> {
    node(document, block, kind).map_or_else(Vec::new, |node| {
        document
            .descendants(node)
            .filter_map(|(id, _)| Some(unguard(document.get(id)?.text())))
            .collect()
    })
}

/// The ops that make `lines` the content of the block `block`'s attachment of kind `kind`, each
/// split where it holds a line break and without the trailing blank lines, which the file would
/// read back elsewhere. They create the node where §1 places it, delete it when no line is left,
/// and change only the lines that differ, so the others keep their original bytes. `None` when
/// `block` is no block or a divider, which has no children (FR-BLK-07).
#[must_use]
pub fn write(
    document: &mut Document,
    block: ItemId,
    kind: Attachment,
    lines: &[String],
) -> Option<Vec<Op>> {
    if blocks::block(document, block)?.divider() {
        return None;
    }
    let mut new: Vec<String> = lines
        .iter()
        .flat_map(|line| {
            line.replace("\r\n", "\n")
                .split(breaks_line)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .map(|line| guard(&line))
        .collect();
    while new.last().is_some_and(String::is_empty) {
        new.pop();
    }
    let Some(node) = node(document, block, kind) else {
        return Some(if new.is_empty() {
            Vec::new()
        } else {
            create(document, block, kind, new)
        });
    };
    if new.is_empty() {
        return Some(vec![structure::delete_line(document, node)?]);
    }
    let old = document.get(node)?.children().to_vec();
    let flat = old.iter().all(|&id| {
        document
            .get(id)
            .is_some_and(|item| item.children().is_empty())
    });
    if !flat {
        let mut ops: Vec<Op> = old
            .iter()
            .rev()
            .filter_map(|&id| structure::delete_line(document, id))
            .collect();
        ops.extend(inserts(document, node, 0, new));
        return Some(ops);
    }
    Some(changes(document, node, &old, &new))
}

/// The ops that change the flat lines `old` of `node` into `new`: the lines of their longest
/// common run stay, each run of lines between them is rewritten in place line for line, and the
/// lines it has too many or too few are removed or added.
fn changes(document: &mut Document, node: ItemId, old: &[ItemId], new: &[String]) -> Vec<Op> {
    let texts: Vec<(ItemId, String)> = old
        .iter()
        .filter_map(|&id| Some((id, document.get(id)?.text().to_owned())))
        .collect();
    let width = new.len() + 1;
    // At `i * width + j`: how long the longest common run of `texts[i..]` and `new[j..]` is.
    let mut common = vec![0_usize; (texts.len() + 1) * width];
    let at = |common: &[usize], i: usize, j: usize| common.get(i * width + j).copied().unwrap_or(0);
    for (i, (_, text)) in texts.iter().enumerate().rev() {
        for (j, line) in new.iter().enumerate().rev() {
            let run = if text == line {
                at(&common, i + 1, j + 1) + 1
            } else {
                at(&common, i + 1, j).max(at(&common, i, j + 1))
            };
            if let Some(cell) = common.get_mut(i * width + j) {
                *cell = run;
            }
        }
    }
    let (mut i, mut j, mut index) = (0, 0, 0);
    let (mut removed, mut added) = (Vec::new(), Vec::new());
    let mut ops = Vec::new();
    loop {
        match (texts.get(i), new.get(j)) {
            (None, None) => break,
            (Some((_, text)), Some(line)) if text == line => {
                index = replace(document, node, index, &mut removed, &mut added, &mut ops) + 1;
                i += 1;
                j += 1;
            }
            (None, Some(line)) => {
                added.push(line.clone());
                j += 1;
            }
            // A new line goes first when the common run is as long without it.
            (Some(_), Some(line)) if at(&common, i, j + 1) == at(&common, i, j) => {
                added.push(line.clone());
                j += 1;
            }
            (Some(old), _) => {
                removed.push(old.clone());
                i += 1;
            }
        }
    }
    replace(document, node, index, &mut removed, &mut added, &mut ops);
    ops
}

/// Adds to `ops` those that replace the lines `removed`, each with its text, at `index` on among the children of
/// `node`, with the lines `added`, rewriting them line for line, and empties both; returns the
/// index after the lines added.
fn replace(
    document: &mut Document,
    node: ItemId,
    mut index: usize,
    removed: &mut Vec<(ItemId, String)>,
    added: &mut Vec<String>,
    ops: &mut Vec<Op>,
) -> usize {
    let mut lines = added.drain(..);
    for (id, before) in removed.drain(..) {
        if let Some(line) = lines.next() {
            ops.push(Op::SetText {
                id,
                before,
                after: line,
            });
            index += 1;
        } else if let Some(subtree) = document.snapshot(id) {
            ops.push(Op::Remove {
                parent: node,
                index,
                subtree,
            });
        }
    }
    let rest: Vec<String> = lines.collect();
    let count = rest.len();
    ops.extend(inserts(document, node, index, rest));
    index + count
}

/// The ops that put a new node of kind `kind` holding `lines` under `block`: right after its
/// note for comments, right before its comments for a note, else after the text children that
/// start its children (attachments.md §1).
fn create(document: &mut Document, block: ItemId, kind: Attachment, lines: Vec<String>) -> Vec<Op> {
    let children = document.get(block).map_or(&[][..], |item| item.children());
    let after = |other| {
        node(document, block, other).and_then(|id| children.iter().position(|&child| child == id))
    };
    let index = match kind {
        Attachment::Comments => after(Attachment::Note).map(|at| at + 1),
        Attachment::Note => after(Attachment::Comments),
    }
    .unwrap_or_else(|| {
        children
            .iter()
            .take_while(|&&child| {
                document.get(child).is_some_and(|item| {
                    !blocks::is_special(item)
                        && blocks::block(document, child)
                            .is_some_and(|line| line.kind() == blocks::BlockKind::Text)
                })
            })
            .count()
    });
    let mut ops = structure::repairs(document, block);
    let id = document.new_id();
    ops.push(Op::Insert {
        parent: block,
        index,
        subtree: Snapshot::line(id, format!("{} @hidden", kind.label())),
    });
    ops.extend(inserts(document, id, 0, lines));
    ops
}

/// The ops that put `lines` among the children of `node` from `index` on.
fn inserts(document: &mut Document, node: ItemId, index: usize, lines: Vec<String>) -> Vec<Op> {
    lines
        .into_iter()
        .zip(index..)
        .map(|(line, index)| Op::Insert {
            parent: node,
            index,
            subtree: Snapshot::line(document.new_id(), line),
        })
        .collect()
}

/// `line` as the file stores it, so that it reads as text, keeps its leading whitespace and
/// stays inside its node (attachments.md §1.1): a guard before a line that starts with
/// whitespace, `-` or a guard, and one after the colon that ends its text before its trailing
/// tags, ignoring guards after it.
fn guard(line: &str) -> String {
    let mut text = String::with_capacity(line.len() + 2 * GUARD.len_utf8());
    if line.starts_with(|c| tags::is_whitespace(c) || c == '-' || c == GUARD) {
        text.push(GUARD);
    }
    text.push_str(line);
    let before_tags = tags::trailing_run_start(&text, &tags::scan(&text));
    let unguarded = text[..before_tags].trim_end_matches(GUARD);
    if unguarded.ends_with(':') {
        text.insert(unguarded.len(), GUARD);
    }
    text
}

/// The line the user wrote, from `text` as the file stores it: without one guard at its start
/// and, where its text before its trailing tags ends in a colon and guards, one of those.
fn unguard(text: &str) -> String {
    let mut line = text.strip_prefix(GUARD).unwrap_or(text).to_owned();
    let before_tags = tags::trailing_run_start(&line, &tags::scan(&line));
    let content = &line[..before_tags];
    if content.ends_with(GUARD) && content.trim_end_matches(GUARD).ends_with(':') {
        line.remove(before_tags - GUARD.len_utf8());
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{
        Transaction,
        blocks::children,
        format::{self, serialize},
    };

    /// The first top-level block of `text`'s document.
    fn first(document: &Document) -> ItemId {
        children(document, document.root())[0].id()
    }

    /// `text` once the first top-level block's `kind` attachment holds `lines`, and the ops.
    fn written(text: &str, kind: Attachment, lines: &[&str]) -> (String, Vec<Op>) {
        let mut document = format::parse(text);
        let block = first(&document);
        let lines: Vec<String> = lines.iter().map(|&line| line.to_owned()).collect();
        let ops = write(&mut document, block, kind, &lines).unwrap();
        document
            .apply(&mut Transaction::new(ops.clone(), (), ()))
            .unwrap();
        (serialize(&document), ops)
    }

    // FR-ATT-01, attachments.md §1
    #[test]
    fn content_is_the_first_attachment_of_its_kind_without_guards() {
        let document = format::parse(concat!(
            "- t\n",
            "\tCOMMENTS @hidden\n\t\t[2026-10-08 00:40] c\n",
            "\tNOTE @hidden @x\n\t\t# Title\n\n\t\t\u{200B}- item\n\t\t\u{200B}\tcode\n\t\t\t\tdeep\n",
            "\tNOTE @hidden\n\t\tsecond\n",
            "- u",
        ));
        let block = first(&document);
        assert_eq!(
            content(&document, block, Attachment::Note),
            ["# Title", "", "- item", "\tcode", "deep"]
        );
        assert_eq!(
            content(&document, block, Attachment::Comments),
            ["[2026-10-08 00:40] c"]
        );
        let other = children(&document, document.root())[1].id();
        assert_eq!(content(&document, other, Attachment::Note), [""; 0]);
        let note = node(&document, block, Attachment::Note).unwrap();
        assert_eq!(node(&document, note, Attachment::Note), None, "no block");
        assert_eq!(content(&document, note, Attachment::Note), [""; 0]);
    }

    // attachments.md §1.1
    #[test]
    fn a_line_is_guarded_where_it_would_read_otherwise() {
        let cases = [
            ("plain", "plain"),
            ("", ""),
            ("  indented", "\u{200B}  indented"),
            ("\tcode", "\u{200B}\tcode"),
            ("- item", "\u{200B}- item"),
            ("--flag", "\u{200B}--flag"),
            ("\u{200B}x", "\u{200B}\u{200B}x"),
            ("Steps:", "Steps:\u{200B}"),
            ("def f():", "def f():\u{200B}"),
            ("Meeting: @at(10:30)", "Meeting:\u{200B} @at(10:30)"),
            ("a: @x @y  ", "a:\u{200B} @x @y  "),
            ("a:\u{200B}", "a:\u{200B}\u{200B}"),
            ("- Steps:", "\u{200B}- Steps:\u{200B}"),
            ("Inbox:  ", "Inbox:  "),
            ("a: b", "a: b"),
            (
                "[2026-10-08 00:40] Next:",
                "[2026-10-08 00:40] Next:\u{200B}",
            ),
        ];
        for (line, stored) in cases {
            assert_eq!(guard(line), stored, "{line:?}");
            assert_eq!(unguard(stored), line, "{stored:?}");
            let read = format::parse(&format!("N @hidden\n\t{stored}"));
            let item = read.get(read.descendants(read.root()).nth(1).unwrap().0);
            assert_eq!(
                item.map(|item| (item.text(), item.kind())),
                Some((stored, crate::document::ItemKind::Note)),
                "{stored:?} reads back as text"
            );
        }
    }

    // attachments.md §1.1
    #[test]
    fn unguarding_removes_only_where_guards_go() {
        assert_eq!(unguard("a\u{200B}b"), "a\u{200B}b");
        assert_eq!(unguard("a\u{200B}"), "a\u{200B}");
        assert_eq!(unguard("\u{200B}"), "");
        assert_eq!(unguard(":\u{200B}\u{200B} @t"), ":\u{200B} @t");
    }

    // FR-ATT-01, attachments.md §1
    #[test]
    fn a_new_note_goes_after_the_leading_text_children() {
        let note = |text| written(text, Attachment::Note, &["n"]).0;
        assert_eq!(
            note("P:\n\ttext\n\tmore\n\t- t\n\tL @hidden"),
            "P:\n\ttext\n\tmore\n\tNOTE @hidden\n\t\tn\n\t- t\n\tL @hidden"
        );
        assert_eq!(
            note("P:\n\ttext\n\tL @hidden @log"),
            "P:\n\ttext\n\tNOTE @hidden\n\t\tn\n\tL @hidden @log",
            "before trailing special nodes"
        );
        assert_eq!(note("- t\n- u"), "- t\n\tNOTE @hidden\n\t\tn\n- u");
        assert_eq!(
            note("P:\n\t- t\n\tCOMMENTS @hidden\n\t\tc"),
            "P:\n\t- t\n\tNOTE @hidden\n\t\tn\n\tCOMMENTS @hidden\n\t\tc",
            "right before the comments, wherever they are"
        );
    }

    // FR-ATT-01, attachments.md §1
    #[test]
    fn new_comments_go_after_the_note_or_the_leading_text_children() {
        let comments = |text| written(text, Attachment::Comments, &["c"]).0;
        assert_eq!(
            comments("P:\n\t- t\n\tNOTE @hidden\n\t\tn\n\t- u"),
            "P:\n\t- t\n\tNOTE @hidden\n\t\tn\n\tCOMMENTS @hidden\n\t\tc\n\t- u"
        );
        assert_eq!(
            comments("P:\n\ttext\n\tP2:"),
            "P:\n\ttext\n\tCOMMENTS @hidden\n\t\tc\n\tP2:"
        );
    }

    // FR-ATT-01, file-format.md §3.1
    #[test]
    fn a_new_node_repairs_an_empty_owner() {
        assert_eq!(
            written("\n- u", Attachment::Note, &["n"]).0,
            "…\n\tNOTE @hidden\n\t\tn\n- u"
        );
    }

    // FR-ATT-01, attachments.md §1, §1.1
    #[test]
    fn lines_are_guarded_split_and_their_trailing_blanks_dropped() {
        assert_eq!(
            written(
                "- t",
                Attachment::Note,
                &["Steps:", "", "- one\r\n  two", "", "  "]
            )
            .0,
            "- t\n\tNOTE @hidden\n\t\tSteps:\u{200B}\n\t\t\n\t\t\u{200B}- one\n\t\t\u{200B}  two\n\t\t\n\t\t\u{200B}  "
        );
        let (text, ops) = written("- t", Attachment::Note, &["", ""]);
        assert_eq!(
            (text.as_str(), ops),
            ("- t", Vec::new()),
            "nothing to write"
        );
    }

    // FR-ATT-01, attachments.md §1
    #[test]
    fn a_change_rewrites_only_the_lines_that_differ() {
        let text = "- t\n\tCOMMENTS @hidden\n\t\t[1]   b\n\t\t[2] x\n\t\t[3]   a";
        let comments = |lines: &[&str]| written(text, Attachment::Comments, lines);
        let (added, ops) = comments(&["[0] new", "[1]   b", "[2] y", "[3]   a"]);
        assert_eq!(
            added,
            "- t\n\tCOMMENTS @hidden\n\t\t[0] new\n\t\t[1]   b\n\t\t[2] y\n\t\t[3]   a"
        );
        assert!(
            matches!(ops[..], [Op::Insert { index: 0, .. }, Op::SetText { .. }]),
            "one line added and one rewritten: {ops:?}"
        );
        let (removed, ops) = comments(&["[1]   b", "[3]   a"]);
        assert_eq!(removed, "- t\n\tCOMMENTS @hidden\n\t\t[1]   b\n\t\t[3]   a");
        assert!(matches!(ops[..], [Op::Remove { index: 1, .. }]), "{ops:?}");
        let (moved, ops) = comments(&["[2] x", "[3]   a", "[1]   b", "[4]"]);
        assert_eq!(
            moved,
            "- t\n\tCOMMENTS @hidden\n\t\t[2] x\n\t\t[3]   a\n\t\t[1]   b\n\t\t[4]"
        );
        assert!(
            matches!(
                ops[..],
                [
                    Op::Remove { index: 0, .. },
                    Op::Insert { index: 2, .. },
                    Op::Insert { index: 3, .. }
                ]
            ),
            "{ops:?}"
        );
        let (replaced, ops) = comments(&["z"]);
        assert_eq!(replaced, "- t\n\tCOMMENTS @hidden\n\t\tz");
        assert!(
            matches!(
                ops[..],
                [
                    Op::SetText { .. },
                    Op::Remove { index: 1, .. },
                    Op::Remove { index: 1, .. }
                ]
            ),
            "{ops:?}"
        );
        let (dropped, ops) = comments(&["[2] x", "[3]   a"]);
        assert_eq!(dropped, "- t\n\tCOMMENTS @hidden\n\t\t[2] x\n\t\t[3]   a");
        assert!(matches!(ops[..], [Op::Remove { index: 0, .. }]), "{ops:?}");
        let (dropped, ops) = comments(&["[1]   b", "[3]   a"]);
        assert_eq!(dropped, "- t\n\tCOMMENTS @hidden\n\t\t[1]   b\n\t\t[3]   a");
        assert!(matches!(ops[..], [Op::Remove { index: 1, .. }]), "{ops:?}");
        let (same, ops) = comments(&["[1]   b", "[2] x", "[3]   a"]);
        assert_eq!((same.as_str(), ops), (text, Vec::new()));
    }

    /// How long the longest common run of `old` and `new` is.
    fn common_run(old: &[&str], new: &[&str]) -> usize {
        match (old.split_first(), new.split_first()) {
            (Some((a, old_rest)), Some((b, new_rest))) if a == b => {
                1 + common_run(old_rest, new_rest)
            }
            (Some((_, old_rest)), Some((_, new_rest))) => {
                common_run(old_rest, new).max(common_run(old, new_rest))
            }
            _ => 0,
        }
    }

    // FR-ATT-01, attachments.md §1
    #[test]
    fn a_change_keeps_as_many_lines_as_old_and_new_have_in_common() {
        // Every run of one to four lines, each line one of three.
        let mut runs: Vec<Vec<&str>> = vec![Vec::new()];
        let mut last = runs.clone();
        for _ in 0..4 {
            last = last
                .iter()
                .flat_map(|run| ["a", "b", "c"].map(|line| [run.as_slice(), &[line]].concat()))
                .collect();
            runs.extend(last.iter().cloned());
        }
        for old in runs.iter().filter(|run| !run.is_empty()) {
            let text = format!("- t\n\tNOTE @hidden\n\t\t{}", old.join("\n\t\t"));
            for new in runs.iter().filter(|run| !run.is_empty()) {
                let (_, ops) = written(&text, Attachment::Note, new);
                let touched = ops
                    .iter()
                    .filter(|op| matches!(op, Op::SetText { .. } | Op::Remove { .. }))
                    .count();
                assert_eq!(
                    old.len() - touched,
                    common_run(old, new),
                    "{old:?} to {new:?}: {ops:?}"
                );
            }
        }
    }

    // FR-ATT-01, attachments.md §1
    #[test]
    fn unchanged_lines_keep_their_original_bytes() {
        let text = "- t\r\n    NOTE @hidden  \r\n        one  \r\n        two\r\n";
        let (written_text, _) = written(text, Attachment::Note, &["one  ", "2"]);
        assert_eq!(
            written_text,
            "- t\r\n    NOTE @hidden  \r\n        one  \r\n        2\r\n"
        );
    }

    // FR-ATT-01, attachments.md §1
    #[test]
    fn nested_lines_are_written_flat() {
        let text = "- t\n\tNOTE @hidden\n\t\ta\n\t\t\tdeep\n- u";
        assert_eq!(
            written(text, Attachment::Note, &["a", "deep", "c"]).0,
            "- t\n\tNOTE @hidden\n\t\ta\n\t\tdeep\n\t\tc\n- u"
        );
    }

    // FR-ATT-01, attachments.md §1
    #[test]
    fn emptying_an_attachment_deletes_its_node() {
        let text = "- t\n\tNOTE @hidden\n\t\tn\n\tCOMMENTS @hidden\n\t\tc\n- u";
        assert_eq!(
            written(text, Attachment::Note, &[""]).0,
            "- t\n\tCOMMENTS @hidden\n\t\tc\n- u"
        );
        assert_eq!(
            written(text, Attachment::Comments, &[]).0,
            "- t\n\tNOTE @hidden\n\t\tn\n- u"
        );
    }

    // FR-BLK-07, attachments.md §1
    #[test]
    fn a_divider_or_a_line_that_is_no_block_takes_no_attachment() {
        let mut document = format::parse("---\nX @hidden");
        let divider = first(&document);
        let lines = ["n".to_owned()];
        assert_eq!(
            write(&mut document, divider, Attachment::Note, &lines),
            None
        );
        let hidden = document.descendants(document.root()).nth(1).unwrap().0;
        assert_eq!(write(&mut document, hidden, Attachment::Note, &lines), None);
    }

    // FR-ATT-01, FR-DOC-03
    #[test]
    fn written_content_reads_back_the_same() {
        let lines = [
            "# Notes",
            "",
            "```py",
            "def f():",
            "    return 1",
            "```",
            "- [ ] list:",
            "\u{200B}odd",
            "Meeting: @at(10:30)",
        ];
        let (text, _) = written("P:\n\t- t", Attachment::Note, &lines);
        let document = format::parse(&text);
        assert_eq!(
            content(&document, first(&document), Attachment::Note),
            lines
        );
        assert_eq!(serialize(&document), text, "round-trips byte for byte");
    }
}
