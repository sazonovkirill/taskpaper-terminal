//! Key events to app messages ([keybindings.md](../../../docs/specs/keybindings.md)).

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::app::Message;

/// The message a terminal event asks for, if any. Only key presses count; repeats and releases
/// are ignored.
#[must_use]
pub fn message_for(event: &Event) -> Option<Message> {
    navigate(event.as_key_press_event()?)
}

fn navigate(key: KeyEvent) -> Option<Message> {
    match (key.modifiers, key.code) {
        (KeyModifiers::CONTROL, KeyCode::Char('q')) => Some(Message::Quit),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyEventKind, KeyEventState};

    use super::*;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> Event {
        Event::Key(KeyEvent::new(code, modifiers))
    }

    #[test]
    fn ctrl_q_quits() {
        assert_eq!(
            message_for(&key(KeyCode::Char('q'), KeyModifiers::CONTROL)),
            Some(Message::Quit)
        );
    }

    #[test]
    fn q_without_ctrl_or_with_more_modifiers_does_nothing() {
        assert_eq!(
            message_for(&key(KeyCode::Char('q'), KeyModifiers::NONE)),
            None
        );
        assert_eq!(
            message_for(&key(
                KeyCode::Char('q'),
                KeyModifiers::CONTROL | KeyModifiers::ALT
            )),
            None
        );
        assert_eq!(
            message_for(&key(KeyCode::Char('w'), KeyModifiers::CONTROL)),
            None
        );
    }

    #[test]
    fn release_and_repeat_events_are_ignored() {
        for kind in [KeyEventKind::Release, KeyEventKind::Repeat] {
            let event = Event::Key(KeyEvent::new_with_kind_and_state(
                KeyCode::Char('q'),
                KeyModifiers::CONTROL,
                kind,
                KeyEventState::NONE,
            ));
            assert_eq!(message_for(&event), None);
        }
    }

    #[test]
    fn non_key_events_are_ignored() {
        assert_eq!(message_for(&Event::Resize(80, 24)), None);
        assert_eq!(message_for(&Event::FocusGained), None);
    }
}
