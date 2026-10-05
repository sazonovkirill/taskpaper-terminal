//! `tpt [path]`: opens a `.taskpaper` file, or a new temporary file holding the demo outline
//! without a path, in the terminal.

use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
    thread,
    time::Instant,
};

use anyhow::{Context, bail};
use crossterm::event;
use signal_hook::{
    consts::{SIGHUP, SIGTERM},
    iterator::Signals,
};
use taskpaper_terminal::{
    app::Message,
    ui::{
        self, Input,
        effects::{Backups, Io},
    },
};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().map(PathBuf::from);
    if args.next().is_some() {
        bail!("usage: tpt [path]");
    }
    let state = match path {
        Some(path) => ui::effects::open(&path)?,
        None => ui::effects::create_demo(
            Path::new(ui::effects::TEMPORARY_FOLDER),
            jiff::Zoned::now().datetime(),
        )?,
    };
    let (inputs, received) = mpsc::channel();
    forward_signals(inputs.clone()).context("cannot listen for signals")?;
    let folder =
        ui::effects::backups_folder(std::env::var_os("XDG_DATA_HOME"), std::env::home_dir());
    let today = || jiff::Zoned::now().date();
    let io = Io::start(inputs.clone(), Backups::new(folder, today));
    let mut tui = ui::Tui::new(state, io, today);
    let mut terminal = ratatui::try_init().context("cannot set up the terminal")?;
    forward_terminal_events(inputs);
    let result = run(&mut terminal, &mut tui, &received);
    ratatui::restore();
    result
}

/// Turns `SIGTERM` and `SIGHUP` into [`Message::Terminate`], so the document is saved first.
fn forward_signals(inputs: Sender<Input>) -> std::io::Result<()> {
    let mut signals = Signals::new([SIGTERM, SIGHUP])?;
    thread::spawn(move || {
        for _ in signals.forever() {
            if inputs.send(Input::Core(Message::Terminate)).is_err() {
                break;
            }
        }
    });
    Ok(())
}

/// Reads terminal events on a thread of their own. When reading fails the terminal is gone, as
/// after `SIGHUP`, so the session ends the same way.
fn forward_terminal_events(inputs: Sender<Input>) {
    thread::spawn(move || {
        while let Ok(event) = event::read() {
            if inputs.send(Input::Terminal(event)).is_err() {
                return;
            }
        }
        // A closed channel means the event loop has ended already.
        let _ = inputs.send(Input::Core(Message::Terminate));
    });
}

#[expect(
    clippy::disallowed_methods,
    reason = "the event loop is the edge that reads the clock and passes time to the frontend"
)]
fn run(
    terminal: &mut ratatui::DefaultTerminal,
    tui: &mut ui::Tui,
    received: &Receiver<Input>,
) -> anyhow::Result<()> {
    let start = Instant::now();
    while !tui.should_quit() {
        tui.render(terminal, start.elapsed());
        let input = match tui.deadline() {
            None => Some(received.recv()?),
            Some(deadline) => {
                match received.recv_timeout(deadline.saturating_sub(start.elapsed())) {
                    Ok(input) => Some(input),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(error) => return Err(error.into()),
                }
            }
        };
        match input {
            Some(input) => tui.handle(input, start.elapsed()),
            None => tui.tick(start.elapsed()),
        }
    }
    Ok(())
}
