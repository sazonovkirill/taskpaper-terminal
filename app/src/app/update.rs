//! The update function: messages are user intents and reports from the frontend, effects are
//! what the frontend must do.

use std::{path::PathBuf, time::Duration};

use jiff::civil::Date;

use super::{
    Command, DateInput, Location, MenuInput, Paste, SaveAs, SelectionChange, SidebarInput, State,
    TextEdit, Version, saving::Ending, structure::NewBlock,
};
use crate::document::{
    Block, DateChange, DateTag, Flag, Item, ItemId, TagColor, blocks, format::Typed,
    structure::Shift,
};

/// A user intent or a report from the frontend, independent of how a frontend receives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// The frontend is ready to carry out effects; sent once, first.
    Started,
    /// The frontend's clock reads `now`, the time since it started, on the local date `today`. A
    /// frontend sends one before every other message and whenever [`State::deadline`] passes.
    Tick {
        /// The time since the frontend started.
        now: Duration,
        /// The local date.
        today: Date,
    },
    /// The [`Disk::Save`] under way succeeded.
    Saved,
    /// The [`Disk::Save`] under way failed.
    SaveFailed {
        /// Why, for the status line.
        reason: String,
    },
    /// The [`Disk::Save`] under way was not done: the file on disk is no longer the version it
    /// expected (file-io.md §3).
    SaveRefused {
        /// The file's bytes now, or `None` when there is no file.
        contents: Option<Vec<u8>>,
    },
    /// The open file may have changed on disk: the frontend saw it change or regained focus.
    CheckFile,
    /// What a [`Disk::Check`] found.
    FileChecked {
        /// The file's bytes, or `None` when there is no file.
        contents: Option<Vec<u8>>,
    },
    /// Resolve the conflict by overwriting the file on disk with the local version.
    KeepMine,
    /// Resolve the conflict by reloading the file on disk, discarding the local version.
    TakeTheirs,
    /// Show or hide the diff between the file on disk and the local version.
    ToggleDiff,
    /// The [`Disk::ConflictCopy`] under way was written.
    ConflictCopied {
        /// Where the copy is.
        copy: PathBuf,
    },
    /// The [`Disk::ConflictCopy`] under way failed.
    ConflictCopyFailed {
        /// Why, for the status line.
        reason: String,
    },
    /// A [`Disk::Backup`] failed.
    BackupFailed {
        /// Why, for the status line.
        reason: String,
    },
    /// Leave the app, saving first; if that save fails, ask (file-io.md §2).
    Quit,
    /// The process is told to end (`SIGTERM`, `SIGHUP`): save first and end even if that fails.
    Terminate,
    /// Focus the next visible row.
    FocusNext,
    /// Focus the previous visible row.
    FocusPrevious,
    /// Focus the first visible row.
    FocusFirst,
    /// Focus the last visible row.
    FocusLast,
    /// Move focus down by a page of `rows` rows, the height of the frontend's view.
    PageDown {
        /// Rows in a page.
        rows: usize,
    },
    /// Move focus up by a page of `rows` rows, the height of the frontend's view.
    PageUp {
        /// Rows in a page.
        rows: usize,
    },
    /// Focus the parent block.
    FocusParent,
    /// Focus the next sibling block.
    FocusNextSibling,
    /// Focus the previous sibling block.
    FocusPreviousSibling,
    /// Focus the focused block's last child, expanding the block if it is collapsed.
    FocusLastChild,
    /// Collapse the focused block; if it is a leaf or already collapsed, focus its parent.
    Collapse,
    /// Expand the focused block; if it is already expanded, focus its first child.
    Expand,
    /// Expand the focused block if it is collapsed, else collapse it.
    ToggleFold,
    /// Collapse the focused block's whole subtree if any of it is expanded, else expand it all.
    ToggleSubtree,
    /// Expand every collapsed block at the shallowest depth that has one.
    ExpandLevel,
    /// Collapse every visible expanded block at the deepest depth that has one.
    CollapseLevel,
    /// Expand every block under the zoom.
    ExpandAll,
    /// Collapse every block under the zoom.
    CollapseAll,
    /// Hide done blocks, each with everything inside it, or show them again (FR-DONE-02).
    ToggleShowDone,
    /// Open the shortcuts panel, or close it if it is open (FR-KEY-04).
    ToggleShortcuts,
    /// Close the shortcuts panel.
    CloseShortcuts,
    /// Show the focused block as the root; in the Today view, go to the focused entry in the
    /// outline (D-7).
    ZoomIn,
    /// Show the zoom block's parent as the root, focusing the block zoomed out of.
    ZoomOut,
    /// Show the whole document.
    GoHome,
    /// Return to the previous zoom in the history.
    Back,
    /// Go to the next zoom in the history.
    Forward,
    /// Show the sidebar, or hide it and give the pane the keys (FR-UI-01).
    ToggleSidebar,
    /// Show the sidebar and give it the keys (FR-UI-01).
    FocusSidebar,
    /// A key of the focused sidebar (FR-UI-01).
    Sidebar(SidebarInput),
    /// Complete the selected blocks, else the focused block, dating them `today`, or uncomplete
    /// them when all are done (FR-DONE-01, FR-OPS-07).
    ToggleDone {
        /// The frontend's local date.
        today: Date,
    },
    /// Set or move the `tag` date of the selected blocks, else the focused block, as `change`
    /// says, `today` being the frontend's local date (FR-TAG-06, FR-OPS-07).
    ChangeDate {
        /// The date to change.
        tag: DateTag,
        /// How it changes.
        change: DateChange,
        /// The frontend's local date.
        today: Date,
    },
    /// Add the bare tag `@today` or `@bookmark` to the selected blocks, else the focused block, or
    /// remove it when all have it (FR-TAG-04, D-11, FR-OPS-07).
    ToggleFlag(Flag),
    /// Give the selected blocks, else the focused block, the color after the focused block's own:
    /// none, red, orange, yellow, green, blue, violet and back to none (FR-TAG-06, D-26).
    CycleColor,
    /// Give the selected blocks, else the focused block, `color`, or no color when `None`
    /// (FR-TAG-05, FR-OPS-07, D-48).
    SetColor(Option<TagColor>),
    /// Open the color picker on the focused block, for it or the selected blocks (FR-TAG-05).
    OpenColorPicker,
    /// Make the selected text blocks, else the focused block, sections by adding `@section`, or
    /// with `false` plain text by removing it; a task or project is left as it is, with a notice
    /// (FR-BLK-04, FR-OPS-07, D-52).
    SetSection(bool),
    /// Open the prompt for the focused block's `tag` date, resolving what is typed against
    /// `today`, the frontend's local date (FR-TAG-03).
    OpenDatePrompt {
        /// The date to set.
        tag: DateTag,
        /// The frontend's local date.
        today: Date,
    },
    /// A key of the open date prompt (FR-TAG-03).
    DatePrompt(DateInput),
    /// Remove the `tag` date of the selected blocks, else the focused block (FR-TAG-03, D-50).
    ClearDate(DateTag),
    /// Open the type picker on the focused block (FR-BLK-08).
    OpenTypePicker,
    /// Open the details popup on the focused block (D-3).
    OpenDetails,
    /// Close the details popup.
    CloseDetails,
    /// Open Go to Symbol (FR-NAV-05).
    OpenGoToSymbol,
    /// Open Move to over the selected blocks, else the focused block (FR-OPS-05).
    OpenMoveTo,
    /// Open Go to Command over the commands of navigate mode, each shown with its bindings as
    /// the frontend writes them, for the commands it binds (FR-NAV-06).
    OpenGoToCommand(Vec<(Command, String)>),
    /// Show how many blocks and words are under the focused block (FR-OPS-09).
    Count,
    /// Open the context menu on the focused sidebar entry while the sidebar has the keys, else
    /// on the selection or the focused block (FR-KEY-06).
    OpenContextMenu,
    /// A key of the open menu or picker that `Menu` names.
    Menu(Menu, MenuInput),
    /// Save As: a key of its prompt or the frontend's report (FR-DOC-06).
    SaveAs(SaveAs),
    /// Revert the latest edit (FR-OPS-08); in edit mode, the latest change to the text.
    Undo,
    /// Replay the latest undone edit (FR-OPS-08); in edit mode, the latest undone change to the
    /// text.
    Redo,
    /// Edit the focused block's text with the caret at its end (FR-MODE-01).
    EditAtEnd,
    /// Change the text being edited (FR-MODE-03).
    Text(TextEdit),
    /// Stop editing and return to navigate mode on the same block (FR-MODE-01).
    StopEditing,
    /// Open the command menu at the caret, as typing `/` in edit mode does, `today` being the
    /// frontend's local date that its date commands write (D-50).
    OpenCommandMenu {
        /// The local date.
        today: Date,
    },
    /// Give the block being edited the next type of project, section, task and note, as `Ctrl+T`
    /// does (D-50).
    CycleType,
    /// Add a block where this says, relative to the focused block or the selection, of this type
    /// or, with `None`, of the type that fits there, and edit it (FR-MODE-04, FR-MODE-06).
    New(NewBlock, Option<Typed>),
    /// Stop editing and add the next block: a first child when the block shows children, else
    /// a block below it (editing.md §3).
    NewNext,
    /// Stop editing and edit the previous visible block, keeping the caret's column where it can
    /// (editing.md §2).
    EditPrevious,
    /// Stop editing and edit the next visible block, keeping the caret's column where it can
    /// (editing.md §2).
    EditNext,
    /// Delete the selected blocks, else the focused block, with their subtrees (FR-OPS-06).
    Delete,
    /// Copy the selected text in edit mode, else the selected blocks, else the focused block, to
    /// the clipboard (FR-OPS-06).
    Copy,
    /// Copy as [`Message::Copy`] does, then delete what was copied (FR-OPS-06).
    Cut,
    /// Paste what the clipboard holds; the frontend reads it for [`Effect::ReadClipboard`]
    /// (FR-OPS-06).
    Paste(Paste),
    /// Text from the clipboard or pasted into the terminal, to paste (FR-OPS-06).
    Pasted {
        /// The text.
        text: String,
        /// Where blocks it becomes go.
        at: Paste,
    },
    /// Put a copy of each of the selected blocks, else the focused block, with its subtree after
    /// it (FR-OPS-06).
    Duplicate,
    /// Make the selected blocks, else the focused block, the last children of their previous
    /// siblings (FR-OPS-02); in edit mode, editing goes on.
    Indent,
    /// Make the selected blocks, else the focused block, the next siblings of their parents
    /// (FR-OPS-02); in edit mode, editing goes on.
    Outdent,
    /// Move the selected blocks, else the focused block, above their previous siblings, or at the
    /// first position into the parent's previous sibling (FR-OPS-03).
    MoveUp,
    /// Move the selected blocks, else the focused block, below their next siblings, or at the
    /// last position into the parent's next sibling (FR-OPS-03).
    MoveDown,
    /// Open the focused block's link in the system browser (FR-BLK-06, D-45).
    OpenUrl,
    /// The [`Effect::OpenUrl`] could not start the browser.
    OpenUrlFailed {
        /// The URL.
        url: String,
        /// Why, for the status line.
        reason: String,
    },
    /// Change which blocks are selected (FR-OPS-01).
    Select(SelectionChange),
    /// Put the selected blocks, else the focused block, under a new text block where the first
    /// one was, and edit it (FR-OPS-04).
    Group,
}

/// A menu or picker that takes the keys while it is open, which [`Message::Menu`] names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    /// The color picker (FR-TAG-05).
    ColorPicker,
    /// The type picker (FR-BLK-08).
    TypePicker,
    /// Go to Symbol (FR-NAV-05).
    GoToSymbol,
    /// Move to (FR-OPS-05).
    MoveTo,
    /// Go to Command (FR-NAV-06).
    GoToCommand,
    /// The command menu of edit mode (D-50).
    CommandMenu,
    /// The context menu (FR-KEY-06).
    ContextMenu,
}

impl Message {
    /// Whether this intent belongs to navigate mode, so that it ends an edit session first,
    /// rather than to edit mode or the frontend's reports, which leave the session alone.
    fn ends_editing(&self) -> bool {
        match self {
            Self::Started
            | Self::Tick { .. }
            | Self::Saved
            | Self::SaveFailed { .. }
            | Self::SaveRefused { .. }
            | Self::CheckFile
            | Self::FileChecked { .. }
            | Self::ToggleDiff
            | Self::ConflictCopied { .. }
            | Self::ConflictCopyFailed { .. }
            | Self::BackupFailed { .. }
            | Self::OpenUrlFailed { .. }
            | Self::Undo
            | Self::Redo
            | Self::EditAtEnd
            | Self::Text(_)
            | Self::StopEditing
            | Self::OpenCommandMenu { .. }
            | Self::CycleType
            | Self::DatePrompt(_)
            | Self::NewNext
            | Self::EditPrevious
            | Self::EditNext
            | Self::ToggleShortcuts
            | Self::CloseShortcuts
            | Self::Indent
            | Self::Outdent
            | Self::Copy
            | Self::Cut
            | Self::Paste(_)
            | Self::Pasted { .. } => false,
            Self::Quit
            | Self::Terminate
            | Self::KeepMine
            | Self::TakeTheirs
            | Self::FocusNext
            | Self::FocusPrevious
            | Self::FocusFirst
            | Self::FocusLast
            | Self::PageDown { .. }
            | Self::PageUp { .. }
            | Self::FocusParent
            | Self::FocusNextSibling
            | Self::FocusPreviousSibling
            | Self::FocusLastChild
            | Self::Collapse
            | Self::Expand
            | Self::ToggleFold
            | Self::ToggleSubtree
            | Self::ExpandLevel
            | Self::CollapseLevel
            | Self::ExpandAll
            | Self::CollapseAll
            | Self::ToggleShowDone
            | Self::ZoomIn
            | Self::ZoomOut
            | Self::GoHome
            | Self::Back
            | Self::Forward
            | Self::ToggleSidebar
            | Self::FocusSidebar
            | Self::Sidebar(_)
            | Self::ToggleDone { .. }
            | Self::ChangeDate { .. }
            | Self::ToggleFlag(_)
            | Self::CycleColor
            | Self::SetColor(_)
            | Self::SetSection(_)
            | Self::OpenColorPicker
            | Self::OpenDatePrompt { .. }
            | Self::ClearDate(_)
            | Self::OpenTypePicker
            | Self::OpenDetails
            | Self::CloseDetails
            | Self::OpenGoToSymbol
            | Self::OpenMoveTo
            | Self::OpenGoToCommand(_)
            | Self::Count
            | Self::SaveAs(_)
            | Self::OpenContextMenu
            | Self::New(..)
            | Self::Delete
            | Self::Duplicate
            | Self::MoveUp
            | Self::MoveDown
            | Self::OpenUrl
            | Self::Select(_)
            | Self::Group => true,
            Self::Menu(menu, _) => menu.ends_editing(),
        }
    }
}

impl Menu {
    /// Whether a key of this menu belongs to navigate mode, ending an edit session first, rather
    /// than to the command menu of edit mode.
    fn ends_editing(self) -> bool {
        match self {
            Self::CommandMenu => false,
            Self::ColorPicker
            | Self::TypePicker
            | Self::GoToSymbol
            | Self::MoveTo
            | Self::GoToCommand
            | Self::ContextMenu => true,
        }
    }
}

/// Work the frontend performs for the core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// End the session.
    Quit,
    /// Disk work, carried out in order and away from the frontend's event handling (N-UX-01).
    Disk(Disk),
    /// Put this text on the clipboard (FR-OPS-06).
    CopyToClipboard(String),
    /// Read the clipboard and send what it holds as [`Message::Pasted`] at this place
    /// (FR-OPS-06).
    ReadClipboard(Paste),
    /// Show this URL in the system browser; a failure comes back as [`Message::OpenUrlFailed`]
    /// (FR-BLK-06).
    OpenUrl(String),
    /// Give this command, chosen in Go to Command, as its keys would: its message carries what
    /// the frontend knows, or the frontend carries it out (FR-NAV-06).
    Run(Command),
    /// Complete the folder that ends this path as typed, `~` standing for the home folder, and
    /// report the result as [`SaveAs::Completed`] when there is one (file-io.md §2).
    CompletePath(String),
    /// Resolve this path as typed, `~` standing for the home folder, and report it as
    /// [`SaveAs::Resolved`].
    ResolvePath(String),
    /// Watch the file at this path for changes made elsewhere, instead of any watched before,
    /// sending [`Message::CheckFile`] after each burst of them (file-io.md §3).
    Watch(PathBuf),
}

/// Disk work for the frontend; each kind reports its result back as a [`Message`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Disk {
    /// Make sure today's backup copy of the file at `path` exists, copying the file as it is on
    /// disk if not (FR-DOC-07); a failure comes back as [`Message::BackupFailed`].
    Backup {
        /// The open file.
        path: PathBuf,
    },
    /// Replace the file at `path` with `text` atomically (F-FILE-04), unless just before that
    /// the file is no longer at `expected`; the result comes back as [`Message::Saved`],
    /// [`Message::SaveFailed`] or [`Message::SaveRefused`] (file-io.md §3).
    Save {
        /// The open file.
        path: PathBuf,
        /// The serialized document.
        text: String,
        /// The version the file must still be, or `None` when there must be no file.
        expected: Option<Version>,
    },
    /// Read the file at `path`; the result comes back as [`Message::FileChecked`].
    Check {
        /// The open file.
        path: PathBuf,
    },
    /// Write `text` to the file at `path` for Save As, as [`Disk::Save`] does; the result comes
    /// back as [`SaveAs::Saved`], [`SaveAs::Failed`] or [`SaveAs::Refused`] (FR-DOC-06).
    SaveAs {
        /// The file to write.
        path: PathBuf,
        /// The serialized document.
        text: String,
        /// The version the file must still be, or `None` when there must be no file.
        expected: Option<Version>,
    },
    /// Write `contents` to a new conflict copy in the backups folder of the file at `path`, even
    /// when backups are off; the result comes back as [`Message::ConflictCopied`] or
    /// [`Message::ConflictCopyFailed`] (file-io.md §3).
    ConflictCopy {
        /// The open file.
        path: PathBuf,
        /// The version a conflict choice discards.
        contents: Vec<u8>,
    },
}

impl State {
    /// Applies `message` and returns the effects the frontend must carry out.
    pub fn update(&mut self, message: Message) -> Vec<Effect> {
        let Some(message) = self.as_located(message) else {
            return Vec::new();
        };
        if message.ends_editing() {
            self.stop_editing();
        }
        let mut effects = self.dispatch(message);
        self.repair_focus();
        self.keep_visible_selection();
        self.expire_notice();
        effects.extend(self.save_if_due());
        effects
    }

    /// Hands `message` to the feature that handles it and returns the effects that asks for.
    fn dispatch(&mut self, message: Message) -> Vec<Effect> {
        let mut effects = Vec::new();
        match message {
            Message::Started => effects = [self.backup_on_open(), self.watch()].concat(),
            Message::Tick { now, today } => {
                self.now = now;
                self.today = today;
            }
            Message::Saved => self.saved(),
            Message::SaveFailed { reason } => self.save_failed(reason),
            Message::SaveRefused { contents } => self.save_refused(contents),
            Message::CheckFile => effects = self.check_file(),
            Message::FileChecked { contents } => self.found(contents),
            Message::KeepMine => effects = self.keep_mine(),
            Message::TakeTheirs => effects = self.take_theirs(),
            Message::ToggleDiff => self.toggle_diff(),
            Message::ConflictCopied { copy } => self.conflict_copied(copy),
            Message::ConflictCopyFailed { reason } => self.conflict_copy_failed(reason),
            Message::BackupFailed { reason } => self.backup_failed(reason),
            Message::Quit => self.autosave.end(Ending::Quit),
            Message::Terminate => self.autosave.end(Ending::Terminate),
            Message::FocusNext => self.focus_down(1),
            Message::FocusPrevious => self.focus_up(1),
            Message::FocusFirst => self.move_focus(|_, _| 0),
            Message::FocusLast => self.move_focus(|_, last| last),
            Message::PageDown { rows } => self.focus_down(rows),
            Message::PageUp { rows } => self.focus_up(rows),
            Message::FocusParent => self.focus_parent(),
            Message::FocusNextSibling => self.move_to_sibling(|at| at.checked_add(1)),
            Message::FocusPreviousSibling => self.move_to_sibling(|at| at.checked_sub(1)),
            Message::FocusLastChild => self.focus_last_child(),
            Message::Collapse => self.collapse(),
            Message::Expand => self.expand(),
            Message::ToggleFold => self.toggle_fold(),
            Message::ToggleSubtree => self.toggle_subtree(),
            Message::ExpandLevel => self.expand_level(),
            Message::CollapseLevel => self.collapse_level(),
            Message::ExpandAll => self.expand_all(),
            Message::CollapseAll => self.collapse_all(),
            Message::ToggleShowDone => self.pane.show_done = !self.pane.show_done,
            Message::ToggleShortcuts => self.shortcuts = !self.shortcuts,
            Message::CloseShortcuts => self.shortcuts = false,
            Message::ZoomIn => self.zoom_in(),
            Message::ZoomOut => self.zoom_out(),
            Message::GoHome => self.go_to(Location::Outline(None)),
            Message::Back => self.walk_history(self.pane.history_at.checked_sub(1)),
            Message::Forward => self.walk_history(self.pane.history_at.checked_add(1)),
            Message::ToggleSidebar => self.toggle_sidebar(),
            Message::FocusSidebar => self.focus_sidebar(),
            Message::Sidebar(input) => self.sidebar_input(input),
            Message::ToggleDone { today } => self.toggle_done(today),
            Message::ChangeDate { tag, change, today } => self.change_date(tag, change, today),
            Message::ToggleFlag(flag) => self.toggle_flag(flag),
            Message::CycleColor => self.cycle_color(),
            Message::SetColor(color) => self.set_color(color),
            Message::SetSection(on) => self.set_section(on),
            Message::OpenColorPicker => self.open_color_picker(),
            Message::OpenDatePrompt { tag, today } => self.open_date_prompt(tag, today),
            Message::DatePrompt(input) => self.date_prompt_input(input),
            Message::ClearDate(tag) => self.clear_date(tag),
            Message::OpenTypePicker => self.open_type_picker(),
            Message::OpenDetails => self.open_details(),
            Message::CloseDetails => self.close_details(),
            Message::OpenGoToSymbol => self.open_go_to_symbol(),
            Message::OpenMoveTo => self.open_move_to(),
            Message::OpenGoToCommand(bindings) => self.open_go_to_command(&bindings),
            Message::Count => self.count(),
            Message::OpenContextMenu => self.open_context_menu(),
            Message::Menu(menu, input) => effects = self.menu_input(menu, input),
            Message::SaveAs(event) => effects = self.save_as_event(event),
            Message::Undo => self.undo(),
            Message::Redo => self.redo(),
            Message::EditAtEnd => self.start_editing(),
            Message::Text(edit) => self.edit_text(edit),
            Message::StopEditing => self.stop_editing(),
            Message::OpenCommandMenu { today } => self.open_command_menu(today),
            Message::CycleType => self.cycle_type(),
            Message::New(at, typed) => self.add_block(at, typed),
            Message::NewNext => self.new_next(),
            Message::EditPrevious => self.edit_adjacent(false),
            Message::EditNext => self.edit_adjacent(true),
            Message::Delete => self.delete_selected(),
            Message::Duplicate => self.duplicate(),
            Message::Copy => effects = self.copy(),
            Message::Cut => effects = self.cut(),
            Message::Paste(at) => effects.push(Effect::ReadClipboard(at)),
            Message::Pasted { text, at } => self.pasted(&text, at),
            Message::Indent => self.shift_selected(Shift::Indent),
            Message::Outdent => self.shift_selected(Shift::Outdent),
            Message::MoveUp => self.shift_selected(Shift::Up),
            Message::MoveDown => self.shift_selected(Shift::Down),
            Message::Select(change) => self.change_selection(change),
            Message::Group => self.group(),
            Message::OpenUrl => effects = self.open_url(),
            Message::OpenUrlFailed { url, reason } => self.open_url_failed(url, reason),
        }
        effects
    }

    /// Hands a key of the open `menu` to the feature that owns it and returns the effects that
    /// asks for.
    fn menu_input(&mut self, menu: Menu, input: MenuInput) -> Vec<Effect> {
        match menu {
            Menu::ColorPicker => self.color_picker_input(input),
            Menu::TypePicker => self.type_picker_input(input),
            Menu::GoToSymbol => self.go_to_symbol_input(input),
            Menu::MoveTo => self.move_to_input(input),
            Menu::GoToCommand => return self.go_to_command_input(input).into_iter().collect(),
            Menu::CommandMenu => return self.command_menu_input(input),
            Menu::ContextMenu => return self.context_menu_input(input).into_iter().collect(),
        }
        Vec::new()
    }

    /// The focused block and whether it is the title row.
    pub(super) fn focused(&self) -> Option<(Block, bool)> {
        let id = self.pane.focus?;
        Some((blocks::block(self.document(), id)?, Some(id) == self.zoom()))
    }

    /// The focused block unless it is the title, which stands for the view's root: it does not
    /// fold, zoom in or move to its parent or siblings.
    pub(super) fn focused_block(&self) -> Option<Block> {
        self.focused()
            .and_then(|(block, title)| (!title).then_some(block))
    }

    /// The blocks in the list of the block `id`: its parent's children, itself included.
    pub(super) fn siblings(&self, id: ItemId) -> Vec<Block> {
        self.document()
            .get(id)
            .and_then(Item::parent)
            .map(|line| blocks::children(self.document(), line))
            .unwrap_or_default()
    }

    /// Zooms into the focused block (FR-NAV-03), or in the Today view goes to it in the outline
    /// (D-7).
    fn zoom_in(&mut self) {
        let Some(block) = self.focused_block() else {
            return;
        };
        match self.pane.location {
            Location::Outline(_) => self.go_to(Location::Outline(Some(block.id()))),
            Location::Today => self.go_to_entry(&block),
        }
    }

    /// Zooms out to the parent of the zoomed block (FR-NAV-03).
    fn zoom_out(&mut self) {
        if let Some(zoom) = self.zoom() {
            let parent = blocks::parent(self.document(), zoom).map(|block| block.id());
            self.go_to(Location::Outline(parent));
        }
    }

    /// Shows `location`, a new entry in the history that drops any forward entries.
    pub(super) fn go_to(&mut self, location: Location) {
        if location == self.pane.location {
            return;
        }
        self.pane.history.truncate(self.pane.history_at + 1);
        self.pane.history.push(location);
        self.pane.history_at = self.pane.history.len() - 1;
        self.show(location);
    }

    /// Shows the history entry at `at`, if there is one.
    fn walk_history(&mut self, at: Option<usize>) {
        if let Some(at) = at
            && let Some(&location) = self.pane.history.get(at)
        {
            self.pane.history_at = at;
            self.show(location);
        }
    }

    /// Shows `location` and focuses the block left when it is visible: the block zoomed out of,
    /// or the focused entry when leaving the Today view; else the first block under the title,
    /// else the title.
    fn show(&mut self, location: Location) {
        let left = match self.pane.location {
            Location::Outline(zoom) => zoom,
            Location::Today => self.pane.focus,
        };
        self.pane.location = location;
        let rows = self.rows();
        self.pane.focus = left
            .filter(|&id| rows.iter().any(|row| row.id == id))
            .or_else(|| {
                rows.iter()
                    .find(|row| !row.title)
                    .or(rows.first())
                    .map(|row| row.id)
            });
    }

    fn focus_parent(&mut self) {
        if let Some(parent) = self
            .focused_block()
            .and_then(|block| blocks::parent(self.document(), block.id()))
        {
            self.pane.focus = Some(parent.id());
        }
    }

    /// Focuses the sibling at the index `to` picks from the focused block's index among its
    /// siblings, if there is one.
    fn move_to_sibling(&mut self, to: impl FnOnce(usize) -> Option<usize>) {
        let Some(id) = self.focused_block().map(|block| block.id()) else {
            return;
        };
        let mut siblings = self.siblings(id);
        siblings.retain(|sibling| self.shows(sibling));
        if let Some(at) = siblings.iter().position(|block| block.id() == id)
            && let Some(sibling) = to(at).and_then(|index| siblings.get(index))
        {
            self.pane.focus = Some(sibling.id());
        }
    }

    /// On the title, focuses the last block below it and leaves the zoom block's fold alone.
    fn focus_last_child(&mut self) {
        let Some((block, title)) = self.focused() else {
            return;
        };
        if let Some(child) = self.shown_children(&block).last() {
            if !title {
                self.folds.remove(&block.id());
            }
            self.pane.focus = Some(child.id());
        }
    }

    fn collapse(&mut self) {
        let Some(block) = self.focused_block() else {
            return;
        };
        if self.is_expanded(&block) {
            self.folds.insert(block.id());
        } else {
            self.focus_parent();
        }
    }

    fn expand(&mut self) {
        let Some((block, title)) = self.focused() else {
            return;
        };
        if (title || !self.folds.remove(&block.id()))
            && let Some(child) = self.shown_children(&block).first()
        {
            self.pane.focus = Some(child.id());
        }
    }

    fn toggle_fold(&mut self) {
        let Some(block) = self.focused_block() else {
            return;
        };
        if !self.folds.remove(&block.id()) {
            self.fold([block]);
        }
    }

    /// On the title, acts on the blocks below it.
    fn toggle_subtree(&mut self) {
        let Some((block, title)) = self.focused() else {
            return;
        };
        let mut subtree = if title {
            Vec::new()
        } else {
            vec![block.clone()]
        };
        self.push_descendants(&block, &mut subtree);
        if subtree.iter().any(|block| self.is_expanded(block)) {
            self.fold(subtree);
        } else {
            for block in &subtree {
                self.folds.remove(&block.id());
            }
        }
    }

    /// Appends every descendant block of `block`, collapsed or not, to `out`.
    fn push_descendants(&self, block: &Block, out: &mut Vec<Block>) {
        for child in self.children(block) {
            self.push_descendants(&child, out);
            out.push(child);
        }
    }

    fn expand_level(&mut self) {
        let collapsed: Vec<(Block, usize)> = self
            .blocks(false)
            .into_iter()
            .filter(|(block, _)| {
                self.folds.contains(&block.id()) && !self.children(block).is_empty()
            })
            .collect();
        let Some(depth) = collapsed.iter().map(|&(_, depth)| depth).min() else {
            return;
        };
        for (block, _) in collapsed.iter().filter(|&&(_, d)| d == depth) {
            self.folds.remove(&block.id());
        }
    }

    fn collapse_level(&mut self) {
        let expanded: Vec<(Block, usize)> = self
            .blocks(true)
            .into_iter()
            .filter(|(block, _)| self.is_expanded(block))
            .collect();
        let Some(depth) = expanded.iter().map(|&(_, depth)| depth).max() else {
            return;
        };
        let deepest = expanded
            .into_iter()
            .filter(|&(_, d)| d == depth)
            .map(|(block, _)| block);
        self.fold(deepest);
    }

    fn expand_all(&mut self) {
        for (block, _) in self.blocks(false) {
            self.folds.remove(&block.id());
        }
    }

    fn collapse_all(&mut self) {
        let all = self.blocks(false).into_iter().map(|(block, _)| block);
        self.fold(all);
    }

    /// Whether `block` has children and is not collapsed.
    pub(super) fn is_expanded(&self, block: &Block) -> bool {
        !self.folds.contains(&block.id()) && !self.children(block).is_empty()
    }

    /// Collapses each of `blocks` that has children; a leaf has nothing to fold.
    fn fold(&mut self, blocks: impl IntoIterator<Item = Block>) {
        for block in blocks {
            if !self.children(&block).is_empty() {
                self.folds.insert(block.id());
            }
        }
    }

    /// Moves focus off a block a fold or hiding done blocks hid: to the next visible block in
    /// document order, else the previous one, else the first row, the title (navigation §2). With
    /// no focus, as when every block was hidden, focuses the first row if one is back.
    fn repair_focus(&mut self) {
        let visible: Vec<_> = self.rows().into_iter().map(|row| row.id).collect();
        let Some(focus) = self.pane.focus else {
            self.pane.focus = visible.first().copied();
            return;
        };
        if visible.contains(&focus) {
            return;
        }
        let order: Vec<_> = self
            .blocks(false)
            .into_iter()
            .map(|(block, _)| block.id())
            .collect();
        let at = order.iter().position(|&id| id == focus).unwrap_or_default();
        self.pane.focus = order
            .iter()
            .skip(at)
            .chain(order.iter().take(at).rev())
            .find(|id| visible.contains(id))
            .or(visible.first())
            .copied();
    }

    /// Focuses the row `rows` rows below the focused one, or the last.
    fn focus_down(&mut self, rows: usize) {
        self.move_focus(|at, last| at.saturating_add(rows).min(last));
    }

    /// Focuses the row `rows` rows above the focused one, or the first.
    fn focus_up(&mut self, rows: usize) {
        self.move_focus(|at, _| at.saturating_sub(rows));
    }

    /// Focuses the row at the index `to` picks from the focused index and the last index. Without
    /// a focus there are no rows, so there is nothing to move.
    fn move_focus(&mut self, to: impl FnOnce(usize, usize) -> usize) {
        let rows = self.rows();
        let Some(at) = rows.iter().position(|row| Some(row.id) == self.pane.focus) else {
            return;
        };
        if let Some(row) = rows.get(to(at, rows.len() - 1)) {
            self.pane.focus = Some(row.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    use crate::document::{Document, format};

    /// Five rows: A, b, c, d, e.
    fn state() -> State {
        State::new(format::parse("A:\n\t- b\n\t\tc\nd\n- e"), None)
    }

    /// The index of the focused row.
    fn focused(state: &State) -> Option<usize> {
        state
            .rows()
            .iter()
            .position(|row| Some(row.id) == state.focus())
    }

    /// Focuses row `index` by moving down from the first row.
    fn at(index: usize) -> State {
        let mut state = state();
        for _ in 0..index {
            state.update(Message::FocusNext);
        }
        assert_eq!(focused(&state), Some(index));
        state
    }

    #[track_caller]
    fn check(from: usize, message: Message, expected: usize) {
        let mut state = at(from);
        let name = format!("{message:?}");
        assert_eq!(state.update(message), [], "{name} has no effects");
        assert_eq!(focused(&state), Some(expected), "{name} from {from}");
    }

    #[test]
    fn quit_asks_the_frontend_to_quit() {
        assert_eq!(state().update(Message::Quit), [Effect::Quit]);
    }

    // FR-NAV-01
    #[test]
    fn next_and_previous_move_one_row_and_stop_at_the_ends() {
        check(0, Message::FocusNext, 1);
        check(3, Message::FocusNext, 4);
        check(4, Message::FocusNext, 4);
        check(4, Message::FocusPrevious, 3);
        check(1, Message::FocusPrevious, 0);
        check(0, Message::FocusPrevious, 0);
    }

    // FR-NAV-01
    #[test]
    fn first_and_last_jump_to_the_ends() {
        check(2, Message::FocusFirst, 0);
        check(0, Message::FocusFirst, 0);
        check(2, Message::FocusLast, 4);
        check(4, Message::FocusLast, 4);
    }

    // FR-NAV-01
    #[test]
    fn pages_move_by_the_page_height_and_stop_at_the_ends() {
        check(0, Message::PageDown { rows: 2 }, 2);
        check(1, Message::PageDown { rows: 3 }, 4);
        check(3, Message::PageDown { rows: 2 }, 4);
        check(0, Message::PageDown { rows: usize::MAX }, 4);
        check(4, Message::PageUp { rows: 3 }, 1);
        check(2, Message::PageUp { rows: 3 }, 0);
        check(2, Message::PageDown { rows: 0 }, 2);
        check(2, Message::PageUp { rows: 0 }, 2);
    }

    #[test]
    fn a_single_row_keeps_its_focus() {
        let mut state = State::new(format::parse("- only"), None);
        let only = state.focus();
        assert!(only.is_some());
        for message in [
            Message::FocusNext,
            Message::FocusPrevious,
            Message::FocusLast,
            Message::PageDown { rows: 5 },
        ] {
            state.update(message.clone());
            assert_eq!(state.focus(), only, "{message:?}");
        }
    }

    /// Each visible row as its depth, display text and hidden-children count.
    fn visible(state: &State) -> Vec<(usize, String, usize)> {
        state
            .rows()
            .into_iter()
            .map(|row| (row.depth, row.text, row.hidden_children))
            .collect()
    }

    /// The display text of the focused row.
    fn focus_text(state: &State) -> String {
        let rows = state.rows();
        let row = rows.iter().find(|row| Some(row.id) == state.focus());
        row.map(|row| row.text.clone()).unwrap_or_default()
    }

    /// `text` with focus moved down `index` rows.
    fn open(text: &str, index: usize) -> State {
        let mut state = State::new(format::parse(text), None);
        for _ in 0..index {
            state.update(Message::FocusNext);
        }
        state
    }

    /// Three levels: A > b > c, A > d, then e.
    const TREE: &str = "A:\n\t- b\n\t\t- c\n\t- d\n- e";

    // FR-NAV-02
    #[test]
    fn collapse_hides_children_and_counts_them() {
        let mut state = open(TREE, 0);
        state.update(Message::Collapse);
        assert_eq!(
            visible(&state),
            [(0, "A".to_owned(), 2), (0, "e".to_owned(), 0)]
        );
        assert_eq!(focus_text(&state), "A");
    }

    // FR-NAV-01, FR-NAV-02
    #[test]
    fn collapse_on_a_leaf_or_collapsed_block_focuses_the_parent() {
        let mut state = open(TREE, 2);
        state.update(Message::Collapse);
        assert_eq!(focus_text(&state), "b");
        state.update(Message::Collapse);
        assert_eq!(focus_text(&state), "b");
        state.update(Message::Collapse);
        assert_eq!(focus_text(&state), "A");
        let mut top = open(TREE, 4);
        top.update(Message::Collapse);
        assert_eq!(focus_text(&top), "e", "a top-level leaf has no parent");
    }

    // FR-NAV-01, FR-NAV-02
    #[test]
    fn expand_opens_then_focuses_the_first_child() {
        let mut state = open(TREE, 0);
        state.update(Message::Collapse);
        state.update(Message::Expand);
        assert_eq!(visible(&state).len(), 5);
        assert_eq!(focus_text(&state), "A");
        state.update(Message::Expand);
        assert_eq!(focus_text(&state), "b");
        let mut leaf = open(TREE, 4);
        leaf.update(Message::Expand);
        assert_eq!(focus_text(&leaf), "e");
    }

    // FR-NAV-02
    #[test]
    fn toggle_fold_flips_a_block_and_ignores_leaves() {
        let mut state = open(TREE, 1);
        state.update(Message::ToggleFold);
        assert_eq!(visible(&state)[1], (1, "b".to_owned(), 1));
        state.update(Message::ToggleFold);
        assert_eq!(visible(&state)[1], (1, "b".to_owned(), 0));
        let mut leaf = open(TREE, 4);
        leaf.update(Message::ToggleFold);
        assert_eq!(leaf.folds, HashSet::new());
    }

    // FR-NAV-02
    #[test]
    fn toggle_subtree_collapses_any_expanded_part_else_expands_all() {
        let mut state = open(TREE, 0);
        state.update(Message::ToggleSubtree);
        assert_eq!(state.folds.len(), 2, "A and b, not the leaves");
        state.update(Message::Expand);
        assert_eq!(visible(&state)[1], (1, "b".to_owned(), 1));
        state.update(Message::ToggleSubtree);
        assert_eq!(state.folds.len(), 2);
        state.update(Message::ToggleSubtree);
        assert_eq!(visible(&state).len(), 5);
        assert_eq!(state.folds, HashSet::new());
    }

    // FR-NAV-02
    #[test]
    fn levels_expand_the_shallowest_and_collapse_the_deepest() {
        let mut state = open(TREE, 0);
        state.update(Message::CollapseLevel);
        assert_eq!(visible(&state)[1], (1, "b".to_owned(), 1));
        assert_eq!(visible(&state)[0], (0, "A".to_owned(), 0));
        state.update(Message::CollapseLevel);
        assert_eq!(visible(&state)[0], (0, "A".to_owned(), 2));
        state.update(Message::CollapseLevel);
        assert_eq!(state.folds.len(), 2, "nothing left to collapse");
        state.update(Message::ExpandLevel);
        assert_eq!(visible(&state)[1], (1, "b".to_owned(), 1));
        state.update(Message::ExpandLevel);
        assert_eq!(visible(&state).len(), 5);
        state.update(Message::ExpandLevel);
        assert_eq!(state.folds, HashSet::new());
    }

    // FR-NAV-02
    #[test]
    fn expand_level_skips_collapsed_leaves() {
        let mut state = open("- a\n- b\n\t- c\n\t\t- d", 0);
        state.folds.insert(state.focus().expect("a focus"));
        state.update(Message::FocusNext);
        state.update(Message::FocusNext);
        state.update(Message::ToggleFold);
        assert_eq!(visible(&state)[2], (1, "c".to_owned(), 1));
        state.update(Message::ExpandLevel);
        assert_eq!(visible(&state)[2], (1, "c".to_owned(), 0));
    }

    // FR-NAV-02
    #[test]
    fn expand_all_and_collapse_all() {
        let mut state = open(TREE, 0);
        state.update(Message::CollapseAll);
        assert_eq!(
            visible(&state),
            [(0, "A".to_owned(), 2), (0, "e".to_owned(), 0)]
        );
        assert_eq!(state.folds.len(), 2);
        state.update(Message::ExpandAll);
        assert_eq!(visible(&state).len(), 5);
    }

    // navigation §2
    #[test]
    fn a_hidden_focus_moves_to_the_next_visible_block_else_the_previous() {
        let mut state = open(TREE, 2);
        state.update(Message::CollapseAll);
        assert_eq!(focus_text(&state), "e");
        let mut last = open("A:\n\t- b\n\t\t- c", 2);
        last.update(Message::CollapseAll);
        assert_eq!(focus_text(&last), "A");
    }

    /// Done blocks at two depths, one with a child, among blocks that are not done.
    const DONE: &str = "A:\n\t- b @done\n\t\t- c\n\t- d\nE: @done\n\t- f\n- g @done";

    /// The display text of each visible row.
    fn texts(state: &State) -> Vec<String> {
        state.rows().into_iter().map(|row| row.text).collect()
    }

    // FR-DONE-02
    #[test]
    fn done_blocks_show_until_hidden_with_everything_inside_them() {
        let mut state = open(DONE, 0);
        assert_eq!(texts(&state), ["A", "b", "c", "d", "E", "f", "g"]);
        state.update(Message::ToggleShowDone);
        assert_eq!(texts(&state), ["A", "d"]);
        state.update(Message::ToggleShowDone);
        assert_eq!(texts(&state), ["A", "b", "c", "d", "E", "f", "g"]);
    }

    // FR-DONE-02, navigation §2
    #[test]
    fn hiding_the_focused_done_block_focuses_the_next_visible_else_the_previous() {
        let mut state = open(DONE, 2);
        state.update(Message::ToggleShowDone);
        assert_eq!(focus_text(&state), "d", "from inside a done block");
        let mut last = open(DONE, 6);
        last.update(Message::ToggleShowDone);
        assert_eq!(focus_text(&last), "d");
        let mut zoomed = open("A:\n\t- b @done\n- c", 0);
        zoomed.update(Message::ZoomIn);
        zoomed.update(Message::ToggleShowDone);
        assert_eq!(texts(&zoomed), ["A"], "the title stays");
        assert_eq!(focus_text(&zoomed), "A", "the parent");
    }

    // FR-DONE-02, navigation §2
    #[test]
    fn hiding_every_block_leaves_no_focus_until_one_shows_again() {
        let mut state = open("- a @done\n- b @done", 1);
        state.update(Message::ToggleShowDone);
        assert_eq!(state.rows(), []);
        assert_eq!(state.focus(), None);
        state.update(Message::ToggleShowDone);
        assert_eq!(focus_text(&state), "a", "the first row");
    }

    // FR-DONE-02, FR-NAV-01
    #[test]
    fn child_and_sibling_motions_skip_hidden_done_blocks() {
        let mut state = open("A:\n\t- b @done\n\t- c\n\t- d @done\n- e", 0);
        state.update(Message::ToggleShowDone);
        state.update(Message::Expand);
        assert_eq!(focus_text(&state), "c", "the first child shown");
        state.update(Message::FocusNextSibling);
        assert_eq!(focus_text(&state), "c");
        state.update(Message::FocusPreviousSibling);
        assert_eq!(focus_text(&state), "c");
        state.update(Message::FocusParent);
        state.update(Message::FocusLastChild);
        assert_eq!(focus_text(&state), "c", "the last child shown");
        let mut done = open("A:\n\t- b @done\n- e", 0);
        done.update(Message::ToggleShowDone);
        done.update(Message::Collapse);
        done.update(Message::FocusLastChild);
        assert_eq!(focus_text(&done), "A", "no child shows");
        assert_eq!(done.folds.len(), 1, "nothing to unfold for");
    }

    // FR-NAV-01
    #[test]
    fn parent_and_siblings() {
        let mut state = open(TREE, 2);
        state.update(Message::FocusParent);
        assert_eq!(focus_text(&state), "b");
        state.update(Message::FocusNextSibling);
        assert_eq!(focus_text(&state), "d");
        state.update(Message::FocusNextSibling);
        assert_eq!(focus_text(&state), "d");
        state.update(Message::FocusPreviousSibling);
        assert_eq!(focus_text(&state), "b");
        state.update(Message::FocusPreviousSibling);
        assert_eq!(focus_text(&state), "b");
        state.update(Message::FocusParent);
        state.update(Message::FocusParent);
        assert_eq!(focus_text(&state), "A");
        state.update(Message::FocusNextSibling);
        assert_eq!(focus_text(&state), "e");
    }

    // FR-NAV-01
    #[test]
    fn last_child_expands_a_collapsed_block_and_ignores_leaves() {
        let mut state = open(TREE, 0);
        state.update(Message::Collapse);
        state.update(Message::FocusLastChild);
        assert_eq!(visible(&state).len(), 5);
        assert_eq!(focus_text(&state), "d");
        state.update(Message::FocusLastChild);
        assert_eq!(focus_text(&state), "d", "a leaf has no children");
        let mut zoomed = open(TREE, 0);
        zoomed.update(Message::Collapse);
        zoomed.update(Message::ZoomIn);
        zoomed.update(Message::FocusFirst);
        zoomed.update(Message::FocusLastChild);
        assert_eq!(focus_text(&zoomed), "d", "the title's last child");
        assert_eq!(zoomed.folds.len(), 1, "the title does not unfold");
    }

    // FR-SPC-01
    #[test]
    fn a_block_with_only_special_children_is_a_leaf() {
        let mut state = open("- a\n\tC @hidden\n- b", 0);
        state.update(Message::ToggleSubtree);
        state.update(Message::CollapseAll);
        state.update(Message::Collapse);
        assert_eq!(state.folds, HashSet::new());
        assert_eq!(visible(&state)[0], (0, "a".to_owned(), 0));
    }

    /// The display text of the zoom block, or `Home`.
    fn zoom_text(state: &State) -> String {
        state
            .breadcrumbs()
            .pop()
            .unwrap_or_else(|| "Home".to_owned())
    }

    // FR-NAV-03, FR-FMT-01, FR-FMT-03, D-45
    #[test]
    fn breadcrumbs_show_each_block_without_its_formatting_delimiters() {
        let mut state = open(
            "**Work** `x`:\n\t## [Notes](https://a.b)\n\t\t- [c](https://d.e) `f`\n\t\t\tg",
            2,
        );
        state.update(Message::ZoomIn);
        assert_eq!(state.breadcrumbs(), ["Work x", "Notes", "c f"]);
    }

    // FR-NAV-03
    #[test]
    fn zoom_in_shows_a_title_and_focuses_the_first_child() {
        let mut state = open(TREE, 1);
        state.update(Message::ZoomIn);
        assert_eq!(state.zoom(), state.rows().first().map(|row| row.id));
        assert_eq!(state.breadcrumbs(), ["A", "b"]);
        assert_eq!(
            visible(&state),
            [(0, "b".to_owned(), 0), (1, "c".to_owned(), 0)]
        );
        assert_eq!(
            state.rows().iter().map(|row| row.title).collect::<Vec<_>>(),
            [true, false]
        );
        assert_eq!(focus_text(&state), "c");
        state.update(Message::ZoomIn);
        assert_eq!(zoom_text(&state), "c");
        assert_eq!(focus_text(&state), "c", "a leaf focuses its title");
        state.update(Message::ZoomIn);
        assert_eq!(zoom_text(&state), "c", "the title does not zoom");
    }

    // FR-NAV-03
    #[test]
    fn zoom_out_and_home_focus_the_block_left() {
        let mut state = open(TREE, 2);
        state.update(Message::ZoomIn);
        state.update(Message::ZoomOut);
        assert_eq!(zoom_text(&state), "b");
        assert_eq!(focus_text(&state), "c");
        state.update(Message::ZoomOut);
        state.update(Message::ZoomOut);
        assert_eq!(zoom_text(&state), "Home");
        assert_eq!(focus_text(&state), "A");
        state.update(Message::ZoomOut);
        assert_eq!(zoom_text(&state), "Home");
        state.update(Message::FocusNext);
        state.update(Message::ZoomIn);
        state.update(Message::GoHome);
        assert_eq!(focus_text(&state), "b");
    }

    // FR-NAV-03
    #[test]
    fn going_to_a_view_whose_block_left_is_hidden_focuses_its_first_block() {
        let mut state = open(TREE, 1);
        let b = state.focus().expect("a focus");
        state.update(Message::ZoomIn);
        state.update(Message::ZoomIn);
        assert_eq!(zoom_text(&state), "c");
        state.folds.insert(b);
        state.update(Message::GoHome);
        assert_eq!(
            focus_text(&state),
            "A",
            "c is hidden inside the collapsed b"
        );
    }

    // FR-NAV-04
    #[test]
    fn back_and_forward_walk_the_history_and_a_new_zoom_drops_the_rest() {
        let mut state = open(TREE, 1);
        state.update(Message::ZoomIn);
        state.update(Message::ZoomIn);
        state.update(Message::Back);
        assert_eq!(zoom_text(&state), "b");
        state.update(Message::Back);
        assert_eq!(zoom_text(&state), "Home");
        state.update(Message::Back);
        assert_eq!(zoom_text(&state), "Home");
        state.update(Message::Forward);
        state.update(Message::Forward);
        assert_eq!(zoom_text(&state), "c");
        state.update(Message::Forward);
        assert_eq!(zoom_text(&state), "c");
        state.update(Message::Back);
        state.update(Message::GoHome);
        assert_eq!(state.pane.history.len(), 3);
        state.update(Message::Forward);
        assert_eq!(zoom_text(&state), "Home");
        state.update(Message::GoHome);
        assert_eq!(
            state.pane.history.len(),
            3,
            "staying put is not a new entry"
        );
    }

    // FR-NAV-02, FR-NAV-03
    #[test]
    fn the_title_does_not_fold_or_move_to_its_parent_or_siblings() {
        let mut state = open(TREE, 1);
        state.update(Message::ZoomIn);
        state.update(Message::FocusPrevious);
        assert_eq!(focus_text(&state), "b");
        for message in [
            Message::Collapse,
            Message::ToggleFold,
            Message::FocusParent,
            Message::FocusNextSibling,
            Message::FocusPreviousSibling,
        ] {
            state.update(message.clone());
            assert_eq!(focus_text(&state), "b", "{message:?}");
            assert_eq!(state.folds, HashSet::new(), "{message:?}");
        }
        state.update(Message::Expand);
        assert_eq!(focus_text(&state), "c");
        state.update(Message::FocusParent);
        assert_eq!(focus_text(&state), "b", "the title is the parent");
    }

    // FR-NAV-02, FR-NAV-03
    #[test]
    fn folding_under_a_zoom_stays_under_it() {
        let mut state = open("A:\n\t- b\n\t\t- c\nD:\n\t- e", 0);
        state.update(Message::ZoomIn);
        state.update(Message::CollapseAll);
        assert_eq!(state.folds.len(), 1, "only b");
        state.update(Message::FocusPrevious);
        state.update(Message::ToggleSubtree);
        assert_eq!(
            state.folds,
            HashSet::new(),
            "the title expands what is below it"
        );
        state.update(Message::ToggleSubtree);
        assert_eq!(state.folds.len(), 1);
        state.update(Message::GoHome);
        state.update(Message::FocusLast);
        state.update(Message::Collapse);
        state.update(Message::Collapse);
        state.update(Message::FocusFirst);
        state.update(Message::ZoomIn);
        state.update(Message::ExpandAll);
        assert_eq!(state.folds.len(), 1, "D stays collapsed");
    }

    // FR-NAV-03
    #[test]
    fn a_collapsed_block_shows_its_children_when_zoomed() {
        let mut state = open(TREE, 0);
        state.update(Message::Collapse);
        state.update(Message::ZoomIn);
        assert_eq!(visible(&state)[0], (0, "A".to_owned(), 0));
        assert_eq!(visible(&state).len(), 4);
    }

    #[test]
    fn motions_in_an_empty_document_keep_no_focus() {
        let mut state = State::new(Document::default(), None);
        for message in [
            Message::FocusNext,
            Message::FocusPrevious,
            Message::FocusFirst,
            Message::FocusLast,
            Message::PageDown { rows: 5 },
            Message::PageUp { rows: 5 },
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
            Message::ZoomIn,
            Message::ZoomOut,
            Message::GoHome,
            Message::Back,
            Message::Forward,
        ] {
            assert_eq!(state.update(message.clone()), [], "{message:?}");
            assert_eq!(state.focus(), None, "{message:?}");
        }
    }
}
