//! Step 3.9: inline formatting. `**bold**`, `` `code` `` and bare URLs drawn with their delimiters
//! hidden in navigate mode and raw in edit mode, and the keys that toggle bold and code.

use std::{fs, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::{Color, Modifier};

use crate::support::{Harness, outline_line, outline_rows};

/// The open file's text once the autosave delay has passed and its save is done.
fn saved(app: &mut Harness) -> String {
    app.wait(Duration::from_secs(1));
    app.settle();
    let path = app.path.as_ref().expect("a file was opened");
    fs::read_to_string(path).expect("the file is readable")
}

// FR-FMT-01, D-45, D-49
#[test]
fn navigate_mode_draws_bold_code_and_urls_without_delimiters_and_edit_mode_shows_them() {
    let mut app = Harness::with_text("- Buy **milk** at `Shop` https://shop.example\n", 50, 5);
    assert_eq!(
        outline_rows(&mut app)[0],
        "- Buy milk at Shop https://shop.example"
    );
    let buffer = app.render().buffer().clone();
    assert!(buffer[(8, 2)].modifier.contains(Modifier::BOLD), "bold");
    assert!(!buffer[(12, 2)].modifier.contains(Modifier::BOLD), "plain");
    assert_eq!(
        (buffer[(17, 2)].fg, buffer[(17, 2)].modifier),
        (Color::Reset, Modifier::DIM),
        "code keeps the task's color, dimmed"
    );
    assert!(
        buffer[(22, 2)].modifier.contains(Modifier::UNDERLINED),
        "a bare URL is underlined"
    );
    assert_eq!(
        buffer[(22, 2)].fg,
        Color::Cyan,
        "in the link color in a task too"
    );
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        outline_rows(&mut app)[0],
        "- Buy **milk** at `Shop` https://shop.example"
    );
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    let buffer = app.render().buffer().clone();
    assert_eq!(
        outline_line(&buffer, 2),
        "- Buy milk at Shop https://shop.example"
    );
}

// FR-FMT-02
#[test]
fn ctrl_b_bolds_the_word_at_the_caret_and_again_takes_it_off() {
    let mut app = Harness::with_text("- Buy milk\n", 40, 5);
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    app.press(KeyCode::Char('b'), KeyModifiers::CONTROL);
    assert_eq!(outline_rows(&mut app)[0], "- Buy **milk**");
    assert_eq!(saved(&mut app), "- Buy **milk**\n");
    app.press(KeyCode::Char('b'), KeyModifiers::CONTROL);
    assert_eq!(outline_rows(&mut app)[0], "- Buy milk");
    app.press(KeyCode::Char('b'), KeyModifiers::CONTROL);
    app.press(KeyCode::Char('z'), KeyModifiers::CONTROL);
    assert_eq!(outline_rows(&mut app)[0], "- Buy milk", "one undo step");
}

// FR-FMT-02
#[test]
fn alt_backtick_and_ctrl_e_make_the_selection_inline_code() {
    let mut app = Harness::with_text("run make all\n", 40, 5);
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    app.press(KeyCode::Left, KeyModifiers::CONTROL | KeyModifiers::SHIFT);
    app.press(KeyCode::Left, KeyModifiers::CONTROL | KeyModifiers::SHIFT);
    app.press(KeyCode::Char('`'), KeyModifiers::ALT);
    assert_eq!(outline_rows(&mut app)[0], "run `make all`");
    app.press(KeyCode::Char('e'), KeyModifiers::CONTROL);
    assert_eq!(outline_rows(&mut app)[0], "run make all");
    app.press(KeyCode::Char('e'), KeyModifiers::CONTROL);
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(outline_rows(&mut app)[0], "run make all");
    assert_eq!(saved(&mut app), "run `make all`\n");
}
