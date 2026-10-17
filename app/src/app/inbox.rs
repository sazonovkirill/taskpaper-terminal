//! The inbox project: Select Inbox Project, a fuzzy finder over the projects that tags the chosen
//! one `@inbox`, and Reset Inbox Project, which takes the tag away
//! ([navigation.md §6](../../../docs/specs/navigation.md#6-navigator-fr-ui-01-d-7-d-11-d-28-d-79),
//! D-141).

use super::{
    Access, MenuInput, Notice, State,
    finder::{Finder, FinderView},
    picker::Picked,
};
use crate::document::{ItemId, blocks, metadata};

impl State {
    /// Select Inbox Project, while it is open (D-141).
    #[must_use]
    pub fn inbox_picker(&self) -> Option<FinderView<'_, ItemId>> {
        self.inbox_picker.as_ref().map(Finder::view)
    }

    /// Opens Select Inbox Project over the projects as Go to Project lists them, the inbox
    /// project highlighted, or says there is no project to choose. A read-only file refuses with
    /// a notice.
    pub(super) fn open_inbox_picker(&mut self) {
        if self.access() == Access::ReadOnly {
            self.notify(Notice::ReadOnly);
            return;
        }
        let candidates = self.project_candidates();
        if candidates.is_empty() {
            self.notify(Notice::NoProjects);
            return;
        }
        let mut finder = Finder::new(candidates);
        if let Some(inbox) = self.inbox_project() {
            finder.highlight(&inbox);
        }
        self.inbox_picker = Some(finder);
    }

    /// Lists the projects of a reloaded document in the open Select Inbox Project, keeping its
    /// query and highlighting `highlighted` (file-io.md §3, D-104).
    pub(super) fn relist_inbox_picker(&mut self, highlighted: Option<ItemId>) {
        if self.inbox_picker.is_some() {
            let candidates = self.project_candidates();
            if let Some(finder) = &mut self.inbox_picker {
                finder.relist(candidates, highlighted.as_ref());
            }
        }
    }

    /// Applies `input` to Select Inbox Project, and once a project is chosen, makes it the inbox
    /// project.
    pub(super) fn inbox_picker_input(&mut self, input: MenuInput) {
        let Some(closed) = self
            .inbox_picker
            .as_mut()
            .and_then(|finder| finder.input(input))
        else {
            return;
        };
        self.inbox_picker = None;
        if let Picked::Chosen(id) = closed
            && blocks::block(self.document(), id).is_some()
        {
            self.set_inbox(Some(id));
        }
    }

    /// Reset Inbox Project: takes `@inbox` from every project.
    pub(super) fn reset_inbox(&mut self) {
        self.set_inbox(None);
    }

    /// Makes the project `to` the only one with `@inbox`, or with `None` leaves none, as one step
    /// to undo, and says so; says there is no inbox project when there is none to reset. A
    /// read-only file refuses with a notice.
    fn set_inbox(&mut self, to: Option<ItemId>) {
        let block = to.and_then(|id| blocks::block(self.document(), id));
        let ops = metadata::set_inbox(self.document(), block.as_ref());
        if ops.is_empty() && to.is_none() {
            self.notify(Notice::NoInbox);
        } else if self.access() == Access::ReadOnly {
            self.notify(Notice::ReadOnly);
        } else {
            self.commit_any(ops);
            self.notify(Notice::Inbox {
                project: to.map(|id| self.shown_text(id)),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::{
        app::{Menu, Message},
        document::format,
    };

    const TEXT: &str = "Home:\n\tErrands:\n\t\t- milk\nWork: @inbox\n- loose";

    fn open(text: &str) -> State {
        State::new(format::parse(text), None)
    }

    fn pick(state: &mut State, input: MenuInput) {
        state.update(Message::Menu(Menu::InboxPicker, input));
    }

    /// The texts of the projects Select Inbox Project lists and the highlighted position.
    fn listed(state: &State) -> (Vec<String>, usize) {
        let view = state.inbox_picker().expect("open");
        (
            view.entries
                .iter()
                .map(|entry| entry.text.to_string())
                .collect(),
            view.selected,
        )
    }

    fn lines(state: &State) -> Vec<String> {
        format::serialize(state.document())
            .lines()
            .map(str::to_owned)
            .collect()
    }

    // D-141
    #[test]
    fn choosing_a_project_makes_it_the_only_inbox_as_one_step_to_undo() {
        let mut state = open(TEXT);
        state.update(Message::OpenInboxPicker);
        assert_eq!(
            listed(&state),
            (["Work", "Home", "Errands"].map(str::to_owned).to_vec(), 0),
            "the inbox first and highlighted"
        );
        for c in "err".chars() {
            pick(&mut state, MenuInput::Type(c));
        }
        pick(&mut state, MenuInput::Choose);
        assert_eq!(state.inbox_picker(), None);
        assert_eq!(
            lines(&state),
            [
                "Home:",
                "\tErrands: @inbox",
                "\t\t- milk",
                "Work:",
                "- loose"
            ]
        );
        let inbox = state.inbox_project().expect("an inbox");
        assert_eq!(state.shown_text(inbox), "Errands");
        assert_eq!(
            state.notice(),
            Some(&Notice::Inbox {
                project: Some("Errands".to_owned())
            })
        );
        state.update(Message::Undo);
        assert_eq!(format::serialize(state.document()), TEXT);
    }

    // D-141
    #[test]
    fn dismissing_changes_nothing_and_without_projects_nothing_opens() {
        let mut state = open(TEXT);
        state.update(Message::OpenInboxPicker);
        pick(&mut state, MenuInput::Next);
        pick(&mut state, MenuInput::Dismiss);
        assert_eq!(state.inbox_picker(), None);
        assert_eq!(format::serialize(state.document()), TEXT);
        let mut state = open("- a\nnote");
        state.update(Message::OpenInboxPicker);
        assert_eq!(state.inbox_picker(), None);
        assert_eq!(state.notice(), Some(&Notice::NoProjects));
    }

    // D-141
    #[test]
    fn reset_takes_every_inbox_tag_away_or_says_there_is_none() {
        let mut state = open("A: @inbox\n\tB: @inbox(x)\n- c @inbox");
        state.update(Message::ResetInbox);
        assert_eq!(lines(&state), ["A:", "\tB:", "- c @inbox"]);
        assert_eq!(state.notice(), Some(&Notice::Inbox { project: None }));
        state.update(Message::ResetInbox);
        assert_eq!(state.notice(), Some(&Notice::NoInbox));
        state.update(Message::Undo);
        assert_eq!(lines(&state), ["A: @inbox", "\tB: @inbox(x)", "- c @inbox"]);
    }

    // D-141
    #[test]
    fn a_read_only_file_refuses_both() {
        let mut state = State::new(format::parse(TEXT), Some(PathBuf::from("/x.taskpaper")))
            .with_access(Access::ReadOnly);
        state.update(Message::OpenInboxPicker);
        assert_eq!(state.inbox_picker(), None);
        assert_eq!(state.notice(), Some(&Notice::ReadOnly));
        state.update(Message::ResetInbox);
        assert_eq!(state.notice(), Some(&Notice::ReadOnly));
        assert_eq!(format::serialize(state.document()), TEXT);
    }
}
