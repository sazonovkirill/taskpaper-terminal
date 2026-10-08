//! Step 3.3: setting a start or due date by typing it in a prompt with a live preview.

use std::{fs, time::Duration};

use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::style::{Color, Modifier};
use taskpaper_terminal::ui::Input;

use crate::support::{DUE, Harness, focused_rows, line};

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

/// Rows 5 to 8 of a 50 × 10 screen, where the prompt's panel is drawn above the status line.
fn prompt_box(app: &mut Harness) -> Vec<String> {
    screen(app).into_iter().skip(5).take(4).collect()
}

/// The prompt's panel as drawn across a 50-column screen, with `title`, `text` and `preview`.
fn drawn(title: &str, text: &str, preview: &str) -> Vec<String> {
    let rule = format!("{title}{}", "─".repeat(48 - title.chars().count()));
    vec![
        format!("┌{rule}┐"),
        format!("│ {text:<47}│"),
        format!("│ {preview:<47}│"),
        format!("└{}┘", "─".repeat(48)),
    ]
}

// FR-TAG-03, search.md §4, FR-OPS-08
#[test]
fn md_previews_a_typed_date_and_enter_makes_it_due() {
    let mut app = Harness::with_text("- Milk\n- Bread @due(2026-10-20)\n", 50, 10);
    app.type_keys("jmd");
    assert_eq!(
        prompt_box(&mut app),
        drawn(" Due date ", "2026-10-20", "Tue 20 Oct 2026")
    );
    app.type_keys("next fri");
    assert_eq!(
        prompt_box(&mut app),
        drawn(" Due date ", "next fri", "Fri 16 Oct 2026"),
        "typing replaced the date"
    );
    assert_eq!(
        app.terminal.get_cursor_position().map(|p| (p.x, p.y)).ok(),
        Some((10, 6)),
        "the caret after the text"
    );
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        screen(&mut app)[3],
        format!("▌ - Bread{:>41}", format!("{DUE} next Fri"))
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
        drawn(" Start date ", "", "No date: Enter removes it")
    );
    app.type_keys("jk+3d");
    assert_eq!(
        prompt_box(&mut app)[1..3],
        drawn("", "jk+3d", "Not a date from “jk+3d”")[1..3],
        "letters are typed, not navigation"
    );
    press(&mut app, KeyCode::Esc);
    assert_eq!(screen(&mut app)[5], "");
    assert_eq!(screen(&mut app)[2], "▌ - Milk");
    app.type_keys("j");
    assert_eq!(saved(&mut app), "- Milk\n");
}

// ui.md §1, D-54
#[test]
fn the_panel_is_in_the_accent_color_and_the_rest_of_the_screen_is_dimmed() {
    let mut app = Harness::with_text("- Milk\n", 50, 10);
    app.type_keys("md");
    assert_eq!(
        screen(&mut app)[2],
        "▌ - Milk",
        "the outline keeps its place"
    );
    let buffer = app.render().buffer().clone();
    let dimmed = |x, y| buffer[(x, y)].modifier.contains(Modifier::DIM);
    assert!(dimmed(4, 0), "breadcrumbs");
    assert!(dimmed(4, 2), "outline");
    assert!(dimmed(0, 9), "status line");
    assert!(!dimmed(0, 5) && !dimmed(2, 6), "panel");
    let accent = Color::Yellow;
    assert_eq!(buffer[(0, 5)].fg, accent, "border");
    assert_eq!(buffer[(3, 5)].fg, accent, "title");
    press(&mut app, KeyCode::Esc);
    let buffer = app.render().buffer().clone();
    assert!(!buffer[(4, 2)].modifier.contains(Modifier::DIM));
}

// ui.md §1, D-54: the outline scrolls only when the panel would cover the focus.
#[test]
fn the_focused_block_stays_visible_above_the_panel() {
    let text = (0..20)
        .map(|n| format!("- t{n}\n"))
        .collect::<Vec<_>>()
        .concat();
    let mut app = Harness::with_text(&text, 50, 10);
    app.type_keys("jjjj");
    assert_eq!(screen(&mut app)[6], "▌ - t4");
    app.type_keys("md");
    let rows = screen(&mut app);
    assert_eq!(rows[2..5], ["  - t3", "▌ - t4", "  - t5"]);
    assert!(rows[5].starts_with("┌ Due date "), "{rows:#?}");
}

// FR-TAG-03, editing.md §5: empty input removes the tag.
#[test]
fn enter_on_an_emptied_prompt_removes_the_date() {
    let mut app = Harness::with_text("- Milk @start(2026-10-01) @due(2026-10-20)\n", 50, 10);
    app.type_keys("md");
    press(&mut app, KeyCode::Backspace);
    assert_eq!(
        prompt_box(&mut app)[1..3],
        drawn("", "", "No date: Enter removes it")[1..3]
    );
    press(&mut app, KeyCode::Enter);
    assert_eq!(saved(&mut app), "- Milk @start(2026-10-01)\n");
}

// search.md §4: the prompt rejects unparseable input and says why.
#[test]
fn a_text_that_is_no_date_says_why_and_enter_keeps_the_prompt() {
    let mut app = Harness::with_text("- Milk\n", 50, 10);
    app.type_keys("md9999-12-31 +1d");
    assert_eq!(prompt_box(&mut app)[2], drawn("", "", "Out of range")[2]);
    app.press(KeyCode::Char('a'), KeyModifiers::CONTROL);
    app.type_keys("next");
    assert_eq!(
        prompt_box(&mut app)[2],
        drawn("", "", "Not a date: something is missing")[2]
    );
    app.press(KeyCode::Char('w'), KeyModifiers::CONTROL);
    app.type_keys("02-30");
    assert_eq!(
        prompt_box(&mut app)[1..3],
        drawn("", "02-30", "No such day: “02-30”")[1..3],
        "Ctrl+W deleted the word"
    );
    press(&mut app, KeyCode::Enter);
    assert_eq!(prompt_box(&mut app)[1], drawn("", "02-30", "")[1]);

    press(&mut app, KeyCode::Home);
    app.type_keys("2026-");
    press(&mut app, KeyCode::End);
    press(&mut app, KeyCode::Backspace);
    press(&mut app, KeyCode::Backspace);
    app.type_keys("28");
    press(&mut app, KeyCode::Enter);
    assert_eq!(saved(&mut app), "- Milk @due(2026-02-28)\n");
}

// FR-TAG-03: a long text scrolls to keep the caret inside the panel.
#[test]
fn a_long_text_scrolls_inside_the_box() {
    let mut app = Harness::with_text("- Milk\n", 50, 10);
    app.type_keys("md");
    app.type_keys(&"+1d ".repeat(12));
    let text_row = &prompt_box(&mut app)[1];
    assert!(text_row.ends_with("+1d +1d  │"), "{text_row}");
    assert_eq!(
        app.terminal.get_cursor_position().map(|p| (p.x, p.y)).ok(),
        Some((48, 6))
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
    // A wrapped block takes more than one screen line, so the task is found by moving to it.
    for _ in 0..40 {
        let focused = focused_rows(&mut app).concat();
        if focused.contains("Return Piranesi to the library") {
            break;
        }
        app.type_keys("j");
    }
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
    assert!(task.ends_with(&format!("{DUE} tomorrow")), "{task}");
}

// FR-TAG-03, keybindings.md §4: the prompt takes the standard text keys, pasting included.
#[test]
fn text_pasted_into_the_terminal_or_with_ctrl_v_is_typed_into_the_prompt() {
    let mut app = Harness::with_text("12-24\n- Milk\n", 50, 10);
    app.type_keys("yjmd");
    app.send(Input::Terminal(Event::Paste("fri\n".to_owned())));
    assert_eq!(
        prompt_box(&mut app)[1..3],
        drawn("", "fri", "Fri 9 Oct 2026")[1..3],
        "the pasted text replaced the selection"
    );
    app.press(KeyCode::Char('w'), KeyModifiers::CONTROL);
    app.press(KeyCode::Char('v'), KeyModifiers::CONTROL);
    assert_eq!(
        prompt_box(&mut app)[1..3],
        drawn("", "12-24", "Thu 24 Dec 2026")[1..3],
        "what y copied, without its line break"
    );
    press(&mut app, KeyCode::Enter);
    assert_eq!(saved(&mut app), "12-24\n- Milk @due(2026-12-24)\n");
}
