//! One line of text being edited with the standard text keys: a caret and a selection that move
//! by grapheme and by word, insertion and deletion (FR-MODE-03,
//! [editing.md §2](../../../docs/specs/editing.md#2-edit-mode)). Hides grapheme and word
//! segmentation.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

/// A change to a [`TextField`], as a text key asks for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEdit {
    /// Type a character in place of the selection, or at the caret.
    Insert(char),
    /// Move the caret; with `select`, extend the selection to it.
    Move {
        /// Where to.
        motion: Motion,
        /// Whether the selection extends to the new caret instead of being cleared.
        select: bool,
    },
    /// Delete the selection, or the grapheme before the caret.
    DeleteBackward,
    /// Delete the selection, or the grapheme after the caret.
    DeleteForward,
    /// Delete the selection, or back to the start of the word before the caret.
    DeleteWordBackward,
    /// Select the whole text.
    SelectAll,
}

/// Where a caret motion goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    /// One grapheme back.
    Left,
    /// One grapheme forward.
    Right,
    /// To the start of the word before the caret.
    WordLeft,
    /// To the end of the word after the caret.
    WordRight,
    /// To the start of the text.
    Start,
    /// To the end of the text.
    End,
}

/// A text with a caret and an optional selection, both at grapheme boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextField {
    text: String,
    /// The caret's byte offset.
    caret: usize,
    /// Where the selection started, as a byte offset; the caret is its other end.
    anchor: Option<usize>,
}

impl TextField {
    /// `text` with the caret at its start, or at its end with `at_end`.
    #[must_use]
    pub fn new(text: String, at_end: bool) -> Self {
        let caret = if at_end { text.len() } else { 0 };
        Self {
            text,
            caret,
            anchor: None,
        }
    }

    /// `text` with the caret at byte `caret`, at the end when that is past it, and nothing
    /// selected.
    #[must_use]
    pub fn with_caret(text: String, caret: usize) -> Self {
        let caret = caret.min(text.len());
        Self {
            text,
            caret,
            anchor: None,
        }
    }

    /// The text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The caret's byte offset into the text.
    #[must_use]
    pub fn caret(&self) -> usize {
        self.caret
    }

    /// The selected bytes, if any are selected.
    #[must_use]
    pub fn selection(&self) -> Option<Range<usize>> {
        let anchor = self.anchor.filter(|&anchor| anchor != self.caret)?;
        Some(anchor.min(self.caret)..anchor.max(self.caret))
    }

    /// The field after `edit`.
    #[must_use]
    pub fn edited(&self, edit: TextEdit) -> Self {
        match edit {
            TextEdit::Insert(c) => self.replaced(self.selection_or(self.caret), &String::from(c)),
            TextEdit::Move { motion, select } => self.moved(motion, select),
            TextEdit::DeleteBackward => self.replaced(self.selection_or(self.left()), ""),
            TextEdit::DeleteForward => self.replaced(self.selection_or(self.right()), ""),
            TextEdit::DeleteWordBackward => self.replaced(self.selection_or(self.word_left()), ""),
            TextEdit::SelectAll => Self {
                text: self.text.clone(),
                caret: self.text.len(),
                anchor: Some(0),
            },
        }
    }

    /// The selection, or the bytes between the caret and `to` when nothing is selected.
    fn selection_or(&self, to: usize) -> Range<usize> {
        self.selection()
            .unwrap_or(to.min(self.caret)..to.max(self.caret))
    }

    /// The field with `range` replaced by `with` and the caret after it.
    fn replaced(&self, range: Range<usize>, with: &str) -> Self {
        let mut text = self.text.clone();
        text.replace_range(range.clone(), with);
        Self {
            text,
            caret: range.start + with.len(),
            anchor: None,
        }
    }

    fn moved(&self, motion: Motion, select: bool) -> Self {
        let selection = self.selection().filter(|_| !select);
        let caret = match (motion, selection) {
            (Motion::Left, Some(selection)) => selection.start,
            (Motion::Right, Some(selection)) => selection.end,
            (Motion::Left, None) => self.left(),
            (Motion::Right, None) => self.right(),
            (Motion::WordLeft, _) => self.word_left(),
            (Motion::WordRight, _) => self.word_right(),
            (Motion::Start, _) => 0,
            (Motion::End, _) => self.text.len(),
        };
        Self {
            text: self.text.clone(),
            caret,
            anchor: select.then(|| self.anchor.unwrap_or(self.caret)),
        }
    }

    /// The start of the grapheme before the caret, or the caret at the start.
    fn left(&self) -> usize {
        self.text
            .grapheme_indices(true)
            .map(|(at, _)| at)
            .take_while(|&at| at < self.caret)
            .last()
            .unwrap_or(self.caret)
    }

    /// The end of the grapheme after the caret, or the caret at the end.
    fn right(&self) -> usize {
        self.text
            .grapheme_indices(true)
            .find(|&(at, _)| at >= self.caret)
            .map_or(self.caret, |(at, grapheme)| at + grapheme.len())
    }

    /// The start of the word before the caret, or the start of the text.
    fn word_left(&self) -> usize {
        self.text
            .unicode_word_indices()
            .map(|(at, _)| at)
            .take_while(|&at| at < self.caret)
            .last()
            .unwrap_or(0)
    }

    /// The end of the word after the caret, or the end of the text.
    fn word_right(&self) -> usize {
        self.text
            .unicode_word_indices()
            .map(|(at, word)| at + word.len())
            .find(|&end| end > self.caret)
            .unwrap_or(self.text.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A field written with `|` for the caret and `[` for the selection's anchor.
    fn field(marked: &str) -> TextField {
        let caret = marked.find('|').unwrap();
        let anchor = marked.find('[');
        let text: String = marked.chars().filter(|&c| c != '|' && c != '[').collect();
        let shift =
            |at: usize, other: Option<usize>| at - usize::from(other.is_some_and(|o| o < at));
        TextField {
            caret: shift(caret, anchor),
            anchor: anchor.map(|anchor| shift(anchor, Some(caret))),
            text,
        }
    }

    /// The field written as in [`field`].
    fn marked(field: &TextField) -> String {
        let mut marks = vec![(field.caret, '|')];
        marks.extend(field.anchor.map(|anchor| (anchor, '[')));
        marks.sort_by_key(|&(at, mark)| (std::cmp::Reverse(at), mark == '['));
        let mut text = field.text.clone();
        for (at, mark) in marks {
            text.insert(at, mark);
        }
        text
    }

    #[track_caller]
    fn check(before: &str, edit: TextEdit, after: &str) {
        assert_eq!(
            marked(&field(before).edited(edit)),
            after,
            "{edit:?} on {before:?}"
        );
    }

    fn go(motion: Motion) -> TextEdit {
        TextEdit::Move {
            motion,
            select: false,
        }
    }

    fn select(motion: Motion) -> TextEdit {
        TextEdit::Move {
            motion,
            select: true,
        }
    }

    #[test]
    fn starts_with_the_caret_at_either_end() {
        assert_eq!(marked(&TextField::new("ab".into(), true)), "ab|");
        assert_eq!(marked(&TextField::new("ab".into(), false)), "|ab");
        assert_eq!(marked(&TextField::with_caret("ab".into(), 1)), "a|b");
        assert_eq!(marked(&TextField::with_caret("ab".into(), 9)), "ab|");
        assert_eq!(TextField::new("ab".into(), true).text(), "ab");
    }

    // FR-MODE-03
    #[test]
    fn inserts_at_the_caret_or_over_the_selection() {
        check("a|b", TextEdit::Insert('x'), "ax|b");
        check("|", TextEdit::Insert('é'), "é|");
        check("a[bc|d", TextEdit::Insert('x'), "ax|d");
        check("a|bc[d", TextEdit::Insert('x'), "ax|d");
    }

    // FR-MODE-03
    #[test]
    fn moves_by_grapheme() {
        let family = "👨‍👩‍👧";
        check(
            &format!("a{family}|b"),
            go(Motion::Left),
            &format!("a|{family}b"),
        );
        check(
            &format!("a|{family}b"),
            go(Motion::Right),
            &format!("a{family}|b"),
        );
        check("e\u{301}|", go(Motion::Left), "|e\u{301}");
        check("|ab", go(Motion::Left), "|ab");
        check("ab|", go(Motion::Right), "ab|");
    }

    // FR-MODE-03
    #[test]
    fn moves_by_word() {
        check("one two| three", go(Motion::WordLeft), "one |two three");
        check("one two |three", go(Motion::WordLeft), "one |two three");
        check("one t|wo three", go(Motion::WordLeft), "one |two three");
        check("  |one", go(Motion::WordLeft), "|  one");
        check("one| two, three", go(Motion::WordRight), "one two|, three");
        check("one tw|o three", go(Motion::WordRight), "one two| three");
        check("one two|, ", go(Motion::WordRight), "one two, |");
    }

    // FR-MODE-03
    #[test]
    fn moves_to_either_end() {
        check("ab|c", go(Motion::Start), "|abc");
        check("a|bc", go(Motion::End), "abc|");
    }

    // FR-MODE-03
    #[test]
    fn shift_motions_extend_the_selection() {
        check("ab|c", select(Motion::Left), "a|b[c");
        check("a|b[c", select(Motion::Left), "|ab[c");
        check("a|b[c", select(Motion::End), "ab[c|");
        check("one two|", select(Motion::WordLeft), "one |two[");
        check("ab|", TextEdit::SelectAll, "[ab|");
        assert_eq!(field("a[bc|d").selection(), Some(1..3));
        assert_eq!(field("a|bc[d").selection(), Some(1..3));
        assert_eq!(field("ab|").selection(), None);
        assert_eq!(field("a[|b").selection(), None, "an empty selection");
    }

    // FR-MODE-03
    #[test]
    fn plain_motions_collapse_the_selection() {
        check("a[bc|d", go(Motion::Left), "a|bcd");
        check("a|bc[d", go(Motion::Right), "abc|d");
        check("a[bc|d", go(Motion::End), "abcd|");
        check("a[bc|d", go(Motion::WordLeft), "|abcd");
    }

    // FR-MODE-03
    #[test]
    fn deletes_a_grapheme_or_the_selection() {
        check("ae\u{301}|b", TextEdit::DeleteBackward, "a|b");
        check("a|e\u{301}b", TextEdit::DeleteForward, "a|b");
        check("|ab", TextEdit::DeleteBackward, "|ab");
        check("ab|", TextEdit::DeleteForward, "ab|");
        check("a[bc|d", TextEdit::DeleteBackward, "a|d");
        check("a|bc[d", TextEdit::DeleteForward, "a|d");
    }

    // FR-MODE-03
    #[test]
    fn deletes_back_to_the_start_of_a_word() {
        check("one two|", TextEdit::DeleteWordBackward, "one |");
        check("one two  |x", TextEdit::DeleteWordBackward, "one |x");
        check("|one", TextEdit::DeleteWordBackward, "|one");
        check("on[e tw|o", TextEdit::DeleteWordBackward, "on|o");
    }
}
