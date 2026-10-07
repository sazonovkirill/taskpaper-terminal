//! Step 4.4: `gm` opens Move to, a fuzzy finder over the blocks the selected blocks, else the
//! focused block, can move under; choosing one makes them its last children and the focus follows
//! them.

use std::{fs, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Harness, focused_rows, line, outline_rows};

const TEXT: &str =
    "Home:\n\t- Paint the hall\nWork:\n\t- Write report\n\t\t- Draft\n\tLater @section\n";

/// Every screen row, without trailing blanks.
fn screen(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    (buffer.area.top()..buffer.area.bottom())
        .map(|y| line(&buffer, y))
        .collect()
}

/// The open file's text once the autosave delay has passed and its save is done.
fn saved(app: &mut Harness) -> String {
    app.wait(Duration::from_secs(1));
    app.settle();
    let path = app.path.as_ref().expect("a file was opened");
    fs::read_to_string(path).expect("the file is readable")
}

// FR-OPS-05, keybindings.md §2, ui.md §1
#[test]
fn g_m_lists_every_block_but_the_focused_one_and_what_it_holds() {
    let mut app = Harness::with_text(TEXT, 60, 16);
    app.type_keys("jjj");
    app.type_keys("gm");
    assert_eq!(
        screen(&mut app)[5..12],
        [
            "┌Move to───────────────────────────────────────────────────┐",
            "│›                                                         │",
            "│ ▸ Home                                                   │",
            "│ ▸ Work                                                   │",
            "│ § Later  Work                                            │",
            "│ ☐ Paint the hall  Home                                   │",
            "└──────────────────────────────────────────────────────────┘",
        ]
    );
}

// FR-OPS-05, FR-OPS-08
#[test]
fn enter_makes_the_focused_block_the_last_child_of_the_chosen_one_and_z_undoes_it() {
    let mut app = Harness::with_text(TEXT, 60, 16);
    app.type_keys("jjj");
    app.type_keys("gmhome");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        outline_rows(&mut app)[..6],
        [
            "Home:",
            "  - Paint the hall",
            "  - Write report",
            "    - Draft",
            "Work:",
            "  Later",
        ]
    );
    assert_eq!(focused_rows(&mut app), ["  - Write report"]);
    assert_eq!(
        saved(&mut app),
        "Home:\n\t- Paint the hall\n\t- Write report\n\t\t- Draft\nWork:\n\tLater @section\n"
    );
    app.type_keys("z");
    assert_eq!(saved(&mut app), TEXT);
}

// FR-OPS-05, FR-OPS-07
#[test]
fn the_selected_blocks_move_together_and_esc_cancels() {
    let mut app = Harness::with_text(TEXT, 60, 16);
    app.type_keys("jvjjjv");
    app.type_keys("gm");
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    assert!(!screen(&mut app).iter().any(|row| row.contains("Move to")));
    assert_eq!(saved(&mut app), TEXT);
    app.type_keys("gmlater");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        saved(&mut app),
        "Home:\nWork:\n\t- Write report\n\tLater @section\n\t\t- Paint the hall\n\t\t- Draft\n"
    );
    assert_eq!(focused_rows(&mut app), ["    - Draft"]);
}

// FR-OPS-05, FR-OPS-08, D-56
#[test]
fn a_project_moves_to_home_listed_first_making_it_the_last_top_level_block() {
    let mut app = Harness::with_text(TEXT, 60, 16);
    app.type_keys("gm");
    assert_eq!(
        screen(&mut app)[5..9],
        [
            "┌Move to───────────────────────────────────────────────────┐",
            "│›                                                         │",
            "│ ⌂ Home                                                   │",
            "│ ▸ Work                                                   │",
        ]
    );
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(focused_rows(&mut app), ["Home:"]);
    assert_eq!(
        saved(&mut app),
        "Work:\n\t- Write report\n\t\t- Draft\n\tLater @section\nHome:\n\t- Paint the hall\n"
    );
    app.type_keys("z");
    assert_eq!(saved(&mut app), TEXT);
}
