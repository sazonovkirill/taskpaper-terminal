//! `tpt [path]`: opens a `.taskpaper` file, or the demo outline without a path, in the terminal.

use std::{path::PathBuf, time::Instant};

use anyhow::{Context, bail};
use crossterm::event;
use taskpaper_terminal::{app::State, ui};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().map(PathBuf::from);
    if args.next().is_some() {
        bail!("usage: tpt [path]");
    }
    let state = match path {
        Some(path) => State::new(ui::effects::read_document(&path)?, Some(path)),
        None => State::demo(),
    };
    let mut tui = ui::Tui::new(state);
    let mut terminal = ratatui::try_init().context("cannot set up the terminal")?;
    let result = run(&mut terminal, &mut tui);
    ratatui::restore();
    result
}

#[expect(
    clippy::disallowed_methods,
    reason = "the event loop is the edge that reads the clock and passes time to the frontend"
)]
fn run(terminal: &mut ratatui::DefaultTerminal, tui: &mut ui::Tui) -> anyhow::Result<()> {
    let start = Instant::now();
    while !tui.should_quit() {
        terminal.draw(|frame| tui.draw(frame))?;
        match tui.deadline() {
            Some(deadline) if !event::poll(deadline.saturating_sub(start.elapsed()))? => {
                tui.tick(start.elapsed());
            }
            _ => tui.handle_event(&event::read()?, start.elapsed()),
        }
    }
    Ok(())
}
