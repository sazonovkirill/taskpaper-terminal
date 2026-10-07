//! Key events to app messages, and to the frontend's own commands
//! ([keybindings.md](../../../docs/specs/keybindings.md)).

use std::{mem, time::Duration};

use crossterm::event::{Event, KeyCode, KeyModifiers};
use jiff::civil::Date;
use thiserror::Error;

use crate::app::{
    Command, DateInput, Inputs, Menu, MenuInput, Message, Mode, Motion, PaneChange, Paste, SaveAs,
    SettingsInput, SidebarInput, TextEdit,
    commands::{REGISTRY, SIDEBAR},
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
    /// The height a page motion moves, in rows.
    pub page_rows: usize,
    /// The local date, which completing a block and the date keys write.
    pub today: Date,
    /// Whether keys navigate or edit text, which decides their bindings.
    pub mode: Mode,
    /// Whether the command menu is open, which takes the keys of edit mode (D-50).
    pub command_menu: bool,
    /// The popup over the outline that takes the keys of navigate mode, if one is open.
    pub overlay: Option<Overlay>,
    /// Whether the shortcuts panel is open, which `Esc` closes in navigate mode (FR-KEY-04).
    pub shortcuts: bool,
    /// Whether the sidebar is open, and so has the keys (FR-UI-01, D-79).
    pub sidebar: bool,
}

impl Context {
    /// What a command's message carries of this context.
    fn inputs(&self) -> Inputs {
        Inputs {
            today: self.today,
            page_rows: self.page_rows,
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
    /// Makes the focused pane narrower, as widths are the frontend's to choose (FR-UI-03).
    NarrowPane,
    /// Makes the focused pane wider.
    WidenPane,
    /// Scrolls the shortcuts panel down a page, as scrolling is the frontend's (D-55).
    ScrollShortcutsDown,
    /// Scrolls the shortcuts panel up a page.
    ScrollShortcutsUp,
}

/// A popup over the outline in navigate mode, which takes its keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    /// The color picker (FR-TAG-05).
    ColorPicker,
    /// The date prompt (FR-TAG-03).
    DatePrompt,
    /// The type picker (FR-BLK-08).
    TypePicker,
    /// The details popup, which any key closes (D-3).
    Details,
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

/// Conflict prompt bindings: the choices, scrolling the diff and quitting.
const PROMPT: &[PromptBinding] = &[
    (key!(ctrl 'q'), |_| PromptKey::Send(Message::Quit)),
    (key!('m'), |_| PromptKey::Send(Message::KeepMine)),
    (key!('t'), |_| PromptKey::Send(Message::TakeTheirs)),
    (key!('d'), |_| PromptKey::Send(Message::ToggleDiff)),
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
/// quits, a binding, else the change to the text it asks for.
fn settings_text_message(key: Key) -> Option<Message> {
    if key == key!(ctrl 'q') {
        return Some(Message::Quit);
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

/// The date-prompt message for `key`: `Ctrl+Q` quits, a prompt binding, else the change to the
/// text it asks for.
fn date_prompt_message(key: Key) -> Option<Message> {
    if key == key!(ctrl 'q') {
        return Some(Message::Quit);
    }
    let input = DATE_PROMPT
        .iter()
        .find(|(bound, _)| *bound == key)
        .map(|&(_, input)| input)
        .or_else(|| text_edit(key).map(DateInput::Edit))?;
    Some(Message::DatePrompt(input))
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

/// The Save As message for `key`: `Ctrl+Q` quits, a prompt binding, else the change to the path
/// it asks for.
fn save_as_message(key: Key) -> Option<Message> {
    if key == key!(ctrl 'q') {
        return Some(Message::Quit);
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
        Command::NarrowPane => Some(KeyAction::NarrowPane),
        Command::WidenPane => Some(KeyAction::WidenPane),
        Command::ScrollShortcutsDown => Some(KeyAction::ScrollShortcutsDown),
        Command::ScrollShortcutsUp => Some(KeyAction::ScrollShortcutsUp),
        command => command.message(&context.inputs()).map(KeyAction::Send),
    }
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
    /// Each key of the open sidebar with its command.
    sidebar: Vec<(Key, Command)>,
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
        let sidebar = SIDEBAR
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
            sidebar,
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
    /// navigate-mode keys if it is a command of navigate mode, else its keys in the open sidebar,
    /// or the other way round where the `sidebar` is open.
    #[must_use]
    pub fn bindings(&self, command: Command, sidebar: bool) -> String {
        let navigate = || {
            REGISTRY
                .iter()
                .any(|entry| entry.mode == Mode::Navigate && entry.command == command)
                .then(|| self.labels(command, Mode::Navigate))
        };
        let focused_sidebar = || {
            let keys: Vec<String> = self
                .sidebar
                .iter()
                .filter(|&&(_, bound)| bound == command)
                .map(|&(key, _)| key_label(key))
                .collect();
            (!keys.is_empty()).then(|| keys.join(", "))
        };
        let keys = if sidebar {
            focused_sidebar().or_else(navigate)
        } else {
            navigate().or_else(focused_sidebar)
        };
        keys.unwrap_or_default()
    }

    /// What the navigate-mode binding whose keys are `keys` asks for, if any.
    fn bound(&self, keys: &[Key], context: &Context) -> Option<KeyAction> {
        let (_, command) = self.navigate.iter().find(|(bound, _)| bound == keys)?;
        action(*command, context)
    }

    /// What `pressed` asks for while the sidebar is open: its own keys alone (keybindings.md §4,
    /// D-79).
    fn sidebar_action(&self, pressed: Key, context: &Context) -> Option<KeyAction> {
        match self.sidebar.iter().find(|(key, _)| *key == pressed) {
            Some((_, command)) => action(*command, context),
            None => typed_char(pressed)
                .map(|c| KeyAction::Send(Message::Sidebar(SidebarInput::Type(c)))),
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
    /// navigate and edit mode alone; repeats, releases and other events are ignored. A key that
    /// continues no binding ends the pending sequence and does nothing, unless the sequence, or
    /// one that timed out, is a binding of its own: then what it asks for comes back and
    /// [`Keymap::replay`] says to handle `event` again once that is done.
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
            let menu = context.command_menu || context.overlay.is_some() || context.sidebar;
            return (!menu).then(|| {
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
                Overlay::Details if pressed == key!(ctrl 'q') => Some(Message::Quit),
                Overlay::Details => Some(Message::CloseDetails),
            };
            return message.map(KeyAction::Send);
        }
        if context.sidebar {
            return self.sidebar_action(pressed, context);
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
        self.pending
            .iter()
            .map(|(_, code)| code.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyEvent, KeyEventKind, KeyEventState};

    use super::*;
    use crate::{
        app::{
            Location, NewBlock, PaneChange, SelectionChange, SidebarInput, SpecialView, Symbols,
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
                    | KeyAction::NarrowPane
                    | KeyAction::WidenPane
                    | KeyAction::ScrollShortcutsDown
                    | KeyAction::ScrollShortcutsUp => None,
                })
        }
    }

    /// How the sequence `keys`, in the config file's notation, reads on screen.
    fn label(keys: &str) -> Option<String> {
        parse_sequence(keys).map(|keys| sequence_label(&keys))
    }

    const PAGE: usize = 7;

    const TODAY: Date = jiff::civil::date(2026, 10, 5);

    const CONTEXT: &Context = &Context {
        page_rows: PAGE,
        today: TODAY,
        mode: Mode::Navigate,
        command_menu: false,
        overlay: None,
        shortcuts: false,
        sidebar: false,
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
        let next = Some(&Message::FocusNextSibling);
        let previous = Some(&Message::FocusPreviousSibling);
        check(KeyCode::Down, KeyModifiers::CONTROL, next);
        check(KeyCode::Up, KeyModifiers::CONTROL, previous);
        assert_eq!(messages("gj", CONTEXT), [None, next.cloned()]);
        assert_eq!(messages("gk", CONTEXT), [None, previous.cloned()]);
        assert_eq!(messages("j", CONTEXT), [Some(Message::FocusNext)]);
    }

    // FR-NAV-01
    #[test]
    fn page_keys_carry_the_page_height() {
        let down = Some(&Message::PageDown { rows: PAGE });
        let up = Some(&Message::PageUp { rows: PAGE });
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
            .chain(SIDEBAR.iter().copied());
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
    // window, the prompts and the conflict prompt are all sent alike by both key reportings.
    #[test]
    fn every_key_of_the_other_contexts_is_both_variants_at_once() {
        let keys = TEXT
            .iter()
            .map(|&(key, _)| key)
            .chain(MENU.iter().map(|&(key, _)| key))
            .chain(SETTINGS.iter().map(|(key, _)| *key))
            .chain(SETTINGS_TEXT.iter().map(|(key, _)| *key))
            .chain(DATE_PROMPT.iter().map(|&(key, _)| key))
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

    // keybindings.md §6: every bound command, and opening a choice in a new pane from Go to
    // Symbol, has a sequence of keys that no common terminal takes for itself. Pasting is the
    // exception: a terminal that takes `Ctrl+V` pastes the clipboard itself, which the app takes
    // as pasted text.
    #[test]
    fn every_command_has_keys_no_common_terminal_takes() {
        let bindings = REGISTRY
            .iter()
            .filter(|entry| !matches!(entry.command, Command::Paste(_)))
            .map(|entry| (format!("{:?} {:?}", entry.command, entry.mode), entry.keys))
            .chain(
                SIDEBAR
                    .iter()
                    .map(|&(command, keys)| (format!("{command:?} in the sidebar"), keys)),
            );
        for (command, keys) in bindings.filter(|(_, keys)| !keys.is_empty()) {
            assert!(
                keys.iter().any(|keys| {
                    parse_sequence(keys)
                        .expect("registry keys parse")
                        .into_iter()
                        .all(|key| taken_by(key).is_none())
                }),
                "{command}: {keys:?}"
            );
        }
        assert!(
            OPEN_IN_NEW_PANE.iter().any(|&key| taken_by(key).is_none()),
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

    // D-3, D-42: plain characters, which both key reportings send alike.
    #[test]
    fn m_i_opens_the_details_and_any_key_closes_them() {
        assert_eq!(messages("mi", CONTEXT), [None, Some(Message::OpenDetails)]);
        let open = &Context {
            overlay: Some(Overlay::Details),
            ..*CONTEXT
        };
        let mut keymap = Keymap::default();
        let mut press = |code, modifiers| keymap.message_for(&key(code, modifiers), ms(0), open);
        let none = KeyModifiers::NONE;
        for code in [
            KeyCode::Esc,
            KeyCode::Char('j'),
            KeyCode::Char('g'),
            KeyCode::Enter,
        ] {
            assert_eq!(press(code, none), Some(Message::CloseDetails), "{code:?}");
        }
        assert_eq!(
            press(KeyCode::Char('q'), KeyModifiers::CONTROL),
            Some(Message::Quit)
        );
        assert_eq!(keymap.pending(), "");
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
        keymap.message_for(&key(KeyCode::Char('q'), KeyModifiers::NONE), ms(0), CONTEXT);
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
        assert_eq!(prompt(KeyCode::Char('m'), none), send(Message::KeepMine));
        assert_eq!(prompt(KeyCode::Char('t'), none), send(Message::TakeTheirs));
        assert_eq!(prompt(KeyCode::Char('d'), none), send(Message::ToggleDiff));
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
            ..KeyEvent::new(KeyCode::Char('m'), none)
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

    // FR-OPS-06, editing.md §4
    #[test]
    fn text_pasted_into_the_terminal_is_pasted_below_except_in_menus_and_prompts() {
        let pasted = Event::Paste("a\nb".to_owned());
        let expected = Some(Message::Pasted {
            text: "a\nb".to_owned(),
            at: Paste::Below,
        });
        let mut keymap = Keymap::default();
        assert_eq!(keymap.message_for(&pasted, ms(0), CONTEXT), expected);
        assert_eq!(keymap.message_for(&pasted, ms(0), EDITING), expected);
        for context in [
            Context {
                command_menu: true,
                ..*EDITING
            },
            Context {
                overlay: Some(Overlay::ColorPicker),
                ..*CONTEXT
            },
            Context {
                overlay: Some(Overlay::DatePrompt),
                ..*CONTEXT
            },
            *IN_SIDEBAR,
        ] {
            assert_eq!(keymap.message_for(&pasted, ms(0), &context), None);
        }
    }

    // FR-UI-01, D-42, D-79: `Ctrl+G` is the same key with and without the kitty protocol, and
    // the sidebar's old keys are gone.
    #[test]
    fn ctrl_g_opens_the_sidebar() {
        check(
            KeyCode::Char('g'),
            KeyModifiers::CONTROL,
            Some(&Message::ToggleSidebar),
        );
        check(KeyCode::Char('0'), KeyModifiers::ALT, None);
        assert_eq!(messages("g0", CONTEXT), [None, None]);
    }

    // keybindings.md §4, D-79: the open sidebar is a window that keeps the keys.
    #[test]
    fn the_open_sidebar_takes_no_keys_that_focus_a_pane() {
        let typed = |c| Some(Message::Sidebar(SidebarInput::Type(c)));
        assert_eq!(messages("g1", IN_SIDEBAR), [typed('g'), typed('1')]);
        let mut keymap = Keymap::default();
        for (code, modifiers) in [
            (KeyCode::Char('2'), KeyModifiers::ALT),
            (KeyCode::Right, KeyModifiers::CONTROL | KeyModifiers::SHIFT),
            (KeyCode::Char('w'), KeyModifiers::CONTROL),
        ] {
            assert_eq!(
                keymap.message_for(&key(code, modifiers), ms(0), IN_SIDEBAR),
                None
            );
            assert_eq!(keymap.pending(), "", "nothing waits for a sequence");
        }
        assert_eq!(
            keymap.message_for(&key(KeyCode::Down, KeyModifiers::NONE), ms(1), IN_SIDEBAR),
            Some(Message::Sidebar(SidebarInput::Next)),
        );
    }

    const IN_SIDEBAR: &Context = &Context {
        sidebar: true,
        ..*CONTEXT
    };

    // FR-UI-01, keybindings.md §4, D-86
    #[test]
    fn the_open_sidebar_takes_its_keys_and_ctrl_q_and_ctrl_g_and_other_characters_filter() {
        let none = KeyModifiers::NONE;
        let ctrl = KeyModifiers::CONTROL;
        let sidebar = |input| Some(Message::Sidebar(input));
        for (code, modifiers, expected) in [
            (KeyCode::Down, none, sidebar(SidebarInput::Next)),
            (KeyCode::Char('n'), ctrl, sidebar(SidebarInput::Next)),
            (KeyCode::Up, none, sidebar(SidebarInput::Previous)),
            (KeyCode::Char('p'), ctrl, sidebar(SidebarInput::Previous)),
            (KeyCode::Left, none, sidebar(SidebarInput::Collapse)),
            (KeyCode::Right, none, sidebar(SidebarInput::Expand)),
            (KeyCode::Enter, none, sidebar(SidebarInput::Choose)),
            (KeyCode::Backspace, none, sidebar(SidebarInput::Erase)),
            (KeyCode::Esc, none, sidebar(SidebarInput::Leave)),
            (KeyCode::Char('q'), ctrl, Some(Message::Quit)),
            (KeyCode::Char('g'), ctrl, Some(Message::ToggleSidebar)),
            (KeyCode::Char('b'), ctrl, None),
            (KeyCode::Char('j'), none, sidebar(SidebarInput::Type('j'))),
            (
                KeyCode::Char('W'),
                KeyModifiers::SHIFT,
                sidebar(SidebarInput::Type('W')),
            ),
            (KeyCode::Char(' '), none, sidebar(SidebarInput::Type(' '))),
            (KeyCode::Char('j'), ctrl, None),
        ] {
            let mut keymap = Keymap::default();
            assert_eq!(
                keymap.message_for(&key(code, modifiers), ms(0), IN_SIDEBAR),
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
                Some(Message::ToggleSidebar)
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

    // FR-KEY-06, keybindings.md §1, §2 and §4, D-42
    #[test]
    fn backslash_shift_f10_and_the_menu_key_open_the_context_menu_in_the_pane_and_sidebar_without_backslash()
     {
        let open = Some(&Message::OpenContextMenu);
        check(KeyCode::Char('\\'), KeyModifiers::NONE, open);
        check(KeyCode::F(10), KeyModifiers::SHIFT, open);
        check(KeyCode::Menu, KeyModifiers::NONE, open);
        check(KeyCode::F(10), KeyModifiers::NONE, None);
        let mut keymap = Keymap::default();
        let sidebar = &Context {
            sidebar: true,
            ..*CONTEXT
        };
        for (at, (code, modifiers)) in [
            (KeyCode::F(10), KeyModifiers::SHIFT),
            (KeyCode::Menu, KeyModifiers::NONE),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                keymap
                    .message_for(&key(code, modifiers), ms(at as u64), sidebar)
                    .as_ref(),
                open,
                "{code:?}"
            );
        }
        assert_eq!(
            keymap.message_for(
                &key(KeyCode::Char('\\'), KeyModifiers::NONE),
                ms(9),
                sidebar
            ),
            Some(Message::Sidebar(SidebarInput::Type('\\'))),
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
    fn a_commands_bindings_are_its_navigate_keys_or_its_sidebar_keys() {
        let keymap = Keymap::default();
        let bindings = |command, sidebar| keymap.bindings(command, sidebar);
        assert_eq!(bindings(Command::ContextMenu, false), "\\, Shift+F10, Menu");
        assert_eq!(bindings(Command::Count, false), "");
        assert_eq!(
            bindings(Command::Retype(Typed::Task), false),
            "",
            "edit mode's"
        );
        assert_eq!(
            bindings(Command::Sidebar(SidebarInput::Choose), false),
            "Enter"
        );
        assert_eq!(
            bindings(Command::Sidebar(SidebarInput::Next), false),
            "↓, Ctrl+N"
        );
        let open = Command::Pane(PaneChange::Open);
        assert_eq!(bindings(open, false), "Ctrl+\\, Ctrl+W v");
        assert_eq!(
            bindings(open, true),
            "Alt+Enter, Ctrl+V, Ctrl+T",
            "the sidebar's own"
        );
        assert_eq!(bindings(Command::Undo, true), "z, Ctrl+Z", "no sidebar key");
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
    // under the kitty protocol, as `p` with `Shift`; `gp` still focuses the parent.
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
        assert_eq!(messages("gp", CONTEXT), [None, Some(Message::FocusParent)]);
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
            (PaneChange::FocusLeft, "Ctrl+Shift+←, Ctrl+W ←, Ctrl+W h"),
            (PaneChange::FocusRight, "Ctrl+Shift+→, Ctrl+W →, Ctrl+W l"),
        ] {
            assert_eq!(keymap.bindings(Command::Pane(change), true), keys);
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

    // FR-TAG-13, D-92, D-42: `g*` goes to Starred whether `*` arrives plain or, under the kitty
    // protocol, with `Shift`.
    #[test]
    fn g_star_goes_to_starred_in_both_key_reportings() {
        let starred = Some(Message::GoTo(Location::View(SpecialView::Starred)));
        assert_eq!(messages("g*", CONTEXT), [None, starred.clone()]);
        let mut keymap = Keymap::default();
        let sent: Vec<_> = [
            key(KeyCode::Char('g'), KeyModifiers::NONE),
            key(KeyCode::Char('*'), KeyModifiers::SHIFT),
        ]
        .iter()
        .map(|key| keymap.message_for(key, ms(0), CONTEXT))
        .collect();
        assert_eq!(sent, [None, starred]);
        assert_eq!(
            Keymap::default().bindings(Command::GoToView(SpecialView::Starred), true),
            "g*"
        );
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
    // variant `Ctrl+V` in Go to Symbol and the sidebar, and `Ctrl+T` stands beside both, which
    // Windows Terminal takes; they mean nothing to other pickers.
    #[test]
    fn alt_enter_ctrl_v_and_ctrl_t_open_the_choice_of_go_to_symbol_and_the_sidebar_in_a_new_pane() {
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
                actions(&[(code, modifiers)], IN_SIDEBAR),
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
}
