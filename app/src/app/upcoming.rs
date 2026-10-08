//! The Upcoming view: the blocks started or due in the 7 days after today, grouped by day
//! ([navigation.md §7.1](../../../docs/specs/navigation.md#71-other-special-views-fr-tag-07-d-80),
//! FR-TAG-08, D-81).

use jiff::civil::Date;

use super::{
    Row, State,
    pane::Pane,
    special_view::{SpecialView, ViewGroup, under_headings},
};

/// The day the Upcoming view lists a block with the start date `start` and due date `due` under on
/// `today`: the earliest of them that falls from tomorrow to a week from today, if one does (D-81).
pub(super) fn upcoming_day(start: Option<Date>, due: Option<Date>, today: Date) -> Option<Date> {
    [start, due]
        .into_iter()
        .flatten()
        .filter(|date| (1..=7).contains(&(*date - today).get_days()))
        .min()
}

impl State {
    /// The Upcoming view's entries in `pane`, as [`State::matching_rows`] gives them, grouped by
    /// the day each is listed under, earliest first, in document order within a day (D-81).
    pub(super) fn upcoming_rows(&self, pane: &Pane) -> Vec<Row> {
        let mut entries: Vec<_> = self
            .matching_rows(pane, SpecialView::Upcoming)
            .into_iter()
            .filter_map(|row| Some((upcoming_day(row.start, row.due, self.today)?, row)))
            .collect();
        entries.sort_by_key(|(day, _)| *day);
        under_headings(
            entries
                .into_iter()
                .map(|(day, row)| (ViewGroup::Day(day), row)),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use jiff::civil::date;

    use super::*;
    use crate::{
        app::{Heading, Location, Message, SelectionChange, SpecialView},
        document::{DateChange, DateTag, format},
    };

    /// The local date of these tests, a Monday.
    const TODAY: Date = date(2026, 10, 5);

    /// Blocks dated in the coming week at several depths, out of date order, among blocks that
    /// are not: dated today, earlier and later, undated, starred or tagged `@today`, a special
    /// node and a done project.
    const OUTLINE: &str = "\
Home:
\t- Water plants @due(2026-10-08)
\t- Paint the hall @start(2026-10-05)
\tErrands:
\t\t- Post office @start(2026-10-06) @due(2026-10-09)
\t\t- Bank @start(2026-10-01) @due(2026-10-12)
\t\t\tC @hidden @due(2026-10-07)
\t- Call Ann @due(2026-10-13)
\t- Pay rent @star @today
Work: @done
\t- Report @due(2026-10-08)
- Plan the week @start(2026-10-06)
- Read @due(soon)
";

    /// `text` opened on `today` in the Upcoming view.
    fn upcoming_on(text: &str, today: Date) -> State {
        let mut state = State::new(format::parse(text), None);
        state.update(Message::Tick {
            now: Duration::ZERO,
            today,
        });
        state.update(Message::GoTo(Location::View(SpecialView::Upcoming)));
        assert_eq!(state.location(), Location::View(SpecialView::Upcoming));
        state
    }

    /// `text` opened on [`TODAY`] in the Upcoming view.
    fn upcoming(text: &str) -> State {
        upcoming_on(text, TODAY)
    }

    /// Each row's text and path, joined as `path › text`, after the group of the heading it
    /// starts, with a `*` for the first heading.
    fn listed(state: &State) -> Vec<String> {
        state
            .rows()
            .iter()
            .map(|row| {
                assert_eq!(row.depth, 0, "entries are flat");
                let heading = row.heading.map_or_default(|Heading { group, first }| {
                    format!("[{group:?}{}] ", if first { "*" } else { "" })
                });
                let path = row
                    .path
                    .iter()
                    .map(|segment| &**segment)
                    .chain([&*row.text])
                    .collect::<Vec<_>>()
                    .join(" › ");
                heading + &path
            })
            .collect()
    }

    /// The display text of the focused block.
    fn focused(state: &State) -> String {
        state
            .focus()
            .and_then(|id| state.document().get(id))
            .map(crate::document::Item::display_text)
            .map(str::to_owned)
            .unwrap_or_default()
    }

    // FR-TAG-08, D-81, D-80, FR-SPC-01
    #[test]
    fn lists_blocks_dated_in_the_coming_week_under_their_earliest_day_in_date_order() {
        let state = upcoming(OUTLINE);
        assert_eq!(
            listed(&state),
            [
                "[Day(2026-10-06)*] Home › Errands › Post office",
                "Plan the week",
                "[Day(2026-10-08)] Home › Water plants",
                "Work › Report",
                "[Day(2026-10-12)] Home › Errands › Bank",
            ]
        );
        assert_eq!(state.breadcrumbs(), Vec::<String>::new());
        assert_eq!(focused(&state), "Post office");
    }

    // D-81: the range runs from tomorrow to a week from today, and a block dated twice in it is
    // listed once, under the earlier date, whichever tag holds it.
    #[test]
    fn the_week_runs_from_tomorrow_to_seven_days_on_and_each_block_is_listed_once() {
        let state = upcoming(
            "- a @due(2026-10-12)\n- b @start(2026-10-12)\n- c @due(2026-10-05)\n\
             - d @start(2026-10-13)\n- e @start(2026-10-09) @due(2026-10-07)\n",
        );
        assert_eq!(
            listed(&state),
            ["[Day(2026-10-07)*] e", "[Day(2026-10-12)] a", "b"]
        );
    }

    // D-81: `@today` and `@star` alone list nothing, nor does a date that is no date.
    #[test]
    fn today_tags_stars_and_undated_blocks_are_not_listed() {
        let state = upcoming("- a @today\n- b @star\n- c @due(soon)\n- d\n");
        assert_eq!(listed(&state), Vec::<String>::new());
    }

    // FR-DONE-02, D-81
    #[test]
    fn done_blocks_follow_the_show_completed_setting() {
        let mut state = upcoming(
            "- a @due(2026-10-06) @done\n- b @due(2026-10-06)\nP: @done\n\t- c @start(2026-10-07)\n",
        );
        assert_eq!(
            listed(&state),
            ["[Day(2026-10-06)*] a", "b", "[Day(2026-10-07)] P › c"]
        );
        state.update(Message::ToggleShowDone);
        assert_eq!(listed(&state), ["[Day(2026-10-06)*] b"]);
    }

    // D-81, D-80: the list follows the local date, also at midnight after it was drawn.
    #[test]
    fn the_list_follows_the_local_date() {
        let mut state =
            upcoming("- a @due(2026-10-06)\n- b @due(2026-10-12)\n- c @due(2026-10-13)\n");
        assert_eq!(
            listed(&state),
            ["[Day(2026-10-06)*] a", "[Day(2026-10-12)] b"]
        );
        state.update(Message::Tick {
            now: Duration::from_secs(1),
            today: date(2026, 10, 6),
        });
        assert_eq!(
            listed(&state),
            ["[Day(2026-10-12)*] b", "[Day(2026-10-13)] c"]
        );
    }

    // D-81, FR-TAG-03: a block whose date moves goes to that day's group, and one whose date
    // leaves the week leaves the view.
    #[test]
    fn the_list_follows_date_changes() {
        let mut state = upcoming("- a @due(2026-10-06)\n- b @due(2026-10-06)\n");
        state.update(Message::ChangeDate {
            tag: DateTag::Due,
            change: DateChange::Later,
            today: TODAY,
        });
        assert_eq!(
            listed(&state),
            ["[Day(2026-10-06)*] b", "[Day(2026-10-07)] a"]
        );
        assert_eq!(focused(&state), "a");
        state.update(Message::ChangeDate {
            tag: DateTag::Due,
            change: DateChange::Today,
            today: TODAY,
        });
        assert_eq!(listed(&state), ["[Day(2026-10-06)*] b"]);
    }

    // D-80, D-57: only Today can be reordered.
    #[test]
    fn move_up_and_down_do_nothing() {
        let mut state = upcoming("- a @due(2026-10-06)\n- b @due(2026-10-06)\n");
        state.update(Message::FocusNext);
        state.update(Message::Select(SelectionChange::ExtendUp));
        let before = (listed(&state), format::serialize(state.document()));
        for message in [Message::MoveUp, Message::MoveDown] {
            assert_eq!(state.update(message.clone()), [], "{message:?}");
            assert_eq!(
                (listed(&state), format::serialize(state.document())),
                before,
                "{message:?}"
            );
        }
    }

    // FR-NAV-03, D-80: zooming in on an entry goes to it in the outline.
    #[test]
    fn zooming_in_on_an_entry_goes_to_it_in_the_outline() {
        let mut state = upcoming(OUTLINE);
        state.update(Message::FocusLast);
        state.update(Message::ZoomIn);
        assert_eq!(state.breadcrumbs(), ["Home", "Errands"]);
        assert_eq!(focused(&state), "Bank");
        state.update(Message::Back);
        assert_eq!(state.location(), Location::View(SpecialView::Upcoming));
    }
}
