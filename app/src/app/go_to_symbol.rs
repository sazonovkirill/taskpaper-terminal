//! Go to Symbol: a fuzzy finder over every block, bookmarks first, then projects and sections,
//! or over the projects alone as Go to Project, that zooms to the chosen block's parent and
//! focuses it
//! ([navigation.md §8](../../../docs/specs/navigation.md#8-go-to-symbol-and-go-to-command-fr-nav-05-fr-nav-06),
//! FR-NAV-05, D-65).

use std::collections::HashSet;

use super::{
    Location, MenuInput, Row, Rows, State,
    finder::{Candidate, Detail, Finder, FinderView},
    picker::Picked,
};
use crate::document::{Block, BlockKind, ItemId, blocks};

/// Which blocks Go to Symbol lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Symbols {
    /// Every block but dividers (FR-NAV-05).
    All,
    /// Only projects, as Go to Project (D-65).
    Projects,
}

/// The blocks among `rows` that are `excluded` or inside one.
fn left_out(rows: &Rows, excluded: &[Block]) -> HashSet<ItemId> {
    let mut left_out = HashSet::new();
    // The depth of the excluded block whose rows are being passed.
    let mut passing = None;
    for row in rows {
        if passing.is_none_or(|depth| row.depth <= depth) {
            passing = excluded
                .iter()
                .any(|gone| gone.id() == row.id)
                .then_some(row.depth);
        }
        if passing.is_some() {
            left_out.insert(row.id);
        }
    }
    left_out
}

/// How a block ranks in Go to Symbol, by its row: bookmarks, then projects and sections, then the
/// rest.
fn tier(row: &Row) -> u8 {
    if row.bookmark {
        0
    } else if row.kind == BlockKind::Project || row.section {
        1
    } else {
        2
    }
}

impl State {
    /// Go to Symbol over every block, while it is open (FR-NAV-05).
    #[must_use]
    pub fn go_to_symbol(&self) -> Option<FinderView<'_, ItemId>> {
        self.symbols_view(Symbols::All)
    }

    /// Go to Project, while it is open (D-65).
    #[must_use]
    pub fn go_to_project(&self) -> Option<FinderView<'_, ItemId>> {
        self.symbols_view(Symbols::Projects)
    }

    /// Go to Symbol while it is open over `listed`.
    fn symbols_view(&self, listed: Symbols) -> Option<FinderView<'_, ItemId>> {
        self.symbols
            .as_ref()
            .filter(|(open, _)| *open == listed)
            .map(|(_, finder)| finder.view())
    }

    /// `Ctrl+K`: opens Go to Symbol over every block in document order but dividers, or `gP`
    /// over the projects among them, as Go to Project.
    pub(super) fn open_go_to_symbol(&mut self, listed: Symbols) {
        self.symbols = Some((listed, Finder::new(self.symbol_candidates(listed))));
    }

    /// The candidates Go to Symbol lists over `listed`.
    fn symbol_candidates(&self, listed: Symbols) -> Vec<Candidate<ItemId>> {
        match listed {
            Symbols::All => self.block_candidates(&[]),
            Symbols::Projects => self.project_candidates(),
        }
    }

    /// Lists the blocks of a reloaded document in the open Go to Symbol or Go to Project, keeping
    /// its query and highlighting `highlighted` (file-io.md §3, D-104).
    pub(super) fn relist_symbols(&mut self, highlighted: Option<ItemId>) {
        if let Some(listed) = self.symbols.as_ref().map(|(listed, _)| *listed) {
            let candidates = self.symbol_candidates(listed);
            if let Some((_, finder)) = &mut self.symbols {
                finder.relist(candidates, highlighted.as_ref());
            }
        }
    }

    /// The candidates of [`State::block_candidates`] that are projects, as Go to Project and
    /// Quick Add's project picker list them (D-65, D-96).
    pub(super) fn project_candidates(&self) -> Vec<Candidate<ItemId>> {
        self.candidates(&[], Symbols::Projects)
    }

    /// A candidate for each block in document order, with the path of the blocks above it and
    /// ranked bookmarks first, then projects and sections, but none for dividers, which have no
    /// name, nor for `excluded` blocks and completed blocks while those are hidden (FR-DONE-02),
    /// nor what they hold (navigation.md §8, FR-OPS-05).
    pub(super) fn block_candidates(&self, excluded: &[Block]) -> Vec<Candidate<ItemId>> {
        self.candidates(excluded, Symbols::All)
    }

    /// The candidates of [`State::block_candidates`] for the blocks `listed` names, from the
    /// outline's rows with nothing collapsed, walked again only where a change built them again.
    fn candidates(&self, excluded: &[Block], listed: Symbols) -> Vec<Candidate<ItemId>> {
        let show_done = self.pane.show_done;
        let rows = self.unfolded_rows(Location::Outline(None), show_done);
        let candidates = self.candidates_walks.list(
            (listed, show_done),
            &rows,
            self,
            |above, row, candidates| {
                above.reach(row.depth);
                let lists = match listed {
                    Symbols::All => !row.divider,
                    Symbols::Projects => row.kind == BlockKind::Project,
                };
                if lists {
                    candidates.push(Candidate {
                        value: row.id,
                        text: self.shared_shown_text(row.id),
                        detail: Detail::path(above.path(self)),
                        tier: tier(row),
                    });
                }
                above.enter(row.id);
            },
        );
        if excluded.is_empty() {
            return candidates;
        }
        let left_out = left_out(&rows, excluded);
        candidates
            .into_iter()
            .filter(|candidate| !left_out.contains(&candidate.value))
            .collect()
    }

    /// Applies `input` to Go to Symbol or Go to Project, and once a block is chosen, zooms to its
    /// parent and focuses it (navigation.md §8).
    pub(super) fn go_to_symbol_input(&mut self, input: MenuInput) {
        let Some(closed) = self
            .symbols
            .as_mut()
            .and_then(|(_, finder)| finder.input(input))
        else {
            return;
        };
        self.symbols = None;
        if let Picked::Chosen(id) = closed
            && let Some(block) = blocks::block(&self.document, id)
        {
            let parent = blocks::parent(&self.document, id).map(|parent| parent.id());
            self.go_to(Location::Outline(parent));
            self.pane.focus = Some(block.id());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::{Menu, Message},
        document::{Document, format},
    };

    const TEXT: &str = "Home:\n\t- Paint the **hall**\n\tNotes @bookmark\n---\nWork:\n\t- Write report\n\tLater @section\n\t\t- Call Ann";

    fn state() -> State {
        State::new(format::parse(TEXT), None)
    }

    /// The text and path of each entry Go to Symbol lists, and the highlighted position.
    fn listed(state: &State) -> (Vec<(String, String)>, usize) {
        let view = state.go_to_symbol().unwrap();
        (
            view.entries
                .iter()
                .map(|entry| (entry.text.to_string(), entry.detail.to_string()))
                .collect(),
            view.selected,
        )
    }

    fn typed(state: &mut State, text: &str) {
        for c in text.chars() {
            state.update(Message::Menu(Menu::GoToSymbol, MenuInput::Type(c)));
        }
    }

    /// The text of the block `id`.
    fn text(document: &Document, id: Option<ItemId>) -> Option<String> {
        Some(document.get(id?)?.display_text().to_owned())
    }

    // FR-NAV-05
    #[test]
    fn lists_every_block_but_dividers_bookmarks_first_then_projects_and_sections() {
        let mut state = state();
        assert_eq!(state.go_to_symbol(), None);
        state.update(Message::OpenGoToSymbol(Symbols::All));
        let entry = |text: &str, path: &str| (text.to_owned(), path.to_owned());
        assert_eq!(
            listed(&state),
            (
                vec![
                    entry("Notes", "Home"),
                    entry("Home", ""),
                    entry("Work", ""),
                    entry("Later", "Work"),
                    entry("Paint the hall", "Home"),
                    entry("Write report", "Work"),
                    entry("Call Ann", "Work › Later"),
                ],
                0
            )
        );
    }

    // FR-NAV-05, navigation.md §8
    #[test]
    fn choosing_a_block_zooms_to_its_parent_and_focuses_it() {
        let mut state = state();
        state.update(Message::OpenGoToSymbol(Symbols::All));
        typed(&mut state, "ann");
        assert_eq!(listed(&state).0.len(), 1);
        state.update(Message::Menu(Menu::GoToSymbol, MenuInput::Choose));
        assert_eq!(state.go_to_symbol(), None);
        assert_eq!(text(state.document(), state.zoom()).unwrap(), "Later");
        assert_eq!(text(state.document(), state.focus()).unwrap(), "Call Ann");
        state.update(Message::Back);
        assert_eq!(state.zoom(), None, "the zoom is a step in the history");
    }

    // FR-NAV-05, FR-DONE-02
    #[test]
    fn hidden_completed_blocks_are_not_listed() {
        let mut state = State::new(
            format::parse("A:\n\t- done @done\n\t\t- inside\n\t- open"),
            None,
        );
        state.update(Message::ToggleShowDone);
        state.update(Message::OpenGoToSymbol(Symbols::All));
        let texts: Vec<String> = listed(&state).0.into_iter().map(|(text, _)| text).collect();
        assert_eq!(texts, ["A", "open"]);
    }

    // navigation.md §8
    #[test]
    fn a_top_level_block_is_focused_at_home() {
        let mut state = state();
        state.update(Message::FocusLast);
        state.update(Message::ZoomIn);
        state.update(Message::OpenGoToSymbol(Symbols::All));
        typed(&mut state, "work");
        state.update(Message::Menu(Menu::GoToSymbol, MenuInput::Choose));
        assert_eq!(state.zoom(), None);
        assert_eq!(text(state.document(), state.focus()).unwrap(), "Work");
    }

    #[test]
    fn dismissing_or_choosing_nothing_leaves_the_view_as_it_was() {
        let mut state = state();
        state.update(Message::OpenGoToSymbol(Symbols::All));
        state.update(Message::Menu(Menu::GoToSymbol, MenuInput::Dismiss));
        assert_eq!(state.go_to_symbol(), None);
        assert_eq!(text(state.document(), state.focus()).unwrap(), "Home");
        state.update(Message::OpenGoToSymbol(Symbols::All));
        typed(&mut state, "zzz");
        state.update(Message::Menu(Menu::GoToSymbol, MenuInput::Choose));
        assert!(
            state.go_to_symbol().is_some(),
            "nothing to choose keeps it open"
        );
        assert_eq!(state.zoom(), None);
    }

    /// Projects at several depths, under a note, bookmarked and done, among other blocks.
    const PROJECTS: &str = "Home:\n\t- Paint the hall\n\tGarden:\n\t\t- Weed\nIdeas\n\tTrip: @bookmark\nOld: @done\nWork:\n\tLater @section";

    /// The text and path of each entry Go to Project lists.
    fn projects_listed(state: &State) -> Vec<(String, String)> {
        state
            .go_to_project()
            .unwrap()
            .entries
            .iter()
            .map(|entry| (entry.text.to_string(), entry.detail.to_string()))
            .collect()
    }

    // D-65, FR-DONE-02
    #[test]
    fn go_to_project_lists_only_projects_with_their_paths_bookmarks_first() {
        let mut state = State::new(format::parse(PROJECTS), None);
        state.update(Message::OpenGoToSymbol(Symbols::Projects));
        assert_eq!(state.go_to_symbol(), None, "Go to Symbol is not open");
        let entry = |text: &str, path: &str| (text.to_owned(), path.to_owned());
        assert_eq!(
            projects_listed(&state),
            [
                entry("Trip", "Ideas"),
                entry("Home", ""),
                entry("Garden", "Home"),
                entry("Old", ""),
                entry("Work", ""),
            ]
        );
        state.update(Message::Menu(Menu::GoToSymbol, MenuInput::Dismiss));
        state.update(Message::ToggleShowDone);
        state.update(Message::OpenGoToSymbol(Symbols::Projects));
        assert!(
            !projects_listed(&state).contains(&entry("Old", "")),
            "hidden completed projects are not listed"
        );
        state.update(Message::Menu(Menu::GoToSymbol, MenuInput::Dismiss));
        state.update(Message::OpenGoToSymbol(Symbols::All));
        assert_eq!(state.go_to_project(), None, "Go to Project is not open");
    }

    // D-65, navigation.md §8
    #[test]
    fn choosing_a_project_zooms_to_its_parent_and_focuses_it() {
        let mut state = State::new(format::parse(PROJECTS), None);
        state.update(Message::OpenGoToSymbol(Symbols::Projects));
        typed(&mut state, "gar");
        state.update(Message::Menu(Menu::GoToSymbol, MenuInput::Choose));
        assert_eq!(state.go_to_project(), None);
        assert_eq!(text(state.document(), state.zoom()).unwrap(), "Home");
        assert_eq!(text(state.document(), state.focus()).unwrap(), "Garden");
    }

    #[test]
    fn keys_without_go_to_symbol_open_do_nothing() {
        let mut state = state();
        state.update(Message::Menu(Menu::GoToSymbol, MenuInput::Choose));
        assert_eq!(state.go_to_symbol(), None);
        assert_eq!(text(state.document(), state.focus()).unwrap(), "Home");
    }

    #[test]
    fn a_block_deleted_while_listed_is_not_gone_to() {
        let mut state = state();
        state.update(Message::OpenGoToSymbol(Symbols::All));
        typed(&mut state, "ann");
        state.document = format::parse("Other:\n\t- Task");
        state.update(Message::Menu(Menu::GoToSymbol, MenuInput::Choose));
        assert_eq!(state.zoom(), None);
    }
}
