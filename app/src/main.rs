//! `tpt [path]`: opens a `.taskpaper` file, or the demo outline without a path, in the terminal.

use std::{
    path::PathBuf,
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
    app::{Message, State},
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
        None => State::demo(),
    };
    let (inputs, received) = mpsc::channel();
    forward_signals(inputs.clone()).context("cannot listen for signals")?;
    let folder =
        ui::effects::backups_folder(std::env::var_os("XDG_DATA_HOME"), std::env::home_dir());
    let io = Io::start(
        inputs.clone(),
        Backups::new(folder, || jiff::Zoned::now().date()),
    );
    let mut tui = ui::Tui::new(state, io);
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
    // Once told to end, the terminal may be gone, so the loop only finishes saving.
    let mut ending = false;
    while !tui.should_quit() {
        if !ending {
            terminal.draw(|frame| tui.draw(frame))?;
        }
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
            Some(input) => {
                if input == Input::Core(Message::Terminate) {
                    ending = true;
                }
                tui.handle(input, start.elapsed());
            }
            None => tui.tick(start.elapsed()),
        }
    }
    Ok(())
}
