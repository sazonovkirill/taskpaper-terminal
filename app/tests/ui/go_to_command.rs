//! Step 4.3: `Ctrl+P` and `:` open Go to Command, a fuzzy finder over every command of navigate
//! mode by name with its bindings; `Enter` gives the chosen command, such as Count.

use std::{fs, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::Modifier;

use crate::support::{Harness, focused_rows, line};

const TEXT: &str = "Trip:\n\t- Book the hotel\n\t\tNear the station\n\t- Pack\nOther\n";

/// Every screen row, without trailing blanks.
fn screen(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    (buffer.area.top()..buffer.area.bottom())
        .map(|y| line(&buffer, y))
        .collect()
}

fn press(app: &mut Harness, code: KeyCode) {
    app.press(code, KeyModifiers::NONE);
}

/// Opens Go to Command with `Ctrl+P`, types `query` and gives the best match.
fn give(app: &mut Harness, query: &str) {
    app.press(KeyCode::Char('p'), KeyModifiers::CONTROL);
    app.type_keys(query);
    press(app, KeyCode::Enter);
}

// FR-NAV-06, navigation.md §8, ui.md §1
#[test]
fn ctrl_p_lists_the_commands_by_name_each_with_its_bindings() {
    let mut app = Harness::with_text(TEXT, 60, 16);
    app.press(KeyCode::Char('p'), KeyModifiers::CONTROL);
    assert_eq!(
        screen(&mut app)[2..7],
        [
            "┌Go to Command─────────────────────────────────────────────┐",
            "│›                                                         │",
            "│ › Back  H, Alt+Shift+←                                   │",
            "│ ● Choose Theme  Ctrl+T                                   │",
            "│ ⇥ Clear Due Date                                         │",
        ]
    );
    let buffer = app.render().buffer().clone();
    assert!(
        buffer[(2, 4)].modifier.contains(Modifier::REVERSED),
        "the first entry is highlighted"
    );
    assert!(
        buffer[(10, 4)].modifier.contains(Modifier::DIM),
        "its keys are dim"
    );
    app.terminal.backend_mut().assert_cursor_position((3, 3));
    app.type_keys("symbol");
    assert_eq!(
        screen(&mut app)[7..10],
        [
            "┌Go to Command─────────────────────────────────────────────┐",
            "│› symbol                                                  │",
            "│ › Go to Symbol  Ctrl+K, gs, /                            │",
        ]
    );
}

// FR-NAV-06, FR-OPS-09, keybindings.md §2
#[test]
fn colon_opens_it_and_enter_gives_count_which_says_what_is_under_the_focus() {
    let mut app = Harness::with_text(TEXT, 100, 16);
    app.type_keys(":count");
    press(&mut app, KeyCode::Enter);
    let rows = screen(&mut app);
    assert!(
        !rows.iter().any(|row| row.contains("Go to Command")),
        "{rows:#?}"
    );
    assert!(rows[15].ends_with(" · 3 blocks, 7 words"), "{}", rows[15]);
    press(&mut app, KeyCode::Char('j'));
    give(&mut app, "count");
    assert!(screen(&mut app)[15].ends_with(" · 1 block, 3 words"));
}

// FR-NAV-06
#[test]
fn a_chosen_command_acts_as_its_keys_do() {
    let mut app = Harness::with_text(TEXT, 60, 16);
    press(&mut app, KeyCode::Char('j'));
    give(&mut app, "toggle done");
    app.wait(Duration::from_secs(1));
    app.settle();
    let path = app.path.clone().expect("a file was opened");
    assert_eq!(
        fs::read_to_string(path).unwrap(),
        "Trip:\n\t- Book the hotel @done(2026-10-05)\n\t\tNear the station\n\t- Pack\nOther\n",
        "dated with the frontend's date"
    );
    give(&mut app, "zoom in");
    assert_eq!(screen(&mut app)[0], "  ⌂ › Trip › Book the hotel");
    give(&mut app, "go to symbol");
    assert!(
        screen(&mut app)
            .iter()
            .any(|row| row.contains("Go to Symbol")),
        "a command can open another finder"
    );
}

// FR-NAV-06, FR-UI-05
#[test]
fn a_command_the_frontend_carries_out_runs_too() {
    let mut app = Harness::with_text("- A task long enough to wrap at this width\n", 30, 8);
    give(&mut app, "word wrap");
    assert_eq!(screen(&mut app)[2], "▌ - A task long enough to wra…");
}

// keybindings.md §4
#[test]
fn esc_closes_it_and_nothing_to_choose_keeps_it_open() {
    let mut app = Harness::with_text(TEXT, 60, 16);
    give(&mut app, "qqqq");
    assert!(
        screen(&mut app)
            .iter()
            .any(|row| row.contains("No matches")),
        "Enter on nothing keeps it open"
    );
    press(&mut app, KeyCode::Esc);
    let rows = screen(&mut app);
    assert!(
        !rows.iter().any(|row| row.contains("Go to Command")),
        "{rows:#?}"
    );
    assert_eq!(focused_rows(&mut app), ["Trip:"]);
}
