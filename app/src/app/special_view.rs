//! The special views: flat lists of blocks a pane shows instead of an outline, and the intents a
//! pane takes in one, since its entries stand alone rather than in a tree
//! ([navigation.md §7, §7.1](../../../docs/specs/navigation.md#7-today-view-fr-tag-04-d-7),
//! FR-TAG-04, FR-TAG-07, D-7, D-80).

use std::collections::HashSet;

use jiff::civil::Date;
use serde::{Deserialize, Serialize};

use super::{
    Location, Message, Row, State, above::Above, logbook::in_logbook, pane::Pane, today::for_today,
    upcoming::upcoming_day,
};
use crate::document::{Block, TagColor, blocks};

/// A special view, in the order the Navigator lists them (D-80).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecialView {
    /// The blocks for today (FR-TAG-04, D-7).
    Today,
    /// The blocks started or due in the 7 days after today, grouped by day (FR-TAG-08, D-81).
    Upcoming,
    /// The open blocks whose due date has passed, grouped by due date (FR-TAG-09, D-82).
    Overdue,
    /// The blocks with a color tag, grouped by color (FR-TAG-10, D-83).
    Colors,
    /// The blocks completed in the last 7 days, grouped by day of completion (FR-TAG-11, D-84).
    Logbook,
    /// The starred blocks (FR-TAG-13, D-92).
    Starred,
    /// The flagged blocks (FR-TAG-14, D-93).
    Flagged,
}

impl SpecialView {
    /// Every special view, in the Navigator's order (D-80).
    pub const ALL: [Self; 7] = [
        Self::Today,
        Self::Upcoming,
        Self::Overdue,
        Self::Colors,
        Self::Logbook,
        Self::Starred,
        Self::Flagged,
    ];

    /// The view's name, as its header, Navigator entry and Go to command show it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::Upcoming => "Upcoming",
            Self::Overdue => "Overdue",
            Self::Colors => "Colors",
            Self::Logbook => "Logbook",
            Self::Starred => "Starred",
            Self::Flagged => "Flagged",
        }
    }
}

/// What the entries of a group of a special view have in common, which its heading names
/// (D-80).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewGroup {
    /// The blocks with this color tag (D-83).
    Color(TagColor),
    /// The blocks listed for this day, such as their due date in the Overdue view (D-81, D-82).
    Day(Date),
}

/// The heading a special view's entry starts its group under: a line above the entry, not a row
/// of its own, so focus, selection and motion never reach it (navigation.md §7.1, D-80).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Heading {
    /// The group it names.
    pub group: ViewGroup,
    /// Whether it is the view's first heading, which has no blank line above it (ui.md §2).
    pub first: bool,
}

/// `entries`, each with its group and sorted by group, as rows: each group's first entry under the
/// group's heading.
pub(super) fn under_headings(entries: impl IntoIterator<Item = (ViewGroup, Row)>) -> Vec<Row> {
    let mut previous = None;
    entries
        .into_iter()
        .map(|(group, row)| {
            let heading = (previous != Some(group)).then_some(Heading {
                group,
                first: previous.is_none(),
            });
            previous = Some(group);
            Row { heading, ..row }
        })
        .collect()
}

impl State {
    /// The entries of `view` in `pane`.
    pub(super) fn view_rows(&self, pane: &Pane, view: SpecialView) -> Vec<Row> {
        match view {
            SpecialView::Today => self.today_rows(pane),
            SpecialView::Upcoming => self.upcoming_rows(pane),
            SpecialView::Overdue => self.overdue_rows(pane),
            SpecialView::Colors => self.colors_rows(pane),
            SpecialView::Logbook => self.logbook_rows(pane),
            SpecialView::Starred | SpecialView::Flagged => {
                self.matching_rows(pane, |block| self.lists(view, block))
            }
        }
    }

    /// Whether `view` lists `block`, by what its line says; the Overdue view also leaves out done
    /// blocks and what is inside them (D-82). The Navigator counts the open tasks it lists by
    /// this.
    pub(super) fn lists(&self, view: SpecialView, block: &Block) -> bool {
        match view {
            SpecialView::Today => for_today(block, self.today, self.today_starred),
            SpecialView::Upcoming => upcoming_day(block.start(), block.due(), self.today).is_some(),
            SpecialView::Overdue => block.due().is_some_and(|due| due < self.today),
            SpecialView::Colors => block.color().is_some(),
            SpecialView::Logbook => in_logbook(block, self.today),
            SpecialView::Starred => block.starred(),
            SpecialView::Flagged => block.flag(),
        }
    }

    /// Every block `pane` shows for which `lists` holds, in document order, each at depth 0 with
    /// the path of the blocks above it. Completed blocks, each with everything inside it, are
    /// left out while hidden, as in the outline (FR-DONE-02); special nodes never show
    /// (FR-SPC-01).
    pub(super) fn matching_rows(&self, pane: &Pane, lists: impl Fn(&Block) -> bool) -> Vec<Row> {
        self.matching(pane, pane.show_done, lists)
    }

    /// The rows [`State::matching_rows`] gives, completed blocks and what is inside them
    /// included whatever the pane's show-completed setting (D-82, D-84).
    pub(super) fn matching_rows_with_done(
        &self,
        pane: &Pane,
        lists: impl Fn(&Block) -> bool,
    ) -> Vec<Row> {
        self.matching(pane, true, lists)
    }

    /// The entries in `pane` of the blocks for which `lists` holds among the outline's rows with
    /// nothing collapsed, completed blocks shown when `show_done` says so: the rows a change
    /// builds again only where it changed, gone through rather than the document's tree.
    fn matching(&self, pane: &Pane, show_done: bool, lists: impl Fn(&Block) -> bool) -> Vec<Row> {
        let mut every = Pane::first();
        every.show_done = show_done;
        let mut above = Above::default();
        let mut entries = Vec::new();
        for row in &self.rows_folded(&every, &HashSet::new()) {
            above.reach(row.depth);
            if blocks::block(&self.document, row.id).is_some_and(|block| lists(&block)) {
                entries.push(Row {
                    depth: 0,
                    selected: pane.selection.contains(row.id),
                    path: above.path(self),
                    ..row.clone()
                });
            }
            above.enter(row.id);
        }
        entries
    }

    /// `message` as the pane takes it where it is: in a special view an intent that needs the
    /// outline's tree around the focused block, such as folding, moving to a parent, child or
    /// sibling, or adding, pasting or indenting blocks, is `None`, as is a paste outside
    /// edit mode and the date prompt, and ending an edit with `Enter` only stops editing (navigation.md §7).
    pub(super) fn as_located(&self, message: Message) -> Option<Message> {
        if let Location::Outline(_) = self.pane.location {
            return Some(message);
        }
        match message {
            Message::NewNext => Some(Message::StopEditing),
            Message::Paste(_) | Message::Pasted { .. }
                if self.edit.is_none() && self.date_prompt.is_none() =>
            {
                None
            }
            message if needs_the_tree(&message) => None,
            message => Some(message),
        }
    }

    /// Goes from a special view's entry `block` to it in the outline: zooms to its parent and
    /// focuses it, as Go to Symbol does (navigation.md §7, §8).
    pub(super) fn go_to_entry(&mut self, block: &Block) {
        let parent = blocks::parent(&self.document, block.id()).map(|parent| parent.id());
        self.go_to(Location::Outline(parent));
        self.pane.focus = Some(block.id());
    }

    /// Moves the selected entries, else the focused one, a step up or down with `down` in
    /// `view`: only Today has an order of the user's, and the other views keep their own
    /// (D-57, D-80).
    pub(super) fn move_entries(&mut self, view: SpecialView, down: bool) {
        match view {
            SpecialView::Today => self.reorder_entries(down),
            SpecialView::Upcoming
            | SpecialView::Overdue
            | SpecialView::Colors
            | SpecialView::Logbook
            | SpecialView::Starred
            | SpecialView::Flagged => {}
        }
    }
}

/// Whether `message` needs the outline's tree around the focused block, which a special view
/// does not show (navigation.md §7).
fn needs_the_tree(message: &Message) -> bool {
    match message {
        Message::FocusParent
        | Message::FocusNextSibling
        | Message::FocusPreviousSibling
        | Message::FocusLastChild
        | Message::Collapse
        | Message::Expand
        | Message::ToggleFold
        | Message::ToggleSubtree
        | Message::ExpandLevel
        | Message::CollapseLevel
        | Message::ExpandAll
        | Message::CollapseAll
        | Message::New(..)
        | Message::Group
        | Message::Indent
        | Message::Outdent => true,
        Message::NewNext
        | Message::Started
        | Message::Tick { .. }
        | Message::Saved { .. }
        | Message::SaveFailed { .. }
        | Message::SaveRefused { .. }
        | Message::CheckFile
        | Message::FileChecked { .. }
        | Message::KeepMine
        | Message::TakeTheirs
        | Message::ToggleDiff
        | Message::ConflictCopied { .. }
        | Message::ConflictCopyFailed { .. }
        | Message::BackupFailed { .. }
        | Message::ConfigProblems(_)
        | Message::Configure(_)
        | Message::Quit
        | Message::Terminate
        | Message::FocusNext
        | Message::FocusPrevious
        | Message::FocusFirst
        | Message::FocusLast
        | Message::PageDown { .. }
        | Message::PageUp { .. }
        | Message::Focus(_)
        | Message::GoToCrumb(_)
        | Message::OpenPaneMenu(_)
        | Message::ToggleShowDone
        | Message::ToggleShortcuts
        | Message::CloseShortcuts
        | Message::ZoomIn
        | Message::ZoomOut
        | Message::GoTo(_)
        | Message::Back
        | Message::Forward
        | Message::ToggleNavigator
        | Message::Navigator(_)
        | Message::ToggleDone { .. }
        | Message::ChangeDate { .. }
        | Message::ToggleFlag(_)
        | Message::CycleColor
        | Message::SetColor(_)
        | Message::OpenColorPicker
        | Message::QuickAdd(_)
        | Message::SetSection(_)
        | Message::OpenDatePrompt { .. }
        | Message::DatePrompt(_)
        | Message::ClearDate(_)
        | Message::OpenTypePicker
        | Message::OpenDetails
        | Message::CloseDetails
        | Message::OpenGoToSymbol(_)
        | Message::OpenMoveTo
        | Message::OpenGoToCommand(_)
        | Message::OpenThemePicker { .. }
        | Message::Settings(_)
        | Message::Count
        | Message::SaveAs(_)
        | Message::OpenContextMenu
        | Message::Undo
        | Message::Redo
        | Message::EditAtEnd
        | Message::Text(_)
        | Message::StopEditing
        | Message::OpenCommandMenu { .. }
        | Message::Menu(..)
        | Message::CycleType
        | Message::EditPrevious
        | Message::EditNext
        | Message::Delete
        | Message::Copy
        | Message::Cut
        | Message::Paste(_)
        | Message::Pasted { .. }
        | Message::Duplicate
        | Message::MoveUp
        | Message::MoveDown
        | Message::OpenUrl
        | Message::OpenUrlFailed { .. }
        | Message::Select(_)
        | Message::Pane(_)
        | Message::ResizePane(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Flag, format};

    /// Starred blocks at several depths among blocks that are not: dated today, a special node, a
    /// done block and blocks inside a done project.
    const OUTLINE: &str = "\
Home:
\t- Water plants @star
\t- Paint the hall @due(2026-10-01)
\tErrands:
\t\t- Bank @star @done(2026-10-04)
\t\t\tC @hidden @star
Work: @done @star
\t- Report @star
\t- Slides
- Plan the week @star
";

    /// `text` in the Starred view.
    fn starred(text: &str) -> State {
        let mut state = State::new(format::parse(text), None);
        state.go_to(Location::View(SpecialView::Starred));
        assert_eq!(state.location(), Location::View(SpecialView::Starred));
        state
    }

    /// Each row's text and path, joined as `path › text`.
    fn listed(state: &State) -> Vec<String> {
        state
            .rows()
            .iter()
            .map(|row| {
                assert_eq!(row.depth, 0, "entries are flat");
                row.path
                    .iter()
                    .map(|segment| &**segment)
                    .chain([&*row.text])
                    .collect::<Vec<_>>()
                    .join(" › ")
            })
            .collect()
    }

    // D-80, D-92
    #[test]
    fn starred_follows_the_other_views_in_every_list_of_them() {
        assert_eq!(
            SpecialView::ALL,
            [
                SpecialView::Today,
                SpecialView::Upcoming,
                SpecialView::Overdue,
                SpecialView::Colors,
                SpecialView::Logbook,
                SpecialView::Starred,
                SpecialView::Flagged
            ]
        );
        assert_eq!(SpecialView::Starred.name(), "Starred");
    }

    // FR-TAG-13, D-92, FR-SPC-01
    #[test]
    fn starred_lists_every_starred_block_flat_in_document_order_with_its_path() {
        let state = starred(OUTLINE);
        assert_eq!(
            listed(&state),
            [
                "Home › Water plants",
                "Home › Errands › Bank",
                "Work",
                "Work › Report",
                "Plan the week",
            ]
        );
        assert!(state.rows().iter().all(|row| row.starred));
    }

    // FR-DONE-02, D-92
    #[test]
    fn starred_leaves_out_hidden_completed_blocks_with_everything_inside_them() {
        let mut state = starred(OUTLINE);
        state.update(Message::ToggleShowDone);
        assert_eq!(listed(&state), ["Home › Water plants", "Plan the week"]);
    }

    // FR-KEY-05, D-92
    #[test]
    fn a_selected_entry_shows_selected() {
        let mut state = starred("- a @star\n- b @star\n");
        state.update(Message::Select(crate::app::SelectionChange::Toggle));
        let selected: Vec<bool> = state.rows().iter().map(|row| row.selected).collect();
        assert_eq!(selected, [true, false]);
    }

    // D-92
    #[test]
    fn an_entry_whose_star_is_removed_leaves_starred() {
        let mut state = starred("- a @star\n- b @star\n");
        state.update(Message::ToggleFlag(Flag::Star));
        assert_eq!(listed(&state), ["b"]);
        assert_eq!(format::serialize(state.document()), "- a\n- b @star\n");
    }

    // D-80, D-92
    #[test]
    fn moving_an_entry_up_or_down_leaves_starred_as_it_is() {
        let mut state = starred("- a @star\n- b @star\n- c @star\n");
        state.update(Message::FocusNext);
        for message in [Message::MoveDown, Message::MoveUp] {
            assert_eq!(state.update(message), []);
            assert_eq!(listed(&state), ["a", "b", "c"]);
        }
        assert_eq!(
            format::serialize(state.document()),
            "- a @star\n- b @star\n- c @star\n"
        );
    }
    /// `text` in the Flagged view.
    fn flagged(text: &str) -> State {
        let mut state = State::new(format::parse(text), None);
        state.go_to(Location::View(SpecialView::Flagged));
        state
    }

    /// Flagged blocks at several depths, with and without a value, among blocks that are not: a
    /// special node, a done project and a block with other tags.
    const FLAGGED: &str = "\
Home: @flag
\t- Water plants @star
\t- Paint the **hall** @flag(soon)
\tErrands:
\t\t- Post office @flag
\t\t\tC @hidden @flag
Work: @done
\t- Report @flag
- Plan the week @flagged @red
";

    // D-93
    #[test]
    fn flagged_comes_after_the_other_views() {
        assert_eq!(SpecialView::ALL.last(), Some(&SpecialView::Flagged));
        assert_eq!(SpecialView::Flagged.name(), "Flagged");
    }

    // FR-TAG-14, D-93, FR-SPC-01
    #[test]
    fn flagged_lists_every_flagged_block_flat_in_document_order_with_its_path() {
        let state = flagged(FLAGGED);
        assert_eq!(
            listed(&state),
            [
                "Home",
                "Home › Paint the **hall**",
                "Home › Errands › Post office",
                "Work › Report",
            ]
        );
        assert_eq!(state.breadcrumbs(), Vec::<String>::new());
    }

    // FR-TAG-14, D-93, FR-DONE-02
    #[test]
    fn flagged_follows_the_panes_show_completed_setting() {
        let mut state = flagged(FLAGGED);
        state.update(Message::ToggleShowDone);
        assert_eq!(
            listed(&state),
            [
                "Home",
                "Home › Paint the **hall**",
                "Home › Errands › Post office"
            ],
            "a done block leaves with every block inside it"
        );
    }

    // FR-TAG-14, D-93: an entry whose flag is toggled off leaves the list.
    #[test]
    fn an_entry_that_loses_its_flag_leaves_flagged() {
        let mut state = flagged("- a @flag\n- b @flag\n");
        state.update(Message::ToggleFlag(Flag::Flagged));
        assert_eq!(listed(&state), ["b"]);
        state.update(Message::Undo);
        assert_eq!(listed(&state), ["a", "b"]);
    }

    // D-80, D-93: only Today has an order of the user's.
    #[test]
    fn moving_entries_in_flagged_changes_nothing() {
        let mut state = flagged("- a @flag\n- b @flag\n");
        state.update(Message::FocusLast);
        let view_state = state.view_state();
        for message in [Message::MoveUp, Message::MoveDown] {
            assert_eq!(state.update(message), []);
            assert_eq!(listed(&state), ["a", "b"]);
        }
        assert_eq!(state.document().revision(), 0);
        assert_eq!(state.view_state(), view_state, "no order is kept");
    }
}
