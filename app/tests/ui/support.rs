//! Starting the app on fixtures and driving it with keys.

use std::path::{Path, PathBuf};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use taskpaper_terminal::{
    app::State,
    ui::{Tui, effects::read_document},
};
use tempfile::TempDir;

/// A running app with its terminal and the temporary folder holding its file.
pub struct Harness {
    pub tui: Tui,
    pub terminal: Terminal<TestBackend>,
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

    /// Starts without a path, on the demo outline.
    pub fn demo(width: u16, height: u16) -> Self {
        Self::start(State::demo(), None, width, height)
    }

    fn start(state: State, dir: Option<TempDir>, width: u16, height: u16) -> Self {
        Self {
            tui: Tui::new(state),
            terminal: Terminal::new(TestBackend::new(width, height)).expect("a test terminal"),
            _dir: dir,
        }
    }

    /// Presses `code` with `modifiers`.
    pub fn press(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        self.tui
            .handle_event(&Event::Key(KeyEvent::new(code, modifiers)));
    }

    /// Draws a frame and returns the backend to check.
    pub fn render(&mut self) -> &TestBackend {
        self.terminal
            .draw(|frame| self.tui.draw(frame))
            .expect("drawing to a test backend");
        self.terminal.backend()
    }
}
