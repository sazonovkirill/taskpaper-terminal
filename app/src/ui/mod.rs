//! The terminal frontend: turns crossterm events into app messages, carries out effects and
//! draws the state with Ratatui.

pub mod config;
pub mod effects;
pub mod keymap;
pub mod layout;
mod mouse;
pub mod theme;
pub mod widgets;

use std::{collections::HashMap, mem, path::PathBuf, time::Duration};

use crossterm::event::{Event, KeyModifiers};
use jiff::Zoned;
use ratatui::{
    Frame, Terminal,
    backend::Backend,
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Widget},
};

use crate::{
    app::{
        Disk, Effect, Location, Message, Mode, Origin, PaneId, PaneView, SaveAs, SidebarInput,
        State,
        commands::{Command, Group, REGISTRY},
    },
    document::ItemId,
};
use config::Config;
use effects::{Browser, Clipboard, FileWatch, Io, Recovery, RecoveryError};
use keymap::{Context, KeyAction, Keymap, Overlay, PromptKey};
use layout::Scroll;
pub use mouse::acts_on;
use theme::{Icon, IconSet, IconSetting};
use widgets::{
    ConflictPrompt, DatePromptPanel, FinderPopup, Keyboard, OutlineView, Popup, SaveAsPrompt,
    ShortcutsPanel, SidebarPanel,
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
    /// Watches the open file for changes made elsewhere.
    files: FileWatch,
    /// The home folder, which `~` stands for in paths typed (file-io.md §2).
    home: Option<PathBuf>,
    /// Shows URLs the core asks to open.
    browser: Browser,
    /// Holds what the core copies and gives it back to paste.
    clipboard: Clipboard,
    /// Reads the local time, whose date messages that write one carry and chips are drawn on.
    clock: fn() -> Zoned,
    /// When the local date next changes while the outline shows a date chip, whose relative date
    /// then changes and which may become overdue (FR-TAG-03, D-46), or the pane shows the Today
    /// view, which then lists the blocks for the new date (D-7).
    midnight: Option<Duration>,
    keymap: Keymap,
    /// Whether text too long for its line wraps, or ends in `…` (FR-UI-05).
    wrap: bool,
    /// The glyphs icons are drawn with: as the `icons` setting asks, or ASCII without a UTF-8
    /// locale (ui.md §3, §7).
    icons: IconSet,
    /// Where each pane's outline starts on screen: its top row stays put when rows above it
    /// change height.
    scrolls: HashMap<PaneId, Scroll>,
    /// How many columns of the strip of panes are scrolled past on its left (navigation.md §5).
    strip_scroll: u16,
    /// The focused pane's width at the last draw, which narrowing and widening change.
    pane_width: u16,
    /// The width of the strip of panes at the last draw, the most a pane is widened to.
    strip_width: u16,
    /// The focused pane's outline height at the last draw, which a page motion moves.
    page_rows: usize,
    /// The outline line the focused block started on at the last draw, counted from the top of
    /// the screen.
    focus_line: Option<usize>,
    /// Where the next draw puts the focused block's first line, counted from the top of the
    /// screen: where it was before a page motion.
    page_anchor: Option<usize>,
    /// The screen's width at the last draw, which decides whether the sidebar has room.
    width: u16,
    /// The first sidebar line on screen.
    sidebar_scroll: usize,
    /// Whether the process was told to end, so only saving is left.
    ending: bool,
    /// The first line of the conflict prompt's diff on screen.
    diff_scroll: usize,
    /// How many diff lines the prompt showed at the last draw, which a page scrolls.
    diff_rows: usize,
    /// What the terminal has shown of its keyboard, which the shortcuts panel notes.
    keyboard: Keyboard,
    /// The first line of the shortcuts panel's list on screen (D-55).
    shortcuts_scroll: usize,
    /// How many lines a page of the shortcuts panel was at the last draw, which its keys scroll.
    shortcuts_page: usize,
    /// Where the last draw put what the mouse acts on.
    drawn: mouse::Drawn,
    /// The panes the wheel scrolled, which stay where it left them rather than show their focus
    /// until a key or a click, with the lines it scrolled since the last draw (ui.md §6).
    wheel: HashMap<PaneId, isize>,
    /// Where a right-click opened the context menu, which shows there while it is open (ui.md §5).
    menu_anchor: Option<(u16, u16)>,
    quit: bool,
}

impl Tui {
    /// A frontend showing `state` that carries out disk work with `io`, watches the open file
    /// with `files`, opens URLs with `browser`, copies and pastes with `clipboard`, reads the
    /// local time from `clock` and takes `~` in paths typed for `home`, starting with what the
    /// core asks for on opening.
    #[must_use]
    pub fn new(
        state: State,
        io: Io,
        files: FileWatch,
        browser: Browser,
        clipboard: Clipboard,
        clock: fn() -> Zoned,
        home: Option<PathBuf>,
    ) -> Self {
        let mut tui = Self {
            state,
            io,
            files,
            home,
            browser,
            clipboard,
            clock,
            midnight: None,
            keymap: Keymap::default(),
            wrap: true,
            icons: IconSet::default(),
            scrolls: HashMap::new(),
            strip_scroll: 0,
            pane_width: 0,
            strip_width: 0,
            page_rows: 0,
            focus_line: None,
            page_anchor: None,
            width: 0,
            sidebar_scroll: 0,
            ending: false,
            diff_scroll: 0,
            diff_rows: 0,
            keyboard: Keyboard::default(),
            shortcuts_scroll: 0,
            shortcuts_page: 0,
            drawn: mouse::Drawn::default(),
            wheel: HashMap::new(),
            menu_anchor: None,
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

    /// The frontend on a terminal whose locale is UTF-8 when `utf8` says so, and otherwise
    /// drawing its icons in ASCII (ui.md §3, §7).
    #[must_use]
    pub fn with_locale(mut self, utf8: bool) -> Self {
        if !utf8 {
            self.icons = IconSet::Ascii;
        }
        self
    }

    /// The frontend drawing its icons as the `icons` setting `setting` asks, until a settings
    /// file says otherwise (ui.md §7).
    #[must_use]
    pub fn with_icons(mut self, setting: IconSetting) -> Self {
        self.icons = self.icons.with_setting(setting);
        self
    }

    /// The frontend running with the key bindings, autosave delay and icons `config` holds,
    /// reporting once what it ignored of the settings file (FR-SET-04). Its backups are the disk
    /// worker's.
    #[must_use]
    pub fn with_config(mut self, config: Config) -> Self {
        let problems: Vec<String> = config.problems().iter().map(ToString::to_string).collect();
        self.keymap = config.keymap;
        if let Some(setting) = config.icons {
            self.icons = self.icons.with_setting(setting);
        }
        if let Some(delay) = config.autosave_delay {
            self.send(Message::SetAutosaveDelay(delay), Duration::ZERO);
        }
        if !problems.is_empty() {
            self.send(Message::ConfigProblems(problems), Duration::ZERO);
        }
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
        // A sidebar the screen became too narrow for gives the keys back to the pane.
        if self.state.sidebar_focused() && layout::sidebar_width(self.width).is_none() {
            self.send(Message::Sidebar(SidebarInput::Leave), now);
        }
        if let Event::Mouse(mouse) = event {
            self.handle_mouse(*mouse, now);
            return;
        }
        if event.is_key_press() || matches!(event, Event::Paste(_)) {
            // A key acts where the panes show their focus again.
            self.wheel.clear();
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
            sidebar: self.state.sidebar_focused(),
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
        let effects = self.state.update(Message::Tick {
            now,
            today: (self.clock)().date(),
        });
        self.run(effects, now);
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

    /// When the local date changes, counted like `now`, if a pane shows a date chip, which the
    /// new date names differently, or the Today view, whose entries it changes.
    fn next_midnight(&self, now: Duration) -> Option<Duration> {
        let dated = self.state.panes().iter().any(|pane| {
            pane.location == Location::Today
                || pane
                    .rows
                    .iter()
                    .any(|row| row.start.is_some() || row.due.is_some())
        });
        if !dated {
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
            KeyAction::NarrowPane => {
                let width = layout::narrower(self.pane_width);
                self.send(Message::ResizePane(width), now);
            }
            KeyAction::WidenPane => {
                let width = layout::wider(self.pane_width, self.strip_width);
                self.send(Message::ResizePane(width), now);
            }
            KeyAction::ScrollShortcutsDown => {
                self.shortcuts_scroll = self.shortcuts_scroll.saturating_add(self.shortcuts_page);
            }
            KeyAction::ScrollShortcutsUp => {
                self.shortcuts_scroll = self.shortcuts_scroll.saturating_sub(self.shortcuts_page);
            }
            KeyAction::GoToCommand => {
                let bindings = REGISTRY
                    .iter()
                    .filter(|entry| entry.mode == Mode::Navigate)
                    .map(|entry| {
                        let keys = self.keymap.labels(entry.command, Mode::Navigate);
                        (entry.command, keys)
                    })
                    .collect();
                self.send(Message::OpenGoToCommand(bindings), now);
            }
        }
    }

    /// Tells the core the time, then sends it `message`, and carries out the effects.
    fn send(&mut self, message: Message, now: Duration) {
        if message == Message::Terminate {
            self.ending = true;
        }
        let mut effects = self.state.update(Message::Tick {
            now,
            today: (self.clock)().date(),
        });
        effects.extend(self.state.update(message));
        self.run(effects, now);
        self.midnight = self.next_midnight(now);
    }

    /// Carries out `effects` at time `now`.
    fn run(&mut self, effects: Vec<Effect>, now: Duration) {
        for effect in effects {
            match effect {
                Effect::Quit => {
                    self.keep_view();
                    self.quit = true;
                }
                Effect::Disk(disk) => {
                    let saving = matches!(disk, Disk::Save { .. });
                    self.io.submit(disk);
                    if saving {
                        self.keep_view();
                    }
                }
                Effect::CopyToClipboard(text) => self.clipboard.copy(&text),
                Effect::ReadClipboard(at) => {
                    if let Some(text) = self.clipboard.paste() {
                        let effects = self.state.update(Message::Pasted { text, at });
                        self.run(effects, now);
                    }
                }
                Effect::OpenUrl(url) => {
                    if let Err(error) = self.browser.open(&url) {
                        let reason = error.to_string();
                        let effects = self.state.update(Message::OpenUrlFailed { url, reason });
                        self.run(effects, now);
                    }
                }
                Effect::CompletePath(text) => {
                    if let Some(completed) = effects::complete_path(&text, self.home.as_deref()) {
                        self.send(Message::SaveAs(SaveAs::Completed(completed)), now);
                    }
                }
                Effect::ResolvePath(text) => {
                    let path = effects::expand_path(&text, self.home.as_deref());
                    self.send(Message::SaveAs(SaveAs::Resolved(path)), now);
                }
                Effect::Watch(path) => self.files.watch(&path),
                Effect::Run(command) => {
                    if let Some(action) = keymap::action(command, &self.context()) {
                        self.act(action, now);
                    }
                }
            }
        }
    }

    /// Keeps the layout of the open file, when it has one to keep, for its next launch: on every
    /// save and on quitting (file-io.md §5).
    fn keep_view(&self) {
        if let (Some(path), Some(view)) = (self.state.path(), self.state.view_state()) {
            self.io.store_view(path.to_owned(), view);
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

    /// Draws the whole screen: the sidebar, the panes side by side, each a breadcrumb header over
    /// a rule and its outline scrolled to keep its focus visible, the shortcuts panel at the right
    /// while open, the date prompt's panel below them while open, and the status line.
    pub fn draw(&mut self, frame: &mut Frame) {
        let screen = frame.area();
        self.width = screen.width;
        let icons = self.icons;
        let [main, status] = screen.layout(&Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
        ]));
        // The outline gives up the panel's rows rather than lying under it, so the focus scrolls
        // into view above it; the panel spans the screen, below the sidebar too (D-54).
        let panel_height = if self.state.date_prompt().is_some() {
            DatePromptPanel::HEIGHT
        } else {
            0
        };
        let [body, panel] = main.layout(&Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(panel_height),
        ]));
        let screen_body = body;
        let (body, entry) = self.draw_sidebar(frame, body);
        let (strip, sidebar) = match self.state.shortcuts() {
            Some(_) => layout::secondary_sidebar(screen, body),
            None => (body, None),
        };
        // Popups over the panes go below their headers, over every pane.
        let [_, outline] = strip.layout(&Layout::vertical([
            Constraint::Length(2),
            Constraint::Fill(1),
        ]));
        let small = screen.width < 40 || screen.height < 10;
        let focused = self.draw_panes(frame, strip, small);
        // Centered popups go over the sidebar, which a narrow screen draws over the outline.
        self.draw_shortcuts(frame, sidebar);
        if let Some(caret) = focused.caret {
            frame.set_cursor_position(caret);
            if let Some(menu) = self.state.command_menu() {
                draw_anchored(frame, Popup::commands(&menu, icons), outline, caret);
            }
        }
        let centered = self
            .state
            .color_picker()
            .map(|picker| Popup::colors(&picker, icons))
            .or_else(|| {
                self.state
                    .type_picker()
                    .map(|picker| Popup::block_types(&picker, icons))
            })
            .or_else(|| self.state.details().map(|details| Popup::details(&details)));
        if let Some(popup) = centered {
            let (width, height) = popup.size();
            let area = outline.centered(Constraint::Length(width), Constraint::Length(height));
            frame.render_widget(popup, area);
        }
        let finder = self
            .state
            .go_to_symbol()
            .map(|view| ("Go to Symbol", view))
            .or_else(|| self.state.move_to().map(|view| ("Move to", view)));
        if !self.state.in_conflict() {
            if let Some((title, view)) = finder {
                let icon = |id: &ItemId| widgets::block_icon(self.state.document(), *id);
                draw_finder(frame, FinderPopup::new(title, &view, icons, &icon), outline);
            }
            if let Some(view) = self.state.go_to_command() {
                let icon = |command: &Command| widgets::command_icon(*command);
                let finder = FinderPopup::new("Go to Command", &view, icons, &icon);
                draw_finder(frame, finder, outline);
            }
            if let Some(view) = self.state.save_as() {
                let prompt = SaveAsPrompt::new(&view);
                frame.set_cursor_position(prompt.caret(outline));
                frame.render_widget(prompt, outline);
            }
            if self.state.context_menu().is_none() {
                self.menu_anchor = None;
            }
            let anchor = self.menu_anchor.or(if self.state.sidebar_focused() {
                entry
            } else {
                focused.block
            });
            if let (Some(menu), Some(anchor)) = (self.state.context_menu(), anchor) {
                let sidebar = self.state.sidebar_focused();
                let popup = Popup::context_menu(
                    &menu,
                    |command| self.keymap.bindings(command, sidebar),
                    icons,
                );
                draw_anchored(frame, popup, screen_body, anchor);
            }
        }
        self.draw_status(frame, status);
        self.draw_date_prompt(frame, panel);
        if self.state.in_conflict() {
            self.draw_conflict(frame, main);
        }
    }

    /// The panes side by side in `strip`, each as wide as [`layout::pane_widths`] makes it, the
    /// strip scrolled to keep the focused pane fully in view, or on a `small` screen only the
    /// focused pane, across the strip (ui.md §1); returns what overlays need of the focused pane.
    fn draw_panes(&mut self, frame: &mut Frame, strip: Rect, small: bool) -> FocusedPane {
        let mut panes: Vec<(usize, PaneView)> =
            self.state.panes().into_iter().enumerate().collect();
        self.scrolls
            .retain(|id, _| panes.iter().any(|(_, pane)| pane.id == *id));
        self.wheel
            .retain(|id, _| panes.iter().any(|(_, pane)| pane.id == *id));
        if small {
            panes.retain(|(_, pane)| pane.focused);
        }
        let widths = if small {
            vec![strip.width]
        } else {
            let chosen: Vec<Option<u16>> = panes.iter().map(|(_, pane)| pane.width).collect();
            layout::pane_widths(strip.width, &chosen)
        };
        let starts: Vec<u16> = widths
            .iter()
            .scan(0u16, |start, &width| {
                let this = *start;
                *start = start.saturating_add(width);
                Some(this)
            })
            .collect();
        let total = widths
            .iter()
            .fold(0u16, |sum, &width| sum.saturating_add(width));
        let at = panes
            .iter()
            .position(|(_, pane)| pane.focused)
            .unwrap_or_default();
        let start = starts.get(at).copied().unwrap_or_default();
        let width = widths.get(at).copied().unwrap_or_default();
        self.pane_width = width;
        self.strip_width = strip.width;
        self.strip_scroll =
            layout::strip_offset(self.strip_scroll, start..start + width, strip.width, total);
        let mut focused = FocusedPane::default();
        self.drawn.panes.clear();
        let count = panes.len();
        for (index, ((at, pane), (&start, &width))) in
            panes.iter().zip(starts.iter().zip(&widths)).enumerate()
        {
            let divider = index + 1 < count;
            let left = start.saturating_sub(self.strip_scroll);
            let skipped = self.strip_scroll.saturating_sub(start);
            let shown = width
                .saturating_sub(skipped)
                .min(strip.width.saturating_sub(left));
            if shown == 0 {
                continue;
            }
            let x = strip.x.saturating_add(left);
            self.drawn.panes.push(mouse::DrawnPane {
                at: *at,
                id: pane.id,
                shown: Rect::new(x, strip.y, shown, strip.height),
                skipped,
                width: width.saturating_sub(u16::from(divider)),
            });
            if shown == width {
                let area = Rect::new(x, strip.y, width, strip.height);
                let drawn = self.draw_pane(frame.buffer_mut(), area, pane, divider);
                if pane.focused {
                    focused = drawn;
                }
            } else {
                // A pane cut off at an edge of the strip is drawn whole and shown in part. It
                // never has the keys, as the focused pane always fits in view, so the overlays
                // need nothing of it.
                let area = Rect::new(0, strip.y, width, strip.height);
                let mut whole = Buffer::empty(area);
                self.draw_pane(&mut whole, area, pane, divider);
                let shown = Rect::new(x, strip.y, shown, strip.height);
                copy(&whole, skipped, frame.buffer_mut(), shown);
            }
        }
        focused
    }

    /// `pane` in `area` with a rule on its right edge when `divider` says another pane follows:
    /// its breadcrumb header, dimmed unless it has the keys (ui.md §1), over a rule and its
    /// outline; returns what overlays need of it.
    fn draw_pane(
        &mut self,
        buf: &mut Buffer,
        area: Rect,
        pane: &PaneView,
        divider: bool,
    ) -> FocusedPane {
        let area = if divider {
            let rule = Block::new()
                .borders(Borders::RIGHT)
                .border_style(widgets::rule_style());
            let inner = rule.inner(area);
            rule.render(area, buf);
            inner
        } else {
            area
        };
        let [header, separator, outline] = area.layout(&Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ]));
        let [_, crumbs] = header.layout(&Layout::horizontal([
            Constraint::Length(widgets::GUTTER_WIDTH),
            Constraint::Fill(1),
        ]));
        let line = match pane.location {
            Location::Outline(_) => {
                widgets::breadcrumbs(&pane.breadcrumbs, usize::from(crumbs.width), self.icons)
            }
            Location::Today => widgets::today_header(self.icons),
        };
        line.render(crumbs, buf);
        if !pane.focused {
            buf.set_style(crumbs, Style::new().dim());
        }
        Line::styled(
            "─".repeat(usize::from(separator.width)),
            widgets::rule_style(),
        )
        .render(separator, buf);
        self.draw_outline(buf, outline, pane)
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
    /// (FR-KEY-04, D-28), its list scrolled no further than its end and naming the keys that
    /// scroll it while they do (D-55); a closed panel opens at the top.
    fn draw_shortcuts(&mut self, frame: &mut Frame, area: Option<ratatui::layout::Rect>) {
        let (Some(groups), Some(area)) = (self.state.shortcuts(), area) else {
            self.drawn.shortcuts = None;
            self.shortcuts_scroll = 0;
            return;
        };
        self.drawn.shortcuts = Some(area);
        let context = self.context();
        let scrolls =
            context.mode == Mode::Navigate && context.overlay.is_none() && !context.sidebar;
        let [up, down] =
            [Command::ScrollShortcutsUp, Command::ScrollShortcutsDown].map(|command| {
                if scrolls {
                    self.keymap.bindings(command, false)
                } else {
                    String::new()
                }
            });
        // The keys are the keymap's, so the panel shows the config file's bindings (ui.md §4); a
        // command the config file left without keys is not listed.
        let groups: Vec<(Group, Vec<(Command, String)>)> = groups
            .into_iter()
            .map(|(group, entries)| {
                let keys = entries
                    .into_iter()
                    .map(|entry| (entry.command, self.keymap.labels(entry.command, entry.mode)))
                    .filter(|(_, keys)| !keys.is_empty())
                    .collect();
                (group, keys)
            })
            .collect();
        let panel = ShortcutsPanel::new(&groups, self.keyboard, self.icons);
        let (last, page) = panel.scroll_range(area);
        self.shortcuts_scroll = self.shortcuts_scroll.min(last);
        self.shortcuts_page = page;
        frame.render_widget(panel.scrolled(self.shortcuts_scroll, [&up, &down]), area);
    }

    /// `pane`'s outline in `area`, scrolled to keep its focused block in view, which is marked
    /// when the pane has the keys, and drawn as its editor while editing; returns, for the pane
    /// with the keys, where the caret and the focused block's text are on screen.
    fn draw_outline(&mut self, buf: &mut Buffer, area: Rect, pane: &PaneView) -> FocusedPane {
        let rows = &pane.rows;
        let focused = pane
            .focus
            .and_then(|focus| rows.iter().position(|row| row.id == focus));
        let editor = self.state.editor().filter(|_| pane.focused);
        let icons = self.icons;
        let view = OutlineView::new(rows, (self.clock)().date())
            .editor(editor.as_ref())
            .wrap(self.wrap)
            .icons(icons);
        let scroll = self.scrolls.get(&pane.id).copied().unwrap_or_default();
        let anchor = if pane.focused {
            self.page_rows = usize::from(area.height);
            self.page_anchor.take()
        } else {
            None
        };
        // Only the rows near the focus or the view are measured, so drawing does not grow with
        // the outline.
        let row_height = |index| {
            rows.get(index..=index)
                .map_or(1, |row| view.rows(row).heights(area.width).iter().sum())
        };
        let height = usize::from(area.height);
        let (scroll, focus_line) = if let Some(lines) = self.wheel.get_mut(&pane.id) {
            let scroll =
                layout::scroll_by(scroll, mem::take(lines), height, rows.len(), &row_height);
            let line = focused.and_then(|at| layout::line_of(scroll, at, height, &row_height));
            (scroll, line)
        } else {
            let (scroll, line) =
                layout::scroll_rows(scroll, focused, anchor, height, rows.len(), &row_height);
            (scroll, focused.map(|_| line))
        };
        self.scrolls.insert(pane.id, scroll);
        let shown = rows.get(scroll.row..).unwrap_or_default();
        view.rows(shown)
            .focused(
                focused
                    .filter(|_| pane.focused)
                    .and_then(|index| index.checked_sub(scroll.row)),
            )
            .offset(scroll.line)
            .render(area, buf);
        if !pane.focused {
            return FocusedPane::default();
        }
        self.focus_line = focus_line;
        let line = u16::try_from(focus_line.unwrap_or_default()).unwrap_or(u16::MAX);
        // A focus the wheel scrolled out of view anchors no caret or menu.
        let row = focused
            .filter(|_| focus_line.is_some())
            .and_then(|index| rows.get(index));
        let caret = editor
            .as_ref()
            .filter(|_| !self.state.in_conflict())
            .zip(row)
            .map(|(editor, row)| {
                let text_width = area.width.saturating_sub(widgets::GUTTER_WIDTH);
                let (_, column) = widgets::editor_line(row, editor, usize::from(text_width), icons);
                let x = u16::try_from(column)
                    .unwrap_or(u16::MAX)
                    .min(text_width.saturating_sub(1));
                (
                    area.x.saturating_add(widgets::GUTTER_WIDTH + x),
                    area.y.saturating_add(line),
                )
            });
        let block = row.map(|row| {
            let column = u16::try_from(widgets::text_column(row, icons)).unwrap_or(u16::MAX);
            (
                area.x
                    .saturating_add(widgets::GUTTER_WIDTH)
                    .saturating_add(column),
                area.y.saturating_add(line),
            )
        });
        FocusedPane { caret, block }
    }

    /// The sidebar on the left of `area`, scrolled to keep its focused entry visible, while it is
    /// shown and `area` has room for it; returns the rest of `area`, for the pane (ui.md §1), and
    /// where the focused entry starts on screen, if one is focused.
    fn draw_sidebar(
        &mut self,
        frame: &mut Frame,
        area: ratatui::layout::Rect,
    ) -> (ratatui::layout::Rect, Option<(u16, u16)>) {
        self.drawn.sidebar = None;
        let (Some(view), Some(width)) = (self.state.sidebar(), layout::sidebar_width(area.width))
        else {
            return (area, None);
        };
        let [sidebar, pane] = area.layout(&Layout::horizontal([
            Constraint::Length(width),
            Constraint::Fill(1),
        ]));
        let panel = SidebarPanel::new(&view, self.icons);
        self.sidebar_scroll = panel.scroll(self.sidebar_scroll, usize::from(sidebar.height));
        let entry = panel.focused_line().map(|line| {
            let line = u16::try_from(line.saturating_sub(self.sidebar_scroll)).unwrap_or(u16::MAX);
            (
                sidebar.x.saturating_add(widgets::GUTTER_WIDTH),
                sidebar.y.saturating_add(line),
            )
        });
        self.drawn.sidebar = Some((sidebar, self.sidebar_scroll));
        frame.render_widget(panel.offset(self.sidebar_scroll), sidebar);
        (pane, entry)
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
        let icons = self.icons;
        let (icon, mode) = match self.state.mode() {
            Mode::Navigate => (Icon::Navigate, "NAVIGATE"),
            Mode::Edit => (Icon::Edit, "EDIT"),
        };
        let mut spans = vec![format!("{} {mode}", icon.glyph(icons)).bold()];
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
            icons,
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
        } else if self.state.move_to().is_some() {
            Some(Overlay::MoveTo)
        } else if self.state.go_to_command().is_some() {
            Some(Overlay::GoToCommand)
        } else if self.state.save_as().is_some() {
            Some(Overlay::SaveAs)
        } else if self.state.context_menu().is_some() {
            Some(Overlay::ContextMenu)
        } else {
            self.state.details().map(|_| Overlay::Details)
        }
    }

    /// Whether the user asked to quit.
    #[must_use]
    pub fn should_quit(&self) -> bool {
        self.quit
    }
}

/// What overlays need of the pane with the keys, as drawn: where the caret is while editing, and
/// where the focused block's text starts.
#[derive(Debug, Default)]
struct FocusedPane {
    caret: Option<(u16, u16)>,
    block: Option<(u16, u16)>,
}

/// Copies the columns of `from` starting `skipped` columns from its left onto `area` of `to`.
fn copy(from: &Buffer, skipped: u16, to: &mut Buffer, area: Rect) {
    for y in area.top()..area.bottom() {
        for (offset, x) in (area.left()..area.right()).enumerate() {
            let column = from
                .area
                .x
                .saturating_add(skipped)
                .saturating_add(u16::try_from(offset).unwrap_or(u16::MAX));
            if let (Some(cell), Some(target)) = (from.cell((column, y)), to.cell_mut((x, y))) {
                *target = cell.clone();
            }
        }
    }
}

/// `popup` anchored at the caret or element at `anchor` inside `area`, no taller than the room
/// there (ui.md §5).
fn draw_anchored(frame: &mut Frame, popup: Popup, area: ratatui::layout::Rect, anchor: (u16, u16)) {
    let (width, height) = popup.size();
    let height = height.min(layout::room(area, anchor.1));
    frame.render_widget(popup, layout::anchored(area, anchor, (width, height)));
}

/// The fuzzy finder `finder` over `area`, with the caret after its query.
fn draw_finder<T>(frame: &mut Frame, finder: FinderPopup<'_, T>, area: ratatui::layout::Rect) {
    frame.set_cursor_position(finder.caret(area));
    frame.render_widget(finder, area);
}
