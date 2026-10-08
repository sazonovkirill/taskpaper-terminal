//! Real-user scenarios: runs every `.scenario` file under `tests/scenarios/library/` against the
//! real binary in a pseudo-terminal, in parallel, and reports each failing step with the screen
//! and the file (docs/quality/testing-strategy.md, Scenarios).
//!
//! `cargo test --test scenarios -- <filter>...` runs the scenarios whose path contains any filter.
//! A scenario with a `# BUG:` line is expected to fail; when it passes, the run fails until the
//! line is removed, so a fixed bug stays fixed.

mod run;
mod script;

use std::{
    io::Write as _,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

/// What came of one scenario.
enum Outcome {
    Passed,
    Failed(String),
    /// A `# BUG:` scenario that still fails.
    KnownBug,
    /// A `# BUG:` scenario that passes.
    Fixed,
}

fn main() -> ExitCode {
    let library = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios/library");
    let filters: Vec<String> = std::env::args()
        .skip(1)
        .filter(|arg| !arg.starts_with('-'))
        .collect();
    let mut paths = Vec::new();
    if let Err(error) = collect(&library, &mut paths) {
        eprintln!("cannot read {}: {error}", library.display());
        return ExitCode::FAILURE;
    }
    paths.sort();
    paths.retain(|path| {
        let name = path
            .strip_prefix(&library)
            .unwrap_or(path)
            .to_string_lossy();
        filters.is_empty() || filters.iter().any(|filter| name.contains(filter.as_str()))
    });
    let today = jiff::Zoned::now().date();
    let binary = Path::new(env!("CARGO_BIN_EXE_tpt"));
    let next = AtomicUsize::new(0);
    let outcomes = Mutex::new(Vec::new());
    let jobs = thread::available_parallelism().map_or(4, |jobs| jobs.get() * 2);
    thread::scope(|scope| {
        for _ in 0..jobs.min(paths.len()) {
            scope.spawn(|| {
                while let Some(path) = paths.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let outcome = outcome(path, binary, today);
                    let name = path.strip_prefix(&library).unwrap_or(path);
                    report(name, &outcome);
                    if let Ok(mut outcomes) = outcomes.lock() {
                        outcomes.push((name.to_owned(), outcome));
                    }
                }
            });
        }
    });
    let outcomes = outcomes.into_inner().unwrap_or_default();
    let count = |wanted: fn(&Outcome) -> bool| outcomes.iter().filter(|(_, o)| wanted(o)).count();
    let failed = count(|o| matches!(o, Outcome::Failed(_)));
    let fixed = count(|o| matches!(o, Outcome::Fixed));
    println!(
        "\nscenarios: {} passed, {failed} failed, {} known bugs, {fixed} fixed bugs still marked",
        count(|o| matches!(o, Outcome::Passed)),
        count(|o| matches!(o, Outcome::KnownBug)),
    );
    if failed + fixed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Every `.scenario` file under `dir`.
fn collect(dir: &Path, paths: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, paths)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == "scenario")
        {
            paths.push(path);
        }
    }
    Ok(())
}

fn outcome(path: &Path, binary: &Path, today: jiff::civil::Date) -> Outcome {
    let scenario = match std::fs::read_to_string(path)
        .map_err(|error| error.to_string())
        .and_then(|text| script::parse(&text))
    {
        Ok(scenario) => scenario,
        Err(error) => return Outcome::Failed(format!("cannot read the scenario: {error}")),
    };
    match (run::run(&scenario, binary, today), &scenario.bug) {
        (Ok(()), None) => Outcome::Passed,
        (Ok(()), Some(_)) => Outcome::Fixed,
        (Err(_), Some(_)) => Outcome::KnownBug,
        (Err(failure), None) => Outcome::Failed(format!(
            "{}\nline {}: {}\n--- screen\n{}\n--- file\n{}",
            scenario.title, failure.line, failure.message, failure.screen, failure.file
        )),
    }
}

fn report(name: &Path, outcome: &Outcome) {
    let name = name.display();
    let text = match outcome {
        Outcome::Passed => format!("ok      {name}\n"),
        Outcome::KnownBug => format!("bug     {name}\n"),
        Outcome::Fixed => {
            format!("FIXED   {name}: passes; remove its BUG line\n")
        }
        Outcome::Failed(details) => format!("FAILED  {name}: {details}\n"),
    };
    // One write per scenario keeps the reports of parallel runs from interleaving.
    let _ = std::io::stdout().lock().write_all(text.as_bytes());
}
