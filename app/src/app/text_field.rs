//! A line of text being edited with the standard text keys: a caret and a selection that move by
//! grapheme and by word, insertion and deletion (FR-MODE-03,
//! [editing.md §2](../../../docs/specs/editing.md#2-edit-mode)). Hides grapheme and word
//! segmentation.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

use crate::document::{
    format::breaks_line,
    inline::{self, Marker},
};

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
    /// Delete the selection, or on to the end of the word after the caret.
    DeleteWordForward,
    /// Delete the selection, or back to the start of the text.
    DeleteToStart,
    /// Select the whole text.
    SelectAll,
    /// Put the marker around the selection, or the word at the caret, or take it off when it is
    /// there already (FR-FMT-02).
    Toggle(Marker),
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
    /// To this byte offset of the text, or the start of the grapheme it falls in, or the end
    /// when it is past it, as a click places the caret (FR-KEY-05).
    To(usize),
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
    /// `text` with the caret at its end.
    #[must_use]
    pub fn new(text: String) -> Self {
        Self {
            caret: text.len(),
            text,
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

    /// `text` with the caret `column` graphemes from its start, at the end when it is shorter,
    /// and nothing selected.
    #[must_use]
    pub fn at_column(text: String, column: usize) -> Self {
        let caret = text
            .grapheme_indices(true)
            .nth(column)
            .map_or(text.len(), |(at, _)| at);
        Self::with_caret(text, caret)
    }

    /// How many graphemes stand before the caret.
    #[must_use]
    pub fn column(&self) -> usize {
        self.text
            .get(..self.caret)
            .map_or(0, |before| before.graphemes(true).count())
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
            TextEdit::DeleteWordForward => self.replaced(self.selection_or(self.word_right()), ""),
            TextEdit::DeleteToStart => self.replaced(self.selection_or(0), ""),
            TextEdit::SelectAll => Self {
                text: self.text.clone(),
                caret: self.text.len(),
                anchor: Some(0),
            },
            TextEdit::Toggle(marker) => self.toggled(marker),
        }
    }

    /// The field with `text` typed in place of the selection, or at the caret, leaving out the
    /// characters that break lines, which its one line cannot hold; unchanged when that leaves
    /// nothing to type.
    #[must_use]
    pub fn pasted(&self, text: &str) -> Self {
        let typed: String = text.chars().filter(|&c| !breaks_line(c)).collect();
        if typed.is_empty() {
            return self.clone();
        }
        self.replaced(self.selection_or(self.caret), &typed)
    }

    /// The field with `marker` toggled on the selection, which stays on the same text, or on
    /// the word at the caret, or else at the caret, which stays in the same place in the text.
    fn toggled(&self, marker: Marker) -> Self {
        let selection = self.selection();
        let target = selection.clone().unwrap_or_else(|| self.word_at_caret());
        let (text, range) = inline::toggle(&self.text, target.clone(), marker);
        let anchor = selection.map(|selection| {
            if self.caret == selection.end {
                range.start
            } else {
                range.end
            }
        });
        let caret = match anchor {
            Some(anchor) if anchor == range.start => range.end,
            Some(_) => range.start,
            None => range.start + (self.caret - target.start),
        };
        Self {
            text,
            caret,
            anchor,
        }
    }

    /// The word the caret is in or touches, or the empty range at the caret.
    fn word_at_caret(&self) -> Range<usize> {
        self.text
            .unicode_word_indices()
            .map(|(at, word)| at..at + word.len())
            .find(|word| word.start <= self.caret && self.caret <= word.end)
            .unwrap_or(self.caret..self.caret)
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
            (Motion::To(at), _) => self.boundary(at),
        };
        Self {
            text: self.text.clone(),
            caret,
            anchor: select.then(|| self.anchor.unwrap_or(self.caret)),
        }
    }

    /// The start of the grapheme byte `at` falls in, or the end of the text when it is past it.
    fn boundary(&self, at: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .map(|(start, _)| start)
            .chain([self.text.len()])
            .take_while(|&start| start <= at)
            .last()
            .unwrap_or_default()
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

/// `text` as one line for a field that cannot break it, which takes a pasted text's lines as
/// words: without the line breaks that end it, and each other line break as a space.
#[must_use]
pub fn one_line(text: &str) -> String {
    text.trim_end_matches(breaks_line)
        .replace("\r\n", " ")
        .replace(breaks_line, " ")
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
    fn starts_with_the_caret_at_the_end_or_where_asked() {
        assert_eq!(marked(&TextField::new("ab".into())), "ab|");
        assert_eq!(marked(&TextField::with_caret("ab".into(), 1)), "a|b");
        assert_eq!(marked(&TextField::with_caret("ab".into(), 9)), "ab|");
        assert_eq!(TextField::new("ab".into()).text(), "ab");
    }

    // FR-MODE-03
    #[test]
    fn inserts_at_the_caret_or_over_the_selection() {
        check("a|b", TextEdit::Insert('x'), "ax|b");
        check("|", TextEdit::Insert('é'), "é|");
        check("a[bc|d", TextEdit::Insert('x'), "ax|d");
        check("a|bc[d", TextEdit::Insert('x'), "ax|d");
    }

    // keybindings.md §4: a prompt takes pasted text as typed, on its one line.
    #[test]
    fn pastes_at_the_caret_or_over_the_selection_without_line_breaks() {
        let pasted = |before: &str, text: &str| marked(&field(before).pasted(text));
        assert_eq!(pasted("a|b", "xy"), "axy|b");
        assert_eq!(pasted("a[bc|d", "x\r\ny\u{2029}z\n"), "axyz|d");
        assert_eq!(pasted("|", "日本"), "日本|");
        assert_eq!(
            pasted("a[bc|d", "\n"),
            "a[bc|d",
            "nothing typed, so nothing replaced"
        );
        assert_eq!(pasted("a[bc|d", ""), "a[bc|d");
    }

    // editing.md §6, §7, attachments.md §5
    #[test]
    fn one_line_joins_the_lines_with_spaces_and_drops_the_breaks_that_end_it() {
        assert_eq!(one_line("a\r\nb\u{2029}c\rd\n\r\n"), "a b c d");
        assert_eq!(one_line("\nab"), " ab");
        assert_eq!(one_line("ab"), "ab");
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

    // FR-KEY-05
    #[test]
    fn moves_to_an_offset_at_the_start_of_its_grapheme() {
        let family = "👨‍👩‍👧";
        check("ab|c", go(Motion::To(0)), "|abc");
        check("|abc", go(Motion::To(2)), "ab|c");
        check("|abc", go(Motion::To(3)), "abc|");
        check("|abc", go(Motion::To(9)), "abc|");
        check(
            &format!("|a{family}b"),
            go(Motion::To(3)),
            &format!("a|{family}b"),
        );
        check("a[b|c", go(Motion::To(3)), "abc|");
        check("a|bc", select(Motion::To(3)), "a[bc|");
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

    // FR-MODE-03, D-144
    #[test]
    fn deletes_on_to_the_end_of_a_word() {
        check("one |two three", TextEdit::DeleteWordForward, "one | three");
        check("x|  one two", TextEdit::DeleteWordForward, "x| two");
        check("one|", TextEdit::DeleteWordForward, "one|");
        check("on[e tw|o", TextEdit::DeleteWordForward, "on|o");
    }

    // FR-MODE-03, D-144
    #[test]
    fn deletes_back_to_the_start_of_the_text() {
        check("one tw|o", TextEdit::DeleteToStart, "|o");
        check("|one", TextEdit::DeleteToStart, "|one");
        check("on[e tw|o", TextEdit::DeleteToStart, "on|o");
    }

    // FR-FMT-02
    #[test]
    fn toggling_a_marker_acts_on_the_selection_keeping_it() {
        let bold = TextEdit::Toggle(Marker::Bold);
        check("a [bc| d", bold, "a **[bc|** d");
        check("a |bc[ d", bold, "a **|bc[** d");
        check("a **[bc|** d", bold, "a [bc| d");
        check("[**bc**|", bold, "[bc|");
    }

    // FR-FMT-02
    #[test]
    fn toggling_a_marker_without_a_selection_acts_on_the_word_at_the_caret() {
        let code = TextEdit::Toggle(Marker::Code);
        check("one tw|o three", code, "one `tw|o` three");
        check("one two| three", code, "one `two|` three");
        check("one |two", code, "one `|two`");
        check("one `tw|o`", code, "one tw|o");
        check("one |", code, "one `|`");
        check("one `|`", code, "one |");
        check("|", TextEdit::Toggle(Marker::Bold), "**|**");
        check("**a b|**", TextEdit::Toggle(Marker::Bold), "a b|");
    }

    #[test]
    fn counts_and_places_the_caret_by_grapheme_column() {
        let family = "👨‍👩‍👧";
        assert_eq!(field(&format!("a{family}|b")).column(), 2);
        assert_eq!(field("|ab").column(), 0);
        assert_eq!(
            marked(&TextField::at_column(format!("a{family}b"), 2)),
            format!("a{family}|b")
        );
        assert_eq!(marked(&TextField::at_column("ab".into(), 0)), "|ab");
        assert_eq!(marked(&TextField::at_column("ab".into(), 5)), "ab|");
    }
}
