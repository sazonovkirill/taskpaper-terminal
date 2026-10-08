//! Steps 2.3 and 2.12: backups on opening, read-only files, ending the session and crashes.

use std::{fs, os::unix::fs::PermissionsExt as _, path::Path, time::Duration};

use jiff::civil::date;
use taskpaper_terminal::{
    app::Message,
    ui::{Input, effects::Recovery},
};

use crate::support::{Harness, line, outline_rows};

/// The status line of `app` after a draw.
fn status_line(app: &mut Harness) -> String {
    let buffer = app.render().buffer().clone();
    line(&buffer, buffer.area.bottom() - 1)
}

// FR-DOC-07, F-FILE-09
#[test]
fn opening_a_file_backs_it_up_in_a_dated_copy() {
    let mut app = Harness::with_text("- Milk\n", 60, 5);
    app.settle();
    let folders: Vec<_> = fs::read_dir(&app.backups)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    let [folder] = folders.as_slice() else {
        panic!("one backup folder: {folders:?}");
    };
    let name = folder.file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.starts_with("outline.taskpaper-"), "{name}");
    assert_eq!(
        fs::read_to_string(folder.join("2026-10-05.taskpaper")).unwrap(),
        "- Milk\n"
    );
    assert_eq!(status_line(&mut app), app.status(60).trim_end());
}

// FR-DOC-07
#[test]
fn a_failed_backup_is_reported_in_the_status_line_for_a_while() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("backups"), "a file where the folder goes").unwrap();
    let path = dir.path().join("todo.taskpaper");
    fs::write(&path, "- Milk\n").unwrap();
    let mut app = Harness::open_path(dir, &path, 100, 4);
    app.settle();
    let status = status_line(&mut app);
    assert!(
        status.starts_with(&format!("{} · backup failed: ", app.status(100).trim_end())),
        "{status}"
    );
    app.wait(Duration::from_secs(10));
    assert_eq!(status_line(&mut app), app.status(100).trim_end());
    assert_eq!(fs::read_to_string(&path).unwrap(), "- Milk\n");
}

// file-io.md §1
#[test]
fn a_read_only_file_opens_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("todo.taskpaper");
    fs::write(&path, "- Milk\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
    let mut app = Harness::open_path(dir, &path, 80, 4);
    assert_eq!(
        status_line(&mut app),
        app.status_with(80, " · read-only").trim_end().to_owned()
    );
    assert_eq!(outline_rows(&mut app), ["- Milk"]);
}

// F-FILE-05
#[test]
fn a_termination_signal_ends_the_session() {
    let mut app = Harness::open("groceries.taskpaper", 40, 6);
    app.send(Input::Core(Message::Terminate));
    assert!(app.tui.should_quit());
}

/// Recovery files in `dir/recovery`.
fn recovery(dir: &Path) -> Recovery {
    Recovery::new(Some(dir.join("recovery")), || {
        date(2026, 10, 5).at(9, 0, 0, 0)
    })
}

// N-REL-01, file-io.md §4
#[test]
fn a_file_opened_with_its_recovered_changes_saves_them_after_a_backup() {
    let mut app = Harness::recovered("- Milk\n", "- Milk\n- Eggs\n", 60, 5);
    assert_eq!(outline_rows(&mut app), ["- Milk", "- Eggs"]);
    assert_eq!(
        status_line(&mut app),
        app.status_with(60, " ●").trim_end().to_owned()
    );
    app.wait(Duration::from_secs(1));
    app.settle();
    let path = app.path.clone().unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "- Milk\n- Eggs\n");
    assert_eq!(status_line(&mut app), app.status(60).trim_end());
    let folder = fs::read_dir(&app.backups)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(
        fs::read_to_string(folder.join("2026-10-05.taskpaper")).unwrap(),
        "- Milk\n",
        "the file as it was before the recovery"
    );
}

// N-REL-01
#[test]
fn a_crash_writes_the_unsaved_changes_to_a_recovery_file() {
    let app = Harness::recovered("- Milk\n", "- Milk\n- Eggs\n", 60, 5);
    let dir = tempfile::tempdir().unwrap();
    let written = app.tui.recover(&recovery(dir.path())).unwrap().unwrap();
    assert_eq!(
        written.parent(),
        Some(dir.path().join("recovery").as_path())
    );
    assert_eq!(fs::read_to_string(written).unwrap(), "- Milk\n- Eggs\n");
}

// N-REL-01
#[test]
fn a_crash_before_an_edit_is_saved_writes_it_to_a_recovery_file() {
    let mut app = Harness::with_text("- Milk\n", 60, 5);
    app.type_keys("x");
    let dir = tempfile::tempdir().unwrap();
    let written = app.tui.recover(&recovery(dir.path())).unwrap().unwrap();
    assert_eq!(
        fs::read_to_string(written).unwrap(),
        "- Milk @done(2026-10-05)\n"
    );
    assert_eq!(
        fs::read_to_string(app.path.as_ref().unwrap()).unwrap(),
        "- Milk\n"
    );
}

// N-REL-01
#[test]
fn a_crash_with_every_change_saved_writes_no_recovery_file() {
    let app = Harness::open("groceries.taskpaper", 40, 6);
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(app.tui.recover(&recovery(dir.path())).unwrap(), None);
    assert!(!dir.path().join("recovery").exists());
}

// N-REL-01, file-io.md §4
#[test]
fn a_crash_while_a_save_is_under_way_writes_the_recovery_file_after_it() {
    let mut app = Harness::with_text("- Milk\n- Eggs\n", 60, 5);
    app.settle();
    app.type_keys("x");
    app.wait(Duration::from_secs(1));
    app.type_keys("jx");
    let dir = tempfile::tempdir().unwrap();
    let recovery = recovery(dir.path());
    let written = app.tui.recover(&recovery).unwrap().unwrap();
    let path = app.path.clone().unwrap();
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "- Milk @done(2026-10-05)\n- Eggs\n",
        "the save under way is written first"
    );
    assert_eq!(recovery.find(&path).unwrap(), Some(written));
}
