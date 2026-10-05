//! The terminal frontend: turns crossterm events into app messages, carries out effects and
//! draws the state with Ratatui.

pub mod effects;
pub mod keymap;
pub mod layout;
pub mod widgets;

use std::{ops::Range, time::Duration};

use crossterm::event::Event;
use ratatui::{
    Frame, Terminal,
    backend::Backend,
    layout::{Constraint, Layout},
    style::Stylize,
    text::Line,
};

use crate::app::{Effect, Message, Origin, State};
use effects::Io;
use keymap::Keymap;
use widgets::OutlineView;

/// What the event loop waits for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// A terminal event.
    Terminal(Event),
    /// A message for the core from outside the terminal: a signal or the result of an effect.
    Core(Message),
}

/// The app as the terminal sees it: the core state plus what the frontend tracks itself.
#[derive(Debug)]
pub struct Tui {
    state: State,
    io: Io,
    keymap: Keymap,
    /// The first outline row on screen.
    scroll: usize,
    /// The outline's height at the last draw, which a page motion moves.
    page_rows: usize,
    /// Whether the process was told to end, so only saving is left.
    ending: bool,
    quit: bool,
}

impl Tui {
    /// A frontend showing `state` that carries out disk work with `io`, starting with what the
    /// core asks for on opening.
    #[must_use]
    pub fn new(state: State, io: Io) -> Self {
        let mut tui = Self {
            state,
            io,
            keymap: Keymap::default(),
            scroll: 0,
            page_rows: 0,
            ending: false,
            quit: false,
        };
        tui.send(Message::Started, Duration::ZERO);
        tui
    }

    /// Handles one input at time `now` (since the frontend started).
    pub fn handle(&mut self, input: Input, now: Duration) {
        match input {
            Input::Terminal(event) => self.handle_event(&event, now),
            Input::Core(message) => self.send(message, now),
        }
    }

    /// Handles one terminal event at time `now` (since the frontend started): maps it to a
    /// message, updates the state and runs the effects.
    fn handle_event(&mut self, event: &Event, now: Duration) {
        let Some(message) = self.keymap.message_for(event, now, self.page_rows) else {
            return;
        };
        let paging = matches!(message, Message::PageDown { .. } | Message::PageUp { .. });
        let before = self.focused_lines().map(|lines| lines.start);
        self.send(message, now);
        // A page motion scrolls the view with the focus, so the focused row keeps its place on
        // screen instead of only being dragged into view.
        if paging
            && let (Some(before), Some(after)) =
                (before, self.focused_lines().map(|lines| lines.start))
        {
            self.scroll = (self.scroll + after).saturating_sub(before);
        }
    }

    /// Lets time pass to `now` without an event: ends a key sequence that timed out and lets the
    /// core start a save that is due.
    pub fn tick(&mut self, now: Duration) {
        self.keymap.expire(now);
        let effects = self.state.update(Message::Tick { now });
        self.run(effects);
    }

    /// When [`Tui::tick`] next has work to do, or `None` when only an input can change anything.
    #[must_use]
    pub fn deadline(&self) -> Option<Duration> {
        self.keymap
            .deadline()
            .into_iter()
            .chain(self.state.deadline())
            .min()
    }

    /// Waits until the disk work asked for so far is done; its results are inputs to handle.
    pub fn flush(&self) {
        self.io.flush();
    }

    /// Tells the core the time, then sends it `message`, and carries out the effects.
    fn send(&mut self, message: Message, now: Duration) {
        if message == Message::Terminate {
            self.ending = true;
        }
        let mut effects = self.state.update(Message::Tick { now });
        effects.extend(self.state.update(message));
        self.run(effects);
    }

    fn run(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Quit => self.quit = true,
                Effect::Disk(disk) => self.io.submit(disk),
            }
        }
    }

    /// Draws the screen on `terminal` at time `now`, unless the session is ending, when the
    /// terminal may be gone. A terminal that cannot be drawn on is gone, as after `SIGHUP`, so
    /// the session ends as on [`Message::Terminate`], saving first.
    pub fn render<B: Backend>(&mut self, terminal: &mut Terminal<B>, now: Duration) {
        if !self.ending && terminal.draw(|frame| self.draw(frame)).is_err() {
            self.send(Message::Terminate, now);
        }
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
        let [_, crumbs] = header.layout(&Layout::horizontal([
            Constraint::Length(widgets::GUTTER_WIDTH),
            Constraint::Fill(1),
        ]));
        frame.render_widget(
            widgets::breadcrumbs(&self.state.breadcrumbs(), usize::from(crumbs.width)),
            crumbs,
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

    /// The mode, the file, marked when temporary, with its save state and notice, and any pending
    /// key sequence ([ui.md §1](../../../docs/specs/ui.md#1-layout)).
    fn draw_status(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let mut spans = vec!["NAVIGATE".bold()];
        if let Some(path) = self.state.path() {
            spans.push(format!(" · {}", path.display()).into());
        }
        if self.state.origin() == Origin::Temporary {
            spans.push(" · temporary".into());
        }
        spans.extend(widgets::file_status(
            self.state.save_state(),
            self.state.access(),
            self.state.confirming_quit(),
            self.state.notice(),
        ));
        frame.render_widget(Line::from(spans), area);
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
