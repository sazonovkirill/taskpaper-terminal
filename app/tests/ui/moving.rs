//! Step 2.9: indenting, outdenting and moving blocks.

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

// FR-OPS-03, FR-DOC-02
#[test]
fn alt_j_and_alt_k_reorder_a_list_and_cross_into_the_neighboring_parent() {
    let mut app = Harness::open("groceries.taskpaper", 50, 12);
    app.type_keys("j");
    app.press(KeyCode::Char('j'), KeyModifiers::ALT);
    assert_eq!(
        outline_rows(&mut app)[..4],
        [
            "Groceries:",
            "  - Bread",
            "    Wholegrain if they have it.",
            "  - Milk"
        ]
    );
    assert_eq!(focused_rows(&mut app), ["  - Milk"]);
    app.type_keys("Gk");
    app.press(KeyCode::Char('k'), KeyModifiers::ALT);
    assert_eq!(
        outline_rows(&mut app)[3..7],
        [
            "  - Milk",
            "    - Cheese",
            "  Dairy:",
            "Notes about the week."
        ],
        "the first of its list goes last into the one before"
    );
    app.press(KeyCode::Down, KeyModifiers::ALT);
    assert_eq!(
        outline_rows(&mut app)[3..6],
        ["  - Milk", "  Dairy:", "    - Cheese"],
        "and back first into the one after"
    );
    app.press(KeyCode::Up, KeyModifiers::ALT);
    assert_eq!(focused_rows(&mut app), ["    - Cheese"]);
    assert_eq!(
        saved(&mut app),
        "Groceries: @home\n\t- Bread\n\t\tWholegrain if they have it.\n\t- Milk\n\t\t- Cheese\n\tDairy:\nNotes about the week.\n"
    );
}

// FR-OPS-02, FR-OPS-08
#[test]
fn tab_indents_shift_tab_outdents_and_z_undoes() {
    let mut app = Harness::open("groceries.taskpaper", 50, 12);
    app.type_keys("jjjj");
    app.press(KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(
        outline_rows(&mut app)[2..6],
        [
            "  - Bread",
            "    Wholegrain if they have it.",
            "    Dairy:",
            "      - Cheese"
        ]
    );
    assert_eq!(focused_rows(&mut app), ["    Dairy:"]);
    assert_eq!(
        saved(&mut app),
        GROCERIES.replace("\tDairy:\n\t\t- Cheese", "\t\tDairy:\n\t\t\t- Cheese")
    );
    app.press(KeyCode::BackTab, KeyModifiers::SHIFT);
    app.press(KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(
        outline_rows(&mut app)[4..7],
        ["Dairy:", "  - Cheese", "Notes about the week."]
    );
    app.type_keys("zzz");
    assert_eq!(saved(&mut app), GROCERIES);
}

// FR-OPS-02, editing.md §2
#[test]
fn tab_while_typing_a_new_block_indents_it_and_keeps_editing() {
    let mut app = Harness::open("groceries.taskpaper", 50, 12);
    app.type_keys("joOat");
    app.press(KeyCode::Tab, KeyModifiers::NONE);
    assert!(status_line(&mut app).starts_with("EDIT · "));
    app.type_keys(" milk");
    assert_eq!(outline_rows(&mut app)[2], "    - Oat milk");
    app.render();
    app.terminal.backend_mut().assert_cursor_position((16, 4));
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(
        outline_rows(&mut app)[1..4],
        ["  - Milk", "    - Oat milk", "  - Bread"]
    );
    assert_eq!(
        saved(&mut app),
        GROCERIES.replace("\t- Milk\n", "\t- Milk\n\t\t- Oat milk\n")
    );
}

// D-43
#[test]
fn indenting_a_text_line_after_a_text_line_leaves_them_apart() {
    let mut app = Harness::with_text("Ideas:\n\tFirst note\nSecond note\n", 200, 6);
    app.type_keys("jj");
    app.press(KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(focused_rows(&mut app), ["  Second note"]);
    assert_eq!(
        outline_rows(&mut app)[1..3],
        ["  First note", "  Second note"]
    );
}
