//! The Overdue view: the open blocks whose due date has passed, grouped by due date under
//! headings, opened with `go` or from the Navigator (D-80, D-82).

use std::time::Duration;

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::Modifier;

use crate::support::{Harness, OVERDUE, focused_rows, line, outline_line, outline_rows};

/// Overdue blocks out of date order among one due today and a done one, on a Monday.
const OUTLINE: &str = "\
Home:
\t- Water plants @due(2026-10-02)
\t- Paint the hall @due(2026-10-05)
\t- Post office @due(2026-10-04)
- Bank @due(2026-10-02)
- Old @due(2026-10-01) @done(2026-10-02)
";

/// The Overdue view's rows on a 60×12 screen, below its header: each due date's heading, then
/// its entries.
fn listed() -> Vec<String> {
    vec![
        "3 days ago · Fri 2 Oct".to_owned(),
        format!("- Water plants  Home                          {OVERDUE} 3 days ago"),
        format!("- Bank                                        {OVERDUE} 3 days ago"),
        String::new(),
        "Yesterday · Sun 4 Oct".to_owned(),
        format!("- Post office  Home                            {OVERDUE} yesterday"),
        String::new(),
        String::new(),
        String::new(),
    ]
}

/// Opens `text` on a 60×12 screen in the Overdue view.
fn overdue(text: &str) -> Harness {
    let mut app = Harness::with_text(text, 60, 12);
    app.type_keys("go");
    app
}

// FR-TAG-09, D-82, D-80, D-42, ui.md §1, §2: `go` shows the open blocks due before today under
// the Overdue header, oldest due date first, each date's entries in document order under a bold
// heading naming the date as a chip does and then the date.
#[test]
fn g_o_shows_the_overdue_blocks_under_a_heading_per_due_date() {
    let mut app = overdue(OUTLINE);
    let buffer = app.render().buffer().clone();
    assert_eq!(outline_line(&buffer, 0), format!("{OVERDUE} Overdue"));
    assert_eq!(outline_rows(&mut app), listed());
    assert!(
        buffer[(2, 2)].modifier.contains(Modifier::BOLD),
        "the heading is bold"
    );
    assert_eq!(
        focused_rows(&mut app),
        [format!(
            "- Water plants  Home                          {OVERDUE} 3 days ago"
        )]
    );
}

// D-80, ui.md §7: the view's icon is the overdue chip's, in its color.
#[test]
fn the_header_shows_the_overdue_chips_icon_in_its_color() {
    let mut app = Harness::with_config(OUTLINE, "icons = \"nerd\"\n", 60, 12);
    app.type_keys("go");
    let buffer = app.render().buffer().clone();
    assert_eq!(outline_line(&buffer, 0), "\u{f0a31} Overdue");
    let chip = (0..60)
        .find(|&x| buffer[(x, 3)].symbol() == "\u{f0a31}")
        .expect("an overdue chip");
    assert_eq!(buffer[(2, 0)].fg, buffer[(chip, 3)].fg);
}

// D-82, FR-DONE-02: done blocks never show, and hiding completed blocks changes nothing.
#[test]
fn the_list_is_the_same_whether_completed_blocks_show_or_not() {
    let mut app = overdue(OUTLINE);
    app.press(KeyCode::Char('o'), KeyModifiers::CONTROL);
    assert_eq!(outline_rows(&mut app), listed());
}

// D-82, D-80: at midnight a block due the day before joins the view under its own heading, and
// the headings name their dates anew, without input.
#[test]
fn at_midnight_the_blocks_due_the_day_before_join_and_the_headings_follow() {
    let mut app = overdue(OUTLINE);
    assert_eq!(outline_rows(&mut app), listed(), "a frame before midnight");
    // The harness starts at 09:30.
    let until_midnight = Duration::from_mins(14 * 60 + 30);
    assert_eq!(app.tui.deadline(), Some(until_midnight));
    app.wait(until_midnight);
    assert_eq!(
        outline_rows(&mut app),
        [
            "4 days ago · Fri 2 Oct".to_owned(),
            format!("- Water plants  Home                          {OVERDUE} 4 days ago"),
            format!("- Bank                                        {OVERDUE} 4 days ago"),
            String::new(),
            "2 days ago · Sun 4 Oct".to_owned(),
            format!("- Post office  Home                           {OVERDUE} 2 days ago"),
            String::new(),
            "Yesterday · Mon 5 Oct".to_owned(),
            format!("- Paint the hall  Home                         {OVERDUE} yesterday"),
        ]
    );
}

// D-82: completing an entry takes it out of the view.
#[test]
fn completing_an_entry_takes_it_out() {
    let mut app = overdue(OUTLINE);
    app.type_keys("x");
    assert_eq!(
        outline_rows(&mut app)[..2],
        [
            "3 days ago · Fri 2 Oct".to_owned(),
            format!("- Bank                                        {OVERDUE} 3 days ago"),
        ]
    );
}

// D-80, FR-NAV-06
#[test]
fn go_to_command_lists_go_to_overdue_with_its_key_and_gives_it() {
    let mut app = Harness::with_text(OUTLINE, 80, 12);
    app.type_keys(":go to overdue");
    let buffer = app.render().buffer().clone();
    let rows: Vec<String> = (0..buffer.area.height).map(|y| line(&buffer, y)).collect();
    assert!(
        rows.iter()
            .any(|row| row.contains(&format!("│ {OVERDUE} Go to Overdue  go "))),
        "{rows:#?}"
    );
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    let buffer = app.render().buffer().clone();
    assert_eq!(outline_line(&buffer, 0), format!("{OVERDUE} Overdue"));
}

/// The Navigator's lines on a 60×18 screen showing `text` with `settings`.
fn navigator(text: &str, settings: &str) -> (Harness, Vec<String>) {
    let mut app = Harness::with_config(text, settings, 60, 18);
    app.render();
    app.press(KeyCode::Char('g'), KeyModifiers::CONTROL);
    let buffer = app.render().buffer().clone();
    let rows = (0..buffer.area.height).map(|y| line(&buffer, y)).collect();
    (app, rows)
}

// D-85, D-80, navigation §6: the Navigator lists Overdue after Today with the open tasks it
// lists counted, and choosing it shows it.
#[test]
fn the_navigator_entry_counts_the_open_tasks_it_lists_and_opens_the_view() {
    let (mut app, rows) = navigator(OUTLINE, "");
    let at = |name: &str| rows.iter().position(|row| row.contains(name));
    let entry = at(&format!("{OVERDUE} Overdue")).expect("listed");
    assert_eq!(at(" Today"), Some(entry - 1), "after Today");
    assert!(
        rows[entry].contains(&format!("{OVERDUE} Overdue{:33}3 │", "")),
        "{}",
        rows[entry]
    );
    app.press(KeyCode::Down, KeyModifiers::NONE);
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    let buffer = app.render().buffer().clone();
    assert_eq!(outline_line(&buffer, 0), format!("{OVERDUE} Overdue"));
}

// D-80, ui.md §8: `sidebar_overdue = false` drops the entry, and `go` still opens the view.
#[test]
fn with_its_setting_off_the_navigator_leaves_it_out_and_g_o_still_opens_it() {
    let (mut app, rows) = navigator(OUTLINE, "sidebar_overdue = false\n");
    assert!(
        !rows.iter().any(|row| row.contains(" Overdue")),
        "{rows:#?}"
    );
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    app.type_keys("go");
    let buffer = app.render().buffer().clone();
    assert_eq!(outline_line(&buffer, 0), format!("{OVERDUE} Overdue"));
}
