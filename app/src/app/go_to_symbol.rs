//! Go to Symbol: a fuzzy finder over every block, bookmarks first, then projects and sections,
//! that zooms to the chosen block's parent and focuses it
//! ([navigation.md §8](../../../docs/specs/navigation.md#8-go-to-symbol-and-go-to-command-fr-nav-05-fr-nav-06),
//! FR-NAV-05).

use super::{
    Location, MenuInput, State,
    finder::{Candidate, Finder, FinderView},
    picker::Picked,
};
use crate::document::{Block, BlockKind, ItemId, blocks};

/// How a block ranks in Go to Symbol: bookmarks, then projects and sections, then the rest.
fn tier(block: &Block) -> u8 {
    if block.bookmark() {
        0
    } else if block.kind() == BlockKind::Project || block.section() {
        1
    } else {
        2
    }
}

impl State {
    /// Go to Symbol, while it is open (FR-NAV-05).
    #[must_use]
    pub fn go_to_symbol(&self) -> Option<FinderView<'_, ItemId>> {
        self.symbols.as_ref().map(Finder::view)
    }

    /// `Ctrl+P`: opens Go to Symbol over every block in document order but dividers.
    pub(super) fn open_go_to_symbol(&mut self) {
        self.symbols = Some(Finder::new(self.block_candidates(&[])));
    }

    /// A candidate for each block in document order, with the path of the blocks above it and
    /// ranked bookmarks first, then projects and sections, but none for dividers, which have no
    /// name, nor for `excluded` blocks and completed blocks while those are hidden (FR-DONE-02),
    /// nor what they hold (navigation.md §8, FR-OPS-05).
    pub(super) fn block_candidates(&self, excluded: &[Block]) -> Vec<Candidate<ItemId>> {
        let mut candidates = Vec::new();
        self.push_candidates(self.document.root(), excluded, &mut candidates);
        candidates
    }

    /// Appends a candidate for each block below `line`, depth first, as
    /// [`State::block_candidates`] lists them.
    fn push_candidates(&self, line: ItemId, excluded: &[Block], out: &mut Vec<Candidate<ItemId>>) {
        for block in blocks::children(&self.document, line) {
            if !self.shows(&block) || excluded.iter().any(|gone| gone.id() == block.id()) {
                continue;
            }
            if !block.divider() {
                let mut path = self.path_to(&block);
                let text = path.pop().unwrap_or_default();
                out.push(Candidate {
                    value: block.id(),
                    text,
                    detail: path.join(" › "),
                    tier: tier(&block),
                });
            }
            self.push_candidates(block.id(), excluded, out);
        }
    }

    /// Applies `input` to Go to Symbol, and once a block is chosen, zooms to its parent and
    /// focuses it (navigation.md §8).
    pub(super) fn go_to_symbol_input(&mut self, input: MenuInput) {
        let Some(closed) = self.symbols.as_mut().and_then(|finder| finder.input(input)) else {
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
                .map(|entry| (entry.text.clone(), entry.detail.clone()))
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
        Some(document.get(id?)?.display_text())
    }

    // FR-NAV-05
    #[test]
    fn lists_every_block_but_dividers_bookmarks_first_then_projects_and_sections() {
        let mut state = state();
        assert_eq!(state.go_to_symbol(), None);
        state.update(Message::OpenGoToSymbol);
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
        state.update(Message::OpenGoToSymbol);
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
        state.update(Message::OpenGoToSymbol);
        let texts: Vec<String> = listed(&state).0.into_iter().map(|(text, _)| text).collect();
        assert_eq!(texts, ["A", "open"]);
    }

    // navigation.md §8
    #[test]
    fn a_top_level_block_is_focused_at_home() {
        let mut state = state();
        state.update(Message::FocusLast);
        state.update(Message::ZoomIn);
        state.update(Message::OpenGoToSymbol);
        typed(&mut state, "work");
        state.update(Message::Menu(Menu::GoToSymbol, MenuInput::Choose));
        assert_eq!(state.zoom(), None);
        assert_eq!(text(state.document(), state.focus()).unwrap(), "Work");
    }

    #[test]
    fn dismissing_or_choosing_nothing_leaves_the_view_as_it_was() {
        let mut state = state();
        state.update(Message::OpenGoToSymbol);
        state.update(Message::Menu(Menu::GoToSymbol, MenuInput::Dismiss));
        assert_eq!(state.go_to_symbol(), None);
        assert_eq!(text(state.document(), state.focus()).unwrap(), "Home");
        state.update(Message::OpenGoToSymbol);
        typed(&mut state, "zzz");
        state.update(Message::Menu(Menu::GoToSymbol, MenuInput::Choose));
        assert!(
            state.go_to_symbol().is_some(),
            "nothing to choose keeps it open"
        );
        assert_eq!(state.zoom(), None);
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
        state.update(Message::OpenGoToSymbol);
        typed(&mut state, "ann");
        state.document = format::parse("Other:\n\t- Task");
        state.update(Message::Menu(Menu::GoToSymbol, MenuInput::Choose));
        assert_eq!(state.zoom(), None);
    }
}
