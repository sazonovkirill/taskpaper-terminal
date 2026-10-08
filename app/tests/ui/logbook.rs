//! The Logbook view: the blocks completed in the last 7 days, grouped by day of completion, newest
//! first (D-80, D-84).

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::Modifier;

use crate::support::{BOOK, Harness, outline_line, outline_rows};

/// Blocks completed today, yesterday, 6 and 7 days ago and with no date, on
/// [`crate::support::TODAY`], a Monday, among open blocks.
const OUTLINE: &str = "\
Home: @done(2026-10-04)
\t- Water plants @done(2026-10-05)
\t- Paint the hall
- Bank @done(2026-09-28)
- Post office @done
- Plan the week @done(2026-10-05)
- Bills @done(2026-09-29)
";

/// The rows on screen in the Logbook of `OUTLINE`.
const LISTED: [&str; 9] = [
    "Today · Mon 5 Oct",
    "✔ Water plants  Home",
    "✔ Plan the week",
    "",
    "Yesterday · Sun 4 Oct",
    "Home:",
    "",
    "6 days ago · Tue 29 Sep",
    "✔ Bills",
];

// FR-TAG-11, D-84, D-80, D-42, ui.md §1, §2: `gL` shows the Logbook under its header, with and
// without the kitty protocol: each day's completions under a bold heading naming the day, newest
// first, a blank line above every heading but the first.
#[test]
fn g_shift_l_shows_the_last_weeks_completions_under_a_heading_per_day() {
    for kitty in [false, true] {
        for shift in [KeyModifiers::NONE, KeyModifiers::SHIFT] {
            let mut app = Harness::with_text(OUTLINE, 80, 12);
            app.tui = app.tui.with_kitty_keyboard(kitty);
            app.type_keys("g");
            app.press(KeyCode::Char('L'), shift);
            let buffer = app.render().buffer().clone();
            assert_eq!(outline_line(&buffer, 0), format!("{BOOK} Logbook"));
            assert_eq!(outline_rows(&mut app), LISTED, "kitty {kitty}, {shift:?}");
            assert!(
                buffer[(2, 2)].modifier.contains(Modifier::BOLD),
                "the heading is bold"
            );
        }
    }
}

// D-84, FR-DONE-02: hiding completed blocks leaves the Logbook as it is.
#[test]
fn hiding_completed_blocks_leaves_the_list_as_it_is() {
    let mut app = Harness::with_text(OUTLINE, 80, 12);
    app.type_keys("gL");
    let before = outline_rows(&mut app);
    app.press(KeyCode::Char('o'), KeyModifiers::CONTROL);
    assert_eq!(outline_rows(&mut app), before);
}

// D-84, D-80, D-46: at local midnight the days move on, after a frame drawn the day before: the
// headings name the days anew and a completion 7 days old leaves.
#[test]
fn the_list_and_its_headings_change_at_local_midnight() {
    let mut app = Harness::with_text(OUTLINE, 80, 12);
    app.type_keys("gL");
    assert_eq!(outline_rows(&mut app), LISTED);
    let deadline = app.tui.deadline().expect("a redraw at midnight");
    app.wait(deadline);
    assert_eq!(
        outline_rows(&mut app),
        [
            "Yesterday · Mon 5 Oct",
            "✔ Water plants  Home",
            "✔ Plan the week",
            "",
            "2 days ago · Sun 4 Oct",
            "Home:",
            "",
            "",
            "",
        ]
    );
}

// FR-TAG-07, D-80, D-85, ui.md §7: the Navigator lists Logbook after Colors with its icon and no
// count, since it lists no open task, and choosing it shows it; with `sidebar_logbook` off it is
// left out, and `gL` still opens it.
#[test]
fn the_navigator_lists_logbook_unless_its_setting_is_off() {
    let mut app = Harness::with_text(OUTLINE, 80, 16);
    app.render();
    app.press(KeyCode::Char('g'), KeyModifiers::CONTROL);
    let buffer = app.render().buffer().clone();
    let rows: Vec<String> = (0..buffer.area.height)
        .map(|y| crate::support::line(&buffer, y))
        .collect();
    let at = rows
        .iter()
        .position(|row| row.contains(&format!("│  {BOOK} Logbook ")))
        .expect("listed");
    assert!(rows[at - 1].contains(" Colors "), "{rows:#?}");
    assert!(
        rows[at].trim_end().ends_with(" │"),
        "no count: {}",
        rows[at]
    );
    for _ in 0..4 {
        app.press(KeyCode::Down, KeyModifiers::NONE);
    }
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        outline_line(app.render().buffer(), 0),
        format!("{BOOK} Logbook")
    );
    let mut app = Harness::with_config(OUTLINE, "sidebar_logbook = false\n", 80, 16);
    app.render();
    app.press(KeyCode::Char('g'), KeyModifiers::CONTROL);
    let buffer = app.render().buffer().clone();
    assert!(
        !(0..buffer.area.height).any(|y| crate::support::line(&buffer, y).contains("Logbook")),
        "left out"
    );
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    app.type_keys("gL");
    assert_eq!(
        outline_line(app.render().buffer(), 0),
        format!("{BOOK} Logbook"),
        "gL still opens it"
    );
}

// D-80, FR-NAV-06, ui.md §7
#[test]
fn go_to_command_lists_go_to_logbook_with_its_key_and_gives_it() {
    let mut app = Harness::with_text(OUTLINE, 80, 12);
    app.type_keys(":go to logbook");
    let buffer = app.render().buffer().clone();
    let rows: Vec<String> = (0..buffer.area.height)
        .map(|y| crate::support::line(&buffer, y))
        .collect();
    assert!(
        rows.iter()
            .any(|row| row.contains(&format!("│ {BOOK} Go to Logbook  gL "))),
        "{rows:#?}"
    );
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        outline_line(app.render().buffer(), 0),
        format!("{BOOK} Logbook")
    );
}

// ui.md §7, D-80: with Nerd Font icons the Logbook's icon is a book with a check.
#[test]
fn with_nerd_font_icons_the_header_shows_a_book_with_a_check() {
    let mut app = Harness::with_config(OUTLINE, "icons = \"nerd\"\n", 80, 12);
    app.type_keys("gL");
    assert_eq!(outline_line(app.render().buffer(), 0), "\u{f14f3} Logbook");
}
