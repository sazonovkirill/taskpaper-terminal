//! The demo document that a start without a path opens (FR-DOC-05, D-27): the outline kept in
//! `assets/demo.taskpaper`, with its date placeholders resolved to a given day
//! ([file-io.md §1](../../../docs/specs/file-io.md#1-opening)).

use jiff::{Span, civil::Date};

/// The demo outline as written, with placeholders.
const DEMO: &str = include_str!("../../assets/demo.taskpaper");

/// The demo document with every placeholder resolved relative to `today`.
#[must_use]
pub fn text(today: Date) -> String {
    resolve(DEMO, today)
}

/// `text` with each `{today}`, `{today+N}` and `{today-N}` replaced by that day, `N` days from
/// `today`, as `YYYY-MM-DD`, so due and overdue dates stay meaningful whenever the demo is made.
/// Braces holding anything else are kept as written.
fn resolve(text: &str, today: Date) -> String {
    let mut parts = text.split('{');
    let mut out = parts.next().unwrap_or_default().to_owned();
    for part in parts {
        if let Some((day, after)) = part
            .split_once('}')
            .and_then(|(inside, after)| Some((day(inside, today)?, after)))
        {
            out.push_str(&day.to_string());
            out.push_str(after);
        } else {
            out.push('{');
            out.push_str(part);
        }
    }
    out
}

/// The day a placeholder's inside, such as `today-2`, stands for.
fn day(inside: &str, today: Date) -> Option<Date> {
    let offset = inside.strip_prefix("today")?;
    let days = if offset.is_empty() {
        0
    } else if offset.starts_with(['+', '-']) {
        offset.parse::<i64>().ok()?
    } else {
        return None;
    };
    today.checked_add(Span::new().try_days(days).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;
    use crate::{
        app::State,
        document::{BlockKind, Document, Item, ItemId, format},
    };

    const TODAY: Date = date(2026, 10, 5);

    #[track_caller]
    fn check(text: &str, expected: &str) {
        assert_eq!(resolve(text, TODAY), expected, "{text}");
    }

    #[test]
    fn resolves_today_and_days_before_and_after_it() {
        check("@due({today})", "@due(2026-10-05)");
        check("@due({today+1})", "@due(2026-10-06)");
        check("@due({today-5})", "@due(2026-09-30)");
        check("{today+27} and {today-1}.", "2026-11-01 and 2026-10-04.");
        check("- 日本 {today+0} 語", "- 日本 2026-10-05 語");
    }

    #[test]
    fn keeps_braces_that_hold_no_placeholder() {
        for text in [
            "",
            "no braces",
            "{",
            "}",
            "{}",
            "{today",
            "today}",
            "{tomorrow}",
            "{today1}",
            "{today+}",
            "{today+x}",
            "{today+99999999}",
            "{today*2}",
            "{ today}",
            "{{today}",
        ] {
            let expected = text.replace("{{today}", "{2026-10-05");
            check(text, &expected);
        }
    }

    /// The canonical-outline rules of file-format.md §3.1, checked on the parsed tree.
    fn assert_canonical(document: &Document) {
        let items: Vec<&Item> = document
            .descendants(document.root())
            .filter_map(|(id, _)| document.get(id))
            .collect();
        let placed = |item: &&Item| item.extra_indent() == 0;
        let not_empty_parent = |item: &&Item| !item.text().is_empty() || item.children().is_empty();
        let trimmed = |item: &&Item| !item.text().starts_with(char::is_whitespace);
        let blank = |item: &&&Item| item.text().is_empty();
        assert!(items.iter().all(placed), "placement");
        assert!(items.iter().all(not_empty_parent), "empty parent");
        assert!(items.iter().all(trimmed), "leading whitespace");
        let trailing_parents: Vec<Option<ItemId>> = items
            .iter()
            .rev()
            .take_while(blank)
            .copied()
            .map(Item::parent)
            .collect();
        let top_level = vec![Some(document.root()); trailing_parents.len()];
        assert_eq!(trailing_parents, top_level, "trailing indented blank");
    }

    // FR-DOC-05, D-27
    #[test]
    fn demo_resolves_every_placeholder() {
        let text = text(TODAY);
        assert!(!text.contains('{'), "an unresolved placeholder");
        assert!(
            text.contains("@done(2026-10-04)"),
            "demo lacks a resolved date"
        );
    }

    // FR-DOC-05, D-27
    #[test]
    fn demo_is_a_canonical_outline_with_every_implemented_type() {
        let state = State::new(format::parse(&text(TODAY)), None);
        assert_canonical(state.document());
        let rows = state.rows();
        for kind in [
            BlockKind::Project,
            BlockKind::Task,
            BlockKind::Note,
            BlockKind::Section,
            BlockKind::Code,
            BlockKind::Url,
            BlockKind::Divider,
        ] {
            assert!(
                rows.iter().any(|row| row.kind == kind),
                "demo lacks a {kind:?}"
            );
        }
        assert!(rows.iter().any(|row| row.depth >= 2), "demo lacks nesting");
        assert_eq!(
            rows.first().map(|row| row.lines.clone()),
            Some(vec!["Trip to Lisbon".to_owned()])
        );
        assert!(
            rows.iter()
                .any(|row| row.kind == BlockKind::Task && row.done),
            "demo lacks a done task"
        );
        assert!(
            rows.iter()
                .any(|row| row.kind == BlockKind::Task && !row.done),
            "demo lacks a task to check off"
        );
        let section_after_task = rows.iter().enumerate().any(|(at, row)| {
            row.kind == BlockKind::Section
                && rows[..at]
                    .iter()
                    .rev()
                    .find(|above| above.depth <= row.depth)
                    .is_some_and(|above| above.depth == row.depth && above.kind == BlockKind::Task)
        });
        assert!(
            section_after_task,
            "demo lacks a list with tasks above a section, where `S` adds one"
        );
        let document = state.document();
        let tagged = document
            .descendants(document.root())
            .filter_map(|(id, _)| document.get(id))
            .filter(|item| !item.tags().is_empty())
            .count();
        assert!(tagged >= 3, "demo lacks tagged lines");
        assert!(
            rows.iter()
                .flat_map(|row| &row.lines)
                .all(|line| !line.contains('@')),
            "demo shows a tag"
        );
        assert!(
            rows.iter()
                .any(|row| row.kind == BlockKind::Note && row.lines.len() > 1),
            "demo lacks a multi-line note"
        );
        let hidden = document
            .descendants(document.root())
            .filter_map(|(id, _)| document.get(id))
            .any(|item| item.tags().iter().any(|tag| tag.name == "hidden"));
        assert!(hidden, "demo lacks a special node");
    }
}
