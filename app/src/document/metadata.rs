//! Block metadata commands as line edits: which tags a command writes or removes on which lines
//! of a block ([data-model.md §3–4](../../../docs/specs/data-model.md#4-edits),
//! [editing.md §5](../../../docs/specs/editing.md#5-block-commands)).

use jiff::civil::Date;

use super::{
    blocks::Block,
    format::{canonical::PLACEHOLDER, tags},
    model::{Document, ItemId, Op},
};

/// The ops that complete `block` with `@done(today)` on its first line, or, when it is done
/// already, remove `@done` with any value from every line that carries it (FR-DONE-01, D-4).
#[must_use]
pub fn toggle_done(document: &Document, block: &Block, today: Date) -> Vec<Op> {
    let lines = block
        .lines()
        .iter()
        .filter_map(|&id| Some((id, document.get(id)?.text())));
    if block.done() {
        lines
            .filter_map(|(id, text)| set_text(id, text, kept(text, tags::remove(text, DONE))))
            .collect()
    } else {
        lines
            .take(1)
            .filter_map(|(id, text)| set_text(id, text, tags::set(text, DONE, &today.to_string())))
            .collect()
    }
}

/// The tag that marks a block completed.
const DONE: &str = "done";

/// `after`, or the placeholder when an edit emptied `before`, because an empty line would be read
/// back elsewhere when it has children or ends the file below the top level; a line that was
/// blank stays blank.
fn kept(before: &str, after: String) -> String {
    if after.is_empty() && !before.is_empty() {
        PLACEHOLDER.to_owned()
    } else {
        after
    }
}

/// The op that changes line `id` from `before` to `after`, or `None` when they are the same.
fn set_text(id: ItemId, before: &str, after: String) -> Option<Op> {
    (before != after).then(|| Op::SetText {
        id,
        before: before.to_owned(),
        after,
    })
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;
    use crate::document::{blocks, format::parse};

    const TODAY: Date = date(2026, 10, 5);

    /// Each line's text after toggling the first block of `text` and applying the ops.
    #[track_caller]
    fn toggled(text: &str) -> Vec<String> {
        let mut document = parse(text);
        let block = blocks::children(&document, document.root())
            .into_iter()
            .next()
            .unwrap();
        let ops = toggle_done(&document, &block, TODAY);
        assert!(
            ops.iter().all(|op| matches!(op, Op::SetText { .. })),
            "only text changes: {ops:?}"
        );
        let mut transaction = crate::document::Transaction::new(ops, (), ());
        document.apply(&mut transaction).unwrap();
        document
            .descendants(document.root())
            .map(|(id, _)| document.get(id).unwrap().text().to_owned())
            .collect()
    }

    // FR-DONE-01, D-4
    #[test]
    fn an_open_block_gets_done_with_today() {
        assert_eq!(
            toggled("- Milk @due(1)"),
            ["- Milk @due(1) @done(2026-10-05)"]
        );
        assert_eq!(toggled("Inbox:"), ["Inbox: @done(2026-10-05)"]);
        assert_eq!(toggled("---"), ["--- @done(2026-10-05)"]);
    }

    // FR-DONE-01, D-17
    #[test]
    fn a_done_block_loses_done_with_any_value() {
        assert_eq!(toggled("- Milk @done(2026-10-01) @x"), ["- Milk @x"]);
        assert_eq!(toggled("- Milk @done"), ["- Milk"]);
        assert_eq!(toggled("Inbox: @done(someday)"), ["Inbox:"]);
    }

    // FR-DONE-01
    #[test]
    fn a_note_block_is_completed_on_its_first_line() {
        assert_eq!(toggled("a\nb\n- c"), ["a @done(2026-10-05)", "b", "- c"]);
    }

    // FR-DONE-01, data-model §3
    #[test]
    fn a_note_block_loses_done_on_every_line_that_carries_it() {
        assert_eq!(toggled("a\nb @done\nc @x @done(1)"), ["a", "b", "c @x"]);
    }

    // file-format §3.1
    #[test]
    fn a_line_left_empty_gets_the_placeholder_so_the_outline_stays_canonical() {
        assert_eq!(toggled("@done\n\t- x"), ["…", "- x"]);
        assert_eq!(toggled("a\n@done(1)\n\t- c"), ["a", "…", "- c"]);
        assert_eq!(toggled("@done"), ["…"]);
        assert_eq!(
            toggled("a @done\n\nb"),
            ["a", "", "b"],
            "blank lines stay blank"
        );
    }

    #[test]
    fn toggling_twice_restores_an_open_block() {
        let once = toggled("- Milk @due(1)");
        assert_eq!(toggled(&once.join("\n")), ["- Milk @due(1)"]);
    }
}
