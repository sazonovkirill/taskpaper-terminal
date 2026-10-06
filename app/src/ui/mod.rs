//! The terminal frontend: turns crossterm events into app messages, carries out effects and
//! draws the state with Ratatui.

pub mod effects;
pub mod keymap;
pub mod layout;
mod theme;
pub mod widgets;

use std::{path::PathBuf, time::Duration};

use crossterm::event::{Event, KeyModifiers};
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
use keymap::{Context, KeyAction, Keymap, Overlay, PromptKey};
use widgets::{
    ConflictPrompt, DatePromptPanel, FinderPopup, Keyboard, OutlineView, Popup, ShortcutsPanel,
};

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
    /// Whether text too long for its line wraps, or ends in `…` (FR-UI-05).
    wrap: bool,
    /// The first outline line on screen.
    scroll: usize,
    /// The outline's height at the last draw, which a page motion moves.
    page_rows: usize,
    /// The outline line the focused block started on at the last draw, counted from the top of
    /// the screen.
    focus_line: Option<usize>,
    /// Where the next draw puts the focused block's first line, counted from the top of the
    /// screen: where it was before a page motion.
    page_anchor: Option<usize>,
    /// Whether the process was told to end, so only saving is left.
    ending: bool,
    /// The first line of the conflict prompt's diff on screen.
    diff_scroll: usize,
    /// How many diff lines the prompt showed at the last draw, which a page scrolls.
    diff_rows: usize,
    /// What the terminal has shown of its keyboard, which the shortcuts panel notes.
    keyboard: Keyboard,
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
            wrap: true,
            scroll: 0,
            page_rows: 0,
            focus_line: None,
            page_anchor: None,
            ending: false,
            diff_scroll: 0,
            diff_rows: 0,
            keyboard: Keyboard::default(),
            quit: false,
        };
        tui.send(Message::Started, Duration::ZERO);
        tui
    }

    /// The frontend told whether the terminal speaks the kitty keyboard protocol, which the
    /// shortcuts panel notes (keybindings.md §1).
    #[must_use]
    pub fn with_kitty_keyboard(mut self, kitty: bool) -> Self {
        self.keyboard.kitty = kitty;
        self
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
        if let Some(key) = event.as_key_press_event() {
            self.keyboard.alt_seen |= key.modifiers.contains(KeyModifiers::ALT);
        }
        if self.state.in_conflict() {
            self.handle_prompt_key(event, now);
            return;
        }
        let Some(action) = self.keymap.action_for(event, now, &self.context()) else {
            return;
        };
        let replay = self.keymap.replay();
        if let KeyAction::Send(Message::PageDown { .. } | Message::PageUp { .. }) = action {
            // A page motion scrolls the view with the focus, so the focused block keeps its
            // place on screen instead of only being dragged into view.
            self.page_anchor = self.focus_line;
        }
        self.act(action, now);
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
            shortcuts: self.state.shortcuts().is_some(),
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
        if let Some(action) = self.keymap.expire(now, &self.context()) {
            self.act(action, now);
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

    /// Does what a key asks for at time `now`.
    fn act(&mut self, action: KeyAction, now: Duration) {
        match action {
            KeyAction::Send(message) => self.send(message, now),
            KeyAction::ToggleWrap => self.wrap = !self.wrap,
        }
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
    /// visible, the shortcuts panel at the right while open, the date prompt's panel below them
    /// while open, and the status line.
    pub fn draw(&mut self, frame: &mut Frame) {
        let screen = frame.area();
        let [main, status] = screen.layout(&Layout::vertical([
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
        let [body, panel] = main.layout(&Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(panel_height),
        ]));
        let (pane, sidebar) = match self.state.shortcuts() {
            Some(_) => layout::secondary_sidebar(screen, body),
            None => (body, None),
        };
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
        self.draw_outline(frame, outline);
        // Centered popups go over the sidebar, which a narrow screen draws over the outline.
        self.draw_shortcuts(frame, sidebar);
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
        if let Some(view) = self
            .state
            .go_to_symbol()
            .filter(|_| !self.state.in_conflict())
        {
            let finder = FinderPopup::new("Go to Symbol", &view);
            frame.set_cursor_position(finder.caret(outline));
            frame.render_widget(finder, outline);
        }
        self.draw_status(frame, status);
        self.draw_date_prompt(frame, panel);
        if self.state.in_conflict() {
            self.draw_conflict(frame, main);
        }
    }

    /// The conflict prompt over `area`, its diff scrolled no further than its last page.
    fn draw_conflict(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let diff = self.state.diff();
        self.diff_rows = ConflictPrompt::diff_rows(area);
        let lines = diff.as_ref().map_or(0, Vec::len);
        self.diff_scroll = self.diff_scroll.min(lines.saturating_sub(self.diff_rows));
        frame.render_widget(ConflictPrompt::new(diff.as_deref(), self.diff_scroll), area);
    }

    /// The shortcuts panel in the secondary sidebar's `area`, while it is open and has room
    /// (FR-KEY-04, D-28).
    fn draw_shortcuts(&self, frame: &mut Frame, area: Option<ratatui::layout::Rect>) {
        if let (Some(groups), Some(area)) = (self.state.shortcuts(), area) {
            frame.render_widget(ShortcutsPanel::new(&groups, self.keyboard), area);
        }
    }

    /// The outline in `area`, scrolled to keep the focused block in view, and while editing the
    /// caret with the command menu at it, unless the conflict prompt covers them.
    fn draw_outline(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let rows = self.state.rows();
        let focused = self.focused_index();
        let editor = self.state.editor();
        let view = OutlineView::new(&rows, (self.clock)().date())
            .focused(focused)
            .editor(editor.as_ref())
            .wrap(self.wrap);
        // The first screen line of each row, and the line after the last.
        let starts: Vec<usize> = std::iter::once(0)
            .chain(
                view.heights(area.width)
                    .into_iter()
                    .scan(0, |line, height| {
                        *line += height;
                        Some(*line)
                    }),
            )
            .collect();
        let start = |index: usize| starts.get(index).copied().unwrap_or_default();
        let focus = focused.map_or(0..1, |index| start(index)..start(index + 1));
        if let Some(line) = self.page_anchor.take() {
            self.scroll = focus.start.saturating_sub(line);
        }
        self.page_rows = usize::from(area.height);
        let lines = starts.last().copied().unwrap_or_default();
        self.scroll = layout::scroll_offset(self.scroll, focus.clone(), self.page_rows, lines);
        self.focus_line = focused.map(|_| focus.start.saturating_sub(self.scroll));
        frame.render_widget(view.offset(self.scroll), area);
        if let Some(editor) = editor.as_ref().filter(|_| !self.state.in_conflict())
            && let Some(row) = focused.and_then(|index| rows.get(index))
        {
            let text_width = area.width.saturating_sub(widgets::GUTTER_WIDTH);
            let (_, column) = widgets::editor_line(row, editor, usize::from(text_width));
            let x = u16::try_from(column)
                .unwrap_or(u16::MAX)
                .min(text_width.saturating_sub(1));
            let y = u16::try_from(focus.start.saturating_sub(self.scroll)).unwrap_or(u16::MAX);
            let caret = (
                area.x.saturating_add(widgets::GUTTER_WIDTH + x),
                area.y.saturating_add(y),
            );
            frame.set_cursor_position(caret);
            if let Some(menu) = self.state.command_menu() {
                let popup = Popup::commands(&menu);
                let (width, height) = popup.size();
                let height = height.min(layout::room(area, caret.1));
                let anchored = layout::anchored(area, caret, (width, height));
                frame.render_widget(popup, anchored);
            }
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
        } else if self.state.go_to_symbol().is_some() {
            Some(Overlay::GoToSymbol)
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
