//! Step 2.7: adding a project or task with one key.

use std::{fs, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Harness, outline_rows};

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

// FR-MODE-06, OQ-29, FR-DOC-02, D-53
#[test]
fn a_place_key_and_a_type_letter_add_a_block_of_that_type_there() {
    let mut app = Harness::open("groceries.taskpaper", 50, 12);
    app.settle();
    app.type_keys("jj");
    shifted(&mut app, 'A');
    app.type_keys("t");
    assert_caret(&mut app, 6, 3);
    app.type_keys("Eggs");
    esc(&mut app);
    app.type_keys("op");
    app.type_keys("Bakery");
    esc(&mut app);
    app.type_keys("ot");
    app.type_keys("Rye");
    esc(&mut app);
    assert_eq!(
        outline_rows(&mut app)[..5],
        ["Groceries:", "  - Eggs", "  Bakery:", "  - Rye", "  - Milk"]
    );
    app.type_keys("G");
    shifted(&mut app, 'A');
    app.type_keys("p");
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

// FR-MODE-04, FR-MODE-06, D-53
#[test]
fn a_and_shift_a_add_at_the_end_and_start_of_the_list_and_a_lone_key_waits() {
    let mut app = Harness::with_text("- a\n- b\n", 40, 10);
    app.settle();
    app.type_keys("as");
    app.type_keys("Later");
    esc(&mut app);
    app.type_keys("gg");
    app.type_keys("In");
    app.type_keys("Why");
    esc(&mut app);
    app.type_keys("a");
    app.wait(Duration::from_secs(1));
    app.type_keys("c");
    esc(&mut app);
    shifted(&mut app, 'A');
    app.type_keys("Q");
    esc(&mut app);
    assert_eq!(
        saved(&mut app),
        "- a\n\tQ\n\tWhy\n\tc\n- b\nLater @section\n",
        "a and A act on the list of the block just added"
    );
}
