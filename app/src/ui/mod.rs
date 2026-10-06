//! The terminal frontend: turns crossterm events into app messages, carries out effects and
//! draws the state with Ratatui.

pub mod effects;
pub mod keymap;
pub mod layout;
mod theme;
pub mod widgets;

use std::{path::PathBuf, time::Duration};

use crossterm::event::Event;
use jiff::Zoned;
use ratatui::{
    Frame, Terminal,
    backend::Backend,
    layout::{Constraint, Layout},
    style::{Style, Stylize},
    text::Line,
};

use crate::app::{Effect, Message, Mode, Origin, State};
use effects::{Browser, Clipboard, Io, Recovery, RecoveryError};
use keymap::{Context, Keymap, Overlay, PromptKey};
use widgets::{ConflictPrompt, DatePromptPanel, OutlineView, Popup};

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
    /// Shows URLs the core asks to open.
    browser: Browser,
    /// Holds what the core copies and gives it back to paste.
    clipboard: Clipboard,
    /// Reads the local time, whose date messages that write one carry and chips are drawn on.
    clock: fn() -> Zoned,
    /// When the local date next changes while the outline shows a date chip, whose relative date
    /// then changes and which may become overdue (FR-TAG-03, D-46).
    midnight: Option<Duration>,
    keymap: Keymap,
    /// The first outline row on screen.
    scroll: usize,
    /// The outline's height at the last draw, which a page motion moves.
    page_rows: usize,
    /// Whether the process was told to end, so only saving is left.
    ending: bool,
    /// The first line of the conflict prompt's diff on screen.
    diff_scroll: usize,
    /// How many diff lines the prompt showed at the last draw, which a page scrolls.
    diff_rows: usize,
    quit: bool,
}

impl Tui {
    /// A frontend showing `state` that carries out disk work with `io`, opens URLs with
    /// `browser`, copies and pastes with `clipboard` and reads the local time from `clock`,
    /// starting with what the core asks for on opening.
    #[must_use]
    pub fn new(
        state: State,
        io: Io,
        browser: Browser,
        clipboard: Clipboard,
        clock: fn() -> Zoned,
    ) -> Self {
        let mut tui = Self {
            state,
            io,
            browser,
            clipboard,
            clock,
            midnight: None,
            keymap: Keymap::default(),
            scroll: 0,
            page_rows: 0,
            ending: false,
            diff_scroll: 0,
            diff_rows: 0,
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
        // Coming back to the terminal is when changes made elsewhere are likeliest (file-io.md §3).
        if *event == Event::FocusGained {
            self.send(Message::CheckFile, now);
            return;
        }
        if self.state.in_conflict() {
            self.handle_prompt_key(event, now);
            return;
        }
        let Some(message) = self.keymap.message_for(event, now, &self.context()) else {
            return;
        };
        let replay = self.keymap.replay();
        let paging = matches!(message, Message::PageDown { .. } | Message::PageUp { .. });
        let before = self.focused_index();
        self.send(message, now);
        // A page motion scrolls the view with the focus, so the focused row keeps its place on
        // screen instead of only being dragged into view.
        if paging && let (Some(before), Some(after)) = (before, self.focused_index()) {
            self.scroll = (self.scroll + after).saturating_sub(before);
        }
        // The key ended a sequence that sent its own message, such as a lone `o`, and now acts
        // where that message left the app, so typing right after `o` goes into the new block.
        if replay {
            self.handle_event(event, now);
        }
    }

    /// What the keymap needs to know to turn a key into a message.
    fn context(&self) -> Context {
        Context {
            page_rows: self.page_rows,
            today: (self.clock)().date(),
            mode: self.state.mode(),
            command_menu: self.state.command_menu().is_some(),
            overlay: self.overlay(),
        }
    }

    /// Handles a key of the conflict prompt at time `now`.
    fn handle_prompt_key(&mut self, event: &Event, now: Duration) {
        match keymap::prompt_key(event, self.diff_rows) {
            Some(PromptKey::Send(message)) => {
                if message == Message::ToggleDiff {
                    self.diff_scroll = 0;
                }
                self.send(message, now);
            }
            Some(PromptKey::Scroll(lines)) => {
                self.diff_scroll = self.diff_scroll.saturating_add_signed(lines);
            }
            None => {}
        }
    }

    /// Lets time pass to `now` without an event: ends a key sequence that timed out, sending its
    /// message when it is a binding of its own, such as a lone `o`, and lets the core start a save
    /// that is due.
    pub fn tick(&mut self, now: Duration) {
        if let Some(message) = self.keymap.expire(now, &self.context()) {
            self.send(message, now);
        }
        let effects = self.state.update(Message::Tick { now });
        self.run(effects);
        self.midnight = self.next_midnight(now);
    }

    /// When [`Tui::tick`] next has work to do or the screen changes by itself, or `None` when
    /// only an input can change anything.
    #[must_use]
    pub fn deadline(&self) -> Option<Duration> {
        self.keymap
            .deadline()
            .into_iter()
            .chain(self.state.deadline())
            .chain(self.midnight)
            .min()
    }

    /// When the local date changes, counted like `now`, if the outline shows a date chip, which the
    /// new date names differently.
    fn next_midnight(&self, now: Duration) -> Option<Duration> {
        if !self
            .state
            .rows()
            .iter()
            .any(|row| row.start.is_some() || row.due.is_some())
        {
            return None;
        }
        let local = (self.clock)();
        let tomorrow = local.tomorrow().and_then(|day| day.start_of_day()).ok()?;
        let wait = Duration::try_from(local.duration_until(&tomorrow)).ok()?;
        now.checked_add(wait)
    }

    /// Writes the document to a recovery file with `recovery` when its file lacks some of it, as
    /// after a crash, and returns where (N-REL-01). Disk work asked for so far is done first, so
    /// that no save under way changes the file after it and stops the recovery file from being
    /// offered (file-io.md §4).
    ///
    /// # Errors
    ///
    /// [`RecoveryError`] when the recovery file could not be written.
    pub fn recover(&self, recovery: &Recovery) -> Result<Option<PathBuf>, RecoveryError> {
        self.io.flush();
        self.state
            .unsaved_text()
            .map(|text| recovery.write(self.state.path(), &text))
            .transpose()
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
        self.midnight = self.next_midnight(now);
    }

    fn run(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Quit => self.quit = true,
                Effect::Disk(disk) => self.io.submit(disk),
                Effect::CopyToClipboard(text) => self.clipboard.copy(&text),
                Effect::ReadClipboard(at) => {
                    if let Some(text) = self.clipboard.paste() {
                        let effects = self.state.update(Message::Pasted { text, at });
                        self.run(effects);
                    }
                }
                Effect::OpenUrl(url) => {
                    if let Err(error) = self.browser.open(&url) {
                        let reason = error.to_string();
                        let effects = self.state.update(Message::OpenUrlFailed { url, reason });
                        self.run(effects);
                    }
                }
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
    /// visible, the date prompt's panel below it while open, and the status line.
    pub fn draw(&mut self, frame: &mut Frame) {
        let [pane, status] = frame.area().layout(&Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
        ]));
        // The outline gives up the panel's rows rather than lying under it, so the focus scrolls
        // into view above it (D-54).
        let panel_height = if self.state.date_prompt().is_some() {
            DatePromptPanel::HEIGHT
        } else {
            0
        };
        let [header, separator, outline, panel] = pane.layout(&Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(panel_height),
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
            focused.unwrap_or_default(),
            self.page_rows,
            rows.len(),
        );
        let editor = self.state.editor();
        frame.render_widget(
            OutlineView::new(&rows, (self.clock)().date())
                .offset(self.scroll)
                .focused(focused)
                .editor(editor.as_ref()),
            outline,
        );
        // The conflict prompt covers the outline, caret and command menu included.
        if let Some(editor) = editor.as_ref().filter(|_| !self.state.in_conflict())
            && let Some(index) = focused
            && let Some(row) = rows.get(index)
        {
            let text_width = outline.width.saturating_sub(widgets::GUTTER_WIDTH);
            let (_, column) = widgets::editor_line(row, editor, usize::from(text_width));
            let x = u16::try_from(column)
                .unwrap_or(u16::MAX)
                .min(text_width.saturating_sub(1));
            let y = u16::try_from(index.saturating_sub(self.scroll)).unwrap_or(u16::MAX);
            let caret = (
                outline.x.saturating_add(widgets::GUTTER_WIDTH + x),
                outline.y.saturating_add(y),
            );
            frame.set_cursor_position(caret);
            if let Some(menu) = self.state.command_menu() {
                let popup = Popup::commands(&menu);
                let (width, height) = popup.size();
                let height = height.min(layout::room(outline, caret.1));
                let area = layout::anchored(outline, caret, (width, height));
                frame.render_widget(popup, area);
            }
        }
        let centered = self
            .state
            .color_picker()
            .map(|picker| Popup::colors(&picker))
            .or_else(|| {
                self.state
                    .type_picker()
                    .map(|picker| Popup::block_types(&picker))
            })
            .or_else(|| self.state.details().map(|details| Popup::details(&details)));
        if let Some(popup) = centered {
            let (width, height) = popup.size();
            let area = outline.centered(Constraint::Length(width), Constraint::Length(height));
            frame.render_widget(popup, area);
        }
        self.draw_status(frame, status);
        self.draw_date_prompt(frame, panel);
        if self.state.in_conflict() {
            let diff = self.state.diff();
            self.diff_rows = ConflictPrompt::diff_rows(pane);
            let lines = diff.as_ref().map_or(0, Vec::len);
            self.diff_scroll = self.diff_scroll.min(lines.saturating_sub(self.diff_rows));
            frame.render_widget(ConflictPrompt::new(diff.as_deref(), self.diff_scroll), pane);
        }
    }

    /// The date prompt's panel over `area` with its caret, if it is open, and the rest of the
    /// screen dimmed to set it apart (D-54). The conflict prompt covers both.
    fn draw_date_prompt(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        if let Some(prompt) = self.state.date_prompt()
            && !self.state.in_conflict()
        {
            let screen = frame.area();
            frame.buffer_mut().set_style(screen, Style::new().dim());
            let panel = DatePromptPanel::new(&prompt);
            frame.set_cursor_position(panel.caret(area));
            frame.render_widget(panel, area);
        }
    }

    /// The mode, the file, marked when temporary, with its save state and notice, and any pending
    /// key sequence ([ui.md §1](../../../docs/specs/ui.md#1-layout)).
    fn draw_status(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let mode = match self.state.mode() {
            Mode::Navigate => "NAVIGATE",
            Mode::Edit => "EDIT",
        };
        let mut spans = vec![mode.bold()];
        if let Some(path) = self.state.path() {
            spans.push(format!(" · {}", path.display()).into());
        }
        if self.state.origin() == Origin::Temporary {
            spans.push(" · temporary".into());
        }
        spans.extend(widgets::file_status(
            self.state.save_state(),
            self.state.access(),
            self.state.gone_from_disk(),
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

    /// The popup over the outline that takes the keys of navigate mode, if one is open.
    fn overlay(&self) -> Option<Overlay> {
        if self.state.date_prompt().is_some() {
            Some(Overlay::DatePrompt)
        } else if self.state.color_picker().is_some() {
            Some(Overlay::ColorPicker)
        } else if self.state.type_picker().is_some() {
            Some(Overlay::TypePicker)
        } else {
            self.state.details().map(|_| Overlay::Details)
        }
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
