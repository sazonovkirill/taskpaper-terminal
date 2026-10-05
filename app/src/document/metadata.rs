//! Block metadata commands as line edits: which tags a command writes or removes on a block's
//! line ([data-model.md §3–4](../../../docs/specs/data-model.md#4-edits),
//! [editing.md §5](../../../docs/specs/editing.md#5-block-commands)).

use jiff::civil::Date;

use super::{
    blocks::{Block, TagColor},
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
    let current = match tag {
        DateTag::Start => block.start(),
        DateTag::Due => block.due(),
    };
    let date = match change {
        DateChange::Today => Ok(today),
        DateChange::Tomorrow => today.tomorrow(),
        DateChange::Earlier => current.unwrap_or(today).yesterday(),
        DateChange::Later => current.unwrap_or(today).tomorrow(),
    }
    .ok()?;
    set_date(document, block, tag, Some(date))
}

/// The op that sets the `tag` date of `block` to `date` as `YYYY-MM-DD` (D-6), or removes the
/// tag with any value when `date` is `None` (editing.md §5). `None` when the line stays as it is
/// or the block is not in `document`.
#[must_use]
pub fn set_date(
    document: &Document,
    block: &Block,
    tag: DateTag,
    date: Option<Date>,
) -> Option<Op> {
    let before = document.get(block.id())?.text();
    let name = match tag {
        DateTag::Start => "start",
        DateTag::Due => "due",
    };
    let after = match date {
        Some(date) => tags::set(before, name, &date.to_string()),
        None => kept(tags::remove(before, name)),
    };
    (after != before).then(|| Op::SetText {
        id: block.id(),
        before: before.to_owned(),
        after,
    })
}

/// A bare tag that a command adds to a block or removes from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    /// `@today`, which puts a block on today's list (FR-TAG-04).
    Today,
    /// `@bookmark`, which lists a block in the sidebar (D-11).
    Bookmark,
}

/// The op that adds the bare tag `flag` to `block`, or, when it has the tag, removes it with any
/// value (editing.md §5); `None` when the block is not in `document`.
#[must_use]
pub fn toggle_flag(document: &Document, block: &Block, flag: Flag) -> Option<Op> {
    let before = document.get(block.id())?.text();
    let (name, set) = match flag {
        Flag::Today => ("today", block.today()),
        Flag::Bookmark => ("bookmark", block.bookmark()),
    };
    let after = if set {
        kept(tags::remove(before, name))
    } else {
        tags::set(before, name, "")
    };
    Some(Op::SetText {
        id: block.id(),
        before: before.to_owned(),
        after,
    })
}

/// The op that gives `block` the color `color`, or none: every other color tag goes and the
/// color's own tag stays where it is or is added (FR-TAG-05, D-5). `None` when the line stays as
/// it is or the block is not in `document`.
#[must_use]
pub fn set_color(document: &Document, block: &Block, color: Option<TagColor>) -> Option<Op> {
    let before = document.get(block.id())?.text();
    let mut after = TagColor::ALL
        .into_iter()
        .filter(|&other| Some(other) != color)
        .fold(before.to_owned(), |text, other| {
            tags::remove(&text, other.tag())
        });
    if let Some(color) = color {
        after = tags::set(&after, color.tag(), "");
    }
    let after = kept(after);
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

    /// The first line's text after the op `make` returns for its block, or `None` without one.
    #[track_caller]
    fn edited(text: &str, make: impl Fn(&Document, &Block) -> Option<Op>) -> Option<String> {
        let mut document = parse(text);
        let block = blocks::children(&document, document.root())
            .into_iter()
            .next()
            .unwrap();
        let op = make(&document, &block)?;
        assert!(
            matches!(op, Op::SetText { .. }),
            "only text changes: {op:?}"
        );
        let mut transaction = crate::document::Transaction::new(vec![op], (), ());
        document.apply(&mut transaction).unwrap();
        Some(document.get(block.id()).unwrap().text().to_owned())
    }

    // FR-TAG-04, D-11, editing.md §5
    #[test]
    fn toggling_a_flag_adds_it_bare_or_removes_it_with_any_value() {
        let check = |text: &str, flag, expected: &str| {
            assert_eq!(
                edited(text, |document, block| toggle_flag(document, block, flag)).as_deref(),
                Some(expected),
                "{text:?} {flag:?}"
            );
        };
        check("- a", Flag::Today, "- a @today");
        check("- a @x", Flag::Bookmark, "- a @x @bookmark");
        check("P: @today", Flag::Today, "P:");
        check("- a @today(x) @y @today", Flag::Today, "- a @y");
        check("- a @bookmark @today", Flag::Bookmark, "- a @today");
        check("- a @today", Flag::Bookmark, "- a @today @bookmark");
        check("@bookmark", Flag::Bookmark, PLACEHOLDER);
    }

    #[test]
    fn a_flag_of_a_block_not_in_the_document_is_not_toggled() {
        let other = parse("- x\n- y");
        let stale = blocks::children(&other, other.root())[1].clone();
        assert_eq!(toggle_flag(&parse("- a"), &stale, Flag::Today), None);
    }

    // FR-TAG-05, D-5
    #[test]
    fn setting_a_color_replaces_every_other_color() {
        use TagColor::{Blue, Orange, Red};
        let check = |text: &str, color, expected: Option<&str>| {
            assert_eq!(
                edited(text, |document, block| set_color(document, block, color)).as_deref(),
                expected,
                "{text:?} {color:?}"
            );
        };
        check("- a", Some(Red), Some("- a @red"));
        check("- a @blue @x", Some(Orange), Some("- a @x @orange"));
        check("- a @red @x @blue", Some(Blue), Some("- a @x @blue"));
        check("- a @blue @x @red", Some(Blue), Some("- a @blue @x"));
        check("P: @green @violet @yellow", None, Some("P:"));
        check("@red", None, Some(PLACEHOLDER));
        check("- a @red @x", Some(Red), None);
        check("- a @x", None, None);
    }

    #[test]
    fn a_color_of_a_block_not_in_the_document_is_not_set() {
        let other = parse("- x\n- y");
        let stale = blocks::children(&other, other.root())[1].clone();
        assert_eq!(set_color(&parse("- a"), &stale, Some(TagColor::Red)), None);
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

    /// The first line's text after setting its `tag` date to `date`, or `None` when nothing
    /// changes.
    #[track_caller]
    fn set(text: &str, tag: DateTag, date: Option<Date>) -> Option<String> {
        let mut document = parse(text);
        let block = blocks::children(&document, document.root())
            .into_iter()
            .next()
            .unwrap();
        let op = set_date(&document, &block, tag, date)?;
        let mut transaction = crate::document::Transaction::new(vec![op], (), ());
        document.apply(&mut transaction).unwrap();
        Some(document.get(block.id()).unwrap().text().to_owned())
    }

    // FR-TAG-03, D-6
    #[test]
    fn set_date_writes_the_date_in_place_or_adds_it() {
        let friday = Some(date(2026, 10, 9));
        assert_eq!(
            set("- a", DateTag::Due, friday).as_deref(),
            Some("- a @due(2026-10-09)")
        );
        assert_eq!(
            set("- a @start(2026-01-01) @x", DateTag::Start, friday).as_deref(),
            Some("- a @start(2026-10-09) @x")
        );
        assert_eq!(
            set("- a @due(soon)", DateTag::Due, friday).as_deref(),
            Some("- a @due(2026-10-09)")
        );
        assert_eq!(set("- a @due(2026-10-09)", DateTag::Due, friday), None);
    }

    // FR-TAG-03, editing.md §5
    #[test]
    fn set_date_without_a_date_removes_the_tag_with_any_value() {
        assert_eq!(
            set("- a @start(2026-10-01) @due(x) @y", DateTag::Due, None).as_deref(),
            Some("- a @start(2026-10-01) @y")
        );
        assert_eq!(
            set("- a @start @due(2026-10-01)", DateTag::Start, None).as_deref(),
            Some("- a @due(2026-10-01)")
        );
        assert_eq!(set("- a @due(2026-10-01)", DateTag::Start, None), None);
        assert_eq!(
            set("@due(2026-10-01)", DateTag::Due, None).as_deref(),
            Some("…"),
            "a line left empty gets the placeholder"
        );
    }

    #[test]
    fn set_date_of_a_block_not_in_the_document_changes_nothing() {
        let other = parse("- x\n- y");
        let stale = blocks::children(&other, other.root())[1].clone();
        assert_eq!(set_date(&parse("- a"), &stale, DateTag::Due, None), None);
    }

    #[test]
    fn toggling_twice_restores_an_open_block() {
        let once = toggled("- Milk @due(1)");
        assert_eq!(toggled(&once.join("\n")), ["- Milk @due(1)"]);
    }
}
