//! Step 1.3: tags hidden from rows, and done tasks.

use ratatui::{buffer::Buffer, layout::Rect, style::Style};

use crate::support::Harness;

// FR-TAG-01, FR-TAG-02, FR-DONE-01, D-3, D-17
#[test]
fn hides_tags_and_shows_done_tasks_checked_dimmed_and_struck_through() {
    let mut app = Harness::open("tags.taskpaper", 40, 8);
    let mut expected = Buffer::with_lines([
        "▾ Errands                               ",
        "  ☐ Call about the lease                ",
        "  ☑ Post the letter                     ",
        "  ☑ Buy stamps                          ",
        "  ☑ Return books                        ",
        "  ▾ Paid                                ",
        "    mail me@example.com about Proj:@x   ",
        &app.status(40),
    ]);
    let done = Style::new().dim().crossed_out();
    expected.set_style(Rect::new(2, 0, 7, 1), Style::new().bold());
    expected.set_style(Rect::new(4, 2, 15, 1), done);
    expected.set_style(Rect::new(4, 3, 10, 1), done);
    expected.set_style(Rect::new(4, 4, 12, 1), done);
    expected.set_style(Rect::new(4, 5, 4, 1), Style::new().bold());
    expected.set_style(Rect::new(0, 0, 40, 1), Style::new().reversed());
    expected.set_style(Rect::new(0, 7, 8, 1), Style::new().bold());
    app.render().assert_buffer(&expected);
}

// FR-DOC-05, FR-TAG-01, FR-DONE-01
#[test]
fn demo_hides_its_tags_and_shows_a_done_task() {
    let mut app = Harness::demo(40, 9);
    let screen = app.render().to_string();
    assert!(!screen.contains('@'), "a tag is visible:\n{screen}");
    assert!(screen.contains("☑ Passport"), "no done task:\n{screen}");
}
