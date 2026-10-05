//! `tpt [path]`: opens a `.taskpaper` file, or a new temporary file holding the demo outline
//! without a path, in the terminal.

use std::{
    io,
    panic::{self, AssertUnwindSafe},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
    thread,
    time::Instant,
};

use anyhow::{Context, bail};
use crossterm::{
    event::{
        self, DisableFocusChange, EnableFocusChange, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
};
use signal_hook::{
    consts::{SIGHUP, SIGTERM},
    iterator::Signals,
};
use taskpaper_terminal::{
    app::Message,
    ui::{
        self, Input,
        effects::{Backups, Io, Recovery},
    },
};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().map(PathBuf::from);
    if args.next().is_some() {
        bail!("usage: tpt [path]");
    }
    let home = std::env::home_dir();
    let recovery = Recovery::new(
        ui::effects::recovery_folder(std::env::var_os("XDG_STATE_HOME"), home.clone()),
        || jiff::Zoned::now().datetime(),
    );
    let backups = Backups::new(
        ui::effects::backups_folder(std::env::var_os("XDG_DATA_HOME"), home),
        || jiff::Zoned::now().datetime(),
    );
    let state = match path {
        Some(path) => {
            // Opening first reports what is wrong with the path before asking anything.
            let opened = ui::effects::open(&path)?;
            match recovery
                .offer(&path, &mut io::stdin().lock(), &mut io::stderr())
                .context("cannot look for recovery files")?
            {
                Some(recovered) => ui::effects::recover(&path, &recovered, &backups)?,
                None => opened,
            }
        }
        None => ui::effects::create_demo(
            Path::new(ui::effects::TEMPORARY_FOLDER),
            jiff::Zoned::now().datetime(),
        )?,
    };
    let (inputs, received) = mpsc::channel();
    forward_signals(inputs.clone()).context("cannot listen for signals")?;
    let today = || jiff::Zoned::now().date();
    let io = Io::start(inputs.clone(), backups);
    // Without a watcher, the checks on regaining focus and before every save still find changes.
    let _watcher = state
        .path()
        .and_then(|path| ui::effects::watch(path, inputs.clone()).ok());
    let mut tui = ui::Tui::new(state, io, today);
    let mut terminal = ratatui::try_init().context("cannot set up the terminal")?;
    // Terminals that cannot report focus or the kitty keyboard protocol ignore the requests, so
    // keys arrive as legacy reporting sends them (keybindings.md §1).
    let _ = execute!(
        std::io::stdout(),
        EnableFocusChange,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    );
    forward_terminal_events(inputs);
    // The panic hook `try_init` installed restores the terminal first; once the panic has
    // unwound to here, the document is written out as it stands.
    let result = panic::catch_unwind(AssertUnwindSafe(|| run(&mut terminal, &mut tui, &received)));
    let _ = execute!(
        std::io::stdout(),
        PopKeyboardEnhancementFlags,
        DisableFocusChange
    );
    ratatui::restore();
    result.unwrap_or_else(|panic| {
        report_recovery(&tui, &recovery);
        panic::resume_unwind(panic)
    })
}

/// After a crash, writes the unsaved document to a recovery file and says where (N-REL-01).
fn report_recovery(tui: &ui::Tui, recovery: &Recovery) {
    match tui.recover(recovery) {
        Ok(Some(path)) => eprintln!("tpt: the unsaved changes are in {}", path.display()),
        Ok(None) => {}
        Err(error) => {
            eprintln!("tpt: cannot write the unsaved changes to a recovery file: {error}");
        }
    }
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
