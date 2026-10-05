//! Key events to app messages ([keybindings.md](../../../docs/specs/keybindings.md)).

use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyModifiers};
use jiff::civil::Date;

use crate::{
    app::{MenuInput, Message, Mode, Motion, TextEdit},
    document::{DateChange, DateTag, format::Typed},
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
    /// Whether the type menu is open, which takes the keys of edit mode (D-44).
    pub type_menu: bool,
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
    (&[key!('z')], |_| Message::Undo),
    (&[key!(ctrl 'z')], |_| Message::Undo),
    (&[key!('Z')], |_| Message::Redo),
    (&[key!(ctrl 'Z')], |_| Message::Redo),
    (&[key!(ctrl 'y')], |_| Message::Redo),
    (&[key!(Enter)], |_| Message::EditAtEnd),
    (&[key!('o')], |_| Message::NewBelow),
    (&[key!('O')], |_| Message::NewAbove),
    (&[key!('i')], |_| Message::NewLastChild),
    (&[key!('I')], |_| Message::NewFirstChild),
    (&[key!('P')], |_| Message::NewTop(Typed::Project)),
    (&[key!('p')], |_| Message::NewTypedBelow(Typed::Project)),
    (&[key!('S')], |_| Message::NewTop(Typed::Section)),
    (&[key!('s')], |_| Message::NewTypedBelow(Typed::Section)),
    (&[key!('A')], |_| Message::NewTop(Typed::Task)),
    (&[key!('a')], |_| Message::NewTypedBelow(Typed::Task)),
    (&[key!('d'), key!('d')], |_| Message::Delete),
    (&[key!(Delete)], |_| Message::Delete),
    (&[key!(Tab)], |_| Message::Indent),
    (&[key!(shift BackTab)], |_| Message::Outdent),
    (&[key!(alt 'k')], |_| Message::MoveUp),
    (&[key!(alt Up)], |_| Message::MoveUp),
    (&[key!(alt 'j')], |_| Message::MoveDown),
    (&[key!(alt Down)], |_| Message::MoveDown),
];

/// Changing the focused block's `tag` date as `change` says, from the local date (FR-TAG-06).
fn date(tag: DateTag, change: DateChange, context: &Context) -> Message {
    Message::ChangeDate {
        tag,
        change,
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

/// Edit-mode bindings ([keybindings.md §3], [editing.md §2]); characters without other bindings
/// are typed.
///
/// [keybindings.md §3]: ../../../docs/specs/keybindings.md#3-edit-mode
/// [editing.md §2]: ../../../docs/specs/editing.md#2-edit-mode
const EDIT: &[Binding] = &[
    (&[key!(ctrl 'q')], |_| Message::Quit),
    (&[key!(Esc)], |_| Message::StopEditing),
    (&[key!('/')], |_| Message::OpenTypeMenu),
    (&[key!(Enter)], |_| Message::NewNext),
    (&[key!(Tab)], |_| Message::Indent),
    (&[key!(shift BackTab)], |_| Message::Outdent),
    (&[key!(Left)], |_| go(Motion::Left, false)),
    (&[key!(Right)], |_| go(Motion::Right, false)),
    (&[key!(ctrl Left)], |_| go(Motion::WordLeft, false)),
    (&[key!(ctrl Right)], |_| go(Motion::WordRight, false)),
    (&[key!(alt 'b')], |_| go(Motion::WordLeft, false)),
    (&[key!(alt 'f')], |_| go(Motion::WordRight, false)),
    (&[key!(Home)], |_| go(Motion::Start, false)),
    (&[key!(End)], |_| go(Motion::End, false)),
    (&[key!(Up)], |_| Message::EditPrevious),
    (&[key!(Down)], |_| Message::EditNext),
    (&[key!(shift Left)], |_| go(Motion::Left, true)),
    (&[key!(shift Right)], |_| go(Motion::Right, true)),
    (&[key!(ctrl shift Left)], |_| go(Motion::WordLeft, true)),
    (&[key!(ctrl shift Right)], |_| go(Motion::WordRight, true)),
    (&[key!(shift Home)], |_| go(Motion::Start, true)),
    (&[key!(shift End)], |_| go(Motion::End, true)),
    (&[key!(shift Up)], |_| go(Motion::Start, true)),
    (&[key!(shift Down)], |_| go(Motion::End, true)),
    (&[key!(Backspace)], |_| {
        Message::Text(TextEdit::DeleteBackward)
    }),
    (&[key!(Delete)], |_| Message::Text(TextEdit::DeleteForward)),
    (&[key!(ctrl 'w')], |_| {
        Message::Text(TextEdit::DeleteWordBackward)
    }),
    (&[key!(alt Backspace)], |_| {
        Message::Text(TextEdit::DeleteWordBackward)
    }),
    (&[key!(ctrl 'a')], |_| Message::Text(TextEdit::SelectAll)),
    (&[key!(ctrl 'z')], |_| Message::Undo),
    (&[key!(ctrl 'Z')], |_| Message::Redo),
    (&[key!(ctrl 'y')], |_| Message::Redo),
];

/// Moving the caret by `motion`, extending the selection with `select`.
fn go(motion: Motion, select: bool) -> Message {
    Message::Text(TextEdit::Move { motion, select })
}

/// Bindings of the open type menu ([keybindings.md §3], [editing.md §2.3]); characters without
/// other bindings filter it, and other keys do nothing.
///
/// [keybindings.md §3]: ../../../docs/specs/keybindings.md#3-edit-mode
/// [editing.md §2.3]: ../../../docs/specs/editing.md#23-type-menu
const TYPE_MENU: &[(Key, MenuInput)] = &[
    (key!(Esc), MenuInput::Dismiss),
    (key!(Enter), MenuInput::Choose),
    (key!(Up), MenuInput::Previous),
    (key!(ctrl 'p'), MenuInput::Previous),
    (key!(Down), MenuInput::Next),
    (key!(ctrl 'n'), MenuInput::Next),
    (key!(Backspace), MenuInput::Erase),
];

/// The type-menu message for `key`: `Ctrl+Q` quits, a menu binding, else typing the character
/// it is, as in edit mode.
fn type_menu_message(key: Key) -> Option<Message> {
    if key == key!(ctrl 'q') {
        return Some(Message::Quit);
    }
    if let Some((_, input)) = TYPE_MENU.iter().find(|(bound, _)| *bound == key) {
        return Some(Message::TypeMenu(*input));
    }
    typed_char(key).map(|c| Message::TypeMenu(MenuInput::Type(c)))
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

/// The edit-mode message for `key`: its binding, else typing the character it is.
fn edit_message(key: Key, context: &Context) -> Option<Message> {
    if let Some((_, message)) = EDIT.iter().find(|(keys, _)| *keys == [key]) {
        return Some(message(context));
    }
    typed_char(key).map(|c| Message::Text(TextEdit::Insert(c)))
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
    /// `context`. Only key presses count; repeats, releases and other events are ignored. A
    /// key that continues no binding ends the pending sequence and does nothing.
    pub fn message_for(
        &mut self,
        event: &Event,
        now: Duration,
        context: &Context,
    ) -> Option<Message> {
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
        if context.mode == Mode::Edit {
            self.pending.clear();
            return if context.type_menu {
                type_menu_message(pressed)
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
        type_menu: false,
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
        check(KeyCode::Char('o'), none, Some(&Message::NewBelow));
        check(
            KeyCode::Char('O'),
            KeyModifiers::SHIFT,
            Some(&Message::NewAbove),
        );
        check(KeyCode::Char('i'), none, Some(&Message::NewLastChild));
        check(
            KeyCode::Char('I'),
            KeyModifiers::SHIFT,
            Some(&Message::NewFirstChild),
        );
        check(KeyCode::Char('o'), KeyModifiers::ALT, None);
        check(KeyCode::Delete, none, Some(&Message::Delete));
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
        for (lower, typed) in [
            ('p', Typed::Project),
            ('s', Typed::Section),
            ('a', Typed::Task),
        ] {
            let upper = lower.to_ascii_uppercase();
            let top = Message::NewTop(typed);
            check(KeyCode::Char(upper), KeyModifiers::SHIFT, Some(&top));
            check(KeyCode::Char(upper), KeyModifiers::NONE, Some(&top));
            let below = Message::NewTypedBelow(typed);
            check(KeyCode::Char(lower), KeyModifiers::NONE, Some(&below));
        }
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
        for c in ['j', 'x', 'g', 'z', 'i', ' ', '@', 'é'] {
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

    const IN_MENU: &Context = &Context {
        type_menu: true,
        ..*EDITING
    };

    /// The message each key sends while the type menu is open.
    #[track_caller]
    fn check_menu(code: KeyCode, modifiers: KeyModifiers, expected: Option<MenuInput>) {
        let mut keymap = Keymap::default();
        assert_eq!(
            keymap.message_for(&key(code, modifiers), ms(0), IN_MENU),
            expected.map(Message::TypeMenu),
            "{modifiers:?} {code:?}"
        );
    }

    // D-44
    #[test]
    fn slash_opens_the_type_menu_in_edit_mode() {
        check_editing(
            KeyCode::Char('/'),
            KeyModifiers::NONE,
            Some(&Message::OpenTypeMenu),
        );
    }

    // D-44, keybindings.md §3
    #[test]
    fn the_type_menu_takes_the_keys_of_pickers() {
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

    #[test]
    fn shift_only_drops_from_characters() {
        check(KeyCode::Down, KeyModifiers::SHIFT, None);
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
}
