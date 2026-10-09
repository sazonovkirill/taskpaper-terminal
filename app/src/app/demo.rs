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

    // FR-FMT-03, D-27
    #[test]
    fn demo_has_markdown_headings_and_links_in_text_blocks() {
        let rows = State::new(format::parse(&text(TODAY)), None).rows();
        let texts: Vec<&str> = rows
            .iter()
            .filter(|row| row.kind == BlockKind::Text)
            .map(|row| &*row.text)
            .collect();
        assert!(
            texts.iter().any(|text| text.starts_with("## ")),
            "demo lacks a Markdown heading in a text block"
        );
        for (syntax, what) in [("](", "a Markdown link"), ("<https://", "a <url> link")] {
            assert!(
                texts.iter().any(|text| text.contains(syntax)),
                "demo lacks {what} in a text block"
            );
        }
    }

    // FR-DOC-05, D-27
    #[test]
    fn demo_is_a_canonical_outline_with_every_implemented_type() {
        let state = State::new(format::parse(&text(TODAY)), None);
        assert_canonical(state.document());
        let rows = state.rows();
        for kind in [BlockKind::Project, BlockKind::Task, BlockKind::Text] {
            assert!(
                rows.iter().any(|row| row.kind == kind),
                "demo lacks a {kind:?}"
            );
        }
        assert!(rows.iter().any(|row| row.section), "demo lacks a section");
        assert!(rows.iter().any(|row| row.divider), "demo lacks a divider");
        assert!(rows.iter().any(|row| row.depth >= 2), "demo lacks nesting");
        assert_eq!(rows.first().map(|row| &*row.text), Some("Trip to Lisbon"));
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
        assert!(
            rows.iter()
                .any(|row| row.kind == BlockKind::Project && row.done)
                && rows.iter().any(|row| row.inside_done && !row.done),
            "demo lacks a done project with open blocks inside it"
        );
        let starts = rows.iter().filter(|row| row.start.is_some()).count();
        assert!(starts >= 2, "demo lacks start dates");
        assert!(
            rows.iter()
                .any(|row| row.due.is_some_and(|due| due > TODAY)),
            "demo lacks a due date to come"
        );
        assert!(
            rows.iter()
                .any(|row| row.due.is_some_and(|due| (due - TODAY).get_days() >= 14)),
            "demo lacks a date far enough ahead to be counted in days (D-46)"
        );
        assert!(
            rows.iter().any(|row| row.overdue(TODAY)),
            "demo lacks an overdue task"
        );
        assert!(rows.iter().any(|row| row.starred), "demo lacks @star");
        assert!(rows.iter().any(|row| row.flag), "demo lacks @flag");
        let colors = rows.iter().filter_map(|row| row.color);
        assert!(colors.count() >= 2, "demo lacks color tags");
        for (syntax, what) in [("**", "bold"), ("`", "code"), ("https://", "a bare URL")] {
            assert!(
                rows.iter().any(|row| !row.url && row.text.contains(syntax)),
                "demo lacks {what} in a block's text"
            );
        }
        let document = state.document();
        assert!(
            document
                .descendants(document.root())
                .filter_map(|(id, _)| crate::document::blocks::block(document, id))
                .any(|block| block.bookmark()),
            "demo lacks a bookmark"
        );
        let tagged = document
            .descendants(document.root())
            .filter_map(|(id, _)| document.get(id))
            .filter(|item| !item.tags().is_empty())
            .count();
        assert!(tagged >= 3, "demo lacks tagged lines");
        assert!(
            rows.iter().all(|row| !row.text.contains('@')),
            "demo shows a tag"
        );
        let hidden = document
            .descendants(document.root())
            .filter_map(|(id, _)| document.get(id))
            .any(|item| item.tags().iter().any(|tag| tag.name == "hidden"));
        assert!(hidden, "demo lacks a special node");
    }

    // FR-DOC-05, FR-OPS-07
    #[test]
    fn demo_has_five_open_tasks_in_a_row_to_select() {
        let rows = State::new(format::parse(&text(TODAY)), None).rows();
        let open_task_run = rows.to_vec().windows(5).any(|run| {
            run.iter()
                .all(|row| row.kind == BlockKind::Task && !row.done && row.depth == run[0].depth)
        });
        assert!(
            open_task_run,
            "demo lacks five open tasks in a row to select and complete at once"
        );
    }

    // FR-BLK-06, FR-FMT-03, D-45
    #[test]
    fn demo_has_links_in_a_task_and_a_text_block_and_a_line_with_a_url() {
        let rows = State::new(format::parse(&text(TODAY)), None).rows();
        for kind in [BlockKind::Task, BlockKind::Text] {
            assert!(
                rows.iter()
                    .any(|row| { row.kind == kind && !row.url && row.text.contains("](") }),
                "demo lacks a Markdown link in a {kind:?}"
            );
        }
        assert!(rows.iter().any(|row| row.url), "demo lacks an @url line");
    }

    // editing.md §7
    #[test]
    fn demo_has_unsupported_tags_and_hidden_data_for_the_edit_node_dialog() {
        use crate::document::blocks;
        let state = State::new(format::parse(&text(TODAY)), None);
        let document = state.document();
        let blocks: Vec<_> = state
            .rows()
            .iter()
            .filter_map(|row| blocks::block(document, row.id))
            .collect();
        assert!(
            blocks.iter().any(|block| {
                let line = document.get(block.id()).unwrap();
                line.tags()
                    .iter()
                    .any(|tag| !blocks::supported(&tag.name, block.kind()))
            }),
            "demo lacks an unsupported tag for the dialog"
        );
        assert!(
            blocks
                .iter()
                .any(|block| !blocks::hidden_data(document, block.id(), &[])
                    .unwrap()
                    .is_empty()),
            "demo lacks hidden data for the dialog"
        );
    }

    // FR-DOC-05, FR-OPS-06
    #[test]
    fn demo_has_a_template_list_to_duplicate() {
        let rows = State::new(format::parse(&text(TODAY)), None).rows();
        let template = rows
            .iter()
            .position(|row| row.kind == BlockKind::Project && row.text.ends_with("template"))
            .expect("demo lacks a template list to duplicate");
        assert!(
            rows.get(template + 1).is_some_and(|row| row.depth == 1),
            "the template has no blocks"
        );
    }

    // FR-DOC-05, D-53
    #[test]
    fn demo_has_text_lines_that_point_to_the_creation_and_clipboard_keys() {
        let rows = State::new(format::parse(&text(TODAY)), None).rows();
        for (hint, what) in [
            ("then s, p, t or n", "the creation sequences"),
            ("copy it with y", "y and p"),
        ] {
            assert!(
                rows.iter()
                    .any(|row| row.kind == BlockKind::Text && row.text.contains(hint)),
                "demo lacks a hint at {what}"
            );
        }
    }

    // FR-DOC-05, FR-UI-11
    #[test]
    fn demo_has_a_text_line_that_points_to_the_landing_flash_and_its_setting() {
        let rows = State::new(format::parse(&text(TODAY)), None).rows();
        for (hint, what) in [
            ("the block focus lands on flashes", "the landing flash"),
            ("animations, autosave delay", "the animations setting"),
            (
                "its text eases into the done style",
                "the ease of a completed task",
            ),
        ] {
            assert!(
                rows.iter()
                    .any(|row| row.kind == BlockKind::Text && row.text.contains(hint)),
                "demo lacks a hint at {what}"
            );
        }
    }

    // FR-DOC-05, D-52
    #[test]
    fn demo_has_a_text_line_that_points_to_the_section_keys() {
        let rows = State::new(format::parse(&text(TODAY)), None).rows();
        assert!(
            rows.iter().any(|row| row.kind == BlockKind::Text
                && !row.section
                && row.text.contains("section with S")),
            "demo lacks a hint at the section keys"
        );
    }

    // FR-DOC-05, D-50
    #[test]
    fn demo_has_a_text_line_that_points_to_the_command_menu() {
        let rows = State::new(format::parse(&text(TODAY)), None).rows();
        assert!(
            rows.iter()
                .any(|row| row.kind == BlockKind::Text && row.text.contains("type / for")),
            "demo lacks a hint at the command menu"
        );
    }

    // FR-DOC-05, FR-KEY-05
    #[test]
    fn demo_has_a_text_line_that_points_to_the_mouse() {
        let rows = State::new(format::parse(&text(TODAY)), None).rows();
        assert!(
            rows.iter()
                .any(|row| row.kind == BlockKind::Text && row.text.contains("right-click")),
            "demo lacks a hint at the mouse"
        );
    }
}
