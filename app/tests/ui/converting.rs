//! Step 3.7: `my` opens the type picker and converts the selected or focused blocks to the
//! chosen type.

use std::{fs, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Harness, line};

/// The open file's text once the autosave delay has passed and its save is done.
fn saved(app: &mut Harness) -> String {
    app.wait(Duration::from_secs(1));
    app.settle();
    let path = app.path.as_ref().expect("a file was opened");
    fs::read_to_string(path).expect("the file is readable")
}

fn press(app: &mut Harness, code: KeyCode) {
    app.press(code, KeyModifiers::NONE);
}

/// Every screen row, without trailing blanks.
fn screen(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    (buffer.area.top()..buffer.area.bottom())
        .map(|y| line(&buffer, y))
        .collect()
}

// FR-BLK-08, FR-BLK-04, keybindings.md §4, FR-OPS-08
#[test]
fn m_y_turns_a_text_block_into_a_section_and_z_undoes_it() {
    let mut app = Harness::with_text("Groceries @x\n\t- Milk\n", 30, 14);
    app.type_keys("my");
    assert_eq!(
        screen(&mut app)[2..12],
        [
            "▌ Groceries",
            "    - Milk",
            "         ┌Node type:┐",
            "         │ Project  │",
            "         │ Section  │",
            "         │ Note     │",
            "         │ Task     │",
            "         │ Divider  │",
            "         └──────────┘",
            "",
        ]
    );
    app.type_keys("sec");
    press(&mut app, KeyCode::Enter);
    assert!(
        !screen(&mut app)
            .iter()
            .any(|row| row.contains("Node type:"))
    );
    assert_eq!(saved(&mut app), "Groceries @x @section\n\t- Milk\n");
    app.type_keys("z");
    assert_eq!(saved(&mut app), "Groceries @x\n\t- Milk\n");
}

// FR-BLK-08, FR-BLK-06, D-45
#[test]
fn a_line_with_a_url_keeps_it_under_its_new_type() {
    let mut app = Harness::with_text("Docs @url(https://a.b)\n- Milk\n", 100, 14);
    app.type_keys("mytask");
    press(&mut app, KeyCode::Enter);
    assert_eq!(saved(&mut app), "- Docs @url(https://a.b)\n- Milk\n");
    assert_eq!(screen(&mut app)[2], "▌ - Docs");
    let buffer = app.render().buffer().clone();
    assert_eq!(
        buffer[(4, 2)].fg,
        ratatui::style::Color::Cyan,
        "still a link"
    );
}

// FR-BLK-08, FR-BLK-07, D-44
#[test]
fn a_task_becomes_a_divider_keeping_its_tags_and_esc_leaves_a_block_alone() {
    let mut app = Harness::with_text("- Milk @red\n- Bread\n", 30, 14);
    app.type_keys("mydiv");
    press(&mut app, KeyCode::Enter);
    assert_eq!(saved(&mut app), "--- @red\n- Bread\n");
    app.type_keys("jmy");
    press(&mut app, KeyCode::Up);
    press(&mut app, KeyCode::Esc);
    assert!(
        !screen(&mut app)
            .iter()
            .any(|row| row.contains("Node type:"))
    );
    assert_eq!(saved(&mut app), "--- @red\n- Bread\n");
}

// FR-BLK-08, FR-OPS-07, FR-OPS-08
#[test]
fn m_y_turns_every_selected_block_into_a_task_and_z_undoes_them_at_once() {
    let mut app = Harness::with_text("Milk\nBread\nEggs\n", 30, 14);
    app.type_keys("JJmytask");
    press(&mut app, KeyCode::Enter);
    assert_eq!(saved(&mut app), "- Milk\n- Bread\n- Eggs\n");
    app.type_keys("z");
    assert_eq!(saved(&mut app), "Milk\nBread\nEggs\n", "one step to undo");
}
