//! Panes: views of the outline side by side, each with its own location and history, focused
//! block, selection and show-completed setting, while the document and the folds are shared by
//! all of them; which pane has the keys, and keeping every pane on blocks that still show after
//! a change made in any of them
//! ([navigation.md §5](../../../docs/specs/navigation.md#5-panes-fr-ui-02--fr-ui-04-d-13),
//! FR-UI-02, FR-UI-03, D-13).

use std::{cmp::Ordering, rc::Rc};

use super::{
    Location, Row, SidebarInput, State,
    commands::{Command, NodeCommand},
    selection::Selection,
};
use crate::document::{Block, ItemId, blocks};

/// What tells a pane apart from the others for as long as it is open, so that a frontend can keep
/// what it shows of each pane, such as how far it is scrolled, while panes open and close.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PaneId(u64);

/// What one pane shows and where its keys act.
#[derive(Debug, Clone)]
pub(super) struct Pane {
    id: PaneId,
    /// The focused row's item, `None` only when the pane has no rows (FR-NAV-01).
    pub(super) focus: Option<ItemId>,
    /// The selected blocks (FR-OPS-01).
    pub(super) selection: Selection,
    /// What the pane shows: the outline below a zoom, or the Today view (FR-NAV-03, D-7).
    pub(super) location: Location,
    /// Every location visited, oldest first, for back and forward (FR-NAV-04).
    pub(super) history: Vec<Location>,
    /// The position of the current location in `history`.
    pub(super) history_at: usize,
    /// Whether done blocks show, with everything inside them (FR-DONE-02).
    pub(super) show_done: bool,
    /// The width the frontend last gave the pane when it was narrowed or widened, kept for it,
    /// or `None` while the pane takes its share of the screen.
    pub(super) width: Option<u16>,
}

impl Pane {
    /// The pane `id` at `location`, the only one in its history, that shows completed blocks and
    /// has nothing focused or selected yet.
    fn new(id: PaneId, location: Location) -> Self {
        Self {
            id,
            focus: None,
            selection: Selection::default(),
            location,
            history: vec![location],
            history_at: 0,
            show_done: true,
            width: None,
        }
    }

    /// The first pane of a document: at home.
    pub(super) fn first() -> Self {
        Self::new(PaneId(0), Location::Outline(None))
    }

    /// The block shown as the root, or `None` at home and in the Today view (FR-NAV-03).
    pub(super) fn zoom(&self) -> Option<ItemId> {
        self.location.zoom()
    }

    /// Keeps the pane on the blocks of a reloaded document: its locations and focus on the block
    /// `nearest` finds for theirs, home or none when it finds none, and its selection on the
    /// blocks `exact` finds (file-io.md §3).
    pub(super) fn rematch(
        &mut self,
        nearest: impl Fn(ItemId) -> Option<ItemId> + Copy,
        exact: impl Fn(ItemId) -> Option<ItemId>,
    ) {
        self.location = self.location.rematched(nearest);
        self.history = self
            .history
            .iter()
            .map(|location| location.rematched(nearest))
            .collect();
        self.focus = self.focus.and_then(nearest);
        self.selection = self.selection.mapped(exact);
    }

    /// Whether `block` shows where the blocks around it do: not when it is done while completed
    /// blocks are hidden (FR-DONE-02).
    pub(super) fn shows(&self, block: &Block) -> bool {
        self.show_done || !block.done()
    }
}

/// A change to which panes are open and which of them has the keys (navigation.md §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneChange {
    /// Open the highlighted entry of Go to Symbol while it is open, else the focused sidebar entry
    /// while the sidebar has the keys, else the focused block, in a new pane right of the focused
    /// one, zoomed into it, and give that pane the keys.
    Open,
    /// Open the focused pane's location in a new pane right of it, and give that pane the keys.
    OpenLocation,
    /// Open the focused pane's breadcrumb at this depth, as [`Message::GoToCrumb`] counts it, in
    /// a new pane right of it, and give that pane the keys (FR-KEY-05).
    ///
    /// [`Message::GoToCrumb`]: super::Message::GoToCrumb
    OpenCrumb(usize),
    /// Give the keys to the pane at this position, counted from 0 at the left, if there is one.
    Focus(usize),
    /// Give the keys to the pane left of the focused one.
    FocusLeft,
    /// Give the keys to the pane right of the focused one.
    FocusRight,
    /// Close the focused pane, unless it is the only one, and give the keys to its left
    /// neighbor, else its right one.
    Close,
}

/// One pane as a frontend shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneView {
    /// What tells it apart from the other panes while it is open.
    pub id: PaneId,
    /// Whether it has the keys.
    pub focused: bool,
    /// What it shows.
    pub location: Location,
    /// The text of each block from the top level down to its zoom block, as
    /// [`State::breadcrumbs`] gives them.
    pub breadcrumbs: Vec<String>,
    /// Its visible rows.
    pub rows: Rc<[Row]>,
    /// Its focused row's item.
    pub focus: Option<ItemId>,
    /// The width the frontend gave it when it was last narrowed or widened, or `None` while it
    /// takes its share of the screen.
    pub width: Option<u16>,
}

/// Where a pane stood before a change, which it goes back to when the change removes the blocks
/// it was on.
#[derive(Debug)]
pub(super) struct Landmark {
    id: PaneId,
    /// The zoom block and its ancestors, from the zoom block up.
    zoom_line: Vec<ItemId>,
    /// The blocks its rows showed.
    rows: Vec<ItemId>,
}

/// The block to focus on arriving at a location with `rows`: the first block under the title,
/// else the title.
pub(super) fn landing(rows: &[Row]) -> Option<ItemId> {
    rows.iter()
        .find(|row| !row.title)
        .or(rows.first())
        .map(|row| row.id)
}

impl State {
    /// Every pane from left to right (FR-UI-02).
    #[must_use]
    pub fn panes(&self) -> Vec<PaneView> {
        self.panes_in_order()
            .map(|pane| self.pane_view(pane, pane.id == self.pane.id))
            .collect()
    }

    /// Every pane from left to right.
    pub(super) fn panes_in_order(&self) -> impl Iterator<Item = &Pane> {
        let (left, right) = self.others.split_at(self.pane_at.min(self.others.len()));
        left.iter().chain([&self.pane]).chain(right)
    }

    /// Replaces the panes with `panes` from left to right, giving the keys to the one at
    /// `focused`, or the last when there are fewer; without any, the panes stay as they are.
    pub(super) fn set_panes(&mut self, mut panes: Vec<Pane>, focused: usize) {
        let at = focused.min(panes.len().saturating_sub(1));
        if at < panes.len() {
            self.pane = panes.remove(at);
            self.others = panes;
            self.pane_at = at;
        }
    }

    /// `pane` as a frontend shows it, with the keys when `focused`.
    fn pane_view(&self, pane: &Pane, focused: bool) -> PaneView {
        PaneView {
            id: pane.id,
            focused,
            location: pane.location,
            breadcrumbs: self.breadcrumbs_of(pane),
            rows: self.rows_of(pane),
            focus: pane.focus,
            width: pane.width,
        }
    }

    /// Applies `change` to the panes.
    pub(super) fn change_panes(&mut self, change: PaneChange) {
        match change {
            PaneChange::Open => self.open_chosen(),
            PaneChange::OpenLocation => self.open_pane(self.pane.location, self.pane.focus),
            PaneChange::OpenCrumb(depth) => {
                if let Some(location) = self.crumb(depth) {
                    self.open_pane(location, None);
                }
            }
            PaneChange::Focus(at) => self.focus_pane(at),
            PaneChange::FocusLeft => {
                if let Some(at) = self.pane_at.checked_sub(1) {
                    self.focus_pane(at);
                }
            }
            PaneChange::FocusRight => self.focus_pane(self.pane_at.saturating_add(1)),
            PaneChange::Close => self.close_pane(),
        }
    }

    /// Keeps `width` as the focused pane's, as the frontend chose it when narrowing or widening
    /// it.
    pub(super) fn resize_pane(&mut self, width: u16) {
        self.pane.width = Some(width);
    }

    /// Opens the highlighted Go to Symbol entry, the focused sidebar entry or the focused block
    /// in a new pane, as [`PaneChange::Open`] says, closing Go to Symbol and giving the pane the
    /// keys back from the sidebar.
    fn open_chosen(&mut self) {
        let chosen = if let Some(finder) = self.symbols.take() {
            finder
                .highlighted()
                .copied()
                .map(|id| Location::Outline(Some(id)))
        } else if self.sidebar_focused() {
            let location = self.sidebar_location();
            self.sidebar_input(SidebarInput::Leave);
            location
        } else {
            self.pane
                .focus
                .filter(|&id| blocks::block(&self.document, id).is_some())
                .map(|id| Location::Outline(Some(id)))
        };
        if let Some(location) = chosen {
            self.open_pane(location, None);
        }
    }

    /// Opens a pane at `location` right of the focused one, focusing `focus`, which must show
    /// there, or with `None` the block it lands on, and gives it the keys. It shows completed
    /// blocks when the focused pane does.
    fn open_pane(&mut self, location: Location, focus: Option<ItemId>) {
        let mut pane = self.new_pane(location);
        pane.show_done = self.pane.show_done;
        pane.focus = focus.or_else(|| landing(&self.rows_of(&pane)));
        let left = std::mem::replace(&mut self.pane, pane);
        self.others.insert(self.pane_at, left);
        self.pane_at += 1;
    }

    /// A pane at `location` told apart from every other, as [`Pane::new`] makes it.
    pub(super) fn new_pane(&mut self, location: Location) -> Pane {
        self.pane_ids = self.pane_ids.saturating_add(1);
        Pane::new(PaneId(self.pane_ids), location)
    }

    /// Gives the keys to the pane at `at`, counted from 0 at the left, if there is one.
    fn focus_pane(&mut self, at: usize) {
        // The other panes leave out the focused one, so those right of it sit one place left of
        // their position, and the focused one goes back between those left and right of it.
        let (index, back) = match at.cmp(&self.pane_at) {
            Ordering::Equal => return,
            Ordering::Less => (at, self.pane_at - 1),
            Ordering::Greater => (at - 1, self.pane_at),
        };
        if index >= self.others.len() {
            return;
        }
        let target = self.others.remove(index);
        let left = std::mem::replace(&mut self.pane, target);
        self.others.insert(back, left);
        self.pane_at = at;
    }

    /// Closes the focused pane unless it is the only one, giving the keys to its left neighbor,
    /// else its right one.
    fn close_pane(&mut self) {
        let index = self.pane_at.saturating_sub(1);
        if index < self.others.len() {
            self.pane = self.others.remove(index);
            self.pane_at = index;
        }
    }

    /// The commands of the panes a context menu offers on the focused block, or on the selection
    /// when `alone` is false: opening the block or the pane's location in a new pane, and closing
    /// the pane when there are others.
    pub(super) fn pane_commands(&self, alone: bool) -> Vec<NodeCommand> {
        alone
            .then_some(PaneChange::Open)
            .into_iter()
            .chain([PaneChange::OpenLocation])
            .chain((!self.others.is_empty()).then_some(PaneChange::Close))
            .map(|change| NodeCommand::named(Command::Pane(change)))
            .collect()
    }

    /// Where each pane stands, as a change may remove the blocks it is on.
    pub(super) fn landmarks(&self) -> Vec<Landmark> {
        std::iter::once(&self.pane)
            .chain(&self.others)
            .map(|pane| Landmark {
                id: pane.id,
                zoom_line: std::iter::successors(
                    pane.zoom().and_then(|id| blocks::block(&self.document, id)),
                    |block| blocks::parent(&self.document, block.id()),
                )
                .map(|block| block.id())
                .collect(),
                rows: self.rows_of(pane).iter().map(|row| row.id).collect(),
            })
            .collect()
    }

    /// Keeps every pane on what still shows after a change made in any of them, from where it
    /// stood by its `landmarks`: a pane whose zoom block is gone zooms to its nearest surviving
    /// ancestor, which takes the zoom block's place in its history, a focus that no longer shows
    /// moves (navigation.md §2), and blocks no row shows leave the selection.
    pub(super) fn repair_panes(&mut self, landmarks: &[Landmark]) {
        self.pane = self.repaired(&self.pane, landmarks);
        self.others = self
            .others
            .iter()
            .map(|pane| self.repaired(pane, landmarks))
            .collect();
    }

    /// `pane` kept on what still shows, as [`State::repair_panes`] does.
    fn repaired(&self, pane: &Pane, landmarks: &[Landmark]) -> Pane {
        let mut pane = pane.clone();
        let landmark = landmarks.iter().find(|landmark| landmark.id == pane.id);
        if pane
            .zoom()
            .is_some_and(|zoom| blocks::block(&self.document, zoom).is_none())
        {
            let kept = landmark
                .into_iter()
                .flat_map(|landmark| &landmark.zoom_line)
                .copied()
                .find(|&id| blocks::block(&self.document, id).is_some());
            pane.location = Location::Outline(kept);
            if let Some(entry) = pane.history.get_mut(pane.history_at) {
                *entry = pane.location;
            }
            pane.focus = landing(&self.rows_of(&pane));
        }
        let before = landmark.map_or(&[][..], |landmark| &landmark.rows);
        self.repair_focus(&mut pane, before);
        self.keep_visible_selection(&mut pane);
        pane
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, time::Duration};

    use super::*;
    use crate::{
        app::{Menu, MenuInput, Message, Mode, SelectionChange, TextEdit, Version},
        document::{Item, format},
    };

    /// Two projects with tasks, and a third without.
    const OUTLINE: &str = "Work:\n\t- Report\n\t- Slides @done\nHome:\n\t- Plants\nLater:\n";

    fn open(text: &str) -> State {
        let mut state = State::new(format::parse(text), None);
        state.update(Message::Tick {
            now: Duration::ZERO,
            today: jiff::civil::date(2026, 10, 5),
        });
        state
    }

    fn pane(state: &mut State, change: PaneChange) {
        state.update(Message::Pane(change));
    }

    /// The display text of block `id`.
    fn text(state: &State, id: Option<ItemId>) -> String {
        id.and_then(|id| state.document().get(id))
            .map(Item::display_text)
            .map(str::to_owned)
            .unwrap_or_default()
    }

    /// Each pane as its last crumb, or `⌂` at home and `Today` in the Today view, with its focused
    /// block's text, the focused pane marked with `*`.
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
                format!("{mark}{at}: {}", text(state, pane.focus))
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

    // FR-UI-02, FR-UI-03, navigation.md §5
    #[test]
    fn opening_the_focused_block_zooms_into_it_in_a_new_pane_right_of_the_focused_one() {
        let mut state = open(OUTLINE);
        assert_eq!(panes(&state), ["*⌂: Work"]);
        focus(&mut state, "Home");
        pane(&mut state, PaneChange::Open);
        assert_eq!(panes(&state), ["⌂: Home", "*Home: Plants"]);
        pane(&mut state, PaneChange::FocusLeft);
        focus(&mut state, "Work");
        pane(&mut state, PaneChange::Open);
        assert_eq!(
            panes(&state),
            ["⌂: Work", "*Work: Report", "Home: Plants"],
            "inserted between them"
        );
        let ids: Vec<PaneId> = state.panes().iter().map(|pane| pane.id).collect();
        assert_eq!(
            ids.iter().collect::<std::collections::HashSet<_>>().len(),
            3,
            "each pane told apart"
        );
        focus(&mut state, "Work");
        pane(&mut state, PaneChange::Open);
        assert_eq!(
            panes(&state)[2],
            "*Work: Report",
            "the title opens its own zoom"
        );
    }

    // FR-UI-03, navigation.md §5
    #[test]
    fn opening_a_leaf_focuses_its_title_and_nothing_opens_without_a_focus() {
        let mut state = open(OUTLINE);
        focus(&mut state, "Later");
        pane(&mut state, PaneChange::Open);
        assert_eq!(panes(&state), ["⌂: Later", "*Later: Later"]);
        let mut empty = open("");
        pane(&mut empty, PaneChange::Open);
        pane(&mut empty, PaneChange::OpenLocation);
        assert_eq!(empty.panes().len(), 2, "only the location opens");
    }

    // FR-UI-03, navigation.md §5
    #[test]
    fn opening_the_location_copies_the_zoom_and_focus_with_a_history_of_its_own() {
        let mut state = open(OUTLINE);
        focus(&mut state, "Work");
        state.update(Message::ZoomIn);
        focus(&mut state, "Slides");
        pane(&mut state, PaneChange::OpenLocation);
        assert_eq!(panes(&state), ["Work: Slides", "*Work: Slides"]);
        state.update(Message::Back);
        assert_eq!(
            panes(&state)[1],
            "*Work: Slides",
            "nothing to go back to in the new pane"
        );
        state.update(Message::ToggleShowDone);
        pane(&mut state, PaneChange::OpenLocation);
        assert_eq!(
            panes(&state)[2],
            "*Work: Report",
            "completed blocks hidden as in the pane it came from"
        );
        pane(&mut state, PaneChange::FocusLeft);
        state.update(Message::GoHome);
        state.update(Message::Pane(PaneChange::FocusRight));
        state.update(Message::FocusSidebar);
        state.update(Message::Sidebar(crate::app::SidebarInput::Choose));
        pane(&mut state, PaneChange::OpenLocation);
        assert_eq!(panes(&state)[3], "*Today: ", "the Today view opens too");
    }

    // FR-KEY-05, FR-UI-03
    #[test]
    fn opening_a_crumb_zooms_a_new_pane_there() {
        let mut state = open(OUTLINE);
        focus(&mut state, "Work");
        state.update(Message::ZoomIn);
        focus(&mut state, "Report");
        state.update(Message::ZoomIn);
        pane(&mut state, PaneChange::OpenCrumb(1));
        assert_eq!(panes(&state), ["Report: Report", "*Work: Report"]);
        pane(&mut state, PaneChange::OpenCrumb(0));
        assert_eq!(panes(&state)[2], "*⌂: Work");
        pane(&mut state, PaneChange::OpenCrumb(1));
        assert_eq!(panes(&state).len(), 3, "home has no deeper crumb");
    }

    // FR-UI-02, FR-DONE-02, FR-NAV-04, FR-OPS-01, D-13
    #[test]
    fn each_pane_has_its_own_zoom_history_focus_selection_and_completed_setting() {
        let mut state = open(OUTLINE);
        pane(&mut state, PaneChange::OpenLocation);
        focus(&mut state, "Report");
        state.update(Message::Select(SelectionChange::Toggle));
        state.update(Message::ToggleShowDone);
        state.update(Message::ZoomOut);
        focus(&mut state, "Work");
        state.update(Message::ZoomIn);
        pane(&mut state, PaneChange::FocusLeft);
        assert_eq!(panes(&state), ["*⌂: Work", "Work: Report"]);
        assert!(state.rows().iter().all(|row| !row.selected));
        assert!(
            state.rows().iter().any(|row| row.done),
            "completed shown here"
        );
        let right = &state.panes()[1];
        assert!(right.rows.iter().all(|row| !row.done), "and hidden there");
        assert!(right.rows.iter().any(|row| row.selected));
        state.update(Message::Back);
        assert_eq!(panes(&state)[0], "*⌂: Work", "no history here");
        pane(&mut state, PaneChange::FocusRight);
        state.update(Message::Back);
        assert_eq!(panes(&state)[1], "*⌂: Work", "back home in its own history");
    }

    // FR-UI-02, D-13
    #[test]
    fn folds_are_shared_and_a_focus_they_hide_moves_in_every_pane() {
        let mut state = open(OUTLINE);
        focus(&mut state, "Report");
        pane(&mut state, PaneChange::OpenLocation);
        pane(&mut state, PaneChange::FocusLeft);
        focus(&mut state, "Work");
        state.update(Message::Collapse);
        assert_eq!(panes(&state), ["*⌂: Work", "⌂: Home"]);
        let right = &state.panes()[1];
        assert_eq!(right.rows.first().map(|row| row.hidden_children), Some(2));
    }

    // FR-UI-02, navigation.md §2 and §5: edits in one pane show in all of them.
    #[test]
    fn a_deletion_in_one_pane_moves_the_focus_zoom_and_selection_of_the_others() {
        let mut state = open(OUTLINE);
        focus(&mut state, "Report");
        state.update(Message::Select(SelectionChange::Toggle));
        pane(&mut state, PaneChange::OpenLocation);
        focus(&mut state, "Work");
        pane(&mut state, PaneChange::Open);
        focus(&mut state, "Report");
        pane(&mut state, PaneChange::Open);
        pane(&mut state, PaneChange::Focus(0));
        assert_eq!(
            panes(&state),
            ["*⌂: Report", "⌂: Work", "Work: Report", "Report: Report"]
        );
        state.update(Message::Select(SelectionChange::Clear));
        pane(&mut state, PaneChange::Focus(1));
        focus(&mut state, "Report");
        state.update(Message::Delete);
        assert_eq!(
            panes(&state),
            ["⌂: Slides", "*⌂: Slides", "Work: Slides", "Work: Slides"],
            "the zoom into the deleted block goes to its parent"
        );
        pane(&mut state, PaneChange::Focus(0));
        assert!(state.rows().iter().all(|row| !row.selected), "unselected");
        focus(&mut state, "Work");
        state.update(Message::Delete);
        assert_eq!(
            panes(&state),
            ["*⌂: Home", "⌂: Home", "⌂: Home", "⌂: Home"],
            "home when every block above the zoom is gone"
        );
        state.update(Message::Undo);
        assert_eq!(panes(&state)[0], "*⌂: Work", "undo returns to where it was");
    }

    // FR-NAV-04, navigation.md §4 and §5
    #[test]
    fn back_returns_to_the_ancestor_a_pane_went_to_when_its_zoom_block_was_deleted() {
        let mut state = open(OUTLINE);
        focus(&mut state, "Report");
        state.update(Message::ZoomIn);
        pane(&mut state, PaneChange::OpenLocation);
        state.update(Message::GoHome);
        focus(&mut state, "Report");
        state.update(Message::Delete);
        pane(&mut state, PaneChange::FocusLeft);
        assert_eq!(panes(&state)[0], "*Work: Slides");
        state.update(Message::GoHome);
        state.update(Message::Back);
        assert_eq!(panes(&state)[0], "*Work: Slides");
    }

    // FR-NAV-04, navigation.md §4
    #[test]
    fn back_and_forward_skip_blocks_another_pane_deleted() {
        let mut state = open(OUTLINE);
        for project in ["Work", "Home", "Later"] {
            focus(&mut state, project);
            state.update(Message::ZoomIn);
            state.update(Message::GoHome);
        }
        pane(&mut state, PaneChange::OpenLocation);
        focus(&mut state, "Home");
        state.update(Message::Delete);
        pane(&mut state, PaneChange::FocusLeft);
        for expected in ["Later", "⌂", "⌂", "Work", "⌂"] {
            state.update(Message::Back);
            assert_eq!(
                state.breadcrumbs().last().map_or("⌂", String::as_str),
                expected
            );
        }
        state.update(Message::Back);
        assert_eq!(
            state.breadcrumbs(),
            Vec::<String>::new(),
            "stays at the start"
        );
        for expected in ["Work", "⌂", "⌂", "Later"] {
            state.update(Message::Forward);
            assert_eq!(
                state.breadcrumbs().last().map_or("⌂", String::as_str),
                expected
            );
        }
    }

    // FR-UI-03, navigation.md §5
    #[test]
    fn focusing_a_pane_by_number_or_neighbor_keeps_their_order() {
        let mut state = open(OUTLINE);
        for project in ["Work", "Home", "Later"] {
            pane(&mut state, PaneChange::Focus(0));
            focus(&mut state, project);
            pane(&mut state, PaneChange::Open);
            for _ in 0..3 {
                pane(&mut state, PaneChange::FocusRight);
            }
        }
        let order = ["⌂: Later", "Later: Later", "Home: Plants", "Work: Report"];
        let marked = |at: usize| -> Vec<String> {
            order
                .iter()
                .enumerate()
                .map(|(index, pane)| {
                    let mark = if index == at { "*" } else { "" };
                    format!("{mark}{pane}")
                })
                .collect()
        };
        assert_eq!(panes(&state), marked(3), "right stops at the last");
        for at in [1, 0, 2, 3, 0] {
            pane(&mut state, PaneChange::Focus(at));
            assert_eq!(panes(&state), marked(at));
        }
        pane(&mut state, PaneChange::FocusLeft);
        assert_eq!(panes(&state), marked(0), "left stops at the first");
        pane(&mut state, PaneChange::Focus(4));
        assert_eq!(panes(&state), marked(0), "no fifth pane");
        pane(&mut state, PaneChange::FocusRight);
        pane(&mut state, PaneChange::FocusRight);
        assert_eq!(panes(&state), marked(2));
    }

    // FR-UI-03, navigation.md §5
    #[test]
    fn closing_a_pane_focuses_its_left_neighbor_else_its_right_and_the_last_stays() {
        let mut state = open(OUTLINE);
        focus(&mut state, "Work");
        pane(&mut state, PaneChange::Open);
        pane(&mut state, PaneChange::FocusLeft);
        focus(&mut state, "Home");
        pane(&mut state, PaneChange::Open);
        assert_eq!(panes(&state), ["⌂: Home", "*Home: Plants", "Work: Report"]);
        pane(&mut state, PaneChange::Close);
        assert_eq!(panes(&state), ["*⌂: Home", "Work: Report"]);
        pane(&mut state, PaneChange::Close);
        assert_eq!(panes(&state), ["*Work: Report"]);
        pane(&mut state, PaneChange::Close);
        assert_eq!(panes(&state), ["*Work: Report"], "the last pane stays");
    }

    // FR-UI-03, navigation.md §5
    #[test]
    fn a_resized_pane_keeps_the_width_the_frontend_chose() {
        let mut state = open(OUTLINE);
        pane(&mut state, PaneChange::OpenLocation);
        state.update(Message::ResizePane(44));
        let widths: Vec<Option<u16>> = state.panes().iter().map(|pane| pane.width).collect();
        assert_eq!(widths, [None, Some(44)]);
    }

    // navigation.md §5: only the focused pane edits.
    #[test]
    fn going_to_another_pane_ends_editing_and_resizing_does_not() {
        let mut state = open(OUTLINE);
        pane(&mut state, PaneChange::OpenLocation);
        state.update(Message::EditAtEnd);
        state.update(Message::ResizePane(40));
        assert_eq!(state.mode(), Mode::Edit);
        state.update(Message::Text(TextEdit::Insert('!')));
        pane(&mut state, PaneChange::FocusLeft);
        assert_eq!(state.mode(), Mode::Navigate);
        assert_eq!(panes(&state), ["*⌂: Work!", "⌂: Work!"]);
    }

    // FR-NAV-05, FR-UI-03, navigation.md §8
    #[test]
    fn go_to_symbol_opens_the_highlighted_block_in_a_new_pane() {
        let mut state = open(OUTLINE);
        state.update(Message::OpenGoToSymbol);
        for c in "plants".chars() {
            state.update(Message::Menu(Menu::GoToSymbol, MenuInput::Type(c)));
        }
        pane(&mut state, PaneChange::Open);
        assert_eq!(state.go_to_symbol(), None, "closed");
        assert_eq!(panes(&state), ["⌂: Work", "*Plants: Plants"]);
        state.update(Message::OpenGoToSymbol);
        state.update(Message::Menu(Menu::GoToSymbol, MenuInput::Type('#')));
        pane(&mut state, PaneChange::Open);
        assert_eq!(state.go_to_symbol(), None);
        assert_eq!(state.panes().len(), 2, "nothing matched, nothing opened");
    }

    // FR-UI-01, FR-UI-03, navigation.md §6
    #[test]
    fn the_sidebar_opens_its_focused_entry_in_a_new_pane_which_gets_the_keys() {
        let mut state = open(OUTLINE);
        state.update(Message::FocusSidebar);
        state.update(Message::Sidebar(crate::app::SidebarInput::Next));
        state.update(Message::Sidebar(crate::app::SidebarInput::Next));
        pane(&mut state, PaneChange::Open);
        assert!(!state.sidebar_focused());
        assert_eq!(panes(&state), ["⌂: Work", "*Home: Plants"]);
        state.update(Message::FocusSidebar);
        state.update(Message::Sidebar(crate::app::SidebarInput::Previous));
        state.update(Message::Sidebar(crate::app::SidebarInput::Previous));
        pane(&mut state, PaneChange::Open);
        assert_eq!(panes(&state)[2], "*Today: ");
    }

    // FR-KEY-06, ui.md §5
    #[test]
    fn the_context_menu_offers_the_panes_group_with_close_pane_while_there_are_others() {
        let labels = |state: &State| -> Vec<String> {
            state
                .context_menu()
                .expect("open")
                .rows
                .into_iter()
                .filter_map(|row| match row {
                    crate::app::MenuRow::Command { label, .. } => Some(label.to_owned()),
                    crate::app::MenuRow::Divider => None,
                })
                .collect()
        };
        let mut state = open(OUTLINE);
        pane(&mut state, PaneChange::OpenLocation);
        state.update(Message::OpenContextMenu);
        assert!(labels(&state).ends_with(&[
            "Open in New Pane".to_owned(),
            "Open Location in New Pane".to_owned(),
            "Close Pane".to_owned(),
        ]));
        state.update(Message::Menu(Menu::ContextMenu, MenuInput::Dismiss));
        state.update(Message::Select(SelectionChange::ExtendDown));
        state.update(Message::OpenContextMenu);
        let listed = labels(&state);
        assert!(listed.ends_with(&[
            "Open Location in New Pane".to_owned(),
            "Close Pane".to_owned(),
        ]));
        assert!(
            !listed.contains(&"Open in New Pane".to_owned()),
            "the focused block's own command"
        );
    }

    // FR-UI-02, file-io.md §3
    #[test]
    fn a_reload_keeps_every_pane_on_its_blocks() {
        let path = PathBuf::from("/notes/todo.taskpaper");
        let mut state = State::new(format::parse(OUTLINE), Some(path))
            .with_version(Some(Version::of(OUTLINE.as_bytes())));
        focus(&mut state, "Home");
        pane(&mut state, PaneChange::Open);
        pane(&mut state, PaneChange::FocusLeft);
        focus(&mut state, "Report");
        state.update(Message::Select(SelectionChange::Toggle));
        pane(&mut state, PaneChange::FocusRight);
        let changed = "Intro\nWork:\n\t- Report\n\t- Slides @done\nHome:\n\t- Plants\n\t- Bins\n";
        state.update(Message::FileChecked {
            contents: Some(changed.as_bytes().to_vec()),
        });
        assert_eq!(panes(&state), ["⌂: Report", "*Home: Plants"]);
        pane(&mut state, PaneChange::FocusLeft);
        let selected: Vec<String> = state
            .rows()
            .iter()
            .filter(|row| row.selected)
            .map(|row| row.text.to_string())
            .collect();
        assert_eq!(selected, ["Report"]);
    }
}
