//! Count: how many blocks are under the focused block and how many words they hold
//! ([editing.md §4](../../../docs/specs/editing.md#4-structural-operations-fr-ops), FR-OPS-09).

use super::{Notice, State};
use crate::document::{ItemId, blocks};

impl State {
    /// Shows how many blocks are under the focused block, which is the zoom root when its title
    /// is focused and an entry in the Today view, and how many words their text holds, completed
    /// and folded ones included; none when nothing is focused. A divider's `---` is no word.
    pub(super) fn count(&mut self) {
        let (mut count, mut words) = (0, 0);
        let mut pending: Vec<ItemId> = self.pane.focus.into_iter().collect();
        while let Some(line) = pending.pop() {
            for block in blocks::each_child(&self.document, line) {
                count += 1;
                if !block.divider() {
                    words += self.shown_text(&block).split_whitespace().count();
                }
                pending.push(block.id());
            }
        }
        self.notify(Notice::Count {
            blocks: count,
            words,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::{Location, Message},
        document::{Document, format},
    };

    fn counted(state: &mut State) -> Notice {
        state.update(Message::Count);
        state.notice().cloned().expect("counting shows a notice")
    }

    fn count(blocks: usize, words: usize) -> Notice {
        Notice::Count { blocks, words }
    }

    // FR-OPS-09
    #[test]
    fn counts_the_blocks_and_words_under_the_focused_block() {
        let text = "Trip: @today\n\t- Book **the** hotel @due(2026-10-09)\n\t\tNear the station\n\t---\n\t- Pack @done\nOther\n";
        let mut state = State::new(format::parse(text), None);
        assert_eq!(counted(&mut state), count(4, 7), "done ones count too");
        state.update(Message::FocusNext);
        assert_eq!(counted(&mut state), count(1, 3));
        state.update(Message::FocusNext);
        assert_eq!(counted(&mut state), count(0, 0), "nothing under a leaf");
    }

    // FR-OPS-09
    #[test]
    fn counts_under_the_zoom_root_when_its_title_is_focused_and_nothing_when_nothing_is() {
        let mut state = State::new(format::parse("A\n\tb c\n\t\td\nE\n"), None);
        state.update(Message::ZoomIn);
        assert_eq!(counted(&mut state), count(1, 1));
        state.update(Message::FocusPrevious);
        assert_eq!(counted(&mut state), count(2, 3), "the title is focused");
        let mut empty = State::new(Document::default(), None);
        assert_eq!(counted(&mut empty), count(0, 0));
    }

    // FR-OPS-09, navigation.md §7
    #[test]
    fn counts_under_the_focused_entry_of_the_today_view_and_nothing_when_it_is_empty() {
        let mut state = State::new(format::parse("A\n\t- b @today\n\t\tc d\nE\n"), None);
        state.go_to(Location::Today);
        assert_eq!(counted(&mut state), count(1, 2));
        let mut none = State::new(format::parse("A\n\tb\n"), None);
        none.go_to(Location::Today);
        assert_eq!(counted(&mut none), count(0, 0), "not the whole document");
    }
}
