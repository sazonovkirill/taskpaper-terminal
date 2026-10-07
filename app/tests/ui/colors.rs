//! The Colors view: the blocks with a color tag, grouped by color under headings (D-80, D-83).

use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::style::{Color, Modifier};

use crate::support::{Harness, PALETTE, focused_rows, outline_line, outline_rows};

/// The color swatch of a group's heading, as drawn with icons off (ui.md §7, D-83).
const SWATCH: &str = "●";

/// Colored blocks out of the colors' order, among blocks without a color.
const OUTLINE: &str = "\
Home:
\t- Water plants @blue
\t- Paint the hall
\t- Post office @red
- Bank @blue
";

/// Opens `text` on an 80×12 screen in the Colors view.
fn colors(text: &str) -> Harness {
    let mut app = Harness::with_text(text, 80, 12);
    app.type_keys("gC");
    app
}

// FR-TAG-10, D-83, D-80, ui.md §1, §2: `gC` shows the Colors view under its header, each color's
// entries under a bold heading of the color's swatch and name, a blank line above every heading but
// the first.
#[test]
fn g_shift_c_shows_the_colored_blocks_under_a_heading_per_color() {
    let mut app = colors(OUTLINE);
    let buffer = app.render().buffer().clone();
    assert_eq!(outline_line(&buffer, 0), format!("{PALETTE} Colors"));
    assert_eq!(
        outline_rows(&mut app)[..6],
        [
            format!("{SWATCH} Red"),
            "- Post office  Home".to_owned(),
            String::new(),
            format!("{SWATCH} Blue"),
            "- Water plants  Home".to_owned(),
            "- Bank".to_owned(),
        ]
    );
    assert!(
        buffer[(2, 2)].modifier.contains(Modifier::BOLD),
        "the heading is bold"
    );
    assert_eq!(focused_rows(&mut app), ["- Post office  Home"]);
}

// D-83, ui.md §7: a heading's icon is the color's swatch, as the color picker shows it, in that
// color.
#[test]
fn a_heading_shows_the_colors_swatch_in_that_color() {
    let mut app = Harness::with_config(OUTLINE, "icons = \"nerd\"\n", 80, 12);
    app.type_keys("gC");
    let buffer = app.render().buffer().clone();
    assert_eq!(outline_line(&buffer, 2), "\u{f03d8} Red");
    assert_eq!(buffer[(2, 2)].fg, Color::Red);
    assert_eq!(outline_line(&buffer, 5), "\u{f03d8} Blue");
    assert_eq!(buffer[(2, 5)].fg, Color::Blue);
}

// D-80, navigation §7.1: focus, motion and paging skip the headings.
#[test]
fn motion_moves_from_entry_to_entry_past_the_headings() {
    let mut app = colors(OUTLINE);
    app.type_keys("j");
    assert_eq!(focused_rows(&mut app), ["- Water plants  Home"]);
    app.type_keys("k");
    assert_eq!(focused_rows(&mut app), ["- Post office  Home"]);
    app.press(KeyCode::PageDown, KeyModifiers::NONE);
    assert_eq!(focused_rows(&mut app), ["- Bank".to_owned()]);
}

// D-80, FR-KEY-05: a click on a heading focuses nothing; one on an entry focuses it.
#[test]
fn a_click_on_a_heading_does_nothing() {
    let mut app = colors(OUTLINE);
    app.render();
    let click = |app: &mut Harness, y| {
        app.mouse(
            MouseEventKind::Down(MouseButton::Left),
            10,
            y,
            KeyModifiers::NONE,
        );
        app.mouse(
            MouseEventKind::Up(MouseButton::Left),
            10,
            y,
            KeyModifiers::NONE,
        );
    };
    click(&mut app, 5);
    assert_eq!(focused_rows(&mut app), ["- Post office  Home"]);
    click(&mut app, 6);
    assert_eq!(focused_rows(&mut app), ["- Water plants  Home"]);
    click(&mut app, 2);
    click(&mut app, 3);
    assert_eq!(
        focused_rows(&mut app),
        ["- Post office  Home"],
        "the first heading takes one line"
    );
}

// FR-MODE-01, D-80: an entry below a heading is edited on its own line.
#[test]
fn the_caret_of_an_edited_entry_is_on_its_line_below_the_heading() {
    let mut app = colors(OUTLINE);
    app.type_keys("j");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    app.render();
    app.terminal.backend_mut().assert_cursor_position((16, 6));
}

// D-80, FR-NAV-06
#[test]
fn go_to_command_lists_go_to_colors_with_its_key_and_gives_it() {
    let mut app = Harness::with_text(OUTLINE, 80, 12);
    app.type_keys(":go to colors");
    let buffer = app.render().buffer().clone();
    let rows: Vec<String> = (0..buffer.area.height)
        .map(|y| crate::support::line(&buffer, y))
        .collect();
    assert!(
        rows.iter()
            .any(|row| row.contains(&format!("│ {PALETTE} Go to Colors  gC "))),
        "{rows:#?}"
    );
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    let buffer = app.render().buffer().clone();
    assert_eq!(outline_line(&buffer, 0), format!("{PALETTE} Colors"));
}

// D-85, D-80: the Navigator's Colors entry counts the open tasks the view lists.
#[test]
fn the_navigator_entry_counts_the_open_tasks_it_lists() {
    let mut app = Harness::with_text(OUTLINE, 80, 16);
    app.render();
    app.press(KeyCode::Char('g'), KeyModifiers::CONTROL);
    let buffer = app.render().buffer().clone();
    let entry = (0..buffer.area.height)
        .map(|y| crate::support::line(&buffer, y))
        .find(|row| row.contains(&format!("{PALETTE} Colors")))
        .expect("listed");
    assert!(entry.trim_end().ends_with("3 │"), "{entry}");
}
