//! Step 3.2: setting and moving start and due dates with single keys, drawn as chips.

use std::{fs, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    style::{Color, Modifier},
};

use crate::support::{DUE, Harness, OVERDUE, START, outline_line, outline_rows};

/// The open file's text once the autosave delay has passed and its save is done.
fn saved(app: &mut Harness) -> String {
    app.wait(Duration::from_secs(1));
    app.settle();
    let path = app.path.as_ref().expect("a file was opened");
    fs::read_to_string(path).expect("the file is readable")
}

/// The color and modifiers of the last character on screen row `y`, where a row's chips end.
fn chip_style(buffer: &Buffer, y: u16) -> (Color, Modifier) {
    let area = buffer.area;
    (area.left()..area.right())
        .rev()
        .map(|x| &buffer[(x, y)])
        .find(|cell| cell.symbol() != " ")
        .map_or((Color::Reset, Modifier::empty()), |cell| {
            (cell.fg, cell.modifier)
        })
}

/// A due date chip that is not overdue: plain red.
const DUE_STYLE: (Color, Modifier) = (Color::Red, Modifier::empty());

/// An overdue chip: red, bold and reversed.
const OVERDUE_STYLE: (Color, Modifier) = (Color::Red, Modifier::BOLD.union(Modifier::REVERSED));

/// Outline row `text` with `chips` at the right edge of a 40-column terminal.
fn with_chips(text: &str, chips: &str) -> String {
    format!("{text}{chips:>width$}", width = 38 - text.chars().count())
}

// FR-TAG-06, FR-TAG-03, FR-DOC-02, D-53
#[test]
fn shift_w_makes_the_focused_task_due_tomorrow_and_z_undoes_it() {
    let mut app = Harness::with_text("- Milk\n- Bread\n", 40, 5);
    app.type_keys("jW");
    assert_eq!(
        outline_rows(&mut app)[..2],
        [
            "- Milk".to_owned(),
            with_chips("- Bread", &format!("{DUE} tomorrow"))
        ]
    );
    assert_eq!(saved(&mut app), "- Milk\n- Bread @due(2026-10-06)\n");
    app.type_keys("z");
    assert_eq!(outline_rows(&mut app)[1], "- Bread");
    assert_eq!(saved(&mut app), "- Milk\n- Bread\n");
}

// FR-TAG-06, D-26, D-53
#[test]
fn t_w_and_shift_t_set_start_and_due_to_today_or_tomorrow() {
    let mut app = Harness::with_text("- a\n- b\n", 40, 5);
    app.type_keys("t");
    assert_eq!(
        outline_rows(&mut app)[0],
        with_chips("- a", &format!("{START} today"))
    );
    app.type_keys("w");
    assert_eq!(
        outline_rows(&mut app)[0],
        with_chips("- a", &format!("{START} tomorrow"))
    );
    app.type_keys("jT");
    assert_eq!(
        outline_rows(&mut app)[1],
        with_chips("- b", &format!("{DUE} today"))
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
        with_chips("- a", &format!("{START} Sun {DUE} in 25 days"))
    );
    app.type_keys("[}}");
    assert_eq!(saved(&mut app), "- a @start(2026-10-10) @due(2026-11-01)\n");
}

// D-6: dates are stored as YYYY-MM-DD, which has no day before 0000-01-01.
#[test]
fn a_date_does_not_move_before_the_first_day_yyyy_mm_dd_can_write() {
    let mut app = Harness::with_text("- a @due(0000-01-02)\n", 40, 4);
    app.type_keys("{{{");
    assert_eq!(saved(&mut app), "- a @due(0000-01-01)\n");
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
    app.press(KeyCode::Char('w'), KeyModifiers::SHIFT);
    assert_eq!(
        outline_rows(&mut app)[0],
        with_chips("- a", &format!("{DUE} tomorrow"))
    );
    app.press(KeyCode::Char('t'), KeyModifiers::SHIFT);
    app.press(KeyCode::Char('{'), KeyModifiers::SHIFT);
    assert_eq!(
        outline_rows(&mut app)[0],
        with_chips("- a", &format!("{OVERDUE} yesterday"))
    );
}

// FR-TAG-03, D-46
#[test]
fn start_dates_are_green_due_dates_red_and_overdue_ones_reversed() {
    let mut app = Harness::with_text(
        "- a @due(2026-10-04)\n- b @due(2026-10-05)\n- c @start(2026-10-01)\n",
        40,
        6,
    );
    let buffer = app.render().buffer().clone();
    assert_eq!(
        outline_line(&buffer, 2),
        with_chips("- a", &format!("{OVERDUE} yesterday"))
    );
    assert_eq!(chip_style(&buffer, 2), OVERDUE_STYLE);
    assert_eq!(
        outline_line(&buffer, 3),
        with_chips("- b", &format!("{DUE} today"))
    );
    assert_eq!(chip_style(&buffer, 3), DUE_STYLE);
    assert_eq!(
        outline_line(&buffer, 4),
        with_chips("- c", &format!("{START} 4 days ago"))
    );
    assert_eq!(chip_style(&buffer, 4), (Color::Green, Modifier::empty()));
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
    assert_eq!(chip_style(&buffer, 2), DUE_STYLE, "still today");
    app.wait(Duration::from_secs(1));
    let buffer = app.render().buffer().clone();
    assert_eq!(
        outline_line(&buffer, 2),
        with_chips("- a", &format!("{OVERDUE} yesterday"))
    );
    assert_eq!(
        chip_style(&buffer, 2),
        OVERDUE_STYLE,
        "overdue since midnight"
    );
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

// D-46: a start date's chip is named anew at midnight too.
#[test]
fn a_start_date_is_named_relative_to_the_new_day_at_midnight() {
    let mut app = Harness::with_text("- a @start(2026-10-06)\n", 40, 4);
    assert_eq!(
        outline_rows(&mut app)[0],
        with_chips("- a", &format!("{START} tomorrow"))
    );
    assert_eq!(app.tui.deadline(), Some(Duration::from_mins(14 * 60 + 30)));
    app.wait(Duration::from_mins(14 * 60 + 30));
    assert_eq!(
        outline_rows(&mut app)[0],
        with_chips("- a", &format!("{START} today"))
    );
}

#[test]
fn without_a_date_midnight_wakes_nothing() {
    let app = Harness::with_text("- a @star @start(x)\n", 40, 4);
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
        outline_line(&buffer, overdue).ends_with(&format!("{OVERDUE} 2 days ago")),
        "{}",
        outline_line(&buffer, overdue)
    );
    assert_eq!(chip_style(&buffer, overdue), OVERDUE_STYLE);
    let deploy = row("    - Deploy to production");
    assert!(
        outline_line(&buffer, deploy).ends_with(&format!("{START} tomorrow {DUE} Thu")),
        "{}",
        outline_line(&buffer, deploy)
    );
    let piranesi = row("    - Return Piranesi to the library");
    assert!(
        outline_line(&buffer, piranesi).ends_with(&format!("{DUE} next Sat")),
        "{}",
        outline_line(&buffer, piranesi)
    );
    let jacket = row("    - Rain jacket");
    assert!(
        outline_line(&buffer, jacket).ends_with(&format!("{DUE} in 20 days")),
        "{}",
        outline_line(&buffer, jacket)
    );
}
