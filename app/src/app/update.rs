//! The update function: messages are user intents and reports from the frontend, effects are
//! what the frontend must do.

use std::{collections::HashSet, path::PathBuf, time::Duration};

use jiff::civil::Date;

use super::{
    AttachmentsInput, Command, ConflictInput, DateInput, Location, MenuInput, NavigatorInput,
    NodeInput, Notice, PaneChange, Paste, QuickAddInput, Row, SaveAs, SelectionChange,
    SettingValue, SettingsInput, SpecialView, State, Symbols, TextEdit, Version,
    pane::{Pane, landing},
    rows::Rows,
    saving::{DocumentText, Ending},
    structure::NewBlock,
};
use crate::document::{
    Attachment, Block, DateChange, DateTag, Flag, Item, ItemId, TagColor, blocks, format::Typed,
    structure::Shift,
};

/// A setting of the frontend's settings file that the core carries out, as the file or the
/// settings window says (FR-SET-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Configuration {
    /// Save this long after the last change from now on (`autosave_delay_ms`, file-io.md §2).
    AutosaveDelay(Duration),
    /// List this special view in the Navigator, or not when `false` (`sidebar_upcoming` and so
    /// on, navigation.md §6, D-80); Today is always listed (D-102).
    NavigatorView(SpecialView, bool),
    /// List starred blocks in the Today view besides dated ones, or only dated ones when `false`
    /// (`today_starred`, navigation.md §7, D-88).
    TodayStarred(bool),
    /// Turn this kind of attachment on, or off when `false` (`attachments_notes`,
    /// `attachments_comments`, attachments.md §6).
    Attachments(Attachment, bool),
}

/// Which sibling of the focused block a sibling motion focuses, among the blocks shown
/// (FR-NAV-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sibling {
    /// The next one.
    Next,
    /// The previous one.
    Previous,
    /// The first one.
    First,
    /// The last one.
    Last,
}

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
    Saved {
        /// The version of the file it wrote.
        version: Version,
    },
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
    /// A key of the conflict prompt (file-io.md §3).
    Conflict(ConflictInput),
    /// The [`Disk::ConflictCopy`] under way was written.
    ConflictCopied {
        /// Where the copy is.
        copy: PathBuf,
    },
    /// The [`Disk::ConflictCopy`] under way failed.
    ConflictCopyFailed {
        /// The backups folder, when writing in it failed.
        folder: Option<PathBuf>,
        /// Why, for the status line.
        reason: String,
    },
    /// A [`Disk::Backup`] failed.
    BackupFailed {
        /// Why, for the status line.
        reason: String,
    },
    /// The frontend's settings file has these problems: at start, the settings it ignored; later,
    /// a setting changed in the app that it could not write, which holds for the session
    /// (FR-SET-04).
    ConfigProblems(Vec<String>),
    /// Apply this setting of the frontend's that the core carries out (FR-SET-01).
    Configure(Configuration),
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
    /// Move focus down by a page of `rows` rows, as many as the frontend's view holds below the
    /// focus.
    PageDown {
        /// Rows in a page.
        rows: usize,
    },
    /// Move focus up by a page of `rows` rows, as many as the frontend's view holds above the
    /// focus.
    PageUp {
        /// Rows in a page.
        rows: usize,
    },
    /// Focus the block `id` in the focused pane, if one of its rows shows it, as a click on it
    /// does (FR-KEY-05).
    Focus(ItemId),
    /// Focus the parent block.
    FocusParent,
    /// Focus a sibling of the focused block.
    FocusSibling(Sibling),
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
    /// Show the focused block as the root; in a special view, go to the focused entry in the
    /// outline (D-7, D-80).
    ZoomIn,
    /// Show the zoom block's parent as the root, focusing the block zoomed out of.
    ZoomOut,
    /// Show `location` in the focused pane as a step of its history: the whole document with
    /// `Location::Outline(None)`, or a special view, as choosing it in the Navigator does (D-64,
    /// D-80).
    GoTo(Location),
    /// Show the focused pane's breadcrumb at `depth` as its root: 0 is the whole document, 1 the
    /// zoom block's top-level ancestor, and so on down to the zoom block (FR-KEY-05).
    GoToCrumb(usize),
    /// Return to the previous zoom in the history.
    Back,
    /// Go to the next zoom in the history.
    Forward,
    /// Close the Navigator while it is open, else open it, giving it the keys (FR-UI-01, D-68,
    /// D-79).
    ToggleNavigator,
    /// A key of the open Navigator (FR-UI-01).
    Navigator(NavigatorInput),
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
    /// Add the bare tag `@star` or `@bookmark` to the selected blocks, else the focused block, or
    /// remove it when all have it (FR-TAG-04, D-11, FR-OPS-07).
    ToggleFlag(Flag),
    /// Give the selected blocks, else the focused block, the color after the focused block's own:
    /// none, red, yellow, green, blue and back to none (FR-TAG-06, D-26, D-138).
    CycleColor,
    /// Give the selected blocks, else the focused block, `color`, or no color when `None`
    /// (FR-TAG-05, FR-OPS-07, D-48).
    SetColor(Option<TagColor>),
    /// Open the color picker on the focused block, for it or the selected blocks (FR-TAG-05).
    OpenColorPicker,
    /// Make the selected text blocks, else the focused block, sections by adding `@section`, or
    /// with `Some(false)` plain text by removing it, or with `None` plain text when every one is
    /// a section and sections otherwise; a task or project is left as it is, with a notice
    /// (FR-BLK-04, FR-OPS-07, D-52, D-130).
    SetSection(Option<bool>),
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
    /// Open Quick Add, or a key of its open panel (`q`, D-96, D-133).
    QuickAdd(QuickAddInput),
    /// Remove the `tag` date of the selected blocks, else the focused block (FR-TAG-03, D-50).
    ClearDate(DateTag),
    /// Open the type picker on the focused block (FR-BLK-08).
    OpenTypePicker,
    /// Open the Edit Node dialog on the focused block, or a key of the open dialog
    /// (FR-MODE-09).
    NodeEditor(NodeInput),
    /// Open the attachments panel on the focused block, or a key of the open panel (FR-ATT-03).
    Attachments(AttachmentsInput),
    /// Open Go to Symbol over these blocks: every one (FR-NAV-05), or the projects, as Go to
    /// Project (D-65).
    OpenGoToSymbol(Symbols),
    /// Open Move to over the selected blocks, else the focused block (FR-OPS-05).
    OpenMoveTo,
    /// Open Select Inbox Project over the projects (D-141).
    OpenInboxPicker,
    /// Take `@inbox` from every project (D-141).
    ResetInbox,
    /// Open Go to Command over the commands of navigate mode, each shown with its bindings as
    /// the frontend writes them, for the commands it binds (FR-NAV-06).
    OpenGoToCommand(Vec<(Command, String)>),
    /// Open the theme picker over the frontend's themes, by name, the one at `current`
    /// highlighted (FR-UI-06).
    OpenThemePicker {
        /// The themes' names, in the frontend's order.
        names: Vec<String>,
        /// The position in `names` of the theme in use.
        current: usize,
    },
    /// Open the settings window over the frontend's settings, or a key of it (FR-SET-02,
    /// ui.md §8).
    Settings(SettingsInput),
    /// Show how many blocks and words are under the focused block (FR-OPS-09).
    Count,
    /// Open the context menu on the focused Navigator entry while the Navigator is open, else
    /// on the selection or the focused block (FR-KEY-06).
    OpenContextMenu,
    /// Open the context menu on the focused pane's header, or with a depth on its breadcrumb at
    /// that depth, as [`Message::GoToCrumb`] counts it (FR-KEY-06).
    OpenPaneMenu(Option<usize>),
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
    /// first position into the parent's previous sibling (FR-OPS-03); in the Today view, up its
    /// list without changing the document (D-57).
    MoveUp,
    /// Move the selected blocks, else the focused block, below their next siblings, or at the
    /// last position into the parent's next sibling (FR-OPS-03); in the Today view, down its list
    /// without changing the document (D-57).
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
    /// The frontend was asked to start the weather in summer, which has none (FR-UI-12).
    NoWeather,
    /// Change which blocks are selected (FR-OPS-01).
    Select(SelectionChange),
    /// Open, focus or close panes (FR-UI-02, FR-UI-03).
    Pane(PaneChange),
    /// Keep this width for the focused pane, which the frontend chose when narrowing or widening
    /// it (FR-UI-03).
    ResizePane(u16),
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
    /// Select Inbox Project (D-141).
    InboxPicker,
    /// Quick Add's project picker (D-141).
    QuickAdd,
    /// Go to Command (FR-NAV-06).
    GoToCommand,
    /// The command menu of edit mode (D-50).
    CommandMenu,
    /// The context menu (FR-KEY-06).
    ContextMenu,
    /// The theme picker (FR-UI-06).
    ThemePicker,
    /// The Edit Node dialog's parent finder (FR-MODE-09).
    NodeEditor,
}

impl Message {
    /// Whether this intent belongs to navigate mode, so that it ends an edit session first,
    /// rather than to edit mode or the frontend's reports, which leave the session alone, as do
    /// the conflict prompt's choices: keeping mine changes nothing being edited, and taking
    /// theirs reloads, which ends the session only when its block is gone (D-104, D-123).
    fn ends_editing(&self) -> bool {
        match self {
            Self::Started
            | Self::Tick { .. }
            | Self::Saved { .. }
            | Self::SaveFailed { .. }
            | Self::SaveRefused { .. }
            | Self::CheckFile
            | Self::FileChecked { .. }
            | Self::Conflict(_)
            | Self::ConflictCopied { .. }
            | Self::ConflictCopyFailed { .. }
            | Self::BackupFailed { .. }
            | Self::ConfigProblems(_)
            | Self::Configure(_)
            | Self::OpenUrlFailed { .. }
            | Self::NoWeather
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
            | Self::Pasted { .. }
            | Self::ResizePane(_) => false,
            Self::Quit
            | Self::Terminate
            | Self::FocusNext
            | Self::FocusPrevious
            | Self::FocusFirst
            | Self::FocusLast
            | Self::PageDown { .. }
            | Self::PageUp { .. }
            | Self::Focus(_)
            | Self::FocusParent
            | Self::FocusSibling(_)
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
            | Self::GoTo(_)
            | Self::GoToCrumb(_)
            | Self::Back
            | Self::Forward
            | Self::ToggleNavigator
            | Self::Navigator(_)
            | Self::ToggleDone { .. }
            | Self::ChangeDate { .. }
            | Self::ToggleFlag(_)
            | Self::CycleColor
            | Self::SetColor(_)
            | Self::SetSection(_)
            | Self::OpenColorPicker
            | Self::OpenDatePrompt { .. }
            | Self::QuickAdd(_)
            | Self::ClearDate(_)
            | Self::OpenTypePicker
            | Self::NodeEditor(_)
            | Self::Attachments(_)
            | Self::OpenGoToSymbol(_)
            | Self::OpenMoveTo
            | Self::OpenInboxPicker
            | Self::ResetInbox
            | Self::OpenGoToCommand(_)
            | Self::OpenThemePicker { .. }
            | Self::Settings(_)
            | Self::Count
            | Self::SaveAs(_)
            | Self::OpenContextMenu
            | Self::OpenPaneMenu(_)
            | Self::New(..)
            | Self::Delete
            | Self::Duplicate
            | Self::MoveUp
            | Self::MoveDown
            | Self::OpenUrl
            | Self::Select(_)
            | Self::Group
            | Self::Pane(_) => true,
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
            | Self::InboxPicker
            | Self::QuickAdd
            | Self::GoToCommand
            | Self::ContextMenu
            | Self::ThemePicker
            | Self::NodeEditor => true,
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
    /// Use the frontend's theme at this position in the names it gave the theme picker from now
    /// on, and keep it in its settings (FR-UI-06, FR-SET-04).
    SetTheme(usize),
    /// Apply this value of the setting at this position in the settings the frontend gave the
    /// settings window, and keep it in its settings (FR-SET-02, FR-SET-04).
    ChangeSetting {
        /// The setting's position.
        at: usize,
        /// Its new value.
        value: SettingValue,
    },
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
        /// The document to write.
        text: DocumentText,
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
        /// The document to write.
        text: DocumentText,
        /// The version the file must still be, or `None` when there must be no file.
        expected: Option<Version>,
    },
    /// Write `contents` to a new conflict copy in the backups folder of the file at `path`, even
    /// when backups are off, or, when that fails while `ending`, to a recovery file; the result
    /// comes back as [`Message::ConflictCopied`] or [`Message::ConflictCopyFailed`]
    /// (file-io.md §3).
    ConflictCopy {
        /// The open file.
        path: PathBuf,
        /// The version a conflict choice discards.
        contents: Vec<u8>,
        /// Whether the session ends once the copy is written, leaving the copy, or the recovery
        /// file when the copy fails, the only place `contents`, the local version, is kept.
        ending: bool,
    },
}

impl State {
    /// Applies `message` and returns the effects the frontend must carry out.
    pub fn update(&mut self, message: Message) -> Vec<Effect> {
        self.changed.clear();
        self.refused = false;
        let Some(message) = self.as_located(message) else {
            return Vec::new();
        };
        if message.ends_editing() {
            self.stop_editing();
        }
        let landmarks = self.landmarks();
        let stamp = self.document.stamp();
        let copies = message == Message::Copy;
        let mut effects = self.dispatch(message);
        // A command acted on the selection: it stays selected, but the focus moves alone again
        // (D-135).
        if copies || self.document.stamp() != stamp {
            self.pane.selection.end_visual();
        }
        self.repair_panes(&landmarks);
        self.expire_notice();
        effects.extend(self.save_if_due());
        let touched = self.document.take_touched();
        self.changed.extend(
            touched
                .into_iter()
                .filter(|&id| self.document.get(id).is_some()),
        );
        effects
    }

    /// Asks to end the session as `ending` says, saving the note being edited first.
    fn end_session(&mut self, ending: Ending) {
        self.save_note();
        self.autosave.end(ending);
    }

    /// Carries out `setting` of the frontend's.
    fn configure(&mut self, setting: Configuration) {
        match setting {
            Configuration::AutosaveDelay(delay) => self.set_autosave_delay(delay),
            Configuration::NavigatorView(view, on) => self.navigator.set_listed(view, on),
            Configuration::TodayStarred(on) => self.today_starred = on,
            Configuration::Attachments(kind, on) => self.show_attachments(kind, on),
        }
    }

    /// Hands `message` to the feature that handles it and returns the effects that asks for.
    fn dispatch(&mut self, message: Message) -> Vec<Effect> {
        let mut effects = Vec::new();
        match message {
            Message::Started => effects = [self.backup_on_open(), self.watch()].concat(),
            Message::Tick { now, today } => self.set_time(now, today),
            Message::Saved { version } => self.saved(version),
            Message::SaveFailed { reason } => self.save_failed(reason),
            Message::SaveRefused { contents } => self.save_refused(contents),
            Message::CheckFile => effects = self.check_file(),
            Message::FileChecked { contents } => self.found(contents),
            Message::Conflict(input) => effects = self.conflict_input(input),
            Message::ConflictCopied { copy } => self.conflict_copied(copy),
            Message::ConflictCopyFailed { folder, reason } => self.copy_failed(folder, reason),
            Message::BackupFailed { reason } => self.backup_failed(reason),
            Message::ConfigProblems(problems) => self.config_problems(problems),
            Message::Configure(setting) => self.configure(setting),
            Message::Quit => self.end_session(Ending::Quit),
            Message::Terminate => self.end_session(Ending::Terminate),
            Message::FocusNext => self.focus_down(1),
            Message::FocusPrevious => self.focus_up(1),
            Message::FocusFirst => self.move_focus(|_, _| 0),
            Message::FocusLast => self.move_focus(|_, last| last),
            Message::PageDown { rows } => self.focus_down(rows),
            Message::PageUp { rows } => self.focus_up(rows),
            Message::Focus(id) => self.focus_block(id),
            Message::FocusParent => self.focus_parent(),
            Message::FocusSibling(to) => self.move_to_sibling(to),
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
            Message::GoTo(location) => self.go_to(location),
            Message::GoToCrumb(depth) => self.go_to_crumb(depth),
            Message::Back | Message::Forward => self.walk_history(message == Message::Forward),
            Message::ToggleNavigator => self.toggle_navigator(),
            Message::Navigator(input) => self.navigator_input(input),
            Message::ToggleDone { today } => self.toggle_done(today),
            Message::ChangeDate { tag, change, today } => self.change_date(tag, change, today),
            Message::ToggleFlag(flag) => self.toggle_flag(flag),
            Message::CycleColor => self.cycle_color(),
            Message::SetColor(color) => self.set_color(color),
            Message::SetSection(on) => self.set_section(on),
            Message::OpenColorPicker => self.open_color_picker(),
            Message::OpenDatePrompt { tag, today } => self.open_date_prompt(tag, today),
            Message::DatePrompt(input) => self.date_prompt_input(input),
            Message::QuickAdd(input) => self.quick_add_input(input),
            Message::ClearDate(tag) => self.clear_date(tag),
            Message::OpenTypePicker => self.open_type_picker(),
            Message::NodeEditor(input) => self.node_editor_input(input),
            Message::Attachments(input) => self.attachments_input(input),
            Message::OpenGoToSymbol(listed) => self.open_go_to_symbol(listed),
            Message::OpenMoveTo => self.open_move_to(),
            Message::OpenInboxPicker => self.open_inbox_picker(),
            Message::ResetInbox => self.reset_inbox(),
            Message::OpenGoToCommand(bindings) => self.open_go_to_command(&bindings),
            Message::OpenThemePicker { names, current } => self.open_theme_picker(names, current),
            Message::Settings(input) => effects.extend(self.settings_input(input)),
            Message::Count => self.count(),
            Message::OpenContextMenu => self.open_context_menu(),
            Message::OpenPaneMenu(crumb) => self.open_pane_menu(crumb),
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
            Message::MoveUp | Message::MoveDown => self.move_blocks(message == Message::MoveDown),
            Message::Select(change) => self.change_selection(change),
            Message::Group => self.group(),
            Message::OpenUrl => effects = self.open_url(),
            Message::OpenUrlFailed { url, reason } => self.open_url_failed(url, reason),
            Message::NoWeather => self.notify(Notice::NoWeather),
            Message::Pane(change) => self.change_panes(change),
            Message::ResizePane(width) => self.resize_pane(width),
        }
        effects
    }

    /// Takes `now`, the time since the frontend started, and `today`, its local date.
    fn set_time(&mut self, now: Duration, today: Date) {
        self.now = now;
        self.today = today;
    }

    /// Hands a key of the open `menu` to the feature that owns it and returns the effects that
    /// asks for.
    fn menu_input(&mut self, menu: Menu, input: MenuInput) -> Vec<Effect> {
        match menu {
            Menu::ColorPicker => self.color_picker_input(input),
            Menu::TypePicker => self.type_picker_input(input),
            Menu::GoToSymbol => self.go_to_symbol_input(input),
            Menu::MoveTo => self.move_to_input(input),
            Menu::InboxPicker => self.inbox_picker_input(input),
            Menu::QuickAdd => self.quick_add_picker_input(input),
            Menu::GoToCommand => return self.go_to_command_input(input).into_iter().collect(),
            Menu::CommandMenu => return self.command_menu_input(input),
            Menu::ContextMenu => return self.context_menu_input(input).into_iter().collect(),
            Menu::ThemePicker => return self.theme_picker_input(input).into_iter().collect(),
            Menu::NodeEditor => self.node_editor_finder_input(input),
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

    /// Zooms into the focused block (FR-NAV-03), or in a special view goes to it in the outline
    /// (D-7, D-80).
    fn zoom_in(&mut self) {
        let Some(block) = self.focused_block() else {
            return;
        };
        match self.pane.location {
            Location::Outline(_) => self.go_to(Location::Outline(Some(block.id()))),
            Location::View(_) => self.go_to_entry(&block),
        }
    }

    /// Moves the selected blocks, else the focused block, a step up or down with `down`: in the
    /// outline with their subtrees (FR-OPS-03), in a special view only in its order (D-57, D-80).
    fn move_blocks(&mut self, down: bool) {
        match self.pane.location {
            Location::Outline(_) => self.shift_selected(if down { Shift::Down } else { Shift::Up }),
            // Today is the only view that takes a move (`as_located`).
            Location::View(_) => self.reorder_entries(down),
        }
    }

    /// Zooms out to the parent of the zoomed block (FR-NAV-03).
    fn zoom_out(&mut self) {
        if let Some(zoom) = self.zoom() {
            let parent = blocks::parent(self.document(), zoom).map(|block| block.id());
            self.go_to(Location::Outline(parent));
        }
    }

    /// Where the focused pane's breadcrumb at `depth` goes, as [`Message::GoToCrumb`] counts
    /// it, if the pane has one there.
    pub(super) fn crumb(&self, depth: usize) -> Option<Location> {
        let Some(above) = depth.checked_sub(1) else {
            return Some(Location::Outline(None));
        };
        let mut line: Vec<ItemId> = std::iter::successors(
            self.zoom()
                .and_then(|id| blocks::block(self.document(), id)),
            |block| blocks::parent(self.document(), block.id()),
        )
        .map(|block| block.id())
        .collect();
        line.reverse();
        line.get(above).map(|&id| Location::Outline(Some(id)))
    }

    /// Shows the focused pane's breadcrumb at `depth`, as [`Message::GoToCrumb`] counts it, if
    /// it has one there.
    fn go_to_crumb(&mut self, depth: usize) {
        if let Some(location) = self.crumb(depth) {
            self.go_to(location);
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

    /// Shows the next history entry, or with `forward` false the previous one, skipping those
    /// whose zoom block no longer exists (FR-NAV-04) and those showing where the pane already is,
    /// which skipping can bring next to each other, so every step goes somewhere.
    fn walk_history(&mut self, forward: bool) {
        let mut at = self.pane.history_at;
        while let Some(next) = if forward {
            at.checked_add(1)
        } else {
            at.checked_sub(1)
        } && let Some(&location) = self.pane.history.get(next)
        {
            at = next;
            if location != self.pane.location
                && location
                    .zoom()
                    .is_none_or(|id| blocks::block(self.document(), id).is_some())
            {
                self.pane.history_at = at;
                self.show(location);
                return;
            }
        }
    }

    /// Shows `location` and focuses the first visible block left: the block zoomed out of, or
    /// when entering a special view from the outline the focused block before it (D-114), or the
    /// focused entry when leaving a special view; else the first block under the title, else the
    /// title.
    fn show(&mut self, location: Location) {
        let left = match (self.pane.location, location) {
            (Location::Outline(zoom), Location::View(_)) => [self.pane.focus, zoom],
            (Location::Outline(zoom), Location::Outline(_)) => [zoom, None],
            (Location::View(_), _) => [self.pane.focus, None],
        };
        self.pane.location = location;
        let rows = self.rows();
        self.pane.focus = left
            .into_iter()
            .flatten()
            .find(|&id| rows.iter().any(|row| row.id == id))
            .or_else(|| landing(&rows));
    }

    /// Focuses the block `id` when one of the focused pane's rows shows it.
    fn focus_block(&mut self, id: ItemId) {
        if self.rows().iter().any(|row| row.id == id) {
            self.pane.focus = Some(id);
        }
    }

    fn focus_parent(&mut self) {
        if let Some(parent) = self
            .focused_block()
            .and_then(|block| blocks::parent(self.document(), block.id()))
        {
            self.pane.focus = Some(parent.id());
        }
    }

    /// Focuses the focused block's sibling `to` among those shown, if there is one.
    fn move_to_sibling(&mut self, to: Sibling) {
        let Some(id) = self.focused_block().map(|block| block.id()) else {
            return;
        };
        let mut siblings = self.siblings(id);
        siblings.retain(|sibling| self.shows(sibling));
        if let Some(at) = siblings.iter().position(|block| block.id() == id)
            && let Some(sibling) = match to {
                Sibling::Next => at.checked_add(1),
                Sibling::Previous => at.checked_sub(1),
                Sibling::First => Some(0),
                Sibling::Last => siblings.len().checked_sub(1),
            }
            .and_then(|index| siblings.get(index))
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
                self.folds.remove(block.id());
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
        if (title || !self.folds.remove(block.id()))
            && let Some(child) = self.shown_children(&block).first()
        {
            self.pane.focus = Some(child.id());
        }
    }

    fn toggle_fold(&mut self) {
        let Some(block) = self.focused_block() else {
            return;
        };
        if !self.folds.remove(block.id()) {
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
                self.folds.remove(block.id());
            }
        }
    }

    /// Appends every descendant block of `block`, collapsed or not, to `out`, without taking
    /// stack for each level, as an outline can be thousands of levels deep (N-REL-01).
    fn push_descendants(&self, block: &Block, out: &mut Vec<Block>) {
        let mut below = self.children(block);
        while let Some(block) = below.pop() {
            below.extend(self.children(&block));
            out.push(block);
        }
    }

    fn expand_level(&mut self) {
        let rows = self.below_zoom();
        let collapsed: Vec<(ItemId, usize)> = parents(&rows)
            .filter(|row| self.folds.contains(&row.id))
            .map(|row| (row.id, row.depth))
            .collect();
        let Some(depth) = collapsed.iter().map(|&(_, depth)| depth).min() else {
            return;
        };
        for (id, _) in collapsed.into_iter().filter(|&(_, d)| d == depth) {
            self.folds.remove(id);
        }
    }

    fn collapse_level(&mut self) {
        let rows = self.below_zoom();
        // The depth of the collapsed or hidden block whose rows are being passed.
        let mut passing = None;
        let mut expanded = Vec::new();
        for (row, parent) in with_children(&rows).filter(|(row, _)| !row.title) {
            if passing.is_some_and(|depth| row.depth > depth) {
                continue;
            }
            let hidden = row.done && !self.pane.show_done;
            passing = (hidden || self.folds.contains(&row.id)).then_some(row.depth);
            if passing.is_none() && parent {
                expanded.push((row.id, row.depth));
            }
        }
        let Some(depth) = expanded.iter().map(|&(_, depth)| depth).max() else {
            return;
        };
        let deepest = expanded.into_iter().filter(|&(_, d)| d == depth);
        self.folds.extend(deepest.map(|(id, _)| id));
    }

    fn expand_all(&mut self) {
        for row in &self.below_zoom() {
            if !row.title {
                self.folds.remove(row.id);
            }
        }
    }

    fn collapse_all(&mut self) {
        let rows = self.below_zoom();
        self.folds.extend(parents(&rows).map(|row| row.id));
    }

    /// Every block below the pane's zoom, collapsed or not, done or not, each followed by those
    /// inside it a level deeper, with the zoom block's title row first when zoomed: kept from one
    /// change to the next, unlike a walk over a large outline's lines (N-PERF-02).
    fn below_zoom(&self) -> Rows {
        self.unfolded_rows(Location::Outline(self.pane.zoom()), true)
    }

    /// Whether `block` has children and is not collapsed.
    pub(super) fn is_expanded(&self, block: &Block) -> bool {
        !self.folds.contains(&block.id()) && self.has_children(block)
    }

    /// Collapses each of `blocks` that has children; a leaf has nothing to fold.
    fn fold(&mut self, blocks: impl IntoIterator<Item = Block>) {
        for block in blocks {
            if self.has_children(&block) {
                self.folds.insert(block.id());
            }
        }
    }

    /// Moves `pane`'s focus off a block that no longer shows: to the next of the rows `before`
    /// the change that still shows, else the previous one, else the first row, the title
    /// (navigation §2). A block no row showed, as one inside a fold, goes by document order
    /// instead. With no focus, as when every block was hidden, focuses the first row if one is
    /// back.
    pub(super) fn repair_focus(&self, pane: &mut Pane, before: Option<&Rows>) {
        let rows = self.rows_of(pane);
        let Some(focus) = pane.focus else {
            pane.focus = rows.first().map(|row| row.id);
            return;
        };
        if rows.iter().any(|row| row.id == focus) {
            return;
        }
        let visible: HashSet<ItemId> = rows.iter().map(|row| row.id).collect();
        let before: Vec<ItemId> = before.into_iter().flatten().map(|row| row.id).collect();
        // The rows give the order a special view shows its entries in, which may not be the
        // document's (D-57).
        let order: Vec<_> = if before.contains(&focus) || self.document.get(focus).is_none() {
            before
        } else {
            self.blocks_of(pane)
                .into_iter()
                .map(|(block, _)| block.id())
                .collect()
        };
        let at = order.iter().position(|&id| id == focus).unwrap_or_default();
        pane.focus = order
            .iter()
            .skip(at)
            .chain(order.iter().take(at).rev())
            .find(|id| visible.contains(id))
            .or(rows.first().map(|row| &row.id))
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

/// Each of `rows` with whether the row after it is deeper: in rows with nothing collapsed, whether
/// its block has children.
fn with_children(rows: &Rows) -> impl Iterator<Item = (&Row, bool)> {
    let mut after = rows.iter().skip(1);
    rows.iter().map(move |row| {
        let parent = after.next().is_some_and(|next| next.depth > row.depth);
        (row, parent)
    })
}

/// The rows of blocks with children among `rows`, which have nothing collapsed, but the zoom
/// block's title row.
fn parents(rows: &Rows) -> impl Iterator<Item = &Row> {
    with_children(rows)
        .filter(|&(row, parent)| parent && !row.title)
        .map(|(row, _)| row)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Notice;
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

    /// Each visible row as its depth, display text and count of hidden open tasks.
    fn visible(state: &State) -> Vec<(usize, String, Option<usize>)> {
        state
            .rows()
            .iter()
            .map(|row| (row.depth, row.text.to_string(), row.hidden_tasks))
            .collect()
    }

    /// The display text of the focused row.
    fn focus_text(state: &State) -> String {
        let rows = state.rows();
        let row = rows.iter().find(|row| Some(row.id) == state.focus());
        row.map(|row| row.text.to_string()).unwrap_or_default()
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
    fn collapse_hides_children_and_counts_their_open_tasks() {
        let mut state = open(TREE, 0);
        state.update(Message::Collapse);
        assert_eq!(
            visible(&state),
            [(0, "A".to_owned(), Some(3)), (0, "e".to_owned(), None)]
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
        assert_eq!(visible(&state)[1], (1, "b".to_owned(), Some(1)));
        state.update(Message::ToggleFold);
        assert_eq!(visible(&state)[1], (1, "b".to_owned(), None));
        let mut leaf = open(TREE, 4);
        leaf.update(Message::ToggleFold);
        assert_eq!(*leaf.folds, HashSet::new());
    }

    // FR-NAV-02, N-REL-01
    #[test]
    fn toggle_subtree_reaches_every_level_of_an_outline_deeper_than_a_stack_could_take() {
        let levels = 100_000;
        let mut state = State::new(Document::chain(levels), None);
        state.update(Message::ToggleSubtree);
        assert_eq!(state.folds.len(), levels - 1, "every block but the leaf");
        state.update(Message::ToggleSubtree);
        assert_eq!(state.rows().len(), levels);
    }

    // FR-NAV-02
    #[test]
    fn toggle_subtree_collapses_any_expanded_part_else_expands_all() {
        let mut state = open(TREE, 0);
        state.update(Message::ToggleSubtree);
        assert_eq!(state.folds.len(), 2, "A and b, not the leaves");
        state.update(Message::Expand);
        assert_eq!(visible(&state)[1], (1, "b".to_owned(), Some(1)));
        state.update(Message::ToggleSubtree);
        assert_eq!(state.folds.len(), 2);
        state.update(Message::ToggleSubtree);
        assert_eq!(visible(&state).len(), 5);
        assert_eq!(*state.folds, HashSet::new());
    }

    // FR-NAV-02
    #[test]
    fn levels_expand_the_shallowest_and_collapse_the_deepest() {
        let mut state = open(TREE, 0);
        state.update(Message::CollapseLevel);
        assert_eq!(visible(&state)[1], (1, "b".to_owned(), Some(1)));
        assert_eq!(visible(&state)[0], (0, "A".to_owned(), None));
        state.update(Message::CollapseLevel);
        assert_eq!(visible(&state)[0], (0, "A".to_owned(), Some(3)));
        state.update(Message::CollapseLevel);
        assert_eq!(state.folds.len(), 2, "nothing left to collapse");
        state.update(Message::ExpandLevel);
        assert_eq!(visible(&state)[1], (1, "b".to_owned(), Some(1)));
        state.update(Message::ExpandLevel);
        assert_eq!(visible(&state).len(), 5);
        state.update(Message::ExpandLevel);
        assert_eq!(*state.folds, HashSet::new());
    }

    // FR-NAV-02, FR-DONE-02
    #[test]
    fn collapse_level_leaves_hidden_completed_blocks_alone() {
        let mut state = open("- a\n\t- b\n- c @done\n\t- d\n\t\t- e", 0);
        state.update(Message::ToggleShowDone);
        state.update(Message::CollapseLevel);
        assert_eq!(visible(&state), [(0, "a".to_owned(), Some(1))]);
        assert_eq!(state.folds.len(), 1, "only a");
        let mut state = open("- a\n\t- b @done\n- c\n\t- d\n\t\t- e", 0);
        state.update(Message::ToggleShowDone);
        state.update(Message::CollapseLevel);
        state.update(Message::CollapseLevel);
        assert_eq!(
            visible(&state),
            [(0, "a".to_owned(), Some(0)), (0, "c".to_owned(), Some(2))],
            "a holds only a hidden completed block, yet folds"
        );
        state.update(Message::ExpandLevel);
        assert_eq!(state.folds.len(), 1, "d stayed collapsed inside c");
    }

    // FR-NAV-02
    #[test]
    fn collapse_level_goes_on_past_a_collapsed_block_to_its_siblings() {
        let mut state = open("- a\n\t- b\n- c\n\t- d", 0);
        state.update(Message::Collapse);
        state.update(Message::CollapseLevel);
        assert_eq!(
            visible(&state),
            [(0, "a".to_owned(), Some(1)), (0, "c".to_owned(), Some(1))]
        );
    }

    // FR-NAV-02, FR-NAV-03
    #[test]
    fn levels_count_from_the_zoom_and_leave_the_zoom_block_alone() {
        let mut state = open("A:\n\t- b\n\t\t- c\n- d\n\t- e\n\t\t- f", 0);
        state.update(Message::Collapse);
        state.update(Message::ZoomIn);
        state.update(Message::CollapseLevel);
        assert_eq!(
            visible(&state),
            [(0, "A".to_owned(), None), (1, "b".to_owned(), Some(1))]
        );
        state.update(Message::CollapseLevel);
        assert_eq!(state.folds.len(), 2, "only b below A, and A itself");
        state.update(Message::ExpandLevel);
        state.update(Message::ExpandLevel);
        assert_eq!(state.folds.len(), 1, "A stays collapsed outside its zoom");
    }

    // FR-NAV-02
    #[test]
    fn expand_level_skips_collapsed_leaves() {
        let mut state = open("- a\n- b\n\t- c\n\t\t- d", 0);
        state.folds.insert(state.focus().expect("a focus"));
        state.update(Message::FocusNext);
        state.update(Message::FocusNext);
        state.update(Message::ToggleFold);
        assert_eq!(visible(&state)[2], (1, "c".to_owned(), Some(1)));
        state.update(Message::ExpandLevel);
        assert_eq!(visible(&state)[2], (1, "c".to_owned(), None));
    }

    // FR-NAV-02
    #[test]
    fn expand_all_and_collapse_all() {
        let mut state = open(TREE, 0);
        state.update(Message::CollapseAll);
        assert_eq!(
            visible(&state),
            [(0, "A".to_owned(), Some(3)), (0, "e".to_owned(), None)]
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
        state
            .rows()
            .iter()
            .map(|row| row.text.to_string())
            .collect()
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
        assert!(state.rows().is_empty());
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
        state.update(Message::FocusSibling(Sibling::Next));
        assert_eq!(focus_text(&state), "c");
        state.update(Message::FocusSibling(Sibling::Previous));
        assert_eq!(focus_text(&state), "c");
        state.update(Message::FocusSibling(Sibling::First));
        assert_eq!(focus_text(&state), "c");
        state.update(Message::FocusSibling(Sibling::Last));
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
        state.update(Message::FocusSibling(Sibling::Next));
        assert_eq!(focus_text(&state), "d");
        state.update(Message::FocusSibling(Sibling::Next));
        assert_eq!(focus_text(&state), "d");
        state.update(Message::FocusSibling(Sibling::Previous));
        assert_eq!(focus_text(&state), "b");
        state.update(Message::FocusSibling(Sibling::Previous));
        assert_eq!(focus_text(&state), "b");
        state.update(Message::FocusParent);
        state.update(Message::FocusParent);
        assert_eq!(focus_text(&state), "A");
        state.update(Message::FocusSibling(Sibling::Next));
        assert_eq!(focus_text(&state), "e");
    }

    // FR-NAV-01, D-118
    #[test]
    fn first_and_last_sibling() {
        let mut state = open("- a\n- b\n\t- c\n- d\n- e", 1);
        state.update(Message::FocusSibling(Sibling::Last));
        assert_eq!(focus_text(&state), "e");
        state.update(Message::FocusSibling(Sibling::Last));
        assert_eq!(focus_text(&state), "e", "already the last");
        state.update(Message::FocusSibling(Sibling::First));
        assert_eq!(focus_text(&state), "a");
        state.update(Message::FocusSibling(Sibling::First));
        assert_eq!(focus_text(&state), "a", "already the first");
        let mut only = open("- a\n\t- c", 1);
        only.update(Message::FocusSibling(Sibling::Last));
        assert_eq!(
            focus_text(&only),
            "c",
            "an only child is its own last sibling"
        );
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
        assert_eq!(*state.folds, HashSet::new());
        assert_eq!(visible(&state)[0], (0, "a".to_owned(), None));
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
            [(0, "b".to_owned(), None), (1, "c".to_owned(), None)]
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
        state.update(Message::GoTo(Location::Outline(None)));
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
        state.update(Message::GoTo(Location::Outline(None)));
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
        state.update(Message::GoTo(Location::Outline(None)));
        assert_eq!(state.pane.history.len(), 3);
        state.update(Message::Forward);
        assert_eq!(zoom_text(&state), "Home");
        state.update(Message::GoTo(Location::Outline(None)));
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
            Message::FocusSibling(Sibling::Next),
            Message::FocusSibling(Sibling::Previous),
            Message::FocusSibling(Sibling::First),
            Message::FocusSibling(Sibling::Last),
        ] {
            state.update(message.clone());
            assert_eq!(focus_text(&state), "b", "{message:?}");
            assert_eq!(*state.folds, HashSet::new(), "{message:?}");
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
            *state.folds,
            HashSet::new(),
            "the title expands what is below it"
        );
        state.update(Message::ToggleSubtree);
        assert_eq!(state.folds.len(), 1);
        state.update(Message::GoTo(Location::Outline(None)));
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
        assert_eq!(visible(&state)[0], (0, "A".to_owned(), None));
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
            Message::FocusSibling(Sibling::Next),
            Message::FocusSibling(Sibling::Previous),
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
            Message::GoTo(Location::Outline(None)),
            Message::Back,
            Message::Forward,
        ] {
            assert_eq!(state.update(message.clone()), [], "{message:?}");
            assert_eq!(state.focus(), None, "{message:?}");
        }
    }

    // FR-SET-04
    #[test]
    fn config_problems_show_in_the_status_line_for_a_while_and_leave_editing_alone() {
        let mut state = state();
        state.update(Message::EditAtEnd);
        let problems = vec!["unknown setting `theme`".to_owned()];
        assert_eq!(state.update(Message::ConfigProblems(problems.clone())), []);
        assert_eq!(state.notice(), Some(&Notice::ConfigProblems(problems)));
        assert!(state.editor().is_some(), "still editing");
    }

    // FR-UI-12, ui.md §10.1
    #[test]
    fn no_weather_says_so_in_the_status_line_and_leaves_editing_alone() {
        let mut state = state();
        state.update(Message::EditAtEnd);
        assert_eq!(state.update(Message::NoWeather), []);
        assert_eq!(state.notice(), Some(&Notice::NoWeather));
        assert!(state.editor().is_some(), "still editing");
    }

    /// The id of the row reading `text`.
    fn row_id(state: &State, text: &str) -> ItemId {
        state
            .rows()
            .iter()
            .find(|row| &*row.text == text)
            .map(|row| row.id)
            .expect("a row reads the text")
    }

    // FR-KEY-05
    #[test]
    fn focus_goes_to_a_shown_block_and_keeps_the_selection() {
        let mut state = open(TREE, 0);
        state.update(Message::Select(SelectionChange::Toggle));
        let d = row_id(&state, "d");
        assert_eq!(state.update(Message::Focus(d)), []);
        assert_eq!(focus_text(&state), "d");
        assert!(state.pane.selection.contains(row_id(&state, "A")));
        let c = row_id(&state, "c");
        state.update(Message::Focus(row_id(&state, "b")));
        state.update(Message::Collapse);
        state.update(Message::Focus(c));
        assert_eq!(focus_text(&state), "b", "c is folded away");
    }

    // FR-KEY-05
    #[test]
    fn focus_ends_editing() {
        let mut state = open(TREE, 0);
        state.update(Message::EditAtEnd);
        state.update(Message::Focus(row_id(&state, "e")));
        assert_eq!(state.editor(), None);
        assert_eq!(focus_text(&state), "e");
    }

    // FR-KEY-05, navigation.md §4
    #[test]
    fn a_crumb_zooms_to_its_depth_as_a_step_in_the_history() {
        let mut state = open(TREE, 1);
        state.update(Message::ZoomIn);
        state.update(Message::ZoomIn);
        assert_eq!(state.breadcrumbs(), ["A", "b", "c"]);
        state.update(Message::GoToCrumb(3));
        assert_eq!(
            state.pane.history.len(),
            3,
            "the zoom block is where the pane is"
        );
        state.update(Message::GoToCrumb(4));
        assert_eq!(zoom_text(&state), "c", "no crumb that deep");
        state.update(Message::GoToCrumb(1));
        assert_eq!(state.breadcrumbs(), ["A"]);
        assert_eq!(focus_text(&state), "c", "the block zoomed out of");
        state.update(Message::Back);
        assert_eq!(zoom_text(&state), "c");
        state.update(Message::GoToCrumb(0));
        assert_eq!(zoom_text(&state), "Home");
    }

    // FR-UI-06
    #[test]
    fn a_theme_picker_key_ends_editing_and_a_command_menu_key_does_not() {
        let mut state = state();
        state.update(Message::EditAtEnd);
        state.update(Message::Menu(Menu::CommandMenu, MenuInput::Dismiss));
        assert!(
            state.editor().is_some(),
            "the command menu belongs to edit mode"
        );
        state.update(Message::Menu(Menu::ThemePicker, MenuInput::Dismiss));
        assert_eq!(state.editor(), None);
    }
}
