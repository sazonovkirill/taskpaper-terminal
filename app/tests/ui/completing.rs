//! Step 2.4: completing blocks, saved, and undoing and redoing it.

use std::{fs, os::unix::fs::PermissionsExt as _, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Harness, line, outline_rows};

const GROCERIES: &str = "Groceries: @home\n\t- Milk\n\t- Bread\n\t\tWholegrain if they have it.\n\tDairy:\n\t\t- Cheese\nNotes about the week.\n";

/// The status line of `app` after a draw.
fn status_line(app: &mut Harness) -> String {
    let buffer = app.render().buffer().clone();
    line(&buffer, buffer.area.bottom() - 1)
}

/// The open file's text once the autosave delay has passed and its save is done.
fn saved(app: &mut Harness) -> String {
    app.wait(Duration::from_secs(1));
    app.settle();
    let path = app.path.as_ref().expect("a file was opened");
    fs::read_to_string(path).expect("the file is readable")
}

// FR-DONE-01, D-4, FR-DOC-02
#[test]
fn x_completes_the_focused_task_and_saves_it_dated_today() {
    let mut app = Harness::open("groceries.taskpaper", 50, 10);
    app.settle();
    app.type_keys("j");
    app.type_keys("x");
    assert_eq!(outline_rows(&mut app)[1], "  ✔ Milk");
    assert_eq!(
        status_line(&mut app),
        format!("{} ●", app.status(50).trim_end())
    );
    assert_eq!(
        saved(&mut app),
        GROCERIES.replace("- Milk", "- Milk @done(2026-10-05)")
    );
    assert_eq!(status_line(&mut app), app.status(50).trim_end());
    app.type_keys("x");
    assert_eq!(outline_rows(&mut app)[1], "  - Milk");
    assert_eq!(saved(&mut app), GROCERIES);
}

// FR-OPS-08, N-UX-02
#[test]
fn z_and_shift_z_undo_and_redo_and_save_each_step() {
    let mut app = Harness::open("groceries.taskpaper", 50, 10);
    app.settle();
    app.type_keys("jxjx");
    app.type_keys("gg");
    let both = GROCERIES
        .replace("- Milk", "- Milk @done(2026-10-05)")
        .replace("- Bread", "- Bread @done(2026-10-05)");
    assert_eq!(saved(&mut app), both);
    app.type_keys("z");
    assert_eq!(outline_rows(&mut app)[2], "  - Bread");
    assert_eq!(
        crate::support::focused_rows(&mut app),
        ["  - Bread"],
        "undo returns to the block it changed"
    );
    app.type_keys("z");
    assert_eq!(saved(&mut app), GROCERIES, "the file is as it was opened");
    app.type_keys("Z");
    assert_eq!(outline_rows(&mut app)[1], "  ✔ Milk");
    assert_eq!(
        saved(&mut app),
        GROCERIES.replace("- Milk", "- Milk @done(2026-10-05)")
    );
}

// FR-OPS-08
#[test]
fn ctrl_z_and_ctrl_y_undo_and_redo() {
    let mut app = Harness::open("groceries.taskpaper", 50, 10);
    app.type_keys("jx");
    app.press(KeyCode::Char('z'), KeyModifiers::CONTROL);
    assert_eq!(outline_rows(&mut app)[1], "  - Milk");
    app.press(KeyCode::Char('y'), KeyModifiers::CONTROL);
    assert_eq!(outline_rows(&mut app)[1], "  ✔ Milk");
}

// FR-DONE-01
#[test]
fn ctrl_enter_completes_the_focused_block() {
    let mut app = Harness::open("groceries.taskpaper", 50, 10);
    app.type_keys("j");
    app.press(KeyCode::Enter, KeyModifiers::CONTROL);
    assert_eq!(outline_rows(&mut app)[1], "  ✔ Milk");
}

// file-io.md §1
#[test]
fn a_read_only_file_refuses_to_complete() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("todo.taskpaper");
    fs::write(&path, "- Milk\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
    let mut app = Harness::open_path(dir, &path, 90, 4);
    app.type_keys("x");
    assert_eq!(outline_rows(&mut app), ["- Milk"]);
    assert_eq!(
        status_line(&mut app),
        format!(
            "{} · read-only · cannot edit a read-only file",
            app.status(90).trim_end()
        )
    );
    app.wait(Duration::from_secs(10));
    app.settle();
    assert_eq!(fs::read_to_string(&path).unwrap(), "- Milk\n");
}

// FR-DOC-05, FR-DONE-01
#[test]
fn the_demo_has_a_task_to_check_off() {
    let mut app = Harness::demo(50, 12);
    app.type_keys("jx");
    assert_eq!(outline_rows(&mut app)[1], "  ✔ Book flights");
    let text = saved(&mut app);
    assert!(
        text.contains("\t- Book flights @done(2026-10-05)\n"),
        "{text}"
    );
}
