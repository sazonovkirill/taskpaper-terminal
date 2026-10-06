//! Naming a block by its place in the outline instead of by its id, which holds only within one
//! document, so that a reloaded document can be matched with the old one and a layout kept
//! between sessions with the file ([file-io.md §3](../../../docs/specs/file-io.md#3-external-changes),
//! [§5](../../../docs/specs/file-io.md#5-view-state-persistence)).

use serde::{Deserialize, Serialize};

use crate::document::{Document, Item, ItemId, blocks};

/// A block's place in an outline: for each block from the top level down to it, its text as shown
/// without tags, so that a change of tags alone keeps the match, and how many earlier sibling
/// blocks have the same text. It is kept as a list of `[text, count]` pairs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct TextPath(Vec<(String, usize)>);

impl TextPath {
    /// The place of block `id` in `document`, or `None` when it is no block there.
    pub(super) fn of(document: &Document, id: ItemId) -> Option<Self> {
        let mut steps: Vec<(String, usize)> =
            std::iter::successors(blocks::block(document, id), |block| {
                blocks::parent(document, block.id())
            })
            .filter_map(|block| step(document, block.id()))
            .collect();
        steps.reverse();
        (!steps.is_empty()).then_some(Self(steps))
    }

    /// The block of `document` at this place, with `true`; or, when the path breaks off, the
    /// nearest block on it that matched, with `false`; `None` when not even the top-level block
    /// matched.
    pub(super) fn find(&self, document: &Document) -> Option<(ItemId, bool)> {
        let mut line = document.root();
        let mut found = None;
        for (text, nth) in &self.0 {
            let Some(next) = blocks::children(document, line)
                .into_iter()
                .filter(|block| shown_text(document, block.id()).as_ref() == Some(text))
                .nth(*nth)
            else {
                return found.map(|id| (id, false));
            };
            line = next.id();
            found = Some(next.id());
        }
        found.map(|id| (id, true))
    }
}

/// The text of block `id` and how many earlier sibling blocks have the same one.
fn step(document: &Document, id: ItemId) -> Option<(String, usize)> {
    let text = shown_text(document, id)?;
    let parent = document.get(id)?.parent()?;
    let nth = blocks::children(document, parent)
        .into_iter()
        .take_while(|block| block.id() != id)
        .filter(|block| shown_text(document, block.id()).as_ref() == Some(&text))
        .count();
    Some((text, nth))
}

/// The text of block `id` as shown.
fn shown_text(document: &Document, id: ItemId) -> Option<String> {
    document.get(id).map(Item::display_text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::format;

    /// The block of `document` whose first line reads `line`, the `nth` such from the top.
    fn block(document: &Document, line: &str, nth: usize) -> ItemId {
        document
            .descendants(document.root())
            .map(|(id, _)| id)
            .filter(|&id| self::line(document, id) == line)
            .nth(nth)
            .expect("the line is in the outline")
    }

    /// The first line of block `id`.
    fn line(document: &Document, id: ItemId) -> &str {
        document.get(id).map_or("", Item::text)
    }

    /// Where the block reading `line`, the `nth` such in `from`, is in `to`.
    fn moved(from: &str, line_text: &str, nth: usize, to: &str) -> Option<(String, bool)> {
        let from = format::parse(from);
        let to = format::parse(to);
        let path = TextPath::of(&from, block(&from, line_text, nth))?;
        path.find(&to)
            .map(|(id, exact)| (line(&to, id).to_owned(), exact))
    }

    // file-io.md §3
    #[test]
    fn finds_a_block_by_the_texts_down_to_it_wherever_its_lines_moved() {
        let from = "A:\n\t- b\n\t- c\nD:\n";
        let to = "Intro\nD:\nA:\n\tnote\n\t- c\n\t- b\n";
        assert_eq!(moved(from, "- b", 0, to), Some(("- b".to_owned(), true)));
        assert_eq!(moved(from, "D:", 0, to), Some(("D:".to_owned(), true)));
    }

    // file-io.md §3
    #[test]
    fn tells_blocks_with_equal_texts_apart_by_their_order_among_siblings() {
        let from = "A:\n\t- x\n\t- x\n\t- y\nB:\n\t- x\n";
        let to = "A:\n\t- y\n\t- x\n\t- z\n\t- x\nB:\n\t- x\n";
        let second = format::parse(to);
        assert_eq!(moved(from, "- x", 1, to), Some(("- x".to_owned(), true)));
        let found = TextPath::of(&format::parse(from), block(&format::parse(from), "- x", 1));
        let (id, _) = found.and_then(|path| path.find(&second)).unwrap();
        assert_eq!(id, block(&second, "- x", 1), "the second of its texts");
        assert_eq!(moved(from, "- x", 2, to), Some(("- x".to_owned(), true)));
    }

    // file-io.md §3: a change of tags alone keeps the match.
    #[test]
    fn matches_texts_without_their_tags() {
        assert_eq!(
            moved(
                "- a @due(2026-10-05)\n",
                "- a @due(2026-10-05)",
                0,
                "- a @done\n"
            ),
            Some(("- a @done".to_owned(), true))
        );
    }

    // file-io.md §3
    #[test]
    fn a_broken_path_finds_the_nearest_block_on_it_and_says_so() {
        let from = "A:\n\tB:\n\t\t- c\n";
        assert_eq!(
            moved(from, "- c", 0, "A:\n\tB:\n\t\t- d\n"),
            Some(("B:".to_owned(), false))
        );
        assert_eq!(
            moved(from, "- c", 0, "A:\n\t- c\n"),
            Some(("A:".to_owned(), false))
        );
        assert_eq!(moved(from, "- c", 0, "Z:\n"), None, "nothing on it matched");
        assert_eq!(moved(from, "A:", 0, ""), None);
    }

    #[test]
    fn only_a_block_has_a_place() {
        let document = format::parse("- a\n");
        assert_eq!(TextPath::of(&document, document.root()), None);
    }
}
