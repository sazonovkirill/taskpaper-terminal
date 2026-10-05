//! Step 3.6: duplicating blocks with `D`, copying, cutting and pasting them with `Ctrl+C`,
//! `Ctrl+X`, `Ctrl+V` and `Alt+V`, and pasting text into the terminal. No system clipboard is
//! reachable here, so copying writes OSC 52 and `Ctrl+V` pastes what was copied last.

use std::{fs, time::Duration};

use crossterm::event::{Event, KeyCode, KeyModifiers};
use taskpaper_terminal::ui::Input;

use crate::support::{Harness, focused_rows, line, outline_rows};

/// The status line of `app` after a draw.
fn status_line(app: &mut Harness) -> String {
    let buffer = app.render().buffer().clone();
    line(&buffer, buffer.area.bottom() - 1)
}

fn ctrl(app: &mut Harness, c: char) {
    app.press(KeyCode::Char(c), KeyModifiers::CONTROL);
}

/// Pastes `text` into the terminal, which brackets it.
fn paste_into_terminal(app: &mut Harness, text: &str) {
    app.send(Input::Terminal(Event::Paste(text.to_owned())));
}

/// The open file's text once the autosave delay has passed and its save is done.
fn saved(app: &mut Harness) -> String {
    app.wait(Duration::from_secs(1));
    app.settle();
    let path = app.path.as_ref().expect("a file was opened");
    fs::read_to_string(path).expect("the file is readable")
}

// FR-OPS-06, FR-OPS-08
#[test]
fn shift_d_duplicates_a_block_with_its_subtree_below_it_and_z_takes_it_back() {
    let mut app = Harness::with_text("Groceries:\n\t- Milk\n\t\tOat\n\t- Bread\n", 50, 10);
    app.type_keys("jD");
    assert_eq!(
        outline_rows(&mut app)[..5],
        ["Groceries:", "  - Milk", "    Oat", "  - Milk", "    Oat"]
    );
    assert_eq!(focused_rows(&mut app), ["  - Milk"]);
    app.type_keys("jj");
    assert_eq!(
        focused_rows(&mut app),
        ["  - Bread"],
        "the copy was focused"
    );
    app.type_keys("kk");
    assert_eq!(
        saved(&mut app),
        "Groceries:\n\t- Milk\n\t\tOat\n\t- Milk\n\t\tOat\n\t- Bread\n"
    );
    app.type_keys("z");
    assert_eq!(
        saved(&mut app),
        "Groceries:\n\t- Milk\n\t\tOat\n\t- Bread\n"
    );
}

// FR-OPS-06, FR-OPS-07
#[test]
fn shift_d_duplicates_every_selected_block() {
    let mut app = Harness::with_text("- a\n- b\n- c\n", 50, 10);
    app.type_keys("JD");
    assert_eq!(
        outline_rows(&mut app)[..5],
        ["- a", "- a", "- b", "- b", "- c"]
    );
}

// FR-OPS-06
#[test]
fn ctrl_c_copies_a_project_with_its_hierarchy_and_ctrl_v_and_alt_v_paste_it() {
    let mut app = Harness::with_text(
        "Trip: @travel\n\t- Book flights\nHome:\n\t- Water plants\n",
        50,
        10,
    );
    ctrl(&mut app, 'c');
    assert_eq!(
        app.terminal_output.copied(),
        ["Trip: @travel\n\t- Book flights\n"],
        "on the terminal's clipboard"
    );
    app.type_keys("jj");
    ctrl(&mut app, 'v');
    assert_eq!(
        outline_rows(&mut app)[..6],
        [
            "Trip:",
            "  - Book flights",
            "Home:",
            "  - Water plants",
            "Trip:",
            "  - Book flights"
        ]
    );
    assert_eq!(focused_rows(&mut app), ["Trip:"], "the pasted block");
    app.type_keys("gg");
    app.press(KeyCode::Char('v'), KeyModifiers::ALT);
    assert_eq!(
        saved(&mut app),
        "Trip: @travel\n\t- Book flights\nTrip: @travel\n\t- Book flights\nHome:\n\t- Water plants\nTrip: @travel\n\t- Book flights\n"
    );
}

// FR-OPS-06
#[test]
fn ctrl_x_and_ctrl_v_move_selected_blocks() {
    let mut app = Harness::with_text("- a\n- b\n- c\n", 50, 10);
    app.type_keys("J");
    ctrl(&mut app, 'x');
    assert_eq!(app.terminal_output.copied(), ["- a\n- b\n"]);
    assert_eq!(outline_rows(&mut app)[..2], ["- c", ""]);
    ctrl(&mut app, 'v');
    assert_eq!(saved(&mut app), "- c\n- a\n- b\n");
}

// FR-OPS-06
#[test]
fn ctrl_v_with_nothing_copied_pastes_nothing() {
    let mut app = Harness::with_text("- a\n", 50, 10);
    ctrl(&mut app, 'v');
    assert_eq!(outline_rows(&mut app)[..2], ["- a", ""]);
}

// FR-OPS-06
#[test]
fn a_project_copied_in_one_file_pastes_into_another_through_the_terminal() {
    let mut from = Harness::with_text("Errands:\n\t- Bank\n\t\tBring the card.\n", 50, 10);
    ctrl(&mut from, 'c');
    let copied = from.terminal_output.copied().remove(0);
    let mut to = Harness::with_text("Home:\n", 50, 10);
    paste_into_terminal(&mut to, &copied);
    assert_eq!(
        saved(&mut to),
        "Home:\nErrands:\n\t- Bank\n\t\tBring the card.\n"
    );
}

// FR-MODE-03, OQ-25
#[test]
fn text_pasted_in_edit_mode_goes_in_at_the_caret_without_tags() {
    let mut app = Harness::with_text("- Call\n", 100, 10);
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    paste_into_terminal(&mut app, " mum @today");
    assert!(
        status_line(&mut app).ends_with(" · Tags are set with commands"),
        "{}",
        status_line(&mut app)
    );
    app.press(KeyCode::Left, KeyModifiers::SHIFT);
    app.press(KeyCode::Left, KeyModifiers::SHIFT);
    ctrl(&mut app, 'x');
    assert_eq!(app.terminal_output.copied(), ["ay"]);
    ctrl(&mut app, 'v');
    ctrl(&mut app, 'v');
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(saved(&mut app), "- Call mum todayay\n");
}

// FR-OPS-06
#[test]
fn several_lines_pasted_into_a_new_block_become_blocks_in_its_place() {
    let mut app = Harness::with_text("Shopping:\n\t- Milk\n", 50, 10);
    app.type_keys("jo");
    paste_into_terminal(&mut app, "- Eggs\n- Bread\n");
    assert!(status_line(&mut app).starts_with("NAVIGATE"));
    assert_eq!(
        saved(&mut app),
        "Shopping:\n\t- Milk\n\t- Eggs\n\t- Bread\n"
    );
}
