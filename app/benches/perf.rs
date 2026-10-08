//! Performance benchmarks for N-PERF-01 and N-PERF-02: opening a 100k-line, 10 MB outline and the
//! time from a keystroke to its rendered frame on it (plan step 7.3).
//!
//! `bash app/scripts/validate.sh bench` (or `cargo bench --bench perf`) builds the outline,
//! measures each scenario and fails when one misses its budget; text after `--` picks the
//! scenarios whose name holds it. Run as a test, as `cargo test --all-targets` does, it drives the
//! same scenarios once on a small outline without timing them, so they cannot rot.

use std::{
    fmt::Write as _,
    path::Path,
    process::ExitCode,
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use jiff::{
    civil::{Date, DateTime, date},
    tz::TimeZone,
};
use ratatui::{Terminal, backend::TestBackend};
use taskpaper_terminal::ui::{
    Input, Tui,
    effects::{self, Backups, Browser, Clipboard, FileWatch, Io, Views},
};

/// The local date the benchmarks run on, so the outline's dates are near it.
const TODAY: Date = date(2026, 10, 5);

/// The local time the app reads.
const NOW: DateTime = TODAY.at(9, 30, 0, 0);

/// The outline N-PERF-01 names: 100k lines and about 10 MB.
const LINES: usize = 100_000;

/// The terminal the benchmarks draw on.
const WIDTH: u16 = 160;
const HEIGHT: u16 = 48;

/// N-PERF-01: open and render.
const OPEN_BUDGET: Duration = Duration::from_millis(300);

/// N-PERF-02: keystroke to rendered frame.
const KEY_BUDGET: Duration = Duration::from_millis(16);

/// How often each keystroke scenario is measured at most; the slowest run is held to the budget.
const RUNS: usize = 15;

/// A scenario stops repeating once its runs took this long, so a slow one does not take minutes.
const SCENARIO_TIME: Duration = Duration::from_secs(2);

fn main() -> ExitCode {
    // `cargo bench` passes `--bench`; a test run passes nothing of the sort.
    let timed = std::env::args().any(|arg| arg == "--bench");
    // `cargo bench --bench perf -- <text>` runs only the scenarios whose name holds the text.
    let only: Vec<String> = std::env::args()
        .skip(1)
        .filter(|arg| !arg.starts_with('-'))
        .collect();
    let lines = if timed { LINES } else { 2_000 };
    let runs = if timed { RUNS } else { 1 };
    let dir = tempfile::tempdir().expect("a temporary folder");
    let path = dir.path().join("large.taskpaper");
    let text = outline(lines);
    if !timed {
        check_size();
    }
    std::fs::write(&path, &text).expect("the temporary folder is writable");
    let mut results = Vec::new();
    let (mut app, open) = App::open(&path, dir.path());
    results.push(Result::new("open and first frame", open, OPEN_BUDGET));
    for scenario in SCENARIOS.iter().filter(|scenario| {
        only.iter()
            .all(|wanted| scenario.name.contains(wanted.as_str()))
    }) {
        let mut took = Vec::new();
        while took.len() < runs && took.iter().sum::<Duration>() < SCENARIO_TIME {
            took.push((scenario.run)(&mut app));
        }
        let slowest = took.iter().max().copied().unwrap_or_default();
        results.push(Result::new(scenario.name, slowest, KEY_BUDGET));
    }
    if !timed {
        return ExitCode::SUCCESS;
    }
    println!(
        "{} lines, {:.1} MB, {WIDTH}×{HEIGHT}, slowest of up to {runs} runs:",
        text.lines().count(),
        megabytes(text.len())
    );
    let mut failed = false;
    for result in &results {
        let mark = if result.time <= result.budget {
            "ok  "
        } else {
            failed = true;
            "SLOW"
        };
        println!(
            "  {mark} {:<28} {:>9.2} ms  (budget {} ms)",
            result.name,
            result.time.as_secs_f64() * 1000.0,
            result.budget.as_millis()
        );
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

#[expect(clippy::cast_precision_loss, reason = "a size shown with one decimal")]
fn megabytes(bytes: usize) -> f64 {
    bytes as f64 / 1_000_000.0
}

/// One measured line of the report.
struct Result {
    name: &'static str,
    time: Duration,
    budget: Duration,
}

impl Result {
    fn new(name: &'static str, time: Duration, budget: Duration) -> Self {
        Self { name, time, budget }
    }
}

/// A keystroke scenario: it measures one keystroke (or a short sequence a user types as one,
/// such as `gg`) and leaves the app ready to run again.
struct Scenario {
    name: &'static str,
    run: fn(&mut App) -> Duration,
}

const SCENARIOS: &[Scenario] = &[
    Scenario {
        name: "j (next block)",
        run: |app| app.keys("j"),
    },
    Scenario {
        name: "k (previous block)",
        run: |app| app.keys("k"),
    },
    Scenario {
        name: "G then gg (last, first)",
        run: |app| app.keys("G").max(app.keys("gg")),
    },
    Scenario {
        name: "Space (toggle fold)",
        run: |app| app.keys(" ").max(app.keys(" ")),
    },
    Scenario {
        name: "_ then + (collapse, expand all)",
        run: |app| app.keys("_").max(app.keys("+")),
    },
    Scenario {
        name: ". then > (zoom in, out)",
        run: |app| app.keys(".").max(app.keys(">")),
    },
    Scenario {
        name: "x (toggle done)",
        run: |app| app.keys("x").max(app.keys("x")),
    },
    Scenario {
        name: "Enter, type, Esc (edit)",
        run: |app| {
            let enter = app.key(KeyCode::Enter, KeyModifiers::NONE);
            let typed = app.keys("a");
            let erased = app.key(KeyCode::Backspace, KeyModifiers::NONE);
            let left = app.key(KeyCode::Esc, KeyModifiers::NONE);
            enter.max(typed).max(erased).max(left)
        },
    },
    Scenario {
        name: "o, type, Esc (new block)",
        run: |app| {
            let new = app.keys("o");
            let typed = app.keys("b");
            let left = app.key(KeyCode::Esc, KeyModifiers::NONE);
            new.max(typed).max(left)
        },
    },
    Scenario {
        name: "Tab, Shift+Tab (indent)",
        run: |app| {
            app.keys("j");
            let indent = app.key(KeyCode::Tab, KeyModifiers::NONE);
            let outdent = app.key(KeyCode::BackTab, KeyModifiers::SHIFT);
            indent.max(outdent)
        },
    },
    Scenario {
        name: "z (undo)",
        run: |app| {
            app.keys("x");
            app.keys("z")
        },
    },
    Scenario {
        name: "gT then H (Today)",
        run: |app| app.special_view("gT"),
    },
    Scenario {
        name: "gU then H (Upcoming)",
        run: |app| app.special_view("gU"),
    },
    Scenario {
        name: "gO then H (Overdue)",
        run: |app| app.special_view("gO"),
    },
    Scenario {
        name: "gC then H (Colors)",
        run: |app| app.special_view("gC"),
    },
    Scenario {
        name: "gL then H (Logbook)",
        run: |app| app.special_view("gL"),
    },
    Scenario {
        name: "gS then H (Starred)",
        run: |app| app.special_view("gS"),
    },
    Scenario {
        name: "gF then H (Flagged)",
        run: |app| app.special_view("gF"),
    },
    Scenario {
        name: "x in the Today view",
        run: |app| {
            app.keys("gT");
            // Undone rather than toggled back, which would date a block done before as done today.
            let done = app.keys("x").max(app.keys("z"));
            app.keys("H");
            done
        },
    },
    Scenario {
        name: "Ctrl+G, Esc (Navigator)",
        run: |app| {
            // A change since the Navigator last opened, as there usually is.
            app.keys("x");
            let open = app.key(KeyCode::Char('g'), KeyModifiers::CONTROL);
            let close = app.key(KeyCode::Esc, KeyModifiers::NONE);
            open.max(close)
        },
    },
    Scenario {
        name: "type in the Navigator",
        run: |app| {
            app.key(KeyCode::Char('g'), KeyModifiers::CONTROL);
            let typed = app.keys("a");
            let erased = app.key(KeyCode::Backspace, KeyModifiers::NONE);
            app.key(KeyCode::Esc, KeyModifiers::NONE);
            typed.max(erased)
        },
    },
    Scenario {
        name: "qq, type, Esc (Quick Add)",
        run: |app| {
            let open = app.keys("qq");
            let typed = app.keys("c");
            let left = app.key(KeyCode::Esc, KeyModifiers::NONE);
            open.max(typed).max(left)
        },
    },
    Scenario {
        name: "Quick Add to a project",
        run: |app| {
            app.keys("x");
            let open = app.keys("qq");
            let typed = app.keys("c ");
            let picker = app.keys("/");
            let chosen = app.key(KeyCode::Enter, KeyModifiers::NONE);
            let added = app.key(KeyCode::Enter, KeyModifiers::NONE);
            app.keys("z");
            open.max(typed).max(picker).max(chosen).max(added)
        },
    },
    Scenario {
        name: "gs, type, Esc (Go to Symbol)",
        run: |app| app.finder("gs"),
    },
    Scenario {
        name: "gP, type, Esc (Go to Project)",
        run: |app| app.finder("gP"),
    },
    Scenario {
        name: "gm, type, Esc (Move to)",
        run: |app| app.finder("gm"),
    },
    Scenario {
        name: "autosave",
        run: |app| {
            app.keys("x");
            app.later(Duration::from_secs(2))
        },
    },
];

/// The app on a file, drawn on a test terminal; disk work runs on the I/O thread as in `tpt`.
struct App {
    tui: Tui,
    terminal: Terminal<TestBackend>,
    inputs: Receiver<Input>,
    now: Duration,
}

impl App {
    /// Opens `path` as `tpt` does, with backups and layouts kept in `dir`, and draws the first
    /// frame; returns how long that took.
    #[expect(clippy::disallowed_methods, reason = "a benchmark measures wall time")]
    fn open(path: &Path, dir: &Path) -> (Self, Duration) {
        let views = dir.join("state/views.json");
        let (results, inputs) = mpsc::channel();
        let started = Instant::now();
        let state = effects::open(path).expect("the outline opens");
        let state = Views::new(Some(views.clone())).restore(state);
        let io = Io::start(
            results,
            Backups::new(Some(dir.join("backups")), || NOW),
            Views::new(Some(views)),
        );
        let tui = Tui::new(
            state,
            io,
            FileWatch::off(),
            Browser::new("tpt-bench-no-such-opener"),
            Clipboard::new(None, std::io::sink()),
            || NOW.to_zoned(TimeZone::UTC).expect("a date in range"),
            None,
        );
        let mut app = Self {
            tui,
            terminal: Terminal::new(TestBackend::new(WIDTH, HEIGHT)).expect("a test terminal"),
            inputs,
            now: Duration::ZERO,
        };
        app.draw();
        let open = started.elapsed();
        (app, open)
    }

    /// Types `keys` and draws the frame after the last one; returns how long that took.
    fn keys(&mut self, keys: &str) -> Duration {
        self.timed(|tui, now| {
            for c in keys.chars() {
                let key = Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
                tui.handle(Input::Terminal(key), now);
            }
        })
    }

    /// Opens a special view with `keys` after a change, as it usually follows one, and goes back
    /// to the outline; returns the slower of the two.
    fn special_view(&mut self, keys: &str) -> Duration {
        self.keys("x");
        self.keys(keys).max(self.keys("H"))
    }

    /// Opens a finder with `keys` after a change, types into it, erases that and closes it;
    /// returns the slowest of these.
    fn finder(&mut self, keys: &str) -> Duration {
        self.keys("x");
        let open = self.keys(keys);
        let typed = self.keys("t");
        let erased = self.key(KeyCode::Backspace, KeyModifiers::NONE);
        let closed = self.key(KeyCode::Esc, KeyModifiers::NONE);
        open.max(typed).max(erased).max(closed)
    }

    /// Presses `code` with `modifiers` and draws the frame; returns how long that took.
    fn key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Duration {
        self.timed(|tui, now| {
            let key = Event::Key(KeyEvent::new(code, modifiers));
            tui.handle(Input::Terminal(key), now);
        })
    }

    /// Lets `wait` pass, as the event loop does when a deadline such as an autosave passes, and
    /// draws the frame; returns how long that took.
    fn later(&mut self, wait: Duration) -> Duration {
        self.now += wait;
        self.timed(Tui::tick)
    }

    /// Runs `input` at the current time and draws, timing both, as the event loop does for one
    /// input. Results of disk work that arrived meanwhile are handed over first, untimed.
    #[expect(clippy::disallowed_methods, reason = "a benchmark measures wall time")]
    fn timed(&mut self, input: impl FnOnce(&mut Tui, Duration)) -> Duration {
        self.settle();
        self.now += Duration::from_millis(1);
        let started = Instant::now();
        input(&mut self.tui, self.now);
        self.draw();
        started.elapsed()
    }

    /// Hands the results of finished disk work to the app.
    fn settle(&mut self) {
        self.tui.flush();
        for input in self.inputs.try_iter().collect::<Vec<_>>() {
            self.tui.handle(input, self.now);
        }
    }

    fn draw(&mut self) {
        self.terminal
            .draw(|frame| self.tui.draw(frame))
            .expect("drawing to a test backend");
    }
}

/// Checks that the benchmark's outline has the size N-PERF-01 names.
fn check_size() {
    let text = outline(LINES);
    assert_eq!(text.lines().count(), LINES);
    let size = megabytes(text.len());
    assert!((9.5..=10.5).contains(&size), "{size} MB");
}

/// A made-up outline of about `lines` lines and 100 bytes per line, the shape of a large
/// personal outline: top-level projects of nested projects, tasks and notes, with dates, done
/// tasks, colors, links, Markdown and tags of the user's own. The same for every run.
fn outline(lines: usize) -> String {
    let mut random = Random(0x5eed);
    let mut text = String::with_capacity(lines * 100);
    let mut written = 0;
    let mut project = 0;
    while written < lines {
        project += 1;
        let _ = writeln!(
            text,
            "Area {project} of the large outline benchmark with a longer name: @area"
        );
        written += 1;
        let mut depth = 1;
        for _ in 0..random.below(60) + 20 {
            if written >= lines {
                break;
            }
            let indent = "\t".repeat(depth);
            let n = written;
            let line = match random.below(20) {
                0 => format!(
                    "Subproject {n} with a name that is long enough to fill most of a line on screen:"
                ),
                1..=10 => task(&mut random, n),
                11..=13 => format!(
                    "A note about item {n} that explains what it is for in a sentence or two, with **bold** text in it"
                ),
                14 => format!(
                    "- Read https://example.com/articles/{n}/a-long-title before the meeting next week @read"
                ),
                15 => format!(
                    "- Call the office about the order number {n} `ABC-{n}` and ask when it will arrive @phone"
                ),
                16 => String::new(),
                _ => format!(
                    "- Task {n} with more words in it to make the line about as long as a real one in a big file @next"
                ),
            };
            let opens = line.ends_with(':');
            let _ = writeln!(text, "{indent}{line}");
            written += 1;
            depth = if opens {
                (depth + 1).min(6)
            } else {
                match random.below(6) {
                    0 => depth.saturating_sub(1).max(1),
                    1 if depth < 6 => depth + 1,
                    _ => depth,
                }
            };
        }
    }
    text
}

/// A task with some of the tags a large outline has.
fn task(random: &mut Random, n: usize) -> String {
    let mut line = format!(
        "- Do the thing number {n} that needs to be done this week, before the review on Friday morning"
    );
    let day = random.below(28) + 1;
    match random.below(8) {
        0 => {
            let _ = write!(line, " @done(2026-09-{day:02})");
        }
        1 => {
            let _ = write!(line, " @due(2026-10-{day:02})");
        }
        2 => {
            let _ = write!(line, " @start(2026-10-{day:02}) @due(2026-11-{day:02})");
        }
        3 => line.push_str(" @today"),
        4 => line.push_str(" @red @priority(1)"),
        _ => line.push_str(" @context(home)"),
    }
    line
}

/// A small linear congruential generator, so the outline is the same on every run without a
/// dependency.
struct Random(u64);

impl Random {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        usize::try_from(self.0 >> 33).expect("31 bits fit a usize") % n
    }
}
