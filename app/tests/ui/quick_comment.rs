//! Quick Comment: `c` adds a comment to the focused block from a one-line prompt at the bottom of
//! the screen, without opening the attachments panel (FR-ATT-07).

use std::{fs, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::Modifier;

use crate::support::{Harness, line};

const TEXT: &str = "- Call the bank\n\tCOMMENTS @hidden\n\t\t[2026-10-01 09:00] busy\n- Pay\n";

/// The screen's width.
const WIDTH: u16 = 60;

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

/// The open file's text once the autosave delay has passed and its save is done.
fn saved(app: &mut Harness) -> String {
    app.wait(Duration::from_secs(1));
    app.settle();
    fs::read_to_string(app.path.clone().expect("a file was opened")).expect("the file is saved")
}

/// A border's top across the screen, with `title`.
fn top(title: &str) -> String {
    let rule = "─".repeat(usize::from(WIDTH) - 2 - title.chars().count());
    format!("┌{title}{rule}┐")
}

/// A row inside a border across the screen.
fn inside(text: &str) -> String {
    format!("│{text:<width$}│", width = usize::from(WIDTH) - 2)
}

// FR-ATT-07, attachments.md §5
#[test]
fn c_opens_a_prompt_at_the_bottom_and_enter_adds_the_newest_comment_as_one_step_to_undo() {
    let mut app = Harness::with_text(TEXT, WIDTH, 12);
    app.type_keys("cCalled back");
    let rows = screen(&mut app);
    assert_eq!(
        rows[8..11],
        [
            top(" Comment on Call the bank "),
            inside(" Called back"),
            format!("└{}┘", "─".repeat(usize::from(WIDTH) - 2)),
        ]
    );
    app.terminal.backend_mut().assert_cursor_position((13, 9));
    let buffer = app.render().buffer().clone();
    let dimmed = |x, y| buffer[(x, y)].modifier.contains(Modifier::DIM);
    assert!(dimmed(4, 2) && dimmed(0, 11), "the outline and status line");
    assert!(!dimmed(0, 8) && !dimmed(2, 9), "the prompt");
    press(&mut app, KeyCode::Enter);
    let rows = screen(&mut app);
    assert!(rows[11].contains("Comment added"), "{rows:?}");
    assert!(rows[2].ends_with("❞ 2"), "the count goes up: {rows:?}");
    assert!(!rows[8].starts_with('┌'), "the prompt is closed");
    assert_eq!(
        saved(&mut app),
        "- Call the bank\n\tCOMMENTS @hidden\n\t\t[2026-10-05 09:30] Called back\n\t\t\
         [2026-10-01 09:00] busy\n- Pay\n"
    );
    app.type_keys("z");
    assert_eq!(saved(&mut app), TEXT);
}

// FR-ATT-07, FR-OPS-06, attachments.md §5
#[test]
fn esc_closes_the_prompt_adding_nothing_and_ctrl_v_pastes_one_line() {
    let mut app = Harness::with_text("- Pay\n- Milk\n", WIDTH, 12);
    app.type_keys("yjc");
    app.press(KeyCode::Char('v'), KeyModifiers::CONTROL);
    app.settle();
    assert_eq!(screen(&mut app)[9], inside(" Pay"));
    press(&mut app, KeyCode::Esc);
    assert!(!screen(&mut app)[8].starts_with('┌'));
    app.type_keys("c ");
    press(&mut app, KeyCode::Enter);
    app.type_keys("j");
    assert_eq!(
        saved(&mut app),
        "- Pay\n- Milk\n",
        "only spaces add nothing"
    );
}
