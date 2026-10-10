//! Key events to app messages, and to the frontend's own commands
//! ([keybindings.md](../../../docs/specs/keybindings.md)).

use std::{borrow::Cow, mem, time::Duration};

use crossterm::event::{Event, KeyCode, KeyModifiers};
use jiff::civil::DateTime;
use thiserror::Error;

use super::widgets::ShortcutGroup;
use crate::app::{
    Animations, AttachmentsInput, Command, DateInput, Inputs, Menu, MenuInput, Message, Mode,
    Motion, NavigatorInput, NodeInput, PaneChange, Paste, QuickAddInput, SaveAs, SettingsInput,
    TextEdit,
    commands::{Group, NAVIGATOR, REGISTRY},
};

/// How long a started multi-key sequence waits for its next key ([keybindings.md §1]).
///
/// [keybindings.md §1]: ../../../docs/specs/keybindings.md#1-terminal-constraints
pub const SEQUENCE_TIMEOUT: Duration = Duration::from_secs(1);

/// One key press: its modifiers and code, with `Shift` dropped from characters because the
/// character already says whether it is shifted.
type Key = (KeyModifiers, KeyCode);

/// What the frontend knows that a message may carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Context {
    /// The rows a page down motion moves past.
    pub page_down_rows: usize,
    /// The rows a page up motion moves past.
    pub page_up_rows: usize,
    /// The local date and time: its date is what completing a block and the date keys write,
    /// and a new comment is stamped with it.
    pub now: DateTime,
    /// Whether keys navigate or edit text, which decides their bindings.
    pub mode: Mode,
    /// Whether the command menu is open, which takes the keys of edit mode (D-50).
    pub command_menu: bool,
    /// The popup over the outline that takes the keys of navigate mode, if one is open.
    pub overlay: Option<Overlay>,
    /// Whether the shortcuts panel is open, which `Esc` closes in navigate mode (FR-KEY-04).
    pub shortcuts: bool,
    /// Whether the Navigator is open, and so has the keys (FR-UI-01, D-79).
    pub navigator: bool,
}

impl Context {
    /// What a command's message carries of this context.
    fn inputs(&self) -> Inputs {
        Inputs {
            today: self.now.date(),
            page_down_rows: self.page_down_rows,
            page_up_rows: self.page_up_rows,
        }
    }
}

/// What a key asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    /// Sends the core this message.
    Send(Message),
    /// Turns word wrap on or off, which is the frontend's to draw (FR-UI-05).
    ToggleWrap,
    /// Opens Go to Command, listing the bindings the frontend shows (FR-NAV-06).
    GoToCommand,
    /// Opens the theme picker over the frontend's themes (FR-UI-06).
    ChooseTheme,
    /// Opens the settings window over the frontend's settings (FR-SET-02).
    OpenSettings,
    /// Animates this much from now on, as animation is the frontend's (FR-UI-11).
    SetAnimations(Animations),
    /// Starts the weather at once, as the weather is the frontend's (FR-UI-12).
    StartWeather,
    /// Makes the focused pane narrower, as widths are the frontend's to choose (FR-UI-03).
    NarrowPane,
    /// Makes the focused pane wider.
    WidenPane,
    /// Scrolls the shortcuts panel down a page, as scrolling is the frontend's (D-55).
    ScrollShortcutsDown,
    /// Scrolls the shortcuts panel up a page.
    ScrollShortcutsUp,
    /// Scrolls the attachments panel or moves its selection, as scrolling is the frontend's (D-55,
    /// D-109).
    ScrollAttachments(ScrollBy),
}

/// How far a key scrolls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollBy {
    /// A line up.
    Up,
    /// A line down.
    Down,
    /// A page up.
    PageUp,
    /// A page down.
    PageDown,
    /// To the top.
    Top,
    /// To the bottom.
    Bottom,
}

/// A popup over the outline in navigate mode, which takes its keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    /// The color picker (FR-TAG-05).
    ColorPicker,
    /// The date prompt (FR-TAG-03).
    DatePrompt,
    /// The Quick Add panel (D-96).
    QuickAdd,
    /// Quick Add's project picker (D-96).
    QuickAddPicker,
    /// The type picker (FR-BLK-08).
    TypePicker,
    /// The Edit Node dialog (FR-MODE-09).
    NodeEditor,
    /// The Edit Node dialog's parent finder.
    NodeEditorFinder,
    /// Go to Symbol, or Go to Project, its form over the projects alone (FR-NAV-05, D-65).
    GoToSymbol,
    /// Move to (FR-OPS-05).
    MoveTo,
    /// Go to Command (FR-NAV-06).
    GoToCommand,
    /// The theme picker (FR-UI-06).
    ThemePicker,
    /// The settings window while it browses its settings (FR-SET-02).
    Settings,
    /// The settings window while its filter or a setting's prompt takes text.
    SettingsText,
    /// The settings window while a setting's value picker is open.
    SettingsPicker,
    /// The Save As prompt (FR-DOC-06).
    SaveAs,
    /// The context menu (FR-KEY-06).
    ContextMenu,
    /// The attachments panel (FR-ATT-03).
    Attachments,
    /// The attachments panel while it edits a note (FR-ATT-04).
    AttachmentsEditor,
    /// The attachments panel while a comment is typed (FR-ATT-05).
    AttachmentsComment,
}

impl Overlay {
    /// The menu or picker it is, which takes the keys of pickers, or `None` for a prompt or
    /// dialog with keys of its own ([keybindings.md §4]).
    ///
    /// [keybindings.md §4]: ../../../docs/specs/keybindings.md#4-other-contexts
    fn menu(self) -> Option<Menu> {
        match self {
            Self::ColorPicker => Some(Menu::ColorPicker),
            Self::QuickAddPicker => Some(Menu::QuickAdd),
            Self::TypePicker => Some(Menu::TypePicker),
            Self::NodeEditorFinder => Some(Menu::NodeEditor),
            Self::GoToSymbol => Some(Menu::GoToSymbol),
            Self::MoveTo => Some(Menu::MoveTo),
            Self::GoToCommand => Some(Menu::GoToCommand),
            Self::ThemePicker => Some(Menu::ThemePicker),
            Self::ContextMenu => Some(Menu::ContextMenu),
            Self::DatePrompt
            | Self::QuickAdd
            | Self::NodeEditor
            | Self::Attachments
            | Self::AttachmentsEditor
            | Self::AttachmentsComment
            | Self::Settings
            | Self::SettingsText
            | Self::SettingsPicker
            | Self::SaveAs => None,
        }
    }

    /// Whether it takes pasted text: a prompt or a dialog with a text to edit.
    fn takes_text(self) -> bool {
        matches!(
            self,
            Self::DatePrompt
                | Self::QuickAdd
                | Self::SaveAs
                | Self::SettingsText
                | Self::NodeEditor
                | Self::AttachmentsEditor
                | Self::AttachmentsComment
        )
    }
}

/// A [`Key`]: `key!('j')` for a character, `key!(ctrl 'q')` with `Ctrl`, `key!(Down)` for a
/// named key, `key!(ctrl Down)` for a named key with `Ctrl`, and likewise with `alt`, `shift`,
/// `alt shift` and `ctrl shift`.
macro_rules! key {
    (ctrl $c:literal) => {
        (KeyModifiers::CONTROL, KeyCode::Char($c))
    };
    (ctrl $code:ident) => {
        (KeyModifiers::CONTROL, KeyCode::$code)
    };
    (alt $c:literal) => {
        (KeyModifiers::ALT, KeyCode::Char($c))
    };
    (alt $code:ident) => {
        (KeyModifiers::ALT, KeyCode::$code)
    };
    (alt shift $code:ident) => {
        (KeyModifiers::ALT.union(KeyModifiers::SHIFT), KeyCode::$code)
    };
    (ctrl shift $code:ident) => {
        (
            KeyModifiers::CONTROL.union(KeyModifiers::SHIFT),
            KeyCode::$code,
        )
    };
    (shift $code:ident) => {
        (KeyModifiers::SHIFT, KeyCode::$code)
    };
    ($c:literal) => {
        (KeyModifiers::NONE, KeyCode::Char($c))
    };
    ($code:ident) => {
        (KeyModifiers::NONE, KeyCode::$code)
    };
}

/// What a key does while the conflict prompt is open (file-io.md §3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptKey {
    /// Sends the core this message.
    Send(Message),
    /// Scrolls the diff by this many lines, down when positive.
    Scroll(isize),
}

/// A prompt binding's key and what it does, given the rows in a page.
type PromptBinding = (Key, fn(isize) -> PromptKey);

/// Conflict prompt bindings: the choices, scrolling the diff and quitting. The prompt opens on
/// its own, maybe while the user types, so its choices take `Ctrl` keys no typing sends (D-115).
const PROMPT: &[PromptBinding] = &[
    (key!(ctrl 'q'), |_| PromptKey::Send(Message::Quit)),
    (key!(ctrl 'k'), |_| PromptKey::Send(Message::KeepMine)),
    (key!(ctrl 'r'), |_| PromptKey::Send(Message::TakeTheirs)),
    (key!(ctrl 'l'), |_| PromptKey::Send(Message::ToggleDiff)),
    (key!('j'), |_| PromptKey::Scroll(1)),
    (key!(Down), |_| PromptKey::Scroll(1)),
    (key!('k'), |_| PromptKey::Scroll(-1)),
    (key!(Up), |_| PromptKey::Scroll(-1)),
    (key!(ctrl 'd'), PromptKey::Scroll),
    (key!(PageDown), PromptKey::Scroll),
    (key!(ctrl 'u'), |rows| {
        PromptKey::Scroll(rows.saturating_neg())
    }),
    (key!(PageUp), |rows| {
        PromptKey::Scroll(rows.saturating_neg())
    }),
];

/// What `event` does in the conflict prompt, where a page is `page_rows` lines of the diff. Only
/// key presses count.
#[must_use]
pub fn prompt_key(event: &Event, page_rows: usize) -> Option<PromptKey> {
    let key = event.as_key_press_event()?;
    let pressed = (key.modifiers, key.code);
    let rows = isize::try_from(page_rows).unwrap_or(isize::MAX);
    PROMPT
        .iter()
        .find(|(key, _)| *key == pressed)
        .map(|(_, action)| action(rows))
}

/// Moving the caret by a [`Motion`], extending the selection when the second argument is `true`.
macro_rules! go {
    ($motion:ident, $select:literal) => {
        TextEdit::Move {
            motion: Motion::$motion,
            select: $select,
        }
    };
}

/// The standard text keys of every text field: caret motions, selection and deletion
/// ([editing.md §2]).
///
/// [editing.md §2]: ../../../docs/specs/editing.md#2-edit-mode
const TEXT: &[(Key, TextEdit)] = &[
    (key!(Left), go!(Left, false)),
    (key!(Right), go!(Right, false)),
    (key!(ctrl Left), go!(WordLeft, false)),
    (key!(ctrl Right), go!(WordRight, false)),
    (key!(alt 'b'), go!(WordLeft, false)),
    (key!(alt 'f'), go!(WordRight, false)),
    (key!(Home), go!(Start, false)),
    (key!(End), go!(End, false)),
    (key!(shift Left), go!(Left, true)),
    (key!(shift Right), go!(Right, true)),
    (key!(ctrl shift Left), go!(WordLeft, true)),
    (key!(ctrl shift Right), go!(WordRight, true)),
    (key!(shift Home), go!(Start, true)),
    (key!(shift End), go!(End, true)),
    (key!(shift Up), go!(Start, true)),
    (key!(shift Down), go!(End, true)),
    (key!(Backspace), TextEdit::DeleteBackward),
    (key!(Delete), TextEdit::DeleteForward),
    (key!(ctrl 'w'), TextEdit::DeleteWordBackward),
    (key!(alt Backspace), TextEdit::DeleteWordBackward),
    (key!(ctrl 'a'), TextEdit::SelectAll),
];

/// Bindings of the open command menu and pickers ([keybindings.md §3–4], [editing.md §2.3]);
/// characters without other bindings filter them, and other keys do nothing.
///
/// [keybindings.md §3–4]: ../../../docs/specs/keybindings.md#4-other-contexts
/// [editing.md §2.3]: ../../../docs/specs/editing.md#23-command-menu
const MENU: &[(Key, MenuInput)] = &[
    (key!(Esc), MenuInput::Dismiss),
    (key!(Enter), MenuInput::Choose),
    (key!(Up), MenuInput::Previous),
    (key!(ctrl 'p'), MenuInput::Previous),
    (key!(Down), MenuInput::Next),
    (key!(ctrl 'n'), MenuInput::Next),
    (key!(Backspace), MenuInput::Erase),
];

/// The keys of Go to Symbol that open the highlighted block in a new pane instead of going to
/// it ([keybindings.md §4]).
///
/// [keybindings.md §4]: ../../../docs/specs/keybindings.md#4-other-contexts
const OPEN_IN_NEW_PANE: [Key; 3] = [key!(alt Enter), key!(ctrl 'v'), key!(ctrl 't')];

/// The message of the open `menu` for `key`: `Ctrl+Q` quits, a menu binding, else typing the
/// character it is, as in edit mode.
fn menu_message(key: Key, menu: Menu) -> Option<Message> {
    if key == key!(ctrl 'q') {
        return Some(Message::Quit);
    }
    Some(Message::Menu(menu, menu_input(key)?))
}

/// What `key` does in a menu or picker: its binding, else typing the character it is.
fn menu_input(key: Key) -> Option<MenuInput> {
    MENU.iter()
        .find(|(bound, _)| *bound == key)
        .map(|&(_, input)| input)
        .or_else(|| typed_char(key).map(MenuInput::Type))
}

/// Bindings of the settings window while it browses its settings ([ui.md §8]); other keys do
/// nothing.
///
/// [ui.md §8]: ../../../docs/specs/ui.md#8-settings-window-fr-set-02-fr-set-03
const SETTINGS: &[(Key, SettingsInput)] = &[
    (key!('j'), SettingsInput::Next),
    (key!(Down), SettingsInput::Next),
    (key!('k'), SettingsInput::Previous),
    (key!(Up), SettingsInput::Previous),
    (key!(Tab), SettingsInput::SwitchColumn),
    (key!(Enter), SettingsInput::Change),
    (key!(' '), SettingsInput::Change),
    (key!('h'), SettingsInput::Cycle(false)),
    (key!(Left), SettingsInput::Cycle(false)),
    (key!('l'), SettingsInput::Cycle(true)),
    (key!(Right), SettingsInput::Cycle(true)),
    (key!('r'), SettingsInput::Reset),
    (key!('/'), SettingsInput::Filter),
    (key!(Esc), SettingsInput::Close),
];

/// The settings window's message for `key` while it browses: `Ctrl+Q` quits, else its binding.
fn settings_message(key: Key) -> Option<Message> {
    if key == key!(ctrl 'q') {
        return Some(Message::Quit);
    }
    SETTINGS
        .iter()
        .find(|(bound, _)| *bound == key)
        .map(|(_, input)| Message::Settings(input.clone()))
}

/// Bindings of the settings window's filter and prompt besides the [`TEXT`] keys; characters
/// without other bindings are typed.
const SETTINGS_TEXT: &[(Key, SettingsInput)] = &[
    (key!(Enter), SettingsInput::Accept),
    (key!(Esc), SettingsInput::Cancel),
    (key!(Down), SettingsInput::Next),
    (key!(Up), SettingsInput::Previous),
];

/// The settings window's message for `key` while its filter or a prompt takes text: `Ctrl+Q`
/// quits, `Ctrl+V` pastes into the text, a binding, else the change to the text it asks for.
fn settings_text_message(key: Key) -> Option<Message> {
    if key == key!(ctrl 'q') {
        return Some(Message::Quit);
    }
    if key == key!(ctrl 'v') {
        return Some(Message::Paste(Paste::Below));
    }
    let input = SETTINGS_TEXT
        .iter()
        .find(|(bound, _)| *bound == key)
        .map(|(_, input)| input.clone())
        .or_else(|| text_edit(key).map(SettingsInput::Edit))?;
    Some(Message::Settings(input))
}

/// The settings window's message for `key` while a value picker is open: `Ctrl+Q` quits, else
/// the key of a picker it is.
fn settings_picker_message(key: Key) -> Option<Message> {
    if key == key!(ctrl 'q') {
        return Some(Message::Quit);
    }
    Some(Message::Settings(SettingsInput::Pick(menu_input(key)?)))
}

/// Bindings of the open date prompt ([keybindings.md §3]) besides the [`TEXT`] keys; characters
/// without other bindings are typed, and other keys do nothing.
///
/// [keybindings.md §3]: ../../../docs/specs/keybindings.md#3-edit-mode
const DATE_PROMPT: &[(Key, DateInput)] = &[
    (key!(Enter), DateInput::Apply),
    (key!(Esc), DateInput::Cancel),
];

/// What the shortcuts panel lists where `overlay` has the keys, if it is the date prompt, Quick
/// Add or the Edit Node dialog: its bindings with what each does, then pasting and quitting,
/// since the outline's keys type text there ([ui.md §4]).
///
/// [ui.md §4]: ../../../docs/specs/ui.md#4-shortcuts-panel-fr-key-04
fn dialog_shortcuts(overlay: Overlay) -> Option<ShortcutGroup> {
    /// Each of `names` with the keys `table` binds to its input.
    fn listed<I: Copy + PartialEq>(
        table: &[(Key, I)],
        names: &[(I, &'static str)],
    ) -> Vec<(Cow<'static, str>, String)> {
        names
            .iter()
            .map(|&(input, name)| {
                let keys = table
                    .iter()
                    .filter(|&&(_, bound)| bound == input)
                    .map(|&(key, _)| key);
                (name.into(), keys_label(keys))
            })
            .collect()
    }
    let (group, mut entries) = match overlay {
        Overlay::DatePrompt => (
            Group::DatePrompt,
            listed(
                DATE_PROMPT,
                &[
                    (DateInput::Apply, "Set Date"),
                    (DateInput::Cancel, "Cancel"),
                ],
            ),
        ),
        Overlay::QuickAdd => (
            Group::QuickAdd,
            listed(
                QUICK_ADD,
                &[
                    (QuickAddInput::Add, "Add"),
                    (QuickAddInput::Cancel, "Cancel"),
                ],
            ),
        ),
        Overlay::NodeEditor => (
            Group::NodeEditor,
            listed(
                NODE_EDITOR,
                &[
                    (NodeInput::Next, "Next Row"),
                    (NodeInput::Previous, "Previous Row"),
                    (NodeInput::CycleType, "Cycle Type"),
                    (NodeInput::Accept, "Save"),
                    (NodeInput::Cancel, "Cancel"),
                ],
            ),
        ),
        _ => return None,
    };
    entries.push(("Paste".into(), key_label(key!(ctrl 'v'))));
    entries.push((Command::Quit.name(), key_label(key!(ctrl 'q'))));
    Some((group, entries))
}

/// The date-prompt message for `key`: `Ctrl+Q` quits, `Ctrl+V` pastes into the text, a prompt
/// binding, else the change to the text it asks for.
fn date_prompt_message(key: Key) -> Option<Message> {
    if key == key!(ctrl 'q') {
        return Some(Message::Quit);
    }
    if key == key!(ctrl 'v') {
        return Some(Message::Paste(Paste::Below));
    }
    let input = DATE_PROMPT
        .iter()
        .find(|(bound, _)| *bound == key)
        .map(|&(_, input)| input)
        .or_else(|| text_edit(key).map(DateInput::Edit))?;
    Some(Message::DatePrompt(input))
}

/// Bindings of the Quick Add panel ([keybindings.md §4]) besides the [`TEXT`] keys; characters
/// without other bindings are typed, `/` among them, and other keys do nothing.
///
/// [keybindings.md §4]: ../../../docs/specs/keybindings.md#4-other-contexts
const QUICK_ADD: &[(Key, QuickAddInput)] = &[
    (key!(Enter), QuickAddInput::Add),
    (key!(Esc), QuickAddInput::Cancel),
];

/// The Quick Add message for `key`: `Ctrl+Q` quits, `Ctrl+V` pastes into the text, a panel
/// binding, else the change to the text it asks for.
fn quick_add_message(key: Key) -> Option<Message> {
    if key == key!(ctrl 'q') {
        return Some(Message::Quit);
    }
    if key == key!(ctrl 'v') {
        return Some(Message::Paste(Paste::Below));
    }
    let input = QUICK_ADD
        .iter()
        .find(|(bound, _)| *bound == key)
        .map(|&(_, input)| input)
        .or_else(|| text_edit(key).map(QuickAddInput::Edit))?;
    Some(Message::QuickAdd(input))
}

/// Bindings of the Edit Node dialog ([keybindings.md §4]) besides the [`TEXT`] keys, which
/// the dialog reads by its focused row; characters without other bindings are typed, and other
/// keys do nothing.
///
/// [keybindings.md §4]: ../../../docs/specs/keybindings.md#4-other-contexts
const NODE_EDITOR: &[(Key, NodeInput)] = &[
    (key!(Enter), NodeInput::Accept),
    (key!(Esc), NodeInput::Cancel),
    (key!(Tab), NodeInput::Next),
    (key!(Down), NodeInput::Next),
    (key!(shift BackTab), NodeInput::Previous),
    (key!(Up), NodeInput::Previous),
    (key!(ctrl 't'), NodeInput::CycleType),
];

/// The Edit Node message for `key`: `Ctrl+Q` quits, `Ctrl+V` pastes into the text, a dialog
/// binding, else the text key it is.
fn node_editor_message(key: Key) -> Option<Message> {
    if key == key!(ctrl 'q') {
        return Some(Message::Quit);
    }
    if key == key!(ctrl 'v') {
        return Some(Message::Paste(Paste::Below));
    }
    let input = NODE_EDITOR
        .iter()
        .find(|(bound, _)| *bound == key)
        .map(|&(_, input)| input)
        .or_else(|| text_edit(key).map(NodeInput::Key))?;
    Some(Message::NodeEditor(input))
}

/// Bindings of the attachments panel ([keybindings.md §4]) besides [`SCROLL`]; other keys do
/// nothing.
///
/// [keybindings.md §4]: ../../../docs/specs/keybindings.md#4-other-contexts
const ATTACHMENTS: &[(Key, AttachmentsInput)] = &[
    (key!(Esc), AttachmentsInput::Close),
    (key!('e'), AttachmentsInput::Edit),
    (key!(Enter), AttachmentsInput::Edit),
    (key!('a'), AttachmentsInput::AddComment),
    (key!('d'), AttachmentsInput::DeleteComment),
    (key!(Delete), AttachmentsInput::DeleteComment),
];

/// The keys that scroll the attachments panel and move its selection (D-107, D-109).
const SCROLL: &[(Key, ScrollBy)] = &[
    (key!('j'), ScrollBy::Down),
    (key!(Down), ScrollBy::Down),
    (key!('k'), ScrollBy::Up),
    (key!(Up), ScrollBy::Up),
    (key!(PageDown), ScrollBy::PageDown),
    (key!(ctrl 'd'), ScrollBy::PageDown),
    (key!(PageUp), ScrollBy::PageUp),
    (key!(ctrl 'u'), ScrollBy::PageUp),
    (key!(Home), ScrollBy::Top),
    (key!('g'), ScrollBy::Top),
    (key!(End), ScrollBy::Bottom),
    (key!('G'), ScrollBy::Bottom),
];

/// The attachments panel's action for `key`: `Ctrl+Q` quits, else its binding or scroll, if any.
fn attachments_action(key: Key) -> Option<KeyAction> {
    if key == key!(ctrl 'q') {
        return Some(KeyAction::Send(Message::Quit));
    }
    ATTACHMENTS
        .iter()
        .find(|(bound, _)| *bound == key)
        .map(|&(_, input)| KeyAction::Send(Message::Attachments(input)))
        .or_else(|| {
            SCROLL
                .iter()
                .find(|(bound, _)| *bound == key)
                .map(|&(_, scroll)| KeyAction::ScrollAttachments(scroll))
        })
}

/// Bindings of the note editor ([keybindings.md §4]) besides the [`TEXT`] keys; characters
/// without other bindings are typed, and other keys do nothing.
///
/// [keybindings.md §4]: ../../../docs/specs/keybindings.md#4-other-contexts
const NOTE_EDITOR: &[(Key, AttachmentsInput)] = &[
    (key!(Esc), AttachmentsInput::Save),
    (key!(Enter), AttachmentsInput::Break),
    (key!(Up), AttachmentsInput::LineUp),
    (key!(Down), AttachmentsInput::LineDown),
];

/// The note editor's message for `key`: `Ctrl+Q` quits, `Ctrl+V` pastes into the note, an
/// editor binding, else the text key it is.
fn note_editor_message(key: Key) -> Option<Message> {
    if key == key!(ctrl 'q') {
        return Some(Message::Quit);
    }
    if key == key!(ctrl 'v') {
        return Some(Message::Paste(Paste::Below));
    }
    let input = NOTE_EDITOR
        .iter()
        .find(|(bound, _)| *bound == key)
        .map(|&(_, input)| input)
        .or_else(|| text_edit(key).map(AttachmentsInput::Text))?;
    Some(Message::Attachments(input))
}

/// The message of the comment being typed for `key` at local time `now`: `Ctrl+Q` quits, `Ctrl+V`
/// pastes into it, `Enter` adds or saves it stamped `now`, `Esc` cancels, else the text key it is.
fn comment_message(key: Key, now: DateTime) -> Option<Message> {
    let input = match key {
        key!(ctrl 'q') => return Some(Message::Quit),
        key!(ctrl 'v') => return Some(Message::Paste(Paste::Below)),
        key!(Enter) => AttachmentsInput::Accept(now),
        key!(Esc) => AttachmentsInput::Cancel,
        _ => AttachmentsInput::Text(text_edit(key)?),
    };
    Some(Message::Attachments(input))
}

/// Bindings of the Save As prompt ([keybindings.md §4]) besides the [`TEXT`] keys; characters
/// without other bindings are typed, and other keys do nothing.
///
/// [keybindings.md §4]: ../../../docs/specs/keybindings.md#4-other-contexts
const SAVE_AS: &[(Key, SaveAs)] = &[
    (key!(Enter), SaveAs::Accept),
    (key!(Esc), SaveAs::Cancel),
    (key!(Tab), SaveAs::Complete),
];

/// The Save As message for `key`: `Ctrl+Q` quits, `Ctrl+V` pastes into the path, a prompt
/// binding, else the change to the path it asks for.
fn save_as_message(key: Key) -> Option<Message> {
    if key == key!(ctrl 'q') {
        return Some(Message::Quit);
    }
    if key == key!(ctrl 'v') {
        return Some(Message::Paste(Paste::Below));
    }
    let event = SAVE_AS
        .iter()
        .find(|(bound, _)| *bound == key)
        .map(|(_, event)| event.clone())
        .or_else(|| text_edit(key).map(SaveAs::Edit))?;
    Some(Message::SaveAs(event))
}

/// The character `key` types: a character without modifiers, or with `Ctrl+Alt`, which some
/// terminals report for `AltGr`.
fn typed_char((modifiers, code): Key) -> Option<char> {
    let typed = modifiers.is_empty() || modifiers == KeyModifiers::CONTROL.union(KeyModifiers::ALT);
    match code {
        KeyCode::Char(c) if typed => Some(c),
        _ => None,
    }
}

/// The change to a text field `key` asks for: its [`TEXT`] binding, else typing the character it
/// is.
fn text_edit(key: Key) -> Option<TextEdit> {
    TEXT.iter()
        .find(|(bound, _)| *bound == key)
        .map(|&(_, edit)| edit)
        .or_else(|| typed_char(key).map(TextEdit::Insert))
}

/// The key `text` names in the notation of the config file ([keybindings.md §5]): a character,
/// or a key name or character after `ctrl+`, `alt+` and `shift+` in any order. `shift+` with a
/// letter is the capital, as the keymap matches it (D-42). `None` for anything else.
///
/// [keybindings.md §5]: ../../../docs/specs/keybindings.md#5-configuration
fn parse_key(text: &str) -> Option<Key> {
    let (modifiers, name) = match text.rsplit_once('+') {
        _ if text.chars().count() == 1 => (None, text),
        None => (None, text),
        Some((modifiers, "")) => (Some(modifiers.strip_suffix('+')?), "+"),
        Some((modifiers, name)) => (Some(modifiers), name),
    };
    let mut held = KeyModifiers::NONE;
    for modifier in modifiers
        .into_iter()
        .flat_map(|modifiers| modifiers.split('+'))
    {
        held |= match modifier {
            "ctrl" => KeyModifiers::CONTROL,
            "alt" => KeyModifiers::ALT,
            "shift" => KeyModifiers::SHIFT,
            _ => return None,
        };
    }
    let mut chars = name.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        let c = if held.contains(KeyModifiers::SHIFT) {
            c.to_ascii_uppercase()
        } else {
            c
        };
        return Some((held.difference(KeyModifiers::SHIFT), KeyCode::Char(c)));
    }
    let code = match name {
        "space" => KeyCode::Char(' '),
        "enter" => KeyCode::Enter,
        "esc" => KeyCode::Esc,
        "tab" if held.contains(KeyModifiers::SHIFT) => KeyCode::BackTab,
        "tab" => KeyCode::Tab,
        "backspace" => KeyCode::Backspace,
        "delete" => KeyCode::Delete,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "menu" => KeyCode::Menu,
        name => KeyCode::F(name.strip_prefix('f')?.parse().ok()?),
    };
    Some((held, code))
}

/// The keys of the sequence `text` names: keys as [`parse_key`] reads them, separated by
/// spaces. `None` when any of them is not a key.
fn parse_sequence(text: &str) -> Option<Vec<Key>> {
    text.split(' ').map(parse_key).collect()
}

/// How the key sequence `keys` reads on screen: as [keybindings.md] writes keys, such as
/// `Ctrl+D`, `Alt+k`, `Ctrl+Shift+Z`, `Shift+↓`, `Space`, a sequence of characters run together
/// (`gg`) and other sequences spaced (`Ctrl+W v`). The same on every platform, unlike crossterm's
/// `Display`, which names `Ctrl` "Control" on Unix.
///
/// [keybindings.md]: ../../../docs/specs/keybindings.md
fn sequence_label(keys: &[Key]) -> String {
    let plain = keys.iter().all(|&(modifiers, code)| {
        modifiers.is_empty() && matches!(code, KeyCode::Char(c) if c != ' ')
    });
    let presses: Vec<String> = keys.iter().copied().map(key_label).collect();
    presses.join(if plain { "" } else { " " })
}

/// `name` as the config file may write a command's name: in lower case, with each run of
/// other characters than letters and digits as one `_`, so that `Move Up`, `move up` and
/// `move_up` are one name ([keybindings.md §5]).
///
/// [keybindings.md §5]: ../../../docs/specs/keybindings.md#5-configuration
fn config_name(name: &str) -> String {
    name.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

/// The command of `mode` named `name`, as the command palette names it or written as
/// [`config_name`] allows. Keys cannot give [`Command::Retype`], which changes an edit session
/// and only the command menu offers (D-44).
fn command_named(mode: Mode, name: &str) -> Option<Command> {
    let name = config_name(name);
    let name = RENAMED
        .iter()
        .find(|(old, _)| *old == name)
        .map_or(name.as_str(), |&(_, new)| new);
    REGISTRY
        .iter()
        .filter(|entry| entry.mode == mode && !matches!(entry.command, Command::Retype(_)))
        .map(|entry| entry.command)
        .find(|command| config_name(&command.name()) == name)
}

/// Names a config file written before a command was renamed may still use, each with the name
/// it now has, as [`config_name`] writes them ([keybindings.md §5]).
///
/// [keybindings.md §5]: ../../../docs/specs/keybindings.md#5-configuration
const RENAMED: &[(&str, &str)] = &[
    ("toggle_sidebar", "open_navigator"),
    ("focus_sidebar", "open_navigator"),
    ("show_details", "edit_node"),
];

/// Why a key binding the config file asks for was ignored ([keybindings.md §5]).
///
/// [keybindings.md §5]: ../../../docs/specs/keybindings.md#5-configuration
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BindingError {
    /// The keys are not written in the notation of keybindings.md §5.
    #[error("\"{keys}\" is not a key")]
    NotAKey {
        /// The keys as written.
        keys: String,
    },
    /// Edit mode types characters, so it binds single keys, not sequences.
    #[error("\"{keys}\": edit mode binds single keys, not sequences")]
    Sequence {
        /// The keys as written.
        keys: String,
    },
    /// No command of the mode has this name.
    #[error("\"{keys}\": no command \"{command}\" here")]
    UnknownCommand {
        /// The keys as written.
        keys: String,
        /// The name as written.
        command: String,
    },
    /// Another binding of the same table names the same keys, written differently.
    #[error("\"{keys}\" is bound twice")]
    BoundTwice {
        /// The keys as written.
        keys: String,
    },
}

/// What giving `command` asks for, its message carrying what it needs of `context`, as its keys
/// or Go to Command give it.
#[must_use]
pub fn action(command: Command, context: &Context) -> Option<KeyAction> {
    match command {
        Command::ToggleWrap => Some(KeyAction::ToggleWrap),
        Command::GoToCommand => Some(KeyAction::GoToCommand),
        Command::ChooseTheme => Some(KeyAction::ChooseTheme),
        Command::OpenSettings => Some(KeyAction::OpenSettings),
        Command::SetAnimations(setting) => Some(KeyAction::SetAnimations(setting)),
        Command::StartWeather => Some(KeyAction::StartWeather),
        Command::NarrowPane => Some(KeyAction::NarrowPane),
        Command::WidenPane => Some(KeyAction::WidenPane),
        Command::ScrollShortcutsDown => Some(KeyAction::ScrollShortcutsDown),
        Command::ScrollShortcutsUp => Some(KeyAction::ScrollShortcutsUp),
        command => command.message(&context.inputs()).map(KeyAction::Send),
    }
}

/// How `keys` read on screen, each as [`key_label`] writes it, separated by commas.
fn keys_label(keys: impl IntoIterator<Item = Key>) -> String {
    keys.into_iter()
        .map(key_label)
        .collect::<Vec<_>>()
        .join(", ")
}

/// How one key reads on screen ([`sequence_label`]): its modifiers, then the key, a letter with `Ctrl` in
/// capitals and a capital letter with a modifier marked with `Shift`.
fn key_label((modifiers, code): Key) -> String {
    let mut label = String::new();
    let shifted = matches!(code, KeyCode::Char(c) if c.is_uppercase()) && !modifiers.is_empty();
    for (modifier, name) in [
        (KeyModifiers::CONTROL, "Ctrl+"),
        (KeyModifiers::ALT, "Alt+"),
    ] {
        if modifiers.contains(modifier) {
            label.push_str(name);
        }
    }
    if shifted || modifiers.contains(KeyModifiers::SHIFT) {
        label.push_str("Shift+");
    }
    let name = match code {
        KeyCode::Char(' ') => "Space".to_owned(),
        KeyCode::Char(c) if modifiers.contains(KeyModifiers::CONTROL) => {
            c.to_uppercase().to_string()
        }
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Up => "↑".to_owned(),
        KeyCode::Down => "↓".to_owned(),
        KeyCode::Left => "←".to_owned(),
        KeyCode::Right => "→".to_owned(),
        KeyCode::PageUp => "PgUp".to_owned(),
        KeyCode::PageDown => "PgDn".to_owned(),
        KeyCode::BackTab => "Tab".to_owned(),
        KeyCode::Enter => "Enter".to_owned(),
        KeyCode::Backspace => "Backspace".to_owned(),
        KeyCode::Delete => "Delete".to_owned(),
        code => code.to_string(),
    };
    label.push_str(&name);
    label
}

/// Turns key presses into messages, holding the keys of a multi-key sequence until it completes,
/// fails to match or times out. Keys that are a binding of their own and start longer ones, such
/// as `o` (D-53), send their own message only when the sequence ends without continuing.
#[derive(Debug)]
pub struct Keymap {
    /// Each navigate-mode sequence with its command.
    navigate: Vec<(Vec<Key>, Command)>,
    /// Each edit-mode key with its command, as a sequence of one key.
    edit: Vec<(Vec<Key>, Command)>,
    /// Each key of the open Navigator with its command.
    navigator: Vec<(Key, Command)>,
    pending: Vec<Key>,
    /// When the pending sequence started, in the frontend's time.
    started: Duration,
    /// Whether the key just handled ended a sequence that was a binding of its own, whose message
    /// came back instead, so the key is still to be handled.
    replay: bool,
}

impl Default for Keymap {
    /// The keymap of the registry's default bindings.
    fn default() -> Self {
        let mut navigate = Vec::new();
        let mut edit = Vec::new();
        for entry in REGISTRY {
            for keys in entry.keys.iter().filter_map(|keys| parse_sequence(keys)) {
                match entry.mode {
                    Mode::Navigate => navigate.push((keys, entry.command)),
                    Mode::Edit => edit.push((keys, entry.command)),
                }
            }
        }
        let navigator = NAVIGATOR
            .iter()
            .flat_map(|&(command, keys)| {
                keys.iter()
                    .filter_map(|keys| parse_key(keys))
                    .map(move |key| (key, command))
            })
            .collect();
        Self {
            navigate,
            edit,
            navigator,
            pending: Vec::new(),
            started: Duration::ZERO,
            replay: false,
        }
    }
}

impl Keymap {
    /// Applies the config file's `bindings` of `mode`, each keys written in the notation of
    /// [keybindings.md §5] with the name of the command they give there, or `none` to unbind
    /// them. A binding takes its keys from the command they gave and puts them first among its
    /// command's keys; the command's other keys stay. A binding that cannot be applied is left
    /// out, and so are bindings that name the same keys, and the default stays.
    ///
    /// [keybindings.md §5]: ../../../docs/specs/keybindings.md#5-configuration
    pub fn bind(&mut self, mode: Mode, bindings: &[(&str, &str)]) -> Vec<BindingError> {
        let mut errors = Vec::new();
        let mut parsed = Vec::new();
        for &(keys, name) in bindings {
            let Some(sequence) = parse_sequence(keys) else {
                errors.push(BindingError::NotAKey { keys: keys.into() });
                continue;
            };
            if mode == Mode::Edit && sequence.len() > 1 {
                errors.push(BindingError::Sequence { keys: keys.into() });
                continue;
            }
            let command = command_named(mode, name);
            if command.is_none() && name != "none" {
                errors.push(BindingError::UnknownCommand {
                    keys: keys.into(),
                    command: name.into(),
                });
                continue;
            }
            parsed.push((sequence, keys, command));
        }
        let twice = |sequence: &Vec<Key>| {
            parsed
                .iter()
                .filter(|(other, ..)| other == sequence)
                .count()
                > 1
        };
        errors.extend(
            parsed
                .iter()
                .filter(|(sequence, ..)| twice(sequence))
                .map(|&(_, keys, _)| BindingError::BoundTwice { keys: keys.into() }),
        );
        // Each goes first among its command's keys, so going backwards keeps the file's order.
        for (sequence, _, command) in parsed.iter().rev() {
            if twice(sequence) {
                continue;
            }
            let bound = match mode {
                Mode::Navigate => &mut self.navigate,
                Mode::Edit => &mut self.edit,
            };
            bound.retain(|(other, _)| other != sequence);
            if let Some(command) = *command {
                let at = bound
                    .iter()
                    .position(|&(_, other)| other == command)
                    .unwrap_or(bound.len());
                bound.insert(at, (sequence.clone(), command));
            }
        }
        errors
    }

    /// How the keys that give `command` in `mode` read on screen, each sequence as
    /// [keybindings.md] writes keys, separated by commas; empty when none does.
    ///
    /// [keybindings.md]: ../../../docs/specs/keybindings.md
    #[must_use]
    pub fn labels(&self, command: Command, mode: Mode) -> String {
        let bound = match mode {
            Mode::Navigate => &self.navigate,
            Mode::Edit => &self.edit,
        };
        bound
            .iter()
            .filter(|&&(_, bound)| bound == command)
            .map(|(keys, _)| sequence_label(keys))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// How the keys that give `command` read on screen, as [`Keymap::labels`] writes them: its
    /// navigate-mode keys if it is a command of navigate mode, else its keys in the open Navigator,
    /// or the other way round where `navigator` is open.
    #[must_use]
    pub fn bindings(&self, command: Command, navigator: bool) -> String {
        let navigate = || {
            REGISTRY
                .iter()
                .any(|entry| entry.mode == Mode::Navigate && entry.command == command)
                .then(|| self.labels(command, Mode::Navigate))
        };
        let focused_navigator = || {
            let keys: Vec<String> = self
                .navigator
                .iter()
                .filter(|&&(_, bound)| bound == command)
                .map(|&(key, _)| key_label(key))
                .collect();
            (!keys.is_empty()).then(|| keys.join(", "))
        };
        let keys = if navigator {
            focused_navigator().or_else(navigate)
        } else {
            navigate().or_else(focused_navigator)
        };
        keys.unwrap_or_default()
    }

    /// What the shortcuts panel lists where `context` gives the keys to the Navigator, a picker,
    /// the date prompt, Quick Add or the Edit Node dialog: their group with each intent's name
    /// and how its keys read; `None` where navigate or edit mode has the keys, or another prompt
    /// or dialog of its own ([ui.md §4]).
    ///
    /// [ui.md §4]: ../../../docs/specs/ui.md#4-shortcuts-panel-fr-key-04
    #[must_use]
    pub fn popup_shortcuts(&self, context: &Context) -> Option<ShortcutGroup> {
        if let Some(overlay) = context.overlay {
            let Some(menu) = overlay.menu() else {
                return dialog_shortcuts(overlay);
            };
            let mut entries: Vec<_> = [
                MenuInput::Next,
                MenuInput::Previous,
                MenuInput::Choose,
                MenuInput::Dismiss,
                MenuInput::Erase,
            ]
            .into_iter()
            .map(|input| {
                let keys = MENU
                    .iter()
                    .filter(|&&(_, bound)| bound == input)
                    .map(|&(key, _)| key);
                (input.name(), keys_label(keys))
            })
            .collect();
            if menu == Menu::GoToSymbol {
                entries.insert(
                    3,
                    (
                        Command::Pane(PaneChange::Open).name(),
                        keys_label(OPEN_IN_NEW_PANE),
                    ),
                );
            }
            entries.push((Command::Quit.name(), key_label(key!(ctrl 'q'))));
            return Some((Group::Picker, entries));
        }
        context.navigator.then(|| {
            let entries = NAVIGATOR
                .iter()
                .map(|&(command, _)| (command.name(), self.bindings(command, true)))
                .collect();
            (Group::Navigator, entries)
        })
    }

    /// What the navigate-mode binding whose keys are `keys` asks for, if any.
    fn bound(&self, keys: &[Key], context: &Context) -> Option<KeyAction> {
        let (_, command) = self.navigate.iter().find(|(bound, _)| bound == keys)?;
        action(*command, context)
    }

    /// What `pressed` asks for while the Navigator is open: its own keys alone (keybindings.md §4,
    /// D-79).
    fn navigator_action(&self, pressed: Key, context: &Context) -> Option<KeyAction> {
        match self.navigator.iter().find(|(key, _)| *key == pressed) {
            Some((_, command)) => action(*command, context),
            None => typed_char(pressed)
                .map(|c| KeyAction::Send(Message::Navigator(NavigatorInput::Type(c)))),
        }
    }

    /// The edit-mode message for `key`: its binding, else the change to the text it asks for.
    fn edit_message(&self, key: Key, context: &Context) -> Option<Message> {
        if let Some((_, command)) = self.edit.iter().find(|(bound, _)| *bound == [key]) {
            return command.message(&context.inputs());
        }
        text_edit(key).map(Message::Text)
    }

    /// What `event` completes at time `now`, if anything, a message carrying what it needs of
    /// `context`. Only key presses and text pasted into the terminal count, the latter in
    /// navigate and edit mode, the date prompt, the Quick Add panel, the Save As prompt, the
    /// settings window's prompt and filter and the Edit Node dialog alone; repeats, releases
    /// and other events are ignored. A key that continues no binding ends the pending sequence
    /// and does nothing, unless the sequence, or one that timed out, is a binding of its own:
    /// then what it asks for comes back and [`Keymap::replay`] says to handle `event` again once
    /// that is done.
    pub fn action_for(
        &mut self,
        event: &Event,
        now: Duration,
        context: &Context,
    ) -> Option<KeyAction> {
        if let Event::Paste(text) = event {
            let pending = mem::take(&mut self.pending);
            if let Some(action) = self.bound(&pending, context) {
                self.replay = true;
                return Some(action);
            }
            let takes_text = context.overlay.map_or(
                !context.command_menu && !context.navigator,
                Overlay::takes_text,
            );
            return takes_text.then(|| {
                KeyAction::Send(Message::Pasted {
                    text: text.clone(),
                    at: Paste::Below,
                })
            });
        }
        let key = event.as_key_press_event()?;
        if let Some(action) = self.expire(now, context) {
            self.replay = true;
            return Some(action);
        }
        if self.pending.is_empty() {
            self.started = now;
        }
        // The kitty protocol sends `Alt+Shift+Z` and `Ctrl+Shift+Z` as `z` with `Shift`, where
        // other reportings send `Z` (D-42); both become `Z`.
        let pressed = match key.code {
            KeyCode::Char(c) => (
                key.modifiers.difference(KeyModifiers::SHIFT),
                KeyCode::Char(if key.modifiers.contains(KeyModifiers::SHIFT) {
                    c.to_ascii_uppercase()
                } else {
                    c
                }),
            ),
            code => (key.modifiers, code),
        };
        if let Some(overlay) = context.overlay {
            self.pending.clear();
            let message = match overlay {
                Overlay::DatePrompt => date_prompt_message(pressed),
                Overlay::QuickAdd => quick_add_message(pressed),
                Overlay::QuickAddPicker => menu_message(pressed, Menu::QuickAdd),
                Overlay::ColorPicker => menu_message(pressed, Menu::ColorPicker),
                Overlay::TypePicker => menu_message(pressed, Menu::TypePicker),
                Overlay::GoToSymbol if OPEN_IN_NEW_PANE.contains(&pressed) => {
                    Some(Message::Pane(PaneChange::Open))
                }
                Overlay::GoToSymbol => menu_message(pressed, Menu::GoToSymbol),
                Overlay::MoveTo => menu_message(pressed, Menu::MoveTo),
                Overlay::GoToCommand => menu_message(pressed, Menu::GoToCommand),
                Overlay::ThemePicker => menu_message(pressed, Menu::ThemePicker),
                Overlay::Settings => settings_message(pressed),
                Overlay::SettingsText => settings_text_message(pressed),
                Overlay::SettingsPicker => settings_picker_message(pressed),
                Overlay::SaveAs => save_as_message(pressed),
                Overlay::ContextMenu => menu_message(pressed, Menu::ContextMenu),
                Overlay::NodeEditor => node_editor_message(pressed),
                Overlay::NodeEditorFinder => menu_message(pressed, Menu::NodeEditor),
                Overlay::Attachments => return attachments_action(pressed),
                Overlay::AttachmentsEditor => note_editor_message(pressed),
                Overlay::AttachmentsComment => comment_message(pressed, context.now),
            };
            return message.map(KeyAction::Send);
        }
        if context.navigator {
            return self.navigator_action(pressed, context);
        }
        if context.mode == Mode::Edit {
            self.pending.clear();
            let message = if context.command_menu {
                menu_message(pressed, Menu::CommandMenu)
            } else {
                self.edit_message(pressed, context)
            };
            return message.map(KeyAction::Send);
        }
        if context.shortcuts && self.pending.is_empty() && pressed == key!(Esc) {
            return Some(KeyAction::Send(Message::CloseShortcuts));
        }
        self.pending.push(pressed);
        let longer = self
            .navigate
            .iter()
            .any(|(keys, _)| keys.len() > self.pending.len() && keys.starts_with(&self.pending));
        if longer {
            return None;
        }
        if let Some(action) = self.bound(&self.pending, context) {
            self.pending.clear();
            return Some(action);
        }
        self.pending.pop();
        let held = self.bound(&self.pending, context);
        self.replay = held.is_some();
        self.pending.clear();
        held
    }

    /// Whether the key [`Keymap::action_for`] just handled is still to be handled, in the
    /// context what it returned leaves; reading it clears it.
    pub fn replay(&mut self) -> bool {
        mem::take(&mut self.replay)
    }

    /// Drops the pending sequence once [`SEQUENCE_TIMEOUT`] has passed since it started, and
    /// returns what the binding it is asks for, if any, such as a lone `o` (D-53).
    pub fn expire(&mut self, now: Duration, context: &Context) -> Option<KeyAction> {
        if self.deadline().is_none_or(|deadline| now < deadline) {
            return None;
        }
        let action = self.bound(&self.pending, context);
        self.pending.clear();
        action
    }

    /// When the pending sequence times out, or `None` when no sequence is pending.
    #[must_use]
    pub fn deadline(&self) -> Option<Duration> {
        (!self.pending.is_empty()).then(|| self.started + SEQUENCE_TIMEOUT)
    }

    /// The keys of the pending sequence for the status line, empty when none is pending.
    #[must_use]
    pub fn pending(&self) -> String {
        sequence_label(&self.pending)
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyEvent, KeyEventKind, KeyEventState};
    use jiff::civil::Date;

    use super::*;
    use crate::{
        app::{
            Location, NavigatorInput, NewBlock, PaneChange, SelectionChange, Sibling, SpecialView,
            Symbols,
        },
        document::{DateChange, DateTag, Flag, TagColor, format::Typed, inline::Marker},
    };

    impl Keymap {
        /// The message `event` sends the core, if any.
        fn message_for(
            &mut self,
            event: &Event,
            now: Duration,
            context: &Context,
        ) -> Option<Message> {
            self.action_for(event, now, context)
                .and_then(|action| match action {
                    KeyAction::Send(message) => Some(message),
                    KeyAction::ToggleWrap
                    | KeyAction::GoToCommand
                    | KeyAction::ChooseTheme
                    | KeyAction::OpenSettings
                    | KeyAction::SetAnimations(_)
                    | KeyAction::StartWeather
                    | KeyAction::NarrowPane
                    | KeyAction::WidenPane
                    | KeyAction::ScrollShortcutsDown
                    | KeyAction::ScrollShortcutsUp
                    | KeyAction::ScrollAttachments(_) => None,
                })
        }
    }

    /// How the sequence `keys`, in the config file's notation, reads on screen.
    fn label(keys: &str) -> Option<String> {
        parse_sequence(keys).map(|keys| sequence_label(&keys))
    }

    const PAGE: usize = 7;

    const PAGE_UP: usize = 5;

    const TODAY: Date = jiff::civil::date(2026, 10, 5);

    const NOW: DateTime = TODAY.at(9, 5, 0, 0);

    const CONTEXT: &Context = &Context {
        page_down_rows: PAGE,
        page_up_rows: PAGE_UP,
        now: NOW,
        mode: Mode::Navigate,
        command_menu: false,
        overlay: None,
        shortcuts: false,
        navigator: false,
    };

    const EDITING: &Context = &Context {
        mode: Mode::Edit,
        ..*CONTEXT
    };

    fn key(code: KeyCode, modifiers: KeyModifiers) -> Event {
        Event::Key(KeyEvent::new(code, modifiers))
    }

    fn ms(millis: u64) -> Duration {
        Duration::from_millis(millis)
    }

    /// The message each key sends from a fresh keymap.
    #[track_caller]
    fn check(code: KeyCode, modifiers: KeyModifiers, expected: Option<&Message>) {
        let mut keymap = Keymap::default();
        assert_eq!(
            keymap
                .message_for(&key(code, modifiers), ms(0), CONTEXT)
                .as_ref(),
            expected,
            "{modifiers:?} {code:?}"
        );
    }

    #[test]
    fn ctrl_q_quits() {
        check(
            KeyCode::Char('q'),
            KeyModifiers::CONTROL,
            Some(&Message::Quit),
        );
    }

    #[test]
    fn q_without_ctrl_or_with_more_modifiers_does_nothing() {
        check(KeyCode::Char('q'), KeyModifiers::NONE, None);
        check(
            KeyCode::Char('q'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
            None,
        );
        check(KeyCode::Char('w'), KeyModifiers::CONTROL, None);
    }

    // FR-NAV-01
    #[test]
    fn moving_keys_send_their_motions() {
        let none = KeyModifiers::NONE;
        let next = Some(&Message::FocusNext);
        let previous = Some(&Message::FocusPrevious);
        let last = Some(&Message::FocusLast);
        check(KeyCode::Char('j'), none, next);
        check(KeyCode::Down, none, next);
        check(KeyCode::Char('k'), none, previous);
        check(KeyCode::Up, none, previous);
        check(KeyCode::Home, none, Some(&Message::FocusFirst));
        check(KeyCode::End, none, last);
        check(KeyCode::Char('G'), KeyModifiers::SHIFT, last);
        check(KeyCode::Char('G'), none, last);
    }

    // FR-NAV-01, keybindings.md §6: `gj`/`gk` beside `Ctrl+↓`/`Ctrl+↑`, which macOS takes.
    #[test]
    fn ctrl_arrows_and_g_j_g_k_move_to_the_next_and_previous_sibling() {
        let next = Some(&Message::FocusSibling(Sibling::Next));
        let previous = Some(&Message::FocusSibling(Sibling::Previous));
        check(KeyCode::Down, KeyModifiers::CONTROL, next);
        check(KeyCode::Up, KeyModifiers::CONTROL, previous);
        assert_eq!(messages("gj", CONTEXT), [None, next.cloned()]);
        assert_eq!(messages("gk", CONTEXT), [None, previous.cloned()]);
        assert_eq!(messages("j", CONTEXT), [Some(Message::FocusNext)]);
    }

    // FR-NAV-01, D-118, D-42: plain characters both key reportings send; the kitty protocol may
    // add `Shift` to them.
    #[test]
    fn caret_dollar_and_hash_go_to_the_first_and_last_sibling_and_the_parent() {
        for (c, message) in [
            ('^', Message::FocusSibling(Sibling::First)),
            ('$', Message::FocusSibling(Sibling::Last)),
            ('#', Message::FocusParent),
        ] {
            check(KeyCode::Char(c), KeyModifiers::NONE, Some(&message));
            check(KeyCode::Char(c), KeyModifiers::SHIFT, Some(&message));
        }
        assert_eq!(
            Keymap::default().bindings(Command::FocusLastChild, true),
            "",
            "Last Child is left to the command palette"
        );
    }

    // D-118, D-42: plain characters, so both key reportings send them alike.
    #[test]
    fn g_l_and_g_p_focus_the_pane_left_and_g_r_and_g_n_the_pane_right() {
        for (keys, change) in [
            ("gl", PaneChange::FocusLeft),
            ("gp", PaneChange::FocusLeft),
            ("gr", PaneChange::FocusRight),
            ("gn", PaneChange::FocusRight),
        ] {
            assert_eq!(
                messages(keys, CONTEXT),
                [None, Some(Message::Pane(change))],
                "{keys}"
            );
        }
    }

    // FR-NAV-01
    #[test]
    fn page_keys_carry_the_page_height() {
        let down = Some(&Message::PageDown { rows: PAGE });
        let up = Some(&Message::PageUp { rows: PAGE_UP });
        check(KeyCode::PageDown, KeyModifiers::NONE, down);
        check(KeyCode::Char('d'), KeyModifiers::CONTROL, down);
        check(KeyCode::PageUp, KeyModifiers::NONE, up);
        check(KeyCode::Char('u'), KeyModifiers::CONTROL, up);
    }

    // FR-DONE-01, FR-OPS-08
    #[test]
    fn editing_keys_complete_undo_and_redo() {
        let none = KeyModifiers::NONE;
        let ctrl = KeyModifiers::CONTROL;
        let done = Some(&Message::ToggleDone { today: TODAY });
        check(KeyCode::Char('x'), none, done);
        check(KeyCode::Enter, ctrl, done);
        check(KeyCode::Char('z'), none, Some(&Message::Undo));
        check(KeyCode::Char('z'), ctrl, Some(&Message::Undo));
        check(
            KeyCode::Char('Z'),
            KeyModifiers::SHIFT,
            Some(&Message::Redo),
        );
        check(KeyCode::Char('y'), ctrl, Some(&Message::Redo));
    }

    // FR-TAG-06, D-26, D-42: plain characters, which both key reportings send alike; the kitty
    // protocol may add `Shift` to shifted ones.
    #[test]
    fn date_keys_set_and_move_the_start_and_due_dates() {
        for (c, tag, change) in [
            ('t', DateTag::Start, DateChange::Today),
            ('w', DateTag::Start, DateChange::Tomorrow),
            ('T', DateTag::Due, DateChange::Today),
            ('W', DateTag::Due, DateChange::Tomorrow),
            ('[', DateTag::Start, DateChange::Earlier),
            (']', DateTag::Start, DateChange::Later),
            ('{', DateTag::Due, DateChange::Earlier),
            ('}', DateTag::Due, DateChange::Later),
        ] {
            let expected = Message::ChangeDate {
                tag,
                change,
                today: TODAY,
            };
            check(KeyCode::Char(c), KeyModifiers::NONE, Some(&expected));
            if matches!(c, 'T' | 'Y' | '{' | '}') {
                check(KeyCode::Char(c), KeyModifiers::SHIFT, Some(&expected));
            }
            check_editing(
                KeyCode::Char(c),
                KeyModifiers::NONE,
                Some(&Message::Text(TextEdit::Insert(c))),
            );
        }
        check(
            KeyCode::Char('t'),
            KeyModifiers::SHIFT,
            Some(&Message::ChangeDate {
                tag: DateTag::Due,
                change: DateChange::Today,
                today: TODAY,
            }),
        );
    }

    // FR-TAG-03, D-42: plain letters, which both key reportings send alike.
    #[test]
    fn m_s_and_m_d_open_the_date_prompt() {
        for (second, tag) in [('s', DateTag::Start), ('d', DateTag::Due)] {
            let mut keymap = Keymap::default();
            let m = key(KeyCode::Char('m'), KeyModifiers::NONE);
            assert_eq!(keymap.message_for(&m, ms(0), CONTEXT), None);
            assert_eq!(keymap.pending(), "m");
            assert_eq!(
                keymap.message_for(
                    &key(KeyCode::Char(second), KeyModifiers::NONE),
                    ms(1),
                    CONTEXT
                ),
                Some(Message::OpenDatePrompt { tag, today: TODAY })
            );
        }
    }

    const IN_DATE_PROMPT: &Context = &Context {
        overlay: Some(Overlay::DatePrompt),
        ..*CONTEXT
    };

    // FR-TAG-03, keybindings.md §3: prompts take the standard text keys, `Enter` and `Esc`.
    #[test]
    fn the_date_prompt_takes_text_keys_enter_and_esc() {
        let none = KeyModifiers::NONE;
        let ctrl = KeyModifiers::CONTROL;
        let input = |input| Some(Message::DatePrompt(input));
        for (code, modifiers, expected) in [
            (KeyCode::Enter, none, input(DateInput::Apply)),
            (KeyCode::Esc, none, input(DateInput::Cancel)),
            (
                KeyCode::Char('j'),
                none,
                input(DateInput::Edit(TextEdit::Insert('j'))),
            ),
            (
                KeyCode::Char('+'),
                none,
                input(DateInput::Edit(TextEdit::Insert('+'))),
            ),
            (
                KeyCode::Left,
                none,
                input(DateInput::Edit(go!(Left, false))),
            ),
            (
                KeyCode::Home,
                KeyModifiers::SHIFT,
                input(DateInput::Edit(go!(Start, true))),
            ),
            (
                KeyCode::Backspace,
                none,
                input(DateInput::Edit(TextEdit::DeleteBackward)),
            ),
            (
                KeyCode::Char('w'),
                ctrl,
                input(DateInput::Edit(TextEdit::DeleteWordBackward)),
            ),
            (KeyCode::Char('q'), ctrl, Some(Message::Quit)),
            (KeyCode::Char('v'), ctrl, Some(Message::Paste(Paste::Below))),
            (KeyCode::Tab, none, None),
            (KeyCode::Up, none, None),
            (KeyCode::Char('z'), ctrl, None),
        ] {
            let mut keymap = Keymap::default();
            assert_eq!(
                keymap.message_for(&key(code, modifiers), ms(0), IN_DATE_PROMPT),
                expected,
                "{modifiers:?} {code:?}"
            );
        }
        let mut keymap = Keymap::default();
        keymap.message_for(&key(KeyCode::Char('g'), none), ms(0), CONTEXT);
        keymap.message_for(&key(KeyCode::Char('g'), none), ms(1), IN_DATE_PROMPT);
        assert_eq!(keymap.pending(), "", "the prompt ends a pending sequence");
    }

    // D-42: the kitty protocol sends `Ctrl+Shift+Z` as `z` with `Shift`, other reportings as `Z`.
    #[test]
    fn ctrl_shift_z_redoes_in_both_key_reportings() {
        let ctrl_shift = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
        for code in [KeyCode::Char('Z'), KeyCode::Char('z')] {
            check(code, ctrl_shift, Some(&Message::Redo));
            check_editing(code, ctrl_shift, Some(&Message::Redo));
        }
    }

    // FR-MODE-01
    #[test]
    fn enter_alone_starts_editing() {
        check(
            KeyCode::Enter,
            KeyModifiers::NONE,
            Some(&Message::EditAtEnd),
        );
    }

    // FR-MODE-04, FR-OPS-06
    #[test]
    fn creating_and_deleting_keys() {
        let none = KeyModifiers::NONE;
        check(KeyCode::Char('o'), KeyModifiers::ALT, None);
        check(KeyCode::Delete, none, Some(&Message::Delete));
        check(
            KeyCode::Char('D'),
            KeyModifiers::SHIFT,
            Some(&Message::Duplicate),
        );
        let mut keymap = Keymap::default();
        let d = key(KeyCode::Char('d'), none);
        assert_eq!(keymap.message_for(&d, ms(0), CONTEXT), None);
        assert_eq!(keymap.pending(), "d");
        assert_eq!(
            keymap.message_for(&d, ms(1), CONTEXT),
            Some(Message::Delete)
        );
    }

    /// The creation key `c`, pressed as a terminal reports it.
    fn creation(c: char) -> Event {
        let shift = if c.is_ascii_uppercase() {
            KeyModifiers::SHIFT
        } else {
            KeyModifiers::NONE
        };
        key(KeyCode::Char(c), shift)
    }

    /// Where each creation key adds a block (D-41, D-53).
    const PLACES: [(char, NewBlock); 6] = [
        ('o', NewBlock::Below),
        ('O', NewBlock::Above),
        ('i', NewBlock::LastChild),
        ('I', NewBlock::FirstChild),
        ('a', NewBlock::End),
        ('A', NewBlock::Start),
    ];

    // FR-MODE-04, FR-MODE-06, D-53
    #[test]
    fn a_creation_key_then_a_type_letter_adds_a_block_of_that_type() {
        for (c, at) in PLACES {
            for (letter, typed) in [
                ('s', Typed::Section),
                ('p', Typed::Project),
                ('t', Typed::Task),
                ('n', Typed::Text),
            ] {
                let mut keymap = Keymap::default();
                assert_eq!(keymap.message_for(&creation(c), ms(0), CONTEXT), None);
                assert_eq!(keymap.pending(), c.to_string());
                let next = key(KeyCode::Char(letter), KeyModifiers::NONE);
                assert_eq!(
                    keymap.message_for(&next, ms(999), CONTEXT),
                    Some(Message::New(at, Some(typed))),
                    "{c}{letter}"
                );
                assert!(!keymap.replay(), "{c}{letter}");
                assert_eq!(keymap.pending(), "");
            }
        }
    }

    // FR-MODE-04, D-53
    #[test]
    fn a_lone_creation_key_adds_a_block_of_the_fitting_type_once_the_sequence_times_out() {
        for (c, at) in PLACES {
            let mut keymap = Keymap::default();
            keymap.message_for(&creation(c), ms(0), CONTEXT);
            assert_eq!(keymap.expire(ms(999), CONTEXT), None, "{c}");
            assert_eq!(keymap.pending(), c.to_string());
            assert_eq!(
                keymap.expire(ms(1_000), CONTEXT),
                Some(KeyAction::Send(Message::New(at, None))),
                "{c}"
            );
            assert_eq!(keymap.pending(), "");
            assert_eq!(keymap.expire(ms(2_000), CONTEXT), None, "once");
        }
    }

    // FR-MODE-04, FR-OPS-06, D-53
    #[test]
    fn text_pasted_after_a_lone_creation_key_adds_the_block_and_is_pasted_again() {
        let mut keymap = Keymap::default();
        let pasted = Event::Paste("a".to_owned());
        keymap.message_for(&creation('o'), ms(0), CONTEXT);
        assert_eq!(
            keymap.message_for(&pasted, ms(10), CONTEXT),
            Some(Message::New(NewBlock::Below, None))
        );
        assert!(keymap.replay());
        keymap.message_for(
            &key(KeyCode::Char('g'), KeyModifiers::NONE),
            ms(20),
            CONTEXT,
        );
        assert!(keymap.message_for(&pasted, ms(30), CONTEXT).is_some());
        assert!(!keymap.replay());
        assert_eq!(keymap.pending(), "", "the paste ends any sequence");
    }

    // FR-MODE-04, D-53
    #[test]
    fn a_key_that_continues_no_creation_sequence_adds_the_block_and_is_handled_again() {
        let mut keymap = Keymap::default();
        keymap.message_for(&creation('o'), ms(0), CONTEXT);
        let b = key(KeyCode::Char('B'), KeyModifiers::SHIFT);
        assert_eq!(
            keymap.message_for(&b, ms(10), CONTEXT),
            Some(Message::New(NewBlock::Below, None))
        );
        assert!(keymap.replay(), "B is still to be typed");
        assert!(!keymap.replay(), "reading it clears it");
        assert_eq!(keymap.pending(), "");
        keymap.message_for(&creation('i'), ms(20), CONTEXT);
        let j = key(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(
            keymap.message_for(&j, ms(2_000), CONTEXT),
            Some(Message::New(NewBlock::LastChild, None)),
            "a sequence that timed out unnoticed"
        );
        assert!(keymap.replay());
        assert_eq!(
            keymap.message_for(&j, ms(2_000), CONTEXT),
            Some(Message::FocusNext)
        );
        assert!(!keymap.replay());
    }

    // FR-BLK-04, D-52
    #[test]
    fn upper_and_lower_s_make_and_unmake_sections() {
        let make = Some(&Message::SetSection(true));
        check(KeyCode::Char('S'), KeyModifiers::SHIFT, make);
        check(KeyCode::Char('S'), KeyModifiers::NONE, make);
        let unmake = Some(&Message::SetSection(false));
        check(KeyCode::Char('s'), KeyModifiers::NONE, unmake);
    }

    // FR-OPS-02, FR-OPS-03
    #[test]
    fn structure_keys_indent_outdent_and_move() {
        let alt = KeyModifiers::ALT;
        let up = Some(&Message::MoveUp);
        let down = Some(&Message::MoveDown);
        check(KeyCode::Tab, KeyModifiers::NONE, Some(&Message::Indent));
        check(
            KeyCode::BackTab,
            KeyModifiers::SHIFT,
            Some(&Message::Outdent),
        );
        check(KeyCode::Char('k'), alt, up);
        check(KeyCode::Up, alt, up);
        check(KeyCode::Char('j'), alt, down);
        check(KeyCode::Down, alt, down);
    }

    // FR-KEY-04, D-42, D-55: plain characters, which both key reportings send alike.
    #[test]
    fn parentheses_scroll_the_shortcuts_panel_in_navigate_mode() {
        let press = |c| key(KeyCode::Char(c), KeyModifiers::NONE);
        let mut keymap = Keymap::default();
        assert_eq!(
            keymap.action_for(&press(')'), ms(0), CONTEXT),
            Some(KeyAction::ScrollShortcutsDown)
        );
        assert_eq!(
            keymap.action_for(&press('('), ms(1), CONTEXT),
            Some(KeyAction::ScrollShortcutsUp)
        );
        assert_eq!(
            keymap.message_for(&press(')'), ms(2), EDITING),
            Some(Message::Text(TextEdit::Insert(')'))),
            "edit mode types them"
        );
    }

    // FR-UI-05, D-42
    #[test]
    fn alt_z_and_g_w_toggle_word_wrap() {
        let mut keymap = Keymap::default();
        let alt_z = key(KeyCode::Char('z'), KeyModifiers::ALT);
        assert_eq!(
            keymap.action_for(&alt_z, ms(0), CONTEXT),
            Some(KeyAction::ToggleWrap)
        );
        let press = |c| key(KeyCode::Char(c), KeyModifiers::NONE);
        assert_eq!(keymap.action_for(&press('g'), ms(0), CONTEXT), None);
        assert_eq!(
            keymap.action_for(&press('w'), ms(1), CONTEXT),
            Some(KeyAction::ToggleWrap)
        );
        assert_eq!(keymap.action_for(&alt_z, ms(2), EDITING), None);
        assert_eq!(
            keymap.message_for(&alt_z, ms(3), CONTEXT),
            None,
            "the core is not told"
        );
    }

    // FR-DONE-02
    #[test]
    fn ctrl_o_and_g_c_show_or_hide_completed_blocks() {
        check(
            KeyCode::Char('o'),
            KeyModifiers::CONTROL,
            Some(&Message::ToggleShowDone),
        );
        assert_eq!(
            messages("gc", CONTEXT),
            [None, Some(Message::ToggleShowDone)]
        );
    }

    // FR-KEY-04, D-42: both key reportings send `?`, the kitty protocol with `Shift`.
    #[test]
    fn question_mark_toggles_the_shortcuts_panel_and_esc_closes_it() {
        let toggle = Some(&Message::ToggleShortcuts);
        check(KeyCode::Char('?'), KeyModifiers::NONE, toggle);
        check(KeyCode::Char('?'), KeyModifiers::SHIFT, toggle);
        let open = &Context {
            shortcuts: true,
            ..*CONTEXT
        };
        let mut keymap = Keymap::default();
        let esc = key(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(
            keymap.message_for(&esc, ms(0), open),
            Some(Message::CloseShortcuts)
        );
        assert_eq!(
            keymap.message_for(&esc, ms(0), CONTEXT),
            Some(Message::Select(SelectionChange::Clear)),
            "without the panel, Esc clears the selection"
        );
        let press = |c| key(KeyCode::Char(c), KeyModifiers::NONE);
        assert_eq!(keymap.message_for(&press('g'), ms(0), open), None);
        assert_eq!(
            keymap.message_for(&esc, ms(1), open),
            None,
            "Esc ends a pending sequence first"
        );
        assert_eq!(messages("?", open), [Some(Message::ToggleShortcuts)]);
        assert_eq!(
            Keymap::default().message_for(
                &esc,
                ms(0),
                &Context {
                    mode: Mode::Edit,
                    ..*open
                }
            ),
            Some(Message::StopEditing),
            "in edit mode Esc stops editing"
        );
        assert_eq!(
            messages(
                "?",
                &Context {
                    mode: Mode::Edit,
                    ..*open
                }
            ),
            [Some(text(TextEdit::Insert('?')))],
            "in edit mode `?` is typed"
        );
    }

    // FR-KEY-04
    #[test]
    fn labels_write_keys_as_the_key_bindings_do() {
        for (keys, expected) in [
            ("j", "j"),
            ("G", "G"),
            ("g g", "gg"),
            ("o s", "os"),
            ("space", "Space"),
            ("' space", "' Space"),
            ("ctrl+q", "Ctrl+Q"),
            ("ctrl+'", "Ctrl+'"),
            ("alt+k", "Alt+k"),
            ("alt+`", "Alt+`"),
            ("ctrl+shift+z", "Ctrl+Shift+Z"),
            ("alt+shift+left", "Alt+Shift+←"),
            ("shift+down", "Shift+↓"),
            ("shift+tab", "Shift+Tab"),
            ("up", "↑"),
            ("right", "→"),
            ("ctrl+down", "Ctrl+↓"),
            ("pagedown", "PgDn"),
            ("pageup", "PgUp"),
            ("delete", "Delete"),
            ("backspace", "Backspace"),
            ("enter", "Enter"),
            ("ctrl+enter", "Ctrl+Enter"),
            ("esc", "Esc"),
            ("home", "Home"),
            ("+", "+"),
            ("ctrl+w v", "Ctrl+W v"),
            ("?", "?"),
            ("f1", "F1"),
            ("ctrl+f12", "Ctrl+F12"),
            ("menu", "Menu"),
        ] {
            assert_eq!(label(keys).as_deref(), Some(expected), "{keys}");
        }
        assert_eq!(label("hyper+x"), None);
        assert_eq!(label("g nothing"), None);
    }

    #[test]
    fn every_registry_binding_has_a_label() {
        for entry in REGISTRY {
            for keys in entry.keys {
                assert!(label(keys).is_some(), "{keys}");
            }
        }
    }

    /// Whether only the kitty keyboard protocol reports `key` as the keymap matches it
    /// ([keybindings.md §1](../../../docs/specs/keybindings.md#1-terminal-constraints)): legacy
    /// reporting sends `Ctrl+Enter` and `Shift+Enter` as `Enter`, `Ctrl+Shift+<letter>` as
    /// `Ctrl+<letter>`, `Ctrl+H`, `Ctrl+I`, `Ctrl+M` and `Ctrl+[` as `Backspace`, `Tab`, `Enter` and
    /// `Esc`, `Ctrl` with other characters as other keys or not at all, and no Menu key.
    fn kitty_only((modifiers, code): Key) -> bool {
        match code {
            KeyCode::Menu => true,
            KeyCode::Enter | KeyCode::Tab | KeyCode::Backspace | KeyCode::Esc => {
                modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::SHIFT)
            }
            KeyCode::Char(c) if modifiers.contains(KeyModifiers::CONTROL) => {
                !c.is_ascii_lowercase() || matches!(c, 'h' | 'i' | 'm')
            }
            _ => false,
        }
    }

    // D-42, FR-KEY-03, keybindings.md §1: every command bound to a key only the kitty protocol
    // reports also has a classic variant, which legacy reporting sends and is not an `Alt` key,
    // and the kitty-only keys are the ones keybindings.md §1 lists with their classic variants.
    #[test]
    fn every_kitty_only_binding_has_a_classic_variant_beside_it() {
        let bindings = REGISTRY
            .iter()
            .map(|entry| (entry.command, entry.keys))
            .chain(NAVIGATOR.iter().copied());
        let mut kitty = Vec::new();
        for (command, keys) in bindings {
            let sequences: Vec<Vec<Key>> = keys
                .iter()
                .map(|keys| parse_sequence(keys).expect("registry keys parse"))
                .collect();
            let classic = sequences.iter().any(|sequence| {
                sequence.iter().all(|&(modifiers, code)| {
                    !kitty_only((modifiers, code)) && !modifiers.contains(KeyModifiers::ALT)
                })
            });
            for (keys, sequence) in keys.iter().zip(&sequences) {
                assert!(
                    sequence.iter().all(|&(modifiers, _)| {
                        (KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT)
                            .contains(modifiers)
                    }),
                    "{keys} of {command:?} takes no Cmd or Super"
                );
                if sequence.iter().any(|&key| kitty_only(key)) {
                    assert!(classic, "{keys} of {command:?} has a classic variant");
                    kitty.push(*keys);
                }
            }
        }
        kitty.sort_unstable();
        kitty.dedup();
        assert_eq!(
            kitty,
            ["ctrl+'", "ctrl+\\", "ctrl+enter", "ctrl+shift+z", "menu"]
        );
    }

    // D-42, keybindings.md §1 and §4: the keys of text fields, menus, pickers, the settings
    // window, the prompts, Quick Add and the conflict prompt are all sent alike by both key
    // reportings.
    #[test]
    fn every_key_of_the_other_contexts_is_both_variants_at_once() {
        let keys = TEXT
            .iter()
            .map(|&(key, _)| key)
            .chain(MENU.iter().map(|&(key, _)| key))
            .chain(SETTINGS.iter().map(|(key, _)| *key))
            .chain(SETTINGS_TEXT.iter().map(|(key, _)| *key))
            .chain(DATE_PROMPT.iter().map(|&(key, _)| key))
            .chain(QUICK_ADD.iter().map(|&(key, _)| key))
            .chain(SAVE_AS.iter().map(|(key, _)| *key))
            .chain(PROMPT.iter().map(|&(key, _)| key))
            .chain(OPEN_IN_NEW_PANE);
        for key in keys {
            assert!(!kitty_only(key), "{}", key_label(key));
        }
    }

    /// What takes `key` for itself by default before the app sees it, in a terminal, multiplexer
    /// or desktop that [keybindings.md §6](../../../docs/specs/keybindings.md#6-validation)
    /// names, if one does. A key such a program lets through when pressed twice, as tmux does its
    /// prefix `Ctrl+B`, is not taken.
    fn taken_by((modifiers, code): Key) -> Option<&'static str> {
        let ctrl = KeyModifiers::CONTROL;
        let ctrl_shift = ctrl | KeyModifiers::SHIFT;
        let alt = KeyModifiers::ALT;
        let alt_shift = alt | KeyModifiers::SHIFT;
        let arrow = matches!(
            code,
            KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right
        );
        match code {
            _ if arrow && modifiers == ctrl => Some("macOS Mission Control and Spaces"),
            KeyCode::Left | KeyCode::Right if modifiers == ctrl_shift => {
                Some("kitty's tabs and WezTerm's panes")
            }
            _ if arrow && (modifiers == alt || modifiers == alt_shift) => {
                Some("Windows Terminal's split panes")
            }
            KeyCode::Enter if modifiers == alt => {
                Some("Windows Terminal's and WezTerm's full screen")
            }
            KeyCode::Char('v') if modifiers == ctrl => Some("Windows Terminal's paste"),
            KeyCode::Char('Z') if modifiers == ctrl => Some("kitty's and WezTerm's own keys"),
            KeyCode::F(10) if modifiers == KeyModifiers::SHIFT => Some("GNOME Terminal's menu"),
            KeyCode::Char('1'..='9') if modifiers == alt => {
                Some("GNOME Terminal's and Ghostty's tabs on Linux")
            }
            _ => None,
        }
    }

    // keybindings.md §6, D-42: every bound command, and opening a choice in a new pane from Go to
    // Symbol, has a sequence of keys that both key reportings send and no common terminal takes
    // for itself, since a terminal that takes a key may not speak the kitty protocol. Pasting is
    // the exception: a terminal that takes `Ctrl+V` pastes the clipboard itself, which the app
    // takes as pasted text.
    #[test]
    fn every_command_has_keys_no_common_terminal_takes() {
        let bindings = REGISTRY
            .iter()
            .filter(|entry| !matches!(entry.command, Command::Paste(_)))
            .map(|entry| (format!("{:?} {:?}", entry.command, entry.mode), entry.keys))
            .chain(
                NAVIGATOR
                    .iter()
                    .map(|&(command, keys)| (format!("{command:?} in the Navigator"), keys)),
            );
        for (command, keys) in bindings.filter(|(_, keys)| !keys.is_empty()) {
            assert!(
                keys.iter().any(|keys| {
                    parse_sequence(keys)
                        .expect("registry keys parse")
                        .into_iter()
                        .all(|key| taken_by(key).is_none() && !kitty_only(key))
                }),
                "{command}: {keys:?}"
            );
        }
        assert!(
            OPEN_IN_NEW_PANE
                .iter()
                .any(|&key| taken_by(key).is_none() && !kitty_only(key)),
            "Go to Symbol's new pane"
        );
        for (keys, taken) in [
            ("ctrl+down", Some("macOS Mission Control and Spaces")),
            ("ctrl+shift+left", Some("kitty's tabs and WezTerm's panes")),
            ("alt+up", Some("Windows Terminal's split panes")),
            ("alt+shift+right", Some("Windows Terminal's split panes")),
            (
                "alt+enter",
                Some("Windows Terminal's and WezTerm's full screen"),
            ),
            ("ctrl+v", Some("Windows Terminal's paste")),
            ("ctrl+shift+z", Some("kitty's and WezTerm's own keys")),
            ("shift+f10", Some("GNOME Terminal's menu")),
            (
                "alt+9",
                Some("GNOME Terminal's and Ghostty's tabs on Linux"),
            ),
            ("ctrl+b", None),
            ("ctrl+t", None),
            ("alt+k", None),
        ] {
            assert_eq!(taken_by(parse_key(keys).expect("a key")), taken, "{keys}");
        }
    }

    // FR-BLK-06
    #[test]
    fn g_x_opens_the_url() {
        let mut keymap = Keymap::default();
        let press = |c| key(KeyCode::Char(c), KeyModifiers::NONE);
        assert_eq!(keymap.message_for(&press('g'), ms(0), CONTEXT), None);
        assert_eq!(
            keymap.message_for(&press('x'), ms(1), CONTEXT),
            Some(Message::OpenUrl)
        );
    }

    /// The message each key sends in edit mode.
    #[track_caller]
    fn check_editing(code: KeyCode, modifiers: KeyModifiers, expected: Option<&Message>) {
        let mut keymap = Keymap::default();
        assert_eq!(
            keymap
                .message_for(&key(code, modifiers), ms(0), EDITING)
                .as_ref(),
            expected,
            "{modifiers:?} {code:?}"
        );
    }

    fn text(edit: TextEdit) -> Message {
        Message::Text(edit)
    }

    // FR-MODE-01, FR-MODE-03
    #[test]
    fn characters_are_typed_in_edit_mode() {
        let none = KeyModifiers::NONE;
        for c in ['j', 'x', 'g', 'z', 'i', '@', 'é', ' '] {
            check_editing(KeyCode::Char(c), none, Some(&text(TextEdit::Insert(c))));
        }
        check_editing(
            KeyCode::Char('A'),
            KeyModifiers::SHIFT,
            Some(&text(TextEdit::Insert('A'))),
        );
        check_editing(
            KeyCode::Char('@'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
            Some(&text(TextEdit::Insert('@'))),
        );
        check_editing(KeyCode::Char('k'), KeyModifiers::CONTROL, None);
        check_editing(KeyCode::Char('x'), KeyModifiers::ALT, None);
        check_editing(KeyCode::F(1), none, None);
    }

    // FR-MODE-01, FR-MODE-04, FR-OPS-02, D-43
    #[test]
    fn esc_stops_editing_enter_adds_the_next_block_tab_indents_and_ctrl_q_quits() {
        let none = KeyModifiers::NONE;
        check_editing(KeyCode::Tab, none, Some(&Message::Indent));
        check_editing(
            KeyCode::BackTab,
            KeyModifiers::SHIFT,
            Some(&Message::Outdent),
        );
        check_editing(KeyCode::Char('j'), KeyModifiers::ALT, None);
        check_editing(KeyCode::Esc, none, Some(&Message::StopEditing));
        check_editing(KeyCode::Enter, none, Some(&Message::NewNext));
        for modifiers in [KeyModifiers::SHIFT, KeyModifiers::ALT] {
            check_editing(KeyCode::Enter, modifiers, None);
        }
        check_editing(KeyCode::Char('j'), KeyModifiers::CONTROL, None);
        check_editing(
            KeyCode::Char('t'),
            KeyModifiers::CONTROL,
            Some(&Message::CycleType),
        );
        check_editing(
            KeyCode::Char('q'),
            KeyModifiers::CONTROL,
            Some(&Message::Quit),
        );
    }

    // FR-MODE-03
    #[test]
    fn text_keys_move_select_and_delete() {
        let none = KeyModifiers::NONE;
        let shift = KeyModifiers::SHIFT;
        let ctrl = KeyModifiers::CONTROL;
        let alt = KeyModifiers::ALT;
        let to = |motion, select| text(TextEdit::Move { motion, select });
        for (code, modifiers, expected) in [
            (KeyCode::Left, none, to(Motion::Left, false)),
            (KeyCode::Right, none, to(Motion::Right, false)),
            (KeyCode::Left, ctrl, to(Motion::WordLeft, false)),
            (KeyCode::Right, ctrl, to(Motion::WordRight, false)),
            (KeyCode::Char('b'), alt, to(Motion::WordLeft, false)),
            (KeyCode::Char('f'), alt, to(Motion::WordRight, false)),
            (KeyCode::Home, none, to(Motion::Start, false)),
            (KeyCode::End, none, to(Motion::End, false)),
            (KeyCode::Left, shift, to(Motion::Left, true)),
            (KeyCode::Right, shift, to(Motion::Right, true)),
            (KeyCode::Left, ctrl | shift, to(Motion::WordLeft, true)),
            (KeyCode::Right, ctrl | shift, to(Motion::WordRight, true)),
            (KeyCode::Home, shift, to(Motion::Start, true)),
            (KeyCode::End, shift, to(Motion::End, true)),
            (KeyCode::Up, none, Message::EditPrevious),
            (KeyCode::Down, none, Message::EditNext),
            (KeyCode::Up, shift, to(Motion::Start, true)),
            (KeyCode::Down, shift, to(Motion::End, true)),
            (KeyCode::Backspace, none, text(TextEdit::DeleteBackward)),
            (KeyCode::Delete, none, text(TextEdit::DeleteForward)),
            (KeyCode::Char('w'), ctrl, text(TextEdit::DeleteWordBackward)),
            (KeyCode::Backspace, alt, text(TextEdit::DeleteWordBackward)),
            (KeyCode::Char('a'), ctrl, text(TextEdit::SelectAll)),
            (KeyCode::Char('z'), ctrl, Message::Undo),
            (KeyCode::Char('y'), ctrl, Message::Redo),
        ] {
            check_editing(code, modifiers, Some(&expected));
        }
    }

    // FR-FMT-02
    #[test]
    fn formatting_keys_toggle_bold_and_inline_code_in_edit_mode() {
        let bold = text(TextEdit::Toggle(Marker::Bold));
        let code = text(TextEdit::Toggle(Marker::Code));
        check_editing(KeyCode::Char('b'), KeyModifiers::CONTROL, Some(&bold));
        check_editing(KeyCode::Char('`'), KeyModifiers::ALT, Some(&code));
        check_editing(KeyCode::Char('e'), KeyModifiers::CONTROL, Some(&code));
        check(KeyCode::Char('b'), KeyModifiers::CONTROL, None);
        check(KeyCode::Char('`'), KeyModifiers::ALT, None);
    }

    const IN_MENU: &Context = &Context {
        command_menu: true,
        ..*EDITING
    };

    /// The message each key sends while the command menu is open.
    #[track_caller]
    fn check_menu(code: KeyCode, modifiers: KeyModifiers, expected: Option<MenuInput>) {
        let mut keymap = Keymap::default();
        assert_eq!(
            keymap.message_for(&key(code, modifiers), ms(0), IN_MENU),
            expected.map(|input| Message::Menu(Menu::CommandMenu, input)),
            "{modifiers:?} {code:?}"
        );
    }

    // D-50
    #[test]
    fn slash_opens_the_command_menu_in_edit_mode_dated_for_its_commands() {
        check_editing(
            KeyCode::Char('/'),
            KeyModifiers::NONE,
            Some(&Message::OpenCommandMenu { today: TODAY }),
        );
    }

    // D-44, D-50, keybindings.md §3
    #[test]
    fn the_command_menu_takes_the_keys_of_pickers() {
        let none = KeyModifiers::NONE;
        let ctrl = KeyModifiers::CONTROL;
        check_menu(KeyCode::Up, none, Some(MenuInput::Previous));
        check_menu(KeyCode::Char('p'), ctrl, Some(MenuInput::Previous));
        check_menu(KeyCode::Down, none, Some(MenuInput::Next));
        check_menu(KeyCode::Char('n'), ctrl, Some(MenuInput::Next));
        check_menu(KeyCode::Enter, none, Some(MenuInput::Choose));
        check_menu(KeyCode::Esc, none, Some(MenuInput::Dismiss));
        check_menu(KeyCode::Backspace, none, Some(MenuInput::Erase));
        check_menu(KeyCode::Char('s'), none, Some(MenuInput::Type('s')));
        check_menu(KeyCode::Char('/'), none, Some(MenuInput::Type('/')));
        check_menu(
            KeyCode::Char('d'),
            ctrl.union(KeyModifiers::ALT),
            Some(MenuInput::Type('d')),
        );
        check_menu(KeyCode::Left, none, None);
        check_menu(KeyCode::Tab, none, None);
        check_menu(KeyCode::Char('z'), ctrl, None);
        let mut keymap = Keymap::default();
        assert_eq!(
            keymap.message_for(&key(KeyCode::Char('q'), ctrl), ms(0), IN_MENU),
            Some(Message::Quit)
        );
    }

    /// The messages `keys` send one after another from a fresh keymap in `context`.
    fn messages(keys: &str, context: &Context) -> Vec<Option<Message>> {
        let mut keymap = Keymap::default();
        keys.chars()
            .map(|c| keymap.message_for(&key(KeyCode::Char(c), KeyModifiers::NONE), ms(0), context))
            .collect()
    }

    // FR-TAG-04, FR-TAG-05, FR-TAG-06, D-11, D-26, D-42, D-48, D-91: plain characters, which both key
    // reportings send alike, and `C`, which the kitty protocol sends as `c` with `Shift`.
    #[test]
    fn color_star_flag_and_bookmark_keys() {
        check(KeyCode::Char('c'), KeyModifiers::NONE, None);
        assert_eq!(messages("cc", CONTEXT), [None, Some(Message::CycleColor)]);
        for (digit, color) in ('1'..='6').zip(TagColor::ALL) {
            assert_eq!(
                messages(&format!("c{digit}"), CONTEXT),
                [None, Some(Message::SetColor(Some(color)))]
            );
        }
        assert_eq!(
            messages("c7j", CONTEXT),
            [None, None, Some(Message::FocusNext)]
        );
        assert_eq!(
            messages("C", CONTEXT),
            [Some(Message::SetColor(None))],
            "legacy reporting sends C"
        );
        check(
            KeyCode::Char('c'),
            KeyModifiers::SHIFT,
            Some(&Message::SetColor(None)),
        );
        for (second, expected) in [
            ('c', Message::OpenColorPicker),
            ('b', Message::ToggleFlag(Flag::Bookmark)),
            ('f', Message::ToggleFlag(Flag::Flagged)),
        ] {
            assert_eq!(
                messages(&format!("m{second}"), CONTEXT),
                [None, Some(expected)]
            );
        }
        assert_eq!(messages("mx", CONTEXT), [None, None]);
        let flag = Message::ToggleFlag(Flag::Flagged);
        check(KeyCode::Char('!'), KeyModifiers::NONE, Some(&flag));
        check(KeyCode::Char('!'), KeyModifiers::SHIFT, Some(&flag));
        assert_eq!(
            messages("mt", CONTEXT),
            [None, None],
            "was Toggle Today (D-88)"
        );
        for modifiers in [KeyModifiers::NONE, KeyModifiers::SHIFT] {
            check(
                KeyCode::Char('*'),
                modifiers,
                Some(&Message::ToggleFlag(Flag::Star)),
            );
        }
        assert_eq!(
            messages("mc", EDITING),
            [
                Some(Message::Text(TextEdit::Insert('m'))),
                Some(Message::Text(TextEdit::Insert('c')))
            ]
        );
    }

    const IN_PICKER: &Context = &Context {
        overlay: Some(Overlay::ColorPicker),
        ..*CONTEXT
    };

    // FR-TAG-05, keybindings.md §4
    #[test]
    fn the_color_picker_takes_the_keys_of_pickers() {
        let mut keymap = Keymap::default();
        let mut press =
            |code, modifiers, context| keymap.message_for(&key(code, modifiers), ms(0), context);
        let none = KeyModifiers::NONE;
        let picker = |input| Some(Message::Menu(Menu::ColorPicker, input));
        assert_eq!(press(KeyCode::Char('g'), none, CONTEXT), None);
        assert_eq!(
            press(KeyCode::Char('g'), none, IN_PICKER),
            picker(MenuInput::Type('g')),
            "the picker ends a pending sequence"
        );
        assert_eq!(
            press(KeyCode::Char('j'), none, IN_PICKER),
            picker(MenuInput::Type('j'))
        );
        assert_eq!(
            press(KeyCode::Down, none, IN_PICKER),
            picker(MenuInput::Next)
        );
        assert_eq!(
            press(KeyCode::Char('p'), KeyModifiers::CONTROL, IN_PICKER),
            picker(MenuInput::Previous)
        );
        assert_eq!(
            press(KeyCode::Enter, none, IN_PICKER),
            picker(MenuInput::Choose)
        );
        assert_eq!(
            press(KeyCode::Esc, none, IN_PICKER),
            picker(MenuInput::Dismiss)
        );
        assert_eq!(
            press(KeyCode::Backspace, none, IN_PICKER),
            picker(MenuInput::Erase)
        );
        assert_eq!(press(KeyCode::Tab, none, IN_PICKER), None);
        assert_eq!(
            press(KeyCode::Char('q'), KeyModifiers::CONTROL, IN_PICKER),
            Some(Message::Quit)
        );
        assert_eq!(keymap.pending(), "");
    }

    // FR-BLK-08, D-42: plain characters, which both key reportings send alike.
    #[test]
    fn m_y_opens_the_type_picker_which_takes_the_keys_of_pickers() {
        assert_eq!(
            messages("my", CONTEXT),
            [None, Some(Message::OpenTypePicker)]
        );
        let open = &Context {
            overlay: Some(Overlay::TypePicker),
            ..*CONTEXT
        };
        let picker = |input| Some(Message::Menu(Menu::TypePicker, input));
        assert_eq!(messages("t", open), [picker(MenuInput::Type('t'))]);
        let mut keymap = Keymap::default();
        let mut press = |code, modifiers| keymap.message_for(&key(code, modifiers), ms(0), open);
        let none = KeyModifiers::NONE;
        assert_eq!(press(KeyCode::Up, none), picker(MenuInput::Previous));
        assert_eq!(
            press(KeyCode::Char('n'), KeyModifiers::CONTROL),
            picker(MenuInput::Next)
        );
        assert_eq!(press(KeyCode::Enter, none), picker(MenuInput::Choose));
        assert_eq!(press(KeyCode::Esc, none), picker(MenuInput::Dismiss));
        assert_eq!(press(KeyCode::Backspace, none), picker(MenuInput::Erase));
        assert_eq!(
            press(KeyCode::Char('q'), KeyModifiers::CONTROL),
            Some(Message::Quit)
        );
    }

    // editing.md §7, D-42: plain characters, which both key reportings send alike.
    #[test]
    fn e_and_m_i_open_the_edit_node_dialog() {
        let open = Some(Message::NodeEditor(NodeInput::Open));
        assert_eq!(messages("e", CONTEXT), std::slice::from_ref(&open));
        assert_eq!(messages("mi", CONTEXT), [None, open]);
    }

    // editing.md §7, keybindings.md §4, D-42
    #[test]
    fn the_edit_node_dialog_takes_its_keys_and_text_keys_and_its_finder_the_keys_of_a_picker() {
        let none = KeyModifiers::NONE;
        let dialog = |code, modifiers| overlay_message(Overlay::NodeEditor, code, modifiers);
        for (code, modifiers, input) in [
            (KeyCode::Enter, none, NodeInput::Accept),
            (KeyCode::Esc, none, NodeInput::Cancel),
            (KeyCode::Tab, none, NodeInput::Next),
            (KeyCode::Down, none, NodeInput::Next),
            (KeyCode::BackTab, KeyModifiers::SHIFT, NodeInput::Previous),
            (KeyCode::Up, none, NodeInput::Previous),
            (
                KeyCode::Char('t'),
                KeyModifiers::CONTROL,
                NodeInput::CycleType,
            ),
            (
                KeyCode::Char(' '),
                none,
                NodeInput::Key(TextEdit::Insert(' ')),
            ),
            (
                KeyCode::Char('x'),
                none,
                NodeInput::Key(TextEdit::Insert('x')),
            ),
            (
                KeyCode::Backspace,
                none,
                NodeInput::Key(TextEdit::DeleteBackward),
            ),
            (
                KeyCode::Left,
                none,
                NodeInput::Key(TextEdit::Move {
                    motion: Motion::Left,
                    select: false,
                }),
            ),
        ] {
            assert_eq!(
                dialog(code, modifiers),
                Some(Message::NodeEditor(input)),
                "{code:?}"
            );
        }
        assert_eq!(dialog(KeyCode::F(2), none), None);
        assert_eq!(
            dialog(KeyCode::Char('v'), KeyModifiers::CONTROL),
            Some(Message::Paste(Paste::Below))
        );
        let pick = |code| overlay_message(Overlay::NodeEditorFinder, code, none);
        for (code, input) in [
            (KeyCode::Char('w'), MenuInput::Type('w')),
            (KeyCode::Down, MenuInput::Next),
            (KeyCode::Enter, MenuInput::Choose),
            (KeyCode::Esc, MenuInput::Dismiss),
        ] {
            assert_eq!(
                pick(code),
                Some(Message::Menu(Menu::NodeEditor, input)),
                "{code:?}"
            );
        }
        for overlay in [Overlay::NodeEditor, Overlay::NodeEditorFinder] {
            assert_eq!(
                overlay_message(overlay, KeyCode::Char('q'), KeyModifiers::CONTROL),
                Some(Message::Quit),
                "{overlay:?}"
            );
        }
    }

    // attachments.md §3, keybindings.md §4, D-42
    #[test]
    fn the_attachments_panel_takes_its_keys_and_ignores_others() {
        let (none, shift, ctrl) = (
            KeyModifiers::NONE,
            KeyModifiers::SHIFT,
            KeyModifiers::CONTROL,
        );
        let action = |code, modifiers| {
            let context = &Context {
                overlay: Some(Overlay::Attachments),
                ..*CONTEXT
            };
            Keymap::default().action_for(&key(code, modifiers), ms(0), context)
        };
        let send = |input| Some(KeyAction::Send(Message::Attachments(input)));
        for (code, modifiers, expected) in [
            (KeyCode::Esc, none, send(AttachmentsInput::Close)),
            (KeyCode::Char('e'), none, send(AttachmentsInput::Edit)),
            (KeyCode::Enter, none, send(AttachmentsInput::Edit)),
            (KeyCode::Char('a'), none, send(AttachmentsInput::AddComment)),
            (
                KeyCode::Char('d'),
                none,
                send(AttachmentsInput::DeleteComment),
            ),
            (KeyCode::Delete, none, send(AttachmentsInput::DeleteComment)),
            (KeyCode::Tab, none, None),
            (
                KeyCode::Char('q'),
                ctrl,
                Some(KeyAction::Send(Message::Quit)),
            ),
            (KeyCode::Char('x'), none, None),
        ] {
            assert_eq!(action(code, modifiers), expected, "{code:?}");
        }
        let scroll = |by| Some(KeyAction::ScrollAttachments(by));
        for (code, modifiers, by) in [
            (KeyCode::Char('j'), none, ScrollBy::Down),
            (KeyCode::Down, none, ScrollBy::Down),
            (KeyCode::Char('k'), none, ScrollBy::Up),
            (KeyCode::Up, none, ScrollBy::Up),
            (KeyCode::PageDown, none, ScrollBy::PageDown),
            (KeyCode::Char('d'), ctrl, ScrollBy::PageDown),
            (KeyCode::PageUp, none, ScrollBy::PageUp),
            (KeyCode::Char('u'), ctrl, ScrollBy::PageUp),
            (KeyCode::Home, none, ScrollBy::Top),
            (KeyCode::Char('g'), none, ScrollBy::Top),
            (KeyCode::End, none, ScrollBy::Bottom),
            (KeyCode::Char('G'), shift, ScrollBy::Bottom),
            (KeyCode::Char('g'), shift, ScrollBy::Bottom),
        ] {
            assert_eq!(
                action(code, modifiers),
                scroll(by),
                "{code:?} {modifiers:?}"
            );
        }
    }

    // attachments.md §5, keybindings.md §4, D-42
    #[test]
    fn a_comment_being_typed_takes_text_keys_and_enter_stamps_it_now() {
        let none = KeyModifiers::NONE;
        let comment =
            |code, modifiers| overlay_message(Overlay::AttachmentsComment, code, modifiers);
        for (code, input) in [
            (KeyCode::Enter, AttachmentsInput::Accept(NOW)),
            (KeyCode::Esc, AttachmentsInput::Cancel),
            (
                KeyCode::Char('d'),
                AttachmentsInput::Text(TextEdit::Insert('d')),
            ),
            (
                KeyCode::Backspace,
                AttachmentsInput::Text(TextEdit::DeleteBackward),
            ),
        ] {
            assert_eq!(
                comment(code, none),
                Some(Message::Attachments(input)),
                "{code:?}"
            );
        }
        assert_eq!(
            comment(KeyCode::Char('q'), KeyModifiers::CONTROL),
            Some(Message::Quit)
        );
        assert_eq!(
            comment(KeyCode::Char('v'), KeyModifiers::CONTROL),
            Some(Message::Paste(Paste::Below))
        );
        assert_eq!(comment(KeyCode::F(5), none), None);
        let context = &Context {
            overlay: Some(Overlay::AttachmentsComment),
            ..*CONTEXT
        };
        assert!(matches!(
            Keymap::default().action_for(&Event::Paste("a".to_owned()), ms(0), context),
            Some(KeyAction::Send(Message::Pasted { .. }))
        ));
    }

    // attachments.md §4, keybindings.md §4, D-42, D-107
    #[test]
    fn the_note_editor_takes_its_keys_and_text_keys() {
        let none = KeyModifiers::NONE;
        let editor = |code, modifiers| overlay_message(Overlay::AttachmentsEditor, code, modifiers);
        for (code, modifiers, input) in [
            (KeyCode::Esc, none, AttachmentsInput::Save),
            (KeyCode::Enter, none, AttachmentsInput::Break),
            (KeyCode::Up, none, AttachmentsInput::LineUp),
            (KeyCode::Down, none, AttachmentsInput::LineDown),
            (
                KeyCode::Char('j'),
                none,
                AttachmentsInput::Text(TextEdit::Insert('j')),
            ),
            (
                KeyCode::Backspace,
                none,
                AttachmentsInput::Text(TextEdit::DeleteBackward),
            ),
        ] {
            assert_eq!(
                editor(code, modifiers),
                Some(Message::Attachments(input)),
                "{code:?}"
            );
        }
        assert_eq!(
            editor(KeyCode::Char('q'), KeyModifiers::CONTROL),
            Some(Message::Quit)
        );
        assert_eq!(
            editor(KeyCode::Char('v'), KeyModifiers::CONTROL),
            Some(Message::Paste(Paste::Below))
        );
        assert_eq!(editor(KeyCode::F(5), none), None);
        let mut keymap = Keymap::default();
        let context = &Context {
            overlay: Some(Overlay::AttachmentsEditor),
            ..*CONTEXT
        };
        assert_eq!(
            keymap.action_for(&Event::Paste("a\nb".to_owned()), ms(0), context),
            Some(KeyAction::Send(Message::Pasted {
                text: "a\nb".to_owned(),
                at: Paste::Below,
            }))
        );
    }

    #[test]
    fn edit_mode_ends_a_pending_sequence() {
        let mut keymap = Keymap::default();
        keymap.message_for(&key(KeyCode::Char('g'), KeyModifiers::NONE), ms(0), CONTEXT);
        assert_eq!(
            keymap.message_for(&key(KeyCode::Char('g'), KeyModifiers::NONE), ms(1), EDITING),
            Some(Message::Text(TextEdit::Insert('g')))
        );
        assert_eq!(keymap.pending(), "");
    }

    // FR-OPS-01, D-42
    #[test]
    fn selection_keys_extend_toggle_select_all_and_clear() {
        let none = KeyModifiers::NONE;
        let shift = KeyModifiers::SHIFT;
        let down = Some(&Message::Select(SelectionChange::ExtendDown));
        let up = Some(&Message::Select(SelectionChange::ExtendUp));
        check(KeyCode::Char('J'), none, down);
        check(KeyCode::Char('J'), shift, down);
        check(KeyCode::Char('j'), shift, down);
        check(KeyCode::Down, shift, down);
        check(KeyCode::Char('K'), shift, up);
        check(KeyCode::Char('k'), shift, up);
        check(KeyCode::Up, shift, up);
        check(
            KeyCode::Char('v'),
            none,
            Some(&Message::Select(SelectionChange::Toggle)),
        );
        check(
            KeyCode::Char('a'),
            KeyModifiers::CONTROL,
            Some(&Message::Select(SelectionChange::All)),
        );
        check(
            KeyCode::Esc,
            none,
            Some(&Message::Select(SelectionChange::Clear)),
        );
    }

    // FR-OPS-04
    #[test]
    fn g_shift_g_groups() {
        let mut keymap = Keymap::default();
        let g = key(KeyCode::Char('g'), KeyModifiers::NONE);
        assert_eq!(keymap.message_for(&g, ms(0), CONTEXT), None);
        let shifted = key(KeyCode::Char('G'), KeyModifiers::SHIFT);
        assert_eq!(
            keymap.message_for(&shifted, ms(1), CONTEXT),
            Some(Message::Group)
        );
    }

    #[test]
    fn shift_only_drops_from_characters() {
        check(KeyCode::Right, KeyModifiers::SHIFT, None);
        check(KeyCode::Char('x'), KeyModifiers::ALT, None);
    }

    #[test]
    fn g_g_within_the_timeout_focuses_the_first_row() {
        let mut keymap = Keymap::default();
        let g = key(KeyCode::Char('g'), KeyModifiers::NONE);
        assert_eq!(keymap.message_for(&g, ms(5_000), CONTEXT), None);
        assert_eq!(keymap.pending(), "g");
        assert_eq!(keymap.deadline(), Some(ms(6_000)));
        assert_eq!(
            keymap.message_for(&g, ms(5_999), CONTEXT),
            Some(Message::FocusFirst)
        );
        assert_eq!(keymap.pending(), "");
        assert_eq!(keymap.deadline(), None);
    }

    #[test]
    fn a_sequence_times_out_after_one_second() {
        let mut keymap = Keymap::default();
        let g = key(KeyCode::Char('g'), KeyModifiers::NONE);
        keymap.message_for(&g, ms(0), CONTEXT);
        assert_eq!(keymap.expire(ms(999), CONTEXT), None);
        assert_eq!(keymap.pending(), "g");
        assert_eq!(keymap.expire(ms(1_000), CONTEXT), None);
        assert_eq!(keymap.pending(), "");
        assert_eq!(keymap.deadline(), None);
    }

    #[test]
    fn a_key_after_the_timeout_starts_a_new_sequence() {
        let mut keymap = Keymap::default();
        let g = key(KeyCode::Char('g'), KeyModifiers::NONE);
        keymap.message_for(&g, ms(0), CONTEXT);
        assert_eq!(keymap.message_for(&g, ms(1_000), CONTEXT), None);
        assert_eq!(keymap.pending(), "g");
        assert_eq!(keymap.deadline(), Some(ms(2_000)));
    }

    #[test]
    fn a_key_that_continues_no_sequence_cancels_it_and_does_nothing() {
        let mut keymap = Keymap::default();
        keymap.message_for(&key(KeyCode::Char('g'), KeyModifiers::NONE), ms(0), CONTEXT);
        assert_eq!(
            keymap.message_for(&key(KeyCode::Down, KeyModifiers::NONE), ms(10), CONTEXT),
            None
        );
        assert_eq!(keymap.pending(), "");
        assert_eq!(
            keymap.message_for(
                &key(KeyCode::Char('j'), KeyModifiers::NONE),
                ms(20),
                CONTEXT
            ),
            Some(Message::FocusNext)
        );
    }

    #[test]
    fn unbound_keys_leave_nothing_pending() {
        let mut keymap = Keymap::default();
        keymap.message_for(
            &key(KeyCode::Char('Q'), KeyModifiers::SHIFT),
            ms(0),
            CONTEXT,
        );
        assert_eq!(keymap.pending(), "");
        assert_eq!(keymap.deadline(), None);
    }

    #[test]
    fn release_and_repeat_events_are_ignored() {
        for kind in [KeyEventKind::Release, KeyEventKind::Repeat] {
            let event = |code| {
                Event::Key(KeyEvent::new_with_kind_and_state(
                    code,
                    KeyModifiers::NONE,
                    kind,
                    KeyEventState::NONE,
                ))
            };
            let mut keymap = Keymap::default();
            assert_eq!(
                keymap.message_for(&event(KeyCode::Char('j')), ms(0), CONTEXT),
                None
            );
            assert_eq!(
                keymap.message_for(&event(KeyCode::Char('g')), ms(0), CONTEXT),
                None
            );
            assert_eq!(keymap.pending(), "");
        }
    }

    #[test]
    fn non_key_events_are_ignored_and_keep_the_pending_sequence() {
        let mut keymap = Keymap::default();
        assert_eq!(
            keymap.message_for(&Event::Resize(80, 24), ms(0), CONTEXT),
            None
        );
        assert_eq!(
            keymap.message_for(&Event::FocusGained, ms(0), CONTEXT),
            None
        );
        keymap.message_for(&key(KeyCode::Char('g'), KeyModifiers::NONE), ms(0), CONTEXT);
        keymap.message_for(&Event::Resize(80, 24), ms(10), CONTEXT);
        assert_eq!(keymap.pending(), "g");
    }

    // file-io.md §3
    #[test]
    fn prompt_keys_choose_scroll_the_diff_and_quit() {
        let prompt = |code, modifiers| prompt_key(&key(code, modifiers), PAGE);
        let none = KeyModifiers::NONE;
        let ctrl = KeyModifiers::CONTROL;
        let send = |message| Some(PromptKey::Send(message));
        assert_eq!(prompt(KeyCode::Char('k'), ctrl), send(Message::KeepMine));
        assert_eq!(prompt(KeyCode::Char('r'), ctrl), send(Message::TakeTheirs));
        assert_eq!(prompt(KeyCode::Char('l'), ctrl), send(Message::ToggleDiff));
        for letter in ['m', 't', 'd', 'r', 'l'] {
            assert_eq!(
                prompt(KeyCode::Char(letter), none),
                None,
                "a plain {letter} typed for the editor answers nothing (D-115)"
            );
        }
        assert_eq!(prompt(KeyCode::Char('q'), ctrl), send(Message::Quit));
        for (code, modifiers, by) in [
            (KeyCode::Char('j'), none, 1),
            (KeyCode::Down, none, 1),
            (KeyCode::Char('k'), none, -1),
            (KeyCode::Up, none, -1),
            (KeyCode::Char('d'), ctrl, 7),
            (KeyCode::PageDown, none, 7),
            (KeyCode::Char('u'), ctrl, -7),
            (KeyCode::PageUp, none, -7),
        ] {
            assert_eq!(
                prompt(code, modifiers),
                Some(PromptKey::Scroll(by)),
                "{code}"
            );
        }
        assert_eq!(prompt(KeyCode::Char('x'), none), None);
        assert_eq!(prompt(KeyCode::Char('q'), none), None);
        assert_eq!(prompt_key(&Event::FocusGained, PAGE), None);
        let release = Event::Key(KeyEvent {
            kind: KeyEventKind::Release,
            ..KeyEvent::new(KeyCode::Char('k'), ctrl)
        });
        assert_eq!(prompt_key(&release, PAGE), None);
        assert_eq!(
            prompt_key(&key(KeyCode::PageDown, none), usize::MAX),
            Some(PromptKey::Scroll(isize::MAX))
        );
    }

    // FR-OPS-06, D-42
    #[test]
    fn clipboard_keys_copy_cut_and_paste_in_both_modes() {
        let ctrl = KeyModifiers::CONTROL;
        let below = Message::Paste(Paste::Below);
        for context in [CONTEXT, EDITING] {
            let message = |code, modifiers| {
                Keymap::default().message_for(&key(code, modifiers), ms(0), context)
            };
            assert_eq!(message(KeyCode::Char('c'), ctrl), Some(Message::Copy));
            assert_eq!(message(KeyCode::Char('x'), ctrl), Some(Message::Cut));
            assert_eq!(message(KeyCode::Char('v'), ctrl), Some(below.clone()));
        }
        check(
            KeyCode::Char('v'),
            KeyModifiers::ALT,
            Some(&Message::Paste(Paste::Above)),
        );
    }

    // FR-OPS-06, D-53
    #[test]
    fn y_copies_and_p_pastes_below_and_shift_p_above_in_navigate_mode() {
        let none = KeyModifiers::NONE;
        check(KeyCode::Char('y'), none, Some(&Message::Copy));
        check(
            KeyCode::Char('p'),
            none,
            Some(&Message::Paste(Paste::Below)),
        );
        let above = Some(&Message::Paste(Paste::Above));
        check(KeyCode::Char('P'), KeyModifiers::SHIFT, above);
        check(KeyCode::Char('P'), none, above);
        let typed = |c| Keymap::default().message_for(&key(KeyCode::Char(c), none), ms(0), EDITING);
        for c in ['y', 'p', 'P'] {
            assert_eq!(typed(c), Some(Message::Text(TextEdit::Insert(c))), "{c}");
        }
    }

    // FR-OPS-06, editing.md §4, keybindings.md §4: the date prompt takes text as edit mode does.
    #[test]
    fn text_pasted_into_the_terminal_is_pasted_below_or_into_a_prompt_but_not_menus() {
        let pasted = Event::Paste("a\nb".to_owned());
        let expected = Some(Message::Pasted {
            text: "a\nb".to_owned(),
            at: Paste::Below,
        });
        let mut keymap = Keymap::default();
        assert_eq!(keymap.message_for(&pasted, ms(0), CONTEXT), expected);
        assert_eq!(keymap.message_for(&pasted, ms(0), EDITING), expected);
        assert_eq!(keymap.message_for(&pasted, ms(0), IN_DATE_PROMPT), expected);
        let quick_add = Context {
            overlay: Some(Overlay::QuickAdd),
            ..*CONTEXT
        };
        assert_eq!(keymap.message_for(&pasted, ms(0), &quick_add), expected);
        for context in [
            Context {
                overlay: Some(Overlay::QuickAddPicker),
                ..*CONTEXT
            },
            Context {
                command_menu: true,
                ..*EDITING
            },
            Context {
                overlay: Some(Overlay::ColorPicker),
                ..*CONTEXT
            },
            *IN_NAVIGATOR,
        ] {
            assert_eq!(keymap.message_for(&pasted, ms(0), &context), None);
        }
    }

    // FR-UI-01, D-42, D-79: `Ctrl+G` is the same key with and without the kitty protocol, and
    // the Navigator's old keys are gone.
    #[test]
    fn ctrl_g_opens_the_navigator() {
        check(
            KeyCode::Char('g'),
            KeyModifiers::CONTROL,
            Some(&Message::ToggleNavigator),
        );
        check(KeyCode::Char('0'), KeyModifiers::ALT, None);
        assert_eq!(messages("g0", CONTEXT), [None, None]);
    }

    // keybindings.md §4, D-79: the open Navigator is a window that keeps the keys.
    #[test]
    fn the_open_navigator_takes_no_keys_that_focus_a_pane() {
        let typed = |c| Some(Message::Navigator(NavigatorInput::Type(c)));
        assert_eq!(messages("g1", IN_NAVIGATOR), [typed('g'), typed('1')]);
        let mut keymap = Keymap::default();
        for (code, modifiers) in [
            (KeyCode::Char('2'), KeyModifiers::ALT),
            (KeyCode::Right, KeyModifiers::CONTROL | KeyModifiers::SHIFT),
            (KeyCode::Char('w'), KeyModifiers::CONTROL),
        ] {
            assert_eq!(
                keymap.message_for(&key(code, modifiers), ms(0), IN_NAVIGATOR),
                None
            );
            assert_eq!(keymap.pending(), "", "nothing waits for a sequence");
        }
        assert_eq!(
            keymap.message_for(&key(KeyCode::Down, KeyModifiers::NONE), ms(1), IN_NAVIGATOR),
            Some(Message::Navigator(NavigatorInput::Next)),
        );
    }

    const IN_NAVIGATOR: &Context = &Context {
        navigator: true,
        ..*CONTEXT
    };

    // FR-UI-01, keybindings.md §4, D-86
    #[test]
    fn the_open_navigator_takes_its_keys_and_ctrl_q_and_ctrl_g_and_other_characters_filter() {
        let none = KeyModifiers::NONE;
        let ctrl = KeyModifiers::CONTROL;
        let navigator = |input| Some(Message::Navigator(input));
        for (code, modifiers, expected) in [
            (KeyCode::Down, none, navigator(NavigatorInput::Next)),
            (KeyCode::Char('n'), ctrl, navigator(NavigatorInput::Next)),
            (KeyCode::Up, none, navigator(NavigatorInput::Previous)),
            (
                KeyCode::Char('p'),
                ctrl,
                navigator(NavigatorInput::Previous),
            ),
            (KeyCode::Left, none, navigator(NavigatorInput::Collapse)),
            (KeyCode::Right, none, navigator(NavigatorInput::Expand)),
            (KeyCode::Enter, none, navigator(NavigatorInput::Choose)),
            (KeyCode::Backspace, none, navigator(NavigatorInput::Erase)),
            (KeyCode::Esc, none, navigator(NavigatorInput::Leave)),
            (KeyCode::Char('q'), ctrl, Some(Message::Quit)),
            (KeyCode::Char('g'), ctrl, Some(Message::ToggleNavigator)),
            (KeyCode::Char('b'), ctrl, None),
            (
                KeyCode::Char('j'),
                none,
                navigator(NavigatorInput::Type('j')),
            ),
            (
                KeyCode::Char('W'),
                KeyModifiers::SHIFT,
                navigator(NavigatorInput::Type('W')),
            ),
            (
                KeyCode::Char(' '),
                none,
                navigator(NavigatorInput::Type(' ')),
            ),
            (KeyCode::Char('j'), ctrl, None),
        ] {
            let mut keymap = Keymap::default();
            assert_eq!(
                keymap.message_for(&key(code, modifiers), ms(0), IN_NAVIGATOR),
                expected,
                "{modifiers:?} {code:?}"
            );
            assert_eq!(keymap.pending(), "", "nothing waits for a sequence");
        }
    }

    // keybindings.md §5
    #[test]
    fn keys_are_read_in_the_config_notation() {
        let ctrl = KeyModifiers::CONTROL;
        let alt = KeyModifiers::ALT;
        let shift = KeyModifiers::SHIFT;
        let none = KeyModifiers::NONE;
        for (text, key) in [
            ("j", (none, KeyCode::Char('j'))),
            ("G", (none, KeyCode::Char('G'))),
            ("+", (none, KeyCode::Char('+'))),
            ("ctrl++", (ctrl, KeyCode::Char('+'))),
            ("shift+g", (none, KeyCode::Char('G'))),
            ("ctrl+shift+z", (ctrl, KeyCode::Char('Z'))),
            ("shift+ctrl+z", (ctrl, KeyCode::Char('Z'))),
            ("alt+`", (alt, KeyCode::Char('`'))),
            ("ctrl+'", (ctrl, KeyCode::Char('\''))),
            ("space", (none, KeyCode::Char(' '))),
            ("enter", (none, KeyCode::Enter)),
            ("ctrl+enter", (ctrl, KeyCode::Enter)),
            ("esc", (none, KeyCode::Esc)),
            ("tab", (none, KeyCode::Tab)),
            ("shift+tab", (shift, KeyCode::BackTab)),
            ("backspace", (none, KeyCode::Backspace)),
            ("delete", (none, KeyCode::Delete)),
            ("up", (none, KeyCode::Up)),
            ("down", (none, KeyCode::Down)),
            ("alt+shift+left", (alt.union(shift), KeyCode::Left)),
            ("right", (none, KeyCode::Right)),
            ("home", (none, KeyCode::Home)),
            ("end", (none, KeyCode::End)),
            ("pageup", (none, KeyCode::PageUp)),
            ("pagedown", (none, KeyCode::PageDown)),
        ] {
            assert_eq!(parse_key(text), Some(key), "{text}");
        }
        for text in ["", "meta+j", "ctrl+", "ctrl+nope", "jj", "+j"] {
            assert_eq!(parse_key(text), None, "{text}");
        }
        assert_eq!(
            parse_sequence("g shift+g"),
            Some(vec![(none, KeyCode::Char('g')), (none, KeyCode::Char('G'))])
        );
        assert_eq!(parse_sequence("g nope"), None);
    }

    // keybindings.md §2–3
    #[test]
    fn every_default_binding_is_a_key_sequence() {
        for entry in REGISTRY {
            for keys in entry.keys {
                assert!(parse_sequence(keys).is_some(), "{keys}");
            }
        }
    }

    // FR-NAV-06, keybindings.md §2 and §4
    #[test]
    fn ctrl_p_and_colon_open_go_to_command_which_takes_the_keys_of_pickers() {
        let mut keymap = Keymap::default();
        let ctrl_p = key(KeyCode::Char('p'), KeyModifiers::CONTROL);
        assert_eq!(
            keymap.action_for(&ctrl_p, ms(0), CONTEXT),
            Some(KeyAction::GoToCommand)
        );
        let colon = key(KeyCode::Char(':'), KeyModifiers::NONE);
        assert_eq!(
            keymap.action_for(&colon, ms(1), CONTEXT),
            Some(KeyAction::GoToCommand)
        );
        let finding = &Context {
            overlay: Some(Overlay::GoToCommand),
            ..*CONTEXT
        };
        assert_eq!(
            keymap.message_for(&ctrl_p, ms(2), finding),
            Some(Message::Menu(Menu::GoToCommand, MenuInput::Previous)),
            "the picker's key, not a second Go to Command"
        );
        assert_eq!(
            keymap.message_for(&colon, ms(3), finding),
            Some(Message::Menu(Menu::GoToCommand, MenuInput::Type(':')))
        );
    }

    // FR-UI-06
    #[test]
    fn the_theme_picker_takes_the_keys_of_a_finder() {
        let mut keymap = Keymap::default();
        let choosing = &Context {
            overlay: Some(Overlay::ThemePicker),
            ..*CONTEXT
        };
        let n = key(KeyCode::Char('n'), KeyModifiers::NONE);
        assert_eq!(
            keymap.message_for(&n, ms(0), choosing),
            Some(Message::Menu(Menu::ThemePicker, MenuInput::Type('n')))
        );
    }

    // FR-UI-06, D-62
    #[test]
    fn ctrl_t_chooses_a_theme_in_navigate_mode() {
        let mut keymap = Keymap::default();
        let ctrl_t = key(KeyCode::Char('t'), KeyModifiers::CONTROL);
        assert_eq!(
            keymap.action_for(&ctrl_t, ms(0), CONTEXT),
            Some(KeyAction::ChooseTheme)
        );
    }

    // FR-SET-02, keybindings.md §2
    #[test]
    fn g_comma_opens_the_settings_window() {
        let mut keymap = Keymap::default();
        let press = |c| key(KeyCode::Char(c), KeyModifiers::NONE);
        assert_eq!(keymap.action_for(&press('g'), ms(0), CONTEXT), None);
        assert_eq!(
            keymap.action_for(&press(','), ms(1), CONTEXT),
            Some(KeyAction::OpenSettings)
        );
        assert_eq!(
            action(Command::OpenSettings, CONTEXT),
            Some(KeyAction::OpenSettings)
        );
    }

    /// The message `code` with `modifiers` sends while `overlay` is open.
    fn overlay_message(
        overlay: Overlay,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> Option<Message> {
        let context = &Context {
            overlay: Some(overlay),
            ..*CONTEXT
        };
        Keymap::default().message_for(&key(code, modifiers), ms(0), context)
    }

    // ui.md §8
    #[test]
    fn the_settings_window_takes_its_keys() {
        let none = KeyModifiers::NONE;
        let settings = |code| overlay_message(Overlay::Settings, code, none);
        for (code, input) in [
            (KeyCode::Char('j'), SettingsInput::Next),
            (KeyCode::Down, SettingsInput::Next),
            (KeyCode::Char('k'), SettingsInput::Previous),
            (KeyCode::Up, SettingsInput::Previous),
            (KeyCode::Tab, SettingsInput::SwitchColumn),
            (KeyCode::Enter, SettingsInput::Change),
            (KeyCode::Char(' '), SettingsInput::Change),
            (KeyCode::Char('h'), SettingsInput::Cycle(false)),
            (KeyCode::Left, SettingsInput::Cycle(false)),
            (KeyCode::Char('l'), SettingsInput::Cycle(true)),
            (KeyCode::Right, SettingsInput::Cycle(true)),
            (KeyCode::Char('r'), SettingsInput::Reset),
            (KeyCode::Char('/'), SettingsInput::Filter),
            (KeyCode::Esc, SettingsInput::Close),
        ] {
            assert_eq!(settings(code), Some(Message::Settings(input)), "{code:?}");
        }
        assert_eq!(settings(KeyCode::Char('x')), None, "other keys do nothing");
        for overlay in [
            Overlay::Settings,
            Overlay::SettingsText,
            Overlay::SettingsPicker,
        ] {
            assert_eq!(
                overlay_message(overlay, KeyCode::Char('q'), KeyModifiers::CONTROL),
                Some(Message::Quit),
                "{overlay:?}"
            );
        }
    }

    // ui.md §8
    #[test]
    fn the_settings_windows_filter_and_prompt_take_text_keys() {
        let text = |code, modifiers| overlay_message(Overlay::SettingsText, code, modifiers);
        let none = KeyModifiers::NONE;
        for (code, input) in [
            (KeyCode::Enter, SettingsInput::Accept),
            (KeyCode::Esc, SettingsInput::Cancel),
            (KeyCode::Down, SettingsInput::Next),
            (KeyCode::Up, SettingsInput::Previous),
            (
                KeyCode::Char('j'),
                SettingsInput::Edit(TextEdit::Insert('j')),
            ),
            (
                KeyCode::Backspace,
                SettingsInput::Edit(TextEdit::DeleteBackward),
            ),
        ] {
            assert_eq!(text(code, none), Some(Message::Settings(input)), "{code:?}");
        }
        assert_eq!(text(KeyCode::F(2), none), None);
    }

    // ui.md §8
    #[test]
    fn the_settings_windows_value_picker_takes_the_keys_of_a_picker() {
        let pick = |code| overlay_message(Overlay::SettingsPicker, code, KeyModifiers::NONE);
        for (code, input) in [
            (KeyCode::Char('n'), MenuInput::Type('n')),
            (KeyCode::Down, MenuInput::Next),
            (KeyCode::Enter, MenuInput::Choose),
            (KeyCode::Esc, MenuInput::Dismiss),
        ] {
            assert_eq!(
                pick(code),
                Some(Message::Settings(SettingsInput::Pick(input))),
                "{code:?}"
            );
        }
        assert_eq!(pick(KeyCode::F(2)), None);
    }

    // FR-DOC-06, keybindings.md §4
    #[test]
    fn the_save_as_prompt_takes_text_keys_enter_esc_and_tab() {
        let mut keymap = Keymap::default();
        let saving = &Context {
            overlay: Some(Overlay::SaveAs),
            ..*CONTEXT
        };
        let save_as = |event| Some(Message::SaveAs(event));
        for (code, modifiers, expected) in [
            (KeyCode::Enter, KeyModifiers::NONE, save_as(SaveAs::Accept)),
            (KeyCode::Esc, KeyModifiers::NONE, save_as(SaveAs::Cancel)),
            (KeyCode::Tab, KeyModifiers::NONE, save_as(SaveAs::Complete)),
            (
                KeyCode::Char('~'),
                KeyModifiers::NONE,
                save_as(SaveAs::Edit(TextEdit::Insert('~'))),
            ),
            (
                KeyCode::Backspace,
                KeyModifiers::NONE,
                save_as(SaveAs::Edit(TextEdit::DeleteBackward)),
            ),
            (
                KeyCode::Char('q'),
                KeyModifiers::CONTROL,
                Some(Message::Quit),
            ),
            (KeyCode::F(2), KeyModifiers::NONE, None),
        ] {
            assert_eq!(
                keymap.message_for(&key(code, modifiers), ms(0), saving),
                expected,
                "{code:?}"
            );
        }
    }

    // FR-DOC-06, ui.md §8, keybindings.md §4: the Save As prompt and the settings window's
    // prompt and filter take pasted text, and the rest of the window none.
    #[test]
    fn save_as_and_the_settings_text_fields_take_text_pasted_into_the_terminal_or_with_ctrl_v() {
        let pasted = Event::Paste("a\nb".to_owned());
        let mut keymap = Keymap::default();
        for overlay in [Overlay::SaveAs, Overlay::SettingsText] {
            let context = Context {
                overlay: Some(overlay),
                ..*CONTEXT
            };
            assert_eq!(
                keymap.message_for(&pasted, ms(0), &context),
                Some(Message::Pasted {
                    text: "a\nb".to_owned(),
                    at: Paste::Below,
                }),
                "{overlay:?}"
            );
            assert_eq!(
                overlay_message(overlay, KeyCode::Char('v'), KeyModifiers::CONTROL),
                Some(Message::Paste(Paste::Below)),
                "{overlay:?}"
            );
        }
        for overlay in [Overlay::Settings, Overlay::SettingsPicker] {
            let context = Context {
                overlay: Some(overlay),
                ..*CONTEXT
            };
            assert_eq!(keymap.message_for(&pasted, ms(0), &context), None);
        }
    }

    // FR-MODE-07, keybindings.md §4
    #[test]
    fn quick_add_takes_text_keys_enter_and_esc_and_its_picker_the_keys_of_a_picker() {
        let none = KeyModifiers::NONE;
        let panel = |code| overlay_message(Overlay::QuickAdd, code, none);
        for (code, input) in [
            (KeyCode::Enter, QuickAddInput::Add),
            (KeyCode::Esc, QuickAddInput::Cancel),
            (
                KeyCode::Char('/'),
                QuickAddInput::Edit(TextEdit::Insert('/')),
            ),
            (
                KeyCode::Backspace,
                QuickAddInput::Edit(TextEdit::DeleteBackward),
            ),
        ] {
            assert_eq!(panel(code), Some(Message::QuickAdd(input)), "{code:?}");
        }
        assert_eq!(panel(KeyCode::F(2)), None);
        let pick = |code| overlay_message(Overlay::QuickAddPicker, code, none);
        for (code, input) in [
            (KeyCode::Char('l'), MenuInput::Type('l')),
            (KeyCode::Down, MenuInput::Next),
            (KeyCode::Enter, MenuInput::Choose),
            (KeyCode::Esc, MenuInput::Dismiss),
        ] {
            assert_eq!(
                pick(code),
                Some(Message::Menu(Menu::QuickAdd, input)),
                "{code:?}"
            );
        }
        assert_eq!(
            overlay_message(Overlay::QuickAdd, KeyCode::Char('v'), KeyModifiers::CONTROL),
            Some(Message::Paste(Paste::Below))
        );
        for overlay in [Overlay::QuickAdd, Overlay::QuickAddPicker] {
            assert_eq!(
                overlay_message(overlay, KeyCode::Char('q'), KeyModifiers::CONTROL),
                Some(Message::Quit),
                "{overlay:?}"
            );
        }
    }

    // FR-NAV-06
    #[test]
    fn a_command_asks_for_its_message_or_what_the_frontend_does() {
        assert_eq!(
            action(Command::PageDown, CONTEXT),
            Some(KeyAction::Send(Message::PageDown { rows: PAGE }))
        );
        assert_eq!(
            action(Command::ToggleWrap, CONTEXT),
            Some(KeyAction::ToggleWrap)
        );
        assert_eq!(
            action(Command::GoToCommand, CONTEXT),
            Some(KeyAction::GoToCommand)
        );
        assert_eq!(
            action(Command::StartWeather, CONTEXT),
            Some(KeyAction::StartWeather)
        );
        assert_eq!(action(Command::Retype(Typed::Task), CONTEXT), None);
    }

    // keybindings.md §5
    #[test]
    fn config_bindings_give_keys_to_commands_and_take_them_from_others() {
        let mut keymap = Keymap::default();
        let errors = keymap.bind(
            Mode::Navigate,
            &[("ctrl+d", "toggle_done"), ("x", "none"), ("j", "Move Down")],
        );
        assert_eq!(errors, []);
        let press = |c, modifiers| key(KeyCode::Char(c), modifiers);
        assert_eq!(
            keymap.message_for(&press('d', KeyModifiers::CONTROL), ms(0), CONTEXT),
            Some(Message::ToggleDone { today: TODAY })
        );
        assert_eq!(
            keymap.message_for(&press('x', KeyModifiers::NONE), ms(1), CONTEXT),
            None,
            "unbound"
        );
        assert_eq!(
            keymap.message_for(&press('j', KeyModifiers::NONE), ms(2), CONTEXT),
            Some(Message::MoveDown)
        );
        assert_eq!(
            keymap.labels(Command::ToggleDone, Mode::Navigate),
            "Ctrl+D, Ctrl+Enter"
        );
        assert_eq!(keymap.labels(Command::PageDown, Mode::Navigate), "PgDn");
        assert_eq!(keymap.labels(Command::FocusNext, Mode::Navigate), "↓");
        assert_eq!(
            keymap.labels(Command::MoveDown, Mode::Navigate),
            "j, Alt+j, Alt+↓"
        );
        let errors = keymap.bind(Mode::Edit, &[("ctrl+g", "INLINE CODE")]);
        assert_eq!(errors, []);
        assert_eq!(
            keymap.message_for(&press('g', KeyModifiers::CONTROL), ms(3), EDITING),
            Some(Message::Text(TextEdit::Toggle(Marker::Code)))
        );
    }

    // keybindings.md §5, D-53
    #[test]
    fn a_config_binding_may_start_a_longer_sequence_and_acts_once_it_times_out() {
        let mut keymap = Keymap::default();
        assert_eq!(keymap.bind(Mode::Navigate, &[("g", "go_home")]), []);
        let g = key(KeyCode::Char('g'), KeyModifiers::NONE);
        assert_eq!(keymap.message_for(&g, ms(0), CONTEXT), None);
        assert_eq!(
            keymap.message_for(&g, ms(1), CONTEXT),
            Some(Message::FocusFirst),
            "gg stays"
        );
        assert_eq!(keymap.message_for(&g, ms(2), CONTEXT), None);
        assert_eq!(
            keymap.expire(ms(1002), CONTEXT),
            Some(KeyAction::Send(Message::GoTo(Location::Outline(None))))
        );
    }

    // keybindings.md §5, D-79
    #[test]
    fn the_sidebars_old_names_bind_open_navigator() {
        let mut keymap = Keymap::default();
        let errors = keymap.bind(
            Mode::Navigate,
            &[("ctrl+b", "toggle_sidebar"), ("alt+0", "Focus Sidebar")],
        );
        assert_eq!(errors, []);
        for (at, (code, modifiers)) in [
            (KeyCode::Char('b'), KeyModifiers::CONTROL),
            (KeyCode::Char('0'), KeyModifiers::ALT),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                keymap.message_for(&key(code, modifiers), ms(at as u64), CONTEXT),
                Some(Message::ToggleNavigator)
            );
        }
        assert_eq!(
            keymap
                .bind(Mode::Edit, &[("ctrl+b", "toggle_sidebar")])
                .len(),
            1,
            "only where Open Navigator is bound"
        );
    }

    // keybindings.md §5, D-103
    #[test]
    fn show_details_binds_edit_node() {
        let mut keymap = Keymap::default();
        assert_eq!(
            keymap.bind(Mode::Navigate, &[("ctrl+e", "show_details")]),
            []
        );
        assert_eq!(
            keymap.message_for(
                &key(KeyCode::Char('e'), KeyModifiers::CONTROL),
                ms(0),
                CONTEXT
            ),
            Some(Message::NodeEditor(NodeInput::Open))
        );
    }

    // keybindings.md §5
    #[test]
    fn config_bindings_that_cannot_apply_are_reported_and_leave_the_defaults() {
        let mut keymap = Keymap::default();
        let errors = keymap.bind(
            Mode::Navigate,
            &[
                ("hyper+x", "undo"),
                ("Ctrl+Q", "undo"),
                ("u", "frobnicate"),
                ("v", "stop_editing"),
                ("G", "undo"),
                ("shift+g", "redo"),
                ("u u", "undo"),
            ],
        );
        let keys = |keys: &str| keys.to_owned();
        assert_eq!(
            errors,
            [
                BindingError::NotAKey {
                    keys: keys("hyper+x")
                },
                BindingError::NotAKey {
                    keys: keys("Ctrl+Q")
                },
                BindingError::UnknownCommand {
                    keys: keys("u"),
                    command: keys("frobnicate")
                },
                BindingError::UnknownCommand {
                    keys: keys("v"),
                    command: keys("stop_editing")
                },
                BindingError::BoundTwice { keys: keys("G") },
                BindingError::BoundTwice {
                    keys: keys("shift+g")
                },
            ]
        );
        assert_eq!(keymap.labels(Command::FocusLast, Mode::Navigate), "G, End");
        assert_eq!(
            keymap.labels(Command::Undo, Mode::Navigate),
            "uu, z, Ctrl+Z"
        );
        let errors = keymap.bind(
            Mode::Edit,
            &[
                ("ctrl+w x", "bold"),
                ("ctrl+p", "project"),
                ("ctrl+n", "none"),
            ],
        );
        assert_eq!(
            errors,
            [
                BindingError::Sequence {
                    keys: keys("ctrl+w x")
                },
                BindingError::UnknownCommand {
                    keys: keys("ctrl+p"),
                    command: keys("project")
                },
            ],
            "the command menu's types are not keys' to give"
        );
        assert_eq!(
            BindingError::Sequence { keys: keys("a b") }.to_string(),
            "\"a b\": edit mode binds single keys, not sequences"
        );
        assert_eq!(
            BindingError::UnknownCommand {
                keys: keys("u"),
                command: keys("nope")
            }
            .to_string(),
            "\"u\": no command \"nope\" here"
        );
        assert_eq!(
            BindingError::NotAKey { keys: keys("q+") }.to_string(),
            "\"q+\" is not a key"
        );
    }

    // keybindings.md §5
    #[test]
    fn every_command_keys_can_give_has_a_config_name_of_its_own_in_its_mode() {
        for mode in [Mode::Navigate, Mode::Edit] {
            for entry in REGISTRY.iter().filter(|entry| entry.mode == mode) {
                let name = config_name(&entry.command.name());
                let expected =
                    (!matches!(entry.command, Command::Retype(_))).then_some(entry.command);
                assert_eq!(command_named(mode, &name), expected, "{name} in {mode:?}");
            }
        }
        assert_eq!(config_name("Move to…"), "move_to");
        assert_eq!(config_name("Show/Hide Completed"), "show_hide_completed");
        assert_eq!(config_name("Color: Red"), "color_red");
        assert_eq!(config_name("  Focus Pane 1 "), "focus_pane_1");
    }

    // FR-KEY-04, FR-NAV-06
    #[test]
    fn labels_join_the_labels_of_a_commands_keys_in_a_mode() {
        let keymap = Keymap::default();
        assert_eq!(
            keymap.labels(Command::GoToCommand, Mode::Navigate),
            "Ctrl+P, :"
        );
        assert_eq!(keymap.labels(Command::Count, Mode::Navigate), "");
        assert_eq!(keymap.labels(Command::StopEditing, Mode::Edit), "Esc");
        assert_eq!(keymap.labels(Command::StopEditing, Mode::Navigate), "");
        assert_eq!(label("fx"), None, "only a number makes a function key");
    }

    // FR-KEY-06, keybindings.md §1, §2 and §4, D-42, D-101
    #[test]
    fn backslash_shift_f10_and_the_menu_key_open_the_context_menu_and_ctrl_r_in_place_of_backslash_in_the_navigator()
     {
        let open = Some(&Message::OpenContextMenu);
        check(KeyCode::Char('\\'), KeyModifiers::NONE, open);
        check(KeyCode::F(10), KeyModifiers::SHIFT, open);
        check(KeyCode::Menu, KeyModifiers::NONE, open);
        check(KeyCode::F(10), KeyModifiers::NONE, None);
        check(KeyCode::Char('r'), KeyModifiers::CONTROL, None);
        let mut keymap = Keymap::default();
        let navigator = &Context {
            navigator: true,
            ..*CONTEXT
        };
        for (at, (code, modifiers)) in [
            (KeyCode::Char('r'), KeyModifiers::CONTROL),
            (KeyCode::F(10), KeyModifiers::SHIFT),
            (KeyCode::Menu, KeyModifiers::NONE),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                keymap
                    .message_for(&key(code, modifiers), ms(at as u64), navigator)
                    .as_ref(),
                open,
                "{code:?}"
            );
        }
        assert_eq!(
            keymap.message_for(
                &key(KeyCode::Char('\\'), KeyModifiers::NONE),
                ms(9),
                navigator
            ),
            Some(Message::Navigator(NavigatorInput::Type('\\'))),
            "the Navigator's filter takes \\ as typed (D-86)"
        );
        assert_eq!(
            keymap.message_for(
                &key(KeyCode::Char('\\'), KeyModifiers::NONE),
                ms(10),
                EDITING
            ),
            Some(Message::Text(TextEdit::Insert('\\'))),
            "edit mode types it"
        );
    }

    // FR-KEY-06, keybindings.md §4
    #[test]
    fn the_open_context_menu_takes_the_keys_of_pickers() {
        let mut keymap = Keymap::default();
        let menu = &Context {
            overlay: Some(Overlay::ContextMenu),
            ..*CONTEXT
        };
        for (at, (code, input)) in [
            (KeyCode::Down, MenuInput::Next),
            (KeyCode::Up, MenuInput::Previous),
            (KeyCode::Enter, MenuInput::Choose),
            (KeyCode::Esc, MenuInput::Dismiss),
            (KeyCode::Char('j'), MenuInput::Type('j')),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                keymap.message_for(&key(code, KeyModifiers::NONE), ms(at as u64), menu),
                Some(Message::Menu(Menu::ContextMenu, input))
            );
        }
    }

    // FR-KEY-06, ui.md §5
    #[test]
    fn a_commands_bindings_are_its_navigate_keys_or_its_navigator_keys() {
        let keymap = Keymap::default();
        let bindings = |command, navigator| keymap.bindings(command, navigator);
        assert_eq!(bindings(Command::ContextMenu, false), "\\, Shift+F10, Menu");
        assert_eq!(
            bindings(Command::ContextMenu, true),
            "Ctrl+R, Shift+F10, Menu"
        );
        assert_eq!(bindings(Command::Count, false), "");
        assert_eq!(
            bindings(Command::Retype(Typed::Task), false),
            "",
            "edit mode's"
        );
        assert_eq!(
            bindings(Command::Navigator(NavigatorInput::Choose), false),
            "Enter"
        );
        assert_eq!(
            bindings(Command::Navigator(NavigatorInput::Next), false),
            "↓, Ctrl+N"
        );
        let open = Command::Pane(PaneChange::Open);
        assert_eq!(bindings(open, false), "Ctrl+\\, Ctrl+W v");
        assert_eq!(
            bindings(open, true),
            "Alt+Enter, Ctrl+V, Ctrl+T",
            "the Navigator's own"
        );
        assert_eq!(
            bindings(Command::Undo, true),
            "z, Ctrl+Z",
            "no Navigator key"
        );
    }

    // FR-NAV-05, keybindings.md §2 and §4
    #[test]
    fn ctrl_k_g_s_and_slash_open_go_to_symbol_which_takes_the_keys_of_pickers() {
        let mut keymap = Keymap::default();
        let ctrl_k = key(KeyCode::Char('k'), KeyModifiers::CONTROL);
        assert_eq!(
            keymap.message_for(&ctrl_k, ms(0), CONTEXT),
            Some(Message::OpenGoToSymbol(Symbols::All))
        );
        let char = |c| key(KeyCode::Char(c), KeyModifiers::NONE);
        assert_eq!(keymap.message_for(&char('g'), ms(0), CONTEXT), None);
        assert_eq!(
            keymap.message_for(&char('s'), ms(1), CONTEXT),
            Some(Message::OpenGoToSymbol(Symbols::All))
        );
        assert_eq!(
            keymap.message_for(&char('/'), ms(2), CONTEXT),
            Some(Message::OpenGoToSymbol(Symbols::All))
        );
        let finding = &Context {
            overlay: Some(Overlay::GoToSymbol),
            ..*CONTEXT
        };
        assert_eq!(
            keymap.message_for(
                &key(KeyCode::Char('p'), KeyModifiers::CONTROL),
                ms(0),
                finding
            ),
            Some(Message::Menu(Menu::GoToSymbol, MenuInput::Previous))
        );
        assert_eq!(
            keymap.message_for(&ctrl_k, ms(1), finding),
            None,
            "no second Go to Symbol"
        );
        assert_eq!(
            keymap.message_for(&char('g'), ms(3), finding),
            Some(Message::Menu(Menu::GoToSymbol, MenuInput::Type('g')))
        );
    }

    // D-65, D-42: `gP` opens Go to Project whether `P` arrives as `P`, as `P` with `Shift` or,
    // under the kitty protocol, as `p` with `Shift`; `gp` still focuses the pane left (D-118).
    #[test]
    fn g_shift_p_opens_go_to_project_in_both_key_reportings() {
        let open = Some(KeyAction::Send(Message::OpenGoToSymbol(Symbols::Projects)));
        for (c, modifiers) in [
            ('P', KeyModifiers::NONE),
            ('P', KeyModifiers::SHIFT),
            ('p', KeyModifiers::SHIFT),
        ] {
            assert_eq!(
                actions(
                    &[
                        (KeyCode::Char('g'), KeyModifiers::NONE),
                        (KeyCode::Char(c), modifiers)
                    ],
                    CONTEXT
                ),
                [None, open.clone()],
                "{c} {modifiers:?}"
            );
        }
    }

    /// What the keys `presses` ask for in turn from a fresh keymap in `context`.
    fn actions(presses: &[(KeyCode, KeyModifiers)], context: &Context) -> Vec<Option<KeyAction>> {
        let mut keymap = Keymap::default();
        presses
            .iter()
            .map(|&(code, modifiers)| keymap.action_for(&key(code, modifiers), ms(0), context))
            .collect()
    }

    // FR-UI-03, keybindings.md §1–2, D-42: `Ctrl+\` has the classic variant `Ctrl+W v`, and
    // legacy reporting's `Ctrl+4` is not bound.
    #[test]
    fn ctrl_backslash_and_the_ctrl_w_sequences_open_focus_and_close_panes() {
        let ctrl = KeyModifiers::CONTROL;
        let none = KeyModifiers::NONE;
        let pane = |change| Some(KeyAction::Send(Message::Pane(change)));
        assert_eq!(
            actions(&[(KeyCode::Char('\\'), ctrl)], CONTEXT),
            [pane(PaneChange::Open)]
        );
        assert_eq!(actions(&[(KeyCode::Char('4'), ctrl)], CONTEXT), [None]);
        for (c, expected) in [
            ('v', pane(PaneChange::Open)),
            ('s', pane(PaneChange::OpenLocation)),
            ('h', pane(PaneChange::FocusLeft)),
            ('l', pane(PaneChange::FocusRight)),
            ('c', pane(PaneChange::Close)),
            ('<', Some(KeyAction::NarrowPane)),
            ('>', Some(KeyAction::WidenPane)),
            ('x', None),
        ] {
            assert_eq!(
                actions(
                    &[(KeyCode::Char('w'), ctrl), (KeyCode::Char(c), none)],
                    CONTEXT
                ),
                [None, expected],
                "Ctrl+W {c}"
            );
        }
        assert_eq!(
            actions(
                &[(KeyCode::Char('w'), ctrl), (KeyCode::Char('v'), none)],
                EDITING
            ),
            [
                Some(KeyAction::Send(Message::Text(TextEdit::DeleteWordBackward))),
                Some(KeyAction::Send(Message::Text(TextEdit::Insert('v')))),
            ],
            "edit mode keeps Ctrl+W"
        );
    }

    // FR-UI-03, keybindings.md §2, D-42: `Alt+1` … `Alt+9` have the classic variants `g1` …
    // `g9`.
    #[test]
    fn alt_and_g_with_a_digit_focus_that_pane() {
        for (at, digit) in ('1'..='9').enumerate() {
            let focus = Some(Message::Pane(PaneChange::Focus(at)));
            check(KeyCode::Char(digit), KeyModifiers::ALT, focus.as_ref());
            assert_eq!(messages(&format!("g{digit}"), CONTEXT), [None, focus]);
        }
    }

    // D-71, D-42: `Ctrl+Shift+←`/`→`, which both key reportings send, and their classic variants
    // `Ctrl+W ←`/`→` move to the pane on the left or right; edit mode keeps `Ctrl+Shift+←`/`→` for selecting words.
    #[test]
    fn ctrl_shift_and_ctrl_w_with_an_arrow_move_to_the_pane_on_that_side() {
        let ctrl = KeyModifiers::CONTROL;
        let ctrl_w = (KeyCode::Char('w'), ctrl);
        let pane = |change| Some(KeyAction::Send(Message::Pane(change)));
        for (code, change) in [
            (KeyCode::Left, PaneChange::FocusLeft),
            (KeyCode::Right, PaneChange::FocusRight),
        ] {
            let shifted = (code, ctrl | KeyModifiers::SHIFT);
            assert_eq!(actions(&[shifted], CONTEXT), [pane(change)]);
            let arrow = (code, KeyModifiers::NONE);
            assert_eq!(actions(&[ctrl_w, arrow], CONTEXT), [None, pane(change)]);
        }
        assert_eq!(
            actions(&[ctrl_w, (KeyCode::Char('l'), KeyModifiers::NONE)], CONTEXT),
            [None, pane(PaneChange::FocusRight)],
            "Ctrl+W l too"
        );
        let keymap = Keymap::default();
        for (change, keys) in [
            (
                PaneChange::FocusLeft,
                "Ctrl+Shift+←, Ctrl+W ←, Ctrl+W h, gl, gp",
            ),
            (
                PaneChange::FocusRight,
                "Ctrl+Shift+→, Ctrl+W →, Ctrl+W l, gr, gn",
            ),
        ] {
            assert_eq!(keymap.bindings(Command::Pane(change), true), keys);
        }
    }

    // D-120, D-42: `Ctrl+W H`/`L` move the focused pane, as vim moves a window; the capital
    // arrives plain or, under the kitty protocol, as either case with `Shift`.
    #[test]
    fn ctrl_w_capital_h_and_l_move_the_pane_left_and_right() {
        let ctrl_w = (KeyCode::Char('w'), KeyModifiers::CONTROL);
        for (letter, change, keys) in [
            ('H', PaneChange::MoveLeft, "Ctrl+W H"),
            ('L', PaneChange::MoveRight, "Ctrl+W L"),
        ] {
            let moves = Some(KeyAction::Send(Message::Pane(change)));
            for (c, modifiers) in [
                (letter, KeyModifiers::NONE),
                (letter, KeyModifiers::SHIFT),
                (letter.to_ascii_lowercase(), KeyModifiers::SHIFT),
            ] {
                assert_eq!(
                    actions(&[ctrl_w, (KeyCode::Char(c), modifiers)], CONTEXT),
                    [None, moves.clone()],
                    "Ctrl+W {c} {modifiers:?}"
                );
            }
            assert_eq!(
                Keymap::default().bindings(Command::Pane(change), true),
                keys
            );
        }
    }

    // D-64, D-42: `gt` is plain characters, which both key reportings send alike; `t` alone still
    // sets the start date to today.
    #[test]
    fn g_t_goes_to_today_and_t_alone_still_starts_today() {
        assert_eq!(
            messages("gt", CONTEXT),
            [
                None,
                Some(Message::GoTo(Location::View(SpecialView::Today)))
            ]
        );
        let start = Message::ChangeDate {
            tag: DateTag::Start,
            change: DateChange::Today,
            today: TODAY,
        };
        assert_eq!(messages("t", CONTEXT), [Some(start)]);
    }

    // FR-TAG-04, FR-TAG-08, FR-TAG-09, FR-TAG-10, FR-TAG-11, FR-TAG-13, FR-TAG-14, D-95, D-42: a
    // special view's capital `g` key works whether the capital arrives plain or, under the kitty
    // protocol, with `Shift`, which also reports each key's release.
    #[test]
    fn capital_g_keys_go_to_the_special_views_with_and_without_the_kitty_protocol() {
        let event = |c, modifiers, kind| {
            Event::Key(KeyEvent::new_with_kind_and_state(
                KeyCode::Char(c),
                modifiers,
                kind,
                KeyEventState::NONE,
            ))
        };
        let (none, shift) = (KeyModifiers::NONE, KeyModifiers::SHIFT);
        let (press, release) = (KeyEventKind::Press, KeyEventKind::Release);
        for (letter, view, keys) in [
            ('T', SpecialView::Today, "gt, gT"),
            ('U', SpecialView::Upcoming, "gU"),
            ('O', SpecialView::Overdue, "gO"),
            ('C', SpecialView::Colors, "gC"),
            ('L', SpecialView::Logbook, "gL"),
            ('S', SpecialView::Starred, "gS"),
            ('F', SpecialView::Flagged, "gF"),
        ] {
            let goes = Some(Message::GoTo(Location::View(view)));
            assert_eq!(
                messages(&format!("g{letter}"), CONTEXT),
                [None, goes.clone()],
                "plain g{letter}"
            );
            for shifted in [letter, letter.to_ascii_lowercase()] {
                let mut keymap = Keymap::default();
                let kitty: Vec<_> = [
                    event('g', none, press),
                    event('g', none, release),
                    event(shifted, shift, press),
                    event(shifted, shift, release),
                ]
                .iter()
                .map(|event| keymap.message_for(event, ms(0), CONTEXT))
                .collect();
                assert_eq!(kitty, [None, None, goes.clone(), None], "g Shift+{shifted}");
            }
            assert_eq!(
                Keymap::default().bindings(Command::GoToView(view), true),
                keys
            );
        }
    }

    // D-95: the special views' former keys go to no view; `gn` now focuses the pane right (D-118).
    #[test]
    fn the_former_special_view_keys_go_to_no_view() {
        for keys in ["go", "gD", "g*", "gf"] {
            assert_eq!(messages(keys, CONTEXT), [None, None], "{keys}");
        }
    }

    // FR-UI-03, keybindings.md §2: under the kitty protocol `Alt+Shift+h` arrives as `h` with
    // `Shift`.
    #[test]
    fn alt_shift_h_and_l_narrow_and_widen_the_pane() {
        let alt = KeyModifiers::ALT;
        let alt_shift = KeyModifiers::ALT | KeyModifiers::SHIFT;
        for (code, modifiers, expected) in [
            (KeyCode::Char('H'), alt, KeyAction::NarrowPane),
            (KeyCode::Char('h'), alt_shift, KeyAction::NarrowPane),
            (KeyCode::Char('L'), alt, KeyAction::WidenPane),
            (KeyCode::Char('l'), alt_shift, KeyAction::WidenPane),
        ] {
            assert_eq!(
                actions(&[(code, modifiers)], CONTEXT),
                [Some(expected)],
                "{modifiers:?} {code:?}"
            );
        }
        assert_eq!(
            action(Command::NarrowPane, CONTEXT),
            Some(KeyAction::NarrowPane),
            "as Go to Command gives it"
        );
        assert_eq!(
            action(Command::WidenPane, CONTEXT),
            Some(KeyAction::WidenPane)
        );
    }

    // FR-UI-03, navigation.md §5–6, keybindings.md §4 and §6, D-42: `Alt+Enter` has the classic
    // variant `Ctrl+V` in Go to Symbol and the Navigator, and `Ctrl+T` stands beside both, which
    // Windows Terminal takes; they mean nothing to other pickers.
    #[test]
    fn alt_enter_ctrl_v_and_ctrl_t_open_the_choice_of_go_to_symbol_and_the_navigator_in_a_new_pane()
    {
        let open = Some(KeyAction::Send(Message::Pane(PaneChange::Open)));
        let finding = &Context {
            overlay: Some(Overlay::GoToSymbol),
            ..*CONTEXT
        };
        let moving = &Context {
            overlay: Some(Overlay::MoveTo),
            ..*CONTEXT
        };
        for (code, modifiers) in [
            (KeyCode::Enter, KeyModifiers::ALT),
            (KeyCode::Char('v'), KeyModifiers::CONTROL),
            (KeyCode::Char('t'), KeyModifiers::CONTROL),
        ] {
            assert_eq!(
                actions(&[(code, modifiers)], finding),
                std::slice::from_ref(&open)
            );
            assert_eq!(
                actions(&[(code, modifiers)], IN_NAVIGATOR),
                std::slice::from_ref(&open)
            );
            assert_eq!(actions(&[(code, modifiers)], moving), [None]);
        }
    }

    // FR-OPS-05, keybindings.md §2 and §4
    #[test]
    fn g_m_opens_move_to_which_takes_the_keys_of_pickers() {
        let mut keymap = Keymap::default();
        let char = |c| key(KeyCode::Char(c), KeyModifiers::NONE);
        assert_eq!(keymap.message_for(&char('g'), ms(0), CONTEXT), None);
        assert_eq!(
            keymap.message_for(&char('m'), ms(1), CONTEXT),
            Some(Message::OpenMoveTo)
        );
        let moving = &Context {
            overlay: Some(Overlay::MoveTo),
            ..*CONTEXT
        };
        assert_eq!(
            keymap.message_for(&key(KeyCode::Enter, KeyModifiers::NONE), ms(2), moving),
            Some(Message::Menu(Menu::MoveTo, MenuInput::Choose))
        );
        assert_eq!(
            keymap.message_for(&char('g'), ms(3), moving),
            Some(Message::Menu(Menu::MoveTo, MenuInput::Type('g')))
        );
    }

    // FR-KEY-04, ui.md §4: a menu or picker that has the keys lists the keys it takes, which
    // move, choose, close and erase as they do there; the mode's own and other popups' keys are
    // not listed here.
    #[test]
    fn the_shortcuts_of_a_menu_or_picker_are_the_keys_it_takes() {
        let mut keymap = Keymap::default();
        assert_eq!(keymap.popup_shortcuts(CONTEXT), None, "navigate mode");
        assert_eq!(keymap.popup_shortcuts(EDITING), None, "edit mode");
        for overlay in [
            Overlay::ColorPicker,
            Overlay::DatePrompt,
            Overlay::QuickAdd,
            Overlay::QuickAddPicker,
            Overlay::TypePicker,
            Overlay::NodeEditor,
            Overlay::NodeEditorFinder,
            Overlay::Attachments,
            Overlay::AttachmentsEditor,
            Overlay::AttachmentsComment,
            Overlay::GoToSymbol,
            Overlay::MoveTo,
            Overlay::GoToCommand,
            Overlay::ThemePicker,
            Overlay::Settings,
            Overlay::SettingsText,
            Overlay::SettingsPicker,
            Overlay::SaveAs,
            Overlay::ContextMenu,
        ] {
            let context = &Context {
                overlay: Some(overlay),
                ..*CONTEXT
            };
            let listed = keymap.popup_shortcuts(context);
            let Some(menu) = overlay.menu() else {
                if !matches!(
                    overlay,
                    Overlay::DatePrompt | Overlay::QuickAdd | Overlay::NodeEditor
                ) {
                    assert_eq!(listed, None, "{overlay:?} has keys of its own");
                }
                continue;
            };
            let (group, entries) = listed.expect("a picker lists its keys");
            assert_eq!(group, Group::Picker);
            let names: Vec<_> = entries.iter().map(|(name, _)| name.as_ref()).collect();
            let open = (menu == Menu::GoToSymbol).then_some("Open in New Pane");
            let expected: Vec<_> = ["Next Entry", "Previous Entry", "Choose Entry"]
                .into_iter()
                .chain(open)
                .chain(["Close", "Erase Filter Character", "Quit"])
                .collect();
            assert_eq!(names, expected, "{overlay:?}");
            for (code, modifiers, input) in [
                (KeyCode::Down, KeyModifiers::NONE, MenuInput::Next),
                (
                    KeyCode::Char('p'),
                    KeyModifiers::CONTROL,
                    MenuInput::Previous,
                ),
                (KeyCode::Enter, KeyModifiers::NONE, MenuInput::Choose),
                (KeyCode::Esc, KeyModifiers::NONE, MenuInput::Dismiss),
                (KeyCode::Backspace, KeyModifiers::NONE, MenuInput::Erase),
            ] {
                assert_eq!(
                    keymap.message_for(&key(code, modifiers), ms(0), context),
                    Some(Message::Menu(menu, input)),
                    "{overlay:?} takes {code:?}"
                );
            }
            let keys = |name: &str| {
                entries
                    .iter()
                    .find(|(listed, _)| listed == name)
                    .map(|(_, keys)| keys.as_str())
            };
            assert_eq!(keys("Next Entry"), Some("↓, Ctrl+N"));
            assert_eq!(keys("Erase Filter Character"), Some("Backspace"));
            assert_eq!(keys("Quit"), Some("Ctrl+Q"));
        }
    }

    // FR-KEY-04, ui.md §4: a dialog with keys of its own that takes text lists those keys, not
    // the outline's, which do not work there.
    #[test]
    fn the_shortcuts_of_a_dialog_that_takes_text_are_its_keys() {
        let keymap = Keymap::default();
        let listed = |overlay| {
            let (group, entries) = keymap
                .popup_shortcuts(&Context {
                    overlay: Some(overlay),
                    ..*CONTEXT
                })
                .expect("the dialog lists its keys");
            let entries: Vec<_> = entries
                .into_iter()
                .map(|(name, keys)| format!("{name}: {keys}"))
                .collect();
            (group.name(), entries)
        };
        assert_eq!(
            listed(Overlay::NodeEditor),
            (
                "Edit Node",
                vec![
                    "Next Row: Tab, ↓".to_owned(),
                    "Previous Row: Shift+Tab, ↑".to_owned(),
                    "Cycle Type: Ctrl+T".to_owned(),
                    "Save: Enter".to_owned(),
                    "Cancel: Esc".to_owned(),
                    "Paste: Ctrl+V".to_owned(),
                    "Quit: Ctrl+Q".to_owned(),
                ]
            )
        );
        assert_eq!(
            listed(Overlay::QuickAdd),
            (
                "Quick Add",
                vec![
                    "Add: Enter".to_owned(),
                    "Cancel: Esc".to_owned(),
                    "Paste: Ctrl+V".to_owned(),
                    "Quit: Ctrl+Q".to_owned(),
                ]
            )
        );
        assert_eq!(
            listed(Overlay::DatePrompt),
            (
                "Date Prompt",
                vec![
                    "Set Date: Enter".to_owned(),
                    "Cancel: Esc".to_owned(),
                    "Paste: Ctrl+V".to_owned(),
                    "Quit: Ctrl+Q".to_owned(),
                ]
            )
        );
    }

    // FR-KEY-04, ui.md §4: the open Navigator lists each of its keys, and a picker over it lists
    // the picker's.
    #[test]
    fn the_shortcuts_of_the_open_navigator_are_its_keys() {
        let keymap = Keymap::default();
        let (group, entries) = keymap
            .popup_shortcuts(IN_NAVIGATOR)
            .expect("the Navigator lists its keys");
        assert_eq!(group, Group::Navigator);
        assert_eq!(entries.len(), NAVIGATOR.len());
        assert_eq!(
            entries.first(),
            Some(&("Next Entry".into(), "↓, Ctrl+N".to_owned()))
        );
        assert_eq!(entries.last(), Some(&("Quit".into(), "Ctrl+Q".to_owned())));
        let over = &Context {
            overlay: Some(Overlay::ContextMenu),
            ..*IN_NAVIGATOR
        };
        assert_eq!(
            keymap.popup_shortcuts(over).map(|(group, _)| group),
            Some(Group::Picker)
        );
    }
}
