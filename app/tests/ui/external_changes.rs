//! Step 2.11: the file changed, deleted or replaced by another program while the app has it open.

use std::{fs, path::PathBuf, time::Duration};

use crossterm::event::{Event, KeyCode, KeyModifiers};
use taskpaper_terminal::{app::Message, ui::Input};

use crate::support::{Harness, focused_rows, line, outline_rows};

const GROCERIES: &str = "Groceries:\n\t- Milk\n\t- Eggs\n";

/// The status line of `app` after a draw.
fn status_line(app: &mut Harness) -> String {
    let buffer = app.render().buffer().clone();
    line(&buffer, buffer.area.bottom() - 1)
}

/// The 80 columns in the middle of a 140-column screen, where prompts are drawn, one string per
/// row.
fn middle(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    (buffer.area.top()..buffer.area.bottom())
        .map(|y| (30..110).map(|x| buffer[(x, y)].symbol()).collect())
        .collect()
}

/// Lets the app check its file, as the file watcher asks it to after a change.
fn check(app: &mut Harness) {
    app.send(Input::Core(Message::CheckFile));
    app.settle();
}

fn path(app: &Harness) -> PathBuf {
    app.path.clone().expect("a file was opened")
}

/// The conflict copies in the file's backup folder, as (name, contents).
fn conflict_copies(app: &Harness) -> Vec<(String, Vec<u8>)> {
    let mut copies = Vec::new();
    let entries = |folder| fs::read_dir(folder).expect("a backups folder");
    for folder in entries(app.backups.clone()) {
        for entry in entries(folder.expect("a folder entry").path()) {
            let path = entry.expect("a folder entry").path();
            let name = path.file_name().expect("a file name").to_string_lossy();
            if name.ends_with("-conflict.taskpaper") {
                let contents = fs::read(&path).expect("a readable copy");
                copies.push((name.into_owned(), contents));
            }
        }
    }
    copies.sort();
    copies
}

// F-FILE-06, OQ-16
#[test]
fn a_file_changed_elsewhere_is_reloaded_keeping_the_focus() {
    let mut app = Harness::with_text(GROCERIES, 80, 7);
    app.settle();
    app.type_keys("jj");
    assert_eq!(focused_rows(&mut app), ["  - Eggs"]);
    fs::write(path(&app), "Groceries:\n\t- Bread\n\t- Milk\n\t- Eggs\n").unwrap();
    check(&mut app);
    assert_eq!(
        outline_rows(&mut app),
        ["Groceries:", "  - Bread", "  - Milk", "  - Eggs"]
    );
    assert_eq!(focused_rows(&mut app), ["  - Eggs"]);
    assert_eq!(
        status_line(&mut app),
        format!("{} · Reloaded (changed on disk)", app.status(80).trim_end())
    );
    app.wait(Duration::from_secs(10));
    assert_eq!(status_line(&mut app), app.status(80).trim_end());
}

// F-FILE-06, file-io.md §3
#[test]
fn coming_back_to_the_terminal_checks_the_file() {
    let mut app = Harness::with_text(GROCERIES, 50, 6);
    app.settle();
    fs::write(path(&app), "Groceries:\n\t- Bread\n").unwrap();
    app.send(Input::Terminal(Event::FocusGained));
    app.settle();
    assert_eq!(outline_rows(&mut app), ["Groceries:", "  - Bread", ""]);
}

// F-FILE-06, file-io.md §3
#[test]
fn a_deleted_file_is_kept_and_marked_not_on_disk() {
    let mut app = Harness::with_text(GROCERIES, 60, 6);
    app.settle();
    fs::remove_file(path(&app)).unwrap();
    check(&mut app);
    assert_eq!(
        outline_rows(&mut app),
        ["Groceries:", "  - Milk", "  - Eggs"]
    );
    assert_eq!(
        status_line(&mut app),
        format!("{} · not on disk", app.status(60).trim_end())
    );
}

// F-FILE-06, F-FILE-07, OQ-16
#[test]
fn a_file_that_cannot_be_reloaded_asks_which_version_to_keep() {
    let mut app = Harness::with_text(GROCERIES, 140, 8);
    app.settle();
    let theirs = b"Groceries:\n\t- Caf\xe9\n\t- Milk\n".to_vec();
    fs::write(path(&app), &theirs).unwrap();
    check(&mut app);
    let row = |text: &str| format!("│{text:78}│");
    let top = format!("┌ The file changed on disk {}┐", "─".repeat(52));
    let bottom = format!("└{}┘", "─".repeat(78));
    assert_eq!(
        middle(&mut app)[1..6],
        [
            "─".repeat(80),
            top.clone(),
            row("The file on disk is not UTF-8, so it cannot be reloaded."),
            row("m keep mine · t take theirs · d show diff"),
            bottom.clone(),
        ]
    );
    // The prompt dims the outline's focus marker (D-69), so the focus is checked once it closes.
    app.type_keys("jx");
    app.type_keys("d");
    let diff = [
        top,
        row("- on disk  + yours"),
        row("  Groceries:"),
        row("-   - Caf\u{fffd}"),
        row("    - Milk"),
        row("m keep mine · t take theirs · d hide diff"),
        bottom,
    ];
    assert_eq!(middle(&mut app)[..7], diff);
    app.type_keys("jj");
    let scrolled = middle(&mut app);
    assert_eq!(
        scrolled[2..5],
        [
            row("-   - Caf\u{fffd}"),
            row("    - Milk"),
            row("+   - Eggs")
        ],
        "the diff scrolls as far as its last line"
    );
    app.type_keys("k");
    assert_eq!(middle(&mut app)[..7], diff);
    app.type_keys("jdd");
    assert_eq!(
        middle(&mut app)[..7],
        diff,
        "showing the diff again starts at its top"
    );
    app.type_keys("t");
    assert!(
        status_line(&mut app)
            .contains("cannot take the version on disk: the file is not valid UTF-8"),
        "{}",
        status_line(&mut app)
    );
    app.type_keys("m");
    app.settle();
    assert_eq!(
        conflict_copies(&app),
        [("2026-10-05-093000-conflict.taskpaper".to_owned(), theirs)]
    );
    assert_eq!(fs::read_to_string(path(&app)).unwrap(), GROCERIES);
    assert!(
        status_line(&mut app).contains("Kept your version; the one on disk is in "),
        "{}",
        status_line(&mut app)
    );
    assert_eq!(
        outline_rows(&mut app)[..3],
        ["Groceries:", "  - Milk", "  - Eggs"]
    );
    assert_eq!(
        focused_rows(&mut app),
        ["Groceries:"],
        "keys answer the prompt"
    );
}

// F-FILE-06, OQ-16, D-99
#[test]
fn a_file_turned_not_utf8_under_unsaved_changes_says_they_are_not_saved() {
    let mut app = Harness::with_text(GROCERIES, 140, 8);
    app.settle();
    app.type_keys("jx");
    fs::write(path(&app), b"caf\xe9\n").unwrap();
    check(&mut app);
    assert_eq!(
        middle(&mut app)[3],
        format!("│{:78}│", "Your changes are not saved yet.")
    );
}

// F-FILE-05, file-io.md §3
#[test]
fn quitting_while_asked_keeps_both_versions() {
    let mut app = Harness::with_text(GROCERIES, 60, 12);
    app.settle();
    fs::write(path(&app), b"caf\xe9\n").unwrap();
    check(&mut app);
    app.press(KeyCode::Char('q'), KeyModifiers::CONTROL);
    assert!(!app.tui.should_quit(), "the copy is written first");
    app.settle();
    assert!(app.tui.should_quit());
    assert_eq!(
        conflict_copies(&app),
        [(
            "2026-10-05-093000-conflict.taskpaper".to_owned(),
            GROCERIES.as_bytes().to_vec()
        )]
    );
    assert_eq!(fs::read(path(&app)).unwrap(), b"caf\xe9\n");
}

/// Lets the autosave write the file, then reads it.
fn saved(app: &mut Harness) -> String {
    app.wait(Duration::from_secs(2));
    app.settle();
    fs::read_to_string(path(app)).expect("a readable file")
}

// F-FILE-06, FR-MODE-01, D-104
#[test]
fn typing_carries_on_through_a_reload() {
    let mut app = Harness::with_text(GROCERIES, 80, 7);
    app.settle();
    app.type_keys("j");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    app.type_keys(" and");
    assert_eq!(saved(&mut app), "Groceries:\n\t- Milk and\n\t- Eggs\n");
    fs::write(path(&app), "Groceries:\n\t- Milk and\n\t- Brown eggs\n").unwrap();
    check(&mut app);
    app.type_keys(" honey");
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(
        saved(&mut app),
        "Groceries:\n\t- Milk and honey\n\t- Brown eggs\n"
    );
}

// F-FILE-06, FR-OPS-05, D-104
#[test]
fn move_to_stays_open_through_a_reload() {
    let mut app = Harness::with_text("Inbox:\n\t- Milk\nHome:\n", 80, 12);
    app.settle();
    app.type_keys("jgm");
    fs::write(path(&app), "Inbox:\n\t- Milk\nHome:\n\t- Shelf\n").unwrap();
    check(&mut app);
    app.type_keys("home");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(saved(&mut app), "Inbox:\nHome:\n\t- Shelf\n\t- Milk\n");
}
