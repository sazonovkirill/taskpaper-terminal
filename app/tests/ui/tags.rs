//! Step 1.3: tags hidden from rows, and done tasks.

use ratatui::{buffer::Buffer, layout::Rect, style::Style};

use crate::support::{CURRENT_LINE, Harness};

// FR-TAG-01, FR-TAG-02, FR-TAG-04, FR-DONE-01, D-3, D-17, D-40
#[test]
fn hides_tags_and_shows_done_tasks_checked_dimmed_and_struck_through() {
    let mut app = Harness::open("tags.taskpaper", 40, 10);
    let mut expected = Buffer::with_lines([
        "  ⌂                                     ",
        "────────────────────────────────────────",
        "▎ Errands:                              ",
        "    - Call about the lease              ",
        "    ✔ Post the letter                   ",
        "    ✔ Buy stamps                        ",
        "    ✔ Return books                     ★",
        "    Paid:                               ",
        "    mail me@example.com about Proj:@x   ",
        &app.status(40),
    ]);
    let done = Style::new().dim().crossed_out();
    let project = Style::new().bold().light_blue();
    expected.set_style(Rect::new(2, 0, 38, 1), Style::new().bold());
    expected.set_style(Rect::new(0, 1, 40, 1), Style::new().dark_gray());
    expected.set_style(Rect::new(0, 2, 40, 1), Style::new().bg(CURRENT_LINE));
    expected.set_style(Rect::new(2, 2, 8, 1), project);
    expected.set_style(Rect::new(4, 4, 2, 3), Style::new().dim());
    expected.set_style(Rect::new(6, 4, 15, 1), done);
    expected.set_style(Rect::new(6, 5, 10, 1), done);
    expected.set_style(Rect::new(6, 6, 12, 1), done);
    expected.set_style(Rect::new(39, 6, 1, 1), Style::new().yellow().dim());
    expected.set_style(Rect::new(4, 7, 5, 1), project.dim().crossed_out());
    expected.set_style(Rect::new(4, 8, 33, 1), Style::new().gray());
    expected.set_style(Rect::new(0, 2, 1, 1), Style::new().light_blue());
    expected.set_style(Rect::new(0, 9, 8, 1), Style::new().bold());
    app.render().assert_buffer(&expected);
}

// FR-DOC-05, FR-TAG-01, FR-DONE-01
#[test]
fn demo_hides_its_tags_and_shows_a_done_task() {
    let mut app = Harness::demo(40, 11);
    let screen = app.render().to_string();
    assert!(!screen.contains('@'), "a tag is visible:\n{screen}");
    assert!(screen.contains("✔ Passport"), "no done task:\n{screen}");
}
