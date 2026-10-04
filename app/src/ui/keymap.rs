//! Key events to app messages ([keybindings.md](../../../docs/specs/keybindings.md)).

use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyModifiers};

use crate::app::Message;

/// How long a started multi-key sequence waits for its next key ([keybindings.md §1]).
///
/// [keybindings.md §1]: ../../../docs/specs/keybindings.md#1-terminal-constraints
pub const SEQUENCE_TIMEOUT: Duration = Duration::from_secs(1);

/// One key press: its modifiers and code, with `Shift` dropped from characters because the
/// character already says whether it is shifted.
type Key = (KeyModifiers, KeyCode);

/// A binding's keys and the message they send, given the rows in a page.
type Binding = (&'static [Key], fn(usize) -> Message);

/// A [`Key`]: `key!('j')` for a character, `key!(ctrl 'q')` with `Ctrl`, `key!(Down)` for a
/// named key.
macro_rules! key {
    (ctrl $c:literal) => {
        (KeyModifiers::CONTROL, KeyCode::Char($c))
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
    (&[key!(ctrl 'd')], |rows| Message::PageDown { rows }),
    (&[key!(PageDown)], |rows| Message::PageDown { rows }),
    (&[key!(ctrl 'u')], |rows| Message::PageUp { rows }),
    (&[key!(PageUp)], |rows| Message::PageUp { rows }),
];

/// Turns key presses into messages, holding the keys of a multi-key sequence until it completes,
/// fails to match or times out.
#[derive(Debug, Default)]
pub struct Keymap {
    pending: Vec<Key>,
    /// When the pending sequence started, in the frontend's time.
    started: Duration,
}

impl Keymap {
    /// The message `event` completes at time `now`, if any. `page_rows` is the height a page
    /// motion moves. Only key presses count; repeats, releases and other events are ignored. A
    /// key that continues no binding ends the pending sequence and does nothing.
    pub fn message_for(
        &mut self,
        event: &Event,
        now: Duration,
        page_rows: usize,
    ) -> Option<Message> {
        let key = event.as_key_press_event()?;
        self.expire(now);
        if self.pending.is_empty() {
            self.started = now;
        }
        let modifiers = match key.code {
            KeyCode::Char(_) => key.modifiers.difference(KeyModifiers::SHIFT),
            _ => key.modifiers,
        };
        self.pending.push((modifiers, key.code));
        if let Some((_, message)) = NAVIGATE.iter().find(|(keys, _)| *keys == self.pending) {
            self.pending.clear();
            return Some(message(page_rows));
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

    fn key(code: KeyCode, modifiers: KeyModifiers) -> Event {
        Event::Key(KeyEvent::new(code, modifiers))
    }

    fn ms(millis: u64) -> Duration {
        Duration::from_millis(millis)
    }

    /// The message each key sends from a fresh keymap.
    #[track_caller]
    fn check(code: KeyCode, modifiers: KeyModifiers, expected: Option<Message>) {
        let mut keymap = Keymap::default();
        assert_eq!(
            keymap.message_for(&key(code, modifiers), ms(0), PAGE),
            expected,
            "{modifiers:?} {code:?}"
        );
    }

    #[test]
    fn ctrl_q_quits() {
        check(
            KeyCode::Char('q'),
            KeyModifiers::CONTROL,
            Some(Message::Quit),
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
        let next = Some(Message::FocusNext);
        let previous = Some(Message::FocusPrevious);
        let last = Some(Message::FocusLast);
        check(KeyCode::Char('j'), none, next);
        check(KeyCode::Down, none, next);
        check(KeyCode::Char('k'), none, previous);
        check(KeyCode::Up, none, previous);
        check(KeyCode::Home, none, Some(Message::FocusFirst));
        check(KeyCode::End, none, last);
        check(KeyCode::Char('G'), KeyModifiers::SHIFT, last);
        check(KeyCode::Char('G'), none, last);
    }

    // FR-NAV-01
    #[test]
    fn page_keys_carry_the_page_height() {
        let down = Some(Message::PageDown { rows: PAGE });
        let up = Some(Message::PageUp { rows: PAGE });
        check(KeyCode::PageDown, KeyModifiers::NONE, down);
        check(KeyCode::Char('d'), KeyModifiers::CONTROL, down);
        check(KeyCode::PageUp, KeyModifiers::NONE, up);
        check(KeyCode::Char('u'), KeyModifiers::CONTROL, up);
    }

    #[test]
    fn shift_only_drops_from_characters() {
        check(KeyCode::Down, KeyModifiers::SHIFT, None);
        check(KeyCode::Char('j'), KeyModifiers::ALT, None);
    }

    #[test]
    fn g_g_within_the_timeout_focuses_the_first_row() {
        let mut keymap = Keymap::default();
        let g = key(KeyCode::Char('g'), KeyModifiers::NONE);
        assert_eq!(keymap.message_for(&g, ms(5_000), PAGE), None);
        assert_eq!(keymap.pending(), "g");
        assert_eq!(keymap.deadline(), Some(ms(6_000)));
        assert_eq!(
            keymap.message_for(&g, ms(5_999), PAGE),
            Some(Message::FocusFirst)
        );
        assert_eq!(keymap.pending(), "");
        assert_eq!(keymap.deadline(), None);
    }

    #[test]
    fn a_sequence_times_out_after_one_second() {
        let mut keymap = Keymap::default();
        let g = key(KeyCode::Char('g'), KeyModifiers::NONE);
        keymap.message_for(&g, ms(0), PAGE);
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
        keymap.message_for(&g, ms(0), PAGE);
        assert_eq!(keymap.message_for(&g, ms(1_000), PAGE), None);
        assert_eq!(keymap.pending(), "g");
        assert_eq!(keymap.deadline(), Some(ms(2_000)));
    }

    #[test]
    fn a_key_that_continues_no_sequence_cancels_it_and_does_nothing() {
        let mut keymap = Keymap::default();
        keymap.message_for(&key(KeyCode::Char('g'), KeyModifiers::NONE), ms(0), PAGE);
        assert_eq!(
            keymap.message_for(&key(KeyCode::Char('j'), KeyModifiers::NONE), ms(10), PAGE),
            None
        );
        assert_eq!(keymap.pending(), "");
        assert_eq!(
            keymap.message_for(&key(KeyCode::Char('j'), KeyModifiers::NONE), ms(20), PAGE),
            Some(Message::FocusNext)
        );
    }

    #[test]
    fn unbound_keys_leave_nothing_pending() {
        let mut keymap = Keymap::default();
        keymap.message_for(&key(KeyCode::Char('w'), KeyModifiers::NONE), ms(0), PAGE);
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
                keymap.message_for(&event(KeyCode::Char('j')), ms(0), PAGE),
                None
            );
            assert_eq!(
                keymap.message_for(&event(KeyCode::Char('g')), ms(0), PAGE),
                None
            );
            assert_eq!(keymap.pending(), "");
        }
    }

    #[test]
    fn non_key_events_are_ignored_and_keep_the_pending_sequence() {
        let mut keymap = Keymap::default();
        assert_eq!(
            keymap.message_for(&Event::Resize(80, 24), ms(0), PAGE),
            None
        );
        assert_eq!(keymap.message_for(&Event::FocusGained, ms(0), PAGE), None);
        keymap.message_for(&key(KeyCode::Char('g'), KeyModifiers::NONE), ms(0), PAGE);
        keymap.message_for(&Event::Resize(80, 24), ms(10), PAGE);
        assert_eq!(keymap.pending(), "g");
    }
}
