//! Step 4.5: word wrap, on by default and toggled with `Alt+z` and `gw`.

use std::fmt::Write as _;

use crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{DUE, Harness, focused_rows, outline_rows};

const NONE: KeyModifiers = KeyModifiers::NONE;

const LONG: &str = "- buy milk and eggs from the shop\n- b\n";

/// The outline lines on screen that are not blank.
fn shown(app: &mut Harness) -> Vec<String> {
    outline_rows(app)
        .into_iter()
        .filter(|row| !row.is_empty())
        .collect()
}

const WRAPPED: [&str; 4] = ["- buy milk and", "  eggs from the", "  shop", "- b"];

// FR-UI-05, D-42
#[test]
fn long_text_wraps_until_alt_z_turns_wrap_off_and_on_again() {
    let mut app = Harness::with_text(LONG, 20, 8);
    assert_eq!(shown(&mut app), WRAPPED, "wrap is on by default");
    app.press(KeyCode::Char('z'), KeyModifiers::ALT);
    assert_eq!(shown(&mut app), ["- buy milk and eg…", "- b"]);
    app.press(KeyCode::Char('z'), KeyModifiers::ALT);
    assert_eq!(shown(&mut app), WRAPPED);
}

// FR-UI-05, D-42
#[test]
fn g_w_toggles_wrap_and_keeps_the_focus() {
    let mut app = Harness::with_text(LONG, 20, 8);
    app.type_keys("j");
    app.type_keys("gw");
    assert_eq!(shown(&mut app), ["- buy milk and eg…", "- b"]);
    assert_eq!(focused_rows(&mut app), ["- b"]);
    app.type_keys("gw");
    assert_eq!(shown(&mut app), WRAPPED);
    assert_eq!(focused_rows(&mut app), ["- b"]);
}

// FR-UI-05, FR-TAG-03
#[test]
fn a_wrapped_block_keeps_its_date_chip_on_its_first_line() {
    let mut app = Harness::with_text("- buy milk and eggs @due(2026-10-05)\n", 24, 6);
    assert_eq!(
        shown(&mut app),
        [format!("- buy milk and {DUE} today"), "  eggs".to_owned()]
    );
}

/// `n` tasks whose text takes two lines in a 16-column terminal.
fn long_tasks(n: usize) -> String {
    (1..=n).fold(String::new(), |mut text, i| {
        writeln!(text, "- Task {i:02} has two lines").expect("writing to a String cannot fail");
        text
    })
}

// FR-UI-05, navigation §2
#[test]
fn scrolling_keeps_every_line_of_the_focused_wrapped_block_in_view() {
    let mut app = Harness::with_text(&long_tasks(6), 16, 9);
    for _ in 0..5 {
        app.type_keys("j");
        assert_eq!(focused_rows(&mut app).len(), 2, "both lines on screen");
    }
    assert_eq!(
        shown(&mut app),
        [
            "- Task 04 has",
            "  two lines",
            "- Task 05 has",
            "  two lines",
            "- Task 06 has",
            "  two lines"
        ]
    );
    app.type_keys("gg");
    assert_eq!(shown(&mut app)[..2], ["- Task 01 has", "  two lines"]);
}

// FR-UI-05, FR-NAV-01
#[test]
fn paging_keeps_the_focused_wrapped_block_in_its_place_on_screen() {
    let mut app = Harness::with_text(&long_tasks(12), 16, 9);
    app.type_keys("jj");
    assert_eq!(shown(&mut app)[2], "- Task 03 has");
    app.press(KeyCode::PageDown, NONE);
    assert_eq!(focused_rows(&mut app), ["- Task 09 has", "  two lines"]);
    assert_eq!(shown(&mut app)[2], "- Task 09 has", "where Task 03 was");
}

// FR-UI-05, FR-MODE-02
#[test]
fn the_caret_sits_on_the_edited_block_below_a_wrapped_one_which_edits_on_one_line() {
    let mut app = Harness::with_text("- aaa bbb ccc\n- d\n", 9, 8);
    app.type_keys("j");
    app.press(KeyCode::Enter, NONE);
    app.render();
    app.terminal.backend_mut().assert_cursor_position((5, 5));
    app.press(KeyCode::Up, NONE);
    assert_eq!(shown(&mut app), ["- aaa b", "- d"]);
    app.terminal.backend_mut().assert_cursor_position((5, 2));
}
