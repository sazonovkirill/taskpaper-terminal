//! The clipboard: copying and cutting the selected blocks, else the focused block, as TaskPaper
//! text or the selected text being edited, and pasting text as blocks or into the text being
//! edited ([editing.md §2, §4](../../../docs/specs/editing.md#4-structural-operations-fr-ops),
//! FR-OPS-06).

use super::{Access, Effect, Notice, State, TextEdit};
use crate::document::{
    Block,
    format::{self, breaks_line},
    structure::{self, Place},
};

/// Where pasted blocks go, relative to the focused block (editing.md §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paste {
    /// After its subtree, as its next siblings.
    Below,
    /// Before it, as its previous siblings.
    Above,
}

impl State {
    /// `Ctrl+C`: copies the selected text in edit mode, else the selected blocks, else the
    /// focused block, with their subtrees as TaskPaper text; nothing when nothing is selected in
    /// edit mode or the focus is on the zoom title, which stands for the view's root.
    pub(super) fn copy(&self) -> Vec<Effect> {
        let text = if let Some(editor) = self.editor() {
            editor
                .selection
                .and_then(|range| editor.text.get(range))
                .map(str::to_owned)
        } else {
            let tops: Vec<_> = self.roots().iter().map(Block::id).collect();
            (!tops.is_empty()).then(|| format::serialize_blocks(self.document(), &tops))
        };
        text.map(Effect::CopyToClipboard).into_iter().collect()
    }

    /// `Ctrl+X`: copies as [`State::copy`] does, then deletes what it copied. A read-only file
    /// refuses with a notice.
    pub(super) fn cut(&mut self) -> Vec<Effect> {
        if self.access() == Access::ReadOnly {
            self.notify(Notice::ReadOnly);
            return Vec::new();
        }
        let effects = self.copy();
        if !effects.is_empty() {
            if self.edit.is_some() {
                self.edit_text(TextEdit::DeleteBackward);
            } else {
                self.delete_selected();
            }
        }
        effects
    }

    /// Pastes `text`. The open date prompt, Quick Add panel, Save As prompt, settings window,
    /// Edit Node dialog or note editor takes it as typed; in edit mode, a single line goes into the text being edited, and several lines end editing and become
    /// blocks below the block, which they replace when it is empty and not the zoom title; in
    /// navigate mode, the text becomes blocks `at` the focused block (FR-OPS-06).
    pub(super) fn pasted(&mut self, text: &str, at: Paste) {
        if let Some(prompt) = &mut self.date_prompt {
            prompt.paste(text);
            return;
        }
        if self.quick_add_paste(text)
            || self.save_as_paste(text)
            || self.settings_paste(text)
            || self.node_editor_paste(text)
            || self.attachments_paste(text)
        {
            return;
        }
        let line = text.trim_end_matches(breaks_line);
        if self.editor().is_none() {
            self.paste_blocks(text, at, None);
            return;
        }
        if !line.contains(breaks_line) {
            self.paste_text(line);
            return;
        }
        self.stop_editing();
        // The zoom title stands for the view's root, so it is pasted under, never replaced.
        let empty = self.focused_block().filter(|block| {
            self.document().get(block.id()).is_some_and(|item| {
                item.display_text().is_empty()
                    && item.tags().is_empty()
                    && item.children().is_empty()
            })
        });
        self.paste_blocks(text, at, empty);
    }

    /// Reads `text` as TaskPaper text and puts its blocks `at` the focused block at its depth,
    /// as the first blocks under the zoom title or of an empty outline, taking out `replaced`,
    /// as one step to undo that focuses the last of them. A read-only file refuses with a
    /// notice.
    fn paste_blocks(&mut self, text: &str, at: Paste, replaced: Option<Block>) {
        if self.access() == Access::ReadOnly {
            self.notify(Notice::ReadOnly);
            return;
        }
        let pasted = format::parse(text);
        let focused = self.focused();
        let place = match (&focused, at) {
            (None, _) => Place::Start,
            (Some((block, true)), _) => Place::FirstChild(block),
            (Some((block, false)), Paste::Below) => Place::Below(block),
            (Some((block, false)), Paste::Above) => Place::Above(block),
        };
        let (tops, mut ops) = structure::paste(&mut self.document, place, &pasted)
            .expect("the focused block was just found in the document");
        let Some(&last) = tops.last() else {
            return;
        };
        ops.extend(replaced.and_then(|block| structure::delete(self.document(), &block)));
        self.commit_focusing(ops, Some(last));
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::{
        app::{Message, Mode, Motion, SelectionChange},
        document::format,
    };

    /// Each line as a tab per depth and its text.
    fn lines(state: &State) -> Vec<String> {
        let document = state.document();
        document
            .descendants(document.root())
            .map(|(id, depth)| {
                format!("{}{}", "\t".repeat(depth), document.get(id).unwrap().text())
            })
            .collect()
    }

    /// `text` with focus moved down `index` rows.
    fn open(text: &str, index: usize) -> State {
        let mut state = State::new(format::parse(text), None);
        for _ in 0..index {
            state.update(Message::FocusNext);
        }
        state
    }

    /// The text of the focused row.
    fn focused(state: &State) -> String {
        let rows = state.rows();
        let row = rows.iter().find(|row| Some(row.id) == state.focus());
        row.unwrap().text.to_string()
    }

    /// The text being edited with `|` at the caret.
    fn shown(state: &State) -> String {
        let editor = state.editor().unwrap();
        let mut text = editor.text.to_owned();
        text.insert(editor.caret, '|');
        text
    }

    fn copied(text: &str) -> Vec<Effect> {
        vec![Effect::CopyToClipboard(text.to_owned())]
    }

    fn paste(state: &mut State, text: &str, at: Paste) -> Vec<Effect> {
        state.update(Message::Pasted {
            text: text.to_owned(),
            at,
        })
    }

    fn select_left(state: &mut State, graphemes: usize) {
        for _ in 0..graphemes {
            state.update(Message::Text(TextEdit::Move {
                motion: Motion::Left,
                select: true,
            }));
        }
    }

    // FR-OPS-06
    #[test]
    fn copy_puts_the_focused_block_with_its_subtree_on_the_clipboard_as_taskpaper() {
        let mut state = open("P:\n\t- a @due(1)\n\t\tn\n\t- b", 1);
        assert_eq!(state.update(Message::Copy), copied("- a @due(1)\n\tn\n"));
        assert_eq!(state.document().revision(), 0, "nothing changes");
        assert_eq!(state.mode(), Mode::Navigate);
    }

    // FR-OPS-06, FR-OPS-07
    #[test]
    fn copy_takes_the_selected_blocks_and_nothing_on_the_zoom_title() {
        let mut state = open("P:\n\t- a\n\t\tn\n\t- b\nQ:", 1);
        state.update(Message::Select(SelectionChange::ExtendDown));
        state.update(Message::Select(SelectionChange::ExtendDown));
        assert_eq!(state.update(Message::Copy), copied("- a\n\tn\n- b\n"));
        let mut zoomed = open("P:\n\t- a", 0);
        zoomed.update(Message::ZoomIn);
        zoomed.update(Message::FocusFirst);
        assert_eq!(zoomed.update(Message::Copy), []);
    }

    // FR-OPS-06, FR-OPS-08
    #[test]
    fn cut_copies_and_deletes_in_one_step() {
        let mut state = open("- a\n\t- b\n- c", 0);
        assert_eq!(state.update(Message::Cut), copied("- a\n\t- b\n"));
        assert_eq!(lines(&state), ["- c"]);
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["- a", "\t- b", "- c"]);
        assert_eq!(State::new(format::parse(""), None).update(Message::Cut), []);
    }

    // FR-OPS-06, file-io.md §1
    #[test]
    fn a_read_only_file_refuses_cut_and_paste_but_copies() {
        let read_only = || {
            State::new(format::parse("- a"), Some(PathBuf::from("/a.taskpaper")))
                .with_access(Access::ReadOnly)
        };
        let mut state = read_only();
        assert_eq!(state.update(Message::Cut), []);
        assert_eq!(state.notice(), Some(&Notice::ReadOnly));
        let mut state = read_only();
        paste(&mut state, "- b", Paste::Below);
        assert_eq!(state.notice(), Some(&Notice::ReadOnly));
        assert_eq!(lines(&state), ["- a"]);
        assert_eq!(read_only().update(Message::Copy), copied("- a\n"));
    }

    // FR-OPS-06
    #[test]
    fn paste_asks_the_frontend_for_the_clipboard_without_ending_editing() {
        let mut state = open("- a", 0);
        assert_eq!(
            state.update(Message::Paste(Paste::Above)),
            [Effect::ReadClipboard(Paste::Above)]
        );
        state.update(Message::EditAtEnd);
        assert_eq!(
            state.update(Message::Paste(Paste::Below)),
            [Effect::ReadClipboard(Paste::Below)]
        );
        assert_eq!(state.mode(), Mode::Edit);
    }

    // FR-OPS-06, FR-OPS-08, editing.md §4
    #[test]
    fn pasted_text_becomes_blocks_below_or_above_the_focused_block_in_one_step() {
        let clip = "- x\n\t- y\nz\n";
        let mut state = open("P:\n\t- a\n\t\t- b\n\t- c", 1);
        paste(&mut state, clip, Paste::Below);
        assert_eq!(
            lines(&state),
            ["P:", "\t- a", "\t\t- b", "\t- x", "\t\t- y", "\tz", "\t- c"]
        );
        assert_eq!(focused(&state), "z", "the last block pasted");
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["P:", "\t- a", "\t\t- b", "\t- c"]);
        assert_eq!(focused(&state), "a");
        paste(&mut state, clip, Paste::Above);
        assert_eq!(
            lines(&state),
            ["P:", "\t- x", "\t\t- y", "\tz", "\t- a", "\t\t- b", "\t- c"]
        );
        paste(&mut state, "", Paste::Below);
        assert_eq!(state.document().revision(), 3, "nothing to paste");
    }

    // FR-OPS-06, FR-NAV-03
    #[test]
    fn pasted_blocks_go_first_under_the_zoom_title_and_into_an_empty_outline() {
        let mut state = open("P:\n\t- a", 0);
        state.update(Message::ZoomIn);
        state.update(Message::FocusFirst);
        paste(&mut state, "- x", Paste::Below);
        assert_eq!(lines(&state), ["P:", "\t- x", "\t- a"]);
        let mut empty = State::new(format::parse(""), None);
        paste(&mut empty, "a\nb", Paste::Above);
        assert_eq!(lines(&empty), ["a", "b"]);
        assert_eq!(focused(&empty), "b");
    }

    // FR-MODE-03
    #[test]
    fn in_edit_mode_copy_and_cut_take_the_selected_text() {
        let mut state = open("- buy milk @due(1)", 0);
        state.update(Message::EditAtEnd);
        assert_eq!(state.update(Message::Copy), [], "nothing selected");
        select_left(&mut state, 4);
        assert_eq!(state.update(Message::Copy), copied("milk"));
        assert_eq!(state.update(Message::Cut), copied("milk"));
        assert_eq!(shown(&state), "buy |");
        assert_eq!(state.mode(), Mode::Edit);
        assert_eq!(state.update(Message::Cut), []);
        assert_eq!(shown(&state), "buy |", "nothing selected, nothing cut");
    }

    // FR-MODE-03, FR-OPS-06
    #[test]
    fn a_single_line_pasted_in_edit_mode_goes_in_at_the_caret_as_one_change() {
        let mut state = open("- buy milk", 0);
        state.update(Message::EditAtEnd);
        select_left(&mut state, 4);
        paste(&mut state, "oat milk\n", Paste::Below);
        assert_eq!(shown(&state), "buy oat milk|");
        assert_eq!(lines(&state), ["- buy oat milk"]);
        state.update(Message::Text(TextEdit::Insert('!')));
        state.update(Message::Undo);
        assert_eq!(
            shown(&state),
            "buy oat milk|",
            "typing after it is a change of its own"
        );
        state.update(Message::Undo);
        assert_eq!(shown(&state), "buy |milk", "with the selection it replaced");
        assert_eq!(state.editor().unwrap().selection, Some(4..8));
        assert_eq!(state.notice(), None);
    }

    // OQ-25, editing.md §2
    #[test]
    fn a_line_pasted_in_edit_mode_loses_each_at_sign_that_would_start_a_tag() {
        let mut state = open("note", 0);
        state.update(Message::EditAtEnd);
        paste(&mut state, " @due(1) mail me@x.y @", Paste::Below);
        assert_eq!(shown(&state), "note due(1) mail me@x.y |");
        assert_eq!(state.notice(), Some(&Notice::TypedTag));
        state.update(Message::Undo);
        assert_eq!(shown(&state), "note|", "one change");
    }

    // FR-OPS-06, editing.md §2
    #[test]
    fn several_lines_pasted_in_edit_mode_end_editing_and_become_blocks_below() {
        let mut state = open("- a\n- c", 0);
        state.update(Message::EditAtEnd);
        state.update(Message::Text(TextEdit::Insert('!')));
        paste(&mut state, "- x\n\t- y\n", Paste::Below);
        assert_eq!(state.mode(), Mode::Navigate);
        assert_eq!(lines(&state), ["- a!", "- x", "\t- y", "- c"]);
        assert_eq!(focused(&state), "x");
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["- a!", "- c"], "the paste");
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["- a", "- c"], "then the typing");
    }

    // FR-OPS-06, editing.md §2
    #[test]
    fn several_lines_pasted_into_an_empty_block_replace_it() {
        let mut state = open("- a\n- c", 0);
        state.update(Message::New(crate::app::NewBlock::Below, None));
        paste(&mut state, "- x\n- y", Paste::Below);
        assert_eq!(lines(&state), ["- a", "- x", "- y", "- c"]);
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["- a", "- ", "- c"]);
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["- a", "- c"]);
        for kept in ["- @due(1)\n- c", "-  \n\t- b"] {
            let mut state = open(kept, 0);
            state.update(Message::EditAtEnd);
            paste(&mut state, "x\ny", Paste::Below);
            assert_eq!(
                lines(&state).len(),
                lines(&open(kept, 0)).len() + 2,
                "{kept:?}"
            );
        }
    }

    // FR-OPS-06, FR-NAV-03
    #[test]
    fn several_lines_pasted_into_an_emptied_zoom_title_go_under_it() {
        let mut state = open("- a", 0);
        state.update(Message::ZoomIn);
        state.update(Message::EditAtEnd);
        state.update(Message::Text(TextEdit::DeleteBackward));
        paste(&mut state, "x\ny", Paste::Below);
        assert_eq!(lines(&state), ["- ", "\tx", "\ty"]);
        assert_eq!(state.notice(), None);
    }
}
