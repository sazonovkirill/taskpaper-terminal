//! Step 2.7: adding a project, section or task with one key.

use std::{fs, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Harness, focused_rows, outline_rows};

const GROCERIES: &str = "Groceries: @home\n\t- Milk\n\t- Bread\n\t\tWholegrain if they have it.\n\tDairy:\n\t\t- Cheese\nNotes about the week.\n";

/// The open file's text once the autosave delay has passed and its save is done.
fn saved(app: &mut Harness) -> String {
    app.wait(Duration::from_secs(1));
    app.settle();
    let path = app.path.as_ref().expect("a file was opened");
    fs::read_to_string(path).expect("the file is readable")
}

/// Checks that the terminal's cursor, the caret, is at column `x` of screen row `y`.
#[track_caller]
fn assert_caret(app: &mut Harness, x: u16, y: u16) {
    app.render();
    app.terminal.backend_mut().assert_cursor_position((x, y));
}

/// Presses the upper-case `c` as a terminal reports it, with `Shift`.
fn shifted(app: &mut Harness, c: char) {
    app.press(KeyCode::Char(c), KeyModifiers::SHIFT);
}

fn esc(app: &mut Harness) {
    app.press(KeyCode::Esc, KeyModifiers::NONE);
}

// FR-MODE-06, OQ-29, FR-DOC-05
#[test]
fn shift_s_in_the_demo_adds_a_section_after_the_tasks_of_the_list() {
    let mut app = Harness::demo(60, 20);
    app.settle();
    app.type_keys("j");
    shifted(&mut app, 'S');
    assert_caret(&mut app, 4, 8);
    app.type_keys("Day trips");
    esc(&mut app);
    assert_eq!(
        outline_rows(&mut app)[5..8],
        ["  ↗ Lisbon travel guide", "  Day trips", "  Packing"]
    );
    assert_eq!(focused_rows(&mut app), ["  Day trips"]);
    assert!(
        saved(&mut app).contains("\tDay trips @section\n\tPacking @section\n"),
        "saved as a section"
    );
}

// FR-MODE-06, OQ-29, FR-DOC-02
#[test]
fn shift_a_and_shift_p_add_at_the_top_of_the_list_and_p_and_a_below() {
    let mut app = Harness::open("groceries.taskpaper", 50, 12);
    app.settle();
    app.type_keys("jj");
    shifted(&mut app, 'A');
    assert_caret(&mut app, 6, 3);
    app.type_keys("Eggs");
    esc(&mut app);
    app.type_keys("p");
    app.type_keys("Bakery");
    esc(&mut app);
    app.type_keys("a");
    app.type_keys("Rye");
    esc(&mut app);
    assert_eq!(
        outline_rows(&mut app)[..5],
        ["Groceries:", "  - Eggs", "  Bakery:", "  - Rye", "  - Milk"]
    );
    app.type_keys("G");
    shifted(&mut app, 'P');
    app.type_keys("Inbox");
    esc(&mut app);
    assert_eq!(outline_rows(&mut app)[..2], ["Inbox:", "Groceries:"]);
    assert_eq!(
        saved(&mut app),
        format!(
            "Inbox:\n{}",
            GROCERIES.replace("\t- Milk", "\t- Eggs\n\tBakery:\n\t- Rye\n\t- Milk")
        )
    );
}
