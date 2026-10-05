//! Block metadata commands as line edits: which tags a command writes or removes on a block's
//! line ([data-model.md §3–4](../../../docs/specs/data-model.md#4-edits),
//! [editing.md §5](../../../docs/specs/editing.md#5-block-commands)).

use jiff::civil::Date;

use super::{
    blocks::Block,
    format::{canonical::PLACEHOLDER, tags},
    model::{Document, Op},
};

/// The op that completes `block` with `@done(today)`, or, when it is done already, removes
/// `@done` with any value (FR-DONE-01, D-4); `None` when the block is not in `document`.
#[must_use]
pub fn toggle_done(document: &Document, block: &Block, today: Date) -> Option<Op> {
    let before = document.get(block.id())?.text();
    let after = if block.done() {
        kept(tags::remove(before, DONE))
    } else {
        tags::set(before, DONE, &today.to_string())
    };
    Some(Op::SetText {
        id: block.id(),
        before: before.to_owned(),
        after,
    })
}

/// The tag that marks a block completed.
const DONE: &str = "done";

/// `after`, or the placeholder when the edit emptied the line, because an empty line would be
/// read back elsewhere when it has children or ends the file below the top level.
fn kept(after: String) -> String {
    if after.is_empty() {
        PLACEHOLDER.to_owned()
    } else {
        after
    }
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
        let op = toggle_done(&document, &block, TODAY).unwrap();
        assert!(
            matches!(op, Op::SetText { .. }),
            "only text changes: {op:?}"
        );
        let mut transaction = crate::document::Transaction::new(vec![op], (), ());
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

    // FR-DONE-01, D-40
    #[test]
    fn a_text_block_is_completed_on_its_own_line() {
        assert_eq!(toggled("a\nb"), ["a @done(2026-10-05)", "b"]);
    }

    // file-format §3.1
    #[test]
    fn a_line_left_empty_gets_the_placeholder_so_the_outline_stays_canonical() {
        assert_eq!(toggled("@done\n\t- x"), ["…", "- x"]);
        assert_eq!(toggled("@done"), ["…"]);
    }

    #[test]
    fn a_block_not_in_the_document_is_not_toggled() {
        let other = parse("- x\n- y");
        let stale = blocks::children(&other, other.root())[1].clone();
        assert_eq!(toggle_done(&parse("- a"), &stale, TODAY), None);
    }

    #[test]
    fn toggling_twice_restores_an_open_block() {
        let once = toggled("- Milk @due(1)");
        assert_eq!(toggled(&once.join("\n")), ["- Milk @due(1)"]);
    }
}
