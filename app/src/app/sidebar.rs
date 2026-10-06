//! The navigation sidebar: its entries, which one has the keys while it is focused and which
//! project entries are folded ([navigation.md §6](../../../docs/specs/navigation.md#6-sidebar-fr-ui-01-d-7-d-11-d-28),
//! FR-UI-01).

use std::collections::HashSet;

use super::{
    Location, PaneChange, State,
    commands::{Command, NodeCommand},
};
use crate::document::{BlockKind, ItemId, blocks};

/// Whether the sidebar is shown, its focused entry while it has the keys, and its folded project
/// entries, which are its own and not the outline's.
#[derive(Debug, Clone)]
pub(super) struct Sidebar {
    shown: bool,
    /// The index of the focused entry, while the sidebar has the keys.
    focus: Option<usize>,
    folds: HashSet<ItemId>,
}

impl Sidebar {
    /// Whether it is shown.
    pub(super) fn shown(&self) -> bool {
        self.shown
    }

    /// Shows it, or hides it when `shown` is false, without giving it the keys.
    pub(super) fn show(&mut self, shown: bool) {
        self.shown = shown;
        self.focus = None;
    }

    /// Keeps each folded project entry folded on the block `rematch` finds for its block in a
    /// reloaded document, and drops the fold when it finds none (file-io.md §3).
    pub(super) fn rematch_folds(&mut self, rematch: impl Fn(ItemId) -> Option<ItemId>) {
        self.folds = self.folds.iter().filter_map(|&id| rematch(id)).collect();
    }
}

impl Default for Sidebar {
    /// Shown, without the keys, every project entry expanded.
    fn default() -> Self {
        Self {
            shown: true,
            focus: None,
            folds: HashSet::new(),
        }
    }
}

/// A key of the focused sidebar ([keybindings.md §4](../../../docs/specs/keybindings.md#4-other-contexts)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarInput {
    /// Focus the entry below.
    Next,
    /// Focus the entry above.
    Previous,
    /// Fold the focused project entry, hiding its child projects.
    Collapse,
    /// Unfold the focused project entry.
    Expand,
    /// Zoom the pane to the focused entry and give the pane the keys.
    Choose,
    /// Give the pane the keys.
    Leave,
}

/// One entry of the sidebar, in its sections' order: bookmarks, Today, projects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarEntry {
    /// A block with `@bookmark` (D-11).
    Bookmark {
        /// The block.
        id: ItemId,
        /// Its text as shown, without formatting delimiters.
        text: String,
    },
    /// The built-in Today view (D-7).
    Today,
    /// A project, nested under the closest project above it.
    Project {
        /// The block.
        id: ItemId,
        /// Its text as shown, without formatting delimiters.
        text: String,
        /// How many projects above it nest it, starting at 0.
        depth: usize,
        /// Whether its child projects are listed.
        fold: Fold,
    },
}

impl SidebarEntry {
    /// Where choosing the entry goes: into its block, or the Today view.
    fn location(&self) -> Location {
        match self {
            Self::Bookmark { id, .. } | Self::Project { id, .. } => Location::Outline(Some(*id)),
            Self::Today => Location::Today,
        }
    }
}

/// Whether a project entry's child projects are listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fold {
    /// It has no child projects.
    Leaf,
    /// They are listed below it.
    Expanded,
    /// They are hidden.
    Collapsed,
}

/// The shown sidebar as a frontend draws it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarView {
    /// Its entries in order; Today is always among them.
    pub entries: Vec<SidebarEntry>,
    /// The index of the focused entry, while the sidebar has the keys.
    pub focused: Option<usize>,
}

impl State {
    /// The sidebar, or `None` while it is hidden (FR-UI-01).
    #[must_use]
    pub fn sidebar(&self) -> Option<SidebarView> {
        if !self.sidebar.shown {
            return None;
        }
        let entries = self.sidebar_entries();
        let focused = self.sidebar_focus(&entries);
        Some(SidebarView { entries, focused })
    }

    /// Whether the sidebar has the keys rather than the pane.
    #[must_use]
    pub fn sidebar_focused(&self) -> bool {
        self.sidebar.focus.is_some()
    }

    /// `Ctrl+B`: shows the sidebar, or hides it and gives the pane the keys.
    pub(super) fn toggle_sidebar(&mut self) {
        self.sidebar.show(!self.sidebar.shown);
    }

    /// `Alt+0`: shows the sidebar and gives it the keys, on its first entry unless it has them.
    pub(super) fn focus_sidebar(&mut self) {
        self.sidebar.shown = true;
        self.sidebar.focus.get_or_insert(0);
    }

    /// Applies a key of the focused sidebar.
    pub(super) fn sidebar_input(&mut self, input: SidebarInput) {
        let entries = self.sidebar_entries();
        let Some(at) = self.sidebar_focus(&entries) else {
            return;
        };
        let focused = entries.get(at);
        match input {
            SidebarInput::Next => {
                self.sidebar.focus =
                    Some(at.saturating_add(1).min(entries.len().saturating_sub(1)));
            }
            SidebarInput::Previous => self.sidebar.focus = Some(at.saturating_sub(1)),
            SidebarInput::Collapse => {
                if let Some(&SidebarEntry::Project {
                    id,
                    fold: Fold::Expanded,
                    ..
                }) = focused
                {
                    self.sidebar.folds.insert(id);
                }
            }
            SidebarInput::Expand => {
                if let Some(SidebarEntry::Project { id, .. }) = focused {
                    self.sidebar.folds.remove(id);
                }
            }
            SidebarInput::Choose => {
                if let Some(location) = focused.map(SidebarEntry::location) {
                    self.sidebar.focus = None;
                    self.go_to(location);
                }
            }
            SidebarInput::Leave => self.sidebar.focus = None,
        }
    }

    /// Where the focused entry goes, while the sidebar has the keys.
    pub(super) fn sidebar_location(&self) -> Option<Location> {
        let entries = self.sidebar_entries();
        entries
            .get(self.sidebar_focus(&entries)?)
            .map(SidebarEntry::location)
    }

    /// The commands the context menu offers on the focused entry while the sidebar has the keys:
    /// going to it, opening it in a new pane and folding a project entry with child projects, then leaving or hiding the
    /// sidebar (FR-KEY-06).
    pub(super) fn sidebar_commands(&self) -> Option<Vec<Vec<NodeCommand>>> {
        let entries = self.sidebar_entries();
        let entry = entries.get(self.sidebar_focus(&entries)?)?;
        let fold = match entry {
            SidebarEntry::Project {
                fold: Fold::Expanded,
                ..
            } => Some(SidebarInput::Collapse),
            SidebarEntry::Project {
                fold: Fold::Collapsed,
                ..
            } => Some(SidebarInput::Expand),
            SidebarEntry::Project {
                fold: Fold::Leaf, ..
            }
            | SidebarEntry::Bookmark { .. }
            | SidebarEntry::Today => None,
        };
        let entry = [NodeCommand::named(Command::Sidebar(SidebarInput::Choose))]
            .into_iter()
            .chain([NodeCommand::named(Command::Pane(PaneChange::Open))])
            .chain(fold.map(|input| NodeCommand::named(Command::Sidebar(input))))
            .collect();
        let sidebar = vec![
            NodeCommand::named(Command::Sidebar(SidebarInput::Leave)),
            NodeCommand::labelled("Hide Sidebar", Command::ToggleSidebar),
        ];
        Some(vec![entry, sidebar])
    }

    /// The focused entry's index among `entries`, kept on the last one when entries went away.
    fn sidebar_focus(&self, entries: &[SidebarEntry]) -> Option<usize> {
        self.sidebar
            .focus
            .map(|at| at.min(entries.len().saturating_sub(1)))
    }

    /// Every bookmarked block in document order, then Today, then the project tree.
    fn sidebar_entries(&self) -> Vec<SidebarEntry> {
        let root = self.document().root();
        let mut entries = Vec::new();
        self.push_bookmarks(root, &mut entries);
        entries.push(SidebarEntry::Today);
        entries.extend(self.projects(root, 0));
        entries
    }

    /// Appends an entry for each bookmarked block below `line`, depth first, but not for
    /// completed blocks and what they hold while those are hidden (FR-DONE-02).
    fn push_bookmarks(&self, line: ItemId, out: &mut Vec<SidebarEntry>) {
        for block in blocks::children(self.document(), line) {
            if !self.shows(&block) {
                continue;
            }
            if block.bookmark() {
                out.push(SidebarEntry::Bookmark {
                    id: block.id(),
                    text: self.shown_text(&block),
                });
            }
            self.push_bookmarks(block.id(), out);
        }
    }

    /// The project entries below the line `line` at `depth`, skipping the blocks between
    /// projects, the projects inside folded entries, and completed blocks with what they hold
    /// while those are hidden (FR-DONE-02).
    fn projects(&self, line: ItemId, depth: usize) -> Vec<SidebarEntry> {
        let mut out = Vec::new();
        for block in blocks::children(self.document(), line) {
            if !self.shows(&block) {
                continue;
            }
            if block.kind() != BlockKind::Project {
                out.extend(self.projects(block.id(), depth));
                continue;
            }
            let below = self.projects(block.id(), depth + 1);
            let fold = if below.is_empty() {
                Fold::Leaf
            } else if self.sidebar.folds.contains(&block.id()) {
                Fold::Collapsed
            } else {
                Fold::Expanded
            };
            out.push(SidebarEntry::Project {
                id: block.id(),
                text: self.shown_text(&block),
                depth,
                fold,
            });
            if fold == Fold::Expanded {
                out.extend(below);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::{Message, Mode},
        document::{Document, Flag, format},
    };

    /// Bookmarks at two depths, one formatted and one inside a special node, and projects
    /// nested through a task and a note.
    const OUTLINE: &str = "\
Work:
\t- **Launch** plan @bookmark
\t- Report
\t\tDrafts:
\t\t\tOld:
\tNotes
\t\tIdeas:
Home: @bookmark
\tSecrets @hidden
\t\tVault: @bookmark
- Errand
";

    fn open(text: &str) -> State {
        State::new(format::parse(text), None)
    }

    /// Each entry as `kind text`, project entries indented 2 spaces per level behind their fold.
    fn listed(state: &State) -> Vec<String> {
        let view = state.sidebar().expect("the sidebar is shown");
        view.entries
            .iter()
            .map(|entry| match entry {
                SidebarEntry::Bookmark { text, .. } => format!("bookmark {text}"),
                SidebarEntry::Today => "today".to_owned(),
                SidebarEntry::Project {
                    text, depth, fold, ..
                } => format!("{}{fold:?} {text}", "  ".repeat(*depth)),
            })
            .collect()
    }

    fn focused(state: &State) -> Option<usize> {
        state.sidebar().and_then(|view| view.focused)
    }

    fn input(state: &mut State, inputs: &[SidebarInput]) {
        for &input in inputs {
            state.update(Message::Sidebar(input));
        }
    }

    // FR-UI-01, navigation §6
    #[test]
    fn lists_bookmarks_in_document_order_then_today_then_the_project_tree() {
        assert_eq!(
            listed(&open(OUTLINE)),
            [
                "bookmark Launch plan",
                "bookmark Home",
                "today",
                "Expanded Work",
                "  Expanded Drafts",
                "    Leaf Old",
                "  Leaf Ideas",
                "Leaf Home",
            ]
        );
    }

    // FR-UI-01, FR-DONE-02
    #[test]
    fn hidden_completed_blocks_take_their_bookmarks_and_projects_with_them() {
        let mut state = open(
            "Done: @done\n\tInner:\n\t- kept @bookmark\n- x @done @bookmark\nOpen:\n\t- y @done\n\t\tDeep:",
        );
        let all = listed(&state);
        state.update(Message::ToggleShowDone);
        assert_eq!(listed(&state), ["today", "Leaf Open"]);
        state.update(Message::ToggleShowDone);
        assert_eq!(listed(&state), all);
        assert_eq!(all.len(), 7, "{all:?}");
    }

    // FR-UI-01
    #[test]
    fn an_outline_without_bookmarks_or_projects_lists_only_today() {
        assert_eq!(listed(&open("- a\nb")), ["today"]);
        assert_eq!(listed(&State::new(Document::default(), None)), ["today"]);
    }

    // FR-UI-01
    #[test]
    fn the_sidebar_is_shown_until_toggled_and_shown_again_by_the_next_toggle() {
        let mut state = open(OUTLINE);
        assert!(state.sidebar().is_some(), "shown by default");
        state.update(Message::ToggleSidebar);
        assert_eq!(state.sidebar(), None);
        state.update(Message::ToggleSidebar);
        assert_eq!(focused(&state), None, "shown without the keys");
        assert!(state.sidebar().is_some());
    }

    // FR-UI-01
    #[test]
    fn focusing_shows_the_sidebar_and_gives_its_first_entry_the_keys() {
        let mut state = open(OUTLINE);
        state.update(Message::ToggleSidebar);
        assert!(!state.sidebar_focused());
        state.update(Message::FocusSidebar);
        assert!(state.sidebar_focused());
        assert_eq!(focused(&state), Some(0));
        input(&mut state, &[SidebarInput::Next]);
        state.update(Message::FocusSidebar);
        assert_eq!(focused(&state), Some(1), "focusing again keeps the entry");
    }

    // FR-UI-01
    #[test]
    fn hiding_the_sidebar_gives_the_pane_the_keys() {
        let mut state = open(OUTLINE);
        state.update(Message::FocusSidebar);
        state.update(Message::ToggleSidebar);
        assert!(!state.sidebar_focused());
        state.update(Message::ToggleSidebar);
        assert_eq!(focused(&state), None);
    }

    // FR-MODE-01
    #[test]
    fn focusing_the_sidebar_ends_editing() {
        let mut state = open(OUTLINE);
        state.update(Message::EditAtEnd);
        state.update(Message::FocusSidebar);
        assert_eq!(state.mode(), Mode::Navigate);
    }

    // keybindings.md §4
    #[test]
    fn next_and_previous_move_the_focus_and_stop_at_the_ends() {
        let mut state = open(OUTLINE);
        state.update(Message::FocusSidebar);
        input(&mut state, &[SidebarInput::Previous]);
        assert_eq!(focused(&state), Some(0));
        input(&mut state, &[SidebarInput::Next, SidebarInput::Next]);
        assert_eq!(focused(&state), Some(2));
        input(&mut state, &[SidebarInput::Previous]);
        assert_eq!(focused(&state), Some(1));
        input(&mut state, &[SidebarInput::Next; 10]);
        assert_eq!(focused(&state), Some(7), "the last entry");
    }

    // keybindings.md §4
    #[test]
    fn keys_of_the_sidebar_do_nothing_until_it_is_focused() {
        let mut state = open(OUTLINE);
        input(&mut state, &[SidebarInput::Next, SidebarInput::Collapse]);
        assert_eq!(focused(&state), None);
        assert_eq!(listed(&state)[3], "Expanded Work");
    }

    // keybindings.md §4
    #[test]
    fn collapse_and_expand_fold_project_entries_only() {
        let mut state = open(OUTLINE);
        state.update(Message::FocusSidebar);
        input(&mut state, &[SidebarInput::Next; 3]);
        input(&mut state, &[SidebarInput::Collapse]);
        assert_eq!(
            listed(&state)[2..],
            ["today", "Collapsed Work", "Leaf Home"],
            "child projects hide"
        );
        input(&mut state, &[SidebarInput::Collapse]);
        assert_eq!(listed(&state)[3], "Collapsed Work", "still collapsed");
        input(&mut state, &[SidebarInput::Expand]);
        assert_eq!(listed(&state)[4], "  Expanded Drafts");
        input(&mut state, &[SidebarInput::Expand]);
        assert_eq!(listed(&state)[3], "Expanded Work", "still expanded");
        input(&mut state, &[SidebarInput::Next, SidebarInput::Next]);
        input(&mut state, &[SidebarInput::Collapse]);
        assert_eq!(
            listed(&state)[5],
            "    Leaf Old",
            "a leaf has nothing to fold"
        );
        for at in [0, 2] {
            let mut state = open(OUTLINE);
            state.update(Message::FocusSidebar);
            input(&mut state, &vec![SidebarInput::Next; at]);
            input(&mut state, &[SidebarInput::Collapse, SidebarInput::Expand]);
            assert_eq!(listed(&state), listed(&open(OUTLINE)), "entry {at}");
        }
    }

    /// The display text of the zoom block, or `None` at home.
    fn zoomed(state: &State) -> Option<String> {
        state.breadcrumbs().last().cloned()
    }

    // FR-UI-01, navigation §6
    #[test]
    fn choosing_a_bookmark_or_project_zooms_the_pane_to_it_and_gives_the_pane_the_keys() {
        let mut state = open(OUTLINE);
        state.update(Message::FocusSidebar);
        input(&mut state, &[SidebarInput::Choose]);
        assert_eq!(zoomed(&state).as_deref(), Some("Launch plan"));
        assert!(!state.sidebar_focused());
        state.update(Message::FocusSidebar);
        input(&mut state, &[SidebarInput::Next; 4]);
        input(&mut state, &[SidebarInput::Choose]);
        assert_eq!(zoomed(&state).as_deref(), Some("Drafts"));
        state.update(Message::Back);
        assert_eq!(
            zoomed(&state).as_deref(),
            Some("Launch plan"),
            "each choice is a step of the zoom history"
        );
    }

    // D-7, navigation §6, §7
    #[test]
    fn choosing_today_shows_the_today_view_and_gives_the_pane_the_keys() {
        let mut state = open(OUTLINE);
        state.update(Message::FocusSidebar);
        input(&mut state, &[SidebarInput::Choose]);
        state.update(Message::FocusSidebar);
        input(&mut state, &[SidebarInput::Next, SidebarInput::Next]);
        input(&mut state, &[SidebarInput::Choose]);
        assert_eq!(state.location(), Location::Today);
        assert!(!state.sidebar_focused());
        state.update(Message::Back);
        assert_eq!(
            zoomed(&state).as_deref(),
            Some("Launch plan"),
            "Today is a step of the zoom history"
        );
    }

    // keybindings.md §4
    #[test]
    fn leaving_gives_the_pane_the_keys_on_the_same_view() {
        let mut state = open(OUTLINE);
        state.update(Message::FocusSidebar);
        input(&mut state, &[SidebarInput::Next, SidebarInput::Leave]);
        assert!(!state.sidebar_focused());
        assert_eq!(focused(&state), None);
        assert_eq!(zoomed(&state), None);
        assert!(state.sidebar().is_some(), "still shown");
    }

    // FR-UI-01
    #[test]
    fn the_focus_stays_on_the_last_entry_when_entries_go_away() {
        let mut state = open("- a @bookmark\nP:");
        state.update(Message::FocusSidebar);
        input(&mut state, &[SidebarInput::Next, SidebarInput::Next]);
        assert_eq!(focused(&state), Some(2));
        state.update(Message::ToggleFlag(Flag::Bookmark));
        assert_eq!(listed(&state), ["today", "Leaf P"]);
        assert_eq!(focused(&state), Some(1));
        input(&mut state, &[SidebarInput::Previous]);
        assert_eq!(focused(&state), Some(0));
    }

    /// The labels of the focused entry's context-menu commands, each group's joined by `, `.
    fn entry_commands(state: &State) -> Option<Vec<String>> {
        let groups = state.sidebar_commands()?;
        Some(
            groups
                .iter()
                .map(|group| {
                    group
                        .iter()
                        .map(|command| command.label.as_ref())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .collect(),
        )
    }

    // FR-KEY-06
    #[test]
    fn the_focused_entrys_commands_go_to_it_open_it_fold_it_and_leave_or_hide_the_sidebar() {
        let mut state = open(OUTLINE);
        assert_eq!(entry_commands(&state), None, "the pane has the keys");
        state.update(Message::FocusSidebar);
        let leave = "Return to Outline, Hide Sidebar".to_owned();
        assert_eq!(
            entry_commands(&state),
            Some(vec![
                "Go to Entry, Open in New Pane".to_owned(),
                leave.clone()
            ]),
            "a bookmark"
        );
        input(&mut state, &[SidebarInput::Next, SidebarInput::Next]);
        assert_eq!(
            entry_commands(&state),
            Some(vec![
                "Go to Entry, Open in New Pane".to_owned(),
                leave.clone()
            ]),
            "Today"
        );
        input(&mut state, &[SidebarInput::Next]);
        assert_eq!(
            entry_commands(&state),
            Some(vec![
                "Go to Entry, Open in New Pane, Collapse Entry".to_owned(),
                leave.clone()
            ])
        );
        input(&mut state, &[SidebarInput::Collapse]);
        assert_eq!(
            entry_commands(&state),
            Some(vec![
                "Go to Entry, Open in New Pane, Expand Entry".to_owned(),
                leave.clone()
            ])
        );
        input(&mut state, &[SidebarInput::Next]);
        assert_eq!(
            entry_commands(&state),
            Some(vec!["Go to Entry, Open in New Pane".to_owned(), leave]),
            "a project without child projects"
        );
        let groups = state.sidebar_commands().unwrap();
        assert_eq!(groups[1][1].command, Command::ToggleSidebar);
        assert_eq!(groups[0][0].command, Command::Sidebar(SidebarInput::Choose));
    }
}
