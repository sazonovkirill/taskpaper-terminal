//! Typed shortcuts: a Markdown-style prefix typed at the start of a text block or task and then a
//! space give the block another type, which the very next `Backspace` or `Ctrl+Z` takes back
//! ([editing.md §2.1](../../../../docs/specs/editing.md#21-typed-shortcuts), FR-MODE-05, D-24).

use jiff::civil::Date;

use super::{Session, TextField};
use crate::{
    app::{State, structure::NewBlock, text_field::TextEdit},
    document::{BlockKind, blocks, format::Typed},
};

/// What a typed prefix makes of the block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shortcut {
    /// `---`: a divider.
    Divider,
    /// `[]` or `[ ]`: a task.
    Task,
    /// `[x]` or `[X]`: a completed task.
    Done,
}

impl Shortcut {
    /// The shortcut `prefix` is, if any.
    fn of(prefix: &str) -> Option<Self> {
        match prefix {
            "---" => Some(Self::Divider),
            "[]" | "[ ]" => Some(Self::Task),
            "[x]" | "[X]" => Some(Self::Done),
            _ => None,
        }
    }

    /// Whether it converts a block of type `kind`: a text block takes every shortcut, a task
    /// only the divider. A text block whose text is `---` reads as a divider while it is typed.
    fn converts(self, kind: BlockKind) -> bool {
        match kind {
            BlockKind::Text | BlockKind::Divider => true,
            BlockKind::Task => self == Self::Divider,
            _ => false,
        }
    }
}

impl Session {
    /// Puts back the parts and text as typed when the latest change is a typed shortcut, and
    /// returns whether it did. The prefix and space come back as text, so they do not fire again.
    pub(super) fn revert_shortcut(&mut self) -> bool {
        let Some((parts, field)) = self.shortcut.take() else {
            return false;
        };
        self.parts = parts;
        self.field = field;
        self.typing = false;
        true
    }
}

impl State {
    /// A typed space: converts the block when the text before the caret is a shortcut that
    /// converts it, `today` dating a task it completes, and otherwise types the space
    /// (editing.md §2.1).
    pub(in crate::app) fn type_space(&mut self, today: Date) {
        let shortcut = self.typed_shortcut();
        let Some((session, (shortcut, kind))) = self.edit.as_mut().zip(shortcut) else {
            self.edit_text(TextEdit::Insert(' '));
            return;
        };
        if shortcut == Shortcut::Divider {
            session.retype(Typed::Divider);
            self.write(false);
            self.stop_editing();
            let typed = if kind == BlockKind::Task {
                Typed::Task
            } else {
                Typed::Text
            };
            self.add_block(NewBlock::Next, Some(typed));
            if let Some(session) = &mut self.edit {
                session.joins = true;
            }
            return;
        }
        let typed = (
            session.parts.clone(),
            session.field.edited(TextEdit::Insert(' ')),
        );
        let rest = &session.field.text()[session.field.caret()..];
        let (parts, text) = session.parts.retyped(Typed::Task, rest);
        let parts = if shortcut == Shortcut::Done {
            parts.tagged("done", &today.to_string())
        } else {
            parts
        };
        session.change(parts, TextField::with_caret(text, 0));
        session.shortcut = Some(typed);
        self.write(false);
    }

    /// The shortcut that the text before the caret is, with the type of the block it converts,
    /// when a space typed now fires it: nothing is selected, and a divider also needs no text
    /// after the caret and a block without children (FR-BLK-07) that is not the zoom title,
    /// whose next block would be its child.
    fn typed_shortcut(&self) -> Option<(Shortcut, BlockKind)> {
        let session = self.edit.as_ref()?;
        let field = &session.field;
        if field.selection().is_some() {
            return None;
        }
        let (prefix, rest) = field.text().split_at(field.caret());
        let kind = blocks::block(&self.document, session.id)?.kind();
        let shortcut = Shortcut::of(prefix).filter(|shortcut| shortcut.converts(kind))?;
        let divides = shortcut == Shortcut::Divider;
        let room_below = self
            .document
            .get(session.id)
            .is_some_and(|item| item.children().is_empty())
            && self.zoom_block().is_none_or(|zoom| zoom.id() != session.id);
        (!divides || rest.is_empty() && room_below).then_some((shortcut, kind))
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;
    use crate::{
        app::{Message, Mode, Motion},
        document::format,
    };

    const TODAY: Date = date(2026, 10, 5);

    /// Each line's text.
    fn texts(state: &State) -> Vec<String> {
        let document = state.document();
        document
            .descendants(document.root())
            .map(|(id, _)| document.get(id).unwrap().text().to_owned())
            .collect()
    }

    /// `text` with its first block edited, the caret at the end of its text.
    fn editing(text: &str) -> State {
        let mut state = State::new(format::parse(text), None);
        state.update(Message::EditAtEnd);
        state
    }

    /// A new empty text block edited below the text `a`.
    fn editing_new() -> State {
        let mut state = editing("a");
        state.update(Message::NewNext);
        state
    }

    /// Types `text`, its spaces as typed spaces.
    fn typed(state: &mut State, text: &str) {
        for c in text.chars() {
            state.update(if c == ' ' {
                Message::TypeSpace { today: TODAY }
            } else {
                Message::Text(TextEdit::Insert(c))
            });
        }
    }

    fn backspace(state: &mut State) {
        state.update(Message::Text(TextEdit::DeleteBackward));
    }

    /// The text being edited with `|` at the caret.
    fn shown(state: &State) -> String {
        let editor = state.editor().unwrap();
        let mut text = editor.text.to_owned();
        text.insert(editor.caret, '|');
        text
    }

    // FR-MODE-05, editing.md §2.1
    #[test]
    fn brackets_and_a_space_make_a_text_block_a_task() {
        for prefix in ["[]", "[ ]"] {
            let mut state = editing_new();
            typed(&mut state, prefix);
            typed(&mut state, " ");
            assert_eq!(texts(&state), ["a", "- "], "{prefix}");
            assert_eq!(shown(&state), "|");
            typed(&mut state, "Buy milk");
            assert_eq!(texts(&state), ["a", "- Buy milk"]);
        }
    }

    // FR-MODE-05, FR-DONE-01, editing.md §2.1
    #[test]
    fn a_crossed_box_makes_a_completed_task_of_the_text_after_it() {
        for prefix in ["[x]", "[X]"] {
            let mut state = editing("Buy milk @x");
            state.update(Message::Text(TextEdit::Move {
                motion: Motion::Start,
                select: false,
            }));
            typed(&mut state, prefix);
            typed(&mut state, " ");
            assert_eq!(texts(&state), ["- Buy milk @x @done(2026-10-05)"]);
            assert_eq!(shown(&state), "|Buy milk");
            assert!(state.rows()[0].done);
        }
    }

    // editing.md §2.1
    #[test]
    fn backspace_or_undo_right_after_takes_the_shortcut_back_as_text() {
        for undo in [Message::Text(TextEdit::DeleteBackward), Message::Undo] {
            let mut state = editing("milk @x");
            state.update(Message::Text(TextEdit::Move {
                motion: Motion::Start,
                select: false,
            }));
            typed(&mut state, "[x] ");
            state.update(undo.clone());
            assert_eq!(texts(&state), ["[x] milk @x"], "{undo:?}");
            assert_eq!(shown(&state), "[x] |milk");
            assert_eq!(state.rows()[0].kind, BlockKind::Text);
            backspace(&mut state);
            assert_eq!(shown(&state), "[x]|milk", "the next one deletes");
            typed(&mut state, " ");
            assert_eq!(texts(&state), ["- milk @x @done(2026-10-05)"], "fires anew");
        }
    }

    // editing.md §2.1
    #[test]
    fn only_the_very_next_key_takes_the_shortcut_back() {
        let mut state = editing_new();
        typed(&mut state, "[] b");
        backspace(&mut state);
        assert_eq!(texts(&state), ["a", "- "], "typing came between");
        let mut state = editing_new();
        typed(&mut state, "[] ");
        state.update(Message::Text(TextEdit::Move {
            motion: Motion::End,
            select: false,
        }));
        backspace(&mut state);
        assert_eq!(texts(&state), ["a"], "a motion came between");
        let mut state = editing_new();
        typed(&mut state, "[] ");
        state.update(Message::OpenCommandMenu {
            today: jiff::civil::date(2026, 10, 6),
        });
        state.update(Message::CommandMenu(crate::app::MenuInput::Erase));
        state.update(Message::Undo);
        assert_eq!(
            texts(&state),
            ["a", "[]"],
            "undo takes back the conversion as a change"
        );
        state.update(Message::Redo);
        state.update(Message::Indent);
        backspace(&mut state);
        assert_eq!(texts(&state), ["a"], "an indent came between");
    }

    // editing.md §2.1
    #[test]
    fn a_shortcut_fires_only_at_the_start_of_a_text_block_or_task() {
        for (line, prefix, expected) in [
            ("- a", "[] ", "- [] a"),
            ("- a", "[x] ", "- [x] a"),
            ("P:", "[] ", "[] P:"),
            ("P @section", "--- ", "--- P @section"),
            ("a", " [] ", " [] a"),
        ] {
            let mut state = editing(line);
            state.update(Message::Text(TextEdit::Move {
                motion: Motion::Start,
                select: false,
            }));
            typed(&mut state, prefix);
            assert_eq!(texts(&state), [expected], "{line:?} {prefix:?}");
        }
        let mut state = editing("[]");
        state.update(Message::Text(TextEdit::SelectAll));
        typed(&mut state, " ");
        assert_eq!(texts(&state), [" "], "a selection is replaced");
        let mut state = editing("a");
        typed(&mut state, " []");
        typed(&mut state, " ");
        assert_eq!(texts(&state), ["a [] "]);
    }

    // FR-MODE-05, editing.md §2.1
    #[test]
    fn dashes_and_a_space_make_a_divider_and_open_a_block_below() {
        let mut state = editing("- a\nb");
        state.update(Message::NewNext);
        typed(&mut state, "--- x");
        assert_eq!(texts(&state), ["- a", "---", "- x", "b"]);
        assert_eq!(state.rows()[1].kind, BlockKind::Divider);
        assert_eq!(state.mode(), Mode::Edit);
        assert_eq!(shown(&state), "x|", "a task after a task");
        let mut state = editing("b @x");
        state.update(Message::Text(TextEdit::SelectAll));
        typed(&mut state, "--- ");
        assert_eq!(texts(&state), ["--- @x", ""], "text after text");
        assert_eq!(state.rows()[1].kind, BlockKind::Text);
        backspace(&mut state);
        assert_eq!(
            texts(&state),
            ["--- @x"],
            "Backspace takes out only the new block"
        );
        assert_eq!(state.mode(), Mode::Navigate);
    }

    // FR-BLK-07, editing.md §2.1
    #[test]
    fn a_divider_needs_nothing_after_the_caret_and_no_children() {
        let mut state = editing("a\n\tchild");
        state.update(Message::Text(TextEdit::SelectAll));
        typed(&mut state, "--- ");
        assert_eq!(texts(&state), ["--- ", "child"]);
        let mut state = editing("a");
        state.update(Message::Text(TextEdit::Move {
            motion: Motion::Start,
            select: false,
        }));
        typed(&mut state, "--- ");
        assert_eq!(texts(&state), ["--- a"]);
        let mut state = State::new(format::parse("a"), None);
        state.update(Message::ZoomIn);
        state.update(Message::EditAtEnd);
        state.update(Message::Text(TextEdit::SelectAll));
        typed(&mut state, "--- x");
        state.update(Message::StopEditing);
        assert_eq!(
            texts(&state),
            ["--- x"],
            "the zoom title takes no block below"
        );
    }

    // FR-OPS-08, editing.md §2.1
    #[test]
    fn undo_takes_back_the_divider_with_the_block_below_in_one_step() {
        let mut state = editing("- a");
        state.update(Message::NewNext);
        typed(&mut state, "--- next");
        state.update(Message::StopEditing);
        assert_eq!(texts(&state), ["- a", "---", "- next"]);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["- a"]);
        assert_eq!(state.focus(), Some(state.rows()[0].id));
        state.update(Message::Redo);
        assert_eq!(texts(&state), ["- a", "---", "- next"]);
        let mut state = editing("note");
        state.update(Message::Text(TextEdit::SelectAll));
        typed(&mut state, "--- ");
        state.update(Message::Indent);
        state.update(Message::StopEditing);
        state.update(Message::Undo);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["note"], "the indent is a step of its own");
        let mut state = editing("note");
        state.update(Message::Text(TextEdit::SelectAll));
        typed(&mut state, "--- ");
        backspace(&mut state);
        state.update(Message::Undo);
        assert_eq!(texts(&state), ["note"], "the divider stays a step");
    }
}
