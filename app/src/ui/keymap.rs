//! Key events to app messages ([keybindings.md](../../../docs/specs/keybindings.md)).

use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyModifiers};
use jiff::civil::Date;

use crate::{
    app::{
        DateInput, MenuInput, Message, Mode, Motion, NewBlock, Paste, SelectionChange, TextEdit,
    },
    document::{DateChange, DateTag, Flag, TagColor, format::Typed, inline::Marker},
};

/// How long a started multi-key sequence waits for its next key ([keybindings.md §1]).
///
/// [keybindings.md §1]: ../../../docs/specs/keybindings.md#1-terminal-constraints
pub const SEQUENCE_TIMEOUT: Duration = Duration::from_secs(1);

/// One key press: its modifiers and code, with `Shift` dropped from characters because the
/// character already says whether it is shifted.
type Key = (KeyModifiers, KeyCode);

/// A binding's keys and the message they send, given what the frontend knows.
type Binding = (&'static [Key], fn(&Context) -> Message);

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

/// Navigate-mode bindings ([keybindings.md §2]).
///
/// [keybindings.md §2]: ../../../docs/specs/keybindings.md#2-navigate-mode
const NAVIGATE: &[Binding] = &[
    (&[key!(ctrl 'q')], |_| Message::Quit),
    (&[key!('j')], |_| Message::FocusNext),
    (&[key!(Down)], |_| Message::FocusNext),
    (&[key!('k')], |_| Message::FocusPrevious),
    (&[key!(Up)], |_| Message::FocusPrevious),
    (&[key!('g'), key!('g')], |_| Message::FocusFirst),
    (&[key!(Home)], |_| Message::FocusFirst),
    (&[key!('G')], |_| Message::FocusLast),
    (&[key!(End)], |_| Message::FocusLast),
    (&[key!(ctrl 'd')], |context| Message::PageDown {
        rows: context.page_rows,
    }),
    (&[key!(PageDown)], |context| Message::PageDown {
        rows: context.page_rows,
    }),
    (&[key!(ctrl 'u')], |context| Message::PageUp {
        rows: context.page_rows,
    }),
    (&[key!(PageUp)], |context| Message::PageUp {
        rows: context.page_rows,
    }),
    (&[key!('h')], |_| Message::Collapse),
    (&[key!(Left)], |_| Message::Collapse),
    (&[key!('l')], |_| Message::Expand),
    (&[key!(Right)], |_| Message::Expand),
    (&[key!(ctrl Down)], |_| Message::FocusNextSibling),
    (&[key!(ctrl Up)], |_| Message::FocusPreviousSibling),
    (&[key!('g'), key!('p')], |_| Message::FocusParent),
    (&[key!('g'), key!('l')], |_| Message::FocusLastChild),
    (&[key!('\'')], |_| Message::ToggleFold),
    (&[key!(' ')], |_| Message::ToggleFold),
    (&[key!('"')], |_| Message::ToggleSubtree),
    (&[key!('=')], |_| Message::ExpandLevel),
    (&[key!('-')], |_| Message::CollapseLevel),
    (&[key!('+')], |_| Message::ExpandAll),
    (&[key!('_')], |_| Message::CollapseAll),
    (&[key!('.')], |_| Message::ZoomIn),
    (&[key!(alt 'l')], |_| Message::ZoomIn),
    (&[key!(alt Right)], |_| Message::ZoomIn),
    (&[key!('g'), key!('z')], |_| Message::ZoomIn),
    (&[key!('>')], |_| Message::ZoomOut),
    (&[key!(alt 'h')], |_| Message::ZoomOut),
    (&[key!(alt Left)], |_| Message::ZoomOut),
    (&[key!('g'), key!('u')], |_| Message::ZoomOut),
    (&[key!(ctrl '\'')], |_| Message::GoHome),
    (&[key!('g'), key!('h')], |_| Message::GoHome),
    (&[key!('H')], |_| Message::Back),
    (&[key!(alt shift Left)], |_| Message::Back),
    (&[key!('L')], |_| Message::Forward),
    (&[key!(alt shift Right)], |_| Message::Forward),
    (&[key!('x')], |context| Message::ToggleDone {
        today: context.today,
    }),
    (&[key!(ctrl Enter)], |context| Message::ToggleDone {
        today: context.today,
    }),
    (&[key!('t')], |c| date(DateTag::Start, DateChange::Today, c)),
    (&[key!('y')], |c| {
        date(DateTag::Start, DateChange::Tomorrow, c)
    }),
    (&[key!('T')], |c| date(DateTag::Due, DateChange::Today, c)),
    (&[key!('Y')], |c| {
        date(DateTag::Due, DateChange::Tomorrow, c)
    }),
    (&[key!('[')], |c| {
        date(DateTag::Start, DateChange::Earlier, c)
    }),
    (&[key!(']')], |c| date(DateTag::Start, DateChange::Later, c)),
    (&[key!('{')], |c| date(DateTag::Due, DateChange::Earlier, c)),
    (&[key!('}')], |c| date(DateTag::Due, DateChange::Later, c)),
    (&[key!('c'), key!('c')], |_| Message::CycleColor),
    (&[key!('c'), key!('1')], |_| {
        Message::SetColor(Some(TagColor::Red))
    }),
    (&[key!('c'), key!('2')], |_| {
        Message::SetColor(Some(TagColor::Orange))
    }),
    (&[key!('c'), key!('3')], |_| {
        Message::SetColor(Some(TagColor::Yellow))
    }),
    (&[key!('c'), key!('4')], |_| {
        Message::SetColor(Some(TagColor::Green))
    }),
    (&[key!('c'), key!('5')], |_| {
        Message::SetColor(Some(TagColor::Blue))
    }),
    (&[key!('c'), key!('6')], |_| {
        Message::SetColor(Some(TagColor::Violet))
    }),
    (&[key!('C')], |_| Message::SetColor(None)),
    (&[key!('m'), key!('c')], |_| Message::OpenColorPicker),
    (&[key!('m'), key!('t')], |_| {
        Message::ToggleFlag(Flag::Today)
    }),
    (&[key!('m'), key!('b')], |_| {
        Message::ToggleFlag(Flag::Bookmark)
    }),
    (&[key!('m'), key!('s')], |c| date_prompt(DateTag::Start, c)),
    (&[key!('m'), key!('d')], |c| date_prompt(DateTag::Due, c)),
    (&[key!('m'), key!('y')], |_| Message::OpenTypePicker),
    (&[key!('m'), key!('i')], |_| Message::OpenDetails),
    (&[key!('z')], |_| Message::Undo),
    (&[key!(ctrl 'z')], |_| Message::Undo),
    (&[key!('Z')], |_| Message::Redo),
    (&[key!(ctrl 'Z')], |_| Message::Redo),
    (&[key!(ctrl 'y')], |_| Message::Redo),
    (&[key!(Enter)], |_| Message::EditAtEnd),
    (&[key!('o')], |_| Message::New(NewBlock::Below, None)),
    (&[key!('O')], |_| Message::New(NewBlock::Above, None)),
    (&[key!('i')], |_| Message::New(NewBlock::LastChild, None)),
    (&[key!('I')], |_| Message::New(NewBlock::FirstChild, None)),
    (&[key!('P')], |_| {
        Message::New(NewBlock::Start, Some(Typed::Project))
    }),
    (&[key!('p')], |_| {
        Message::New(NewBlock::Below, Some(Typed::Project))
    }),
    (&[key!('S')], |_| Message::SetSection(true)),
    (&[key!('s')], |_| Message::SetSection(false)),
    (&[key!('A')], |_| {
        Message::New(NewBlock::Start, Some(Typed::Task))
    }),
    (&[key!('a')], |_| {
        Message::New(NewBlock::Below, Some(Typed::Task))
    }),
    (&[key!('d'), key!('d')], |_| Message::Delete),
    (&[key!(Delete)], |_| Message::Delete),
    (&[key!('D')], |_| Message::Duplicate),
    (&[key!(ctrl 'c')], |_| Message::Copy),
    (&[key!(ctrl 'x')], |_| Message::Cut),
    (&[key!(ctrl 'v')], |_| Message::Paste(Paste::Below)),
    (&[key!(alt 'v')], |_| Message::Paste(Paste::Above)),
    (&[key!(Tab)], |_| Message::Indent),
    (&[key!(shift BackTab)], |_| Message::Outdent),
    (&[key!(alt 'k')], |_| Message::MoveUp),
    (&[key!(alt Up)], |_| Message::MoveUp),
    (&[key!(alt 'j')], |_| Message::MoveDown),
    (&[key!(alt Down)], |_| Message::MoveDown),
    (&[key!('g'), key!('x')], |_| Message::OpenUrl),
    (&[key!('g'), key!('G')], |_| Message::Group),
    (&[key!('J')], |_| {
        Message::Select(SelectionChange::ExtendDown)
    }),
    (&[key!(shift Down)], |_| {
        Message::Select(SelectionChange::ExtendDown)
    }),
    (&[key!('K')], |_| Message::Select(SelectionChange::ExtendUp)),
    (&[key!(shift Up)], |_| {
        Message::Select(SelectionChange::ExtendUp)
    }),
    (&[key!('v')], |_| Message::Select(SelectionChange::Toggle)),
    (&[key!(ctrl 'a')], |_| Message::Select(SelectionChange::All)),
    (&[key!(Esc)], |_| Message::Select(SelectionChange::Clear)),
];

/// Changing the focused block's `tag` date as `change` says, from the local date (FR-TAG-06).
fn date(tag: DateTag, change: DateChange, context: &Context) -> Message {
    Message::ChangeDate {
        tag,
        change,
        today: context.today,
    }
}

/// Opening the prompt for the focused block's `tag` date, resolved against the local date
/// (FR-TAG-03).
fn date_prompt(tag: DateTag, context: &Context) -> Message {
    Message::OpenDatePrompt {
        tag,
        today: context.today,
    }
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

/// Edit-mode bindings ([keybindings.md §3], [editing.md §2]); the [`TEXT`] keys change the text,
/// and characters without other bindings are typed.
///
/// [keybindings.md §3]: ../../../docs/specs/keybindings.md#3-edit-mode
/// [editing.md §2]: ../../../docs/specs/editing.md#2-edit-mode
const EDIT: &[Binding] = &[
    (&[key!(ctrl 'q')], |_| Message::Quit),
    (&[key!(Esc)], |_| Message::StopEditing),
    (&[key!('/')], |context| Message::OpenCommandMenu {
        today: context.today,
    }),
    (&[key!(' ')], |context| Message::TypeSpace {
        today: context.today,
    }),
    (&[key!(Enter)], |_| Message::NewNext),
    (&[key!(Tab)], |_| Message::Indent),
    (&[key!(shift BackTab)], |_| Message::Outdent),
    (&[key!(ctrl 't')], |_| Message::CycleType),
    (&[key!(Up)], |_| Message::EditPrevious),
    (&[key!(Down)], |_| Message::EditNext),
    (&[key!(ctrl 'b')], |_| {
        Message::Text(TextEdit::Toggle(Marker::Bold))
    }),
    (&[key!(alt '`')], |_| {
        Message::Text(TextEdit::Toggle(Marker::Code))
    }),
    // `Alt` needs a setting on macOS, so inline code also has a key without it (keybindings.md §1).
    (&[key!(ctrl 'e')], |_| {
        Message::Text(TextEdit::Toggle(Marker::Code))
    }),
    (&[key!(ctrl 'z')], |_| Message::Undo),
    (&[key!(ctrl 'Z')], |_| Message::Redo),
    (&[key!(ctrl 'y')], |_| Message::Redo),
    (&[key!(ctrl 'c')], |_| Message::Copy),
    (&[key!(ctrl 'x')], |_| Message::Cut),
    (&[key!(ctrl 'v')], |_| Message::Paste(Paste::Below)),
];

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

/// The message of the open command menu or a picker for `key`, made by `to`: `Ctrl+Q` quits, a
/// menu binding, else typing the character it is, as in edit mode.
fn menu_message(key: Key, to: fn(MenuInput) -> Message) -> Option<Message> {
    if key == key!(ctrl 'q') {
        return Some(Message::Quit);
    }
    if let Some((_, input)) = MENU.iter().find(|(bound, _)| *bound == key) {
        return Some(to(*input));
    }
    typed_char(key).map(|c| to(MenuInput::Type(c)))
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

/// The edit-mode message for `key`: its binding, else the change to the text it asks for.
fn edit_message(key: Key, context: &Context) -> Option<Message> {
    if let Some((_, message)) = EDIT.iter().find(|(keys, _)| *keys == [key]) {
        return Some(message(context));
    }
    text_edit(key).map(Message::Text)
}

/// Turns key presses into messages, holding the keys of a multi-key sequence until it completes,
/// fails to match or times out.
#[derive(Debug, Default)]
pub struct Keymap {
    pending: Vec<Key>,
    /// When the pending sequence started, in the frontend's time.
    started: Duration,
}

impl Keymap {
    /// The message `event` completes at time `now`, if any, carrying what it needs of
    /// `context`. Only key presses and text pasted into the terminal count, the latter in
    /// navigate and edit mode alone; repeats, releases and other events are ignored. A key that
    /// continues no binding ends the pending sequence and does nothing.
    pub fn message_for(
        &mut self,
        event: &Event,
        now: Duration,
        context: &Context,
    ) -> Option<Message> {
        if let Event::Paste(text) = event {
            let menu = context.command_menu || context.overlay.is_some();
            return (!menu).then(|| Message::Pasted {
                text: text.clone(),
                at: Paste::Below,
            });
        }
        let key = event.as_key_press_event()?;
        self.expire(now);
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
            return match overlay {
                Overlay::DatePrompt => date_prompt_message(pressed),
                Overlay::ColorPicker => menu_message(pressed, Message::ColorPicker),
                Overlay::TypePicker => menu_message(pressed, Message::TypePicker),
                Overlay::Details if pressed == key!(ctrl 'q') => Some(Message::Quit),
                Overlay::Details => Some(Message::CloseDetails),
            };
        }
        if context.mode == Mode::Edit {
            self.pending.clear();
            return if context.command_menu {
                menu_message(pressed, Message::CommandMenu)
            } else {
                edit_message(pressed, context)
            };
        }
        self.pending.push(pressed);
        if let Some((_, message)) = NAVIGATE.iter().find(|(keys, _)| *keys == self.pending) {
            self.pending.clear();
            return Some(message(context));
        }
        // No binding equals the pending keys, so any binding they start is longer.
        if !NAVIGATE
            .iter()
            .any(|(keys, _)| keys.starts_with(&self.pending))
        {
            self.pending.clear();
        }
        None
    }

    /// Drops the pending sequence once [`SEQUENCE_TIMEOUT`] has passed since it started.
    pub fn expire(&mut self, now: Duration) {
        if self.deadline().is_some_and(|deadline| now >= deadline) {
            self.pending.clear();
        }
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

    const PAGE: usize = 7;

    const TODAY: Date = jiff::civil::date(2026, 10, 5);

    const CONTEXT: &Context = &Context {
        page_rows: PAGE,
        today: TODAY,
        mode: Mode::Navigate,
        command_menu: false,
        overlay: None,
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
            ('y', DateTag::Start, DateChange::Tomorrow),
            ('T', DateTag::Due, DateChange::Today),
            ('Y', DateTag::Due, DateChange::Tomorrow),
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
        check(
            KeyCode::Char('o'),
            none,
            Some(&Message::New(NewBlock::Below, None)),
        );
        check(
            KeyCode::Char('O'),
            KeyModifiers::SHIFT,
            Some(&Message::New(NewBlock::Above, None)),
        );
        check(
            KeyCode::Char('i'),
            none,
            Some(&Message::New(NewBlock::LastChild, None)),
        );
        check(
            KeyCode::Char('I'),
            KeyModifiers::SHIFT,
            Some(&Message::New(NewBlock::FirstChild, None)),
        );
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

    // FR-MODE-06, D-26
    #[test]
    fn typed_creation_keys() {
        for (lower, typed) in [('p', Typed::Project), ('a', Typed::Task)] {
            let upper = lower.to_ascii_uppercase();
            let top = Message::New(NewBlock::Start, Some(typed));
            check(KeyCode::Char(upper), KeyModifiers::SHIFT, Some(&top));
            check(KeyCode::Char(upper), KeyModifiers::NONE, Some(&top));
            let below = Message::New(NewBlock::Below, Some(typed));
            check(KeyCode::Char(lower), KeyModifiers::NONE, Some(&below));
        }
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
        for c in ['j', 'x', 'g', 'z', 'i', '@', 'é'] {
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
        check(KeyCode::Char('t'), KeyModifiers::CONTROL, None);
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
            expected.map(Message::CommandMenu),
            "{modifiers:?} {code:?}"
        );
    }

    // FR-MODE-05, editing.md §2.1
    #[test]
    fn space_in_edit_mode_is_typed_dated_for_a_shortcut() {
        check_editing(
            KeyCode::Char(' '),
            KeyModifiers::NONE,
            Some(&Message::TypeSpace { today: TODAY }),
        );
        check_editing(
            KeyCode::Char(' '),
            KeyModifiers::SHIFT,
            Some(&Message::TypeSpace { today: TODAY }),
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

    // FR-TAG-04, FR-TAG-05, FR-TAG-06, D-11, D-26, D-42, D-48: plain characters, which both key
    // reportings send alike, and `C`, which the kitty protocol sends as `c` with `Shift`.
    #[test]
    fn color_today_and_bookmark_keys() {
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
            ('t', Message::ToggleFlag(Flag::Today)),
            ('b', Message::ToggleFlag(Flag::Bookmark)),
        ] {
            assert_eq!(
                messages(&format!("m{second}"), CONTEXT),
                [None, Some(expected)]
            );
        }
        assert_eq!(messages("mx", CONTEXT), [None, None]);
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
        let picker = |input| Some(Message::ColorPicker(input));
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
        let picker = |input| Some(Message::TypePicker(input));
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
        keymap.expire(ms(999));
        assert_eq!(keymap.pending(), "g");
        keymap.expire(ms(1_000));
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
            keymap.message_for(
                &key(KeyCode::Char('j'), KeyModifiers::NONE),
                ms(10),
                CONTEXT
            ),
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
        keymap.message_for(&key(KeyCode::Char('w'), KeyModifiers::NONE), ms(0), CONTEXT);
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
        ] {
            assert_eq!(keymap.message_for(&pasted, ms(0), &context), None);
        }
    }
}
