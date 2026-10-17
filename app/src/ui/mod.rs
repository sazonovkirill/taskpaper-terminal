//! The terminal frontend: turns crossterm events into app messages, carries out effects and
//! draws the state with Ratatui.

pub mod animation;
pub mod config;
pub mod effects;
pub mod keymap;
pub mod layout;
mod mouse;
mod settings;
pub mod theme;
mod weather;
pub mod widgets;

use std::{collections::HashMap, mem, ops::Range, path::PathBuf, time::Duration};

use crossterm::event::{Event, KeyModifiers};
use jiff::Zoned;
use ratatui::{
    Frame, Terminal,
    backend::Backend,
    buffer::{Buffer, CellWidth},
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Widget},
};

use crate::{
    app::{
        Animations, AttachmentsInput, Configuration, ConflictInput, Disk, Editor, Effect, Location,
        Message, Mode, Origin, PaneId, PaneView, PanelItem, Row, SaveAs, SaveState, SettingsInput,
        State,
        commands::{Command, REGISTRY},
    },
    document::ItemId,
};
use animation::{Animator, Before, DrawnPane, Intent, Shown};
use config::{Config, WriteError};
use effects::{
    BackupSettings, Browser, Clipboard, FileWatch, Io, PlainTexts, Recovery, RecoveryError,
};
use keymap::{Context, KeyAction, Keymap, Overlay, PromptKey, ScrollBy};
use layout::Scroll;
pub use mouse::acts_on;
pub use theme::ColorSupport;
use theme::{BuiltIn, Icon, IconSet, IconSetting, TerminalColors, Theme};
use weather::{Season, Weather};
use widgets::{
    AttachmentsPanel, ConflictPrompt, DatePromptPanel, FinderPopup, KeptLines, Keyboard,
    KeymapDialog, NavigatorPanel, NodeEditorWindow, OutlineView, Popup, QuickAddPanel,
    SaveAsPrompt, SettingsWindow, ShortcutsPanel,
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
    /// The `icons` setting, which the settings window shows and changes (ui.md §7).
    icon_setting: IconSetting,
    /// The backups the disk worker makes, which the settings window shows and changes
    /// (file-io.md §2).
    backups: BackupSettings,
    /// Where each pane's outline starts on screen: its top row stays put when rows above it
    /// change height.
    scrolls: HashMap<PaneId, Scroll>,
    /// How many columns of the strip of panes are scrolled past on its left (navigation.md §5).
    strip_scroll: u16,
    /// The focused pane's width at the last draw, which narrowing and widening change.
    pane_width: u16,
    /// The width of the strip of panes at the last draw, the most a pane is widened to.
    strip_width: u16,
    /// The rows of the focused pane's outline a page down motion moves past, from the last draw.
    page_down_rows: usize,
    /// The rows of the focused pane's outline a page up motion moves past, from the last draw.
    page_up_rows: usize,
    /// The outline line the focused block started on at the last draw, counted from the top of
    /// the screen.
    focus_line: Option<usize>,
    /// Where the next draw puts the focused block's first line, counted from the top of the
    /// screen: where it was before a page motion.
    page_anchor: Option<usize>,
    /// The first Navigator line on screen.
    navigator_scroll: usize,
    /// The screen lines of the outline rows drawn lately.
    kept_lines: KeptLines,
    /// Whether the process was told to end, so only saving is left.
    ending: bool,
    /// The first line of the conflict prompt's diff on screen.
    diff_scroll: usize,
    /// How many diff lines the prompt showed at the last draw, which a page scrolls.
    diff_rows: usize,
    /// What the terminal has shown of its keyboard, which the shortcuts panel notes.
    keyboard: Keyboard,
    /// The theme chosen, drawn unless the theme picker previews another (FR-UI-06).
    theme: BuiltIn,
    /// What colors the terminal and the user allow, which decide how the theme is drawn.
    colors: ColorSupport,
    /// The colors the terminal reported it draws with, if it did, which Terminal Dark is drawn
    /// in with truecolor (D-128).
    terminal_colors: Option<TerminalColors>,
    /// The settings file, where settings changed in the app are kept (FR-SET-04).
    settings: Option<PathBuf>,
    /// The first line of the shortcuts panel's list on screen (D-55).
    shortcuts_scroll: usize,
    /// The mode whose list the panel showed at the last draw; another mode's list starts at its
    /// top (ui.md §4).
    shortcuts_mode: Mode,
    /// How many lines a page of the shortcuts panel was at the last draw, which its keys scroll.
    shortcuts_page: usize,
    /// The first line of the Keymap dialog's sheet on screen while the dialog is open (D-140).
    keymap_scroll: Option<usize>,
    /// How many lines a page of the Keymap dialog was at the last draw, which its keys scroll.
    keymap_page: usize,
    /// The first line of the attachments panel's content on screen, back at the top when the
    /// panel opens (D-107).
    attachments_scroll: usize,
    /// What the attachments panel had selected at the last draw, while it was open.
    attachments_selected: Option<PanelItem>,
    /// How many lines a page of the attachments panel was at the last draw.
    attachments_page: usize,
    /// The lines of what the attachments panel had selected or typed at the last draw.
    attachments_current: Option<Range<usize>>,
    /// How many columns the last draw scrolled the text being typed in a dialog, which the next
    /// keeps while the caret stays in view (editing.md §2).
    field_scroll: usize,
    /// Where the last draw put what the mouse acts on.
    drawn: mouse::Drawn,
    /// The panes the wheel scrolled, which stay where it left them rather than show their focus
    /// until a key or a click, with the lines it scrolled since the last draw (ui.md §6).
    wheel: HashMap<PaneId, isize>,
    /// Where a right-click opened the context menu, which shows there while it is open (ui.md §5).
    menu_anchor: Option<(u16, u16)>,
    /// The effects drawn over the frames and the spinner (ui.md §9).
    animator: Animator,
    /// The `animations` setting a session runs with unless the settings file sets it (ui.md §9.3).
    default_animations: Animations,
    /// The weather the app shows once it sits idle (ui.md §10).
    weather: Weather,
    /// Works out the plain text of each shown document's lines on another thread.
    plain_texts: PlainTexts,
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
            wrap: false,
            icons: IconSet::default(),
            icon_setting: IconSetting::default(),
            backups: BackupSettings::default(),
            scrolls: HashMap::new(),
            strip_scroll: 0,
            pane_width: 0,
            strip_width: 0,
            page_down_rows: 0,
            page_up_rows: 0,
            focus_line: None,
            page_anchor: None,
            navigator_scroll: 0,
            kept_lines: KeptLines::default(),
            ending: false,
            diff_scroll: 0,
            diff_rows: 0,
            keyboard: Keyboard::default(),
            theme: BuiltIn::DEFAULT,
            colors: ColorSupport::default(),
            terminal_colors: None,
            settings: None,
            shortcuts_scroll: 0,
            shortcuts_mode: Mode::Navigate,
            shortcuts_page: 0,
            keymap_scroll: None,
            keymap_page: 0,
            attachments_scroll: 0,
            attachments_selected: None,
            attachments_current: None,
            attachments_page: 0,
            field_scroll: 0,
            drawn: mouse::Drawn::default(),
            wheel: HashMap::new(),
            menu_anchor: None,
            animator: Animator::new(Animations::Full),
            default_animations: Animations::Full,
            weather: Weather::new(),
            plain_texts: PlainTexts::default(),
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
        self.icon_setting = setting;
        self.icons = self.icons.with_setting(setting);
        self
    }

    /// The frontend animating as `setting` allows unless a settings file says otherwise, which the
    /// settings window shows as the default (ui.md §9.3).
    #[must_use]
    pub fn with_animations(mut self, setting: Animations) -> Self {
        self.default_animations = setting;
        self.animator.set(setting);
        self
    }

    /// The frontend drawing in the colors `colors` allows (ui.md §3, N-UX-03).
    #[must_use]
    pub fn with_colors(mut self, colors: ColorSupport) -> Self {
        self.colors = colors;
        self
    }

    /// The frontend drawing Terminal Dark in `colors`, the colors the terminal reported it draws
    /// with if it did, when it has truecolor (ui.md §3, D-128).
    #[must_use]
    pub fn with_terminal_colors(mut self, colors: Option<TerminalColors>) -> Self {
        self.terminal_colors = colors;
        self
    }

    /// The frontend running with the key bindings, autosave delay, icons, word wrap, theme,
    /// animations, inbox handling, special views in the Navigator and backups `config` holds,
    /// keeping settings changed in the app in its file and reporting once what it ignored of the
    /// file (FR-SET-04). The disk worker makes its backups as `config` says until the settings
    /// window changes them.
    #[must_use]
    pub fn with_config(mut self, config: Config) -> Self {
        let problems: Vec<String> = config.problems().iter().map(ToString::to_string).collect();
        self.settings = config.file().map(ToOwned::to_owned);
        self.backups = config.backups().clone();
        if let Some(theme) = config.theme {
            self.theme = theme;
        }
        self.keymap = config.keymap;
        if let Some(setting) = config.icons {
            self = self.with_icons(setting);
        }
        if let Some(wrap) = config.wrap {
            self.wrap = wrap;
        }
        if let Some(setting) = config.animations {
            self.animator.set(setting);
        }
        if let Some(on) = config.weather {
            self.weather.set_on(on);
        }
        if let Some(minutes) = config.weather_idle_minutes {
            self.weather.set_idle_minutes(minutes);
        }
        if let Some(delay) = config.autosave_delay {
            self.send(
                Message::Configure(Configuration::AutosaveDelay(delay)),
                Duration::ZERO,
            );
        }
        if let Some(on) = config.special_inbox {
            self.send(
                Message::Configure(Configuration::SpecialInbox(on)),
                Duration::ZERO,
            );
        }
        if let Some(on) = config.today_starred {
            self.send(
                Message::Configure(Configuration::TodayStarred(on)),
                Duration::ZERO,
            );
        }
        for &(view, on) in &config.navigator_views {
            self.send(
                Message::Configure(Configuration::NavigatorView(view, on)),
                Duration::ZERO,
            );
        }
        if let Some(on) = config.special_today {
            self.send(
                Message::Configure(Configuration::SpecialToday(on)),
                Duration::ZERO,
            );
        }
        for &(kind, on) in &config.attachments {
            self.send(
                Message::Configure(Configuration::Attachments(kind, on)),
                Duration::ZERO,
            );
        }
        if !problems.is_empty() {
            self.send(Message::ConfigProblems(problems), Duration::ZERO);
        }
        self
    }

    /// Handles one input at time `now` (since the frontend started). The input that ends the
    /// weather does nothing else (ui.md §10.1).
    pub fn handle(&mut self, input: Input, now: Duration) {
        if let Input::Terminal(event) = &input
            && weather::is_input(event)
            && self.weather.input(now)
        {
            return;
        }
        self.animator.at(now);
        let before = Before::of(&self.state);
        match input {
            Input::Terminal(event) => self.handle_event(&event, now),
            Input::Core(message) => {
                self.animator.received(&message);
                self.send(message, now);
            }
        }
        self.animator.settle(&before, &self.state);
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
        // The key ended a sequence that sent its own message, such as a configured key that also
        // starts a longer sequence, and now acts where that message left the app.
        if replay {
            self.handle_event(event, now);
        }
    }

    /// What the keymap needs to know to turn a key into a message.
    fn context(&self) -> Context {
        Context {
            page_down_rows: self.page_down_rows,
            page_up_rows: self.page_up_rows,
            now: (self.clock)().datetime(),
            mode: self.state.mode(),
            command_menu: self.state.command_menu().is_some(),
            overlay: self.overlay(),
            shortcuts: self.state.shortcuts().is_some(),
            navigator: self.state.navigator_focused(),
        }
    }

    /// Handles a key of the conflict prompt at time `now`.
    fn handle_prompt_key(&mut self, event: &Event, now: Duration) {
        match keymap::prompt_key(event, self.diff_rows) {
            Some(PromptKey::Send(message)) => {
                if message == Message::Conflict(ConflictInput::ToggleDiff) {
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
    /// message when it is a binding of its own, and lets the core start a save that is due.
    pub fn tick(&mut self, now: Duration) {
        self.animator.at(now);
        let before = Before::of(&self.state);
        if let Some(action) = self.keymap.expire(now, &self.context()) {
            self.act(action, now);
        }
        let effects = self.update(Message::Tick {
            now,
            today: (self.clock)().date(),
        });
        self.run(effects, now);
        self.midnight = self.next_midnight(now);
        self.animator.settle(&before, &self.state);
        self.weather.tick(now, self.weather_season());
    }

    /// The season whose weather the idle timer starts: none in summer, or unless animations are
    /// full, which they are not by default over SSH (ui.md §10.1).
    fn weather_season(&self) -> Option<Season> {
        (self.animator.setting() == Animations::Full)
            .then(|| Season::of((self.clock)().date()))
            .flatten()
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
            .chain(self.animator.deadline())
            .chain(self.weather.deadline(self.weather_season()))
            .min()
    }

    /// When the local date changes, counted like `now`, if a pane shows a date chip, which the
    /// new date names differently, or a special view, whose entries it may change.
    fn next_midnight(&self, now: Duration) -> Option<Duration> {
        let dated = self.state.panes().iter().any(|pane| {
            matches!(pane.location, Location::View(_))
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

    /// What to tell the user once the terminal is restored after a quit: where their changes
    /// went when they quit with the conflict prompt open (file-io.md §3).
    #[must_use]
    pub fn farewell(&self) -> Option<String> {
        self.state
            .left_in()
            .map(|copy| format!("tpt: left theirs on disk; mine is in {}", copy.display()))
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
            KeyAction::ScrollAttachments(by) => self.scroll_attachments(by, now),
            KeyAction::ShowKeymap => self.keymap_scroll = Some(0),
            KeyAction::CloseKeymap => self.keymap_scroll = None,
            KeyAction::ScrollKeymap(by) => {
                self.keymap_scroll = self.keymap_scroll.map(|scroll| match by {
                    ScrollBy::Up => scroll.saturating_sub(1),
                    ScrollBy::Down => scroll.saturating_add(1),
                    ScrollBy::PageUp => scroll.saturating_sub(self.keymap_page),
                    ScrollBy::PageDown => scroll.saturating_add(self.keymap_page),
                    ScrollBy::Top => 0,
                    ScrollBy::Bottom => usize::MAX,
                });
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
            KeyAction::OpenSettings => {
                let settings = self.listed_settings();
                self.send(Message::Settings(SettingsInput::Open(settings)), now);
            }
            KeyAction::ChooseTheme => {
                let names = BuiltIn::all()
                    .map(|theme| theme.name().to_owned())
                    .collect();
                let current = self.theme.position();
                self.send(Message::OpenThemePicker { names, current }, now);
            }
            KeyAction::SetAnimations(setting) => self.set_animations(setting, now),
            KeyAction::StartWeather => match Season::of((self.clock)().date()) {
                Some(season) => self.weather.start(now, season),
                None => self.send(Message::NoWeather, now),
            },
        }
    }

    /// Tells the core the time, then sends it `message`, and carries out the effects.
    fn send(&mut self, message: Message, now: Duration) {
        if message == Message::Terminate {
            self.ending = true;
        }
        let mut effects = self.update(Message::Tick {
            now,
            today: (self.clock)().date(),
        });
        effects.extend(self.update(message));
        self.run(effects, now);
        self.midnight = self.next_midnight(now);
    }

    /// Sends the core `message` and returns the effects it asks for, letting the animator take in
    /// what it changed and whether it refused it.
    fn update(&mut self, message: Message) -> Vec<Effect> {
        let intent = Intent::of(&message);
        let effects = self.state.update(message);
        self.animator.observe(intent, &self.state);
        effects
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
                    self.animator.submitted(&disk, now);
                    self.io.submit(disk);
                    if saving {
                        self.keep_view();
                    }
                }
                Effect::CopyToClipboard(text) => self.clipboard.copy(&text),
                Effect::ReadClipboard(at) => {
                    if let Some(text) = self.clipboard.paste() {
                        let effects = self.update(Message::Pasted { text, at });
                        self.run(effects, now);
                    }
                }
                Effect::OpenUrl(url) => {
                    if let Err(error) = self.browser.open(&url) {
                        let reason = error.to_string();
                        let effects = self.update(Message::OpenUrlFailed { url, reason });
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
                Effect::SetTheme(position) => {
                    if let Some(theme) = BuiltIn::nth(position) {
                        self.theme = theme;
                        self.keep_setting("theme", Some(theme.id().into()), now);
                    }
                }
                Effect::ChangeSetting { at, value } => self.change_setting(at, value, now),
            }
        }
    }

    /// Has the disk worker write `value` for the setting `key` to the settings file, or remove
    /// the setting when there is no value, or reports at time `now` that there is no file, the
    /// setting holding for the session either way (FR-SET-04).
    fn keep_setting(&mut self, key: &'static str, value: Option<toml_edit::Value>, now: Duration) {
        if let Some(file) = &self.settings {
            self.io.store_setting(file.clone(), key, value);
        } else {
            let problem = WriteError::NoFile.problem(key);
            self.send(Message::ConfigProblems(vec![problem]), now);
        }
    }

    /// The colors to draw in: the theme the theme picker or the settings window's picker
    /// highlights while it is open, so that it previews it (FR-UI-06, ui.md §8), else the theme
    /// chosen, as the terminal can show them.
    fn palette(&self) -> Theme {
        self.state
            .theme_picker()
            .and_then(|view| view.entries.get(view.selected).map(|entry| entry.value))
            .and_then(BuiltIn::nth)
            .or_else(|| self.previewed_theme())
            .unwrap_or(self.theme)
            .colors(self.colors, self.terminal_colors.as_ref())
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
        self.animator.at(now);
        if !self.ending && terminal.draw(|frame| self.draw(frame)).is_err() {
            self.send(Message::Terminate, now);
        }
    }

    /// Draws the whole screen: the panes side by side, each a breadcrumb header over a rule and
    /// its outline scrolled to keep its focus visible, with the effects running over them, the
    /// shortcuts panel at the right while open, the date prompt's panel below them while open, and
    /// the status line, all dimmed behind a dialog that takes the keys, such as the Navigator.
    pub fn draw(&mut self, frame: &mut Frame) {
        self.kept_lines.next_frame();
        let theme = self.palette();
        let screen = frame.area();
        let icons = self.icons;
        let [main, status] = screen.layout(&Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
        ]));
        // The outline gives up the panel's rows rather than lying under it, so the focus scrolls
        // into view above it; the panel spans the screen (D-54).
        let panel_height = if self.state.date_prompt().is_some() {
            DatePromptPanel::HEIGHT
        } else if self.state.quick_add().is_some() {
            QuickAddPanel::HEIGHT
        } else {
            0
        };
        let [body, panel] = main.layout(&Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(panel_height),
        ]));
        let screen_body = body;
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
        self.animator
            .process(frame.buffer_mut(), &theme, self.colors);
        // Centered popups go over the secondary sidebar, which a narrow screen draws over the
        // outline.
        self.draw_shortcuts(frame, sidebar);
        self.draw_status(frame, status);
        // A dialog that takes the keys sets itself apart from the screen behind it (D-54, D-69).
        if self.overlay().is_some() || self.state.navigator_focused() || self.state.in_conflict() {
            frame.buffer_mut().set_style(screen, Style::new().dim());
        }
        // A dialog over the Navigator dims it too; its own `Clear` would undo the dim above.
        let entry = self.draw_navigator(
            frame,
            outline,
            self.overlay().is_some() || self.state.in_conflict(),
        );
        if let Some(caret) = focused.caret {
            // The weather covers the caret too.
            if !self.weather.playing() {
                frame.set_cursor_position(caret);
            }
            if let Some(menu) = self.state.command_menu() {
                draw_anchored(frame, Popup::commands(&menu, icons, &theme), outline, caret);
            }
        }
        let centered = self
            .state
            .color_picker()
            .map(|picker| Popup::colors(&picker, icons, &theme))
            .or_else(|| {
                self.state
                    .type_picker()
                    .map(|picker| Popup::block_types(&picker, icons))
            });
        if let Some(popup) = centered {
            let (width, height) = popup.size();
            let area = outline.centered(Constraint::Length(width), Constraint::Length(height));
            frame.render_widget(popup, area);
        }
        // How far the text whose caret a dialog places is scrolled; the last placed wins.
        let mut typed = None;
        if !self.state.in_conflict() {
            typed = self.draw_dialogs(frame, outline, screen_body, &theme);
            if self.state.context_menu().is_none() {
                self.menu_anchor = None;
            }
            let anchor = self.menu_anchor.or(if self.state.navigator_focused() {
                entry
            } else {
                focused.block
            });
            if let (Some(menu), Some(anchor)) = (self.state.context_menu(), anchor) {
                let navigator = self.state.navigator_focused();
                let popup = Popup::context_menu(
                    &menu,
                    |command| self.keymap.bindings(command, navigator),
                    icons,
                    &theme,
                );
                draw_anchored(frame, popup, screen_body, anchor);
            }
        }
        let date = self.draw_date_prompt(frame, panel);
        let quick_add = self.draw_quick_add(frame, panel, &theme);
        self.field_scroll = quick_add.or(date).or(typed).unwrap_or_default();
        self.draw_keymap(frame, screen, &theme);
        if self.state.in_conflict() {
            self.draw_conflict(frame, main);
        }
        paint_defaults(frame.buffer_mut(), &theme);
        let previewing = self.state.theme_picker().is_some() || self.state.settings().is_some();
        self.animator.finish(frame.buffer_mut(), theme, previewing);
        self.weather
            .draw(frame.buffer_mut(), self.icons, self.colors);
        hide_cut_graphemes(frame.buffer_mut());
        // Once the frame is worked out, so the work never slows a file's first frame.
        self.plain_texts.shown(self.state.document());
    }

    /// The panes side by side in `strip`, each as wide as [`layout::pane_widths`] makes it, the
    /// strip scrolled to keep the focused pane fully in view, or on a `small` screen only the
    /// focused pane, across the strip (ui.md §1); returns what overlays need of the focused pane.
    fn draw_panes(&mut self, frame: &mut Frame, strip: Rect, small: bool) -> FocusedPane {
        let mut panes: Vec<(usize, PaneView)> =
            self.state.panes().into_iter().enumerate().collect();
        let numbered = panes.len() > 1;
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
            let number = numbered.then_some(at + 1);
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
                let drawn = self.draw_pane(frame.buffer_mut(), area, pane, (divider, number), true);
                if pane.focused {
                    focused = drawn;
                }
            } else {
                // A pane cut off at an edge of the strip is drawn whole and shown in part. It
                // never has the keys, as the focused pane always fits in view, so the overlays
                // need nothing of it.
                let area = Rect::new(0, strip.y, width, strip.height);
                let mut whole = Buffer::empty(area);
                self.draw_pane(&mut whole, area, pane, (divider, number), false);
                let shown = Rect::new(x, strip.y, shown, strip.height);
                copy(&whole, skipped, frame.buffer_mut(), shown);
            }
        }
        focused
    }

    /// `pane` in `area` with a rule on its right edge when `divider` says another pane follows:
    /// its breadcrumb header, dimmed unless it has the keys, ending in its `number` while there
    /// are several panes (ui.md §1, D-116), over a rule and its outline, which effects go over
    /// when `on_screen` says `area` is where it shows; returns what overlays need of it.
    fn draw_pane(
        &mut self,
        buf: &mut Buffer,
        area: Rect,
        pane: &PaneView,
        (divider, number): (bool, Option<usize>),
        on_screen: bool,
    ) -> FocusedPane {
        let theme = self.palette();
        let area = if divider {
            let rule = Block::new()
                .borders(Borders::RIGHT)
                .border_style(widgets::rule_style(&theme));
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
        let [_, crumbs, _] = header.layout(&Layout::horizontal([
            Constraint::Length(widgets::GUTTER_WIDTH),
            Constraint::Fill(1),
            Constraint::Length(u16::from(number.is_some())),
        ]));
        let crumbs = number.map_or(crumbs, |number| {
            widgets::render_count(number, Style::new().fg(theme.dim_text), crumbs, buf)
        });
        let line = match pane.location {
            Location::Outline(_) => {
                widgets::breadcrumbs(&pane.breadcrumbs, usize::from(crumbs.width), self.icons)
            }
            Location::View(view) => widgets::view_header(view, self.icons, &theme),
        };
        let crumb = on_screen.then(|| last_crumb(pane, &line, crumbs, self.icons));
        line.render(crumbs, buf);
        if !pane.focused {
            buf.set_style(crumbs, Style::new().dim());
        }
        Line::styled(
            "─".repeat(usize::from(separator.width)),
            widgets::rule_style(&theme),
        )
        .render(separator, buf);
        self.draw_outline(buf, outline, pane, crumb)
    }

    /// The conflict prompt over `area`, its diff scrolled no further than its last page.
    fn draw_conflict(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let diff = self.state.diff();
        self.diff_rows = ConflictPrompt::diff_rows(area);
        let lines = diff.as_ref().map_or(0, Vec::len);
        self.diff_scroll = self.diff_scroll.min(lines.saturating_sub(self.diff_rows));
        let unsaved = self.state.save_state() != SaveState::Saved;
        let theme = self.palette();
        frame.render_widget(
            ConflictPrompt::new(diff.as_deref(), self.diff_scroll, unsaved, &theme),
            area,
        );
    }

    /// The Keymap dialog over the `screen`, while it is open, its sheet scrolled no further than
    /// its last line (D-140).
    fn draw_keymap(&mut self, frame: &mut Frame, screen: Rect, theme: &Theme) {
        let Some(scroll) = self.keymap_scroll else {
            return;
        };
        let sheet = self.keymap.sheet();
        let dialog = KeymapDialog::new(&sheet, theme);
        let (last, page) = dialog.scroll_range(screen);
        let scroll = scroll.min(last);
        self.keymap_scroll = Some(scroll);
        self.keymap_page = page;
        frame.render_widget(dialog.scrolled(scroll), screen);
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
        if context.mode != self.shortcuts_mode {
            self.shortcuts_mode = context.mode;
            self.shortcuts_scroll = 0;
        }
        let scrolls =
            context.mode == Mode::Navigate && context.overlay.is_none() && !context.navigator;
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
        // While the Navigator, a picker or a dialog that takes text has the keys, the panel lists
        // theirs (ui.md §4).
        let groups: Vec<_> = match self.keymap.popup_shortcuts(&context) {
            Some(group) => vec![group],
            None => groups
                .into_iter()
                .map(|(group, entries)| {
                    let keys = entries
                        .into_iter()
                        .map(|entry| {
                            (
                                entry.command.name(),
                                self.keymap.labels(entry.command, entry.mode),
                            )
                        })
                        .filter(|(_, keys)| !keys.is_empty())
                        .collect();
                    (group, keys)
                })
                .collect(),
        };
        let theme = self.palette();
        let panel = ShortcutsPanel::new(&groups, self.keyboard, self.icons, &theme);
        let (last, page) = panel.scroll_range(area);
        self.shortcuts_scroll = self.shortcuts_scroll.min(last);
        self.shortcuts_page = page;
        frame.render_widget(panel.scrolled(self.shortcuts_scroll, [&up, &down]), area);
    }

    /// `pane`'s outline in `area`, scrolled to keep its focused block in view, which is marked
    /// when the pane has the keys, and drawn as its editor while editing, telling the animator
    /// where its blocks are, and that its last crumb is at `crumb`, when it says `area` is where
    /// it shows; returns, for the pane with the keys, where the caret and the focused block's text
    /// are on screen.
    fn draw_outline(
        &mut self,
        buf: &mut Buffer,
        area: Rect,
        pane: &PaneView,
        crumb: Option<Rect>,
    ) -> FocusedPane {
        let rows = &pane.rows;
        let focused = pane
            .focus
            .and_then(|focus| rows.iter().position(|row| row.id == focus));
        let editor = self.state.editor().filter(|_| pane.focused);
        let icons = self.icons;
        let theme = self.palette();
        let view = OutlineView::new(&[], (self.clock)().date(), &theme)
            .editor(editor.as_ref())
            .editor_scroll(self.drawn.editor_scroll)
            .wrap(self.wrap)
            .kept(&self.kept_lines)
            .icons(icons);
        let scroll = self.scrolls.get(&pane.id).copied().unwrap_or_default();
        let anchor = pane.focused.then(|| self.page_anchor.take()).flatten();
        // Only the rows near the focus or the view are measured, so drawing does not grow with
        // the outline.
        let row_height = |index| {
            rows.get(index).map_or(1, |row| {
                view.rows(std::slice::from_ref(row))
                    .heights(area.width)
                    .iter()
                    .sum()
            })
        };
        let height = usize::from(area.height);
        if pane.focused
            && let Some(at) = focused
        {
            self.page_down_rows = layout::page_rows(at..rows.len(), height, &row_height);
            self.page_up_rows = layout::page_rows((0..at).rev(), height, &row_height);
        }
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
        // Every row is at least one line tall, so no more rows than lines show.
        let shown: Vec<_> = rows.iter_from(scroll.row).take(height).cloned().collect();
        view.rows(&shown)
            .focused(
                focused
                    .filter(|_| pane.focused)
                    .and_then(|index| index.checked_sub(scroll.row)),
            )
            .offset(scroll.line)
            .render(area, buf);
        if let Some(crumb) = crumb {
            let rows = block_lines(area, scroll.line, &shown, (scroll.row..).map(&row_height));
            self.animator.drawn(DrawnPane {
                id: pane.id,
                focused: pane.focused,
                shown: Shown {
                    area,
                    scroll,
                    focus: pane.focus,
                    rows: pane.rows.clone(),
                },
                rows,
                crumb,
            });
        }
        if !pane.focused {
            return FocusedPane::default();
        }
        self.focus_line = focus_line;
        // A focus the wheel scrolled out of view anchors no caret or menu.
        let row = focused
            .filter(|_| focus_line.is_some())
            .and_then(|index| rows.get(index));
        // The focused block's text starts below its group's heading, if it has one (D-80).
        let text_line =
            focus_line.unwrap_or_default() + row.map_or_default(widgets::heading_height);
        let line = u16::try_from(text_line).unwrap_or(u16::MAX);
        let caret = editor
            .as_ref()
            .filter(|_| !self.state.in_conflict())
            .zip(row)
            .map(|(editor, row)| self.editor_caret(editor, row, area, line));
        self.drawn.editor_scroll = caret.map(|(_, scroll)| scroll).unwrap_or_default();
        let caret = caret.map(|(cell, _)| cell);
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

    /// The caret's cell while `editor` edits `row`, drawn on screen line `line` of a pane's
    /// outline over `area`, and how far the text is scrolled, on from the last frame's scroll.
    fn editor_caret(
        &self,
        editor: &Editor,
        row: &Row,
        area: Rect,
        line: u16,
    ) -> ((u16, u16), usize) {
        let text_width = area.width.saturating_sub(widgets::GUTTER_WIDTH);
        let (_, caret) = widgets::editor_line(
            row,
            editor,
            self.drawn.editor_scroll,
            usize::from(text_width),
            self.icons,
            &self.palette(),
        );
        let x = u16::try_from(caret.column)
            .unwrap_or(u16::MAX)
            .min(text_width.saturating_sub(1));
        let cell = (
            area.x.saturating_add(widgets::GUTTER_WIDTH + x),
            area.y.saturating_add(line),
        );
        (cell, caret.scroll)
    }

    /// The open finder, Go to Symbol, Go to Project, Move to, Go to Command or the theme picker,
    /// over `area` in `theme` (navigation.md §8, ui.md §3).
    fn draw_finders(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let icons = self.icons;
        let finder = self
            .state
            .go_to_symbol()
            .map(|view| ("Go to Symbol", view))
            .or_else(|| {
                self.state
                    .go_to_project()
                    .map(|view| ("Go to Project", view))
            })
            .or_else(|| self.state.move_to().map(|view| ("Move to", view)));
        if let Some((title, view)) = finder {
            let document = self.state.document();
            let icon = |id: &ItemId| widgets::block_icon(document, *id);
            let text = |id: &ItemId| widgets::block_text_style(document, *id, theme);
            draw_finder(
                frame,
                FinderPopup::new(title, &view, icons, &icon, theme).text_style(&text),
                area,
            );
        }
        if let Some(view) = self.state.go_to_command() {
            let icon = |command: &Command| widgets::command_icon(*command, theme);
            let finder = FinderPopup::new("Go to Command", &view, icons, &icon, theme);
            draw_finder(frame, finder, area);
        }
        if let Some(view) = self.state.theme_picker() {
            let icon = |_: &usize| widgets::command_icon(Command::ChooseTheme, theme);
            draw_finder(
                frame,
                FinderPopup::new("Theme", &view, icons, &icon, theme)
                    .note(self.colors.themes_note()),
                area,
            );
        }
    }

    /// The settings window over `area` in `theme`, with its value picker over it while open, and
    /// the caret in the text it takes, if any (ui.md §8); returns how far that text is scrolled.
    fn draw_settings(&self, frame: &mut Frame, area: Rect, theme: &Theme) -> Option<usize> {
        let view = self.state.settings()?;
        let file = self
            .settings
            .as_deref()
            .map(|file| effects::abbreviate_path(file, self.home.as_deref()));
        let window = SettingsWindow::new(&view, file.as_deref(), self.icons, settings::icon, theme)
            .text_scroll(self.field_scroll);
        let caret = window.caret(area);
        if let Some((cell, _)) = caret {
            frame.set_cursor_position(cell);
        }
        frame.render_widget(window, area);
        if let Some((name, picker)) = &view.picker {
            let at = view.rows.get(view.selected).map(|row| row.at);
            let icon = |_: &usize| (at.map_or(Icon::Command, settings::icon), Style::new());
            draw_finder(
                frame,
                FinderPopup::new(name, picker, self.icons, &icon, theme)
                    .note(self.previewed_theme().and(self.colors.themes_note())),
                area,
            );
        }
        caret.map(|(_, scroll)| scroll)
    }

    /// The dialogs that take the keys, whichever is open: the finders, the Edit Node dialog and
    /// Save As over `outline`, the panes below their headers, and the settings window and the
    /// attachments panel over `body`, the screen above the status line; returns how far the text
    /// whose caret they place is scrolled.
    fn draw_dialogs(
        &mut self,
        frame: &mut Frame,
        outline: Rect,
        body: Rect,
        theme: &Theme,
    ) -> Option<usize> {
        self.draw_finders(frame, outline, theme);
        let settings = self.draw_settings(frame, body, theme);
        let node = self.draw_node_editor(frame, outline, theme);
        let attachments = self.draw_attachments(frame, body, theme);
        let save_as = self.state.save_as().map(|view| {
            let prompt = SaveAsPrompt::new(&view, theme).text_scroll(self.field_scroll);
            let (caret, scroll) = prompt.caret(outline);
            frame.set_cursor_position(caret);
            frame.render_widget(prompt, outline);
            scroll
        });
        save_as.or(attachments).or(node).or(settings)
    }

    /// The attachments panel over the right of `body`, while it is open, scrolled no further
    /// than its end, to the selected item when the selection moved and to the caret while text
    /// is typed, with the caret (attachments.md §3, §4); returns how far the typed text is
    /// scrolled.
    fn draw_attachments(&mut self, frame: &mut Frame, body: Rect, theme: &Theme) -> Option<usize> {
        let Some(view) = self.state.attachments_panel() else {
            (self.attachments_scroll, self.attachments_selected) = (0, None);
            return None;
        };
        let follow = self.attachments_selected != view.selected;
        self.attachments_selected = view.selected;
        let area = AttachmentsPanel::area(body);
        let panel = AttachmentsPanel::new(&view, self.icons, theme);
        let scrolled = panel.scroll(self.attachments_scroll, area, follow);
        (
            self.attachments_scroll,
            self.attachments_page,
            self.attachments_current,
        ) = (scrolled.scroll, scrolled.page, scrolled.current);
        let panel = panel
            .scrolled(self.attachments_scroll)
            .text_scroll(self.field_scroll);
        let caret = panel.caret(area);
        if let Some((cell, _)) = caret {
            frame.set_cursor_position(cell);
        }
        frame.render_widget(panel, area);
        caret.map(|(_, scroll)| scroll)
    }

    /// Moves through the attachments panel as `by` says at time `now`: down or up through the
    /// selected item while it goes past the panel's edge that way, else to the next or previous
    /// item, and to the top with the first item or the bottom with the last (attachments.md §3,
    /// D-109).
    fn scroll_attachments(&mut self, by: ScrollBy, now: Duration) {
        let (scroll, page) = (self.attachments_scroll, self.attachments_page);
        let current = self.attachments_current.clone().unwrap_or(0..0);
        let step = match by {
            ScrollBy::PageUp | ScrollBy::PageDown => page,
            _ => 1,
        };
        let select = match by {
            ScrollBy::Down | ScrollBy::PageDown if current.end > scroll + page => {
                self.attachments_scroll = (scroll + step).min(current.end - page);
                return;
            }
            ScrollBy::Up | ScrollBy::PageUp if current.start < scroll => {
                self.attachments_scroll = scroll.saturating_sub(step).max(current.start);
                return;
            }
            ScrollBy::Down | ScrollBy::PageDown => 1,
            ScrollBy::Up | ScrollBy::PageUp => -1,
            ScrollBy::Top => {
                self.attachments_scroll = 0;
                isize::MIN
            }
            ScrollBy::Bottom => {
                self.attachments_scroll = usize::MAX;
                isize::MAX
            }
        };
        self.send(Message::Attachments(AttachmentsInput::Select(select)), now);
    }

    /// The Edit Node dialog centered over `area`, with its caret, and its parent finder over it,
    /// while they are open (editing.md §7); returns how far the typed text is scrolled.
    fn draw_node_editor(&self, frame: &mut Frame, area: Rect, theme: &Theme) -> Option<usize> {
        let view = self.state.node_editor()?;
        let window = NodeEditorWindow::new(&view, (self.clock)().date(), self.icons, theme)
            .text_scroll(self.field_scroll);
        let caret = window.caret(area);
        if let Some((cell, _)) = caret {
            frame.set_cursor_position(cell);
        }
        frame.render_widget(window, area);
        if let Some(finder) = self.state.node_editor_finder() {
            let document = self.state.document();
            let icon = |id: &ItemId| widgets::block_icon(document, *id);
            let text = |id: &ItemId| widgets::block_text_style(document, *id, theme);
            draw_finder(
                frame,
                FinderPopup::new("Parent", &finder, self.icons, &icon, theme).text_style(&text),
                area,
            );
        }
        caret.map(|(_, scroll)| scroll)
    }

    /// The Navigator centered over `area`, scrolled to keep its focused entry visible,
    /// while it is open (ui.md §1, D-79); returns where the focused entry starts on screen, if
    /// one is focused. It is dimmed when `dim`, behind a dialog (D-69).
    fn draw_navigator(&mut self, frame: &mut Frame, area: Rect, dim: bool) -> Option<(u16, u16)> {
        self.drawn.navigator = None;
        let view = self.state.navigator()?;
        let theme = self.palette();
        let panel = NavigatorPanel::new(&view, self.icons, &theme);
        let window = panel.area(area);
        let inner = window.inner(ratatui::layout::Margin::new(1, 1));
        self.navigator_scroll = panel.scroll(self.navigator_scroll, usize::from(inner.height));
        let entry = panel.focused_line().map(|line| {
            let line =
                u16::try_from(line.saturating_sub(self.navigator_scroll)).unwrap_or(u16::MAX);
            (
                inner.x.saturating_add(widgets::GUTTER_WIDTH),
                inner.y.saturating_add(line),
            )
        });
        self.drawn.navigator = Some((inner, self.navigator_scroll));
        frame.render_widget(panel.offset(self.navigator_scroll), window);
        if dim {
            frame.buffer_mut().set_style(window, Style::new().dim());
        }
        entry
    }

    /// The date prompt's panel over `area` with its caret, if it is open (D-54); returns how far
    /// its text is scrolled. The conflict prompt covers it.
    fn draw_date_prompt(&self, frame: &mut Frame, area: ratatui::layout::Rect) -> Option<usize> {
        let prompt = self
            .state
            .date_prompt()
            .filter(|_| !self.state.in_conflict())?;
        let theme = self.palette();
        let panel = DatePromptPanel::new(&prompt, &theme).text_scroll(self.field_scroll);
        let (caret, scroll) = panel.caret(area);
        frame.set_cursor_position(caret);
        frame.render_widget(panel, area);
        Some(scroll)
    }

    /// Quick Add's panel over `panel` with its caret, while it is open (D-96); returns how far its
    /// text is scrolled. The conflict prompt covers it.
    fn draw_quick_add(&self, frame: &mut Frame, panel: Rect, theme: &Theme) -> Option<usize> {
        let view = self
            .state
            .quick_add()
            .filter(|_| !self.state.in_conflict())?;
        let widget = QuickAddPanel::new(&view, self.icons, theme).text_scroll(self.field_scroll);
        let (caret, scroll) = widget.caret(panel);
        frame.set_cursor_position(caret);
        frame.render_widget(widget, panel);
        Some(scroll)
    }

    /// The mode, the file, marked when temporary, with its save state and notice, and any pending
    /// key sequence ([ui.md §1](../../../docs/specs/ui.md#1-layout)).
    fn draw_status(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let icons = self.icons;
        let theme = self.palette();
        let (icon, mode) = match self.state.mode() {
            Mode::Navigate if self.state.visual() => (Icon::Navigate, "VISUAL"),
            Mode::Navigate => (Icon::Navigate, "NAVIGATE"),
            Mode::Edit => (Icon::Edit, "EDIT"),
        };
        let mode = format!("{} {mode}", icon.glyph(icons)).bold();
        let temporary = (self.state.origin() == Origin::Temporary).then_some(" · temporary");
        let status = |notice| {
            widgets::file_status(
                self.state.save_state(),
                self.state.access(),
                self.state.gone_from_disk(),
                self.state.confirming_quit(),
                notice,
                icons,
                &theme,
            )
        };
        let after_path = |notice| {
            temporary
                .into_iter()
                .map(Span::raw)
                .chain(status(notice))
                .collect::<Vec<_>>()
        };
        let separator = " · ";
        let path = self
            .state
            .path()
            .map(|path| effects::abbreviate_path(path, self.home.as_deref()));
        // The pending keys are what the user is typing right now, so the rest of the line gives
        // up its room to them, the path first, rather than being drawn over.
        let pending = self.keymap.pending();
        let pending = (!pending.is_empty()).then(|| Line::from(format!(" {pending}")));
        let width = usize::from(area.width).saturating_sub(pending.as_ref().map_or(0, Line::width));
        let quiet_status = Line::from_iter(after_path(None)).width();
        // The message has the room the path leaves when cut to `…` alone.
        let shortest_path = path
            .as_ref()
            .map_or(0, |_| Span::raw(separator).width() + Span::raw("…").width());
        let room = width.saturating_sub(
            mode.width() + shortest_path + quiet_status + Span::raw(separator).width(),
        );
        // A status message whose time is up is drawn while it fades out (UI-ANIM-07).
        let notice = self
            .state
            .notice()
            .or_else(|| self.animator.fading_notice(&theme))
            .map(|notice| widgets::notice_span(notice, self.home.as_deref(), room, &theme));
        // The path gives up its columns first, so the save state and messages stay in view.
        let mut spans = vec![mode];
        if let Some(path) = path {
            let room = width.saturating_sub(
                Line::from_iter(spans.iter().cloned().chain(after_path(notice.clone()))).width()
                    + Span::raw(separator).width(),
            );
            spans.push(format!("{separator}{}", widgets::fitted_path(&path, room)).into());
        }
        let before_status = Line::from_iter(spans.iter().cloned()).width();
        let quiet = Line::from_iter(spans.iter().cloned().chain(after_path(None))).width();
        spans.extend(after_path(notice));
        // A message that still does not fit ends in `…`, so it shows it is cut.
        let line = if before_status < width {
            widgets::truncate(&Line::from(spans), width)
        } else {
            Line::from(spans)
        };
        let quiet = u16::try_from(quiet).unwrap_or(u16::MAX);
        let notice = Rect {
            x: area.x.saturating_add(quiet),
            width: u16::try_from(line.width())
                .unwrap_or(u16::MAX)
                .saturating_sub(quiet),
            ..area
        }
        .intersection(area);
        frame.render_widget(line, area);
        self.animator
            .fade_status(frame.buffer_mut(), notice, &theme);
        if let Some(spinner) = self.animator.spinner(icons) {
            frame.render_widget(Line::from(format!(" {spinner}")).right_aligned(), area);
        }
        if let Some(pending) = pending {
            frame.render_widget(pending.right_aligned(), area);
        }
    }

    /// The popup over the outline that takes the keys of navigate mode, if one is open.
    fn overlay(&self) -> Option<Overlay> {
        if self.keymap_scroll.is_some() {
            Some(Overlay::Keymap)
        } else if self.state.date_prompt().is_some() {
            Some(Overlay::DatePrompt)
        } else if self.state.quick_add().is_some() {
            Some(Overlay::QuickAdd)
        } else if self.state.color_picker().is_some() {
            Some(Overlay::ColorPicker)
        } else if self.state.type_picker().is_some() {
            Some(Overlay::TypePicker)
        } else if self.state.go_to_symbol().is_some() || self.state.go_to_project().is_some() {
            Some(Overlay::GoToSymbol)
        } else if self.state.move_to().is_some() {
            Some(Overlay::MoveTo)
        } else if self.state.go_to_command().is_some() {
            Some(Overlay::GoToCommand)
        } else if self.state.theme_picker().is_some() {
            Some(Overlay::ThemePicker)
        } else if let Some(view) = self.state.settings() {
            Some(if view.picker.is_some() {
                Overlay::SettingsPicker
            } else if view.prompt.is_some()
                || view.filter.as_ref().is_some_and(|(_, typing)| *typing)
            {
                Overlay::SettingsText
            } else {
                Overlay::Settings
            })
        } else if self.state.save_as().is_some() {
            Some(Overlay::SaveAs)
        } else if self.state.context_menu().is_some() {
            Some(Overlay::ContextMenu)
        } else if self.state.node_editor_finder().is_some() {
            Some(Overlay::NodeEditorFinder)
        } else if self.state.node_editor().is_some() {
            Some(Overlay::NodeEditor)
        } else {
            self.state.attachments_panel().map(|view| {
                if view.editing.is_some() {
                    Overlay::AttachmentsEditor
                } else if view.draft.is_some() {
                    Overlay::AttachmentsComment
                } else {
                    Overlay::Attachments
                }
            })
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

/// Gives every cell of `buf` drawn in the terminal's own colors the theme's text color and
/// background instead, so that a theme with colors of its own covers the whole screen.
fn paint_defaults(buf: &mut Buffer, theme: &Theme) {
    for cell in &mut buf.content {
        if cell.fg == Color::Reset {
            cell.fg = theme.text;
        }
        if cell.bg == Color::Reset {
            cell.bg = theme.background;
        }
    }
}

/// Blanks every wide grapheme of `buf` that something drawn later covers in part, such as the
/// edge of a popup, or that runs past the right edge (N-PORT-04). Ratatui sends the terminal a
/// wide grapheme and skips the cells it covers, so it would hide what was drawn over them.
fn hide_cut_graphemes(buf: &mut Buffer) {
    let area = buf.area;
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let width = buf.cell((x, y)).map_or(1, CellWidth::cell_width);
            let end = x.saturating_add(width);
            let covered = (x.saturating_add(1)..end)
                .any(|x| buf.cell((x, y)).is_some_and(|cell| cell.symbol() != " "));
            if (end > area.right() || covered)
                && let Some(cell) = buf.cell_mut((x, y))
            {
                cell.set_symbol(" ");
            }
        }
    }
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

/// Where on screen `pane`'s header `line`, drawn in `crumbs` with `icons`, shows the place the
/// pane is: its last crumb, or a special view's name (UI-ANIM-05).
fn last_crumb(pane: &PaneView, line: &Line, crumbs: Rect, icons: IconSet) -> Rect {
    let width = u16::try_from(line.width()).unwrap_or(u16::MAX);
    let last = match pane.location {
        Location::Outline(_) => {
            let last = pane
                .breadcrumbs
                .last()
                .map_or(Icon::Home.glyph(icons), String::as_str);
            u16::try_from(Line::from(last).width()).unwrap_or(u16::MAX)
        }
        Location::View(_) => width,
    };
    Rect {
        x: crumbs.x.saturating_add(width.saturating_sub(last)),
        width: last,
        ..crumbs
    }
    .intersection(crumbs)
}

/// The screen lines in `area` of the blocks of `shown`, drawn from `line` lines into the first,
/// each as tall as `heights` says with its group's heading, which is not the block's (D-80).
fn block_lines(
    area: Rect,
    line: usize,
    shown: &[Row],
    heights: impl Iterator<Item = usize>,
) -> Vec<(ItemId, Rect)> {
    let lines = shown.iter().zip(heights).flat_map(|(row, height)| {
        let heading = widgets::heading_height(row);
        [(None, heading), (Some(row.id), height - heading)]
    });
    layout::row_lines(area, line, lines)
        .into_iter()
        .filter_map(|(id, lines)| Some((id?, lines)))
        .collect()
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
