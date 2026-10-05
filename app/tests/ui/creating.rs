//! Step 2.6: creating and deleting blocks.

use std::{fs, time::Duration};

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

// FR-MODE-04, FR-DOC-02, OQ-03
#[test]
fn a_new_list_is_written_from_scratch_with_o_enter_and_backspace() {
    let mut app = Harness::with_text("", 40, 8);
    app.settle();
    app.type_keys("o");
    assert!(status_line(&mut app).starts_with("EDIT · "));
    app.type_keys("Trip:");
    press(&mut app, KeyCode::Enter, 1);
    app.type_keys("- Book flights");
    press(&mut app, KeyCode::Enter, 1);
    app.type_keys("Pack");
    press(&mut app, KeyCode::Enter, 1);
    press(&mut app, KeyCode::Backspace, 1);
    assert_caret(&mut app, 8, 4);
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(
        outline_rows(&mut app)[..4],
        ["Trip:", "- Book flights", "- Pack", ""]
    );
    assert_eq!(focused_rows(&mut app), ["- Pack"]);
    assert_eq!(saved(&mut app), "Trip:\n- Book flights\n- Pack");
}

// FR-MODE-04, OQ-03
#[test]
fn shift_o_adds_a_block_above_and_alt_o_a_first_child() {
    let mut app = Harness::open("groceries.taskpaper", 50, 12);
    app.type_keys("jj");
    app.press(KeyCode::Char('O'), KeyModifiers::SHIFT);
    app.type_keys("Eggs");
    press(&mut app, KeyCode::Esc, 1);
    app.press(KeyCode::Char('o'), KeyModifiers::ALT);
    assert_caret(&mut app, 8, 5);
    app.type_keys("Free range");
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(
        outline_rows(&mut app)[..5],
        [
            "Groceries:",
            "  - Milk",
            "  - Eggs",
            "    - Free range",
            "  - Bread"
        ]
    );
    assert_eq!(
        saved(&mut app),
        GROCERIES.replace("\t- Bread", "\t- Eggs\n\t\t- Free range\n\t- Bread")
    );
}

// FR-OPS-06, FR-OPS-08
#[test]
fn dd_and_delete_remove_a_block_with_its_subtree_and_z_brings_it_back() {
    let mut app = Harness::open("groceries.taskpaper", 50, 12);
    app.type_keys("jjdd");
    assert_eq!(
        outline_rows(&mut app)[..4],
        ["Groceries:", "  - Milk", "  Dairy:", "    - Cheese"]
    );
    assert_eq!(focused_rows(&mut app), ["  Dairy:"]);
    press(&mut app, KeyCode::Delete, 1);
    assert_eq!(focused_rows(&mut app), ["Notes about the week."]);
    assert_eq!(
        saved(&mut app),
        "Groceries: @home\n\t- Milk\nNotes about the week.\n"
    );
    app.type_keys("zz");
    assert_eq!(focused_rows(&mut app), ["  - Bread"]);
    assert_eq!(saved(&mut app), GROCERIES);
}

// OQ-26
#[test]
fn deleting_the_block_between_two_notes_says_they_merged() {
    let mut app = Harness::with_text("One note\n- task\nAnother note\n", 200, 6);
    app.type_keys("jdd");
    assert_eq!(outline_rows(&mut app)[..2], ["One note", "Another note"]);
    assert_eq!(focused_rows(&mut app), ["One note", "Another note"]);
    assert!(
        status_line(&mut app).ends_with(" · Notes merged"),
        "{}",
        status_line(&mut app)
    );
}

// file-format §3.1
#[test]
fn an_empty_new_last_block_below_the_top_level_is_saved_at_the_top_level() {
    let mut app = Harness::with_text("Trip:\n\tFlights:\n", 200, 6);
    app.settle();
    app.type_keys("jo");
    assert_eq!(outline_rows(&mut app)[..3], ["Trip:", "  Flights:", ""]);
    assert_eq!(saved(&mut app), "Trip:\n\tFlights:\n", "saving waits");
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(saved(&mut app), "Trip:\n\tFlights:\n\n");
    assert!(
        status_line(&mut app).ends_with(" · Empty lines at the end moved to the top level"),
        "{}",
        status_line(&mut app)
    );
}
