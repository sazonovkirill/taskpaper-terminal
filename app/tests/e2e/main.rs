//! End-to-end smoke tests of the real binary. Failures to open happen before the terminal is set
//! up, so those tests run it without a pseudo-terminal; the others run it in one with termlens.

use std::{
    fmt::Write as _,
    path::PathBuf,
    process::{Command, Output},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn tpt(args: &[&std::ffi::OsStr]) -> std::io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_tpt")).args(args).output()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

// F-FILE-07
#[test]
fn refuses_a_file_that_is_not_utf8() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("latin1.taskpaper");
    std::fs::write(&path, b"- caf\xe9\n")?;
    let output = tpt(&[path.as_os_str()])?;
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("not valid UTF-8"),
        "{}",
        stderr(&output)
    );
    assert_eq!(
        std::fs::read(&path)?,
        b"- caf\xe9\n",
        "the file must stay untouched"
    );
    Ok(())
}

#[test]
fn refuses_a_path_that_cannot_be_read() -> TestResult {
    let dir = tempfile::tempdir()?;
    let output = tpt(&[dir.path().as_os_str()])?;
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("cannot read"),
        "{}",
        stderr(&output)
    );
    Ok(())
}

#[test]
fn refuses_a_file_in_a_missing_folder() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("nowhere/todo.taskpaper");
    let output = tpt(&[path.as_os_str()])?;
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("does not exist"),
        "{}",
        stderr(&output)
    );
    Ok(())
}

#[test]
fn refuses_more_than_one_path() -> TestResult {
    let output = tpt(&["a".as_ref(), "b".as_ref()])?;
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("usage: tpt [path]"),
        "{}",
        stderr(&output)
    );
    Ok(())
}

/// The status line of `screen`, without trailing blanks.
fn status(screen: &termlens::Screen) -> String {
    screen.row_text(screen.rows() - 1).trim_end().to_owned()
}

/// The temporary file a start without a path made, as the status line names it.
fn temporary_file(screen: &termlens::Screen) -> Result<PathBuf, String> {
    let status = status(screen);
    status
        .strip_prefix("NAVIGATE · ")
        .and_then(|rest| rest.strip_suffix(" · temporary"))
        .map(PathBuf::from)
        .ok_or_else(|| format!("no temporary file in {status:?}"))
}

// F-FILE-01, FR-DOC-05, D-27
#[test]
fn writes_the_demo_to_a_new_file_in_tmp_and_quits_with_ctrl_q() -> TestResult {
    let dir = tempfile::tempdir()?;
    let data = dir.path().join("data");
    let mut terminal = termlens::bin!("tpt", env("XDG_DATA_HOME", &data))?;
    terminal.wait_until(|screen| {
        screen.contains("Trip to Lisbon")
            && screen.alternate_screen()
            && status(screen).ends_with("temporary")
    })?;
    let path = temporary_file(&terminal.screen())?;
    terminal.send(termlens::Key::Ctrl('q'))?;
    assert!(terminal.wait_exit()?.success());
    assert!(!terminal.screen().alternate_screen());
    let text = std::fs::read_to_string(&path)?;
    std::fs::remove_file(&path)?;
    assert!(path.starts_with("/tmp"), "{}", path.display());
    assert!(text.starts_with("Trip to Lisbon:"), "{text}");
    assert!(!data.exists(), "the temporary file was backed up");
    Ok(())
}

#[test]
fn shows_the_file_given_on_the_command_line() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("todo.taskpaper");
    std::fs::write(&path, "Errands:\n\t- Post office\n")?;
    let data = dir.path().join("data");
    let mut terminal = termlens::bin!("tpt", arg(&path), env("XDG_DATA_HOME", &data))?;
    terminal.wait_until(|screen| screen.contains("- Post office"))?;
    terminal.send(termlens::Key::Ctrl('q'))?;
    assert!(terminal.wait_exit()?.success());
    assert_eq!(
        std::fs::read_to_string(&path)?,
        "Errands:\n\t- Post office\n"
    );
    Ok(())
}

#[test]
fn a_pending_key_prefix_times_out_without_further_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut terminal = termlens::bin!("tpt", env("XDG_DATA_HOME", dir.path()))?;
    terminal.wait_until(|screen| status(screen).ends_with("temporary"))?;
    let path = temporary_file(&terminal.screen())?;
    terminal.send(termlens::Key::Char('g'))?;
    terminal.wait_until(|screen| status(screen).ends_with(" g"))?;
    terminal.wait_until(|screen| status(screen).ends_with("temporary"))?;
    terminal.send(termlens::Key::Ctrl('q'))?;
    assert!(terminal.wait_exit()?.success());
    std::fs::remove_file(path)?;
    Ok(())
}

#[test]
fn a_completed_key_sequence_acts_at_once() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("long.taskpaper");
    let text = (1..=100).fold(String::new(), |mut text, i| {
        writeln!(text, "- Task {i:03}").expect("writing to a String cannot fail");
        text
    });
    std::fs::write(&path, text)?;
    let data = dir.path().join("data");
    let mut terminal = termlens::bin!("tpt", arg(&path), env("XDG_DATA_HOME", &data))?;
    terminal.wait_until(|screen| screen.contains("Task 001"))?;
    terminal.send(termlens::Key::Char('G'))?;
    terminal.wait_until(|screen| screen.contains("Task 100") && !screen.contains("Task 001"))?;
    terminal.send(termlens::Key::Char('g'))?;
    terminal.send(termlens::Key::Char('g'))?;
    // Were the second `g` only read after the sequence timed out, it would start a new one.
    terminal.wait_until(|screen| screen.contains("Task 001"))?;
    terminal.send(termlens::Key::Ctrl('q'))?;
    assert!(terminal.wait_exit()?.success());
    Ok(())
}

/// The daily copies under the backups folder in `data`, the `XDG_DATA_HOME` the app ran with.
fn daily_copies(data: &std::path::Path) -> std::io::Result<Vec<String>> {
    let backups = data.join("taskpaper-terminal/backups");
    let mut copies = Vec::new();
    for folder in std::fs::read_dir(backups)? {
        for file in std::fs::read_dir(folder?.path())? {
            let file = file?.path();
            if file
                .extension()
                .is_some_and(|extension| extension == "taskpaper")
            {
                copies.push(std::fs::read_to_string(file)?);
            }
        }
    }
    Ok(copies)
}

// FR-DOC-07, F-FILE-09
#[test]
fn opening_a_file_backs_it_up_under_xdg_data_home() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("todo.taskpaper");
    std::fs::write(&path, "- Call the bank\n")?;
    let data = dir.path().join("data");
    let mut terminal = termlens::bin!("tpt", arg(&path), env("XDG_DATA_HOME", &data))?;
    terminal.wait_until(|screen| screen.contains("- Call the bank"))?;
    terminal.send(termlens::Key::Ctrl('q'))?;
    assert!(terminal.wait_exit()?.success());
    assert_eq!(daily_copies(&data)?, ["- Call the bank\n"]);
    Ok(())
}

// F-FILE-05
#[test]
fn sigterm_and_sighup_end_the_session_and_restore_the_terminal() -> TestResult {
    for signal in [termlens::Signal::Term, termlens::Signal::Hup] {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("todo.taskpaper");
        std::fs::write(&path, "- Water the plants\n")?;
        let data = dir.path().join("data");
        let mut terminal = termlens::bin!("tpt", arg(&path), env("XDG_DATA_HOME", &data))?;
        terminal.wait_until(|screen| screen.contains("- Water the plants"))?;
        terminal.signal(signal)?;
        assert!(terminal.wait_exit()?.success(), "{signal:?}");
        assert!(!terminal.screen().alternate_screen(), "{signal:?}");
        assert_eq!(std::fs::read_to_string(&path)?, "- Water the plants\n");
    }
    Ok(())
}
