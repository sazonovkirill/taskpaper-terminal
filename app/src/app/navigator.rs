//! The Navigator, a window that `Ctrl+G` opens over the panes (D-79): its entries, which one has the keys while it is open and which project entries are folded ([navigation.md §6](../../../docs/specs/navigation.md#6-navigator-fr-ui-01-d-7-d-11-d-28-d-79),
//! FR-UI-01).

use std::{collections::HashSet, rc::Rc};

use jiff::civil::Date;

use super::{
    Location, Notice, PaneChange, Row, Rows, SpecialView, State,
    commands::{Command, NodeCommand},
    memo::Memo,
    pane::Pane,
};
use crate::document::{Block, BlockKind, ItemId, Stamp, blocks};

/// The text of the project the Navigator lists apart, above Today (D-75).
const INBOX: &str = "INBOX";

/// The Navigator's focused entry while it is open, its folded project entries, which are its own
/// and not the outline's, whether it lists the `INBOX` project apart and which special views it
/// leaves out.
#[derive(Debug, Clone)]
pub(super) struct Navigator {
    special_inbox: bool,
    unlisted: HashSet<SpecialView>,
    /// The index of the focused entry, while the Navigator is open, which it is only while it has
    /// the keys (D-79).
    focus: Option<usize>,
    /// What was typed to filter the entries while it is open (D-86).
    filter: String,
    folds: HashSet<ItemId>,
    entries: Memo<EntriesKey, Rc<[NavigatorEntry]>>,
    current: Memo<CurrentKey, Option<usize>>,
}

/// Everything the Navigator's entries are computed from.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EntriesKey {
    document: Stamp,
    /// The folded entries, none while a filter is typed.
    folds: HashSet<ItemId>,
    filter: String,
    show_done: bool,
    special_inbox: bool,
    unlisted: HashSet<SpecialView>,
    /// The local date and whether Today lists starred blocks, which the special views' counts
    /// follow (D-85).
    today: Date,
    today_starred: bool,
}

/// Everything the entry of where the focused pane is comes from: the entries, the blocks above
/// the pane's zoom block and the pane's location.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CurrentKey {
    document: Stamp,
    entries: Rc<[NavigatorEntry]>,
    location: Location,
}

impl Navigator {
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

impl Default for Navigator {
    /// Closed, every project entry expanded, the `INBOX` project apart, every special view listed.
    fn default() -> Self {
        Self {
            special_inbox: true,
            unlisted: HashSet::new(),
            focus: None,
            filter: String::new(),
            folds: HashSet::new(),
            entries: Memo::default(),
            current: Memo::default(),
        }
    }
}

/// A key of the open Navigator ([keybindings.md §4](../../../docs/specs/keybindings.md#4-other-contexts)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigatorInput {
    /// Focus the entry below.
    Next,
    /// Focus the entry above.
    Previous,
    /// Fold the focused project entry, hiding its child projects.
    Collapse,
    /// Unfold the focused project entry.
    Expand,
    /// Zoom the pane to the focused entry and close the Navigator.
    Choose,
    /// Close the Navigator, giving the pane the keys.
    Leave,
    /// Focus the entry at this index while the Navigator is open, as a click on it does
    /// (FR-KEY-05).
    Focus(usize),
    /// Add a character to the filter (D-86).
    Type(char),
    /// Erase the filter's last character (D-86).
    Erase,
    /// `gi`: zoom the pane to the inbox entry's project, from anywhere, or say there is none
    /// (D-78).
    GoToInbox,
}

/// One entry of the Navigator, in its sections' order: the inbox, the special views, bookmarks,
/// projects (D-68, D-75, D-80).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigatorEntry {
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
        /// How many open tasks it holds, as a project entry counts them (D-85).
        open_tasks: usize,
    },
    /// A special view, such as Today, with how many open tasks it lists (D-7, D-80, D-85).
    View {
        /// The view.
        view: SpecialView,
        /// How many open tasks it lists (D-85).
        open_tasks: usize,
    },
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

impl NavigatorEntry {
    /// Where choosing the entry goes: into its block, or its special view.
    fn location(&self) -> Location {
        match self {
            Self::Inbox { id, .. } | Self::Bookmark { id, .. } | Self::Project { id, .. } => {
                Location::Outline(Some(*id))
            }
            Self::View { view, .. } => Location::View(*view),
        }
    }

    /// Whether the entry's text holds `filter`, which is in lower case (D-86).
    fn matches(&self, filter: &str) -> bool {
        let text = match self {
            Self::Inbox { text, .. } | Self::Bookmark { text, .. } | Self::Project { text, .. } => {
                text.as_str()
            }
            Self::View { view, .. } => view.name(),
        };
        text.to_lowercase().contains(filter)
    }

    /// How many open tasks the entry holds or lists (D-73, D-85).
    #[must_use]
    pub fn open_tasks(&self) -> usize {
        match *self {
            Self::Inbox { open_tasks, .. }
            | Self::Bookmark { open_tasks, .. }
            | Self::View { open_tasks, .. }
            | Self::Project { open_tasks, .. } => open_tasks,
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

/// The open Navigator as a frontend draws it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigatorView {
    /// Its entries in order.
    pub entries: Rc<[NavigatorEntry]>,
    /// The index of the focused entry, or `None` when it lists none.
    pub focused: Option<usize>,
    /// The index of the entry of where the focused pane is, if one goes there (D-68).
    pub current: Option<usize>,
    /// What was typed to filter the entries (D-86).
    pub filter: String,
}

impl State {
    /// The Navigator, or `None` while it is closed (FR-UI-01, D-79).
    #[must_use]
    pub fn navigator(&self) -> Option<NavigatorView> {
        self.navigator.focus?;
        let entries = self.navigator_entries();
        let focused = self.navigator_focus(&entries)?;
        let focused = (focused < entries.len()).then_some(focused);
        let current = self.navigator_current(&entries);
        Some(NavigatorView {
            entries,
            focused,
            current,
            filter: self.navigator.filter.clone(),
        })
    }

    /// Whether the Navigator lists the `INBOX` project above Today rather than among the projects
    /// (`special_inbox`, D-75).
    #[must_use]
    pub fn special_inbox(&self) -> bool {
        self.navigator.special_inbox
    }

    /// Whether the Navigator lists `view` (`sidebar_today` and so on, D-77, D-80).
    #[must_use]
    pub fn navigator_lists(&self, view: SpecialView) -> bool {
        !self.navigator.unlisted.contains(&view)
    }

    /// Whether the Navigator is open, and so has the keys rather than the pane.
    #[must_use]
    pub fn navigator_focused(&self) -> bool {
        self.navigator.focus.is_some()
    }

    /// `Ctrl+G`: closes the Navigator while it is open, and otherwise opens it on the entry of where
    /// the focused pane is, else on its first entry (D-68, D-79).
    pub(super) fn toggle_navigator(&mut self) {
        if self.navigator_focused() {
            self.close_navigator();
        } else {
            let entries = self.navigator_entries();
            self.navigator.focus = Some(self.navigator_current(&entries).unwrap_or(0));
        }
    }

    /// Applies a key of the focused Navigator.
    pub(super) fn navigator_input(&mut self, input: NavigatorInput) {
        let entries = self.navigator_entries();
        // Focusing picks the entry the others act on.
        let at = match input {
            // The inbox entry comes first while the Navigator lists it (D-75).
            NavigatorInput::GoToInbox => {
                let inbox = matches!(entries.first(), Some(NavigatorEntry::Inbox { .. }));
                if !inbox {
                    self.notify(Notice::NoInbox);
                }
                inbox.then_some(0)
            }
            NavigatorInput::Focus(at) => {
                (self.navigator_focused() && at < entries.len()).then_some(at)
            }
            NavigatorInput::Next
            | NavigatorInput::Previous
            | NavigatorInput::Collapse
            | NavigatorInput::Expand
            | NavigatorInput::Choose
            | NavigatorInput::Leave
            | NavigatorInput::Type(_)
            | NavigatorInput::Erase => self.navigator_focus(&entries),
        };
        let Some(at) = at else {
            return;
        };
        let focused = entries.get(at);
        match input {
            NavigatorInput::Next => {
                self.navigator.focus =
                    Some(at.saturating_add(1).min(entries.len().saturating_sub(1)));
            }
            NavigatorInput::Previous => self.navigator.focus = Some(at.saturating_sub(1)),
            NavigatorInput::Collapse => {
                if let Some(&NavigatorEntry::Project {
                    id,
                    fold: Fold::Expanded,
                    ..
                }) = focused
                    && self.navigator.filter.is_empty()
                {
                    self.navigator.folds.insert(id);
                }
            }
            NavigatorInput::Expand => {
                if let Some(NavigatorEntry::Project { id, .. }) = focused
                    && self.navigator.filter.is_empty()
                {
                    self.navigator.folds.remove(id);
                }
            }
            NavigatorInput::Choose | NavigatorInput::GoToInbox => {
                if let Some(location) = focused.map(NavigatorEntry::location) {
                    self.close_navigator();
                    self.go_to(location);
                }
            }
            NavigatorInput::Leave => self.close_navigator(),
            NavigatorInput::Focus(_) => self.navigator.focus = Some(at),
            NavigatorInput::Type(_) | NavigatorInput::Erase => self.filter_navigator(input),
        }
    }

    /// Changes the filter while the Navigator is open, focusing the first entry it matches, or,
    /// with no filter left, the entry of where the focused pane is, else the first (D-86).
    fn filter_navigator(&mut self, input: NavigatorInput) {
        match input {
            NavigatorInput::Type(c) => self.navigator.filter.push(c),
            // `Backspace` on an empty filter leaves the focus where it is.
            _ => {
                if self.navigator.filter.pop().is_none() {
                    return;
                }
            }
        }
        let entries = self.navigator_entries();
        let filter = self.navigator.filter.to_lowercase();
        let at = if filter.is_empty() {
            self.navigator_current(&entries)
        } else {
            entries.iter().position(|entry| entry.matches(&filter))
        };
        self.navigator.focus = Some(at.unwrap_or(0));
    }

    /// Closes the Navigator, dropping its filter.
    fn close_navigator(&mut self) {
        self.navigator.focus = None;
        self.navigator.filter.clear();
    }

    /// Where the focused entry goes, while the Navigator is open.
    pub(super) fn navigator_location(&self) -> Option<Location> {
        let entries = self.navigator_entries();
        entries
            .get(self.navigator_focus(&entries)?)
            .map(NavigatorEntry::location)
    }

    /// The commands the context menu offers on the focused entry while the Navigator is open: going
    /// to it, opening it in a new pane and folding a project entry with child projects, then
    /// closing the Navigator (FR-KEY-06).
    pub(super) fn navigator_commands(&self) -> Option<Vec<Vec<NodeCommand>>> {
        let entries = self.navigator_entries();
        let entry = entries.get(self.navigator_focus(&entries)?)?;
        let fold = match entry {
            NavigatorEntry::Project {
                fold: Fold::Expanded,
                ..
            } => Some(NavigatorInput::Collapse),
            NavigatorEntry::Project {
                fold: Fold::Collapsed,
                ..
            } => Some(NavigatorInput::Expand),
            NavigatorEntry::Project {
                fold: Fold::Leaf, ..
            }
            | NavigatorEntry::Inbox { .. }
            | NavigatorEntry::Bookmark { .. }
            | NavigatorEntry::View { .. } => None,
        };
        let entry = [NodeCommand::named(Command::Navigator(
            NavigatorInput::Choose,
        ))]
        .into_iter()
        .chain([NodeCommand::named(Command::Pane(PaneChange::Open))])
        .chain(fold.map(|input| NodeCommand::named(Command::Navigator(input))))
        .collect();
        Some(vec![
            entry,
            vec![NodeCommand::named(Command::Navigator(
                NavigatorInput::Leave,
            ))],
        ])
    }

    /// The focused entry's index among `entries`, kept on the last one when entries went away.
    fn navigator_focus(&self, entries: &[NavigatorEntry]) -> Option<usize> {
        self.navigator
            .focus
            .map(|at| at.min(entries.len().saturating_sub(1)))
    }

    /// The index among `entries` of the entry of where the focused pane is: the one going to its
    /// location, else to the closest block above its zoom block that has one, preferring an entry
    /// that is not a bookmark, so a project both bookmarked and listed is its project or inbox
    /// entry, and a block hidden in a folded entry is that entry (D-68).
    fn navigator_current(&self, entries: &Rc<[NavigatorEntry]>) -> Option<usize> {
        let location = self.pane.location;
        let key = CurrentKey {
            document: self.document().stamp(),
            entries: Rc::clone(entries),
            location,
        };
        self.navigator.current.get(key, || {
            let going_to = |location| {
                let to = |entry: &NavigatorEntry| entry.location() == location;
                entries
                    .iter()
                    .position(|entry| {
                        to(entry) && !matches!(entry, NavigatorEntry::Bookmark { .. })
                    })
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
    fn navigator_entries(&self) -> Rc<[NavigatorEntry]> {
        let filter = self.navigator.filter.to_lowercase();
        // A filter looks through folded entries.
        let folds = if filter.is_empty() {
            self.navigator.folds.clone()
        } else {
            HashSet::new()
        };
        let key = EntriesKey {
            document: self.document().stamp(),
            folds: folds.clone(),
            filter: filter.clone(),
            show_done: self.pane.show_done,
            special_inbox: self.navigator.special_inbox,
            unlisted: self.navigator.unlisted.clone(),
            today: self.today,
            today_starred: self.today_starred,
        };
        self.navigator.entries.get(key, || {
            let inbox = self.navigator.special_inbox.then(|| self.inbox()).flatten();
            let apart = inbox.as_ref().and_then(|inbox| inbox.location().zoom());
            // Every block that shows, from the outline's rows with nothing collapsed, which a
            // change builds again only where it changed.
            let mut every = Pane::first();
            every.show_done = self.pane.show_done;
            let rows = self.rows_folded(&every, &HashSet::new());
            let views = self.views_open_tasks(&rows);
            let entries: Vec<_> = inbox
                .into_iter()
                .chain(
                    SpecialView::ALL
                        .into_iter()
                        .zip(views)
                        .filter(|&(view, _)| self.navigator_lists(view))
                        .map(|(view, open_tasks)| NavigatorEntry::View { view, open_tasks }),
                )
                .chain(self.bookmarks(&rows))
                .chain(self.projects(&rows, apart, &folds))
                .collect();
            if filter.is_empty() {
                entries.into()
            } else {
                filtered(entries, &filter).into()
            }
        })
    }

    /// How many open tasks (D-67) each special view lists, in the order of [`SpecialView::ALL`],
    /// from `rows`, the outline's rows with nothing collapsed: every block that shows, so every
    /// block a view could list as an open task.
    fn views_open_tasks(&self, rows: &Rows) -> [usize; SpecialView::ALL.len()] {
        let mut counts = [0; SpecialView::ALL.len()];
        let open = rows.iter().filter(|row| open_task(row));
        for block in open.filter_map(|row| blocks::block(&self.document, row.id)) {
            for (count, view) in counts.iter_mut().zip(SpecialView::ALL) {
                *count += usize::from(self.lists(view, &block));
            }
        }
        counts
    }

    /// The entry of the first shown top-level project whose text is `INBOX`, if any (D-75).
    fn inbox(&self) -> Option<NavigatorEntry> {
        self.top_project(INBOX).map(|block| NavigatorEntry::Inbox {
            id: block.id(),
            text: INBOX.to_owned(),
            open_tasks: self.open_tasks(&block, false),
        })
    }

    /// The project the Navigator lists apart as the inbox, which `qi` adds to (D-75, D-96).
    pub(super) fn inbox_project(&self) -> Option<ItemId> {
        self.navigator
            .special_inbox
            .then(|| self.top_project(INBOX))
            .flatten()
            .map(|block| block.id())
    }

    /// The first shown top-level project whose text is `text`, if any (D-75, D-98).
    pub(super) fn top_project(&self, text: &str) -> Option<Block> {
        blocks::each_child(self.document(), self.document().root())
            .filter(|block| block.kind() == BlockKind::Project && self.shows(block))
            .find(|block| self.shown_text(block.id()) == text)
    }

    /// The bookmark entries of `rows` in document order, each with the open tasks it holds.
    fn bookmarks(&self, rows: &Rows) -> Vec<NavigatorEntry> {
        let mut out = Vec::new();
        // The bookmarks above the current row: their depth, entry and the open tasks before them.
        let mut above: Vec<(usize, usize, usize)> = Vec::new();
        let mut open_so_far = 0;
        let end = |above: &mut Vec<(usize, usize, usize)>, out: &mut Vec<NavigatorEntry>, open| {
            if let Some((_, entry, before)) = above.pop()
                && let Some(NavigatorEntry::Bookmark { open_tasks, .. }) = out.get_mut(entry)
            {
                *open_tasks = open - before;
            }
        };
        for row in rows {
            while above.last().is_some_and(|&(depth, ..)| depth >= row.depth) {
                end(&mut above, &mut out, open_so_far);
            }
            // A bookmarked task holds the tasks below it, not itself.
            open_so_far += usize::from(open_task(row));
            if row.bookmark {
                above.push((row.depth, out.len(), open_so_far));
                out.push(NavigatorEntry::Bookmark {
                    id: row.id,
                    text: self.shown_text(row.id),
                    open_tasks: 0,
                });
            }
        }
        while !above.is_empty() {
            end(&mut above, &mut out, open_so_far);
        }
        out
    }

    /// The project entries of `rows`, each nested under the closest project above it, without
    /// the projects inside folded entries or the block `apart` with what it holds.
    fn projects(
        &self,
        rows: &Rows,
        apart: Option<ItemId>,
        folds: &HashSet<ItemId>,
    ) -> Vec<NavigatorEntry> {
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
            open += usize::from(open_task(row));
            if row.kind != BlockKind::Project {
                continue;
            }
            let parent = above.last().copied();
            if let Some(Above {
                entry: Some(entry),
                folded,
                ..
            }) = parent
                && let Some(NavigatorEntry::Project { fold, .. }) = out.get_mut(entry)
            {
                *fold = if folded {
                    Fold::Collapsed
                } else {
                    Fold::Expanded
                };
            }
            let listed = parent.is_none_or(|above| above.entry.is_some() && !above.folded);
            let entry = listed.then(|| {
                out.push(NavigatorEntry::Project {
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
                folded: folds.contains(&row.id),
                open_before: open,
            });
        }
        while !above.is_empty() {
            end_project(&mut above, &mut out, open);
        }
        out
    }
}

/// The entries of `entries` that match `filter`, which is in lower case, and the project entries
/// above a matching one, so the tree around it stays; none has a fold, as folding does nothing
/// while a filter is typed (D-86).
fn filtered(entries: Vec<NavigatorEntry>, filter: &str) -> Vec<NavigatorEntry> {
    let mut kept: Vec<bool> = Vec::with_capacity(entries.len());
    // The project entries above the current one: their index and depth.
    let mut above: Vec<(usize, usize)> = Vec::new();
    for (at, entry) in entries.iter().enumerate() {
        let matches = entry.matches(filter);
        if let &NavigatorEntry::Project { depth, .. } = entry {
            while above.last().is_some_and(|&(_, above)| above >= depth) {
                above.pop();
            }
            if matches {
                for &(project, _) in &above {
                    if let Some(kept) = kept.get_mut(project) {
                        *kept = true;
                    }
                }
            }
            above.push((at, depth));
        }
        kept.push(matches);
    }
    entries
        .into_iter()
        .zip(kept)
        .filter_map(|(mut entry, kept)| {
            if let NavigatorEntry::Project { fold, .. } = &mut entry {
                *fold = Fold::Leaf;
            }
            kept.then_some(entry)
        })
        .collect()
}

/// Whether `row` is an open task: neither done nor inside a done block (D-67), so a done block
/// holds none.
fn open_task(row: &Row) -> bool {
    row.kind == BlockKind::Task && !row.done && !row.inside_done
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
fn end_project(above: &mut Vec<Above>, out: &mut [NavigatorEntry], open: usize) {
    if let Some(Above {
        entry: Some(entry),
        open_before,
        ..
    }) = above.pop()
        && let Some(NavigatorEntry::Project { open_tasks, .. }) = out.get_mut(entry)
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

    /// `text` opened with Today the only special view the Navigator lists, as these tests were
    /// written for before the other views (D-80).
    fn open(text: &str) -> State {
        let mut state = State::new(format::parse(text), None);
        for view in SpecialView::ALL {
            if view != SpecialView::Today {
                state.update(Message::Configure(Configuration::NavigatorView(
                    view, false,
                )));
            }
        }
        state
    }

    /// The Navigator as it would open, or as it is while it is open.
    fn view(state: &State) -> NavigatorView {
        state.navigator().unwrap_or_else(|| {
            let mut opened = state.clone();
            opened.update(Message::ToggleNavigator);
            opened.navigator().expect("open")
        })
    }

    /// Each entry as `kind text`, project entries indented 2 spaces per level behind their fold.
    fn listed(state: &State) -> Vec<String> {
        view(state)
            .entries
            .iter()
            .map(|entry| match entry {
                NavigatorEntry::Inbox { text, .. } => format!("inbox {text}"),
                NavigatorEntry::Bookmark { text, .. } => format!("bookmark {text}"),
                NavigatorEntry::View { view, .. } => view.name().to_lowercase(),
                NavigatorEntry::Project {
                    text, depth, fold, ..
                } => format!("{}{fold:?} {text}", "  ".repeat(*depth)),
            })
            .collect()
    }

    fn focused(state: &State) -> Option<usize> {
        state.navigator().and_then(|view| view.focused)
    }

    fn input(state: &mut State, inputs: &[NavigatorInput]) {
        for &input in inputs {
            state.update(Message::Navigator(input));
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
    fn with_the_today_setting_off_the_navigator_lists_no_today_but_the_view_still_opens() {
        let mut state = open(INBOXED);
        assert!(state.navigator_lists(SpecialView::Today), "on by default");
        state.update(Message::Configure(Configuration::NavigatorView(
            SpecialView::Today,
            false,
        )));
        assert!(!state.navigator_lists(SpecialView::Today));
        let mut without = listed(&open(INBOXED));
        without.retain(|entry| entry != "today");
        assert_eq!(listed(&state), without);
        state.update(Message::GoTo(Location::View(SpecialView::Today)));
        assert_eq!(state.location(), Location::View(SpecialView::Today));
        assert_eq!(current(&state), None, "no entry goes there");
        state.update(Message::ToggleNavigator);
        assert_eq!(focused(&state), Some(0), "the first entry");
        input(&mut state, &[NavigatorInput::Choose]);
        assert_eq!(state.breadcrumbs(), ["INBOX"]);
        state.update(Message::Configure(Configuration::NavigatorView(
            SpecialView::Today,
            true,
        )));
        assert_eq!(listed(&state), listed(&open(INBOXED)));
    }

    // D-75, D-68
    #[test]
    fn the_inbox_entry_goes_to_the_inbox_has_no_fold_and_beats_its_bookmark_as_current() {
        let mut state = open(INBOXED);
        state.update(Message::ToggleNavigator);
        assert_eq!(focused(&state), Some(0), "the first entry");
        assert_eq!(
            entry_commands(&state),
            Some(vec![
                "Go to Entry, Open in New Pane".to_owned(),
                "Return to Outline".to_owned()
            ])
        );
        input(
            &mut state,
            &[NavigatorInput::Collapse, NavigatorInput::Choose],
        );
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
        input(&mut state, &[NavigatorInput::GoToInbox]);
        assert_eq!(state.breadcrumbs(), ["INBOX"]);
        assert_eq!(state.notice(), None);
        state.update(Message::Back);
        assert_eq!(
            state.breadcrumbs(),
            Vec::<String>::new(),
            "a step of the history"
        );
        state.update(Message::Configure(Configuration::SpecialInbox(false)));
        input(&mut state, &[NavigatorInput::GoToInbox]);
        assert_eq!(
            state.breadcrumbs(),
            Vec::<String>::new(),
            "the setting is off"
        );
        assert_eq!(state.notice(), Some(&Notice::NoInbox));
        let mut state = open("Work:\n\tINBOX:");
        input(&mut state, &[NavigatorInput::GoToInbox]);
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
                NavigatorEntry::Inbox {
                    text, open_tasks, ..
                }
                | NavigatorEntry::Project {
                    text, open_tasks, ..
                } => Some((text.clone(), *open_tasks)),
                NavigatorEntry::Bookmark { .. } | NavigatorEntry::View { .. } => None,
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
        state.update(Message::ToggleNavigator);
        input(
            &mut state,
            &[NavigatorInput::Focus(1), NavigatorInput::Collapse],
        );
        assert_eq!(counts(&state), [all[0].clone(), all[4].clone()], "folded");
        input(&mut state, &[NavigatorInput::Expand]);
        state.update(Message::ToggleShowDone);
        assert_eq!(
            counts(&state),
            [all[0].clone(), all[1].clone(), all[4].clone()]
        );
    }

    // D-85, D-67
    #[test]
    fn bookmarks_count_the_open_tasks_they_hold_and_a_view_the_open_tasks_it_lists() {
        let mut state = open(
            "Plan @bookmark\n\t- a\n\t- b @done\n\t\t- c\n\t- d @bookmark\n\t\t- e @due(2026-10-05)\n\
             - f @star\n\tnote @star\n- g @star @done\nLast @bookmark",
        );
        state.update(Message::Tick {
            now: std::time::Duration::ZERO,
            today: Date::constant(2026, 10, 5),
        });
        let counts = |state: &State| -> Vec<usize> {
            view(state)
                .entries
                .iter()
                .map(NavigatorEntry::open_tasks)
                .collect()
        };
        assert_eq!(
            listed(&state)[..4],
            ["today", "bookmark Plan", "bookmark d", "bookmark Last"]
        );
        assert_eq!(
            counts(&state)[..4],
            [2, 3, 1, 0],
            "Today lists e and f, not the note or done g; Plan holds a, d and e, not done b's c; d \
             holds e, not itself"
        );
        state.update(Message::Tick {
            now: std::time::Duration::from_secs(1),
            today: Date::constant(2026, 10, 4),
        });
        assert_eq!(counts(&state)[0], 1, "the count follows the date");
        state.update(Message::Configure(Configuration::TodayStarred(false)));
        assert_eq!(counts(&state)[0], 0, "and Today's starred setting");
    }

    // D-67, D-80
    #[test]
    fn each_view_counts_the_open_tasks_it_lists() {
        let mut state = State::new(
            format::parse(
                "Home:\n\t- a @due(2026-10-04) @red\n\t- b @due(2026-10-05) @star\n\
                 \t- c @start(2026-10-07) @flag\n\tnote @due(2026-10-01) @blue\n\
                 \t- d @done(2026-10-03) @due(2026-10-01)\n\t\t- e @due(2026-10-01) @green\n\
                 Done: @done(2026-10-05)\n\t- f @start(2026-10-06) @star @flag @orange\n\
                 - g @due(2026-10-12) @done(2026-09-01)\n- h @due(2026-10-12)\n- i @flag",
            ),
            None,
        );
        state.update(Message::Tick {
            now: std::time::Duration::ZERO,
            today: Date::constant(2026, 10, 5),
        });
        for show_done in [false, true] {
            if show_done {
                state.update(Message::ToggleShowDone);
            }
            let mut every = Pane::first();
            every.show_done = show_done;
            let counts: Vec<usize> = view(&state)
                .entries
                .iter()
                .filter_map(|entry| match entry {
                    NavigatorEntry::View { open_tasks, .. } => Some(*open_tasks),
                    _ => None,
                })
                .collect();
            let listed: Vec<usize> = SpecialView::ALL
                .into_iter()
                .map(|view| {
                    let rows = state.view_rows(&every, view);
                    rows.iter().filter(|row| open_task(row)).count()
                })
                .collect();
            assert_eq!(counts, listed, "show_done {show_done}");
            assert_eq!(counts, [2, 2, 1, 1, 0, 1, 2], "show_done {show_done}");
        }
    }

    fn typed(state: &mut State, text: &str) {
        for c in text.chars() {
            state.update(Message::Navigator(NavigatorInput::Type(c)));
        }
    }

    // D-86
    #[test]
    fn typing_lists_the_entries_holding_it_with_the_projects_above_them() {
        let mut state = open(OUTLINE);
        state.update(Message::ToggleNavigator);
        typed(&mut state, "DR");
        assert_eq!(
            listed(&state),
            ["Leaf Work", "  Leaf Drafts"],
            "ignoring case, without folds"
        );
        assert_eq!(
            focused(&state),
            Some(1),
            "on the first match, not the project above it"
        );
        assert_eq!(view(&state).filter, "DR");
        typed(&mut state, "x");
        assert_eq!(listed(&state), Vec::<String>::new());
        assert_eq!(focused(&state), None);
        input(&mut state, &[NavigatorInput::Erase; 3]);
        typed(&mut state, "o");
        assert_eq!(
            listed(&state),
            [
                "today",
                "bookmark Home",
                "Leaf Work",
                "  Leaf Drafts",
                "    Leaf Old",
                "Leaf Home"
            ],
            "Today, a bookmark and projects"
        );
        assert_eq!(focused(&state), Some(0));
        input(&mut state, &[NavigatorInput::Erase]);
        assert_eq!(listed(&state), listed(&open(OUTLINE)), "no filter left");
        assert_eq!(view(&state).filter, "");
        input(&mut state, &[NavigatorInput::Next, NavigatorInput::Erase]);
        assert_eq!(
            focused(&state),
            Some(1),
            "erasing an empty filter leaves the focus"
        );
    }

    // D-86
    #[test]
    fn a_filter_looks_through_folds_which_it_leaves_alone_and_closing_drops_it() {
        let mut state = open(OUTLINE);
        state.update(Message::ToggleNavigator);
        input(
            &mut state,
            &[NavigatorInput::Focus(3), NavigatorInput::Collapse],
        );
        typed(&mut state, "old");
        assert_eq!(
            listed(&state),
            ["Leaf Work", "  Leaf Drafts", "    Leaf Old"]
        );
        input(
            &mut state,
            &[
                NavigatorInput::Focus(0),
                NavigatorInput::Collapse,
                NavigatorInput::Expand,
            ],
        );
        input(&mut state, &[NavigatorInput::Erase; 3]);
        assert_eq!(
            listed(&state)[3],
            "Collapsed Work",
            "folding while filtering does nothing"
        );
        typed(&mut state, "home");
        input(&mut state, &[NavigatorInput::Leave]);
        state.update(Message::ToggleNavigator);
        assert_eq!(view(&state).filter, "");
        assert_eq!(
            listed(&state).len(),
            5,
            "everything but what Work's fold hides"
        );
        input(&mut state, &[NavigatorInput::Leave]);
        typed(&mut state, "a");
        assert_eq!(
            view(&state).filter,
            "",
            "typing does nothing while it is closed"
        );
    }

    // FR-KEY-05
    #[test]
    fn focusing_an_entry_moves_the_focus_there_while_the_navigator_is_open() {
        let mut state = open(OUTLINE);
        input(&mut state, &[NavigatorInput::Focus(3)]);
        assert!(
            !state.navigator_focused(),
            "a closed Navigator takes no keys"
        );
        state.update(Message::ToggleNavigator);
        input(&mut state, &[NavigatorInput::Focus(3)]);
        assert_eq!(focused(&state), Some(3));
        let past = listed(&state).len();
        input(&mut state, &[NavigatorInput::Focus(past)]);
        assert_eq!(focused(&state), Some(3), "no entry there");
        input(&mut state, &[NavigatorInput::Choose]);
        assert_eq!(state.breadcrumbs(), ["Work"]);
        assert_eq!(state.navigator(), None, "closed");
    }

    // FR-UI-01
    #[test]
    fn an_outline_without_bookmarks_or_projects_lists_only_the_special_views() {
        let views = [
            "today", "upcoming", "overdue", "colors", "logbook", "starred", "flagged",
        ];
        assert_eq!(listed(&State::new(format::parse("- a\nb"), None)), views);
        assert_eq!(listed(&State::new(Document::default(), None)), views);
    }

    // FR-TAG-13, D-92: Starred follows the other special views unless its setting is off, and
    // choosing it shows it.
    #[test]
    fn starred_follows_the_other_views_unless_its_setting_is_off() {
        let mut state = State::new(format::parse("INBOX:\nA: @bookmark\n"), None);
        assert!(state.navigator_lists(SpecialView::Starred), "on by default");
        assert_eq!(
            listed(&state),
            [
                "inbox INBOX",
                "today",
                "upcoming",
                "overdue",
                "colors",
                "logbook",
                "starred",
                "flagged",
                "bookmark A",
                "Leaf A"
            ]
        );
        state.update(Message::ToggleNavigator);
        input(
            &mut state,
            &[
                NavigatorInput::Next,
                NavigatorInput::Next,
                NavigatorInput::Next,
                NavigatorInput::Next,
                NavigatorInput::Next,
                NavigatorInput::Next,
                NavigatorInput::Choose,
            ],
        );
        assert_eq!(state.location(), Location::View(SpecialView::Starred));
        assert_eq!(current(&state), Some(6), "its entry is current");
        state.update(Message::Configure(Configuration::NavigatorView(
            SpecialView::Starred,
            false,
        )));
        assert!(!state.navigator_lists(SpecialView::Starred));
        assert!(state.navigator_lists(SpecialView::Today));
        assert_eq!(
            listed(&state),
            [
                "inbox INBOX",
                "today",
                "upcoming",
                "overdue",
                "colors",
                "logbook",
                "flagged",
                "bookmark A",
                "Leaf A"
            ]
        );
    }

    // FR-TAG-07, D-80: the special views stand in the Navigator's order after the inbox, each
    // unless its setting leaves it out, and choosing one shows it.
    #[test]
    fn the_special_views_follow_the_inbox_each_unless_its_setting_is_off() {
        let mut state = State::new(format::parse("INBOX:\nA: @bookmark\n"), None);
        assert_eq!(
            listed(&state),
            [
                "inbox INBOX",
                "today",
                "upcoming",
                "overdue",
                "colors",
                "logbook",
                "starred",
                "flagged",
                "bookmark A",
                "Leaf A"
            ]
        );
        state.update(Message::ToggleNavigator);
        input(&mut state, &[NavigatorInput::Next, NavigatorInput::Next]);
        input(&mut state, &[NavigatorInput::Choose]);
        assert_eq!(state.location(), Location::View(SpecialView::Upcoming));
        state.update(Message::Configure(Configuration::NavigatorView(
            SpecialView::Today,
            false,
        )));
        assert!(!state.navigator_lists(SpecialView::Today));
        assert!(state.navigator_lists(SpecialView::Upcoming));
        assert_eq!(
            listed(&state),
            [
                "inbox INBOX",
                "upcoming",
                "overdue",
                "colors",
                "logbook",
                "starred",
                "flagged",
                "bookmark A",
                "Leaf A"
            ]
        );
        state.update(Message::ToggleNavigator);
        input(&mut state, &[NavigatorInput::Next; 3]);
        input(&mut state, &[NavigatorInput::Choose]);
        assert_eq!(state.location(), Location::View(SpecialView::Logbook));
    }

    // FR-TAG-14, D-93: Flagged comes last of the special views unless its setting is off, and
    // choosing it shows it.
    #[test]
    fn flagged_comes_after_the_other_views_unless_its_setting_is_off() {
        let mut state = State::new(format::parse("INBOX:\nA: @bookmark\n"), None);
        assert!(state.navigator_lists(SpecialView::Flagged), "on by default");
        state.update(Message::ToggleNavigator);
        input(&mut state, &[NavigatorInput::Next; 7]);
        input(&mut state, &[NavigatorInput::Choose]);
        assert_eq!(state.location(), Location::View(SpecialView::Flagged));
        assert_eq!(current(&state), Some(7), "its entry is current");
        state.update(Message::Configure(Configuration::NavigatorView(
            SpecialView::Flagged,
            false,
        )));
        assert!(!state.navigator_lists(SpecialView::Flagged));
        assert_eq!(
            listed(&state),
            [
                "inbox INBOX",
                "today",
                "upcoming",
                "overdue",
                "colors",
                "logbook",
                "starred",
                "bookmark A",
                "Leaf A"
            ]
        );
        assert_eq!(current(&state), None, "no entry goes there");
    }

    // FR-UI-01, D-68, D-79
    #[test]
    fn toggling_opens_the_navigator_with_the_keys_and_closes_it() {
        let mut state = open(OUTLINE);
        assert_eq!(state.navigator(), None, "closed at the start");
        assert!(!state.navigator_focused());
        state.update(Message::ToggleNavigator);
        assert!(state.navigator_focused());
        assert_eq!(focused(&state), Some(0), "the pane is at home");
        state.update(Message::ToggleNavigator);
        assert_eq!(state.navigator(), None);
        assert!(!state.navigator_focused());
    }

    // FR-MODE-01
    #[test]
    fn focusing_the_navigator_ends_editing() {
        let mut state = open(OUTLINE);
        state.update(Message::EditAtEnd);
        state.update(Message::ToggleNavigator);
        assert_eq!(state.mode(), Mode::Navigate);
    }

    // keybindings.md §4
    #[test]
    fn next_and_previous_move_the_focus_and_stop_at_the_ends() {
        let mut state = open(OUTLINE);
        state.update(Message::ToggleNavigator);
        input(&mut state, &[NavigatorInput::Previous]);
        assert_eq!(focused(&state), Some(0));
        input(&mut state, &[NavigatorInput::Next, NavigatorInput::Next]);
        assert_eq!(focused(&state), Some(2));
        input(&mut state, &[NavigatorInput::Previous]);
        assert_eq!(focused(&state), Some(1));
        input(&mut state, &[NavigatorInput::Next; 10]);
        assert_eq!(focused(&state), Some(7), "the last entry");
    }

    // keybindings.md §4
    #[test]
    fn keys_of_the_navigator_do_nothing_until_it_is_open() {
        let mut state = open(OUTLINE);
        input(
            &mut state,
            &[NavigatorInput::Next, NavigatorInput::Collapse],
        );
        assert_eq!(focused(&state), None);
        assert_eq!(listed(&state)[3], "Expanded Work");
    }

    // keybindings.md §4
    #[test]
    fn collapse_and_expand_fold_project_entries_only() {
        let mut state = open(OUTLINE);
        state.update(Message::ToggleNavigator);
        input(&mut state, &[NavigatorInput::Next; 3]);
        input(&mut state, &[NavigatorInput::Collapse]);
        assert_eq!(
            listed(&state)[2..],
            ["bookmark Home", "Collapsed Work", "Leaf Home"],
            "child projects hide"
        );
        input(&mut state, &[NavigatorInput::Collapse]);
        assert_eq!(listed(&state)[3], "Collapsed Work", "still collapsed");
        input(&mut state, &[NavigatorInput::Expand]);
        assert_eq!(listed(&state)[4], "  Expanded Drafts");
        input(&mut state, &[NavigatorInput::Expand]);
        assert_eq!(listed(&state)[3], "Expanded Work", "still expanded");
        input(&mut state, &[NavigatorInput::Next, NavigatorInput::Next]);
        input(&mut state, &[NavigatorInput::Collapse]);
        assert_eq!(
            listed(&state)[5],
            "    Leaf Old",
            "a leaf has nothing to fold"
        );
        for at in [0, 2] {
            let mut state = open(OUTLINE);
            state.update(Message::ToggleNavigator);
            input(&mut state, &vec![NavigatorInput::Next; at]);
            input(
                &mut state,
                &[NavigatorInput::Collapse, NavigatorInput::Expand],
            );
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
        state.update(Message::ToggleNavigator);
        input(&mut state, &[NavigatorInput::Next, NavigatorInput::Choose]);
        assert_eq!(zoomed(&state).as_deref(), Some("Launch plan"));
        assert!(!state.navigator_focused());
        state.update(Message::ToggleNavigator);
        assert_eq!(focused(&state), Some(1), "on the entry of the zoom");
        input(&mut state, &[NavigatorInput::Next; 3]);
        input(&mut state, &[NavigatorInput::Choose]);
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
        state.update(Message::ToggleNavigator);
        input(&mut state, &[NavigatorInput::Next, NavigatorInput::Choose]);
        state.update(Message::ToggleNavigator);
        input(
            &mut state,
            &[NavigatorInput::Previous, NavigatorInput::Choose],
        );
        assert_eq!(state.location(), Location::View(SpecialView::Today));
        assert!(!state.navigator_focused());
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
        state.update(Message::ToggleNavigator);
        input(
            &mut state,
            &[NavigatorInput::Focus(3), NavigatorInput::Collapse],
        );
        input(&mut state, &[NavigatorInput::Leave]);
        assert_eq!(current(&state), Some(3), "Work, folded over Drafts");
    }

    // D-68
    #[test]
    fn the_navigator_opens_on_the_current_entry_which_stays_marked() {
        let mut state = open(OUTLINE);
        state.update(Message::ZoomIn);
        state.update(Message::ToggleNavigator);
        assert_eq!(focused(&state), Some(3), "on Work");
        assert_eq!(current(&state), Some(3));
        input(&mut state, &[NavigatorInput::Next]);
        assert_eq!(focused(&state), Some(4));
        assert_eq!(current(&state), Some(3), "still Work");
        state.update(Message::ToggleNavigator);
        state.update(Message::ToggleNavigator);
        assert_eq!(focused(&state), Some(3), "on Work again");
    }

    // keybindings.md §4
    #[test]
    fn leaving_closes_the_navigator_on_the_same_view() {
        let mut state = open(OUTLINE);
        state.update(Message::ToggleNavigator);
        input(&mut state, &[NavigatorInput::Next, NavigatorInput::Leave]);
        assert!(!state.navigator_focused());
        assert_eq!(state.navigator(), None);
        assert_eq!(zoomed(&state), None);
    }

    // FR-UI-01
    #[test]
    fn the_focus_stays_on_the_last_entry_when_entries_go_away() {
        let mut state = open("- a @bookmark\nP:");
        state.update(Message::ToggleNavigator);
        input(&mut state, &[NavigatorInput::Next, NavigatorInput::Next]);
        assert_eq!(focused(&state), Some(2));
        state.update(Message::ToggleFlag(Flag::Bookmark));
        assert_eq!(listed(&state), ["today", "Leaf P"]);
        assert_eq!(focused(&state), Some(1));
        input(&mut state, &[NavigatorInput::Previous]);
        assert_eq!(focused(&state), Some(0));
    }

    /// The labels of the focused entry's context-menu commands, each group's joined by `, `.
    fn entry_commands(state: &State) -> Option<Vec<String>> {
        let groups = state.navigator_commands()?;
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
    fn the_focused_entrys_commands_go_to_it_open_it_fold_it_and_close_the_navigator() {
        let mut state = open(OUTLINE);
        assert_eq!(entry_commands(&state), None, "the pane has the keys");
        state.update(Message::ToggleNavigator);
        let leave = "Return to Outline".to_owned();
        assert_eq!(
            entry_commands(&state),
            Some(vec![
                "Go to Entry, Open in New Pane".to_owned(),
                leave.clone()
            ]),
            "Today"
        );
        input(&mut state, &[NavigatorInput::Next, NavigatorInput::Next]);
        assert_eq!(
            entry_commands(&state),
            Some(vec![
                "Go to Entry, Open in New Pane".to_owned(),
                leave.clone()
            ]),
            "a bookmark"
        );
        input(&mut state, &[NavigatorInput::Next]);
        assert_eq!(
            entry_commands(&state),
            Some(vec![
                "Go to Entry, Open in New Pane, Collapse Entry".to_owned(),
                leave.clone()
            ])
        );
        input(&mut state, &[NavigatorInput::Collapse]);
        assert_eq!(
            entry_commands(&state),
            Some(vec![
                "Go to Entry, Open in New Pane, Expand Entry".to_owned(),
                leave.clone()
            ])
        );
        input(&mut state, &[NavigatorInput::Next]);
        assert_eq!(
            entry_commands(&state),
            Some(vec![
                "Go to Entry, Open in New Pane".to_owned(),
                leave.clone()
            ]),
            "a project without child projects"
        );
        typed(&mut state, "drafts");
        assert_eq!(
            entry_commands(&state),
            Some(vec![
                "Go to Entry, Open in New Pane".to_owned(),
                leave.clone()
            ]),
            "no folding while a filter is typed (D-86)"
        );
        let groups = state.navigator_commands().unwrap();
        assert_eq!(
            groups[1][0].command,
            Command::Navigator(NavigatorInput::Leave)
        );
        assert_eq!(
            groups[0][0].command,
            Command::Navigator(NavigatorInput::Choose)
        );
    }
}
