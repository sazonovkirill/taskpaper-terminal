//! Running one scenario against the real binary in a pseudo-terminal.

use std::{fmt::Write as _, path::Path, time::Duration};

use jiff::{ToSpan, civil::Date};
use termlens::{Key, Signal, Terminal};

use crate::script::{Press, Scenario, Step};

/// How long a step waits for the screen or the file to show what it expects.
const TIMEOUT: Duration = Duration::from_secs(5);

/// How long the screen stays unchanged before `hide` takes it as drawn.
const QUIET: Duration = Duration::from_millis(150);

/// The pause after `Esc`, so that the next key is not read as an `Alt` chord, as a person's
/// typing never is.
const AFTER_ESC: Duration = Duration::from_millis(60);

/// How long a check waits for the screen to react to the input before it, so that it does not
/// read the screen drawn before; input that changes nothing visible costs this much.
const REACTION: Duration = Duration::from_secs(1);

/// Why a scenario failed: the step's line, what went wrong, and the screen and file then.
pub struct Failure {
    pub line: usize,
    pub message: String,
    pub screen: String,
    pub file: String,
}

/// Runs `scenario` with `binary` in a fresh temporary home, on `today` for the date placeholders.
pub fn run(scenario: &Scenario, binary: &Path, today: Date) -> Result<(), Failure> {
    let failure = |line: usize, message: String| Failure {
        line,
        message,
        screen: String::new(),
        file: String::new(),
    };
    let dir = tempfile::tempdir().map_err(|error| failure(0, error.to_string()))?;
    let path = dir.path().join("outline.taskpaper");
    let file = scenario
        .file
        .as_deref()
        .map(|text| expand(text, today))
        .unwrap_or_default();
    let config = dir.path().join("config");
    let prepare = || -> std::io::Result<()> {
        std::fs::write(&path, &file)?;
        if let Some(settings) = &scenario.config {
            std::fs::create_dir_all(config.join("taskpaper-terminal"))?;
            std::fs::write(config.join("taskpaper-terminal/config.toml"), settings)?;
        }
        Ok(())
    };
    prepare().map_err(|error| failure(0, error.to_string()))?;
    let launch = Launch {
        binary,
        home: dir.path(),
        path: &path,
        env: &scenario.env,
    };
    let terminal = launch
        .spawn(scenario.size)
        .map_err(|error| failure(0, error.to_string()))?;
    let mut session = Session {
        launch,
        terminal,
        size: scenario.size,
        today,
        after_esc: false,
        before: None,
        exited: false,
    };
    session
        .start()
        .map_err(|message| session.failure(0, message))?;
    for (line, step) in &scenario.steps {
        session
            .step(step)
            .map_err(|message| session.failure(*line, message))?;
    }
    Ok(())
}

/// How the app is started: the binary, its file and the folder standing for its home.
struct Launch<'a> {
    binary: &'a Path,
    home: &'a Path,
    path: &'a Path,
    env: &'a [(String, String)],
}

impl Launch<'_> {
    /// Starts the app in a `size` terminal, with every folder it writes to under the home folder.
    fn spawn(&self, (width, height): (u16, u16)) -> termlens::Result<Terminal> {
        Terminal::builder()
            .size(width, height)
            .env_clear()
            .env("HOME", self.home.join("home"))
            .env("XDG_CONFIG_HOME", self.home.join("config"))
            .env("XDG_DATA_HOME", self.home.join("data"))
            .env("XDG_STATE_HOME", self.home.join("state"))
            .env("LANG", "C.UTF-8")
            .env("TERM", "xterm-256color")
            // The runner computes `{today}` in this time zone, so the app must see the same one.
            .envs(std::env::var_os("TZ").map(|zone| ("TZ", zone)))
            .envs(self.env.iter().map(|(name, value)| (name, value)))
            .timeout(TIMEOUT)
            .arg(self.path)
            .spawn(self.binary)
    }
}

/// A running app and what the runner knows about it.
struct Session<'a> {
    launch: Launch<'a>,
    terminal: Terminal,
    size: (u16, u16),
    today: Date,
    /// Whether the last key was `Esc`, so that the next waits a moment.
    after_esc: bool,
    /// The screen's text before the input sent since the last check.
    before: Option<String>,
    exited: bool,
}

impl Session<'_> {
    fn start(&mut self) -> Result<(), String> {
        let rows = self.terminal.screen().rows();
        self.terminal
            .wait_until(|screen| {
                screen.alternate_screen() && !screen.row_text(rows - 1).trim().is_empty()
            })
            .map_err(|error| format!("the app did not start: {error}"))
    }

    /// Quits unless the app has, then starts it again on the same file and folders.
    fn restart(&mut self) -> Result<(), String> {
        if !self.exited {
            self.quit()?;
        }
        self.terminal = self
            .launch
            .spawn(self.size)
            .map_err(|error| error.to_string())?;
        self.exited = false;
        self.after_esc = false;
        self.before = None;
        self.start()
    }

    /// Sends `signal` and requires the app to end cleanly.
    fn signal(&mut self, signal: Signal) -> Result<(), String> {
        self.terminal
            .signal(signal)
            .map_err(|error| error.to_string())?;
        let status = self
            .terminal
            .wait_exit()
            .map_err(|error| format!("the app did not end: {error}"))?;
        self.exited = true;
        if status.success() {
            Ok(())
        } else {
            Err(format!("the app ended with {status:?}"))
        }
    }

    fn step(&mut self, step: &Step) -> Result<(), String> {
        if self.exited && !matches!(step, Step::Saved(_) | Step::Outside(_) | Step::Restart) {
            return Err(
                "the app has ended; only `saved`, `outside` and `restart` can follow".to_owned(),
            );
        }
        if matches!(
            step,
            Step::See(_) | Step::Hide(_) | Step::Status(_) | Step::Resize(..)
        ) {
            self.react();
        } else if !self.exited && !matches!(step, Step::Saved(_)) && self.before.is_none() {
            self.before = Some(self.terminal.screen().text());
        }
        match step {
            Step::Keys(presses) => presses.iter().try_for_each(|&press| self.press(press)),
            Step::Type(text) => text
                .chars()
                .try_for_each(|c| self.press(Press::Key(Key::Char(c)))),
            Step::Paste(text) => {
                self.after_esc = false;
                self.terminal
                    .paste(text.strip_suffix('\n').unwrap_or(text))
                    .map_err(|error| error.to_string())
            }
            Step::See(text) => {
                let text = expand(text, self.today);
                self.terminal
                    .wait_until(|screen| screen.contains(text.trim_end()))
                    .map_err(|_| format!("the screen never showed {text:?}"))
            }
            Step::Hide(text) => {
                let text = expand(text, self.today);
                let screen = self
                    .terminal
                    .wait_stable(QUIET)
                    .map_err(|error| error.to_string())?;
                if screen.contains(text.trim_end()) {
                    Err(format!("the screen shows {text:?}"))
                } else {
                    Ok(())
                }
            }
            Step::Status(text) => {
                let rows = self.terminal.screen().rows();
                self.terminal
                    .wait_until(|screen| screen.row_text(rows - 1).contains(text.as_str()))
                    .map_err(|_| format!("the status line never showed {text:?}"))
            }
            Step::Resize(width, height) => self.resize(*width, *height),
            Step::Outside(text) => std::fs::write(self.launch.path, expand(text, self.today))
                .map_err(|error| format!("cannot write the file: {error}")),
            Step::Restart => self.restart(),
            Step::Signal(signal) => self.signal(*signal),
            Step::Quit => self.quit(),
            Step::Saved(text) => self.saved(&expand(text, self.today)),
        }
    }

    /// Waits until the screen differs from the one before the latest input, or for
    /// [`REACTION`], whichever comes first.
    fn react(&mut self) {
        if let Some(before) = self.before.take() {
            let _ = self
                .terminal
                .wait_until_for(|screen| screen.text() != before, REACTION);
        }
    }

    /// Resizes the terminal once the app has drawn what the input before asked for, then waits
    /// for it to redraw at the new size. Input that reaches the app in the same instant as the
    /// resize can be lost: crossterm 0.29 returns the `Resize` event and drops the input
    /// readiness its edge-triggered poll reported with it, so the key waits unread until more
    /// input arrives (termlens's `Terminal::resize` docs).
    fn resize(&mut self, width: u16, height: u16) -> Result<(), String> {
        self.terminal
            .wait_stable(QUIET)
            .map_err(|error| error.to_string())?;
        self.size = (width, height);
        self.terminal
            .resize(width, height)
            .map_err(|error| error.to_string())?;
        let clipped = self.terminal.screen().text();
        let _ = self
            .terminal
            .wait_until_for(|screen| screen.text() != clipped, REACTION);
        self.terminal
            .wait_stable(QUIET)
            .map(drop)
            .map_err(|error| error.to_string())
    }

    fn press(&mut self, press: Press) -> Result<(), String> {
        let sent = match (press, self.after_esc) {
            (Press::Key(key), false) => self.terminal.send(key),
            (Press::Key(key), true) => self.terminal.send_after(AFTER_ESC, key),
            (Press::Chord(chord), false) => self.terminal.send(chord),
            (Press::Chord(chord), true) => self.terminal.send_after(AFTER_ESC, chord),
        };
        self.after_esc = press == Press::Key(Key::Esc);
        sent.map_err(|error| format!("cannot press {press:?}: {error}"))
    }

    fn quit(&mut self) -> Result<(), String> {
        self.press(Press::Key(Key::Ctrl('q')))?;
        let status = self
            .terminal
            .wait_exit()
            .map_err(|error| format!("the app did not quit: {error}"))?;
        self.exited = true;
        if status.success() {
            Ok(())
        } else {
            Err(format!("the app quit with {status:?}"))
        }
    }

    /// Waits until the file holds `expected`; once the app has quit, the file is final.
    fn saved(&mut self, expected: &str) -> Result<(), String> {
        let matches =
            |path: &Path| std::fs::read_to_string(path).is_ok_and(|text| text == expected);
        let found = if self.exited {
            matches(self.launch.path)
        } else {
            let path = self.launch.path;
            self.terminal.wait_until(|_| matches(path)).is_ok()
        };
        if found {
            return Ok(());
        }
        let actual = std::fs::read_to_string(self.launch.path).unwrap_or_default();
        Err(format!(
            "the file is not as expected:\n{}",
            difference(expected, &actual)
        ))
    }

    fn failure(&self, line: usize, message: String) -> Failure {
        Failure {
            line,
            message,
            screen: self.terminal.screen().text(),
            file: std::fs::read_to_string(self.launch.path).unwrap_or_default(),
        }
    }
}

/// The lines of `expected` and `actual`, marked `-` and `+` where they differ.
fn difference(expected: &str, actual: &str) -> String {
    let mut out = String::new();
    for change in similar::TextDiff::from_lines(expected, actual).iter_all_changes() {
        let sign = match change.tag() {
            similar::ChangeTag::Delete => '-',
            similar::ChangeTag::Insert => '+',
            similar::ChangeTag::Equal => ' ',
        };
        let _ = write!(out, "  {sign} {}", change.value().replace('\t', "→   "));
        if change.missing_newline() {
            out.push_str("⏎ missing\n");
        }
    }
    out
}

/// `text` with each `{today}`, `{today+N}` and `{today-N}` replaced by that date.
pub fn expand(text: &str, today: Date) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("{today") {
        out.push_str(&rest[..start]);
        let after = &rest[start + "{today".len()..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let offset = after[..end].replace('+', "");
        let days = if offset.is_empty() {
            Some(0)
        } else {
            offset.parse::<i32>().ok()
        };
        match days {
            Some(days) => {
                let date = today.checked_add(days.days()).unwrap_or(today);
                let _ = write!(out, "{date}");
            }
            None => out.push_str(&rest[start..=start + "{today".len() + end]),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}
