//! The type menu: `/` in edit mode gives the block another type (D-44).

use std::{fs, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Harness, STAR, focused_rows, line, outline_rows};

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

// D-44, ui.md §5
#[test]
fn slash_opens_the_menu_under_the_caret_and_a_letter_makes_a_section() {
    let mut app = Harness::with_text("- Buy milk\n- Call Ana\n", 30, 12);
    app.settle();
    press(&mut app, KeyCode::Enter);
    app.type_keys("/");
    assert_eq!(
        screen(&mut app)[2..10],
        [
            "▌ - Buy milk",
            "  - Call Ana┌Node type:┐",
            "            │ Project  │",
            "            │ Section  │",
            "            │ Note     │",
            "            │ Task     │",
            "            │ Divider  │",
            "            └──────────┘",
        ]
    );
    app.type_keys("s");
    assert_eq!(outline_rows(&mut app)[..3], ["Buy milk", "- Call Ana", ""]);
    assert!(status_line(&mut app).starts_with("EDIT"));
    app.type_keys("!");
    press(&mut app, KeyCode::Esc);
    assert_eq!(focused_rows(&mut app), ["Buy milk!"]);
    assert_eq!(saved(&mut app), "Buy milk! @section\n- Call Ana\n");
}

// D-44
#[test]
fn typing_filters_the_menu_and_arrows_and_enter_choose() {
    let mut app = Harness::with_text("Plan\n", 30, 10);
    app.settle();
    press(&mut app, KeyCode::Enter);
    app.type_keys("/ec");
    assert_eq!(
        screen(&mut app)[2..7],
        [
            "▌ Plan",
            "      ┌Node type:┐",
            "      │ Project  │",
            "      │ Section  │",
            "      └/ec───────┘",
        ]
    );
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Down);
    app.press(KeyCode::Char('p'), KeyModifiers::CONTROL);
    press(&mut app, KeyCode::Up);
    press(&mut app, KeyCode::Enter);
    assert_eq!(outline_rows(&mut app)[0], "Plan:");
    press(&mut app, KeyCode::Esc);
    assert_eq!(saved(&mut app), "Plan:\n");
}

// D-44
#[test]
fn slash_div_makes_a_divider_that_keeps_the_tags() {
    let mut app = Harness::with_text("- Later @today\n- Next\n", 30, 8);
    app.settle();
    press(&mut app, KeyCode::Enter);
    app.type_keys("/div");
    press(&mut app, KeyCode::Enter);
    assert!(status_line(&mut app).starts_with("NAV"));
    assert_eq!(saved(&mut app), "--- @today\n- Next\n");
    app.type_keys("z");
    assert_eq!(outline_rows(&mut app)[0], format!("- Later {STAR}"));
    assert_eq!(saved(&mut app), "- Later @today\n- Next\n");
}

// D-44
#[test]
fn esc_types_the_slash_and_a_path_stays_text() {
    let mut app = Harness::with_text("- Open\n", 30, 8);
    app.settle();
    press(&mut app, KeyCode::Enter);
    app.type_keys(" /");
    press(&mut app, KeyCode::Esc);
    app.type_keys("/home");
    assert_eq!(outline_rows(&mut app)[0], "- Open //home");
    press(&mut app, KeyCode::Esc);
    assert_eq!(saved(&mut app), "- Open //home\n");
}

// D-44, FR-OPS-08
#[test]
fn the_type_change_undoes_with_the_session_as_one_step() {
    let mut app = Harness::with_text("Milk\n", 30, 8);
    app.settle();
    press(&mut app, KeyCode::Enter);
    app.type_keys("s/t");
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

// FR-DOC-05, D-44
#[test]
fn the_demo_has_a_text_line_to_make_a_task_from_the_menu() {
    let mut app = Harness::demo(60, 12);
    app.type_keys("jjj");
    press(&mut app, KeyCode::Enter);
    app.type_keys("/t");
    press(&mut app, KeyCode::Esc);
    assert!(saved(&mut app).contains("\t\t- Prefer a room with a view of the river.\n"));
}
