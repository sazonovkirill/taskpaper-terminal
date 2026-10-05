//! Step 3.3: setting a start or due date by typing it in a prompt with a live preview.

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

/// Rows 3 to 6 of a 50-column screen, where the prompt is drawn over a short outline.
fn prompt_box(app: &mut Harness) -> Vec<String> {
    screen(app).into_iter().skip(3).take(4).collect()
}

/// The prompt box as drawn on a 50-column screen over `row`, with `title`, `text` and `preview`.
fn drawn(row: &str, title: &str, text: &str, preview: &str) -> Vec<String> {
    let edge = |left: &str, middle: &str, right: &str| format!("{left}{middle}{right}");
    let rule = format!("{title}{}", "─".repeat(38 - title.chars().count()));
    vec![
        edge(&format!("{row:<5}"), &format!("┌{rule}┐"), ""),
        edge("     ", &format!("│ {text:<37}│"), ""),
        edge("     ", &format!("│ {preview:<37}│"), ""),
        edge("     ", &format!("└{}┘", "─".repeat(38)), ""),
    ]
}

// FR-TAG-03, search.md §4, FR-OPS-08
#[test]
fn md_previews_a_typed_date_and_enter_makes_it_due() {
    let mut app = Harness::with_text("- Milk\n- Bread @due(2026-10-20)\n", 50, 10);
    app.type_keys("jmd");
    let mut expected = drawn("▎ - B", " Due date ", "2026-10-20", "Tue 20 Oct 2026");
    expected[0].push_str("0 Oct");
    assert_eq!(prompt_box(&mut app), expected);
    app.type_keys("next fri");
    expected[1] = drawn("", "", "next fri", "")[1].clone();
    expected[2] = drawn("", "", "", "Fri 16 Oct 2026")[2].clone();
    assert_eq!(prompt_box(&mut app), expected, "typing replaced the date");
    assert_eq!(
        app.terminal.get_cursor_position().map(|p| (p.x, p.y)).ok(),
        Some((15, 4)),
        "the caret after the text"
    );
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        screen(&mut app)[3],
        format!("▎ - Bread{:>41}", "due Fri 16 Oct")
    );
    assert_eq!(saved(&mut app), "- Milk\n- Bread @due(2026-10-16)\n");
    app.type_keys("z");
    assert_eq!(saved(&mut app), "- Milk\n- Bread @due(2026-10-20)\n");
}

// FR-TAG-03, editing.md §5
#[test]
fn ms_on_a_block_without_a_start_opens_empty_and_esc_changes_nothing() {
    let mut app = Harness::with_text("- Milk\n", 50, 10);
    app.type_keys("ms");
    assert_eq!(
        prompt_box(&mut app),
        drawn("", " Start date ", "", "No date: Enter removes it")
    );
    app.type_keys("jk+3d");
    assert_eq!(
        prompt_box(&mut app)[1..3],
        drawn("", "", "jk+3d", "Not a date from “jk+3d”")[1..3],
        "letters are typed, not navigation"
    );
    press(&mut app, KeyCode::Esc);
    assert_eq!(screen(&mut app)[3], "");
    assert_eq!(screen(&mut app)[2], "▎ - Milk");
    app.type_keys("j");
    assert_eq!(saved(&mut app), "- Milk\n");
}

// FR-TAG-03, editing.md §5: empty input removes the tag.
#[test]
fn enter_on_an_emptied_prompt_removes_the_date() {
    let mut app = Harness::with_text("- Milk @start(2026-10-01) @due(2026-10-20)\n", 50, 10);
    app.type_keys("md");
    press(&mut app, KeyCode::Backspace);
    assert_eq!(
        prompt_box(&mut app)[1..3],
        drawn("", "", "", "No date: Enter removes it")[1..3]
    );
    press(&mut app, KeyCode::Enter);
    assert_eq!(saved(&mut app), "- Milk @start(2026-10-01)\n");
}

// search.md §4: the prompt rejects unparseable input and says why.
#[test]
fn a_text_that_is_no_date_says_why_and_enter_keeps_the_prompt() {
    let mut app = Harness::with_text("- Milk\n", 50, 10);
    app.type_keys("md9999-12-31 +1d");
    assert_eq!(
        prompt_box(&mut app)[2],
        drawn("", "", "", "Out of range")[2]
    );
    app.press(KeyCode::Char('a'), KeyModifiers::CONTROL);
    app.type_keys("next");
    assert_eq!(
        prompt_box(&mut app)[2],
        drawn("", "", "", "Not a date: something is missing")[2]
    );
    app.press(KeyCode::Char('w'), KeyModifiers::CONTROL);
    app.type_keys("02-30");
    assert_eq!(
        prompt_box(&mut app)[1..3],
        drawn("", "", "02-30", "No such day: “02-30”")[1..3],
        "Ctrl+W deleted the word"
    );
    press(&mut app, KeyCode::Enter);
    assert_eq!(prompt_box(&mut app)[1], drawn("", "", "02-30", "")[1]);

    press(&mut app, KeyCode::Home);
    app.type_keys("2026-");
    press(&mut app, KeyCode::End);
    press(&mut app, KeyCode::Backspace);
    press(&mut app, KeyCode::Backspace);
    app.type_keys("28");
    press(&mut app, KeyCode::Enter);
    assert_eq!(saved(&mut app), "- Milk @due(2026-02-28)\n");
}

// FR-TAG-03: a long text scrolls to keep the caret inside the box.
#[test]
fn a_long_text_scrolls_inside_the_box() {
    let mut app = Harness::with_text("- Milk\n", 50, 10);
    app.type_keys("md");
    app.type_keys(&"+1d ".repeat(12));
    let text_row = &prompt_box(&mut app)[1];
    assert!(text_row.ends_with("+1d +1d  │"), "{text_row}");
    assert_eq!(
        app.terminal.get_cursor_position().map(|p| (p.x, p.y)).ok(),
        Some((43, 4))
    );
    press(&mut app, KeyCode::Enter);
    assert_eq!(saved(&mut app), "- Milk @due(2026-10-17)\n");
}

#[test]
fn ctrl_q_quits_from_the_prompt() {
    let mut app = Harness::with_text("- Milk\n", 50, 10);
    app.type_keys("md");
    app.press(KeyCode::Char('q'), KeyModifiers::CONTROL);
    assert!(app.tui.should_quit());
}

// FR-DOC-05, FR-TAG-03
#[test]
fn the_demo_has_a_due_date_to_change_with_md() {
    let mut app = Harness::demo(70, 40);
    let rows = screen(&mut app);
    let down = rows
        .iter()
        .skip(2)
        .position(|row| row.contains("Return Piranesi to the library"))
        .expect("the demo has the task");
    app.type_keys(&"j".repeat(down));
    app.type_keys("md");
    let rows = screen(&mut app);
    assert!(
        rows.iter().any(|row| row.contains("│ 2026-10-17")),
        "{rows:#?}"
    );
    app.type_keys("tomorrow");
    press(&mut app, KeyCode::Enter);
    let rows = screen(&mut app);
    let task = rows
        .iter()
        .find(|row| row.contains("Return Piranesi to the library"))
        .expect("the task is shown");
    assert!(task.ends_with("due Tue 6 Oct"), "{task}");
}
