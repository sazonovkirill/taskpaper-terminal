//! Naming a block by its place in the outline instead of by its id, which holds only within one
//! document, so that a reloaded document can be matched with the old one and a layout kept
//! between sessions with the file ([file-io.md §3](../../../docs/specs/file-io.md#3-external-changes),
//! [§5](../../../docs/specs/file-io.md#5-view-state-persistence)).

use std::collections::HashMap;

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

/// Where each block of `old` is in `new`, as [`TextPath::find`] finds its [`TextPath::of`]: the
/// block at its whole path with `true`, else the nearest block on it with `false`; a block of
/// which not even the top-level block matched is left out. One walk over `old` that groups each
/// line's children in `new` by text once, so that matching every collapsed block of a large
/// outline, as a reload does, takes time in proportion to the outline rather than to its blocks
/// times their siblings (N-PERF-02).
pub(super) fn matches(old: &Document, new: &Document) -> HashMap<ItemId, (ItemId, bool)> {
    let top = children_by_text(new, new.root());
    let mut found = HashMap::new();
    // Each line of `old` whose children are still to match, with the block of `new` its path
    // led to and whether all of it matched. The paths of the children of the root and of a
    // special node start again at the top level.
    let mut lines = vec![(old.root(), None, true)];
    while let Some((line, at, whole)) = lines.pop() {
        let here;
        let candidates = match (whole, at) {
            (false, _) => None,
            (true, None) => Some(&top),
            (true, Some(at)) => {
                here = children_by_text(new, at);
                Some(&here)
            }
        };
        let mut seen: HashMap<&str, usize> = HashMap::new();
        let children = old.get(line).map_or(&[][..], Item::children);
        for (id, item) in children.iter().filter_map(|&id| Some((id, old.get(id)?))) {
            if blocks::is_special(item) {
                lines.push((id, None, true));
                continue;
            }
            let nth = seen.entry(item.display_text()).or_default();
            let next = candidates
                .and_then(|candidates| candidates.get(item.display_text())?.get(*nth))
                .copied();
            *nth += 1;
            let (at, whole) = next.map_or((at, false), |next| (Some(next), true));
            if let Some(at) = at {
                found.insert(id, (at, whole));
            }
            lines.push((id, at, whole));
        }
    }
    found
}

/// The child blocks of the line `parent` grouped by text, in order.
fn children_by_text(document: &Document, parent: ItemId) -> HashMap<&str, Vec<ItemId>> {
    let mut children: HashMap<&str, Vec<ItemId>> = HashMap::new();
    for block in blocks::each_child(document, parent) {
        if let Some(item) = document.get(block.id()) {
            children
                .entry(item.display_text())
                .or_default()
                .push(block.id());
        }
    }
    children
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
    document.get(id).map(Item::display_text).map(str::to_owned)
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

    /// Where `matches` puts each line of `from` in `to`, as `(line, whole path)` texts.
    fn matched(from: &str, to: &str) -> Vec<(String, Option<(String, bool)>)> {
        let (from, to) = (format::parse(from), format::parse(to));
        let found = matches(&from, &to);
        from.descendants(from.root())
            .map(|(id, _)| {
                let at = found
                    .get(&id)
                    .map(|&(at, whole)| (line(&to, at).to_owned(), whole));
                (line(&from, id).to_owned(), at)
            })
            .collect()
    }

    // file-io.md §3, N-PERF-02
    #[test]
    fn matches_puts_every_block_where_its_path_leads() {
        let found = |text: &str, whole| Some((text.to_owned(), whole));
        assert_eq!(
            matched(
                "A:\n\t- x\n\t- x\n\t\t- y\nH @hidden\n\tA:\nZ:\n",
                "A: @t\n\t- x\n\tnote\n\t- x\nA:\n"
            ),
            [
                ("A:".to_owned(), found("A: @t", true)),
                ("- x".to_owned(), found("- x", true)),
                ("- x".to_owned(), found("- x", true)),
                ("- y".to_owned(), found("- x", false)),
                ("H @hidden".to_owned(), None),
                ("A:".to_owned(), found("A: @t", true)),
                ("Z:".to_owned(), None),
            ]
        );
    }

    /// A line of an outline: its depth, at most one more than the line before's, and its text.
    fn lines() -> impl proptest::strategy::Strategy<Value = String> {
        use proptest::prelude::*;
        let text = prop::sample::select(vec!["A:", "- b", "- b @t", "c", "h @hidden"]);
        proptest::collection::vec((0..3_usize, text), 0..20).prop_map(|lines| {
            let mut depth = 0;
            let mut outline = String::new();
            for (deeper, text) in lines {
                depth = deeper.min(depth + 1);
                outline.push_str(&"\t".repeat(depth));
                outline.push_str(text);
                outline.push('\n');
            }
            outline
        })
    }

    proptest::proptest! {
        // file-io.md §3, N-PERF-02
        #[test]
        fn matches_finds_what_each_blocks_path_finds(from in lines(), to in lines()) {
            let (from, to) = (format::parse(&from), format::parse(&to));
            let found = matches(&from, &to);
            for (id, _) in from.descendants(from.root()) {
                let path = TextPath::of(&from, id).and_then(|path| path.find(&to));
                proptest::prop_assert_eq!(found.get(&id).copied(), path);
            }
        }
    }
}
