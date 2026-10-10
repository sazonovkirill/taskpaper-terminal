//! Step 2.11: the file changed, deleted or replaced by another program while the app has it open.

use std::{fs, path::PathBuf, time::Duration};

use crossterm::event::{Event, KeyCode, KeyModifiers};
use taskpaper_terminal::{app::Message, ui::Input};

use crate::support::{Harness, NAVIGATE, focused_rows, line, outline_rows};

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
        app.status_with(80, " · Reloaded (changed on disk)")
            .trim_end()
            .to_owned()
    );
    app.wait(Duration::from_secs(10));
    assert_eq!(status_line(&mut app), app.status(80).trim_end());
}

// ui.md §1: a pending key prefix takes its room from the path first, and a message cut for it
// ends in `…` rather than being drawn over.
#[test]
fn a_pending_prefix_leaves_a_message_whole_or_cut_with_an_ellipsis() {
    let mut app = Harness::with_text(GROCERIES, 60, 7);
    app.settle();
    fs::write(path(&app), "Groceries:\n\t- Bread\n").unwrap();
    check(&mut app);
    app.type_keys("g");
    assert_eq!(
        status_line(&mut app),
        app.status_with(60, " · Reloaded (changed on disk) g")
            .trim_end()
    );

    let mut app = Harness::with_text(GROCERIES, 30, 7);
    app.settle();
    fs::write(path(&app), "Groceries:\n\t- Bread\n").unwrap();
    check(&mut app);
    app.press(KeyCode::Char('w'), KeyModifiers::CONTROL);
    let status = status_line(&mut app);
    assert!(
        status.ends_with("NAVIGATE · … · Reloa… Ctrl+W"),
        "the message ends in … before the prefix: {status}"
    );
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
        app.status_with(60, " · not on disk").trim_end().to_owned()
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
            row("Ctrl+K keep mine · Ctrl+R take theirs · Ctrl+L show diff"),
            bottom.clone(),
        ]
    );
    // The prompt dims the outline's focus marker (D-69), so the focus is checked once it closes.
    app.type_keys("jxmtd");
    app.press(KeyCode::Char('l'), KeyModifiers::CONTROL);
    let diff = [
        top,
        row("- on disk  + yours"),
        row("  Groceries:"),
        row("-   - Caf\u{fffd}"),
        row("    - Milk"),
        row("Ctrl+K keep mine · Ctrl+R take theirs · Ctrl+L hide diff"),
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
    app.type_keys("j");
    app.press(KeyCode::Char('l'), KeyModifiers::CONTROL);
    app.press(KeyCode::Char('l'), KeyModifiers::CONTROL);
    assert_eq!(
        middle(&mut app)[..7],
        diff,
        "showing the diff again starts at its top"
    );
    app.press(KeyCode::Char('r'), KeyModifiers::CONTROL);
    assert!(
        status_line(&mut app).contains("cannot take theirs: the file is not valid UTF-8"),
        "{}",
        status_line(&mut app)
    );
    app.press(KeyCode::Char('k'), KeyModifiers::CONTROL);
    app.settle();
    assert_eq!(
        conflict_copies(&app),
        [("2026-10-05-093000-conflict.taskpaper".to_owned(), theirs)]
    );
    assert_eq!(fs::read_to_string(path(&app)).unwrap(), GROCERIES);
    assert!(
        status_line(&mut app).contains("Kept mine; theirs is in "),
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
    let copy = app
        .backups
        .read_dir()
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path()
        .join("2026-10-05-093000-conflict.taskpaper");
    assert_eq!(
        app.tui.farewell(),
        Some(format!(
            "tpt: left theirs on disk; mine is in {}",
            copy.display()
        )),
        "the quit says where the changes went"
    );
}

// F-FILE-05, file-io.md §3–4: when the conflict copy cannot be written, quitting keeps mine in a
// recovery file instead, and choosing says which folder failed.
#[test]
fn quitting_while_asked_keeps_mine_in_a_recovery_file_when_the_copy_fails() {
    let mut app = Harness::with_text(GROCERIES, 200, 12);
    app.settle();
    let _ = fs::remove_dir_all(&app.backups);
    fs::write(&app.backups, "a file where the folder goes").unwrap();
    fs::write(path(&app), b"caf\xe9\n").unwrap();
    check(&mut app);
    app.press(KeyCode::Char('k'), KeyModifiers::CONTROL);
    app.settle();
    let status = status_line(&mut app);
    assert!(
        status.contains(&format!(
            "conflict copy failed in {}: Not a directory",
            app.backups.display()
        )),
        "{status}"
    );
    app.press(KeyCode::Char('q'), KeyModifiers::CONTROL);
    app.settle();
    assert!(app.tui.should_quit(), "quitting does not ask");
    assert_eq!(fs::read(path(&app)).unwrap(), b"caf\xe9\n");
    let recovery = app.backups.with_file_name("state").join("recovery");
    let recovered: Vec<_> = recovery
        .read_dir()
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(recovered.len(), 1);
    assert_eq!(fs::read_to_string(&recovered[0]).unwrap(), GROCERIES);
    assert_eq!(
        app.tui.farewell(),
        Some(format!(
            "tpt: left theirs on disk; mine is in {}",
            recovered[0].display()
        ))
    );
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

// F-FILE-06, OQ-16, D-115
#[test]
fn typing_on_when_the_prompt_opens_chooses_nothing() {
    let mut app = Harness::with_text("A:\n\t- x\n", 140, 8);
    app.settle();
    app.type_keys("j");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    app.type_keys(" notes for");
    fs::write(path(&app), "A:\n\t- x\n\t- from phone\n").unwrap();
    check(&mut app);
    app.type_keys(" the meeting");
    assert_eq!(
        middle(&mut app)[3],
        format!("│{:78}│", "Your changes are not saved yet."),
        "the prompt stays open"
    );
    assert!(conflict_copies(&app).is_empty(), "no choice was made");
    assert_eq!(
        fs::read_to_string(path(&app)).unwrap(),
        "A:\n\t- x\n\t- from phone\n"
    );
}

// F-FILE-06, OQ-16, D-104
#[test]
fn keeping_mine_carries_on_typing_where_it_was() {
    let mut app = Harness::with_text(GROCERIES, 80, 7);
    app.settle();
    app.type_keys("j");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    app.type_keys(" and");
    fs::write(path(&app), "Groceries:\n\t- Bread\n").unwrap();
    check(&mut app);
    app.press(KeyCode::Char('k'), KeyModifiers::CONTROL);
    app.settle();
    app.type_keys(" honey");
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(
        saved(&mut app),
        "Groceries:\n\t- Milk and honey\n\t- Eggs\n"
    );
}

// F-FILE-06, OQ-16
#[test]
fn a_narrow_conflict_prompt_shows_every_key() {
    let mut app = Harness::with_text(GROCERIES, 28, 8);
    app.settle();
    app.type_keys("jx");
    fs::write(path(&app), "Groceries:\n\t- Bread\n").unwrap();
    check(&mut app);
    let buffer = app.render().buffer().clone();
    let screen: Vec<String> = (0..8).map(|y| line(&buffer, y)).collect();
    for key in ["Ctrl+K keep mine", "Ctrl+R take theirs", "Ctrl+L show diff"] {
        assert!(
            screen.iter().any(|row| row.contains(key)),
            "{key} in {screen:#?}"
        );
    }
}

// file-io.md §3, ui.md §1
#[test]
fn a_message_too_long_for_the_status_line_ends_in_an_ellipsis() {
    let mut app = Harness::with_text(GROCERIES, 60, 8);
    app.settle();
    fs::write(path(&app), b"caf\xe9\n").unwrap();
    check(&mut app);
    app.press(KeyCode::Char('r'), KeyModifiers::CONTROL);
    assert_eq!(
        status_line(&mut app),
        format!("{NAVIGATE} · … · cannot take theirs: the file is not valid …")
    );
}

// file-io.md §3, ui.md §1
#[test]
fn the_conflict_copys_path_keeps_its_file_name_in_view() {
    for (key, words) in [
        ('k', "Kept mine; theirs is in …"),
        ('r', "Took theirs; mine is in …"),
    ] {
        let mut app = Harness::with_text(GROCERIES, 80, 8);
        app.settle();
        app.type_keys("jx");
        fs::write(path(&app), "Groceries:\n\t- Bread\n").unwrap();
        check(&mut app);
        app.press(KeyCode::Char(key), KeyModifiers::CONTROL);
        app.settle();
        let status = status_line(&mut app);
        assert!(
            status.contains(words) && status.ends_with("/2026-10-05-093000-conflict.taskpaper"),
            "{status}"
        );
    }
}

// F-FILE-06, OQ-16, D-104, D-123
#[test]
fn taking_theirs_carries_on_typing_where_it_was() {
    let mut app = Harness::with_text(GROCERIES, 80, 7);
    app.settle();
    app.type_keys("j");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    app.type_keys(" and");
    app.press(KeyCode::Down, KeyModifiers::NONE);
    fs::write(path(&app), "Groceries:\n\t- Milk\n\t- Eggs\n\t- Bread\n").unwrap();
    check(&mut app);
    app.press(KeyCode::Char('r'), KeyModifiers::CONTROL);
    app.settle();
    app.type_keys(" and ham");
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(
        saved(&mut app),
        "Groceries:\n\t- Milk\n\t- Eggs and ham\n\t- Bread\n"
    );
}

// F-FILE-06, OQ-16, D-123: the block is found again as it was on disk, so typing in it is not
// what keeps it from being found.
#[test]
fn taking_theirs_carries_on_typing_in_the_block_whose_text_was_unsaved() {
    let mut app = Harness::with_text(GROCERIES, 80, 7);
    app.settle();
    app.type_keys("j");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    app.type_keys(" mine");
    fs::write(path(&app), "Groceries:\n\t- Milk\n\t- Eggs\n\t- Bread\n").unwrap();
    check(&mut app);
    app.press(KeyCode::Char('r'), KeyModifiers::CONTROL);
    app.settle();
    app.type_keys("Q");
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(
        saved(&mut app),
        "Groceries:\n\t- MilkQ\n\t- Eggs\n\t- Bread\n"
    );
}
