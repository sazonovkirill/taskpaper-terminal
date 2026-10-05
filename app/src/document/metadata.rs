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

/// Which of a block's dates a command sets (FR-TAG-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateTag {
    /// `@start`.
    Start,
    /// `@due`.
    Due,
}

/// How a quick date command changes a date, without a prompt (FR-TAG-06, D-26).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateChange {
    /// To today.
    Today,
    /// To tomorrow.
    Tomorrow,
    /// One day back.
    Earlier,
    /// One day forward.
    Later,
}

/// The op that sets the `tag` date of `block` as `change` says, `today` being the local date; a
/// block without that date moves as if it were today (editing.md §5). `None` when the date stays
/// as it is, would leave the calendar or the block is not in `document`.
#[must_use]
pub fn change_date(
    document: &Document,
    block: &Block,
    tag: DateTag,
    change: DateChange,
    today: Date,
) -> Option<Op> {
    let before = document.get(block.id())?.text();
    let (name, current) = match tag {
        DateTag::Start => ("start", block.start()),
        DateTag::Due => ("due", block.due()),
    };
    let date = match change {
        DateChange::Today => Ok(today),
        DateChange::Tomorrow => today.tomorrow(),
        DateChange::Earlier => current.unwrap_or(today).yesterday(),
        DateChange::Later => current.unwrap_or(today).tomorrow(),
    }
    .ok()?;
    let after = tags::set(before, name, &date.to_string());
    (after != before).then(|| Op::SetText {
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

    /// The first line's text after `change` to its `tag` date, or `None` when nothing changes.
    #[track_caller]
    fn changed(text: &str, tag: DateTag, change: DateChange, today: Date) -> Option<String> {
        let mut document = parse(text);
        let block = blocks::children(&document, document.root())
            .into_iter()
            .next()
            .unwrap();
        let op = change_date(&document, &block, tag, change, today)?;
        assert!(
            matches!(op, Op::SetText { .. }),
            "only text changes: {op:?}"
        );
        let mut transaction = crate::document::Transaction::new(vec![op], (), ());
        document.apply(&mut transaction).unwrap();
        Some(document.get(block.id()).unwrap().text().to_owned())
    }

    // FR-TAG-06, D-26
    #[test]
    fn today_and_tomorrow_set_the_date_or_add_it() {
        use DateChange::{Today, Tomorrow};
        use DateTag::{Due, Start};
        let check = |text: &str, tag, change, expected: &str| {
            assert_eq!(
                changed(text, tag, change, TODAY).as_deref(),
                Some(expected),
                "{text:?} {tag:?} {change:?}"
            );
        };
        check("- a", Start, Today, "- a @start(2026-10-05)");
        check("- a", Start, Tomorrow, "- a @start(2026-10-06)");
        check("- a @x", Due, Today, "- a @x @due(2026-10-05)");
        check("P:", Due, Tomorrow, "P: @due(2026-10-06)");
        check(
            "- a @due(2026-01-01) @x",
            Due,
            Tomorrow,
            "- a @due(2026-10-06) @x",
        );
        check(
            "- a @start(2026-10-09) @due(2026-10-09)",
            Start,
            Today,
            "- a @start(2026-10-05) @due(2026-10-09)",
        );
        check("- a @due(soon)", Due, Today, "- a @due(2026-10-05)");
        check("- a @due", Due, Tomorrow, "- a @due(2026-10-06)");
    }

    // FR-TAG-06, D-26
    #[test]
    fn earlier_and_later_move_the_date_a_day() {
        use DateChange::{Earlier, Later};
        use DateTag::{Due, Start};
        let check = |text: &str, tag, change, expected: &str| {
            assert_eq!(
                changed(text, tag, change, TODAY).as_deref(),
                Some(expected),
                "{text:?} {tag:?} {change:?}"
            );
        };
        check(
            "- a @start(2026-12-31)",
            Start,
            Later,
            "- a @start(2027-01-01)",
        );
        check(
            "- a @start(2026-03-01)",
            Start,
            Earlier,
            "- a @start(2026-02-28)",
        );
        check(
            "- a @due(2026-10-01) @start(2026-10-01)",
            Due,
            Later,
            "- a @due(2026-10-02) @start(2026-10-01)",
        );
        check("- a @due(2020-01-10)", Due, Earlier, "- a @due(2020-01-09)");
    }

    // editing.md §5
    #[test]
    fn earlier_and_later_without_a_date_count_from_today() {
        use DateChange::{Earlier, Later};
        use DateTag::{Due, Start};
        let check = |text: &str, tag, change, expected: &str| {
            assert_eq!(
                changed(text, tag, change, TODAY).as_deref(),
                Some(expected),
                "{text:?} {tag:?} {change:?}"
            );
        };
        check("- a", Start, Earlier, "- a @start(2026-10-04)");
        check("- a", Due, Later, "- a @due(2026-10-06)");
        check("- a @due(someday)", Due, Earlier, "- a @due(2026-10-04)");
        check(
            "- a @start(2026-10-01)",
            Due,
            Later,
            "- a @start(2026-10-01) @due(2026-10-06)",
        );
    }

    #[test]
    fn a_date_that_stays_or_would_leave_the_calendar_changes_nothing() {
        assert_eq!(
            changed(
                "- a @due(2026-10-05)",
                DateTag::Due,
                DateChange::Today,
                TODAY
            ),
            None
        );
        assert_eq!(
            changed(
                "- a @start(2026-10-06)",
                DateTag::Start,
                DateChange::Tomorrow,
                TODAY
            ),
            None
        );
        assert_eq!(
            changed(
                "- a @due(9999-12-31)",
                DateTag::Due,
                DateChange::Later,
                TODAY
            ),
            None
        );
        assert_eq!(
            changed(
                "- a @start(-009999-01-01)",
                DateTag::Start,
                DateChange::Earlier,
                TODAY
            ),
            None
        );
        assert_eq!(
            changed("- a", DateTag::Due, DateChange::Tomorrow, Date::MAX),
            None
        );
    }

    #[test]
    fn a_date_of_a_block_not_in_the_document_is_not_changed() {
        let other = parse("- x\n- y");
        let stale = blocks::children(&other, other.root())[1].clone();
        assert_eq!(
            change_date(
                &parse("- a"),
                &stale,
                DateTag::Due,
                DateChange::Today,
                TODAY
            ),
            None
        );
    }

    #[test]
    fn toggling_twice_restores_an_open_block() {
        let once = toggled("- Milk @due(1)");
        assert_eq!(toggled(&once.join("\n")), ["- Milk @due(1)"]);
    }
}
