//! A terminal that goes away while the app runs.

use std::{io, time::Duration};

use ratatui::{
    Terminal,
    backend::{Backend, ClearType, TestBackend, WindowSize},
    buffer::{Buffer, Cell},
    layout::{Position, Rect, Size},
};
use taskpaper_terminal::{app::Message, ui::Input};

use crate::support::Harness;

/// A terminal that has hung up: it still reports its size, but every write fails.
struct HungUp;

fn gone<T>() -> io::Result<T> {
    Err(io::ErrorKind::BrokenPipe.into())
}

impl Backend for HungUp {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, _content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        gone()
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        gone()
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        gone()
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        gone()
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, _position: P) -> io::Result<()> {
        gone()
    }

    fn clear(&mut self) -> io::Result<()> {
        gone()
    }

    fn clear_region(&mut self, _clear_type: ClearType) -> io::Result<()> {
        gone()
    }

    fn size(&self) -> io::Result<Size> {
        Ok(Size::new(40, 6))
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        gone()
    }

    fn flush(&mut self) -> io::Result<()> {
        gone()
    }
}

// F-FILE-05
#[test]
fn a_terminal_that_cannot_be_drawn_on_ends_the_session_as_a_hang_up_does() {
    let mut app = Harness::open("groceries.taskpaper", 40, 6);
    let mut terminal = Terminal::new(HungUp).unwrap();
    app.tui.render(&mut terminal, Duration::ZERO);
    assert!(app.tui.should_quit());
}

// F-FILE-05
#[test]
fn once_told_to_end_the_app_no_longer_draws() {
    let mut app = Harness::open("groceries.taskpaper", 40, 6);
    app.send(Input::Core(Message::Terminate));
    let mut terminal = Terminal::new(TestBackend::new(40, 6)).unwrap();
    app.tui.render(&mut terminal, Duration::ZERO);
    terminal
        .backend()
        .assert_buffer(&Buffer::empty(Rect::new(0, 0, 40, 6)));
}

#[test]
fn renders_the_screen_on_a_terminal() {
    let mut app = Harness::open("groceries.taskpaper", 40, 6);
    let mut terminal = Terminal::new(TestBackend::new(40, 6)).unwrap();
    app.tui.render(&mut terminal, Duration::ZERO);
    assert_eq!(terminal.backend().buffer(), app.render().buffer());
    assert!(!app.tui.should_quit());
}
