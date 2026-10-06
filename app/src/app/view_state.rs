//! A file's layout as it is kept between sessions: its panes in order, each with its location
//! and history, focused block, width and show-completed setting, which pane has the keys and
//! whether the sidebar shows, and the order the user gave the Today view's entries (D-57), with
//! blocks named by their text paths, which outlive the ids of one
//! session; restoring it drops what no longer matches. Folds are not kept (D-26)
//! ([file-io.md §5](../../../docs/specs/file-io.md#5-view-state-persistence), FR-UI-04, OQ-05).

use serde::{Deserialize, Serialize};

use super::{
    Location, Origin, State,
    pane::{Pane, landing},
    text_path::TextPath,
    today_order::TodayOrder,
};
use crate::document::{Document, ItemId};

/// The layout of the open file, for a frontend to keep with it and give back on the next launch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewState {
    /// The panes from left to right.
    panes: Vec<PaneState>,
    /// The position of the pane with the keys.
    focused: usize,
    /// Whether the sidebar shows.
    sidebar: bool,
    /// The blocks placed in the Today view's order, in that order; left out while there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    today: Vec<TextPath>,
}

/// One pane of a [`ViewState`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PaneState {
    /// Every location visited, oldest first.
    history: Vec<Place>,
    /// The position in `history` of the location the pane shows.
    at: usize,
    focus: Option<TextPath>,
    width: Option<u16>,
    show_completed: bool,
}

/// A [`Location`] as kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Place {
    Home,
    Zoom(TextPath),
    Today,
}

impl Place {
    /// How `location` in `document` is kept, or `None` when its zoom block is gone.
    fn of(document: &Document, location: Location) -> Option<Self> {
        match location {
            Location::Outline(None) => Some(Self::Home),
            Location::Outline(Some(zoom)) => TextPath::of(document, zoom).map(Self::Zoom),
            Location::Today => Some(Self::Today),
        }
    }

    /// The location this is in `document`, or `None` when its zoom block is not there.
    fn find(&self, document: &Document) -> Option<Location> {
        match self {
            Self::Home => Some(Location::Outline(None)),
            Self::Zoom(path) => exact(path, document).map(|zoom| Location::Outline(Some(zoom))),
            Self::Today => Some(Location::Today),
        }
    }
}

/// The block of `document` at the whole of `path`, if there is one.
fn exact(path: &TextPath, document: &Document) -> Option<ItemId> {
    path.find(document)
        .and_then(|(id, whole)| whole.then_some(id))
}

impl State {
    /// The layout to keep with the open file, or `None` when there is none to keep it with: the
    /// document has no file, or its file is the temporary one, which no later launch opens.
    #[must_use]
    pub fn view_state(&self) -> Option<ViewState> {
        if self.path().is_none() || self.origin() == Origin::Temporary {
            return None;
        }
        Some(ViewState {
            panes: self
                .panes_in_order()
                .map(|pane| self.pane_state(pane))
                .collect(),
            focused: self.pane_at,
            sidebar: self.sidebar.shown(),
            today: self
                .today_order
                .placed()
                .iter()
                .filter_map(|&id| TextPath::of(&self.document, id))
                .collect(),
        })
    }

    /// How `pane` is kept: a history entry whose zoom block is gone is left out.
    fn pane_state(&self, pane: &Pane) -> PaneState {
        let mut history = Vec::new();
        let mut at = 0;
        for (index, &location) in pane.history.iter().enumerate() {
            if let Some(place) = Place::of(&self.document, location) {
                history.push(place);
                if index <= pane.history_at {
                    at = history.len().saturating_sub(1);
                }
            }
        }
        PaneState {
            history,
            at,
            focus: pane.focus.and_then(|id| TextPath::of(&self.document, id)),
            width: pane.width,
            show_completed: pane.show_done,
        }
    }

    /// This state with the layout `view` kept for its file, as far as it matches the document:
    /// a zoom or focused block that is not found is dropped, a pane whose location is dropped
    /// shows home, and a pane whose focus is dropped focuses where it lands; a focus that does not
    /// show moves on the next message, as after any change (file-io.md §5, navigation.md §2).
    /// Without any pane the panes stay as they are. The Today view's order keeps the blocks found.
    #[must_use]
    pub fn with_view_state(mut self, view: ViewState) -> Self {
        let ViewState {
            panes,
            focused,
            sidebar,
            today,
        } = view;
        self.today_order =
            TodayOrder::new(today.iter().filter_map(|path| exact(path, &self.document)));
        let panes = panes.iter().map(|saved| self.restored(saved)).collect();
        self.set_panes(panes, focused);
        self.sidebar.show(sidebar);
        self
    }

    /// The pane `saved` keeps, in the open document.
    fn restored(&mut self, saved: &PaneState) -> Pane {
        let home = Location::Outline(None);
        let mut history: Vec<Location> = Vec::new();
        let mut at = 0;
        for (index, place) in saved.history.iter().enumerate() {
            let current = index == saved.at;
            let Some(location) = place.find(&self.document).or(current.then_some(home)) else {
                continue;
            };
            // Dropping entries can bring two visits of one location together; they are one.
            if history.last() != Some(&location) {
                history.push(location);
            }
            if current {
                at = history.len().saturating_sub(1);
            }
        }
        let location = history.get(at).copied().unwrap_or(home);
        let mut pane = self.new_pane(location);
        if !history.is_empty() {
            pane.history = history;
            pane.history_at = at;
        }
        pane.show_done = saved.show_completed;
        pane.width = saved.width;
        // A focus found is kept even where it does not show yet: the Today view shows nothing
        // until the frontend gives the date, so whether it shows is only known on the next
        // message, which moves it as any focus that stops showing (navigation.md §2).
        pane.focus = saved
            .focus
            .as_ref()
            .and_then(|path| exact(path, &self.document))
            .or_else(|| landing(&self.rows_of(&pane)));
        pane
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, time::Duration};

    use super::*;
    use crate::{
        app::{Message, PaneChange, SidebarInput},
        document::{Item, format},
    };

    /// Two projects with tasks, and a third without.
    const OUTLINE: &str = "Work:\n\t- Report\n\t- Slides @done\nHome:\n\t- Plants\nLater:\n";

    /// The file `/notes/todo.taskpaper` holding `text`.
    fn open(text: &str) -> State {
        let mut state = State::new(
            format::parse(text),
            Some(PathBuf::from("/notes/todo.taskpaper")),
        );
        state.update(Message::Tick {
            now: Duration::ZERO,
            today: jiff::civil::date(2026, 10, 5),
        });
        state
    }

    /// The display text of block `id`.
    fn text(state: &State, id: Option<ItemId>) -> String {
        id.and_then(|id| state.document().get(id))
            .map(Item::display_text)
            .unwrap_or_default()
    }

    /// Each pane as its last crumb, or `⌂` at home and `Today` in the Today view, with its focused
    /// block's text and its width when it has one, the focused pane marked with `*`.
    fn panes(state: &State) -> Vec<String> {
        state
            .panes()
            .iter()
            .map(|pane| {
                let at = match pane.location {
                    Location::Outline(_) => pane.breadcrumbs.last().map_or("⌂", String::as_str),
                    Location::Today => "Today",
                };
                let mark = if pane.focused { "*" } else { "" };
                let width = pane.width.map(|w| format!(" ({w})")).unwrap_or_default();
                format!("{mark}{at}: {}{width}", text(state, pane.focus))
            })
            .collect()
    }

    /// Focuses the row reading `wanted` in the focused pane.
    fn focus(state: &mut State, wanted: &str) {
        state.update(Message::FocusFirst);
        while text(state, state.focus()) != wanted {
            let before = state.focus();
            state.update(Message::FocusNext);
            assert_ne!(state.focus(), before, "no row reads {wanted}");
        }
    }

    /// The breadcrumbs' last crumb, or `⌂` at home.
    fn crumb(state: &State) -> String {
        state
            .breadcrumbs()
            .last()
            .cloned()
            .unwrap_or("⌂".to_owned())
    }

    /// `state`'s layout restored on a new launch of its file holding `text`.
    fn reopened(state: &State, text: &str) -> State {
        let view = state.view_state().expect("a layout to keep");
        open(text).with_view_state(view)
    }

    /// Three panes: home focusing Home with completed blocks hidden; Work zoomed in, focusing
    /// Report, 44 columns wide, with home ahead in its history; and Today, entered from Work.
    /// The middle one has the keys, and the sidebar is hidden.
    fn laid_out() -> State {
        let mut state = open(OUTLINE);
        focus(&mut state, "Work");
        state.update(Message::Pane(PaneChange::Open));
        state.update(Message::ResizePane(44));
        state.update(Message::Pane(PaneChange::OpenLocation));
        state.update(Message::FocusSidebar);
        state.update(Message::Sidebar(SidebarInput::Choose));
        state.update(Message::Pane(PaneChange::Focus(0)));
        state.update(Message::ToggleShowDone);
        focus(&mut state, "Home");
        state.update(Message::Pane(PaneChange::Focus(1)));
        state.update(Message::GoHome);
        state.update(Message::Back);
        state.update(Message::ToggleSidebar);
        state
    }

    /// The text of every row of the pane at `at`.
    fn rows(state: &State, at: usize) -> Vec<String> {
        state.panes()[at]
            .rows
            .iter()
            .map(|row| row.text.clone())
            .collect()
    }

    // FR-UI-04, file-io.md §5, navigation.md §5
    #[test]
    fn a_new_launch_restores_the_panes_their_zooms_focus_widths_and_settings() {
        let state = laid_out();
        let expected = ["⌂: Home", "*Work: Report (44)", "Today: "];
        assert_eq!(panes(&state), expected);
        let mut restored = reopened(&state, OUTLINE);
        assert_eq!(panes(&restored), expected);
        assert_eq!(restored.sidebar(), None, "the sidebar stays hidden");
        assert!(!rows(&restored, 0).contains(&"Slides".to_owned()));
        assert!(rows(&restored, 1).contains(&"Slides".to_owned()));
        restored.update(Message::Forward);
        assert_eq!(crumb(&restored), "⌂", "forward to home");
        restored.update(Message::Pane(PaneChange::FocusRight));
        restored.update(Message::Back);
        assert_eq!(crumb(&restored), "Work", "back to Work");
        restored.update(Message::ToggleSidebar);
        assert!(restored.sidebar().is_some());
        let ids: std::collections::HashSet<_> =
            restored.panes().iter().map(|pane| pane.id).collect();
        assert_eq!(ids.len(), 3, "each pane told apart");
    }

    // FR-UI-04, file-io.md §5
    #[test]
    fn a_new_launch_finds_blocks_that_moved_and_shows_the_sidebar_when_it_showed() {
        let mut state = laid_out();
        state.update(Message::ToggleSidebar);
        let moved =
            "Intro\nLater:\nHome:\n\t- Plants\nWork:\n\t- Slides @done\n\t- Report @today\n";
        let restored = reopened(&state, moved);
        assert_eq!(
            panes(&restored),
            ["⌂: Home", "*Work: Report (44)", "Today: Report"]
        );
        assert!(restored.sidebar().is_some());
    }

    // FR-UI-04, file-io.md §5
    #[test]
    fn what_no_longer_matches_is_dropped() {
        let state = laid_out();
        let mut restored = reopened(&state, "Job:\n\t- Report\nHome:\n\t- Plants\n");
        assert_eq!(
            panes(&restored),
            ["⌂: Home", "*⌂: Job (44)", "Today: "],
            "a zoom that is gone shows home, a focus that is gone lands"
        );
        restored.update(Message::Forward);
        restored.update(Message::Back);
        assert_eq!(crumb(&restored), "⌂", "one visit of home is left");
        restored.update(Message::Pane(PaneChange::FocusRight));
        restored.update(Message::Back);
        assert_eq!(
            restored.location(),
            Location::Today,
            "Work left the history"
        );
    }

    // FR-UI-04, FR-DONE-02, file-io.md §5, navigation.md §2
    #[test]
    fn a_focus_that_does_not_show_moves_as_after_any_change() {
        let mut state = open(OUTLINE);
        state.update(Message::ToggleShowDone);
        focus(&mut state, "Plants");
        let mut restored = reopened(&state, "Work:\nHome:\n\t- Plants @done\n");
        restored.update(Message::CheckFile);
        assert_eq!(panes(&restored), ["*⌂: Home"]);
    }

    // FR-UI-04, FR-TAG-04, file-io.md §5: the date arrives after the layout.
    #[test]
    fn a_today_entry_stays_focused_until_the_date_shows_it() {
        let text = "Errands:\n\t- Post office @today\n\t- Dentist @due(2026-10-05)\n";
        let mut state = open(text);
        state.update(Message::FocusSidebar);
        state.update(Message::Sidebar(SidebarInput::Choose));
        state.update(Message::FocusNext);
        assert_eq!(panes(&state), ["*Today: Dentist"]);
        let view = state.view_state().expect("a layout to keep");
        let mut restored = State::new(
            format::parse(text),
            Some(PathBuf::from("/notes/todo.taskpaper")),
        )
        .with_view_state(view);
        restored.update(Message::Tick {
            now: Duration::ZERO,
            today: jiff::civil::date(2026, 10, 5),
        });
        assert_eq!(panes(&restored), ["*Today: Dentist"]);
    }

    // FR-NAV-04, file-io.md §5
    #[test]
    fn a_history_entry_whose_block_was_deleted_is_not_kept() {
        let mut state = open(OUTLINE);
        state.update(Message::Pane(PaneChange::OpenLocation));
        for project in ["Work", "Later"] {
            focus(&mut state, project);
            state.update(Message::ZoomIn);
            state.update(Message::GoHome);
        }
        focus(&mut state, "Later");
        state.update(Message::ZoomIn);
        state.update(Message::Pane(PaneChange::FocusLeft));
        focus(&mut state, "Work");
        state.update(Message::Delete);
        let mut restored = reopened(&state, OUTLINE);
        restored.update(Message::Pane(PaneChange::FocusRight));
        let mut visited = Vec::new();
        for _ in 0..4 {
            restored.update(Message::Back);
            visited.push(crumb(&restored));
        }
        assert_eq!(visited, ["⌂", "Later", "⌂", "⌂"]);
    }

    // FR-UI-04, D-57, file-io.md §5
    #[test]
    fn a_new_launch_restores_the_today_views_order_as_far_as_it_matches() {
        let text = "Errands:\n\t- Post office @today\n\t- Bank @today\n- Dentist @today\n- Plan\n";
        let mut state = open(text);
        state.go_to(Location::Today);
        state.update(Message::FocusLast);
        state.update(Message::MoveUp);
        state.update(Message::MoveUp);
        assert_eq!(rows(&state, 0), ["Dentist", "Post office", "Bank"]);
        let mut restored = reopened(&state, text);
        restored.go_to(Location::Today);
        assert_eq!(rows(&restored, 0), ["Dentist", "Post office", "Bank"]);
        let changed = "- Plan @today\n- Dentist @today\nErrands:\n\t- Bank @today\n";
        let mut restored = reopened(&state, changed);
        restored.go_to(Location::Today);
        assert_eq!(
            rows(&restored, 0),
            ["Dentist", "Bank", "Plan"],
            "a block that is gone is dropped and a new entry comes last"
        );
    }

    // file-io.md §5
    #[test]
    fn a_layout_out_of_range_keeps_what_it_can() {
        let view: ViewState = serde_json::from_str(
            r#"{"panes": [{"history": [], "at": 0, "focus": null, "width": null,
                           "show_completed": true},
                          {"history": ["today", {"zoom": [["Work", 0]]}], "at": 7,
                           "focus": null, "width": 50, "show_completed": false}],
                "focused": 9, "sidebar": true}"#,
        )
        .expect("a layout");
        let mut state = open(OUTLINE).with_view_state(view);
        assert_eq!(panes(&state), ["⌂: Work", "*Today:  (50)"]);
        state.update(Message::Forward);
        assert_eq!(crumb(&state), "Work");
        let none: ViewState =
            serde_json::from_str(r#"{"panes": [], "focused": 0, "sidebar": false}"#)
                .expect("a layout");
        let state = open(OUTLINE).with_view_state(none);
        assert_eq!(panes(&state), ["*⌂: Work"], "without panes the pane stays");
        assert_eq!(state.sidebar(), None);
    }

    // file-io.md §5
    #[test]
    fn only_a_file_a_later_launch_opens_has_a_layout_to_keep() {
        assert!(open(OUTLINE).view_state().is_some());
        let temporary = open(OUTLINE).with_origin(Origin::Temporary);
        assert_eq!(temporary.view_state(), None);
        assert_eq!(State::new(format::parse(OUTLINE), None).view_state(), None);
    }
}
