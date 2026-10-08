//! The Overdue view: the open blocks whose due date has passed, grouped by due date
//! ([navigation.md §7.1](../../../docs/specs/navigation.md#71-other-special-views-fr-tag-07-d-80),
//! FR-TAG-09, D-82).

use super::{
    Row, State,
    pane::Pane,
    special_view::{SpecialView, ViewGroup, under_headings},
};

impl State {
    /// The Overdue view's entries in `pane`, as [`State::matching_rows`] gives them: every block
    /// whose due date is before today and that is neither done nor inside a done block, whatever
    /// `pane`'s show-completed setting, grouped by due date, oldest first, in document order within
    /// a date (D-82).
    pub(super) fn overdue_rows(&self, pane: &Pane) -> Vec<Row> {
        let mut entries: Vec<_> = self
            .matching_rows(pane, |row| self.lists(SpecialView::Overdue, row))
            .into_iter()
            .filter(|row| row.overdue(self.today))
            .filter_map(|row| Some((row.due?, row)))
            .collect();
        entries.sort_by_key(|(due, _)| *due);
        under_headings(
            entries
                .into_iter()
                .map(|(due, row)| (ViewGroup::Day(due), row)),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use jiff::civil::{Date, date};

    use super::*;
    use crate::{
        app::{Heading, Location, Message, SelectionChange, SpecialView},
        document::{DateChange, DateTag, format},
    };

    /// The local date of these tests, a Monday.
    const TODAY: Date = date(2026, 10, 5);

    /// Overdue blocks at several depths, out of date order, among blocks due today or later,
    /// blocks only started earlier, a due tag that is not a date, a special node, done blocks and
    /// an open block inside a done one.
    const OUTLINE: &str = "\
Home:
\t- Water plants @due(2026-10-04)
\t- Paint the hall @due(2026-10-05)
\tErrands:
\t\t- Post office @due(2026-10-01)
\t\t- Bank @due(2026-10-04) @done(2026-10-04)
\t\t\tC @hidden @due(2026-10-01)
\t- Call Ann @due(soon)
\t- Sort mail @start(2026-10-01)
Work: @done
\t- Report @due(2026-10-02)
Plan: @due(2026-10-01)
\t- Book flights @due(2026-10-06)
";

    /// `text` opened on [`TODAY`] in the Overdue view.
    fn overdue(text: &str) -> State {
        let mut state = State::new(format::parse(text), None);
        tick(&mut state, TODAY);
        state.update(Message::GoTo(Location::View(SpecialView::Overdue)));
        assert_eq!(state.location(), Location::View(SpecialView::Overdue));
        state
    }

    /// Tells `state` the local date is `today`.
    fn tick(state: &mut State, today: Date) {
        state.update(Message::Tick {
            now: Duration::ZERO,
            today,
        });
    }

    /// Each row's text and path, joined as `path › text`, after the date of the heading it
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

    // FR-TAG-09, D-82, D-80, FR-SPC-01
    #[test]
    fn lists_open_overdue_blocks_by_due_date_oldest_first_in_document_order_within_a_date() {
        let state = overdue(OUTLINE);
        assert_eq!(
            listed(&state),
            [
                "[Day(2026-10-01)*] Home › Errands › Post office",
                "Plan",
                "[Day(2026-10-04)] Home › Water plants",
            ]
        );
        assert_eq!(state.breadcrumbs(), Vec::<String>::new());
        assert_eq!(focused(&state), "Post office");
    }

    // D-82: done blocks never show, whatever the show-completed setting, and hiding them hides
    // nothing more.
    #[test]
    fn the_list_is_the_same_whether_completed_blocks_show_or_not() {
        let mut state = overdue(OUTLINE);
        let shown = listed(&state);
        state.update(Message::ToggleShowDone);
        assert_eq!(listed(&state), shown);
    }

    // D-82, D-80: the list follows the local date and changes to due dates and completion.
    #[test]
    fn the_list_follows_the_date_and_the_blocks() {
        let mut state = overdue("- a @due(2026-10-05)\n- b @due(2026-10-04)\n");
        assert_eq!(listed(&state), ["[Day(2026-10-04)*] b"]);
        tick(&mut state, date(2026, 10, 6));
        assert_eq!(
            listed(&state),
            ["[Day(2026-10-04)*] b", "[Day(2026-10-05)] a"]
        );
        state.update(Message::ChangeDate {
            tag: DateTag::Due,
            change: DateChange::Today,
            today: date(2026, 10, 6),
        });
        assert_eq!(listed(&state), ["[Day(2026-10-05)*] a"]);
        state.update(Message::ToggleDone {
            today: date(2026, 10, 6),
        });
        assert_eq!(listed(&state), Vec::<String>::new());
        state.update(Message::FocusNext);
        assert_eq!(state.focus(), None);
    }

    // D-80, D-57: only Today can be reordered.
    #[test]
    fn move_up_and_down_do_nothing() {
        let mut state = overdue("- a @due(2026-10-01)\n- b @due(2026-10-01)\n");
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
        let mut state = overdue(OUTLINE);
        state.update(Message::ZoomIn);
        assert_eq!(state.breadcrumbs(), ["Home", "Errands"]);
        assert_eq!(focused(&state), "Post office");
        state.update(Message::Back);
        assert_eq!(state.location(), Location::View(SpecialView::Overdue));
    }
}
