//! Starting the app on fixtures and driving it with keys.

use std::{
    cell::RefCell,
    io::{self, Write},
    path::{Path, PathBuf},
    rc::Rc,
    sync::mpsc::{self, Receiver},
    time::Duration,
};

use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use jiff::{
    Zoned,
    civil::{Date, DateTime, date},
    tz::TimeZone,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    style::{Color, Modifier},
};
use taskpaper_terminal::{
    app::{Origin, State},
    ui::{
        ColorSupport, Input, Tui,
        config::{self, Config},
        effects::{self, Backups, Browser, Clipboard, FileWatch, Io, Views},
        theme::IconSetting,
    },
};
use tempfile::TempDir;

/// The status line's mode with icons off (ui.md §7).
pub const NAVIGATE: &str = "↕ NAVIGATE";
pub const EDIT: &str = "✎ EDIT";

/// The local date every test runs on.
pub const TODAY: Date = date(2026, 10, 5);

/// The local time every test runs at.
pub const NOW: DateTime = TODAY.at(9, 30, 0, 0);

thread_local! {
    /// The local time the app reads, [`NOW`] in UTC when a harness starts and moved by
    /// [`Harness::wait`]. Each test runs on a thread of its own, so no two tests share it.
    static CLOCK: RefCell<Zoned> = RefCell::new(local(NOW));
}

/// `time` as the local time, in UTC so that no test depends on the machine's time zone.
fn local(time: DateTime) -> Zoned {
    time.to_zoned(TimeZone::UTC)
        .expect("a date jiff can place in UTC")
}

/// Bytes written to the terminal, shared with the test.
#[derive(Debug, Clone, Default)]
pub struct Output(Rc<RefCell<Vec<u8>>>);

impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Output {
    /// The text of each OSC 52 sequence written so far, decoded: what was put on the
    /// terminal's clipboard.
    pub fn copied(&self) -> Vec<String> {
        let written = String::from_utf8(self.0.borrow().clone()).expect("UTF-8 output");
        written
            .split("\x1b]52;c;")
            .skip(1)
            .map(|rest| {
                let encoded = rest.split('\x1b').next().expect("a terminated sequence");
                String::from_utf8(base64_decode(encoded)).expect("UTF-8 text")
            })
            .collect()
    }
}

/// Decodes standard Base64 with padding.
fn base64_decode(encoded: &str) -> Vec<u8> {
    const DIGITS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bits = 0u32;
    let mut count = 0;
    let mut bytes = Vec::new();
    for c in encoded.chars().filter(|&c| c != '=') {
        bits =
            bits << 6 | u32::try_from(DIGITS.find(c).expect("a Base64 digit")).expect("below 64");
        count += 6;
        if count >= 8 {
            count -= 8;
            bytes.push(u8::try_from(bits >> count & 0xFF).expect("masked to a byte"));
        }
    }
    bytes
}

/// The program the app opens URLs with: one that does not exist, so that the error it shows
/// names the URL `gx` chose.
pub const OPENER: &str = "tpt-test-no-such-opener";

/// The icons of a start, a due and an overdue date chip (ui.md §7, D-46), as drawn with icons
/// off, as the tests run unless they turn them on.
pub const START: &str = "↦";
pub const DUE: &str = "⇥";
pub const OVERDUE: &str = "!";

/// The icons after the text of a block with `@star` and of a bookmarked one (D-47), as drawn
/// with icons off.
pub const STAR: &str = "★";
pub const BOOKMARK: &str = "⚑";

/// The Today view's sun (D-88), as drawn with icons off.
pub const SUN: &str = "☼";

/// The flag after the text of a block with `@flag` (D-91), as drawn with icons off.
pub const FLAG: &str = "⚐";

/// The Colors view's icon (ui.md §7), as drawn with icons off.
pub const PALETTE: &str = "◐";

/// The color of text blocks, dimmed less than done tasks (D-37), in Terminal Dark, the theme of a
/// terminal without truecolor.
pub const TEXT: Color = Color::Indexed(248);

/// The background of every line of the focused block (D-39), in Terminal Dark.
pub const CURRENT_LINE: Color = Color::Black;

/// A running app with its terminal, the temporary folder holding its file, its backups folder
/// and its `views.json`, and a clock that only moves when the test says so.
pub struct Harness {
    pub tui: Tui,
    pub terminal: Terminal<TestBackend>,
    /// What the app wrote to the terminal besides drawing: the clipboard's OSC 52, as no system
    /// clipboard is reachable.
    pub terminal_output: Output,
    pub path: Option<PathBuf>,
    /// Whether the file is the temporary one a start without a path makes.
    temporary: bool,
    pub backups: PathBuf,
    /// Where the app keeps the layouts of the files it opens.
    pub views: PathBuf,
    /// The home folder `~` stands for, empty at the start.
    pub home: PathBuf,
    inputs: Receiver<Input>,
    now: Duration,
    _dir: TempDir,
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

impl Harness {
    /// Copies the fixture `name` into a temporary folder and opens it in a `width`×`height` terminal.
    pub fn open(name: &str, width: u16, height: u16) -> Self {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let path = dir.path().join(name);
        std::fs::copy(fixture(name), &path).expect("the fixture exists");
        Self::open_path(dir, &path, width, height)
    }

    /// Starts on `text` as a file in a temporary folder.
    pub fn with_text(text: &str, width: u16, height: u16) -> Self {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let path = dir.path().join("outline.taskpaper");
        std::fs::write(&path, text).expect("the temporary folder is writable");
        Self::open_path(dir, &path, width, height)
    }

    /// Opens `path`, a file in `dir` that the test prepared.
    pub fn open_path(dir: TempDir, path: &Path, width: u16, height: u16) -> Self {
        let state = effects::open(path).expect("the file opens");
        Self::start(state, dir, width, height)
    }

    /// Starts on `text` as a file in a temporary folder, with `settings` as the settings file.
    pub fn with_config(text: &str, settings: &str, width: u16, height: u16) -> Self {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let path = dir.path().join("outline.taskpaper");
        std::fs::write(&path, text).expect("the temporary folder is writable");
        let file = dir.path().join("config.toml");
        std::fs::write(&file, settings).expect("the temporary folder is writable");
        let state = effects::open(&path).expect("the file opens");
        let config = config::load(&file, Some(&dir.path().join("home")));
        Self::start_with(state, dir, config, width, height)
    }

    /// The same app drawing in the colors `colors` allows, as `COLORTERM` and `NO_COLOR` would
    /// say.
    pub fn with_colors(self, colors: ColorSupport) -> Self {
        Self {
            tui: self.tui.with_colors(colors),
            ..self
        }
    }

    /// The settings file [`Harness::with_config`] starts with.
    pub fn config_file(&self) -> PathBuf {
        self.path
            .as_ref()
            .expect("a file was opened")
            .with_file_name("config.toml")
    }

    /// Starts on `text` as a file in a temporary folder, opened with `recovered`, the text of a
    /// recovery file of it.
    pub fn recovered(text: &str, recovered: &str, width: u16, height: u16) -> Self {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let path = dir.path().join("outline.taskpaper");
        std::fs::write(&path, text).expect("the temporary folder is writable");
        let recovery = dir.path().join("recovery.taskpaper");
        std::fs::write(&recovery, recovered).expect("the temporary folder is writable");
        let backups = Backups::new(Some(dir.path().join("backups")), || NOW);
        let state = effects::recover(&path, &recovery, &backups).expect("the files open");
        Self::start(state, dir, width, height)
    }

    /// Starts without a path at 12:30 on [`TODAY`]: on the demo outline, written to a new file
    /// in a temporary folder that stands for `/tmp`.
    pub fn demo(width: u16, height: u16) -> Self {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let state = effects::create_demo(dir.path(), TODAY.at(12, 30, 0, 0))
            .expect("the temporary folder is writable");
        Self::start(state, dir, width, height)
    }

    /// Quits, then starts the app again on the same file in a `width`×`height` terminal, as a
    /// new launch would.
    pub fn reopen(mut self, width: u16, height: u16) -> Self {
        self.press(KeyCode::Char('q'), KeyModifiers::CONTROL);
        self.settle();
        assert!(self.tui.should_quit(), "the app quits");
        let path = self.path.clone().expect("a file was opened");
        let Self { tui, _dir: dir, .. } = self;
        // Dropping the app waits for the disk work it asked for.
        drop(tui);
        Self::open_path(dir, &path, width, height)
    }

    /// Starts the app on `state` with its backups in `dir/backups` and its layouts in
    /// `dir/state/views.json`.
    fn start(state: State, dir: TempDir, width: u16, height: u16) -> Self {
        Self::start_with(state, dir, Config::default(), width, height)
    }

    /// Starts the app as [`Harness::start`] does, with the settings `config`. Icons are off unless
    /// `config` turns them on, so screens read as the Unicode markers of ui.md §2.
    fn start_with(state: State, dir: TempDir, config: Config, width: u16, height: u16) -> Self {
        let backups = dir.path().join("backups");
        let views = dir.path().join("state/views.json");
        let state = Views::new(Some(views.clone())).restore(state);
        let home = dir.path().join("home");
        std::fs::create_dir_all(&home).expect("the temporary folder is writable");
        CLOCK.set(local(NOW));
        let (results, inputs) = mpsc::channel();
        let io = Io::start(
            results,
            Backups::new(Some(backups.clone()), || NOW).with_settings(config.backups()),
            Views::new(Some(views.clone())),
        );
        let terminal_output = Output::default();
        Self {
            path: state.path().map(Path::to_owned),
            temporary: state.origin() == Origin::Temporary,
            tui: Tui::new(
                state,
                io,
                FileWatch::off(),
                Browser::new(OPENER),
                Clipboard::new(None, terminal_output.clone()),
                || CLOCK.with_borrow(Zoned::clone),
                Some(home.clone()),
            )
            .with_icons(IconSetting::Off)
            .with_config(config),
            home,
            terminal_output,
            terminal: Terminal::new(TestBackend::new(width, height)).expect("a test terminal"),
            backups,
            views,
            inputs,
            now: Duration::ZERO,
            _dir: dir,
        }
    }

    /// Waits for the disk work asked for so far and hands its results to the app, until no more
    /// work follows.
    pub fn settle(&mut self) {
        loop {
            self.tui.flush();
            let inputs: Vec<Input> = self.inputs.try_iter().collect();
            if inputs.is_empty() {
                return;
            }
            for input in inputs {
                self.tui.handle(input, self.now);
            }
        }
    }

    /// Hands `input` to the app, as the event loop does with signals and results.
    pub fn send(&mut self, input: Input) {
        self.tui.handle(input, self.now);
    }

    /// Presses `code` with `modifiers`.
    pub fn press(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        let key = Event::Key(KeyEvent::new(code, modifiers));
        self.tui.handle(Input::Terminal(key), self.now);
    }

    /// Draws a frame, as the event loop does before every input, then does `kind` with the mouse
    /// at column `x` of row `y` with `modifiers` held.
    pub fn mouse(&mut self, kind: MouseEventKind, x: u16, y: u16, modifiers: KeyModifiers) {
        self.render();
        let event = Event::Mouse(MouseEvent {
            kind,
            column: x,
            row: y,
            modifiers,
        });
        self.tui.handle(Input::Terminal(event), self.now);
    }

    /// Clicks at column `x` of row `y`.
    pub fn click(&mut self, x: u16, y: u16) {
        self.mouse(
            MouseEventKind::Down(MouseButton::Left),
            x,
            y,
            KeyModifiers::NONE,
        );
    }

    /// Presses each character of `keys` without modifiers.
    pub fn type_keys(&mut self, keys: &str) {
        for c in keys.chars() {
            self.press(KeyCode::Char(c), KeyModifiers::NONE);
        }
    }

    /// The app on a terminal whose locale is not UTF-8.
    pub fn without_utf8(mut self) -> Self {
        self.tui = self.tui.with_locale(false);
        self
    }

    /// Lets `duration` pass without input.
    pub fn wait(&mut self, duration: Duration) {
        CLOCK.with_borrow_mut(|clock| {
            *clock = clock.checked_add(duration).expect("a time jiff can hold");
        });
        self.now += duration;
        self.tui.tick(self.now);
    }

    /// The status line for the opened file: the mode, the file's path and whether it is
    /// temporary.
    pub fn status(&self, width: u16) -> String {
        let path = self.path.as_ref().expect("a file was opened");
        let mark = if self.temporary { " · temporary" } else { "" };
        let status = format!("{NAVIGATE} · {}{mark}", path.display());
        format!("{status:width$.width$}", width = usize::from(width))
    }

    /// Draws a frame and returns the backend to check.
    pub fn render(&mut self) -> &TestBackend {
        self.terminal
            .draw(|frame| self.tui.draw(frame))
            .expect("drawing to a test backend");
        self.terminal.backend()
    }
}

/// The text of row `y`, without trailing blanks.
pub fn line(buffer: &Buffer, y: u16) -> String {
    let area = buffer.area;
    (area.left()..area.right())
        .map(|x| buffer[(x, y)].symbol())
        .collect::<String>()
        .trim_end()
        .to_owned()
}

/// The columns left of the outline that hold the focus marker.
const GUTTER: u16 = 2;

/// The text of outline row `y` after the gutter, without trailing blanks.
pub fn outline_line(buffer: &Buffer, y: u16) -> String {
    let area = buffer.area;
    (area.left() + GUTTER..area.right())
        .map(|x| buffer[(x, y)].symbol())
        .collect::<String>()
        .trim_end()
        .to_owned()
}

/// Whether row `y` carries the focus marker in the gutter, which a selected row carries dimmed.
pub fn is_focused(buffer: &Buffer, y: u16) -> bool {
    let cell = &buffer[(buffer.area.left(), y)];
    cell.symbol() == "▌" && !cell.modifier.contains(Modifier::DIM)
}

/// The text of every row marked as selected but not focused, after the gutter.
pub fn selected_rows(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    let area = buffer.area;
    (area.top()..area.bottom())
        .filter(|&y| {
            let cell = &buffer[(area.left(), y)];
            cell.symbol() == "▌" && cell.modifier.contains(Modifier::DIM)
        })
        .map(|y| outline_line(&buffer, y))
        .collect()
}

/// The text of every row marked as focused, after the gutter.
pub fn focused_rows(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    let area = buffer.area;
    (area.top()..area.bottom())
        .filter(|&y| is_focused(&buffer, y))
        .map(|y| outline_line(&buffer, y))
        .collect()
}

/// The text of the screen's rows between the breadcrumb header with its rule and the status
/// line, after the gutter.
pub fn outline_rows(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    let area = buffer.area;
    (area.top() + 2..area.bottom() - 1)
        .map(|y| outline_line(&buffer, y))
        .collect()
}
