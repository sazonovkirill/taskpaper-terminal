//! Starting the app on fixtures and driving it with keys.

use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    time::Duration,
};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use jiff::civil::{Date, date};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
use taskpaper_terminal::{
    app::State,
    ui::{
        Input, Tui,
        effects::{self, Backups, Io},
    },
};
use tempfile::TempDir;

/// The local date every test runs on.
pub const TODAY: Date = date(2026, 10, 5);

/// A running app with its terminal, the temporary folder holding its file and its backups
/// folder, and a clock that only moves when the test says so.
pub struct Harness {
    pub tui: Tui,
    pub terminal: Terminal<TestBackend>,
    pub path: Option<PathBuf>,
    pub backups: PathBuf,
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

    /// Starts without a path, on the demo outline.
    pub fn demo(width: u16, height: u16) -> Self {
        Self::start(
            State::demo(),
            tempfile::tempdir().expect("a temporary folder"),
            width,
            height,
        )
    }

    /// Starts the app on `state` with its backups in `dir/backups`.
    fn start(state: State, dir: TempDir, width: u16, height: u16) -> Self {
        let backups = dir.path().join("backups");
        let (results, inputs) = mpsc::channel();
        let io = Io::start(results, Backups::new(Some(backups.clone()), || TODAY));
        Self {
            path: state.path().map(Path::to_owned),
            tui: Tui::new(state, io),
            terminal: Terminal::new(TestBackend::new(width, height)).expect("a test terminal"),
            backups,
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

    /// Presses each character of `keys` without modifiers.
    pub fn type_keys(&mut self, keys: &str) {
        for c in keys.chars() {
            self.press(KeyCode::Char(c), KeyModifiers::NONE);
        }
    }

    /// Lets `duration` pass without input.
    pub fn wait(&mut self, duration: Duration) {
        self.now += duration;
        self.tui.tick(self.now);
    }

    /// The status line for the opened file: the mode and the file's path.
    pub fn status(&self, width: u16) -> String {
        let path = self.path.as_ref().expect("a file was opened");
        let status = format!("NAVIGATE · {}", path.display());
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

/// Whether row `y` carries the focus marker in the gutter.
pub fn is_focused(buffer: &Buffer, y: u16) -> bool {
    buffer[(buffer.area.left(), y)].symbol() == "▎"
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
