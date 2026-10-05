//! Typed shortcuts: `--- `, `[] `, `[ ] ` and `[x] ` at the start of a text block or task give
//! it another type, and the very next `Backspace` takes it back (FR-MODE-05, editing.md §2.1).

use std::{fs, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Harness, outline_rows};

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

// FR-MODE-05, editing.md §2.1
#[test]
fn brackets_make_a_task_and_a_cross_completes_it() {
    let mut app = Harness::with_text("Errands\n", 40, 8);
    app.settle();
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter);
    app.type_keys("[] Post office");
    press(&mut app, KeyCode::Enter);
    app.type_keys("[x] Bank");
    press(&mut app, KeyCode::Esc);
    assert_eq!(
        outline_rows(&mut app)[..3],
        ["Errands", "- Post office", "- [x] Bank"],
        "in a task the brackets are text"
    );
    app.type_keys("kk");
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter);
    app.type_keys("[x] Bank");
    press(&mut app, KeyCode::Esc);
    assert_eq!(
        saved(&mut app),
        "Errands\n- Bank @done(2026-10-05)\n- Post office\n- [x] Bank\n"
    );
}

// editing.md §2.1
#[test]
fn backspace_right_after_takes_the_shortcut_back() {
    let mut app = Harness::with_text("Errands\n", 40, 8);
    app.settle();
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter);
    app.type_keys("[x] ");
    press(&mut app, KeyCode::Backspace);
    app.type_keys("is a checkbox");
    press(&mut app, KeyCode::Esc);
    assert_eq!(saved(&mut app), "Errands\n[x] is a checkbox\n");
}

// FR-MODE-05, editing.md §2.1
#[test]
fn dashes_make_a_divider_and_typing_goes_on_below_it() {
    let mut app = Harness::with_text("- Pack\n", 40, 8);
    app.settle();
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter);
    app.type_keys("--- Unpack");
    press(&mut app, KeyCode::Esc);
    assert_eq!(
        outline_rows(&mut app)[..3],
        ["- Pack", "─".repeat(38).as_str(), "- Unpack"]
    );
    assert_eq!(saved(&mut app), "- Pack\n---\n- Unpack\n");
    app.type_keys("z");
    assert_eq!(saved(&mut app), "- Pack\n");
}
