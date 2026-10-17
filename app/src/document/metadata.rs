//! Block metadata commands as line edits: which tags a command writes or removes on a block's
//! line ([data-model.md §3–4](../../../docs/specs/data-model.md#4-edits),
//! [editing.md §5](../../../docs/specs/editing.md#5-block-commands)).

use jiff::civil::Date;

use super::{
    blocks::{self, Block, BlockKind, TagColor},
    format::{self, LineParts, Typed, canonical::PLACEHOLDER, tags},
    model::{Document, ItemKind, Op},
};

/// The op that completes `block` with `@done(today)`, or, when it is done already, removes
/// `@done` with any value (FR-DONE-01, D-4); `None` when the block is not in `document`.
#[must_use]
pub fn toggle_done(document: &Document, block: &Block, today: Date) -> Option<Op> {
    let before = document.get(block.id())?.text();
    let after = if block.done() {
        kept(tags::remove(before, DONE))
    } else {
        tagged(before, DONE, &today.to_string())
    };
    Some(Op::SetText {
        id: block.id(),
        before: before.to_owned(),
        after,
    })
}

/// The ops that complete each of `blocks` that is not done, or, when every one is done, uncomplete
/// them all (editing.md §5, FR-OPS-07); blocks not in `document` are left out.
#[must_use]
pub fn toggle_done_all(document: &Document, blocks: &[Block], today: Date) -> Vec<Op> {
    let done = blocks.iter().all(Block::done);
    blocks
        .iter()
        .filter(|block| block.done() == done)
        .filter_map(|block| toggle_done(document, block, today))
        .collect()
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
/// as it is, would leave the calendar or the dates `YYYY-MM-DD` can write, or the block is not in
/// `document`.
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
/// tag with any value when `date` is `None` (editing.md §5). `None` when the line stays as it is,
/// as it does when the tag holds that date already, written in any form, or when `date` is not
/// [`writable`], or the block is not in `document`.
#[must_use]
pub fn set_date(
    document: &Document,
    block: &Block,
    tag: DateTag,
    date: Option<Date>,
) -> Option<Op> {
    let before = document.get(block.id())?.text();
    let (name, current) = match tag {
        DateTag::Start => ("start", block.start()),
        DateTag::Due => ("due", block.due()),
    };
    if date.is_some_and(|date| !writable(date)) {
        return None;
    }
    // Rewriting such a tag would only drop what the app does not read, such as a time.
    if date.is_some() && date == current {
        return None;
    }
    let after = match date {
        Some(date) => tagged(before, name, &date.to_string()),
        None => kept(tags::remove(before, name)),
    };
    (after != before).then(|| Op::SetText {
        id: block.id(),
        before: before.to_owned(),
        after,
    })
}

/// Whether `date` can be stored as `YYYY-MM-DD` (D-6): a year before 0 would need a sign, and
/// the calendar ends at 9999-12-31 anyway.
#[must_use]
pub fn writable(date: Date) -> bool {
    date.year() >= 0
}

/// A bare tag that a command adds to a block or removes from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    /// `@star`, which stars a block (FR-TAG-04, D-88).
    Star,
    /// `@bookmark`, which lists a block in the Navigator (D-11).
    Bookmark,
    /// `@flag`, which marks a block with a red flag (D-91).
    Flagged,
}

/// The op that adds the bare tag `flag` to `block`, or, when it has the tag, removes it with any
/// value (editing.md §5); `None` when the block is not in `document`.
#[must_use]
pub fn toggle_flag(document: &Document, block: &Block, flag: Flag) -> Option<Op> {
    let before = document.get(block.id())?.text();
    let name = match flag {
        Flag::Star => "star",
        Flag::Bookmark => "bookmark",
        Flag::Flagged => "flag",
    };
    let after = if flagged(block, flag) {
        kept(tags::remove(before, name))
    } else {
        tagged(before, name, "")
    };
    Some(Op::SetText {
        id: block.id(),
        before: before.to_owned(),
        after,
    })
}

/// The ops that add the bare tag `flag` to each of `blocks` without it, or, when every one has
/// it, remove it from them all (editing.md §5, FR-OPS-07); blocks not in `document` are left out.
#[must_use]
pub fn toggle_flag_all(document: &Document, blocks: &[Block], flag: Flag) -> Vec<Op> {
    let set = blocks.iter().all(|block| flagged(block, flag));
    blocks
        .iter()
        .filter(|block| flagged(block, flag) == set)
        .filter_map(|block| toggle_flag(document, block, flag))
        .collect()
}

/// Whether `block` carries the tag `flag`.
fn flagged(block: &Block, flag: Flag) -> bool {
    match flag {
        Flag::Star => block.starred(),
        Flag::Bookmark => block.bookmark(),
        Flag::Flagged => block.flag(),
    }
}

/// The ops that make `to` the inbox project, or with `None` leave none: `@inbox` goes, with any
/// value, from every other project of `document` that has it, and comes to `to` when it lacks it
/// (D-141). Lines inside special nodes are no projects and stay as they are (FR-SPC-01). Empty
/// when nothing changes.
#[must_use]
pub fn set_inbox(document: &Document, to: Option<&Block>) -> Vec<Op> {
    let keep = to.map(Block::id);
    let mut ops = Vec::new();
    // The blocks still to visit at each depth on the way down to the current one.
    let mut levels = vec![blocks::each_child(document, document.root())];
    while let Some(level) = levels.last_mut() {
        let Some(block) = level.next() else {
            levels.pop();
            continue;
        };
        levels.push(blocks::each_child(document, block.id()));
        if block.inbox()
            && Some(block.id()) != keep
            && let Some(item) = document.get(block.id())
        {
            ops.extend(changed(
                item.text(),
                kept(tags::remove(item.text(), INBOX)),
                &block,
            ));
        }
    }
    if let Some(to) = to.filter(|to| !to.inbox())
        && let Some(item) = document.get(to.id())
    {
        ops.extend(changed(item.text(), tagged(item.text(), INBOX, ""), to));
    }
    ops
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
        after = tagged(&after, color.tag(), "");
    }
    let after = kept(after);
    (after != before).then(|| Op::SetText {
        id: block.id(),
        before: before.to_owned(),
        after,
    })
}

/// The op that gives `block` the type `to`, as the command menu of edit mode does: the old type's
/// syntax and the tag `@section` go, other tags stay and the new type's syntax comes
/// (editing.md §5, D-44, D-45). `None` when the line stays as it is or the block is not in
/// `document`.
#[must_use]
pub fn convert_type(document: &Document, block: &Block, to: Typed) -> Option<Op> {
    let before = document.get(block.id())?.text();
    let (parts, text) = LineParts::split(before);
    let (parts, text) = parts.retyped(to, &text);
    changed(before, kept(parts.line(&text)), block)
}

/// The op that makes the text block `block` a section by adding the bare tag `@section`, or with
/// `on` false, plain text by removing the tag with any value (FR-BLK-04, D-52). `None` when it
/// already is what `on` asks, is a task or project, which a section cannot be, or is not in
/// `document`.
#[must_use]
pub fn set_section(document: &Document, block: &Block, on: bool) -> Option<Op> {
    if block.kind() != BlockKind::Text || block.section() == on {
        return None;
    }
    let before = document.get(block.id())?.text();
    let after = if on {
        tags::set(before, SECTION, "")
    } else {
        kept(tags::remove(before, SECTION))
    };
    changed(before, after, block)
}

/// The op that writes `after` over the line `before` of `block`, or `None` when they are the
/// same.
fn changed(before: &str, after: String, block: &Block) -> Option<Op> {
    (after != before).then(|| Op::SetText {
        id: block.id(),
        before: before.to_owned(),
        after,
    })
}

/// The tag that marks a block completed.
const DONE: &str = "done";

/// The tag that makes a project the inbox (D-141).
const INBOX: &str = "inbox";

/// The tag that makes a text block a section (D-52).
const SECTION: &str = "section";

/// `before` with the tag `name` set to `value`. A task with no text gets the placeholder, since
/// `- @tag` would read back as a text block (file-format.md §3.1).
fn tagged(before: &str, name: &str, value: &str) -> String {
    let after = tags::set(before, name, value);
    if format::kind(before) == ItemKind::Task && format::kind(&after) != ItemKind::Task {
        let (parts, _) = LineParts::split(before);
        return tags::set(&parts.line(PLACEHOLDER), name, value);
    }
    after
}

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

    // file-format §3.1: `- @done` would read back as a text block, so an empty task keeps its
    // type with the placeholder.
    #[test]
    fn a_tag_on_an_empty_task_keeps_it_a_task() {
        assert_eq!(toggled("- "), ["- … @done(2026-10-05)"]);
        assert_eq!(
            edited("- ", |document, block| toggle_flag(
                document,
                block,
                Flag::Star
            ))
            .as_deref(),
            Some("- … @star")
        );
        assert_eq!(
            edited("- ", |document, block| {
                set_date(document, block, DateTag::Due, Some(TODAY))
            })
            .as_deref(),
            Some("- … @due(2026-10-05)")
        );
        assert_eq!(
            edited("- ", |document, block| {
                set_color(document, block, Some(TagColor::Red))
            })
            .as_deref(),
            Some("- … @red")
        );
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
        assert_eq!(
            changed(
                "- a @due(0000-01-01)",
                DateTag::Due,
                DateChange::Earlier,
                TODAY
            ),
            None,
            "the day before needs a signed year, which YYYY-MM-DD cannot write (D-6)"
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

    // FR-TAG-04, D-11, D-91, editing.md §5
    #[test]
    fn toggling_a_flag_adds_it_bare_or_removes_it_with_any_value() {
        let check = |text: &str, flag, expected: &str| {
            assert_eq!(
                edited(text, |document, block| toggle_flag(document, block, flag)).as_deref(),
                Some(expected),
                "{text:?} {flag:?}"
            );
        };
        check("- a", Flag::Star, "- a @star");
        check("- a @x", Flag::Bookmark, "- a @x @bookmark");
        check("P: @star", Flag::Star, "P:");
        check("- a @star(x) @y @star", Flag::Star, "- a @y");
        check("- a @bookmark @star", Flag::Bookmark, "- a @star");
        check("- a @star", Flag::Bookmark, "- a @star @bookmark");
        check("- a @red", Flag::Flagged, "- a @red @flag");
        check("- a @flag(x) @red", Flag::Flagged, "- a @red");
        check("@bookmark", Flag::Bookmark, PLACEHOLDER);
    }

    #[test]
    fn a_flag_of_a_block_not_in_the_document_is_not_toggled() {
        let other = parse("- x\n- y");
        let stale = blocks::children(&other, other.root())[1].clone();
        assert_eq!(toggle_flag(&parse("- a"), &stale, Flag::Star), None);
    }

    // FR-TAG-05, D-5, D-138: `@orange` and `@violet` are no colors, so they stay
    #[test]
    fn setting_a_color_replaces_every_other_color() {
        use TagColor::{Blue, Green, Red};
        let check = |text: &str, color, expected: Option<&str>| {
            assert_eq!(
                edited(text, |document, block| set_color(document, block, color)).as_deref(),
                expected,
                "{text:?} {color:?}"
            );
        };
        check("- a", Some(Red), Some("- a @red"));
        check("- a @blue @x", Some(Green), Some("- a @x @green"));
        check("- a @red @x @blue", Some(Blue), Some("- a @x @blue"));
        check("- a @blue @x @red", Some(Blue), Some("- a @blue @x"));
        check("P: @green @violet @yellow", None, Some("P: @violet"));
        check("- a @orange", Some(Red), Some("- a @orange @red"));
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

    /// Every line of `text` after the ops of `set_inbox` for its line numbered `to`, and how many
    /// ops there were.
    #[track_caller]
    fn inboxed(text: &str, to: Option<usize>) -> (Vec<String>, usize) {
        let mut document = parse(text);
        let ids: Vec<_> = document
            .descendants(document.root())
            .map(|(id, _)| id)
            .collect();
        let to = to.and_then(|line| blocks::block(&document, ids[line]));
        let ops = set_inbox(&document, to.as_ref());
        let count = ops.len();
        let mut transaction = crate::document::Transaction::new(ops, (), ());
        document.apply(&mut transaction).unwrap();
        let lines = document
            .descendants(document.root())
            .map(|(id, _)| document.get(id).unwrap().text().to_owned())
            .collect();
        (lines, count)
    }

    // D-141
    #[test]
    fn setting_the_inbox_tags_one_project_and_untags_every_other() {
        let text = "A: @inbox\n\tB:\n\t\tC: @inbox(x) @star\n- d @inbox";
        assert_eq!(
            inboxed(text, Some(1)),
            (
                ["A:", "B: @inbox", "C: @star", "- d @inbox"]
                    .map(str::to_owned)
                    .to_vec(),
                3
            ),
            "a task keeps its tag, which means nothing there"
        );
        assert_eq!(inboxed(text, Some(0)).1, 1, "A keeps its tag");
        assert_eq!(
            inboxed(text, None).0,
            ["A:", "B:", "C: @star", "- d @inbox"]
        );
        assert_eq!(inboxed("A: @inbox", Some(0)).1, 0);
        assert_eq!(inboxed("- a @inbox", None).1, 0);
    }

    // D-141, FR-SPC-01
    #[test]
    fn a_project_line_inside_a_special_node_keeps_its_inbox_tag() {
        let text = "A: @inbox\n\tARCHIVE @hidden\n\t\tOld: @inbox";
        assert_eq!(
            inboxed(text, None).0,
            ["A:", "ARCHIVE @hidden", "Old: @inbox"]
        );
    }

    /// The first line of `text` given the type `to`, or `None` when it stays as it is.
    #[track_caller]
    fn converted(text: &str, to: Typed) -> Option<String> {
        edited(text, |document, block| convert_type(document, block, to))
    }

    // FR-BLK-08, editing.md §5, D-44, D-52
    #[test]
    fn converting_swaps_the_type_syntax_and_keeps_other_tags() {
        use Typed::{Project, Section, Task, Text};
        let check = |text: &str, to, expected: Option<&str>| {
            assert_eq!(converted(text, to).as_deref(), expected, "{text:?} {to:?}");
        };
        check("a @x", Project, Some("a: @x"));
        check("P: @x", Task, Some("- P @x"));
        check("- t @done", Section, Some("t @done @section"));
        check("S @section @x", Text, Some("S @x"));
        check(
            "Guide @url(https://a.b) @x",
            Task,
            Some("- Guide @url(https://a.b) @x"),
        );
        check("@section", Text, Some(PLACEHOLDER));
        check("- t", Task, None);
        check("--- @red", Task, Some("- --- @red"));
        check("---", Text, None);
    }

    #[test]
    fn a_block_not_in_the_document_is_not_converted() {
        let other = parse("- x\n- y");
        let stale = blocks::children(&other, other.root())[1].clone();
        assert_eq!(convert_type(&parse("- a"), &stale, Typed::Text), None);
    }

    /// The first line of `text` with `@section` set as `on` says, or `None` when it stays as it
    /// is.
    #[track_caller]
    fn sectioned(text: &str, on: bool) -> Option<String> {
        edited(text, |document, block| set_section(document, block, on))
    }

    // FR-BLK-04, D-52
    #[test]
    fn a_text_block_becomes_a_section_and_back() {
        assert_eq!(sectioned("a @x", true).as_deref(), Some("a @x @section"));
        assert_eq!(
            sectioned("--- @x", true).as_deref(),
            Some("--- @x @section")
        );
        assert_eq!(
            sectioned("a @section(1) @x", false).as_deref(),
            Some("a @x")
        );
        assert_eq!(sectioned("@section", false).as_deref(), Some(PLACEHOLDER));
        assert_eq!(sectioned("a @section", true), None, "already a section");
        assert_eq!(sectioned("a", false), None, "no section to remove");
    }

    // FR-BLK-04, D-52
    #[test]
    fn a_task_or_project_never_takes_or_loses_the_section_tag() {
        for (text, on) in [
            ("- t", true),
            ("P:", true),
            ("- t @section", false),
            ("P: @section", false),
        ] {
            assert_eq!(sectioned(text, on), None, "{text:?}");
        }
        let other = parse("- x\nb");
        let stale = blocks::children(&other, other.root())[1].clone();
        assert_eq!(set_section(&parse("- a"), &stale, true), None);
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

    // FR-TAG-03, editing.md §5, D-6: a time the app does not model survives keeping the date.
    #[test]
    fn set_date_keeps_a_tag_that_holds_that_date_already_as_written() {
        let friday = Some(date(2026, 10, 9));
        assert_eq!(
            set("- a @due(2026-10-09 14:00)", DateTag::Due, friday),
            None
        );
        assert_eq!(
            set(
                "- a @start(2026-10-09T09:30) @due(2026-10-09 14:00)",
                DateTag::Start,
                friday
            ),
            None
        );
        assert_eq!(
            set("- a @due(2026-10-08 14:00)", DateTag::Due, friday).as_deref(),
            Some("- a @due(2026-10-09)"),
            "another date is written as YYYY-MM-DD"
        );
        assert_eq!(
            changed(
                "- a @due(2026-10-05 14:00)",
                DateTag::Due,
                DateChange::Today,
                TODAY
            ),
            None
        );
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

    // D-6, search.md §4: dates are stored as YYYY-MM-DD.
    #[test]
    fn set_date_writes_only_dates_yyyy_mm_dd_can_hold() {
        assert_eq!(
            set("- a", DateTag::Due, Some(date(0, 1, 1))).as_deref(),
            Some("- a @due(0000-01-01)")
        );
        assert_eq!(set("- a", DateTag::Due, Some(date(-1, 12, 31))), None);
        assert_eq!(
            set("- a @due(2026-10-09)", DateTag::Due, Some(Date::MIN)),
            None
        );
        assert!(writable(date(0, 1, 1)) && writable(Date::MAX));
        assert!(!writable(date(-1, 12, 31)));
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

    /// Each line's text after applying the ops `all` makes for every top-level block of `text`.
    #[track_caller]
    fn all_edited(text: &str, all: impl Fn(&Document, &[Block]) -> Vec<Op>) -> Vec<String> {
        let mut document = parse(text);
        let blocks = blocks::children(&document, document.root());
        let ops = all(&document, &blocks);
        let mut transaction = crate::document::Transaction::new(ops, (), ());
        document.apply(&mut transaction).unwrap();
        document
            .descendants(document.root())
            .map(|(id, _)| document.get(id).unwrap().text().to_owned())
            .collect()
    }

    // FR-OPS-07, FR-DONE-01, editing.md §5
    #[test]
    fn toggling_done_on_many_completes_the_open_ones_unless_all_are_done() {
        let all = |text: &str| all_edited(text, |d, b| toggle_done_all(d, b, TODAY));
        assert_eq!(
            all("- a\n- b @done(1)\n- c"),
            [
                "- a @done(2026-10-05)",
                "- b @done(1)",
                "- c @done(2026-10-05)"
            ]
        );
        assert_eq!(all("- a @done\n- b @done(1)"), ["- a", "- b"]);
        assert_eq!(all("- a"), ["- a @done(2026-10-05)"]);
        assert_eq!(all(""), Vec::<String>::new());
    }

    // FR-OPS-07, FR-TAG-04, D-11
    #[test]
    fn toggling_a_flag_on_many_adds_it_where_missing_unless_all_have_it() {
        let all = |text: &str, flag| all_edited(text, |d, b| toggle_flag_all(d, b, flag));
        assert_eq!(
            all("- a @star\n- b", Flag::Star),
            ["- a @star", "- b @star"]
        );
        assert_eq!(all("- a @star\n- b @star", Flag::Star), ["- a", "- b"]);
        assert_eq!(
            all("- a @bookmark\n- b @star", Flag::Bookmark),
            ["- a @bookmark", "- b @star @bookmark"]
        );
        assert_eq!(all("- a @bookmark", Flag::Bookmark), ["- a"]);
    }

    #[test]
    fn many_blocks_not_in_the_document_get_no_ops() {
        let other = parse("- x\n- y");
        let stale = &blocks::children(&other, other.root())[1..];
        let document = parse("- a");
        assert_eq!(toggle_done_all(&document, stale, TODAY), []);
        assert_eq!(toggle_flag_all(&document, stale, Flag::Star), []);
    }
}
