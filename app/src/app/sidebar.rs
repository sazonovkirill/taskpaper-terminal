//! The navigation sidebar, which the screen shows as the Navigator, a window that `Ctrl+G` opens
//! over the panes (D-79): its entries, which one has the keys while it is open and which project entries are folded ([navigation.md §6](../../../docs/specs/navigation.md#6-navigator-fr-ui-01-d-7-d-11-d-28-d-79),
//! FR-UI-01).

use std::{collections::HashSet, rc::Rc};

use super::{
    Location, Notice, PaneChange, Rows, SpecialView, State,
    commands::{Command, NodeCommand},
    memo::Memo,
    pane::Pane,
};
use crate::document::{BlockKind, ItemId, Stamp, blocks};

/// The text of the project the sidebar lists apart, above Today (D-75).
const INBOX: &str = "INBOX";

/// The sidebar's focused entry while it is open, its folded project entries, which are its own
/// and not the outline's, whether it lists the `INBOX` project apart and which special views it
/// leaves out.
#[derive(Debug, Clone)]
pub(super) struct Sidebar {
    special_inbox: bool,
    unlisted: HashSet<SpecialView>,
    /// The index of the focused entry, while the sidebar is open, which it is only while it has
    /// the keys (D-79).
    focus: Option<usize>,
    folds: HashSet<ItemId>,
    entries: Memo<EntriesKey, Rc<[SidebarEntry]>>,
    current: Memo<CurrentKey, Option<usize>>,
}

/// Everything the sidebar's entries are computed from.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EntriesKey {
    document: Stamp,
    folds: HashSet<ItemId>,
    show_done: bool,
    special_inbox: bool,
    unlisted: HashSet<SpecialView>,
}

/// Everything the entry of where the focused pane is comes from: the entries, the blocks above
/// the pane's zoom block and the pane's location.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CurrentKey {
    document: Stamp,
    entries: Rc<[SidebarEntry]>,
    location: Location,
}

impl Sidebar {
    /// Lists the `INBOX` project above Today rather than among the projects, or not when `on` is
    /// false (navigation.md §6).
    pub(super) fn set_special_inbox(&mut self, on: bool) {
        self.special_inbox = on;
    }

    /// Lists `view`, or not when `on` is false (navigation.md §6).
    pub(super) fn set_listed(&mut self, view: SpecialView, on: bool) {
        if on {
            self.unlisted.remove(&view);
        } else {
            self.unlisted.insert(view);
        }
    }

    /// Keeps each folded project entry folded on the block `rematch` finds for its block in a
    /// reloaded document, and drops the fold when it finds none (file-io.md §3).
    pub(super) fn rematch_folds(&mut self, rematch: impl Fn(ItemId) -> Option<ItemId>) {
        self.folds = self.folds.iter().filter_map(|&id| rematch(id)).collect();
    }
}

impl Default for Sidebar {
    /// Closed, every project entry expanded, the `INBOX` project apart, every special view listed.
    fn default() -> Self {
        Self {
            special_inbox: true,
            unlisted: HashSet::new(),
            focus: None,
            folds: HashSet::new(),
            entries: Memo::default(),
            current: Memo::default(),
        }
    }
}

/// A key of the open sidebar ([keybindings.md §4](../../../docs/specs/keybindings.md#4-other-contexts)).
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
    /// Zoom the pane to the focused entry and close the sidebar.
    Choose,
    /// Close the sidebar, giving the pane the keys.
    Leave,
    /// Focus the entry at this index while the sidebar is open, as a click on it does
    /// (FR-KEY-05).
    Focus(usize),
    /// `gi`: zoom the pane to the inbox entry's project, from anywhere, or say there is none
    /// (D-78).
    GoToInbox,
}

/// One entry of the sidebar, in its sections' order: the inbox, the special views, bookmarks,
/// projects (D-68, D-75, D-80).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarEntry {
    /// The top-level project `INBOX`, apart from the other projects (D-75).
    Inbox {
        /// The block.
        id: ItemId,
        /// Its text as shown, without formatting delimiters.
        text: String,
        /// How many open tasks it holds, as a project entry counts them (D-73).
        open_tasks: usize,
    },
    /// A block with `@bookmark` (D-11).
    Bookmark {
        /// The block.
        id: ItemId,
        /// Its text as shown, without formatting delimiters.
        text: String,
    },
    /// A special view, such as Today (D-7, D-80).
    View(SpecialView),
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
        /// How many open tasks it holds at any depth, counted as a collapsed block's are (D-67,
        /// D-73).
        open_tasks: usize,
    },
}

impl SidebarEntry {
    /// Where choosing the entry goes: into its block, or its special view.
    fn location(&self) -> Location {
        match self {
            Self::Inbox { id, .. } | Self::Bookmark { id, .. } | Self::Project { id, .. } => {
                Location::Outline(Some(*id))
            }
            Self::View(view) => Location::View(*view),
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

/// The open sidebar as a frontend draws it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarView {
    /// Its entries in order.
    pub entries: Rc<[SidebarEntry]>,
    /// The index of the focused entry, or `None` when it lists none.
    pub focused: Option<usize>,
    /// The index of the entry of where the focused pane is, if one goes there (D-68).
    pub current: Option<usize>,
}

impl State {
    /// The sidebar, or `None` while it is closed (FR-UI-01, D-79).
    #[must_use]
    pub fn sidebar(&self) -> Option<SidebarView> {
        self.sidebar.focus?;
        let entries = self.sidebar_entries();
        let focused = self.sidebar_focus(&entries)?;
        let focused = (focused < entries.len()).then_some(focused);
        let current = self.sidebar_current(&entries);
        Some(SidebarView {
            entries,
            focused,
            current,
        })
    }

    /// Whether the sidebar lists the `INBOX` project above Today rather than among the projects
    /// (`special_inbox`, D-75).
    #[must_use]
    pub fn special_inbox(&self) -> bool {
        self.sidebar.special_inbox
    }

    /// Whether the sidebar lists `view` (`sidebar_today` and so on, D-77, D-80).
    #[must_use]
    pub fn sidebar_lists(&self, view: SpecialView) -> bool {
        !self.sidebar.unlisted.contains(&view)
    }

    /// Whether the sidebar is open, and so has the keys rather than the pane.
    #[must_use]
    pub fn sidebar_focused(&self) -> bool {
        self.sidebar.focus.is_some()
    }

    /// `Ctrl+G`: closes the sidebar while it is open, and otherwise opens it on the entry of where
    /// the focused pane is, else on its first entry (D-68, D-79).
    pub(super) fn toggle_sidebar(&mut self) {
        if self.sidebar_focused() {
            self.sidebar.focus = None;
        } else {
            let entries = self.sidebar_entries();
            self.sidebar.focus = Some(self.sidebar_current(&entries).unwrap_or(0));
        }
    }

    /// Applies a key of the focused sidebar.
    pub(super) fn sidebar_input(&mut self, input: SidebarInput) {
        let entries = self.sidebar_entries();
        // Focusing picks the entry the others act on.
        let at = match input {
            // The inbox entry comes first while the sidebar lists it (D-75).
            SidebarInput::GoToInbox => {
                let inbox = matches!(entries.first(), Some(SidebarEntry::Inbox { .. }));
                if !inbox {
                    self.notify(Notice::NoInbox);
                }
                inbox.then_some(0)
            }
            SidebarInput::Focus(at) => (self.sidebar_focused() && at < entries.len()).then_some(at),
            SidebarInput::Next
            | SidebarInput::Previous
            | SidebarInput::Collapse
            | SidebarInput::Expand
            | SidebarInput::Choose
            | SidebarInput::Leave => self.sidebar_focus(&entries),
        };
        let Some(at) = at else {
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
            SidebarInput::Choose | SidebarInput::GoToInbox => {
                if let Some(location) = focused.map(SidebarEntry::location) {
                    self.sidebar.focus = None;
                    self.go_to(location);
                }
            }
            SidebarInput::Leave => self.sidebar.focus = None,
            SidebarInput::Focus(_) => self.sidebar.focus = Some(at),
        }
    }

    /// Where the focused entry goes, while the sidebar is open.
    pub(super) fn sidebar_location(&self) -> Option<Location> {
        let entries = self.sidebar_entries();
        entries
            .get(self.sidebar_focus(&entries)?)
            .map(SidebarEntry::location)
    }

    /// The commands the context menu offers on the focused entry while the sidebar is open: going
    /// to it, opening it in a new pane and folding a project entry with child projects, then
    /// closing the sidebar (FR-KEY-06).
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
            | SidebarEntry::Inbox { .. }
            | SidebarEntry::Bookmark { .. }
            | SidebarEntry::View(_) => None,
        };
        let entry = [NodeCommand::named(Command::Sidebar(SidebarInput::Choose))]
            .into_iter()
            .chain([NodeCommand::named(Command::Pane(PaneChange::Open))])
            .chain(fold.map(|input| NodeCommand::named(Command::Sidebar(input))))
            .collect();
        Some(vec![
            entry,
            vec![NodeCommand::named(Command::Sidebar(SidebarInput::Leave))],
        ])
    }

    /// The focused entry's index among `entries`, kept on the last one when entries went away.
    fn sidebar_focus(&self, entries: &[SidebarEntry]) -> Option<usize> {
        self.sidebar
            .focus
            .map(|at| at.min(entries.len().saturating_sub(1)))
    }

    /// The index among `entries` of the entry of where the focused pane is: the one going to its
    /// location, else to the closest block above its zoom block that has one, preferring an entry
    /// that is not a bookmark, so a project both bookmarked and listed is its project or inbox
    /// entry, and a block hidden in a folded entry is that entry (D-68).
    fn sidebar_current(&self, entries: &Rc<[SidebarEntry]>) -> Option<usize> {
        let location = self.pane.location;
        let key = CurrentKey {
            document: self.document().stamp(),
            entries: Rc::clone(entries),
            location,
        };
        self.sidebar.current.get(key, || {
            let going_to = |location| {
                let to = |entry: &SidebarEntry| entry.location() == location;
                entries
                    .iter()
                    .position(|entry| to(entry) && !matches!(entry, SidebarEntry::Bookmark { .. }))
                    .or_else(|| entries.iter().position(to))
            };
            let above = |&id: &ItemId| blocks::parent(self.document(), id).map(|block| block.id());
            match location {
                Location::View(view) => going_to(Location::View(view)),
                Location::Outline(zoom) => std::iter::successors(zoom, above)
                    .find_map(|id| going_to(Location::Outline(Some(id)))),
            }
        })
    }

    /// The inbox while it is apart, each special view it lists, then every bookmarked block in
    /// document order, then the project tree without the inbox (D-68, D-75, D-77, D-80).
    fn sidebar_entries(&self) -> Rc<[SidebarEntry]> {
        let key = EntriesKey {
            document: self.document().stamp(),
            folds: self.sidebar.folds.clone(),
            show_done: self.pane.show_done,
            special_inbox: self.sidebar.special_inbox,
            unlisted: self.sidebar.unlisted.clone(),
        };
        self.sidebar.entries.get(key, || {
            let inbox = self.sidebar.special_inbox.then(|| self.inbox()).flatten();
            let apart = inbox.as_ref().and_then(|inbox| inbox.location().zoom());
            // Every block that shows, from the outline's rows with nothing collapsed, which a
            // change builds again only where it changed.
            let mut every = Pane::first();
            every.show_done = self.pane.show_done;
            let rows = self.rows_folded(&every, &HashSet::new());
            let bookmarks =
                rows.iter()
                    .filter(|row| row.bookmark)
                    .map(|row| SidebarEntry::Bookmark {
                        id: row.id,
                        text: self.shown_text(row.id),
                    });
            inbox
                .into_iter()
                .chain(
                    SpecialView::ALL
                        .into_iter()
                        .filter(|&view| self.sidebar_lists(view))
                        .map(SidebarEntry::View),
                )
                .chain(bookmarks)
                .chain(self.projects(&rows, apart))
                .collect()
        })
    }

    /// The entry of the first shown top-level project whose text is `INBOX`, if any (D-75).
    fn inbox(&self) -> Option<SidebarEntry> {
        blocks::each_child(self.document(), self.document().root())
            .filter(|block| block.kind() == BlockKind::Project && self.shows(block))
            .map(|block| (self.shown_text(block.id()), block))
            .find(|(text, _)| text == INBOX)
            .map(|(text, block)| SidebarEntry::Inbox {
                id: block.id(),
                text,
                open_tasks: self.open_tasks(&block, false),
            })
    }

    /// The project entries of `rows`, each nested under the closest project above it, without
    /// the projects inside folded entries or the block `apart` with what it holds.
    fn projects(&self, rows: &Rows, apart: Option<ItemId>) -> Vec<SidebarEntry> {
        let mut out = Vec::new();
        // The projects above the current row.
        let mut above: Vec<Above> = Vec::new();
        // How many open tasks (D-67) the rows so far hold: tasks neither done nor inside a done
        // block, so a done project counts none.
        let mut open = 0;
        // The depth of `apart` while going through what it holds.
        let mut inside_apart = None;
        for row in rows {
            if inside_apart.is_some_and(|depth| row.depth > depth) {
                continue;
            }
            inside_apart = (apart == Some(row.id)).then_some(row.depth);
            if inside_apart.is_some() {
                continue;
            }
            while above
                .last()
                .is_some_and(|project| project.depth >= row.depth)
            {
                end_project(&mut above, &mut out, open);
            }
            if row.kind == BlockKind::Task && !row.done && !row.inside_done {
                open += 1;
            }
            if row.kind != BlockKind::Project {
                continue;
            }
            let parent = above.last().copied();
            if let Some(Above {
                entry: Some(entry),
                folded,
                ..
            }) = parent
                && let Some(SidebarEntry::Project { fold, .. }) = out.get_mut(entry)
            {
                *fold = if folded {
                    Fold::Collapsed
                } else {
                    Fold::Expanded
                };
            }
            let listed = parent.is_none_or(|above| above.entry.is_some() && !above.folded);
            let entry = listed.then(|| {
                out.push(SidebarEntry::Project {
                    id: row.id,
                    text: self.shown_text(row.id),
                    depth: above.len(),
                    fold: Fold::Leaf,
                    open_tasks: 0,
                });
                out.len() - 1
            });
            above.push(Above {
                depth: row.depth,
                entry,
                folded: self.sidebar.folds.contains(&row.id),
                open_before: open,
            });
        }
        while !above.is_empty() {
            end_project(&mut above, &mut out, open);
        }
        out
    }
}

/// A project above a row of the outline.
#[derive(Debug, Clone, Copy)]
struct Above {
    /// Its depth in the outline.
    depth: usize,
    /// Its entry, unless an entry above it is folded.
    entry: Option<usize>,
    /// Whether its entry is folded.
    folded: bool,
    /// How many open tasks came before it.
    open_before: usize,
}

/// Ends the last project of `above`, giving its entry in `out` the open tasks since it began,
/// `open` counting those so far.
fn end_project(above: &mut Vec<Above>, out: &mut [SidebarEntry], open: usize) {
    if let Some(Above {
        entry: Some(entry),
        open_before,
        ..
    }) = above.pop()
        && let Some(SidebarEntry::Project { open_tasks, .. }) = out.get_mut(entry)
    {
        *open_tasks = open - open_before;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::{Configuration, Message, Mode},
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

    /// The sidebar as it would open, or as it is while it is open.
    fn view(state: &State) -> SidebarView {
        state.sidebar().unwrap_or_else(|| {
            let mut opened = state.clone();
            opened.update(Message::ToggleSidebar);
            opened.sidebar().expect("open")
        })
    }

    /// Each entry as `kind text`, project entries indented 2 spaces per level behind their fold.
    fn listed(state: &State) -> Vec<String> {
        view(state)
            .entries
            .iter()
            .map(|entry| match entry {
                SidebarEntry::Inbox { text, .. } => format!("inbox {text}"),
                SidebarEntry::Bookmark { text, .. } => format!("bookmark {text}"),
                SidebarEntry::View(view) => view.name().to_lowercase(),
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

    // FR-UI-01, navigation §6, D-68
    #[test]
    fn lists_today_then_bookmarks_in_document_order_then_the_project_tree() {
        assert_eq!(
            listed(&open(OUTLINE)),
            [
                "today",
                "bookmark Launch plan",
                "bookmark Home",
                "Expanded Work",
                "  Expanded Drafts",
                "    Leaf Old",
                "  Leaf Ideas",
                "Leaf Home",
            ]
        );
    }

    /// An inbox holding a project and a bookmark, beside a project with an inbox of its own.
    const INBOXED: &str = "\
Work:
\tINBOX:
INBOX: @bookmark
\t- call @bookmark
\tLater:
\t\t- soon
Home:
";

    // D-75
    #[test]
    fn the_top_level_inbox_comes_first_and_leaves_the_project_tree_with_its_projects() {
        assert_eq!(
            listed(&open(INBOXED)),
            [
                "inbox INBOX",
                "today",
                "bookmark INBOX",
                "bookmark call",
                "Expanded Work",
                "  Leaf INBOX",
                "Leaf Home",
            ]
        );
    }

    // D-75
    #[test]
    fn only_the_first_shown_project_written_inbox_in_capitals_is_apart() {
        assert_eq!(
            listed(&open(
                "Inbox:
INBOX
- INBOX"
            ))[0],
            "today"
        );
        let both = listed(&open(
            "INBOX:
	A:
INBOX:
	B:",
        ));
        assert_eq!(both, ["inbox INBOX", "today", "Expanded INBOX", "  Leaf B"]);
        let mut state = open(
            "INBOX: @done
INBOX:",
        );
        assert_eq!(listed(&state), ["inbox INBOX", "today", "Leaf INBOX"]);
        state.update(Message::ToggleShowDone);
        assert_eq!(listed(&state), ["inbox INBOX", "today"], "the open one");
    }

    // D-75
    #[test]
    fn with_the_setting_off_the_inbox_is_a_project_like_any_other() {
        let mut state = open(INBOXED);
        assert!(state.special_inbox(), "on by default");
        state.update(Message::Configure(Configuration::SpecialInbox(false)));
        assert!(!state.special_inbox());
        assert_eq!(
            listed(&state)[..3],
            ["today", "bookmark INBOX", "bookmark call"]
        );
        assert_eq!(
            listed(&state)[5..],
            ["Expanded INBOX", "  Leaf Later", "Leaf Home"]
        );
        state.update(Message::Configure(Configuration::SpecialInbox(true)));
        assert_eq!(listed(&state), listed(&open(INBOXED)));
        assert_eq!(counts(&state)[0], ("INBOX".to_owned(), 2), "its open tasks");
    }

    // D-77
    #[test]
    fn with_the_today_setting_off_the_sidebar_lists_no_today_but_the_view_still_opens() {
        let mut state = open(INBOXED);
        assert!(state.sidebar_lists(SpecialView::Today), "on by default");
        state.update(Message::Configure(Configuration::SidebarView(
            SpecialView::Today,
            false,
        )));
        assert!(!state.sidebar_lists(SpecialView::Today));
        let mut without = listed(&open(INBOXED));
        without.retain(|entry| entry != "today");
        assert_eq!(listed(&state), without);
        state.update(Message::GoTo(Location::View(SpecialView::Today)));
        assert_eq!(state.location(), Location::View(SpecialView::Today));
        assert_eq!(current(&state), None, "no entry goes there");
        state.update(Message::ToggleSidebar);
        assert_eq!(focused(&state), Some(0), "the first entry");
        input(&mut state, &[SidebarInput::Choose]);
        assert_eq!(state.breadcrumbs(), ["INBOX"]);
        state.update(Message::Configure(Configuration::SidebarView(
            SpecialView::Today,
            true,
        )));
        assert_eq!(listed(&state), listed(&open(INBOXED)));
    }

    // D-75, D-68
    #[test]
    fn the_inbox_entry_goes_to_the_inbox_has_no_fold_and_beats_its_bookmark_as_current() {
        let mut state = open(INBOXED);
        state.update(Message::ToggleSidebar);
        assert_eq!(focused(&state), Some(0), "the first entry");
        assert_eq!(
            entry_commands(&state),
            Some(vec![
                "Go to Entry, Open in New Pane".to_owned(),
                "Return to Outline".to_owned()
            ])
        );
        input(&mut state, &[SidebarInput::Collapse, SidebarInput::Choose]);
        assert_eq!(state.breadcrumbs(), ["INBOX"]);
        assert_eq!(current(&state), Some(0));
        let call = entry_block(&state, 3);
        state.update(Message::GoTo(Location::Outline(Some(call))));
        assert_eq!(state.breadcrumbs(), ["INBOX", "call"]);
        assert_eq!(current(&state), Some(3), "call's own bookmark");
        state.update(Message::Back);
        state.update(Message::FocusLast);
        state.update(Message::ZoomIn);
        assert_eq!(state.breadcrumbs(), ["INBOX", "Later", "soon"]);
        assert_eq!(current(&state), Some(0), "inside the inbox");
    }

    // D-78
    #[test]
    fn go_to_inbox_zooms_to_the_inbox_entrys_project_or_says_there_is_none() {
        let mut state = open(INBOXED);
        input(&mut state, &[SidebarInput::GoToInbox]);
        assert_eq!(state.breadcrumbs(), ["INBOX"]);
        assert_eq!(state.notice(), None);
        state.update(Message::Back);
        assert_eq!(
            state.breadcrumbs(),
            Vec::<String>::new(),
            "a step of the history"
        );
        state.update(Message::Configure(Configuration::SpecialInbox(false)));
        input(&mut state, &[SidebarInput::GoToInbox]);
        assert_eq!(
            state.breadcrumbs(),
            Vec::<String>::new(),
            "the setting is off"
        );
        assert_eq!(state.notice(), Some(&Notice::NoInbox));
        let mut state = open("Work:\n\tINBOX:");
        input(&mut state, &[SidebarInput::GoToInbox]);
        assert_eq!(state.notice(), Some(&Notice::NoInbox), "not top-level");
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

    /// Each project or inbox entry's text with its count of open tasks.
    fn counts(state: &State) -> Vec<(String, usize)> {
        view(state)
            .entries
            .iter()
            .filter_map(|entry| match entry {
                SidebarEntry::Inbox {
                    text, open_tasks, ..
                }
                | SidebarEntry::Project {
                    text, open_tasks, ..
                } => Some((text.clone(), *open_tasks)),
                SidebarEntry::Bookmark { .. } | SidebarEntry::View(_) => None,
            })
            .collect()
    }

    // D-73, D-67, D-40
    #[test]
    fn a_project_entry_counts_the_open_tasks_in_its_whole_subtree() {
        let mut state = open(
            "A:\n\t- a\n\tnote\n\t- b @done\n\t\t- c\n\tB:\n\t\tx\n\t\t\t- d\n\tC: @done\n\t\t- e\n\t\tD:\n\t\t\t- f\nE:",
        );
        let all = [
            ("A".to_owned(), 2),
            ("B".to_owned(), 1),
            ("C".to_owned(), 0),
            ("D".to_owned(), 0),
            ("E".to_owned(), 0),
        ];
        assert_eq!(
            counts(&state),
            all,
            "sub-projects count, done blocks do not"
        );
        state.update(Message::ToggleSidebar);
        input(
            &mut state,
            &[SidebarInput::Focus(1), SidebarInput::Collapse],
        );
        assert_eq!(counts(&state), [all[0].clone(), all[4].clone()], "folded");
        input(&mut state, &[SidebarInput::Expand]);
        state.update(Message::ToggleShowDone);
        assert_eq!(
            counts(&state),
            [all[0].clone(), all[1].clone(), all[4].clone()]
        );
    }

    // FR-KEY-05
    #[test]
    fn focusing_an_entry_moves_the_focus_there_while_the_sidebar_is_open() {
        let mut state = open(OUTLINE);
        input(&mut state, &[SidebarInput::Focus(3)]);
        assert!(!state.sidebar_focused(), "a closed sidebar takes no keys");
        state.update(Message::ToggleSidebar);
        input(&mut state, &[SidebarInput::Focus(3)]);
        assert_eq!(focused(&state), Some(3));
        let past = listed(&state).len();
        input(&mut state, &[SidebarInput::Focus(past)]);
        assert_eq!(focused(&state), Some(3), "no entry there");
        input(&mut state, &[SidebarInput::Choose]);
        assert_eq!(state.breadcrumbs(), ["Work"]);
        assert_eq!(state.sidebar(), None, "closed");
    }

    // FR-UI-01
    #[test]
    fn an_outline_without_bookmarks_or_projects_lists_only_today() {
        assert_eq!(listed(&open("- a\nb")), ["today"]);
        assert_eq!(listed(&State::new(Document::default(), None)), ["today"]);
    }

    // FR-UI-01, D-68, D-79
    #[test]
    fn toggling_opens_the_sidebar_with_the_keys_and_closes_it() {
        let mut state = open(OUTLINE);
        assert_eq!(state.sidebar(), None, "closed at the start");
        assert!(!state.sidebar_focused());
        state.update(Message::ToggleSidebar);
        assert!(state.sidebar_focused());
        assert_eq!(focused(&state), Some(0), "the pane is at home");
        state.update(Message::ToggleSidebar);
        assert_eq!(state.sidebar(), None);
        assert!(!state.sidebar_focused());
    }

    // FR-MODE-01
    #[test]
    fn focusing_the_sidebar_ends_editing() {
        let mut state = open(OUTLINE);
        state.update(Message::EditAtEnd);
        state.update(Message::ToggleSidebar);
        assert_eq!(state.mode(), Mode::Navigate);
    }

    // keybindings.md §4
    #[test]
    fn next_and_previous_move_the_focus_and_stop_at_the_ends() {
        let mut state = open(OUTLINE);
        state.update(Message::ToggleSidebar);
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
    fn keys_of_the_sidebar_do_nothing_until_it_is_open() {
        let mut state = open(OUTLINE);
        input(&mut state, &[SidebarInput::Next, SidebarInput::Collapse]);
        assert_eq!(focused(&state), None);
        assert_eq!(listed(&state)[3], "Expanded Work");
    }

    // keybindings.md §4
    #[test]
    fn collapse_and_expand_fold_project_entries_only() {
        let mut state = open(OUTLINE);
        state.update(Message::ToggleSidebar);
        input(&mut state, &[SidebarInput::Next; 3]);
        input(&mut state, &[SidebarInput::Collapse]);
        assert_eq!(
            listed(&state)[2..],
            ["bookmark Home", "Collapsed Work", "Leaf Home"],
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
            state.update(Message::ToggleSidebar);
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
        state.update(Message::ToggleSidebar);
        input(&mut state, &[SidebarInput::Next, SidebarInput::Choose]);
        assert_eq!(zoomed(&state).as_deref(), Some("Launch plan"));
        assert!(!state.sidebar_focused());
        state.update(Message::ToggleSidebar);
        assert_eq!(focused(&state), Some(1), "on the entry of the zoom");
        input(&mut state, &[SidebarInput::Next; 3]);
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
        state.update(Message::ToggleSidebar);
        input(&mut state, &[SidebarInput::Next, SidebarInput::Choose]);
        state.update(Message::ToggleSidebar);
        input(&mut state, &[SidebarInput::Previous, SidebarInput::Choose]);
        assert_eq!(state.location(), Location::View(SpecialView::Today));
        assert!(!state.sidebar_focused());
        state.update(Message::Back);
        assert_eq!(
            zoomed(&state).as_deref(),
            Some("Launch plan"),
            "Today is a step of the zoom history"
        );
    }

    fn current(state: &State) -> Option<usize> {
        view(state).current
    }

    /// The block of the entry at `at`.
    fn entry_block(state: &State, at: usize) -> ItemId {
        view(state).entries[at]
            .location()
            .zoom()
            .expect("a block's entry")
    }

    // D-68
    #[test]
    fn the_entry_of_where_the_focused_pane_is_is_current() {
        let mut state = open(OUTLINE);
        assert_eq!(current(&state), None, "home has no entry");
        state.update(Message::ZoomIn);
        assert_eq!(current(&state), Some(3), "Work");
        state.update(Message::GoTo(Location::View(SpecialView::Today)));
        assert_eq!(current(&state), Some(0));
        let home = entry_block(&state, 7);
        state.update(Message::GoTo(Location::Outline(Some(home))));
        assert_eq!(
            current(&state),
            Some(7),
            "a bookmarked project's project entry"
        );
        let launch = entry_block(&state, 1);
        state.update(Message::GoTo(Location::Outline(Some(launch))));
        assert_eq!(current(&state), Some(1), "a bookmark");
        state.update(Message::GoTo(Location::Outline(None)));
        assert_eq!(current(&state), None);
    }

    // D-68
    #[test]
    fn a_block_without_an_entry_makes_the_closest_entry_above_it_current() {
        let mut state = open(OUTLINE);
        state.update(Message::ZoomIn);
        state.update(Message::FocusNext);
        state.update(Message::ZoomIn);
        assert_eq!(state.breadcrumbs(), ["Work", "Report"]);
        assert_eq!(current(&state), Some(3), "Work, above the task");
        state.update(Message::ZoomIn);
        assert_eq!(current(&state), Some(4), "Drafts");
        state.update(Message::ToggleSidebar);
        input(
            &mut state,
            &[SidebarInput::Focus(3), SidebarInput::Collapse],
        );
        input(&mut state, &[SidebarInput::Leave]);
        assert_eq!(current(&state), Some(3), "Work, folded over Drafts");
    }

    // D-68
    #[test]
    fn the_sidebar_opens_on_the_current_entry_which_stays_marked() {
        let mut state = open(OUTLINE);
        state.update(Message::ZoomIn);
        state.update(Message::ToggleSidebar);
        assert_eq!(focused(&state), Some(3), "on Work");
        assert_eq!(current(&state), Some(3));
        input(&mut state, &[SidebarInput::Next]);
        assert_eq!(focused(&state), Some(4));
        assert_eq!(current(&state), Some(3), "still Work");
        state.update(Message::ToggleSidebar);
        state.update(Message::ToggleSidebar);
        assert_eq!(focused(&state), Some(3), "on Work again");
    }

    // keybindings.md §4
    #[test]
    fn leaving_closes_the_sidebar_on_the_same_view() {
        let mut state = open(OUTLINE);
        state.update(Message::ToggleSidebar);
        input(&mut state, &[SidebarInput::Next, SidebarInput::Leave]);
        assert!(!state.sidebar_focused());
        assert_eq!(state.sidebar(), None);
        assert_eq!(zoomed(&state), None);
    }

    // FR-UI-01
    #[test]
    fn the_focus_stays_on_the_last_entry_when_entries_go_away() {
        let mut state = open("- a @bookmark\nP:");
        state.update(Message::ToggleSidebar);
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
    fn the_focused_entrys_commands_go_to_it_open_it_fold_it_and_close_the_sidebar() {
        let mut state = open(OUTLINE);
        assert_eq!(entry_commands(&state), None, "the pane has the keys");
        state.update(Message::ToggleSidebar);
        let leave = "Return to Outline".to_owned();
        assert_eq!(
            entry_commands(&state),
            Some(vec![
                "Go to Entry, Open in New Pane".to_owned(),
                leave.clone()
            ]),
            "Today"
        );
        input(&mut state, &[SidebarInput::Next, SidebarInput::Next]);
        assert_eq!(
            entry_commands(&state),
            Some(vec![
                "Go to Entry, Open in New Pane".to_owned(),
                leave.clone()
            ]),
            "a bookmark"
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
        assert_eq!(groups[1][0].command, Command::Sidebar(SidebarInput::Leave));
        assert_eq!(groups[0][0].command, Command::Sidebar(SidebarInput::Choose));
    }
}
