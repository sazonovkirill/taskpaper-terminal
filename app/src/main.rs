//! `tpt [path]`: opens a `.taskpaper` file, or a new temporary file holding the demo outline
//! without a path, in the terminal. `tpt --version` and `tpt --help` print and exit.

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
        self, DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
        EnableFocusChange, EnableMouseCapture, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute, queue,
    terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate},
};
use signal_hook::{
    consts::{SIGHUP, SIGTERM},
    iterator::Signals,
};
use taskpaper_terminal::{
    app::Message,
    ui::{
        self, Input,
        effects::{Backups, Browser, Clipboard, FileWatch, Io, Recovery, SystemClipboard, Views},
    },
};

/// What `tpt --help` prints.
const HELP: &str = "\
usage: tpt [path]
       tpt --help | --version

Opens the TaskPaper file at path, creating it on the first save, or without a path a new
temporary file holding a demo outline. Open a file whose name starts with - as ./-name.";

/// What the command line asks for.
enum Arguments {
    /// Open the file at this path, or the demo without one.
    Open(Option<PathBuf>),
    /// Nothing more: the version or the usage was printed.
    Printed,
}

/// Reads the command line: `tpt [path]`, `tpt --help` or `tpt --version`.
fn arguments() -> anyhow::Result<Arguments> {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().map(PathBuf::from);
    if args.next().is_some() {
        bail!("usage: tpt [path]");
    }
    // A flag is never taken for a file name, so a mistyped one cannot create a file (D-113).
    if let Some(flag) = path.as_deref().and_then(Path::to_str)
        && flag.starts_with('-')
    {
        match flag {
            "--version" => println!("tpt {}", env!("CARGO_PKG_VERSION")),
            "--help" => println!("{HELP}"),
            _ => bail!("unknown option {flag}; open a file named like it as ./{flag}\n{HELP}"),
        }
        return Ok(Arguments::Printed);
    }
    Ok(Arguments::Open(path))
}

fn main() -> anyhow::Result<()> {
    let Arguments::Open(path) = arguments()? else {
        return Ok(());
    };
    let home = std::env::home_dir();
    let config = ui::config::file(std::env::var_os("XDG_CONFIG_HOME"), home.clone())
        .map(|file| ui::config::load(&file, home.as_deref()))
        .unwrap_or_default();
    let recovery = Recovery::new(
        ui::effects::recovery_folder(std::env::var_os("XDG_STATE_HOME"), home.clone()),
        || jiff::Zoned::now().datetime(),
    );
    let backups = Backups::new(
        ui::effects::backups_folder(std::env::var_os("XDG_DATA_HOME"), home.clone()),
        || jiff::Zoned::now().datetime(),
    )
    .with_settings(config.backups());
    let views = Views::new(ui::effects::views_file(
        std::env::var_os("XDG_STATE_HOME"),
        home.clone(),
    ));
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
    let state = views.restore(state);
    let (inputs, received) = mpsc::channel();
    forward_signals(inputs.clone()).context("cannot listen for signals")?;
    let io = Io::start(inputs.clone(), backups, views);
    let files = FileWatch::new(inputs.clone());
    // Without a display server, as over SSH, there is no system clipboard to reach.
    let system = arboard::Clipboard::new()
        .ok()
        .map(|clipboard| Box::new(Arboard(clipboard)) as Box<dyn SystemClipboard>);
    let clipboard = Clipboard::new(system, io::stdout());
    let tui = ui::Tui::new(
        state,
        io,
        files,
        Browser::system(),
        clipboard,
        jiff::Zoned::now,
        home,
    );
    let mut terminal = ratatui::try_init().context("cannot set up the terminal")?;
    // Asked before the event thread starts, which would otherwise read the terminal's answer; a
    // terminal that does not answer is taken not to speak the protocol.
    let kitty = crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false);
    let utf8 = ui::theme::utf8_locale(
        std::env::var_os("LC_ALL").as_deref(),
        std::env::var_os("LC_CTYPE").as_deref(),
        std::env::var_os("LANG").as_deref(),
    );
    let colors = ui::ColorSupport::detect(
        std::env::var_os("NO_COLOR").as_deref(),
        std::env::var_os("COLORTERM").as_deref(),
    );
    let animations = ui::animation::default_for(
        std::env::var_os("SSH_CONNECTION").as_deref(),
        std::env::var_os("SSH_TTY").as_deref(),
    );
    let mut tui = tui
        .with_kitty_keyboard(kitty)
        .with_locale(utf8)
        .with_colors(colors)
        .with_animations(animations)
        .with_config(config);
    // Terminals that cannot report focus or the mouse, bracket pasted text or speak the kitty
    // keyboard protocol ignore the requests, so keys arrive as legacy reporting sends them
    // (keybindings.md §1) and nothing depends on the mouse (D-16).
    let _ = execute!(
        std::io::stdout(),
        EnableFocusChange,
        EnableMouseCapture,
        EnableBracketedPaste,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    );
    forward_terminal_events(inputs);
    // The panic hook `try_init` installed restores the terminal first; once the panic has
    // unwound to here, the document is written out as it stands.
    let result = panic::catch_unwind(AssertUnwindSafe(|| run(&mut terminal, &mut tui, &received)));
    let _ = execute!(
        std::io::stdout(),
        PopKeyboardEnhancementFlags,
        DisableBracketedPaste,
        DisableMouseCapture,
        DisableFocusChange
    );
    ratatui::restore();
    result.unwrap_or_else(|panic| {
        report_recovery(&tui, &recovery);
        panic::resume_unwind(panic)
    })
}

/// The system clipboard through `arboard`.
struct Arboard(arboard::Clipboard);

impl SystemClipboard for Arboard {
    fn set(&mut self, text: &str) -> bool {
        self.0.set_text(text).is_ok()
    }

    fn get(&mut self) -> Option<String> {
        self.0.get_text().ok()
    }
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
            // The terminal reports every move of the pointer; each would otherwise draw a frame.
            if !ui::acts_on(&event) {
                continue;
            }
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
        // The terminal shows each frame whole (ui.md §9.1). A terminal that is gone is noticed by
        // drawing, which ends the session saving first.
        let _ = queue!(terminal.backend_mut(), BeginSynchronizedUpdate);
        tui.render(terminal, start.elapsed());
        let _ = execute!(terminal.backend_mut(), EndSynchronizedUpdate);
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
