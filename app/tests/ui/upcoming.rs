//! The Upcoming view: the blocks started or due in the coming week, grouped by day under headings
//! (D-80, D-81).

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::Modifier;

use crate::support::{ARROW, DUE, Harness, START, focused_rows, line, outline_line, outline_rows};

/// Blocks dated in the coming week out of date order, among blocks dated today and later.
const OUTLINE: &str = "\
Home:
\t- Water plants @due(2026-10-08)
\t- Paint the hall @due(2026-10-05)
\t- Post office @start(2026-10-06)
- Bank @due(2026-10-08)
- Taxes @due(2026-10-13)
";

/// `text` with the chips `chips` at the right edge of a row of the 80-column outline.
fn row(text: &str, chips: &str) -> String {
    let width = 78 - text.chars().count();
    format!("{text}{chips:>width$}")
}

/// Opens `text` on an 80×12 screen, on Monday 5 October 2026, in the Upcoming view.
fn upcoming(text: &str) -> Harness {
    let mut app = Harness::with_text(text, 80, 12);
    app.type_keys("gU");
    app
}

// FR-TAG-08, D-81, D-80, D-42, ui.md §1, §2: `gU` shows the Upcoming view under its header, each
// day's entries under a bold heading naming the day, a blank line above every heading but the first.
#[test]
fn g_shift_u_shows_the_blocks_of_the_coming_week_under_a_heading_per_day() {
    let mut app = upcoming(OUTLINE);
    let buffer = app.render().buffer().clone();
    assert_eq!(outline_line(&buffer, 0), format!("{ARROW} Upcoming"));
    assert_eq!(
        outline_rows(&mut app)[..6],
        [
            "Tomorrow · Tue 6 Oct".to_owned(),
            row("- Post office  Home", &format!("{START} tomorrow")),
            String::new(),
            "Thu 8 Oct".to_owned(),
            row("- Water plants  Home", &format!("{DUE} Thu")),
            row("- Bank", &format!("{DUE} Thu")),
        ]
    );
    assert!(
        buffer[(2, 2)].modifier.contains(Modifier::BOLD),
        "the heading is bold"
    );
    assert_eq!(
        focused_rows(&mut app),
        [row("- Post office  Home", &format!("{START} tomorrow"))]
    );
}

// D-81, D-80: the list follows the local date, so midnight redraws it with the new week.
#[test]
fn the_list_changes_at_local_midnight() {
    let mut app = upcoming(OUTLINE);
    assert_eq!(outline_rows(&mut app)[0], "Tomorrow · Tue 6 Oct");
    let deadline = app.tui.deadline().expect("a redraw at midnight");
    app.wait(deadline);
    let rows = outline_rows(&mut app);
    assert_eq!(rows[0], "Thu 8 Oct");
    assert_eq!(rows[4], "Next Tue · Tue 13 Oct");
}

// D-80, ui.md §7: the header's icon is a calendar with an arrow.
#[test]
fn the_header_shows_a_calendar_with_an_arrow() {
    let mut app = Harness::with_config(OUTLINE, "icons = \"nerd\"\n", 80, 12);
    app.type_keys("gU");
    assert_eq!(outline_line(app.render().buffer(), 0), "\u{f1135} Upcoming");
}

// D-80, FR-NAV-06
#[test]
fn go_to_command_lists_go_to_upcoming_with_its_key_and_gives_it() {
    let mut app = Harness::with_text(OUTLINE, 80, 12);
    app.type_keys(":go to upcoming");
    let buffer = app.render().buffer().clone();
    let rows: Vec<String> = (0..buffer.area.height).map(|y| line(&buffer, y)).collect();
    assert!(
        rows.iter()
            .any(|row| row.contains(&format!("│ {ARROW} Go to Upcoming  gU "))),
        "{rows:#?}"
    );
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        outline_line(app.render().buffer(), 0),
        format!("{ARROW} Upcoming")
    );
}

/// The pane's header after choosing the Navigator's second entry in `app`, past Today.
fn second_entry(app: &mut Harness) -> String {
    app.render();
    app.press(KeyCode::Char('g'), KeyModifiers::CONTROL);
    app.press(KeyCode::Down, KeyModifiers::NONE);
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    outline_line(app.render().buffer(), 0)
}

// D-80, D-85, navigation §6: the Navigator lists Upcoming after Today, counting the open tasks it
// lists, unless its setting is off.
#[test]
fn the_navigator_lists_upcoming_after_today_unless_its_setting_is_off() {
    let mut app = Harness::with_text(OUTLINE, 80, 16);
    app.render();
    app.press(KeyCode::Char('g'), KeyModifiers::CONTROL);
    let buffer = app.render().buffer().clone();
    let entry = (0..buffer.area.height)
        .map(|y| line(&buffer, y))
        .find(|row| row.contains(&format!("{ARROW} Upcoming")))
        .expect("listed");
    let window = entry.split('│').nth(1).expect("inside the window");
    assert!(window.trim_end().ends_with(" 3"), "{entry}");
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(second_entry(&mut app), format!("{ARROW} Upcoming"));
    let mut app = Harness::with_config(OUTLINE, "sidebar_upcoming = false\n", 80, 16);
    assert_ne!(second_entry(&mut app), format!("{ARROW} Upcoming"));
    app.type_keys("gU");
    assert_eq!(
        outline_line(app.render().buffer(), 0),
        format!("{ARROW} Upcoming"),
        "gU still opens it"
    );
}
