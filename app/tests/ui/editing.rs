//! Step 2.5: editing a one-line block's text in place.

use std::{fs, os::unix::fs::PermissionsExt as _, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Harness, focused_rows, line, outline_rows};

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

fn press(app: &mut Harness, code: KeyCode, times: usize) {
    for _ in 0..times {
        app.press(code, KeyModifiers::NONE);
    }
}

/// Checks that the terminal's cursor, the caret, is at column `x` of screen row `y`.
#[track_caller]
fn assert_caret(app: &mut Harness, x: u16, y: u16) {
    app.render();
    app.terminal.backend_mut().assert_cursor_position((x, y));
}

// FR-MODE-01, FR-MODE-02, FR-MODE-03, FR-DOC-02
#[test]
fn enter_edits_a_task_in_place_and_esc_returns_with_it_renamed_and_saved() {
    let mut app = Harness::open("groceries.taskpaper", 50, 10);
    app.settle();
    app.type_keys("j");
    press(&mut app, KeyCode::Enter, 1);
    assert!(status_line(&mut app).starts_with("EDIT · "));
    assert_caret(&mut app, 10, 3);
    press(&mut app, KeyCode::Backspace, 4);
    app.type_keys("Oat milk");
    assert_eq!(outline_rows(&mut app)[1], "  - Oat milk");
    assert_caret(&mut app, 14, 3);
    assert_eq!(saved(&mut app), GROCERIES.replace("Milk", "Oat milk"));
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(status_line(&mut app), app.status(50).trim_end());
    assert_eq!(focused_rows(&mut app), ["  - Oat milk"]);
    app.type_keys("j");
    assert_eq!(focused_rows(&mut app), ["  - Bread"], "keys navigate again");
}

// FR-MODE-01
#[test]
fn i_puts_the_caret_at_the_start_and_enter_also_ends_editing() {
    let mut app = Harness::open("groceries.taskpaper", 50, 10);
    app.type_keys("ji");
    assert_caret(&mut app, 6, 3);
    app.type_keys("Soy ");
    press(&mut app, KeyCode::Enter, 1);
    assert!(status_line(&mut app).starts_with("NAVIGATE"));
    assert_eq!(outline_rows(&mut app)[1], "  - Soy Milk");
}

// FR-MODE-03
#[test]
fn text_keys_move_select_and_delete_by_grapheme_and_word() {
    let mut app = Harness::with_text("- one two three\n", 40, 5);
    press(&mut app, KeyCode::Enter, 1);
    app.press(KeyCode::Left, KeyModifiers::CONTROL);
    app.press(KeyCode::Char('w'), KeyModifiers::CONTROL);
    assert_eq!(outline_rows(&mut app)[0], "- one three");
    app.press(KeyCode::Char('b'), KeyModifiers::ALT);
    app.press(KeyCode::Right, KeyModifiers::SHIFT);
    app.press(KeyCode::Right, KeyModifiers::SHIFT);
    app.type_keys("O");
    assert_eq!(outline_rows(&mut app)[0], "- Oe three");
    press(&mut app, KeyCode::Home, 1);
    press(&mut app, KeyCode::Delete, 1);
    app.press(KeyCode::Char('f'), KeyModifiers::ALT);
    app.press(KeyCode::Backspace, KeyModifiers::ALT);
    assert_eq!(outline_rows(&mut app)[0], "-  three");
    app.press(KeyCode::Char('a'), KeyModifiers::CONTROL);
    app.type_keys("é😀");
    press(&mut app, KeyCode::Left, 1);
    press(&mut app, KeyCode::Backspace, 1);
    press(&mut app, KeyCode::End, 1);
    app.press(KeyCode::Home, KeyModifiers::SHIFT);
    app.type_keys("ok");
    app.press(KeyCode::Left, KeyModifiers::CONTROL | KeyModifiers::SHIFT);
    app.press(KeyCode::Right, KeyModifiers::CONTROL | KeyModifiers::SHIFT);
    app.press(KeyCode::End, KeyModifiers::SHIFT);
    app.press(KeyCode::Right, KeyModifiers::CONTROL);
    assert_eq!(outline_rows(&mut app)[0], "- ok");
    assert_caret(&mut app, 6, 2);
}

// FR-TAG-01, OQ-24
#[test]
fn tags_stay_hidden_and_follow_the_edited_text() {
    let mut app = Harness::with_text("- Call @due(2026-10-09) Ana @home\n", 40, 5);
    press(&mut app, KeyCode::Enter, 1);
    assert_eq!(outline_rows(&mut app)[0], "- Call Ana");
    app.type_keys(" back");
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(outline_rows(&mut app)[0], "- Call Ana back");
    assert_eq!(saved(&mut app), "- Call Ana back @due(2026-10-09) @home\n");
}

// OQ-25
#[test]
fn an_at_sign_starting_a_word_is_refused_with_a_hint() {
    let mut app = Harness::with_text("- Mail\n", 100, 5);
    press(&mut app, KeyCode::Enter, 1);
    app.type_keys(" @me@x.org");
    assert_eq!(outline_rows(&mut app)[0], "- Mail me@x.org");
    assert!(
        status_line(&mut app).ends_with(" · Tags are set with commands"),
        "{}",
        status_line(&mut app)
    );
}

// OQ-27
#[test]
fn the_marker_follows_the_type_the_text_gives() {
    let mut app = Harness::with_text("Ideas\n", 40, 5);
    press(&mut app, KeyCode::Enter, 1);
    app.type_keys(":");
    let buffer = app.render().buffer().clone();
    assert_eq!(
        buffer[(2, 2)].style().fg,
        Some(ratatui::style::Color::LightBlue)
    );
    app.press(KeyCode::Char('a'), KeyModifiers::CONTROL);
    app.type_keys("- Call");
    assert_eq!(outline_rows(&mut app)[0], "- Call");
    assert_caret(&mut app, 8, 2);
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(saved(&mut app), "- Call\n");
}

// FR-OPS-08, keybindings.md §3
#[test]
fn ctrl_z_undoes_typing_and_z_undoes_the_whole_session() {
    let mut app = Harness::open("groceries.taskpaper", 50, 10);
    app.type_keys("j");
    press(&mut app, KeyCode::Enter, 1);
    app.type_keys(" x");
    press(&mut app, KeyCode::Backspace, 1);
    app.press(KeyCode::Char('z'), KeyModifiers::CONTROL);
    assert_eq!(outline_rows(&mut app)[1], "  - Milk x");
    app.press(KeyCode::Char('y'), KeyModifiers::CONTROL);
    app.type_keys("2");
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(outline_rows(&mut app)[1], "  - Milk 2");
    app.type_keys("z");
    assert_eq!(outline_rows(&mut app)[1], "  - Milk");
    assert_eq!(saved(&mut app), GROCERIES);
    app.type_keys("Z");
    assert_eq!(outline_rows(&mut app)[1], "  - Milk 2");
}

// file-format §3.1
#[test]
fn an_emptied_parent_is_not_saved_until_leaving_gives_it_the_placeholder() {
    let mut app = Harness::with_text("Notes\n\t- a\n", 40, 5);
    app.settle();
    press(&mut app, KeyCode::Enter, 1);
    app.press(KeyCode::Char('a'), KeyModifiers::CONTROL);
    press(&mut app, KeyCode::Backspace, 1);
    assert_eq!(outline_rows(&mut app)[0], "");
    assert_eq!(saved(&mut app), "Notes\n\t- a\n", "saving waits");
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(outline_rows(&mut app)[0], "…");
    assert_eq!(saved(&mut app), "…\n\t- a\n");
}

// file-io.md §2
#[test]
fn ctrl_q_in_edit_mode_quits_and_saves_the_edit() {
    let mut app = Harness::with_text("- a\n", 40, 5);
    press(&mut app, KeyCode::Enter, 1);
    app.type_keys("b");
    app.press(KeyCode::Char('q'), KeyModifiers::CONTROL);
    app.settle();
    assert!(app.tui.should_quit());
    assert_eq!(
        fs::read_to_string(app.path.as_ref().unwrap()).unwrap(),
        "- ab\n"
    );
}

// file-io.md §1
#[test]
fn a_read_only_file_refuses_edit_mode() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("todo.taskpaper");
    fs::write(&path, "- Milk\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
    let mut app = Harness::open_path(dir, &path, 90, 4);
    press(&mut app, KeyCode::Enter, 1);
    app.type_keys("x");
    assert!(status_line(&mut app).ends_with("read-only · cannot edit a read-only file"));
    assert_eq!(outline_rows(&mut app), ["- Milk"], "x was not typed");
}

// FR-DOC-05, FR-MODE-02
#[test]
fn the_demo_has_a_task_to_rename() {
    let mut app = Harness::demo(50, 12);
    app.type_keys("j");
    press(&mut app, KeyCode::Enter, 1);
    app.type_keys(" to Lisbon");
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(outline_rows(&mut app)[1], "  - Book flights to Lisbon");
    assert!(saved(&mut app).contains("\t- Book flights to Lisbon\n"));
}
