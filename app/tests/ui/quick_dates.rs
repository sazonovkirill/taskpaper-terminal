//! Step 3.2: setting and moving start and due dates with single keys, drawn as chips.

use std::{fs, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{buffer::Buffer, style::Color};

use crate::support::{Harness, outline_line, outline_rows};

/// The open file's text once the autosave delay has passed and its save is done.
fn saved(app: &mut Harness) -> String {
    app.wait(Duration::from_secs(1));
    app.settle();
    let path = app.path.as_ref().expect("a file was opened");
    fs::read_to_string(path).expect("the file is readable")
}

/// The color of the last character on screen row `y`, where a row's chips end.
fn chip_color(buffer: &Buffer, y: u16) -> Color {
    let area = buffer.area;
    (area.left()..area.right())
        .rev()
        .map(|x| &buffer[(x, y)])
        .find(|cell| cell.symbol() != " ")
        .map_or(Color::Reset, |cell| cell.fg)
}

/// Outline row `text` with `chips` at the right edge of a 40-column terminal.
fn with_chips(text: &str, chips: &str) -> String {
    format!("{text}{chips:>width$}", width = 38 - text.chars().count())
}

// FR-TAG-06, FR-TAG-03, FR-DOC-02
#[test]
fn shift_y_makes_the_focused_task_due_tomorrow_and_z_undoes_it() {
    let mut app = Harness::with_text("- Milk\n- Bread\n", 40, 5);
    app.type_keys("jY");
    assert_eq!(
        outline_rows(&mut app)[..2],
        ["- Milk".to_owned(), with_chips("- Bread", "due Tue 6 Oct")]
    );
    assert_eq!(saved(&mut app), "- Milk\n- Bread @due(2026-10-06)\n");
    app.type_keys("z");
    assert_eq!(outline_rows(&mut app)[1], "- Bread");
    assert_eq!(saved(&mut app), "- Milk\n- Bread\n");
}

// FR-TAG-06, D-26
#[test]
fn t_y_and_shift_t_set_start_and_due_to_today_or_tomorrow() {
    let mut app = Harness::with_text("- a\n- b\n", 40, 5);
    app.type_keys("t");
    assert_eq!(
        outline_rows(&mut app)[0],
        with_chips("- a", "start Mon 5 Oct")
    );
    app.type_keys("y");
    assert_eq!(
        outline_rows(&mut app)[0],
        with_chips("- a", "start Tue 6 Oct")
    );
    app.type_keys("jT");
    assert_eq!(
        outline_rows(&mut app)[1],
        with_chips("- b", "due Mon 5 Oct")
    );
    assert_eq!(
        saved(&mut app),
        "- a @start(2026-10-06)\n- b @due(2026-10-05)\n"
    );
}

// FR-TAG-06, D-26, editing.md §5
#[test]
fn brackets_and_braces_move_the_start_and_due_dates_a_day() {
    let mut app = Harness::with_text("- a @start(2026-10-09) @due(2026-10-31)\n", 40, 4);
    app.type_keys("]]");
    app.type_keys("{");
    assert_eq!(
        outline_rows(&mut app)[0],
        with_chips("- a", "start Sun 11 Oct due Fri 30 Oct")
    );
    app.type_keys("[}}");
    assert_eq!(saved(&mut app), "- a @start(2026-10-10) @due(2026-11-01)\n");
}

// editing.md §5
#[test]
fn moving_a_missing_date_counts_from_today() {
    let mut app = Harness::with_text("- a\n", 50, 4);
    app.type_keys("[}");
    assert_eq!(saved(&mut app), "- a @start(2026-10-04) @due(2026-10-06)\n");
}

// D-42: under the kitty keyboard protocol a shifted letter may arrive lowercase with `Shift`.
#[test]
fn shifted_date_keys_work_in_both_key_reportings() {
    let mut app = Harness::with_text("- a\n", 40, 4);
    app.press(KeyCode::Char('y'), KeyModifiers::SHIFT);
    assert_eq!(
        outline_rows(&mut app)[0],
        with_chips("- a", "due Tue 6 Oct")
    );
    app.press(KeyCode::Char('t'), KeyModifiers::SHIFT);
    app.press(KeyCode::Char('{'), KeyModifiers::SHIFT);
    assert_eq!(
        outline_rows(&mut app)[0],
        with_chips("- a", "due Sun 4 Oct")
    );
}

// FR-TAG-03
#[test]
fn an_overdue_due_date_is_red_and_a_coming_one_is_not() {
    let mut app = Harness::with_text("- a @due(2026-10-04)\n- b @due(2026-10-05)\n", 40, 5);
    let buffer = app.render().buffer().clone();
    assert_eq!(outline_line(&buffer, 2), with_chips("- a", "due Sun 4 Oct"));
    assert_eq!(chip_color(&buffer, 2), Color::Red);
    assert_eq!(chip_color(&buffer, 3), Color::Cyan);
}

// FR-TAG-03, search.md §4: the date chips follow the local date past midnight.
#[test]
fn a_due_date_turns_overdue_at_midnight_without_input() {
    let mut app = Harness::with_text("- a @due(2026-10-05)\n", 40, 4);
    // The harness starts at 09:30.
    let until_midnight = Duration::from_mins(14 * 60 + 30);
    assert_eq!(app.tui.deadline(), Some(until_midnight));
    app.wait(Duration::from_mins(14 * 60 + 29));
    app.wait(Duration::from_secs(59));
    let buffer = app.render().buffer().clone();
    assert_eq!(chip_color(&buffer, 2), Color::Cyan, "still today");
    app.wait(Duration::from_secs(1));
    let buffer = app.render().buffer().clone();
    assert_eq!(chip_color(&buffer, 2), Color::Red, "overdue since midnight");
    assert_eq!(
        app.tui.deadline(),
        Some(until_midnight + Duration::from_hours(24)),
        "the next midnight"
    );
    app.type_keys("T");
    assert_eq!(
        saved(&mut app),
        "- a @due(2026-10-06)\n",
        "today is the new day"
    );
}

#[test]
fn without_a_due_date_midnight_wakes_nothing() {
    let app = Harness::with_text("- a @start(2026-10-05)\n", 40, 4);
    assert_eq!(app.tui.deadline(), None);
}

// FR-DOC-05, FR-TAG-03, FR-TAG-06
#[test]
fn the_demo_shows_dates_and_an_overdue_task() {
    let mut app = Harness::demo(70, 40);
    let buffer = app.render().buffer().clone();
    let row = |text: &str| {
        (2..39)
            .find(|&y| outline_line(&buffer, y).starts_with(text))
            .unwrap_or_else(|| panic!("the demo has {text:?}"))
    };
    let overdue = row("  - Fix the broken contact form");
    assert!(
        outline_line(&buffer, overdue).ends_with("due Sat 3 Oct"),
        "{}",
        outline_line(&buffer, overdue)
    );
    assert_eq!(chip_color(&buffer, overdue), Color::Red);
    let deploy = row("    - Deploy to production");
    assert!(
        outline_line(&buffer, deploy).ends_with("start Tue 6 Oct due Thu 8 Oct"),
        "{}",
        outline_line(&buffer, deploy)
    );
}
