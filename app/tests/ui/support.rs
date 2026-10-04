//! Starting the app on fixtures and driving it with keys.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, style::Modifier};
use taskpaper_terminal::{
    app::State,
    ui::{Tui, effects::read_document},
};
use tempfile::TempDir;

/// A running app with its terminal, the temporary folder holding its file and a clock that only
/// moves when the test says so.
pub struct Harness {
    pub tui: Tui,
    pub terminal: Terminal<TestBackend>,
    pub path: Option<PathBuf>,
    now: Duration,
    _dir: Option<TempDir>,
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
        let document = read_document(&path).expect("the fixture opens");
        Self::start(State::new(document, Some(path)), Some(dir), width, height)
    }

    /// Starts on `text` as a file in a temporary folder.
    pub fn with_text(text: &str, width: u16, height: u16) -> Self {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let path = dir.path().join("outline.taskpaper");
        std::fs::write(&path, text).expect("the temporary folder is writable");
        let document = read_document(&path).expect("the file opens");
        Self::start(State::new(document, Some(path)), Some(dir), width, height)
    }

    /// Starts without a path, on the demo outline.
    pub fn demo(width: u16, height: u16) -> Self {
        Self::start(State::demo(), None, width, height)
    }

    fn start(state: State, dir: Option<TempDir>, width: u16, height: u16) -> Self {
        Self {
            path: state.path().map(Path::to_owned),
            tui: Tui::new(state),
            terminal: Terminal::new(TestBackend::new(width, height)).expect("a test terminal"),
            now: Duration::ZERO,
            _dir: dir,
        }
    }

    /// Presses `code` with `modifiers`.
    pub fn press(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        self.tui
            .handle_event(&Event::Key(KeyEvent::new(code, modifiers)), self.now);
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

/// The text of every highlighted row on screen.
pub fn focused_rows(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    let area = buffer.area;
    (area.top()..area.bottom())
        .filter(|&y| {
            buffer[(area.left(), y)]
                .modifier
                .contains(Modifier::REVERSED)
        })
        .map(|y| line(&buffer, y))
        .collect()
}

/// The text of the screen's rows between the breadcrumb header with its rule and the status
/// line.
pub fn outline_rows(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    let area = buffer.area;
    (area.top() + 2..area.bottom() - 1)
        .map(|y| line(&buffer, y))
        .collect()
}
