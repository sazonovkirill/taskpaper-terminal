//! The Logbook view: the blocks completed in the last 7 days, grouped by the day of completion
//! ([navigation.md §7.1](../../../docs/specs/navigation.md#71-other-special-views-fr-tag-07-d-80),
//! FR-TAG-11, D-84).

use std::cmp::Reverse;

use jiff::civil::Date;

use super::{
    Row, State,
    pane::Pane,
    special_view::{SpecialView, ViewGroup, under_headings},
};

/// How many days before today the Logbook reaches back: with today, the last 7 days (D-84).
const DAYS_BACK: i32 = 6;

/// Whether the Logbook lists the block of `row` on `today`: its `@done` date is today or one of
/// the six days before (D-84).
pub(super) fn in_logbook(row: &Row, today: Date) -> bool {
    row.done_date
        .is_some_and(|date| (0..=DAYS_BACK).contains(&(today - date).get_days()))
}

impl State {
    /// The Logbook's entries in `pane`: every block whose `@done` date is today or one of the
    /// six days before, inside done blocks too and whatever the pane's show-completed setting,
    /// grouped by that day, newest first, in document order within a day (D-84).
    pub(super) fn logbook_rows(&self, pane: &Pane) -> Vec<Row> {
        let mut entries: Vec<_> = self
            .matching_rows_with_done(pane, |row| self.lists(SpecialView::Logbook, row))
            .into_iter()
            .filter_map(|row| Some((row.done_date?, row)))
            .collect();
        entries.sort_by_key(|&(date, _)| Reverse(date));
        under_headings(
            entries
                .into_iter()
                .map(|(date, row)| (ViewGroup::Day(date), row)),
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
        document::{blocks, format},
    };

    /// The local date of these tests, a Monday.
    const TODAY: Date = date(2026, 10, 5);

    /// Blocks completed on days in and out of the last 7, at several depths, among open blocks, a
    /// bare `@done`, a done date that is not one and a special node.
    const OUTLINE: &str = "\
Home: @done(2026-10-03)
\t- Water plants @done(2026-10-05)
\t- Paint the hall
\tErrands:
\t\t- Post office @done(2026-09-29)
\t\t- Bank @done(2026-09-28)
\t\t\tC @hidden @done(2026-10-05)
\t- Call Ann @done
- Plan the week @done(2026-10-05)
- Report @done(soon)
- Slides @done(2026-10-06)
";

    /// `text` opened on `today` in the Logbook.
    fn logbook_on(text: &str, today: Date) -> State {
        let mut state = State::new(format::parse(text), None);
        state.update(Message::Tick {
            now: Duration::ZERO,
            today,
        });
        state.update(Message::GoTo(Location::View(SpecialView::Logbook)));
        assert_eq!(state.location(), Location::View(SpecialView::Logbook));
        state
    }

    /// `text` opened on [`TODAY`] in the Logbook.
    fn logbook(text: &str) -> State {
        logbook_on(text, TODAY)
    }

    /// Each row's text and path, joined as `path › text`, after the day of the heading it starts,
    /// with a `*` for the first heading.
    fn listed(state: &State) -> Vec<String> {
        state
            .rows()
            .iter()
            .map(|row| {
                assert_eq!(row.depth, 0, "entries are flat");
                let heading = row.heading.map_or_default(|Heading { group, first }| {
                    let day = blocks::block(state.document(), row.id)
                        .and_then(|block| block.done_date())
                        .expect("every entry has a done date");
                    assert_eq!(group, ViewGroup::Day(day));
                    format!("[{day}{}] ", if first { "*" } else { "" })
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

    // FR-TAG-11, D-84, D-80, FR-SPC-01
    #[test]
    fn lists_the_last_seven_days_completions_newest_day_first_in_document_order_within_a_day() {
        let state = logbook(OUTLINE);
        assert_eq!(
            listed(&state),
            [
                "[2026-10-05*] Home › Water plants",
                "Plan the week",
                "[2026-10-03] Home",
                "[2026-09-29] Home › Errands › Post office",
            ]
        );
        assert_eq!(state.breadcrumbs(), Vec::<String>::new());
        assert_eq!(focused(&state), "Water plants");
        assert!(state.rows()[0].inside_done, "inside the done Home");
    }

    // D-84: the list does not depend on the pane's show-completed setting.
    #[test]
    fn done_blocks_are_listed_whatever_the_show_completed_setting() {
        let mut state = logbook(OUTLINE);
        let shown = listed(&state);
        state.update(Message::ToggleShowDone);
        assert_eq!(listed(&state), shown);
        state.update(Message::ToggleShowDone);
        assert_eq!(listed(&state), shown);
    }

    // D-84, D-80: the seven days follow the date, as at local midnight.
    #[test]
    fn the_days_follow_the_date() {
        let mut state = logbook(OUTLINE);
        assert_eq!(state.rows().len(), 4);
        state.update(Message::Tick {
            now: Duration::from_secs(1),
            today: date(2026, 10, 6),
        });
        assert_eq!(
            listed(&state),
            [
                "[2026-10-06*] Slides",
                "[2026-10-05] Home › Water plants",
                "Plan the week",
                "[2026-10-03] Home",
            ]
        );
        let empty = logbook_on(OUTLINE, date(2026, 10, 13));
        assert_eq!(listed(&empty), Vec::<String>::new());
        let start = logbook_on(OUTLINE, date(2026, 9, 29));
        assert_eq!(
            listed(&start),
            [
                "[2026-09-29*] Home › Errands › Post office",
                "[2026-09-28] Home › Errands › Bank",
            ],
            "nothing from later days"
        );
    }

    // D-84, FR-DONE-01: a block uncompleted leaves the view.
    #[test]
    fn the_list_follows_completions() {
        let mut state = logbook("- a @done(2026-10-05)\n- b @done(2026-10-04)\n");
        state.update(Message::ToggleDone { today: TODAY });
        assert_eq!(listed(&state), ["[2026-10-04*] b"]);
        let mut empty = logbook("- a\n");
        assert_eq!(listed(&empty), Vec::<String>::new());
        empty.update(Message::FocusNext);
        assert_eq!(empty.focus(), None);
    }

    // D-80, D-57: only Today can be reordered.
    #[test]
    fn move_up_and_down_do_nothing() {
        let mut state = logbook("- a @done(2026-10-05)\n- b @done(2026-10-05)\n");
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
        let mut state = logbook(OUTLINE);
        state.update(Message::FocusLast);
        state.update(Message::ZoomIn);
        assert_eq!(state.breadcrumbs(), ["Home", "Errands"]);
        assert_eq!(focused(&state), "Post office");
        state.update(Message::Back);
        assert_eq!(state.location(), Location::View(SpecialView::Logbook));
    }
}
