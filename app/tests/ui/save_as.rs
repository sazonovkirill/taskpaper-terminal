//! Step 4.3: Save As, from Go to Command, writes the document to a path typed in a prompt, with
//! `~` and folder completion, asks before it replaces a file, and continues with that file.

use std::{fs, path::Path, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Harness, NAVIGATE, line};

/// Every screen row, without trailing blanks.
fn screen(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    (buffer.area.top()..buffer.area.bottom())
        .map(|y| line(&buffer, y))
        .collect()
}

fn press(app: &mut Harness, code: KeyCode) {
    app.press(code, KeyModifiers::NONE);
}

/// Opens Save As from Go to Command.
fn open_save_as(app: &mut Harness) {
    app.press(KeyCode::Char('p'), KeyModifiers::CONTROL);
    app.type_keys("save as");
    press(app, KeyCode::Enter);
}

/// Types `path` over the one Save As opens with and presses `Enter`, then lets the write finish.
fn save_as(app: &mut Harness, path: &Path) {
    open_save_as(app);
    app.type_keys(&path.display().to_string());
    press(app, KeyCode::Enter);
    app.settle();
}

/// The rows of the Save As prompt, from its top border, at a terminal of width 70.
fn prompt(app: &mut Harness) -> Vec<String> {
    let rows = screen(app);
    let top = rows
        .iter()
        .position(|row| row.starts_with("┌ Save As"))
        .expect("the prompt is open");
    rows.get(top..top + 4).expect("the prompt fits").to_vec()
}

/// The folders in the backups folder whose names start with `name`.
fn backup_folders(app: &Harness, name: &str) -> usize {
    fs::read_dir(&app.backups).map_or(0, |entries| {
        entries
            .filter(|entry| {
                entry
                    .as_ref()
                    .is_ok_and(|entry| entry.file_name().to_string_lossy().starts_with(name))
            })
            .count()
    })
}

// FR-DOC-06, FR-DOC-05, file-io.md §1 and §2, keybindings.md §4, ui.md §1
#[test]
fn the_temporary_file_is_kept_in_a_folder_under_home_completed_with_tab() {
    let mut app = Harness::demo(70, 16);
    let temporary = app.path.clone().expect("the demo file");
    let demo = fs::read_to_string(&temporary).unwrap();
    open_save_as(&mut app);
    assert_eq!(
        prompt(&mut app)[2],
        "│ Tab completes a folder, Enter saves                                │"
    );
    fs::create_dir(app.home.join("notes")).unwrap();
    app.type_keys("~/no");
    press(&mut app, KeyCode::Tab);
    app.type_keys("todo.taskpaper");
    assert_eq!(
        prompt(&mut app),
        [
            "┌ Save As ───────────────────────────────────────────────────────────┐",
            "│ ~/notes/todo.taskpaper                                             │",
            "│ Tab completes a folder, Enter saves                                │",
            "└────────────────────────────────────────────────────────────────────┘",
        ]
    );
    let top = screen(&mut app)
        .iter()
        .position(|row| row.starts_with("┌ Save As"))
        .unwrap();
    app.terminal
        .backend_mut()
        .assert_cursor_position((24, u16::try_from(top + 1).unwrap()));
    press(&mut app, KeyCode::Enter);
    app.settle();
    let kept = app.home.join("notes/todo.taskpaper");
    assert_eq!(fs::read_to_string(&kept).unwrap(), demo);
    let rows = screen(&mut app);
    assert!(!rows.iter().any(|row| row.contains("Save As")), "{rows:#?}");
    assert_eq!(
        rows[15],
        format!("{NAVIGATE} · {}", kept.display()),
        "the new file, no longer marked temporary"
    );
    app.type_keys("x");
    app.wait(Duration::from_secs(1));
    app.settle();
    assert_ne!(fs::read_to_string(&kept).unwrap(), demo, "changes go there");
    assert_eq!(
        fs::read_to_string(&temporary).unwrap(),
        demo,
        "the previous file is left as it was"
    );
    assert_eq!(
        backup_folders(&app, "todo.taskpaper-"),
        1,
        "backed up from now on"
    );
}

// FR-DOC-06, file-io.md §2
#[test]
fn a_file_there_is_replaced_once_confirmed_after_a_backup_and_esc_goes_back() {
    let mut app = Harness::with_text("- mine\n", 70, 16);
    let other = app.home.join("other.taskpaper");
    fs::write(&other, "- theirs\n").unwrap();
    save_as(&mut app, &other);
    assert_eq!(
        prompt(&mut app)[2],
        "│ The file exists: Enter replaces it, Esc goes back                  │"
    );
    press(&mut app, KeyCode::Esc);
    assert_eq!(
        prompt(&mut app)[2],
        "│ Tab completes a folder, Enter saves                                │"
    );
    assert_eq!(fs::read_to_string(&other).unwrap(), "- theirs\n");
    press(&mut app, KeyCode::Enter);
    app.settle();
    press(&mut app, KeyCode::Enter);
    app.settle();
    assert_eq!(fs::read_to_string(&other).unwrap(), "- mine\n");
    assert_eq!(
        screen(&mut app)[15],
        format!("{NAVIGATE} · {}", other.display())
    );
    let folder = fs::read_dir(&app.backups)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.to_string_lossy().contains("other.taskpaper-"))
        .expect("the replaced file has a backup folder");
    assert_eq!(
        fs::read_to_string(folder.join("2026-10-05.taskpaper")).unwrap(),
        "- theirs\n"
    );
}

// FR-DOC-06
#[test]
fn a_failed_write_says_why_and_esc_keeps_the_file_open() {
    let mut app = Harness::with_text("- mine\n", 70, 16);
    let opened = app.path.clone().unwrap();
    let missing = app.home.join("missing/a.taskpaper");
    save_as(&mut app, &missing);
    let status = &prompt(&mut app)[2];
    assert!(
        status.starts_with("│ Cannot save: ") && status.contains("No such file"),
        "{status}"
    );
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Esc);
    let rows = screen(&mut app);
    assert!(!rows.iter().any(|row| row.contains("Save As")), "{rows:#?}");
    assert_eq!(rows[15], format!("{NAVIGATE} · {}", opened.display()));
}
