//! The Today view: which blocks it lists, and which intents it takes, since its entries stand
//! alone rather than in a tree
//! ([navigation.md §7](../../../docs/specs/navigation.md#7-today-view-fr-tag-04-d-7), FR-TAG-04,
//! D-7).

use jiff::civil::Date;

use super::{Location, Message, Row, State, pane::Pane};
use crate::document::{Block, ItemId, blocks};

/// Whether the Today view lists `block` on `today`: it has `@today`, or a start or due date on or
/// before `today` (FR-TAG-04).
fn for_today(block: &Block, today: Date) -> bool {
    block.today()
        || [block.start(), block.due()]
            .into_iter()
            .flatten()
            .any(|date| date <= today)
}

impl State {
    /// The Today view's entries in `pane` in document order, each at depth 0 with the path of the blocks
    /// above it. Completed blocks, each with everything inside it, are left out while hidden, as
    /// in the outline (FR-DONE-02); special nodes never show (FR-SPC-01).
    pub(super) fn today_rows(&self, pane: &Pane) -> Vec<Row> {
        let mut rows = Vec::new();
        self.push_today_rows(pane, self.document.root(), false, &mut rows);
        rows
    }

    /// Appends `pane`'s entries below the line `line`, which is done or inside a done block when
    /// `inside_done` says so.
    fn push_today_rows(&self, pane: &Pane, line: ItemId, inside_done: bool, out: &mut Vec<Row>) {
        for block in blocks::children(&self.document, line) {
            if !pane.shows(&block) {
                continue;
            }
            if for_today(&block, self.today) {
                let mut path = self.path_to(&block);
                path.pop();
                out.push(Row {
                    path,
                    hidden_children: 0,
                    ..self.row(pane, &block, 0, false, inside_done)
                });
            }
            self.push_today_rows(pane, block.id(), inside_done || block.done(), out);
        }
    }

    /// `message` as the pane takes it where it is: in the Today view an intent that needs the
    /// outline's tree around the focused block, such as folding, moving to a parent, child or
    /// sibling, or adding, pasting, indenting or moving blocks, is `None`, as is a paste outside
    /// edit mode, and ending an edit with `Enter` only stops editing (navigation.md §7).
    pub(super) fn as_located(&self, message: Message) -> Option<Message> {
        if self.pane.location != Location::Today {
            return Some(message);
        }
        match message {
            Message::NewNext => Some(Message::StopEditing),
            Message::Paste(_) | Message::Pasted { .. } if self.edit.is_none() => None,
            Message::FocusParent
            | Message::FocusNextSibling
            | Message::FocusPreviousSibling
            | Message::FocusLastChild
            | Message::Collapse
            | Message::Expand
            | Message::ToggleFold
            | Message::ToggleSubtree
            | Message::ExpandLevel
            | Message::CollapseLevel
            | Message::ExpandAll
            | Message::CollapseAll
            | Message::New(..)
            | Message::Group
            | Message::Indent
            | Message::Outdent
            | Message::MoveUp
            | Message::MoveDown => None,
            Message::Started
            | Message::Tick { .. }
            | Message::Saved
            | Message::SaveFailed { .. }
            | Message::SaveRefused { .. }
            | Message::CheckFile
            | Message::FileChecked { .. }
            | Message::KeepMine
            | Message::TakeTheirs
            | Message::ToggleDiff
            | Message::ConflictCopied { .. }
            | Message::ConflictCopyFailed { .. }
            | Message::BackupFailed { .. }
            | Message::Quit
            | Message::Terminate
            | Message::FocusNext
            | Message::FocusPrevious
            | Message::FocusFirst
            | Message::FocusLast
            | Message::PageDown { .. }
            | Message::PageUp { .. }
            | Message::ToggleShowDone
            | Message::ToggleShortcuts
            | Message::CloseShortcuts
            | Message::ZoomIn
            | Message::ZoomOut
            | Message::GoHome
            | Message::Back
            | Message::Forward
            | Message::ToggleSidebar
            | Message::FocusSidebar
            | Message::Sidebar(_)
            | Message::ToggleDone { .. }
            | Message::ChangeDate { .. }
            | Message::ToggleFlag(_)
            | Message::CycleColor
            | Message::SetColor(_)
            | Message::OpenColorPicker
            | Message::SetSection(_)
            | Message::OpenDatePrompt { .. }
            | Message::DatePrompt(_)
            | Message::ClearDate(_)
            | Message::OpenTypePicker
            | Message::OpenDetails
            | Message::CloseDetails
            | Message::OpenGoToSymbol
            | Message::OpenMoveTo
            | Message::OpenGoToCommand(_)
            | Message::Count
            | Message::SaveAs(_)
            | Message::OpenContextMenu
            | Message::Undo
            | Message::Redo
            | Message::EditAtEnd
            | Message::Text(_)
            | Message::StopEditing
            | Message::OpenCommandMenu { .. }
            | Message::Menu(..)
            | Message::CycleType
            | Message::EditPrevious
            | Message::EditNext
            | Message::Delete
            | Message::Copy
            | Message::Cut
            | Message::Paste(_)
            | Message::Pasted { .. }
            | Message::Duplicate
            | Message::OpenUrl
            | Message::OpenUrlFailed { .. }
            | Message::Select(_)
            | Message::Pane(_)
            | Message::ResizePane(_) => Some(message),
        }
    }

    /// Goes from the Today view's entry `block` to it in the outline: zooms to its parent and
    /// focuses it, as Go to Symbol does (navigation.md §7, §8).
    pub(super) fn go_to_entry(&mut self, block: &Block) {
        let parent = blocks::parent(&self.document, block.id()).map(|parent| parent.id());
        self.go_to(Location::Outline(parent));
        self.pane.focus = Some(block.id());
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, time::Duration};

    use jiff::civil::date;

    use super::*;
    use crate::{
        app::{Effect, Menu, Mode, NewBlock, Paste, SelectionChange, TextEdit},
        document::{DateChange, DateTag, Flag, format},
    };

    /// The local date of these tests.
    const TODAY: Date = date(2026, 10, 5);

    /// Blocks for today at several depths among blocks that are not: dated earlier, today and
    /// later, a date that is not one, a special node and a done project.
    const OUTLINE: &str = "\
Home:
\t- Water plants @today
\t- Paint the **hall** @due(2026-10-09)
\tErrands:
\t\t- Post office @start(2026-10-05)
\t\t- Bank @due(2026-10-01)
\t\t\tC @hidden @today
\t- Call Ann @due(soon)
Work: @done
\t- Report @due(2026-10-02)
\t- Slides
- Plan the week @start(2026-10-06)
";

    /// `text` opened on [`TODAY`] in the Today view.
    fn today(text: &str) -> State {
        let mut state = State::new(format::parse(text), None);
        state.update(Message::Tick {
            now: Duration::ZERO,
            today: TODAY,
        });
        state.go_to(Location::Today);
        assert_eq!(state.location(), Location::Today);
        state
    }

    /// Each row's text and path, joined as `path › text`.
    fn listed(state: &State) -> Vec<String> {
        state
            .rows()
            .into_iter()
            .map(|row| {
                assert_eq!(row.depth, 0, "entries are flat");
                assert!(!row.title);
                row.path
                    .iter()
                    .chain([&row.text])
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" › ")
            })
            .collect()
    }

    /// The display text of the focused block.
    fn focused(state: &State) -> String {
        state
            .focus()
            .and_then(|id| state.document().get(id))
            .map(crate::document::Item::display_text)
            .unwrap_or_default()
    }

    /// Moves the focus down `rows` rows.
    fn down(state: &mut State, rows: usize) {
        for _ in 0..rows {
            state.update(Message::FocusNext);
        }
    }

    // FR-TAG-04, D-7, FR-SPC-01
    #[test]
    fn lists_blocks_with_today_or_dated_up_to_today_flat_with_their_paths() {
        let state = today(OUTLINE);
        assert_eq!(
            listed(&state),
            [
                "Home › Water plants",
                "Home › Errands › Post office",
                "Home › Errands › Bank",
                "Work › Report",
            ]
        );
        assert_eq!(state.breadcrumbs(), Vec::<String>::new());
        assert_eq!(state.zoom(), None);
        assert_eq!(focused(&state), "Water plants");
    }

    // D-7, D-40
    #[test]
    fn entries_carry_their_block_and_whether_a_block_above_is_done() {
        let state = today(OUTLINE);
        let rows = state.rows();
        let report = rows.last().expect("an entry");
        assert!(report.inside_done && !report.done);
        assert_eq!(report.due, Some(date(2026, 10, 2)));
        let water = rows.first().expect("an entry");
        assert!(water.today && !water.inside_done);
        let mut folded = today("- a @today\n\t- b\n");
        folded.folds.insert(folded.focus().expect("a focus"));
        assert_eq!(folded.rows()[0].hidden_children, 0, "entries do not fold");
    }

    // FR-DONE-02, D-7
    #[test]
    fn hidden_completed_blocks_leave_the_list_with_everything_inside_them() {
        let mut state = today("- a @today @done\n- b @today\nP: @done\n\t- c @today\n");
        assert_eq!(listed(&state), ["a", "b", "P › c"]);
        state.update(Message::ToggleShowDone);
        assert_eq!(listed(&state), ["b"]);
        state.update(Message::ToggleShowDone);
        assert_eq!(listed(&state), ["a", "b", "P › c"]);
    }

    // D-7
    #[test]
    fn the_list_follows_the_local_date() {
        let mut state = today(OUTLINE);
        state.update(Message::Tick {
            now: Duration::from_secs(1),
            today: date(2026, 10, 6),
        });
        assert_eq!(
            listed(&state).last().map(String::as_str),
            Some("Plan the week")
        );
        assert_eq!(listed(&state).len(), 5);
        state.update(Message::Tick {
            now: Duration::from_secs(2),
            today: date(2026, 10, 1),
        });
        assert_eq!(
            listed(&state),
            ["Home › Water plants", "Home › Errands › Bank"]
        );
    }

    // FR-TAG-04, FR-DONE-01, FR-TAG-06, D-7
    #[test]
    fn block_commands_act_on_the_entries_and_the_list_follows_the_changes() {
        let mut state = today(OUTLINE);
        state.update(Message::ToggleDone { today: TODAY });
        assert!(
            state.rows()[0].done,
            "a done entry stays while completed blocks show"
        );
        down(&mut state, 1);
        state.update(Message::ChangeDate {
            tag: DateTag::Start,
            change: DateChange::Tomorrow,
            today: TODAY,
        });
        assert_eq!(listed(&state).len(), 3, "the post office starts tomorrow");
        assert_eq!(focused(&state), "Bank", "the next entry");
        state.update(Message::ToggleFlag(Flag::Today));
        assert!(state.rows()[1].today);
        state.update(Message::SetColor(Some(crate::document::TagColor::Red)));
        assert_eq!(state.rows()[1].color, Some(crate::document::TagColor::Red));
        state.update(Message::FocusLast);
        state.update(Message::ClearDate(DateTag::Due));
        assert_eq!(listed(&state).len(), 2);
        assert_eq!(
            focused(&state),
            "Bank",
            "the previous entry when none is next"
        );
    }

    // FR-MODE-01, D-7
    #[test]
    fn entries_are_edited_in_place_and_enter_only_stops_editing() {
        let mut state = today(OUTLINE);
        state.update(Message::EditAtEnd);
        state.update(Message::Text(TextEdit::Insert('!')));
        state.update(Message::NewNext);
        assert_eq!(state.mode(), Mode::Navigate);
        assert_eq!(listed(&state)[0], "Home › Water plants!");
        state.update(Message::EditAtEnd);
        state.update(Message::EditNext);
        assert_eq!(focused(&state), "Post office", "the next entry");
        assert_eq!(state.mode(), Mode::Edit);
    }

    // D-7, navigation §7
    #[test]
    fn intents_that_need_the_outlines_tree_do_nothing() {
        let mut state = today("A:\n\t- a @today\n\t\t- child\n\t- b @today\n- c\n");
        let before = format::serialize(state.document());
        for message in [
            Message::FocusParent,
            Message::FocusNextSibling,
            Message::FocusPreviousSibling,
            Message::FocusLastChild,
            Message::Collapse,
            Message::Expand,
            Message::ToggleFold,
            Message::ToggleSubtree,
            Message::ExpandLevel,
            Message::CollapseLevel,
            Message::ExpandAll,
            Message::CollapseAll,
            Message::New(NewBlock::Below, None),
            Message::Group,
            Message::Indent,
            Message::Outdent,
            Message::MoveUp,
            Message::MoveDown,
            Message::Paste(Paste::Below),
            Message::Pasted {
                text: "- x @today".to_owned(),
                at: Paste::Below,
            },
            Message::ZoomOut,
        ] {
            assert_eq!(state.update(message.clone()), [], "{message:?}");
            assert_eq!(focused(&state), "a", "{message:?}");
            assert_eq!(state.folds, HashSet::new(), "{message:?}");
            assert_eq!(state.mode(), Mode::Navigate, "{message:?}");
            assert_eq!(format::serialize(state.document()), before, "{message:?}");
            assert_eq!(state.location(), Location::Today, "{message:?}");
        }
    }

    // FR-OPS-06, D-7
    #[test]
    fn pasting_while_editing_an_entry_pastes_into_its_text() {
        let mut state = today("- a @today\n");
        state.update(Message::EditAtEnd);
        assert_eq!(
            state.update(Message::Paste(Paste::Below)),
            [Effect::ReadClipboard(Paste::Below)]
        );
        state.update(Message::Pasted {
            text: "b".to_owned(),
            at: Paste::Below,
        });
        state.update(Message::StopEditing);
        assert_eq!(listed(&state), ["ab"]);
    }

    // FR-OPS-06, FR-OPS-01, D-7
    #[test]
    fn selection_delete_and_duplicate_work_on_entries() {
        let mut state = today("- a @today\n- x\n- b @today\n- c @today\n");
        state.update(Message::Select(SelectionChange::ExtendDown));
        state.update(Message::ToggleFlag(Flag::Today));
        assert_eq!(listed(&state), ["c"]);
        assert_eq!(focused(&state), "c");
        state.update(Message::Duplicate);
        assert_eq!(listed(&state), ["c", "c"]);
        state.update(Message::Delete);
        state.update(Message::Delete);
        assert_eq!(listed(&state), Vec::<String>::new());
        assert_eq!(state.focus(), None);
        state.update(Message::Undo);
        assert_eq!(listed(&state), ["c"]);
        assert_eq!(focused(&state), "c");
    }

    // FR-OPS-05, D-7
    #[test]
    fn move_to_moves_an_entry_and_the_list_shows_its_new_path() {
        let mut state = today("A:\n\t- a @today\nB:\n");
        state.update(Message::OpenMoveTo);
        state.update(Message::Menu(
            Menu::MoveTo,
            crate::app::MenuInput::Type('B'),
        ));
        state.update(Message::Menu(Menu::MoveTo, crate::app::MenuInput::Choose));
        assert_eq!(state.location(), Location::Today);
        assert_eq!(listed(&state), ["B › a"]);
    }

    // FR-NAV-03, D-7, navigation §7
    #[test]
    fn zooming_in_on_an_entry_goes_to_it_in_the_outline() {
        let mut state = today(OUTLINE);
        down(&mut state, 2);
        state.update(Message::ZoomIn);
        assert_eq!(state.location(), Location::Outline(state.zoom()));
        assert_eq!(state.breadcrumbs(), ["Home", "Errands"]);
        assert_eq!(focused(&state), "Bank");
        let mut top = today("- a @today\n- b\n");
        top.update(Message::ZoomIn);
        assert_eq!(
            top.location(),
            Location::Outline(None),
            "a top-level entry is at home"
        );
        assert_eq!(focused(&top), "a");
        let mut empty = today("- a\n");
        empty.update(Message::ZoomIn);
        assert_eq!(empty.location(), Location::Today, "no entry to go to");
    }

    // FR-NAV-04, D-7
    #[test]
    fn today_is_a_step_of_the_history() {
        let mut state = State::new(format::parse(OUTLINE), None);
        state.update(Message::Tick {
            now: Duration::ZERO,
            today: TODAY,
        });
        down(&mut state, 4);
        state.update(Message::ZoomIn);
        state.go_to(Location::Today);
        assert_eq!(
            focused(&state),
            "Post office",
            "the block zoomed out of is an entry"
        );
        state.go_to(Location::Today);
        assert_eq!(
            state.pane.history.len(),
            3,
            "staying put is not a new entry"
        );
        state.update(Message::FocusLast);
        state.update(Message::Back);
        assert_eq!(state.breadcrumbs(), ["Home", "Errands", "Post office"]);
        assert_eq!(
            focused(&state),
            "Post office",
            "the focused entry is not under the zoom"
        );
        state.update(Message::Forward);
        assert_eq!(state.location(), Location::Today);
        assert_eq!(focused(&state), "Post office", "the block zoomed out of");
        state.update(Message::FocusNext);
        state.update(Message::Back);
        state.update(Message::Back);
        assert_eq!(state.location(), Location::Outline(None));
        assert_eq!(focused(&state), "Post office", "the block zoomed out of");
        state.update(Message::Forward);
        state.update(Message::Forward);
        state.update(Message::FocusNext);
        state.update(Message::GoHome);
        assert_eq!(state.location(), Location::Outline(None));
        assert_eq!(focused(&state), "Bank", "the focused entry shows at home");
        state.update(Message::Back);
        assert_eq!(state.location(), Location::Today);
    }
}
