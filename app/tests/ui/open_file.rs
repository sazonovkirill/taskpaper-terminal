//! Step 1.1: opening a file, showing it as an outline, the demo, and quitting.

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{buffer::Buffer, layout::Rect, style::Style};

use crate::support::Harness;

// F-FILE-01, F-FILE-02, FR-DOC-01, FR-DOC-03
#[test]
fn shows_the_file_as_an_indented_outline() {
    let mut app = Harness::open("groceries.taskpaper", 36, 10);
    let mut expected = Buffer::with_lines([
        "  ⌂                                 ",
        "────────────────────────────────────",
        "▎ Groceries:                        ",
        "    - Milk                          ",
        "    - Bread                         ",
        "      Wholegrain if they have it.   ",
        "    Dairy:                          ",
        "      - Cheese                      ",
        "  Notes about the week.             ",
        &app.status(36),
    ]);
    let project = Style::new().bold().light_blue();
    expected.set_style(Rect::new(2, 0, 34, 1), Style::new().bold());
    expected.set_style(Rect::new(0, 1, 36, 1), Style::new().dark_gray());
    expected.set_style(Rect::new(2, 2, 10, 1), project);
    expected.set_style(Rect::new(6, 5, 27, 1), Style::new().dim());
    expected.set_style(Rect::new(4, 6, 6, 1), project);
    expected.set_style(Rect::new(2, 8, 21, 1), Style::new().dim());
    expected.set_style(Rect::new(0, 2, 1, 1), Style::new().light_blue());
    expected.set_style(Rect::new(0, 9, 8, 1), Style::new().bold());
    app.render().assert_buffer(&expected);
}

// FR-DOC-05
#[test]
fn shows_the_demo_outline_without_a_path() {
    let mut app = Harness::demo(40, 6);
    let mut expected = Buffer::with_lines([
        "  ⌂                                     ",
        "────────────────────────────────────────",
        "▎ Trip to Lisbon:                       ",
        "    - Book flights                      ",
        "    - Reserve a hotel near Alfama       ",
        "NAVIGATE · demo, in memory only         ",
    ]);
    expected.set_style(Rect::new(2, 0, 38, 1), Style::new().bold());
    expected.set_style(Rect::new(0, 1, 40, 1), Style::new().dark_gray());
    expected.set_style(Rect::new(2, 2, 15, 1), Style::new().bold().light_blue());
    expected.set_style(Rect::new(0, 2, 1, 1), Style::new().light_blue());
    expected.set_style(Rect::new(0, 5, 8, 1), Style::new().bold());
    app.render().assert_buffer(&expected);
}

#[test]
fn ctrl_q_quits() {
    let mut app = Harness::demo(20, 2);
    assert!(!app.tui.should_quit());
    app.press(KeyCode::Char('q'), KeyModifiers::NONE);
    assert!(!app.tui.should_quit(), "plain q must not quit");
    app.press(KeyCode::Char('q'), KeyModifiers::CONTROL);
    assert!(app.tui.should_quit());
}
