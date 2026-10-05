//! Adding a block next to the focused one to type into, deleting blocks and moving them, each one
//! step to undo ([editing.md §2–4](../../../docs/specs/editing.md#3-creating-blocks-fr-mode-04),
//! FR-MODE-04, FR-OPS-02, FR-OPS-03, FR-OPS-06).

use super::{Access, Notice, Restore, State};
use crate::document::{
    Block, BlockKind, ItemId, Transaction, blocks,
    format::Typed,
    structure::{self, Place, Shift},
};

/// Where a new block goes, relative to the focused block (editing.md §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NewBlock {
    /// After its subtree, as its next sibling.
    Below,
    /// As its previous sibling.
    Above,
    /// As its first child.
    FirstChild,
    /// As its last child.
    LastChild,
    /// As its first child when it shows children, else below it, as `Enter` ends editing.
    Next,
    /// At the top of its list, its parent's children (FR-MODE-06).
    Top,
}

impl State {
    /// Adds an empty block of type `typed`, or of the type that fits, where `new` says and edits
    /// it (FR-MODE-04, FR-MODE-06). On the zoom title, which stands for the view's root, the
    /// block becomes the first one under it, or the last one for [`NewBlock::LastChild`]; in an
    /// outline without rows it is the first block. A read-only file refuses with a notice.
    pub(super) fn add_block(&mut self, new: NewBlock, typed: Option<Typed>) {
        if self.access() == Access::ReadOnly {
            self.notify(Notice::ReadOnly);
            return;
        }
        let focused = self.focused();
        let list = match (&focused, new) {
            (Some((block, false)), NewBlock::Top) => self.siblings(block.id()),
            _ => Vec::new(),
        };
        let place = match &focused {
            None => Place::Start,
            Some((block, true)) if new == NewBlock::LastChild => Place::LastChild(block),
            Some((block, true)) => Place::FirstChild(block),
            Some((block, false)) => match new {
                NewBlock::Above => Place::Above(block),
                NewBlock::FirstChild => Place::FirstChild(block),
                NewBlock::LastChild => Place::LastChild(block),
                NewBlock::Next if self.is_expanded(block) => Place::FirstChild(block),
                NewBlock::Below | NewBlock::Next => Place::Below(block),
                NewBlock::Top => top_of(&list, typed).expect("the focused block is in its list"),
            },
        };
        let (id, ops) = structure::new_block(&mut self.document, place, typed)
            .expect("the focused block was just found in the document");
        if let (Place::FirstChild(block) | Place::LastChild(block), Some((_, false))) =
            (place, &focused)
        {
            self.folds.remove(&block.id());
        }
        let mut transaction = Transaction::new(ops, (), ());
        self.document
            .apply(&mut transaction)
            .expect("the ops are built from the document as it is, with a fresh id");
        let before = self.focus;
        self.focus = Some(id);
        self.edit_new_block(id, transaction.ops().to_vec(), before);
    }

    /// Deletes the focused block with its subtree and focuses the next visible block, else the
    /// previous one (FR-OPS-06). The zoom title, which stands for the view's root, stays.
    pub(super) fn delete_focused(&mut self) {
        let Some((block, false)) = self.focused() else {
            return;
        };
        let rows = self.rows();
        let mut from = rows.iter().skip_while(|row| row.id != block.id());
        let depth = from.next().map_or(0, |row| row.depth);
        let next = from.find(|row| row.depth <= depth);
        let previous = rows.iter().take_while(|row| row.id != block.id()).last();
        self.delete_block(&block, next.or(previous).map(|row| row.id));
    }

    /// `Backspace` in an empty block: deletes it and edits the previous visible block at its end
    /// (editing.md §2). A block with children, whose subtree a text key should not take, and the
    /// first row stay. A block made for this session goes without a trace in the history.
    pub(super) fn delete_empty_block(&mut self) {
        let Some((block, false)) = self.focused() else {
            return;
        };
        let leaf = self
            .document
            .get(block.id())
            .is_some_and(|item| item.children().is_empty());
        let rows = self.rows();
        let previous = rows.iter().take_while(|row| row.id != block.id()).last();
        let Some(previous) = previous.filter(|_| leaf).map(|row| row.id) else {
            return;
        };
        if !self.discard_new_block() {
            self.stop_editing();
            self.delete_block(&block, Some(previous));
        }
        self.focus = Some(previous);
        self.start_editing();
    }

    /// Deletes `block` with its subtree as one step to undo that ends with `focus`. A read-only
    /// file refuses with a notice.
    fn delete_block(&mut self, block: &Block, focus: Option<ItemId>) {
        if self.access() == Access::ReadOnly {
            self.notify(Notice::ReadOnly);
            return;
        }
        let ops = structure::delete(&self.document, block)
            .into_iter()
            .collect();
        let mut transaction = Transaction::new(ops, (), ());
        self.document
            .apply(&mut transaction)
            .expect("the op is built from the document as it is");
        let before = Restore {
            focus: Some(block.id()),
        };
        self.edits.record(Transaction::new(
            transaction.ops().to_vec(),
            before,
            Restore { focus },
        ));
        self.focus = focus;
    }

    /// Moves the focused block with its subtree as `by` says without leaving the zoom, as one
    /// step to undo, expanding the block it lands under (FR-OPS-02, FR-OPS-03). The zoom title,
    /// which stands for the view's root, stays.
    pub(super) fn shift_focused(&mut self, by: Shift) {
        let Some((block, false)) = self.focused() else {
            return;
        };
        let top = self
            .zoom_block()
            .map_or(self.document.root(), |zoom| zoom.id());
        let Some(ops) = structure::shift(&self.document, &block, by, top) else {
            return;
        };
        self.commit(ops);
        if let Some(parent) = blocks::parent(&self.document, block.id()) {
            self.folds.remove(&parent.id());
        }
    }
}

/// Where a block of type `typed` goes at the top of `list`: before its first block, except that
/// a section goes before its first section, after the tasks above it, or at its end when it has
/// none (editing.md §3, OQ-29). `None` for an empty list.
fn top_of(list: &[Block], typed: Option<Typed>) -> Option<Place<'_>> {
    if typed == Some(Typed::Section) {
        list.iter()
            .find(|block| block.kind() == BlockKind::Section)
            .map(Place::Above)
            .or_else(|| list.last().map(Place::Below))
    } else {
        list.first().map(Place::Above)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::{Path, PathBuf},
        time::Duration,
    };

    use super::*;
    use crate::{
        app::{Disk, Effect, Message, Mode, TextEdit, Version},
        document::{Document, Op, format},
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

    fn typed(state: &mut State, text: &str) {
        for c in text.chars() {
            state.update(Message::Text(TextEdit::Insert(c)));
        }
    }

    fn backspace(state: &mut State) {
        state.update(Message::Text(TextEdit::DeleteBackward));
    }

    /// The text of the focused row.
    fn focused(state: &State) -> String {
        let rows = state.rows();
        let row = rows
            .iter()
            .find(|row| Some(row.id) == state.focus())
            .unwrap();
        row.text.clone()
    }

    /// The text being edited with `|` at the caret.
    fn shown(state: &State) -> String {
        let editor = state.editor().unwrap();
        let mut text = editor.text.to_owned();
        text.insert(editor.caret, '|');
        text
    }

    // FR-MODE-04, OQ-03
    #[test]
    fn o_adds_a_block_below_to_type_into_and_esc_keeps_it() {
        let mut state = open("- a\n\t- b\n- c", 0);
        state.update(Message::NewBelow);
        assert_eq!(state.mode(), Mode::Edit);
        assert_eq!(shown(&state), "|");
        assert_eq!(lines(&state), ["- a", "\t- b", "- ", "- c"]);
        assert_eq!(state.editor().unwrap().id, state.focus().unwrap());
        typed(&mut state, "new");
        state.update(Message::StopEditing);
        assert_eq!(lines(&state), ["- a", "\t- b", "- new", "- c"]);
        assert_eq!(focused(&state), "new");
    }

    // FR-MODE-04, FR-OPS-08
    #[test]
    fn adding_a_block_and_typing_into_it_is_one_undo_step() {
        let mut state = open("- a\n- c", 1);
        state.update(Message::NewAbove);
        typed(&mut state, "b");
        state.update(Message::StopEditing);
        assert_eq!(lines(&state), ["- a", "- b", "- c"]);
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["- a", "- c"]);
        assert_eq!(focused(&state), "c", "back where the focus was");
        state.update(Message::Redo);
        assert_eq!(lines(&state), ["- a", "- b", "- c"]);
        assert_eq!(focused(&state), "b");
        state.update(Message::Undo);
        state.update(Message::NewAbove);
        state.update(Message::StopEditing);
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["- a", "- c"], "an empty new block too");
    }

    // FR-MODE-04
    #[test]
    fn i_and_shift_i_add_a_last_and_a_first_child_and_expand_the_block() {
        let mut state = open("P:\n\t- a\nQ:", 0);
        state.update(Message::Collapse);
        state.update(Message::NewFirstChild);
        assert_eq!(state.mode(), Mode::Edit);
        assert_eq!(lines(&state), ["P:", "\t- ", "\t- a", "Q:"]);
        assert_eq!(state.rows().len(), 4, "expanded");
        assert_eq!(state.rows()[1].id, state.focus().unwrap());
        state.update(Message::StopEditing);
        state.update(Message::Undo);
        state.update(Message::Collapse);
        state.update(Message::NewLastChild);
        assert_eq!(state.mode(), Mode::Edit);
        assert_eq!(lines(&state), ["P:", "\t- a", "\t- ", "Q:"]);
        assert_eq!(state.rows().len(), 4, "expanded");
        assert_eq!(state.rows()[2].id, state.focus().unwrap());
        typed(&mut state, "b");
        state.update(Message::StopEditing);
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["P:", "\t- a", "Q:"], "one step");
    }

    // editing.md §3
    #[test]
    fn enter_ends_editing_and_adds_the_next_block() {
        let mut state = open("P:\n\t- a\n- b", 2);
        state.update(Message::EditAtEnd);
        typed(&mut state, "c");
        state.update(Message::NewNext);
        assert_eq!(state.mode(), Mode::Edit);
        assert_eq!(lines(&state), ["P:", "\t- a", "- bc", "- "]);
        typed(&mut state, "d");
        state.update(Message::NewNext);
        typed(&mut state, "e");
        state.update(Message::StopEditing);
        assert_eq!(lines(&state), ["P:", "\t- a", "- bc", "- d", "- e"]);
        state.update(Message::Undo);
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["P:", "\t- a", "- bc"], "one step per block");
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["P:", "\t- a", "- b"]);
    }

    // editing.md §3
    #[test]
    fn enter_on_a_block_showing_children_adds_a_first_child() {
        let mut state = open("P:\n\t- a\nQ:\n\t- b", 0);
        state.update(Message::EditAtEnd);
        state.update(Message::NewNext);
        assert_eq!(lines(&state), ["P:", "\t- ", "\t- a", "Q:", "\t- b"]);
        state.update(Message::StopEditing);
        state.update(Message::FocusLast);
        state.update(Message::FocusParent);
        state.update(Message::Collapse);
        state.update(Message::EditAtEnd);
        state.update(Message::NewNext);
        assert_eq!(
            lines(&state),
            ["P:", "\t- ", "\t- a", "Q:", "\t- b", ""],
            "a collapsed block gets a block below its subtree"
        );
    }

    // FR-NAV-03, FR-MODE-04
    #[test]
    fn on_the_zoom_title_every_new_block_is_its_first_child_but_i_adds_its_last() {
        for new in [Message::NewBelow, Message::NewAbove, Message::NewFirstChild] {
            let mut state = open("P:\n\t- a\nQ:", 0);
            state.update(Message::ZoomIn);
            state.update(Message::FocusFirst);
            state.update(Message::Collapse);
            state.update(new.clone());
            assert_eq!(lines(&state), ["P:", "\t- ", "\t- a", "Q:"], "{new:?}");
            assert_eq!(state.rows()[1].id, state.focus().unwrap());
        }
        let mut state = open("P:\n\t- a\nQ:", 0);
        state.update(Message::ZoomIn);
        state.update(Message::FocusFirst);
        state.update(Message::NewLastChild);
        assert_eq!(lines(&state), ["P:", "\t- a", "\t- ", "Q:"]);
        assert_eq!(state.rows()[2].id, state.focus().unwrap());
    }

    // FR-MODE-04
    #[test]
    fn an_empty_outline_gets_its_first_block() {
        let mut state = State::new(Document::default(), None);
        state.update(Message::NewBelow);
        typed(&mut state, "first");
        state.update(Message::NewNext);
        assert_eq!(lines(&state), ["first", ""], "text after text");
    }

    // FR-MODE-06, OQ-29
    #[test]
    fn upper_case_adds_a_typed_block_at_the_top_of_the_list() {
        let text = "P:\n\t- a\n\tn\n\tS @section\n\t\t- b\n\tT @section\nQ:";
        let mut state = open(text, 2);
        state.update(Message::NewTop(Typed::Project));
        assert_eq!(state.mode(), Mode::Edit);
        assert_eq!(shown(&state), "|");
        typed(&mut state, "Sub");
        state.update(Message::StopEditing);
        assert_eq!(lines(&state)[..3], ["P:", "\tSub:", "\t- a"]);
        assert_eq!(focused(&state), "Sub");
        state.update(Message::Undo);
        assert_eq!(lines(&state), lines(&open(text, 0)), "one step");
        assert_eq!(focused(&state), "n");
        state.update(Message::NewTop(Typed::Task));
        assert_eq!(lines(&state)[..3], ["P:", "\t- ", "\t- a"]);
        state.update(Message::StopEditing);
        state.update(Message::FocusLast);
        state.update(Message::NewTop(Typed::Task));
        assert_eq!(
            lines(&state)[..2],
            ["- ", "P:"],
            "the top level is a list too"
        );
    }

    // FR-MODE-06, OQ-29
    #[test]
    fn upper_case_s_adds_a_section_before_the_first_section_or_at_the_end() {
        let mut state = open("P:\n\t- a\n\tS @section\n\t\t- b\n\t- c\n\tT @section", 5);
        state.update(Message::NewTop(Typed::Section));
        typed(&mut state, "New");
        state.update(Message::StopEditing);
        assert_eq!(
            lines(&state),
            [
                "P:",
                "\t- a",
                "\tNew @section",
                "\tS @section",
                "\t\t- b",
                "\t- c",
                "\tT @section"
            ]
        );
        let mut state = open("P:\n\t- a\n\t\t- b\n\t- c\n\t\tn", 1);
        state.update(Message::NewTop(Typed::Section));
        assert_eq!(
            lines(&state),
            ["P:", "\t- a", "\t\t- b", "\t- c", "\t\tn", "\t@section"],
            "after the last block's subtree"
        );
    }

    // FR-MODE-06
    #[test]
    fn lower_case_adds_a_typed_block_below() {
        for (typed, line) in [
            (Typed::Project, ":"),
            (Typed::Section, "@section"),
            (Typed::Task, "- "),
        ] {
            let mut state = open("n\n\t- a\nm", 0);
            state.update(Message::Collapse);
            state.update(Message::NewTypedBelow(typed));
            assert_eq!(lines(&state), ["n", "\t- a", line, "m"], "{typed:?}");
            assert_eq!(state.mode(), Mode::Edit);
        }
    }

    // FR-NAV-03, FR-MODE-06
    #[test]
    fn typed_blocks_on_the_zoom_title_and_in_an_empty_outline_come_first() {
        for (new, line) in [
            (Message::NewTop(Typed::Section), "@section"),
            (Message::NewTypedBelow(Typed::Project), ":"),
        ] {
            let mut state = open("P:\n\t- a\n\tS @section", 0);
            state.update(Message::ZoomIn);
            state.update(Message::FocusFirst);
            state.update(new.clone());
            assert_eq!(lines(&state)[..2], ["P:".to_owned(), format!("\t{line}")]);
            let mut state = State::new(Document::default(), None);
            state.update(new);
            assert_eq!(lines(&state), [line]);
        }
    }

    // file-io.md §1
    #[test]
    fn a_read_only_file_refuses_new_and_deleted_blocks() {
        for message in [
            Message::NewBelow,
            Message::NewTop(Typed::Task),
            Message::Delete,
        ] {
            let mut state = State::new(format::parse("- a"), Some(PathBuf::from("/a.taskpaper")))
                .with_access(Access::ReadOnly);
            state.update(message);
            assert_eq!(lines(&state), ["- a"]);
            assert_eq!(state.mode(), Mode::Navigate);
            assert_eq!(state.notice(), Some(&Notice::ReadOnly));
        }
    }

    // editing.md §2
    #[test]
    fn backspace_in_a_new_empty_block_takes_it_out_without_a_trace() {
        let mut state = open("- a @x\n- c", 0);
        state.update(Message::EditAtEnd);
        typed(&mut state, "b");
        state.update(Message::NewNext);
        typed(&mut state, "x");
        backspace(&mut state);
        assert_eq!(lines(&state), ["- ab @x", "- ", "- c"]);
        backspace(&mut state);
        assert_eq!(lines(&state), ["- ab @x", "- c"]);
        assert_eq!(shown(&state), "ab|", "editing the block above at its end");
        state.update(Message::StopEditing);
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["- a @x", "- c"], "the earlier session");
    }

    // editing.md §2, FR-OPS-08
    #[test]
    fn backspace_in_an_emptied_block_deletes_it_as_a_step_of_its_own() {
        let mut state = open("P:\n\t- a", 1);
        state.update(Message::EditAtEnd);
        backspace(&mut state);
        assert_eq!(lines(&state), ["P:", "\t- "]);
        backspace(&mut state);
        assert_eq!(lines(&state), ["P:"]);
        assert_eq!(shown(&state), "P|");
        state.update(Message::StopEditing);
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["P:", "\t- "]);
        assert_eq!(focused(&state), "");
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["P:", "\t- a"]);
    }

    // editing.md §2
    #[test]
    fn backspace_keeps_a_block_with_children_and_the_first_row() {
        for (text, index) in [("- a\n- \n\t- c", 1), ("- \n- a", 0)] {
            let mut state = open(text, index);
            state.update(Message::EditAtEnd);
            backspace(&mut state);
            assert_eq!(lines(&state), lines(&open(text, 0)), "{text:?}");
            assert_eq!(state.mode(), Mode::Edit);
        }
    }

    // FR-OPS-06
    #[test]
    fn delete_takes_the_block_with_its_subtree_and_focuses_the_next_block() {
        let mut state = open("- a\n\t- b\n\t\t- c\n\t- d\n- e", 1);
        state.update(Message::Delete);
        assert_eq!(lines(&state), ["- a", "\t- d", "- e"]);
        assert_eq!(focused(&state), "d");
        state.update(Message::Delete);
        assert_eq!(focused(&state), "e", "the next visible block, not a child");
        state.update(Message::Delete);
        assert_eq!(lines(&state), ["- a"]);
        assert_eq!(focused(&state), "a", "else the previous one");
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["- a", "- e"]);
        assert_eq!(focused(&state), "e", "undo focuses the block again");
        state.update(Message::Redo);
        assert_eq!(focused(&state), "a");
        state.update(Message::Delete);
        assert_eq!(lines(&state), Vec::<String>::new());
        assert_eq!(state.focus(), None);
        state.update(Message::Delete);
        assert_eq!(state.document().revision(), 6, "nothing left to delete");
    }

    // FR-OPS-06
    #[test]
    fn delete_skips_the_rows_of_the_subtree_and_a_collapsed_one() {
        let mut state = open("- a\n\t- b\n- c", 0);
        state.update(Message::Delete);
        assert_eq!(focused(&state), "c");
        let mut state = open("- a\n\t- b\n- c\n\t- d", 2);
        state.update(Message::Collapse);
        state.update(Message::Delete);
        assert_eq!(focused(&state), "b");
    }

    // FR-NAV-03
    #[test]
    fn the_zoom_title_is_not_deleted() {
        let mut state = open("P:\n\t- a", 0);
        state.update(Message::ZoomIn);
        state.update(Message::FocusFirst);
        state.update(Message::Delete);
        assert_eq!(lines(&state), ["P:", "\t- a"]);
        state.update(Message::FocusNext);
        state.update(Message::Delete);
        assert_eq!(lines(&state), ["P:"]);
        assert_eq!(focused(&state), "P", "the title is the previous row");
        state.update(Message::EditAtEnd);
        backspace(&mut state);
        backspace(&mut state);
        assert_eq!(lines(&state), [":"], "nor by backspace");
        assert_eq!(state.mode(), Mode::Edit);
    }

    #[test]
    fn a_new_block_that_no_longer_comes_out_cleanly_is_deleted_as_a_step() {
        let mut state = open("- a", 0);
        state.update(Message::NewBelow);
        let root = state.document.root();
        let id = state.document.new_id();
        let op = Op::Insert {
            parent: root,
            index: 0,
            subtree: crate::document::Snapshot::line(id, "- z".to_owned()),
        };
        state
            .document
            .apply(&mut Transaction::new(vec![op], (), ()))
            .unwrap();
        backspace(&mut state);
        assert_eq!(lines(&state), ["- z", "- a"]);
        assert_eq!(shown(&state), "a|");
        state.update(Message::StopEditing);
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["- z", "- a", "- "]);
    }

    /// A file of `text` opened at time 0.
    fn file(text: &str) -> (State, PathBuf) {
        let path = PathBuf::from("/notes/todo.taskpaper");
        let state = State::new(format::parse(text), Some(path.clone()))
            .with_version(Some(Version::of(text.as_bytes())));
        (state, path)
    }

    fn later(state: &mut State, seconds: u64) -> Vec<Effect> {
        state.update(Message::Tick {
            now: Duration::from_secs(seconds),
        })
    }

    /// A save of `text` over the file holding `on_disk`.
    fn save(path: &Path, text: &str, on_disk: &str) -> Vec<Effect> {
        vec![
            Effect::Disk(Disk::Backup { path: path.into() }),
            Effect::Disk(Disk::Save {
                path: path.into(),
                text: text.to_owned(),
                expected: Some(Version::of(on_disk.as_bytes())),
            }),
        ]
    }

    // FR-DOC-02
    #[test]
    fn a_new_block_is_saved_while_it_is_typed_into() {
        let (mut state, path) = file("A:\n\t- a\n");
        state.update(Message::FocusNext);
        state.update(Message::NewBelow);
        assert_eq!(
            later(&mut state, 1),
            save(&path, "A:\n\t- a\n\t- \n", "A:\n\t- a\n")
        );
    }

    // file-format §3.1
    #[test]
    fn saving_waits_while_a_new_empty_last_block_is_below_the_top_level() {
        let (mut state, path) = file("A:\n\tB:\n");
        state.update(Message::FocusNext);
        state.update(Message::NewBelow);
        assert_eq!(state.deadline(), None);
        assert_eq!(later(&mut state, 5), []);
        typed(&mut state, "x");
        assert_eq!(
            later(&mut state, 6),
            save(&path, "A:\n\tB:\n\tx\n", "A:\n\tB:\n")
        );
        state.update(Message::Saved);
        backspace(&mut state);
        assert_eq!(later(&mut state, 10), [], "empty again");
        assert_eq!(
            state.update(Message::StopEditing),
            save(&path, "A:\n\tB:\n\n", "A:\n\tB:\n\tx\n"),
            "lifted to the top level when saved"
        );
        assert_eq!(state.notice(), Some(&Notice::BlanksLifted));
        assert_eq!(lines(&state), ["A:", "\tB:", ""]);
        state.update(Message::Saved);
        state.update(Message::Undo);
        assert_eq!(
            lines(&state),
            ["A:", "\tB:", "\t"],
            "the lift is a step of its own"
        );
    }

    // FR-OPS-08, file-format §3.1
    #[test]
    fn undoing_the_lift_of_a_trailing_blank_lets_the_next_undo_reach_the_block() {
        let (mut state, _) = file("A:\n\tB:\n");
        state.update(Message::FocusNext);
        state.update(Message::NewBelow);
        state.update(Message::StopEditing);
        later(&mut state, 1);
        assert_eq!(lines(&state), ["A:", "\tB:", ""]);
        state.update(Message::Saved);
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["A:", "\tB:", "\t"]);
        assert_eq!(state.save_deadline(), None);
        assert_eq!(later(&mut state, 5), [], "no save lifts it again");
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["A:", "\tB:"]);
        state.update(Message::Redo);
        state.update(Message::Redo);
        assert_eq!(lines(&state), ["A:", "\tB:", ""], "both steps redone");
    }

    // file-format §3.1
    #[test]
    fn a_new_empty_last_block_at_the_top_level_saves_at_once() {
        let (mut state, path) = file("A:");
        state.update(Message::NewBelow);
        assert_eq!(later(&mut state, 1), save(&path, "A:\n\n", "A:"));
    }

    // file-io.md §2, file-format §3.1
    #[test]
    fn quitting_after_undoing_a_lift_lifts_again_and_saves() {
        let (mut state, path) = file("A:\n\tB:\n");
        state.update(Message::FocusNext);
        state.update(Message::NewBelow);
        state.update(Message::StopEditing);
        later(&mut state, 1);
        state.update(Message::Saved);
        state.update(Message::Undo);
        assert_eq!(
            state.update(Message::Quit),
            save(&path, "A:\n\tB:\n\n", "A:\n\tB:\n\n")
        );
    }

    // file-io.md §2, file-format §3.1
    #[test]
    fn quitting_lifts_a_new_empty_last_block_and_saves() {
        let (mut state, path) = file("A:\n\tB:\n");
        state.update(Message::FocusNext);
        state.update(Message::NewBelow);
        assert_eq!(
            state.update(Message::Quit),
            save(&path, "A:\n\tB:\n\n", "A:\n\tB:\n")
        );
    }

    // FR-OPS-08, file-format §3.1
    #[test]
    fn undo_survives_blanks_lifted_while_a_new_block_is_edited() {
        let (mut state, _) = file("Work:\n\t- a\n\n\t- b\n");
        state.update(Message::FocusLast);
        state.update(Message::Delete);
        state.update(Message::NewAbove);
        typed(&mut state, "c");
        later(&mut state, 2);
        assert_eq!(lines(&state), ["Work:", "\t- a", "\tc", ""]);
        state.update(Message::StopEditing);
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["Work:", "\t- a", "\t", ""]);
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["Work:", "\t- a", "\t", "\t"]);
        state.update(Message::Undo);
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["Work:", "\t- a", "\t", "\t- b"]);
        assert_eq!(state.notice(), Some(&Notice::BlanksLifted));
    }

    // FR-OPS-02, FR-OPS-08
    #[test]
    fn tab_indents_under_the_previous_sibling_and_expands_it_as_one_step() {
        let mut state = open("- a\n\t- x\n- b\n\t- c\n- d", 0);
        state.update(Message::Collapse);
        state.update(Message::FocusNext);
        state.update(Message::Indent);
        assert_eq!(lines(&state), ["- a", "\t- x", "\t- b", "\t\t- c", "- d"]);
        assert_eq!(focused(&state), "b");
        assert_eq!(state.rows().len(), 5, "a is expanded");
        state.update(Message::FocusFirst);
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["- a", "\t- x", "- b", "\t- c", "- d"]);
        assert_eq!(focused(&state), "b");
        state.update(Message::Redo);
        assert_eq!(lines(&state), ["- a", "\t- x", "\t- b", "\t\t- c", "- d"]);
        state.update(Message::Outdent);
        assert_eq!(lines(&state), ["- a", "\t- x", "- b", "\t- c", "- d"]);
        assert_eq!(focused(&state), "b");
    }

    // FR-OPS-02
    #[test]
    fn outdent_leaves_the_siblings_after_the_block_where_they_are() {
        let mut state = open("P:\n\t- a\n\t- b\n\t- c", 2);
        state.update(Message::Outdent);
        assert_eq!(lines(&state), ["P:", "\t- a", "\t- c", "- b"]);
        assert_eq!(focused(&state), "b");
        let revision = state.document().revision();
        state.update(Message::Outdent);
        state.update(Message::FocusFirst);
        state.update(Message::Indent);
        assert_eq!(state.document().revision(), revision, "nowhere to go");
    }

    // FR-OPS-03
    #[test]
    fn alt_j_and_alt_k_move_the_block_and_cross_into_the_parents_neighbors() {
        let mut state = open("P:\n\t- a\n\t- b\nQ:\n\t- c", 1);
        state.update(Message::MoveDown);
        assert_eq!(lines(&state), ["P:", "\t- b", "\t- a", "Q:", "\t- c"]);
        state.update(Message::MoveDown);
        assert_eq!(lines(&state), ["P:", "\t- b", "Q:", "\t- a", "\t- c"]);
        assert_eq!(focused(&state), "a");
        state.update(Message::MoveUp);
        state.update(Message::MoveUp);
        assert_eq!(lines(&state), ["P:", "\t- a", "\t- b", "Q:", "\t- c"]);
        state.update(Message::Undo);
        state.update(Message::Undo);
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["P:", "\t- b", "\t- a", "Q:", "\t- c"]);
        assert_eq!(focused(&state), "a", "one step per move");
    }

    // FR-OPS-03
    #[test]
    fn a_block_crossing_into_a_collapsed_block_expands_it() {
        let mut state = open("P:\n\t- a\nQ:\n\t- b", 2);
        state.update(Message::Collapse);
        state.update(Message::FocusPrevious);
        state.update(Message::MoveDown);
        assert_eq!(lines(&state), ["P:", "Q:", "\t- a", "\t- b"]);
        assert_eq!(state.rows().len(), 4);
        assert_eq!(focused(&state), "a");
    }

    // FR-NAV-03, FR-OPS-02, FR-OPS-03
    #[test]
    fn moves_stay_below_the_zoom_and_leave_its_title() {
        let mut state = open("P:\n\t- a\n\t\t- b\nQ:", 0);
        state.update(Message::ZoomIn);
        for message in [
            Message::Indent,
            Message::Outdent,
            Message::MoveUp,
            Message::MoveDown,
        ] {
            state.update(Message::FocusFirst);
            state.update(message.clone());
            state.update(Message::FocusNext);
            state.update(message.clone());
            assert_eq!(
                lines(&state),
                ["P:", "\t- a", "\t\t- b", "Q:"],
                "{message:?}"
            );
        }
        state.update(Message::FocusLast);
        state.update(Message::MoveDown);
        assert_eq!(
            lines(&state),
            ["P:", "\t- a", "\t\t- b", "Q:"],
            "no neighbor of its parent below the zoom"
        );
    }

    // FR-OPS-02, editing.md §2
    #[test]
    fn tab_and_shift_tab_keep_editing_as_one_session() {
        let mut state = open("- a\n- b", 1);
        state.update(Message::EditAtEnd);
        typed(&mut state, "c");
        state.update(Message::Indent);
        assert_eq!(state.mode(), Mode::Edit);
        typed(&mut state, "d");
        assert_eq!(lines(&state), ["- a", "\t- bcd"]);
        state.update(Message::Outdent);
        state.update(Message::Indent);
        state.update(Message::StopEditing);
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["- a", "\t- b"], "the typing");
        state.update(Message::Undo);
        state.update(Message::Undo);
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["- a", "- b"]);
    }

    // FR-MODE-04, FR-OPS-02
    #[test]
    fn a_new_block_indented_while_typed_into_undoes_in_steps() {
        let mut state = open("- a", 0);
        state.update(Message::NewBelow);
        state.update(Message::Indent);
        typed(&mut state, "b");
        state.update(Message::StopEditing);
        assert_eq!(lines(&state), ["- a", "\t- b"]);
        state.update(Message::Undo);
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["- a", "- "]);
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["- a"]);
    }

    // FR-OPS-03
    #[test]
    fn alt_j_in_edit_mode_ends_editing_first() {
        let mut state = open("- a\n- b", 0);
        state.update(Message::EditAtEnd);
        state.update(Message::MoveDown);
        assert_eq!(state.mode(), Mode::Navigate);
        assert_eq!(lines(&state), ["- b", "- a"]);
    }

    // file-io.md §1
    #[test]
    fn a_read_only_file_refuses_moves() {
        let mut state = State::new(
            format::parse("- a\n- b"),
            Some(PathBuf::from("/a.taskpaper")),
        )
        .with_access(Access::ReadOnly);
        state.update(Message::MoveDown);
        assert_eq!(lines(&state), ["- a", "- b"]);
        assert_eq!(state.notice(), Some(&Notice::ReadOnly));
    }

    #[test]
    fn an_empty_outline_moves_nothing() {
        let mut state = State::new(Document::default(), None);
        state.update(Message::Indent);
        assert_eq!(state.document().revision(), 0);
    }
}
