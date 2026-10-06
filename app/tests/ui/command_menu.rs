//! The command menu: `/` in edit mode offers every command for the block, filtered as typed
//! (D-44, D-50).

use std::{fs, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Harness, focused_rows, line, outline_rows};

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

/// The status line of `app` after a draw.
fn status_line(app: &mut Harness) -> String {
    let buffer = app.render().buffer().clone();
    line(&buffer, buffer.area.bottom() - 1)
}

/// Every screen row, without trailing blanks.
fn screen(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    (buffer.area.top()..buffer.area.bottom())
        .map(|y| line(&buffer, y))
        .collect()
}

// D-50, ui.md §5
#[test]
fn slash_opens_the_commands_under_the_caret_scrolled_to_fit() {
    let mut app = Harness::with_text("- Buy milk\n- Call Ana\n", 34, 12);
    app.settle();
    press(&mut app, KeyCode::Enter);
    app.type_keys("/");
    assert_eq!(
        screen(&mut app)[2..11],
        [
            "▌ - Buy milk",
            "  - Call Ana┌Commands:───────┐",
            "            │ Project        │",
            "            │ Task           │",
            "            │ Note           │",
            "            │────────────────│",
            "            │ Complete       │",
            "            │ Toggle Today   │",
            "            └────────────────┘",
        ],
        "as tall as the room below the caret"
    );
    for _ in 0..10 {
        app.press(KeyCode::Char('n'), KeyModifiers::CONTROL);
    }
    assert_eq!(
        screen(&mut app)[4..10],
        [
            "            │ Set Due Date   │",
            "            │ Start Today    │",
            "            │ Start Tomorrow │",
            "            │ Due Today      │",
            "            │ Due Tomorrow   │",
            "            │────────────────│",
        ],
        "the highlight stays in view, skipping the lines between groups"
    );
    press(&mut app, KeyCode::Enter);
    assert!(status_line(&mut app).starts_with("EDIT"));
    press(&mut app, KeyCode::Esc);
    assert_eq!(saved(&mut app), "- Buy milk @due(2026-10-05)\n- Call Ana\n");
}

// D-50
#[test]
fn typing_filters_by_word_and_enter_runs_the_first_match() {
    let mut app = Harness::with_text("Plan\n", 30, 10);
    app.settle();
    press(&mut app, KeyCode::Enter);
    app.type_keys("/date");
    assert_eq!(
        screen(&mut app)[2..7],
        [
            "▌ Plan",
            "      ┌Commands:───────┐",
            "      │ Set Start Date │",
            "      │ Set Due Date   │",
            "      └/date───────────┘",
        ]
    );
    app.type_keys(" ");
    press(&mut app, KeyCode::Backspace);
    press(&mut app, KeyCode::Backspace);
    press(&mut app, KeyCode::Backspace);
    press(&mut app, KeyCode::Backspace);
    press(&mut app, KeyCode::Backspace);
    app.type_keys("p");
    press(&mut app, KeyCode::Enter);
    assert_eq!(outline_rows(&mut app)[0], "Plan:");
    press(&mut app, KeyCode::Esc);
    assert_eq!(saved(&mut app), "Plan:\n");
}

// D-50
#[test]
fn a_command_keeps_editing_with_the_caret_where_it_was() {
    let mut app = Harness::with_text("- Call Ana\n", 40, 8);
    app.settle();
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Left);
    press(&mut app, KeyCode::Left);
    press(&mut app, KeyCode::Left);
    app.type_keys("/color r");
    press(&mut app, KeyCode::Enter);
    app.type_keys("/book");
    press(&mut app, KeyCode::Enter);
    app.type_keys("/comp");
    press(&mut app, KeyCode::Enter);
    app.type_keys("me ");
    assert!(status_line(&mut app).starts_with("EDIT"));
    press(&mut app, KeyCode::Esc);
    assert_eq!(focused_rows(&mut app).len(), 1);
    assert_eq!(
        saved(&mut app),
        "- Call me Ana @red @bookmark @done(2026-10-05)\n"
    );
    for _ in 0..4 {
        app.type_keys("z");
    }
    assert_eq!(saved(&mut app), "- Call Ana\n", "four steps to undo");
}

// D-50, FR-TAG-03
#[test]
fn a_date_prompt_from_the_menu_returns_to_editing() {
    let mut app = Harness::with_text("- Call Ana @start(2026-10-01)\n", 40, 10);
    app.settle();
    press(&mut app, KeyCode::Enter);
    app.type_keys("/set due");
    press(&mut app, KeyCode::Enter);
    assert!(status_line(&mut app).starts_with("NAV"));
    app.type_keys("fri");
    press(&mut app, KeyCode::Enter);
    assert!(status_line(&mut app).starts_with("EDIT"));
    app.type_keys("/clear s");
    press(&mut app, KeyCode::Enter);
    app.type_keys("!");
    press(&mut app, KeyCode::Esc);
    assert_eq!(saved(&mut app), "- Call Ana! @due(2026-10-09)\n");
}

// D-52
#[test]
fn slash_make_section_makes_a_text_block_a_section_and_back() {
    let mut app = Harness::with_text("Later @today\n- Next\n", 30, 8);
    app.settle();
    press(&mut app, KeyCode::Enter);
    app.type_keys("/make");
    press(&mut app, KeyCode::Enter);
    assert!(status_line(&mut app).starts_with("EDIT"));
    press(&mut app, KeyCode::Esc);
    assert_eq!(saved(&mut app), "Later @today @section\n- Next\n");
    press(&mut app, KeyCode::Enter);
    app.type_keys("/remove");
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Esc);
    assert_eq!(saved(&mut app), "Later @today\n- Next\n");
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    app.type_keys("/sec");
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Esc);
    assert_eq!(
        saved(&mut app),
        "Later @today\n- Next/sec\n",
        "a task has no section entry"
    );
}

// D-44, D-50
#[test]
fn esc_types_the_slash_and_a_path_stays_text() {
    let mut app = Harness::with_text("- Open\n", 30, 8);
    app.settle();
    press(&mut app, KeyCode::Enter);
    app.type_keys(" /");
    press(&mut app, KeyCode::Esc);
    app.type_keys("/home and / b");
    assert_eq!(outline_rows(&mut app)[0], "- Open //home and / b");
    press(&mut app, KeyCode::Esc);
    assert_eq!(saved(&mut app), "- Open //home and / b\n");
}

// D-44, FR-OPS-08
#[test]
fn the_type_change_undoes_with_the_session_as_one_step() {
    let mut app = Harness::with_text("Milk\n", 30, 8);
    app.settle();
    press(&mut app, KeyCode::Enter);
    app.type_keys("s/t");
    press(&mut app, KeyCode::Enter);
    assert_eq!(outline_rows(&mut app)[0], "- Milks");
    app.press(KeyCode::Char('z'), KeyModifiers::CONTROL);
    assert_eq!(
        outline_rows(&mut app)[0],
        "Milks",
        "undone within the session"
    );
    app.press(KeyCode::Char('y'), KeyModifiers::CONTROL);
    press(&mut app, KeyCode::Esc);
    assert_eq!(saved(&mut app), "- Milks\n");
    app.type_keys("z");
    assert_eq!(outline_rows(&mut app)[0], "Milk");
    assert_eq!(saved(&mut app), "Milk\n");
}

// FR-DOC-05, D-44, D-50
#[test]
fn the_demo_has_a_text_line_to_make_a_task_from_the_menu() {
    let mut app = Harness::demo(60, 12);
    app.type_keys("jjj");
    press(&mut app, KeyCode::Enter);
    app.type_keys("/t");
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Esc);
    assert!(saved(&mut app).contains("\t\t- Prefer a room with a view of the river.\n"));
}

// D-50, D-42
#[test]
fn ctrl_t_cycles_the_edited_blocks_type() {
    let mut app = Harness::with_text("Milk\n", 30, 8);
    app.settle();
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Left);
    let mut seen = Vec::new();
    for _ in 0..4 {
        app.press(KeyCode::Char('t'), KeyModifiers::CONTROL);
        seen.push(outline_rows(&mut app)[0].clone());
    }
    assert_eq!(seen, ["Milk:", "Milk", "- Milk", "Milk"]);
    app.press(KeyCode::Char('t'), KeyModifiers::CONTROL);
    app.type_keys("!");
    press(&mut app, KeyCode::Esc);
    assert_eq!(saved(&mut app), "Mil!k:\n");
}
