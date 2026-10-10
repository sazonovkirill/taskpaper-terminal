//! Step 1.1: opening a file, showing it as an outline, the demo, and quitting. Step 2.10: the
//! demo in a new temporary file.

use std::{fs, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{buffer::Buffer, layout::Rect, style::Style};
use taskpaper_terminal::app::demo;

use crate::support::{BAND_TEXT, BOOKMARK, CURRENT_LINE, Harness, NAVIGATE, TEXT, TODAY, line};

// F-FILE-01, F-FILE-02, FR-DOC-01, FR-DOC-03
#[test]
fn shows_the_file_as_an_indented_outline() {
    let mut app = Harness::open("groceries.taskpaper", 36, 10);
    let mut expected = Buffer::with_lines([
        "  ⌂                                 ",
        "────────────────────────────────────",
        "▌ Groceries:                        ",
        "    - Milk                          ",
        "    - Bread                         ",
        "      Wholegrain if they have it.   ",
        "    Dairy:                          ",
        "      - Cheese                      ",
        "  Notes about the week.             ",
        &app.status(36),
    ]);
    let project = Style::new().bold().light_blue();
    expected.set_style(Rect::new(2, 0, 34, 1), Style::new().bold());
    expected.set_style(Rect::new(0, 1, 36, 1), Style::new().dark_gray());
    expected.set_style(
        Rect::new(0, 2, 36, 1),
        Style::new().bg(CURRENT_LINE).fg(BAND_TEXT),
    );
    expected.set_style(Rect::new(2, 2, 10, 1), project);
    expected.set_style(Rect::new(6, 5, 27, 1), Style::new().fg(TEXT));
    expected.set_style(Rect::new(4, 6, 6, 1), project);
    expected.set_style(Rect::new(2, 8, 21, 1), Style::new().fg(TEXT));
    expected.set_style(Rect::new(0, 2, 1, 1), Style::new().light_blue());
    expected.set_style(Rect::new(0, 9, 10, 1), Style::new().bold());
    app.render().assert_buffer(&expected);
}

// FR-DOC-05
#[test]
fn shows_the_demo_outline_without_a_path() {
    let mut app = Harness::demo(40, 6);
    let mut expected = Buffer::with_lines([
        "  ⌂                                     ",
        "────────────────────────────────────────",
        &format!("{:<40}", format!("▌ Trip to Lisbon: {BOOKMARK}")),
        "    - Book flights                      ",
        "    - Reserve a hotel near Alfama       ",
        &app.status(40),
    ]);
    expected.set_style(Rect::new(2, 0, 38, 1), Style::new().bold());
    expected.set_style(Rect::new(0, 1, 40, 1), Style::new().dark_gray());
    expected.set_style(
        Rect::new(0, 2, 40, 1),
        Style::new().bg(CURRENT_LINE).fg(BAND_TEXT),
    );
    expected.set_style(Rect::new(2, 2, 15, 1), Style::new().bold().light_blue());
    expected.set_style(Rect::new(18, 2, 1, 1), Style::new().light_blue());
    expected.set_style(Rect::new(0, 2, 1, 1), Style::new().light_blue());
    expected.set_style(Rect::new(0, 5, 10, 1), Style::new().bold());
    app.render().assert_buffer(&expected);
}

#[test]
fn ctrl_q_quits() {
    let mut app = Harness::demo(20, 2);
    assert!(!app.tui.should_quit());
    app.press(KeyCode::Char('q'), KeyModifiers::NONE);
    assert!(!app.tui.should_quit(), "plain q must not quit");
    app.wait(Duration::from_secs(1));
    app.press(KeyCode::Char('q'), KeyModifiers::CONTROL);
    assert!(app.tui.should_quit());
}

// FR-DOC-05, D-27, file-io.md §1
#[test]
fn the_demo_is_a_new_temporary_file_marked_in_the_status_line() {
    let mut app = Harness::demo(160, 3);
    let path = app.path.clone().expect("the demo has a file");
    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some("taskpaper-20261005-123000.taskpaper")
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), demo::text(TODAY));
    let buffer = app.render().buffer().clone();
    assert_eq!(
        line(&buffer, 2),
        format!("{NAVIGATE} · {} · temporary", path.display())
    );
    assert_eq!(app.status(160).trim_end(), line(&buffer, 2));
}

// ui.md §1
#[test]
fn the_status_line_reads_the_home_folder_as_a_tilde() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("home/notes/todo.taskpaper");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "- a\n").unwrap();
    let mut app = Harness::open_path(dir, &path, 60, 3);
    let buffer = app.render().buffer().clone();
    assert_eq!(
        line(&buffer, 2),
        format!("{NAVIGATE} · ~/notes/todo.taskpaper")
    );
}

// ui.md §1
#[test]
fn a_path_too_long_for_the_status_line_gives_its_room_to_a_message() {
    let mut app = Harness::with_text("- a\n", 60, 3);
    app.type_keys("qi");
    let buffer = app.render().buffer().clone();
    assert_eq!(
        line(&buffer, 2),
        format!("{NAVIGATE} · ….taskpaper · No INBOX project in the Navigator")
    );
    app.terminal.backend_mut().resize(40, 3);
    let buffer = app.render().buffer().clone();
    assert_eq!(
        line(&buffer, 2),
        format!("{NAVIGATE} · … · No INBOX project in th…"),
        "the path keeps only its ellipsis, and the cut message ends in one"
    );
}

// FR-DOC-05, file-io.md §2
#[test]
fn the_temporary_file_is_not_backed_up() {
    let mut app = Harness::demo(40, 3);
    app.settle();
    assert!(!app.backups.exists(), "a backup was made");
}
