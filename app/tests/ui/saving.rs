//! Step 2.3: backups on opening, read-only files, and ending the session.

use std::{fs, os::unix::fs::PermissionsExt as _, time::Duration};

use taskpaper_terminal::{app::Message, ui::Input};

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
        format!("{} · read-only", app.status(80).trim_end())
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
