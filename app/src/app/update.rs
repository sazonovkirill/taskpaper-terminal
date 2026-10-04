//! The update function: messages are user intents, effects are what the frontend must do.

use super::State;

/// A user intent, independent of how a frontend receives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    /// Leave the app.
    Quit,
}

/// Work the frontend performs for the core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// End the session.
    Quit,
}

impl State {
    /// Applies `message` and returns the effects the frontend must carry out.
    pub fn update(&mut self, message: Message) -> Vec<Effect> {
        match message {
            Message::Quit => vec![Effect::Quit],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quit_asks_the_frontend_to_quit() {
        assert_eq!(State::demo().update(Message::Quit), [Effect::Quit]);
    }
}
