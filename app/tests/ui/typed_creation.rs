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

/// Opens Go to Command with `Ctrl+P`, types `name` and gives the best match.
fn give(app: &mut Harness, name: &str) {
    app.press(KeyCode::Char('p'), KeyModifiers::CONTROL);
    app.type_keys(name);
    app.press(KeyCode::Enter, KeyModifiers::NONE);
}

// FR-MODE-06, OQ-29, FR-DOC-02, D-53, D-136
#[test]
fn go_to_command_adds_a_block_of_a_chosen_type_at_a_place() {
    let mut app = Harness::open("groceries.taskpaper", 50, 12);
    app.settle();
    app.type_keys("jj");
    give(&mut app, "New Task at Start");
    assert_caret(&mut app, 6, 3);
    app.type_keys("Eggs");
    esc(&mut app);
    give(&mut app, "New Project Below");
    app.type_keys("Bakery");
    esc(&mut app);
    give(&mut app, "New Task Below");
    app.type_keys("Rye");
    esc(&mut app);
    assert_eq!(
        outline_rows(&mut app)[..5],
        ["Groceries:", "  - Eggs", "  Bakery:", "  - Rye", "  - Milk"]
    );
    app.type_keys("G");
    give(&mut app, "New Project at Start");
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

// FR-MODE-04, D-53, D-136
#[test]
fn a_creation_key_adds_a_block_at_once_and_what_follows_is_typed_into_it() {
    let mut app = Harness::with_text("- a\n- b\n", 40, 10);
    app.settle();
    app.type_keys("as");
    esc(&mut app);
    app.type_keys("gg");
    shifted(&mut app, 'I');
    app.type_keys("n");
    esc(&mut app);
    app.type_keys("ot");
    esc(&mut app);
    shifted(&mut app, 'A');
    app.type_keys("p");
    esc(&mut app);
    assert_eq!(
        saved(&mut app),
        "- a\n\t- p\n\t- n\n\t- t\n- b\n- s\n",
        "each key acts at once, and the letter after it is the new block's text"
    );
}
