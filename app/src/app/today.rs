//! The Today view: which blocks it lists, in the order the user gave them
//! ([navigation.md §7](../../../docs/specs/navigation.md#7-today-view-fr-tag-04-d-7), FR-TAG-04,
//! D-7, D-57).

use jiff::civil::Date;

use super::{Row, SpecialView, State, pane::Pane};
use crate::document::BlockKind;

/// Whether the Today view lists the block of `row` on `today`: it has a start or due date on or
/// before `today`, or, when `starred` says to list starred blocks, `@star` (FR-TAG-04, D-88), or
/// it is a task `in_today_project`, inside the TODAY project (D-102).
pub(super) fn for_today(row: &Row, today: Date, starred: bool, in_today_project: bool) -> bool {
    starred && row.starred
        || in_today_project && row.kind == BlockKind::Task
        || [row.start, row.due]
            .into_iter()
            .flatten()
            .any(|date| date <= today)
}

impl State {
    /// The Today view's entries in `pane`, as [`State::matching_rows`] gives them, in the order
    /// the user gave them, else in document order (D-57).
    pub(super) fn today_rows(&self, pane: &Pane) -> Vec<Row> {
        let mut rows = self.matching_rows(pane, SpecialView::Today);
        self.today_order.arrange(&mut rows, |row| row.id);
        rows
    }

    /// Whether the Today view lists starred blocks besides dated ones (`today_starred`, D-88).
    #[must_use]
    pub fn today_starred(&self) -> bool {
        self.today_starred
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, time::Duration};

    use jiff::civil::date;

    use super::*;
    use crate::{
        app::{
            Configuration, Effect, Location, Menu, Message, Mode, NewBlock, PaneChange, Paste,
            SelectionChange, SpecialView, TextEdit,
        },
        document::{DateChange, DateTag, Flag, format},
    };

    /// The local date of these tests.
    const TODAY: Date = date(2026, 10, 5);

    /// Blocks for today at several depths among blocks that are not: dated earlier, today and
    /// later, a date that is not one, a special node and a done project.
    const OUTLINE: &str = "\
Home:
\t- Water plants @star
\t- Paint the **hall** @due(2026-10-09)
\tErrands:
\t\t- Post office @start(2026-10-05)
\t\t- Bank @due(2026-10-01)
\t\t\tC @hidden @star
\t- Call Ann @due(soon)
Work: @done
\t- Report @due(2026-10-02)
\t- Slides
- Plan the week @start(2026-10-06)
";

    /// `text` opened on [`TODAY`] in the Today view.
    fn today(text: &str) -> State {
        let mut state = State::new(format::parse(text), None);
        state.update(Message::Tick {
            now: Duration::ZERO,
            today: TODAY,
        });
        state.go_to(Location::View(SpecialView::Today));
        assert_eq!(state.location(), Location::View(SpecialView::Today));
        state
    }

    /// Each row's text and path, joined as `path › text`.
    fn listed(state: &State) -> Vec<String> {
        state
            .rows()
            .iter()
            .map(|row| {
                assert_eq!(row.depth, 0, "entries are flat");
                assert!(!row.title);
                row.path
                    .iter()
                    .map(|segment| &**segment)
                    .chain([&*row.text])
                    .collect::<Vec<_>>()
                    .join(" › ")
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

    /// Moves the focus down `rows` rows.
    fn down(state: &mut State, rows: usize) {
        for _ in 0..rows {
            state.update(Message::FocusNext);
        }
    }

    // FR-TAG-04, D-7, FR-SPC-01
    #[test]
    fn lists_starred_blocks_or_dated_up_to_today_flat_with_their_paths() {
        let state = today(OUTLINE);
        assert_eq!(
            listed(&state),
            [
                "Home › Water plants",
                "Home › Errands › Post office",
                "Home › Errands › Bank",
                "Work › Report",
            ]
        );
        assert_eq!(state.breadcrumbs(), Vec::<String>::new());
        assert_eq!(state.zoom(), None);
        assert_eq!(focused(&state), "Water plants");
    }

    // FR-TAG-04, D-88
    #[test]
    fn with_the_setting_off_only_dated_blocks_are_listed_and_today_tags_never_are() {
        let mut state = today("- a @star\n- b @star @due(2026-10-05)\n- c @today\n");
        assert!(state.today_starred(), "on by default");
        assert_eq!(listed(&state), ["a", "b"]);
        state.update(Message::Configure(Configuration::TodayStarred(false)));
        assert!(!state.today_starred());
        assert_eq!(listed(&state), ["b"]);
        state.update(Message::Configure(Configuration::TodayStarred(true)));
        assert_eq!(listed(&state), ["a", "b"]);
    }

    // FR-TAG-04, D-102
    #[test]
    fn tasks_inside_the_today_project_are_listed_while_its_setting_is_on() {
        let text = "\
- a @star
TODAY:
\t- call
\tLater:
\t\t- write
\t\tnote
\t- dated @due(2026-10-05)
Home:
\t- other
\tTODAY:
\t\t- nested
";
        let mut state = today(text);
        assert!(state.special_today(), "on by default");
        assert_eq!(
            listed(&state),
            [
                "a",
                "TODAY › call",
                "TODAY › Later › write",
                "TODAY › dated"
            ],
            "tasks at any depth, not the project, its projects or notes, nor a TODAY below the \
             top level"
        );
        state.update(Message::Configure(Configuration::SpecialToday(false)));
        assert_eq!(listed(&state), ["a", "TODAY › dated"]);
        state.update(Message::Configure(Configuration::SpecialToday(true)));
        assert_eq!(listed(&state).len(), 4, "the list follows the setting");
    }

    // FR-TAG-04, D-102
    #[test]
    fn only_the_first_shown_today_project_counts_and_an_emoji_may_name_it() {
        let state = today("☀️ TODAY:\n\t- a\nTODAY:\n\t- b\n");
        assert_eq!(listed(&state), ["☀️ TODAY › a"]);
        let mut done = today("TODAY: @done\n\t- a\nTODAY:\n\t- b\n");
        assert_eq!(listed(&done), ["TODAY › a"], "a done project still shows");
        done.update(Message::ToggleShowDone);
        assert_eq!(listed(&done), ["TODAY › b"]);
    }

    // D-98, D-102: the TODAY project is the first one the Today pane shows, wherever the keys are.
    #[test]
    fn each_today_pane_finds_the_today_project_among_the_blocks_it_shows() {
        let mut state = today("TODAY: @done\n\t- a\nTODAY:\n\t- b\n");
        state.update(Message::Pane(PaneChange::OpenLocation));
        state.go_to(Location::Outline(None));
        state.update(Message::ToggleShowDone);
        // A change, so the Today pane's entries are built again while the other pane has the keys.
        state.update(Message::SetColor(Some(crate::document::TagColor::Red)));
        let first = |state: &State| -> Vec<String> {
            state.panes()[0]
                .rows
                .iter()
                .map(|row| format!("{} › {}", row.path.join(" › "), row.text))
                .collect()
        };
        assert_eq!(first(&state), ["TODAY › a"]);
        state.update(Message::Pane(PaneChange::Focus(0)));
        assert_eq!(first(&state), ["TODAY › a"]);
    }

    // D-7, D-40
    #[test]
    fn entries_carry_their_block_and_whether_a_block_above_is_done() {
        let state = today(OUTLINE);
        let rows = state.rows();
        let report = rows.last().expect("an entry");
        assert!(report.inside_done && !report.done);
        assert_eq!(report.due, Some(date(2026, 10, 2)));
        let water = rows.first().expect("an entry");
        assert!(water.starred && !water.inside_done);
        let mut folded = today("- a @star\n\t- b\n");
        folded.folds.insert(folded.focus().expect("a focus"));
        assert_eq!(folded.rows()[0].hidden_tasks, None, "entries do not fold");
    }

    // FR-DONE-02, D-7
    #[test]
    fn hidden_completed_blocks_leave_the_list_with_everything_inside_them() {
        let mut state = today("- a @star @done\n- b @star\nP: @done\n\t- c @star\n");
        assert_eq!(listed(&state), ["a", "b", "P › c"]);
        state.update(Message::ToggleShowDone);
        assert_eq!(listed(&state), ["b"]);
        state.update(Message::ToggleShowDone);
        assert_eq!(listed(&state), ["a", "b", "P › c"]);
    }

    // D-7
    #[test]
    fn the_list_follows_the_local_date() {
        let mut state = today(OUTLINE);
        state.update(Message::Tick {
            now: Duration::from_secs(1),
            today: date(2026, 10, 6),
        });
        assert_eq!(
            listed(&state).last().map(String::as_str),
            Some("Plan the week")
        );
        assert_eq!(listed(&state).len(), 5);
        state.update(Message::Tick {
            now: Duration::from_secs(2),
            today: date(2026, 10, 1),
        });
        assert_eq!(
            listed(&state),
            ["Home › Water plants", "Home › Errands › Bank"]
        );
    }

    // FR-TAG-04, FR-DONE-01, FR-TAG-06, D-7
    #[test]
    fn block_commands_act_on_the_entries_and_the_list_follows_the_changes() {
        let mut state = today(OUTLINE);
        state.update(Message::ToggleDone { today: TODAY });
        assert!(
            state.rows()[0].done,
            "a done entry stays while completed blocks show"
        );
        down(&mut state, 1);
        state.update(Message::ChangeDate {
            tag: DateTag::Start,
            change: DateChange::Tomorrow,
            today: TODAY,
        });
        assert_eq!(listed(&state).len(), 3, "the post office starts tomorrow");
        assert_eq!(focused(&state), "Bank", "the next entry");
        state.update(Message::ToggleFlag(Flag::Star));
        assert!(state.rows()[1].starred);
        state.update(Message::SetColor(Some(crate::document::TagColor::Red)));
        assert_eq!(state.rows()[1].color, Some(crate::document::TagColor::Red));
        state.update(Message::FocusLast);
        state.update(Message::ClearDate(DateTag::Due));
        assert_eq!(listed(&state).len(), 2);
        assert_eq!(
            focused(&state),
            "Bank",
            "the previous entry when none is next"
        );
    }

    // FR-MODE-01, D-7
    #[test]
    fn entries_are_edited_in_place_and_enter_only_stops_editing() {
        let mut state = today(OUTLINE);
        state.update(Message::EditAtEnd);
        state.update(Message::Text(TextEdit::Insert('!')));
        state.update(Message::NewNext);
        assert_eq!(state.mode(), Mode::Navigate);
        assert_eq!(listed(&state)[0], "Home › Water plants!");
        state.update(Message::EditAtEnd);
        state.update(Message::EditNext);
        assert_eq!(focused(&state), "Post office", "the next entry");
        assert_eq!(state.mode(), Mode::Edit);
    }

    // D-7, navigation §7
    #[test]
    fn intents_that_need_the_outlines_tree_do_nothing() {
        let mut state = today("A:\n\t- a @star\n\t\t- child\n\t- b @star\n- c\n");
        let before = format::serialize(state.document());
        for message in [
            Message::FocusParent,
            Message::FocusNextSibling,
            Message::FocusPreviousSibling,
            Message::FocusLastChild,
            Message::Collapse,
            Message::Expand,
            Message::ToggleFold,
            Message::ToggleSubtree,
            Message::ExpandLevel,
            Message::CollapseLevel,
            Message::ExpandAll,
            Message::CollapseAll,
            Message::New(NewBlock::Below, None),
            Message::Group,
            Message::Indent,
            Message::Outdent,
            Message::Paste(Paste::Below),
            Message::Pasted {
                text: "- x @star".to_owned(),
                at: Paste::Below,
            },
            Message::ZoomOut,
        ] {
            assert_eq!(state.update(message.clone()), [], "{message:?}");
            assert_eq!(focused(&state), "a", "{message:?}");
            assert_eq!(*state.folds, HashSet::new(), "{message:?}");
            assert_eq!(state.mode(), Mode::Navigate, "{message:?}");
            assert_eq!(format::serialize(state.document()), before, "{message:?}");
            assert_eq!(
                state.location(),
                Location::View(SpecialView::Today),
                "{message:?}"
            );
        }
    }

    // FR-OPS-06, D-7
    #[test]
    fn pasting_while_editing_an_entry_pastes_into_its_text() {
        let mut state = today("- a @star\n");
        state.update(Message::EditAtEnd);
        assert_eq!(
            state.update(Message::Paste(Paste::Below)),
            [Effect::ReadClipboard(Paste::Below)]
        );
        state.update(Message::Pasted {
            text: "b".to_owned(),
            at: Paste::Below,
        });
        state.update(Message::StopEditing);
        assert_eq!(listed(&state), ["ab"]);
    }

    // FR-OPS-06, FR-OPS-01, D-7
    #[test]
    fn selection_delete_and_duplicate_work_on_entries() {
        let mut state = today("- a @star\n- x\n- b @star\n- c @star\n");
        state.update(Message::Select(SelectionChange::ExtendDown));
        state.update(Message::ToggleFlag(Flag::Star));
        assert_eq!(listed(&state), ["c"]);
        assert_eq!(focused(&state), "c");
        state.update(Message::Duplicate);
        assert_eq!(listed(&state), ["c", "c"]);
        state.update(Message::Delete);
        state.update(Message::Delete);
        assert_eq!(listed(&state), Vec::<String>::new());
        assert_eq!(state.focus(), None);
        state.update(Message::Undo);
        assert_eq!(listed(&state), ["c"]);
        assert_eq!(focused(&state), "c");
    }

    // FR-OPS-05, D-7
    #[test]
    fn move_to_moves_an_entry_and_the_list_shows_its_new_path() {
        let mut state = today("A:\n\t- a @star\nB:\n");
        state.update(Message::OpenMoveTo);
        state.update(Message::Menu(
            Menu::MoveTo,
            crate::app::MenuInput::Type('B'),
        ));
        state.update(Message::Menu(Menu::MoveTo, crate::app::MenuInput::Choose));
        assert_eq!(state.location(), Location::View(SpecialView::Today));
        assert_eq!(listed(&state), ["B › a"]);
    }

    // FR-NAV-03, D-7, navigation §7
    #[test]
    fn zooming_in_on_an_entry_goes_to_it_in_the_outline() {
        let mut state = today(OUTLINE);
        down(&mut state, 2);
        state.update(Message::ZoomIn);
        assert_eq!(state.location(), Location::Outline(state.zoom()));
        assert_eq!(state.breadcrumbs(), ["Home", "Errands"]);
        assert_eq!(focused(&state), "Bank");
        let mut top = today("- a @star\n- b\n");
        top.update(Message::ZoomIn);
        assert_eq!(
            top.location(),
            Location::Outline(None),
            "a top-level entry is at home"
        );
        assert_eq!(focused(&top), "a");
        let mut empty = today("- a\n");
        empty.update(Message::ZoomIn);
        assert_eq!(
            empty.location(),
            Location::View(SpecialView::Today),
            "no entry to go to"
        );
    }

    // D-64, navigation.md §7
    #[test]
    fn go_to_today_shows_the_today_view_as_a_step_of_the_history() {
        let mut state = State::new(format::parse(OUTLINE), None);
        state.update(Message::Tick {
            now: Duration::ZERO,
            today: TODAY,
        });
        state.update(Message::GoTo(Location::View(SpecialView::Today)));
        assert_eq!(state.location(), Location::View(SpecialView::Today));
        assert_eq!(focused(&state), "Water plants");
        state.update(Message::GoTo(Location::View(SpecialView::Today)));
        assert_eq!(
            state.pane.history.len(),
            2,
            "staying put is not a new entry"
        );
        state.update(Message::Back);
        assert_eq!(state.location(), Location::Outline(None));
    }

    // FR-NAV-04, D-7
    #[test]
    fn today_is_a_step_of_the_history() {
        let mut state = State::new(format::parse(OUTLINE), None);
        state.update(Message::Tick {
            now: Duration::ZERO,
            today: TODAY,
        });
        down(&mut state, 4);
        state.update(Message::ZoomIn);
        state.go_to(Location::View(SpecialView::Today));
        assert_eq!(
            focused(&state),
            "Post office",
            "the block zoomed out of is an entry"
        );
        state.go_to(Location::View(SpecialView::Today));
        assert_eq!(
            state.pane.history.len(),
            3,
            "staying put is not a new entry"
        );
        state.update(Message::FocusLast);
        state.update(Message::Back);
        assert_eq!(state.breadcrumbs(), ["Home", "Errands", "Post office"]);
        assert_eq!(
            focused(&state),
            "Post office",
            "the focused entry is not under the zoom"
        );
        state.update(Message::Forward);
        assert_eq!(state.location(), Location::View(SpecialView::Today));
        assert_eq!(focused(&state), "Post office", "the block zoomed out of");
        state.update(Message::FocusNext);
        state.update(Message::Back);
        state.update(Message::Back);
        assert_eq!(state.location(), Location::Outline(None));
        assert_eq!(focused(&state), "Post office", "the block zoomed out of");
        state.update(Message::Forward);
        state.update(Message::Forward);
        state.update(Message::FocusNext);
        state.update(Message::GoTo(Location::Outline(None)));
        assert_eq!(state.location(), Location::Outline(None));
        assert_eq!(focused(&state), "Bank", "the focused entry shows at home");
        state.update(Message::Back);
        assert_eq!(state.location(), Location::View(SpecialView::Today));
    }
}
