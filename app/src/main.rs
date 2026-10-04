use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    layout::Alignment,
    style::Stylize,
    widgets::{Block, Paragraph},
};

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = run(&mut terminal);
    ratatui::restore();
    result
}

fn run(terminal: &mut DefaultTerminal) -> io::Result<()> {
    loop {
        terminal.draw(draw)?;
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
        {
            return Ok(());
        }
    }
}

fn draw(frame: &mut Frame) {
    let block = Block::bordered().title(" taskpaper-terminal ".bold());
    let text = Paragraph::new("Hello! Press q to quit.")
        .alignment(Alignment::Center)
        .block(block);
    frame.render_widget(text, frame.area());
}
