//! End-to-end smoke tests of the real binary. Failures to open happen before the terminal is set
//! up, so those tests run it without a pseudo-terminal; the others run it in one with termlens.

use std::process::{Command, Output};

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

// F-FILE-01, FR-DOC-05
#[test]
fn shows_the_demo_and_quits_with_ctrl_q_restoring_the_terminal() -> termlens::Result<()> {
    let mut terminal = termlens::bin!("tpt")?;
    terminal.wait_until(|screen| screen.contains("Trip to Lisbon") && screen.alternate_screen())?;
    terminal.send(termlens::Key::Ctrl('q'))?;
    assert!(terminal.wait_exit()?.success());
    assert!(!terminal.screen().alternate_screen());
    Ok(())
}

#[test]
fn shows_the_file_given_on_the_command_line() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("todo.taskpaper");
    std::fs::write(&path, "Errands:\n\t- Post office\n")?;
    let mut terminal = termlens::bin!("tpt", arg(&path))?;
    terminal.wait_until(|screen| screen.contains("☐ Post office"))?;
    terminal.send(termlens::Key::Ctrl('q'))?;
    assert!(terminal.wait_exit()?.success());
    assert_eq!(
        std::fs::read_to_string(&path)?,
        "Errands:\n\t- Post office\n"
    );
    Ok(())
}
