//! The terminal frontend: turns crossterm events into app messages, carries out effects and
//! draws the state with Ratatui.

pub mod effects;
pub mod keymap;
pub mod widgets;

use crossterm::event::Event;
use ratatui::Frame;

use crate::app::{Effect, State};
use widgets::OutlineView;

/// The app as the terminal sees it: the core state plus what the frontend tracks itself.
#[derive(Debug)]
pub struct Tui {
    state: State,
    quit: bool,
}

impl Tui {
    /// A frontend showing `state`.
    #[must_use]
    pub fn new(state: State) -> Self {
        Self { state, quit: false }
    }

    /// Handles one terminal event: maps it to a message, updates the state and runs the effects.
    pub fn handle_event(&mut self, event: &Event) {
        let Some(message) = keymap::message_for(event) else {
            return;
        };
        for effect in self.state.update(message) {
            match effect {
                Effect::Quit => self.quit = true,
            }
        }
    }

    /// Draws the whole screen.
    pub fn draw(&self, frame: &mut Frame) {
        let rows = self.state.rows();
        frame.render_widget(OutlineView::new(&rows), frame.area());
    }

    /// Whether the user asked to quit.
    #[must_use]
    pub fn should_quit(&self) -> bool {
        self.quit
    }
}
