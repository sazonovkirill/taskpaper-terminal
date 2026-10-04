//! The terminal frontend: turns crossterm events into app messages, carries out effects and
//! draws the state with Ratatui.

pub mod effects;
pub mod keymap;
pub mod layout;
pub mod widgets;

use std::{ops::Range, time::Duration};

use crossterm::event::Event;
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::Stylize,
    text::Line,
};

use crate::app::{Effect, Message, State};
use keymap::Keymap;
use widgets::OutlineView;

/// The app as the terminal sees it: the core state plus what the frontend tracks itself.
#[derive(Debug)]
pub struct Tui {
    state: State,
    keymap: Keymap,
    /// The first outline row on screen.
    scroll: usize,
    /// The outline's height at the last draw, which a page motion moves.
    page_rows: usize,
    quit: bool,
}

impl Tui {
    /// A frontend showing `state`.
    #[must_use]
    pub fn new(state: State) -> Self {
        Self {
            state,
            keymap: Keymap::default(),
            scroll: 0,
            page_rows: 0,
            quit: false,
        }
    }

    /// Handles one terminal event at time `now` (since the frontend started): maps it to a
    /// message, updates the state and runs the effects.
    pub fn handle_event(&mut self, event: &Event, now: Duration) {
        let Some(message) = self.keymap.message_for(event, now, self.page_rows) else {
            return;
        };
        let before = self.focused_lines().map(|lines| lines.start);
        for effect in self.state.update(message) {
            match effect {
                Effect::Quit => self.quit = true,
            }
        }
        // A page motion scrolls the view with the focus, so the focused row keeps its place on
        // screen instead of only being dragged into view.
        if let (Message::PageDown { .. } | Message::PageUp { .. }, Some(before), Some(after)) = (
            message,
            before,
            self.focused_lines().map(|lines| lines.start),
        ) {
            self.scroll = (self.scroll + after).saturating_sub(before);
        }
    }

    /// Lets time pass to `now` without an event, ending a key sequence that timed out.
    pub fn tick(&mut self, now: Duration) {
        self.keymap.expire(now);
    }

    /// When [`Tui::tick`] next has work to do, or `None` when only an event can change anything.
    #[must_use]
    pub fn deadline(&self) -> Option<Duration> {
        self.keymap.deadline()
    }

    /// Draws the whole screen: the breadcrumb header over a rule, the outline scrolled to keep the focus
    /// visible, and the status line.
    pub fn draw(&mut self, frame: &mut Frame) {
        let [pane, status] = frame.area().layout(&Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
        ]));
        let [header, separator, outline] = pane.layout(&Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ]));
        frame.render_widget(
            widgets::breadcrumbs(&self.state.breadcrumbs(), usize::from(header.width)),
            header,
        );
        frame.render_widget(
            Line::styled(
                "─".repeat(usize::from(separator.width)),
                widgets::rule_style(),
            ),
            separator,
        );
        let rows = self.state.rows();
        let focused = self.focused_index();
        self.page_rows = usize::from(outline.height);
        self.scroll = layout::scroll_offset(
            self.scroll,
            self.focused_lines().unwrap_or_default(),
            self.page_rows,
            rows.iter().map(|row| row.lines.len()).sum(),
        );
        frame.render_widget(
            OutlineView::new(&rows).offset(self.scroll).focused(focused),
            outline,
        );
        self.draw_status(frame, status);
    }

    /// The mode, the file and any pending key sequence ([ui.md §1](../../../docs/specs/ui.md#1-layout)).
    fn draw_status(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let file = self.state.path().map_or_else(
            || "demo, in memory only".to_owned(),
            |path| path.display().to_string(),
        );
        frame.render_widget(
            Line::from(vec!["NAVIGATE".bold(), format!(" · {file}").into()]),
            area,
        );
        // The pending keys are what the user is typing right now, so they cover the end of a
        // long path rather than the other way round.
        let pending = self.keymap.pending();
        if !pending.is_empty() {
            frame.render_widget(Line::from(format!(" {pending}")).right_aligned(), area);
        }
    }

    /// The screen lines of the outline that the focused block takes.
    fn focused_lines(&self) -> Option<Range<usize>> {
        let rows = self.state.rows();
        let index = self.focused_index()?;
        let start = rows.iter().take(index).map(|row| row.lines.len()).sum();
        let height = rows.get(index)?.lines.len();
        Some(start..start + height)
    }

    /// The position of the focused row among the outline's rows.
    fn focused_index(&self) -> Option<usize> {
        let focus = self.state.focus()?;
        self.state.rows().iter().position(|row| row.id == focus)
    }

    /// Whether the user asked to quit.
    #[must_use]
    pub fn should_quit(&self) -> bool {
        self.quit
    }
}
