//! The attachments panel: the focused block's note and comments, each on a tab of a panel that
//! takes the keys while open, and the settings that turn each kind on
//! ([attachments.md §3, §6](../../../docs/specs/attachments.md#3-attachments-panel-fr-att-03),
//! FR-ATT-03, FR-ATT-06, D-94).

use std::sync::Arc;

use super::{Command, State};
use crate::document::{Attachment, ItemId, attachments};

/// A key of the open panel, or opening it, as an intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentsInput {
    /// Open the panel on the focused block, on this tab (Open Note, Open Comments).
    Open(Attachment),
    /// Show the other tab.
    SwitchTab,
    /// Close the panel.
    Close,
}

/// The open panel: the block it is on and the tab it shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Panel {
    block: ItemId,
    tab: Attachment,
}

/// The open panel as a frontend shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentsView<'a> {
    /// The block's text without its formatting, which the panel's title names.
    pub title: Arc<str>,
    /// The tabs, one per kind turned on, in order.
    pub tabs: &'a [Attachment],
    /// The tab shown.
    pub tab: Attachment,
    /// The lines of the shown tab's attachment, as the user wrote them.
    pub lines: Vec<String>,
}

impl State {
    /// The kinds of attachment turned on, in order (`attachments_notes`,
    /// `attachments_comments`, attachments.md §6).
    #[must_use]
    pub fn attachments_shown(&self) -> &[Attachment] {
        &self.attached
    }

    /// Whether the app offers `command` in its lists of commands: an attachment's only while its
    /// kind is turned on (attachments.md §6).
    pub(super) fn offers(&self, command: Command) -> bool {
        match command {
            Command::OpenAttachment(kind) => self.attached.contains(&kind),
            _ => true,
        }
    }

    /// The attachments panel, while it is open (FR-ATT-03).
    #[must_use]
    pub fn attachments_panel(&self) -> Option<AttachmentsView<'_>> {
        let panel = self.panel.as_ref()?;
        Some(AttachmentsView {
            title: self.document.get(panel.block)?.shared_plain_text(),
            tabs: &self.attached,
            tab: panel.tab,
            lines: attachments::content(&self.document, panel.block, panel.tab),
        })
    }

    /// Applies `input`: opens the panel on the focused block, or switches or closes the open one.
    pub(super) fn attachments_input(&mut self, input: AttachmentsInput) {
        match input {
            AttachmentsInput::Open(tab) => self.open_panel(tab),
            AttachmentsInput::SwitchTab => {
                let tabs = self.attached.clone();
                if let Some(panel) = &mut self.panel
                    && let Some(&next) = tabs.iter().find(|&&tab| tab != panel.tab)
                {
                    panel.tab = next;
                }
            }
            AttachmentsInput::Close => {
                self.panel = None;
                self.resume_editing();
            }
        }
    }

    /// Opens the panel on `tab` for the focused block, a divider aside, which has none, while
    /// that kind is turned on (attachments.md §3, §6).
    fn open_panel(&mut self, tab: Attachment) {
        let Some((block, _)) = self.focused() else {
            return;
        };
        if self.attached.contains(&tab) && !block.divider() {
            self.panel = Some(Panel {
                block: block.id(),
                tab,
            });
        }
    }

    /// Turns the attachments of kind `kind` on or off; the open panel leaves a tab turned off
    /// for the other, or closes without one (attachments.md §6).
    pub(super) fn show_attachments(&mut self, kind: Attachment, on: bool) {
        self.attached = Attachment::ALL
            .into_iter()
            .filter(|&other| {
                if other == kind {
                    on
                } else {
                    self.attached.contains(&other)
                }
            })
            .collect();
        if let Some(panel) = &mut self.panel
            && !self.attached.contains(&panel.tab)
        {
            match self.attached.first() {
                Some(&other) => panel.tab = other,
                None => self.attachments_input(AttachmentsInput::Close),
            }
        }
    }

    /// The panel on the block of a reloaded document that `exact` finds for its own, closed when
    /// it finds none (D-104).
    pub(super) fn rematch_panel(&mut self, exact: impl Fn(ItemId) -> Option<ItemId>) {
        self.panel = self.panel.take().and_then(|panel| {
            Some(Panel {
                block: exact(panel.block)?,
                ..panel
            })
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::{Configuration, Menu, MenuInput, Message, Mode, NodeInput},
        document::format,
    };

    const TEXT: &str = "- Milk\n\tNOTE @hidden\n\t\tFull fat\n\tCOMMENTS @hidden\n\t\t[2026-10-08 09:00] ok\n---\n";

    fn state() -> State {
        State::new(format::parse(TEXT), None)
    }

    fn input(state: &mut State, input: AttachmentsInput) {
        state.update(Message::Attachments(input));
    }

    fn configure(state: &mut State, kind: Attachment, on: bool) {
        state.update(Message::Configure(Configuration::Attachments(kind, on)));
    }

    /// The open panel's tabs and shown tab.
    fn tabs(state: &State) -> Option<(Vec<Attachment>, Attachment)> {
        state
            .attachments_panel()
            .map(|view| (view.tabs.to_vec(), view.tab))
    }

    // FR-ATT-03, attachments.md §3
    #[test]
    fn the_panel_shows_the_focused_blocks_attachment_on_the_tab_it_opened_on() {
        let mut state = state();
        assert_eq!(state.attachments_panel(), None);
        input(&mut state, AttachmentsInput::Open(Attachment::Comments));
        let view = state.attachments_panel().unwrap();
        assert_eq!(&*view.title, "Milk");
        assert_eq!(view.tabs, Attachment::ALL);
        assert_eq!(view.tab, Attachment::Comments);
        assert_eq!(view.lines, ["[2026-10-08 09:00] ok"]);
        input(&mut state, AttachmentsInput::SwitchTab);
        let view = state.attachments_panel().unwrap();
        assert_eq!(
            (view.tab, view.lines),
            (Attachment::Note, vec!["Full fat".to_owned()])
        );
        input(&mut state, AttachmentsInput::SwitchTab);
        assert_eq!(tabs(&state).unwrap().1, Attachment::Comments);
        input(&mut state, AttachmentsInput::Close);
        assert_eq!(state.attachments_panel(), None);
        input(&mut state, AttachmentsInput::SwitchTab);
        assert_eq!(state.attachments_panel(), None, "nothing to switch");
    }

    // FR-ATT-03, FR-ATT-06, FR-BLK-07, attachments.md §1, §3, §6
    #[test]
    fn the_panel_opens_only_on_a_block_that_is_no_divider_for_a_kind_turned_on() {
        let mut state = state();
        configure(&mut state, Attachment::Note, false);
        input(&mut state, AttachmentsInput::Open(Attachment::Note));
        assert_eq!(state.attachments_panel(), None, "notes are off");
        input(&mut state, AttachmentsInput::Open(Attachment::Comments));
        assert_eq!(
            tabs(&state),
            Some((vec![Attachment::Comments], Attachment::Comments))
        );
        input(&mut state, AttachmentsInput::SwitchTab);
        assert_eq!(tabs(&state).unwrap().1, Attachment::Comments, "one tab");
        input(&mut state, AttachmentsInput::Close);
        state.update(Message::FocusLast);
        input(&mut state, AttachmentsInput::Open(Attachment::Comments));
        assert_eq!(state.attachments_panel(), None, "a divider");
        let mut empty = State::new(format::parse(""), None);
        input(&mut empty, AttachmentsInput::Open(Attachment::Note));
        assert_eq!(empty.attachments_panel(), None, "no block");
    }

    // FR-ATT-06, attachments.md §6
    #[test]
    fn turning_the_shown_kind_off_moves_to_the_other_tab_or_closes() {
        let mut state = state();
        input(&mut state, AttachmentsInput::Open(Attachment::Note));
        configure(&mut state, Attachment::Comments, false);
        assert_eq!(
            tabs(&state),
            Some((vec![Attachment::Note], Attachment::Note))
        );
        configure(&mut state, Attachment::Comments, true);
        assert_eq!(
            tabs(&state),
            Some((Attachment::ALL.to_vec(), Attachment::Note))
        );
        configure(&mut state, Attachment::Note, false);
        assert_eq!(
            tabs(&state),
            Some((vec![Attachment::Comments], Attachment::Comments))
        );
        configure(&mut state, Attachment::Comments, false);
        assert_eq!(state.attachments_panel(), None);
        assert_eq!(state.attachments_shown(), []);
        configure(&mut state, Attachment::Comments, true);
        configure(&mut state, Attachment::Note, true);
        assert_eq!(state.attachments_shown(), Attachment::ALL, "in order");
    }

    // FR-ATT-06, FR-NAV-06, FR-KEY-04, attachments.md §6
    #[test]
    fn a_kind_turned_off_leaves_go_to_command_and_the_shortcuts_panel() {
        let mut state = state();
        let open = |kind| Command::OpenAttachment(kind);
        let listed = |state: &mut State| {
            state.update(Message::OpenGoToCommand(Vec::new()));
            let names: Vec<String> = state
                .go_to_command()
                .unwrap()
                .entries
                .iter()
                .map(|entry| entry.text.to_string())
                .collect();
            state.update(Message::Menu(Menu::GoToCommand, MenuInput::Dismiss));
            names
        };
        let shortcuts = |state: &State| -> Vec<Command> {
            state
                .shortcuts()
                .unwrap()
                .into_iter()
                .flat_map(|(_, entries)| entries)
                .map(|entry| entry.command)
                .collect()
        };
        state.update(Message::ToggleShortcuts);
        assert!(listed(&mut state).contains(&"Open Note".to_owned()));
        assert!(shortcuts(&state).contains(&open(Attachment::Note)));
        configure(&mut state, Attachment::Note, false);
        assert!(!listed(&mut state).contains(&"Open Note".to_owned()));
        assert!(listed(&mut state).contains(&"Open Comments".to_owned()));
        assert!(!shortcuts(&state).contains(&open(Attachment::Note)));
        assert!(shortcuts(&state).contains(&open(Attachment::Comments)));
        assert!(state.offers(Command::EditNode));
    }

    // FR-ATT-03, D-50, editing.md §2.3
    #[test]
    fn the_panel_the_command_menu_opens_returns_to_editing_when_closed() {
        let mut state = state();
        state.update(Message::EditAtEnd);
        state.update(Message::OpenCommandMenu {
            today: jiff::civil::date(2026, 10, 5),
        });
        for c in "open note".chars() {
            state.update(Message::Menu(Menu::CommandMenu, MenuInput::Type(c)));
        }
        state.update(Message::Menu(Menu::CommandMenu, MenuInput::Choose));
        assert_eq!(tabs(&state).unwrap().1, Attachment::Note);
        assert_eq!(state.mode(), Mode::Navigate, "the panel takes the keys");
        input(&mut state, AttachmentsInput::Close);
        assert_eq!(state.mode(), Mode::Edit);
    }

    // FR-ATT-06, FR-SPC-01, editing.md §7
    #[test]
    fn the_edit_node_dialog_lists_attachments_only_while_their_kind_is_off() {
        let mut state = state();
        let hidden = |state: &mut State| {
            state.update(Message::NodeEditor(NodeInput::Open));
            let hidden = state.node_editor().unwrap().hidden.len();
            state.update(Message::NodeEditor(NodeInput::Cancel));
            hidden
        };
        assert_eq!(hidden(&mut state), 0);
        configure(&mut state, Attachment::Comments, false);
        assert_eq!(hidden(&mut state), 1);
    }
}
