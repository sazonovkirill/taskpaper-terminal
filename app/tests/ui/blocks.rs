//! Step 1.4: blocks. Note runs, sections, URLs, dividers and code drawn as blocks, focus moving by
//! block, and special nodes never shown.

use ratatui::{
    buffer::Buffer,
    style::{Color, Modifier},
};

use crate::support::{Harness, is_focused, outline_line};

fn screen(app: &mut Harness) -> Buffer {
    app.render().buffer().clone()
}

/// The rows with the focus marker.
fn focused_rows(buffer: &Buffer) -> Vec<u16> {
    (buffer.area.top()..buffer.area.bottom())
        .filter(|&y| is_focused(buffer, y))
        .collect()
}

// FR-BLK-01, FR-BLK-02, FR-BLK-03, FR-BLK-04, FR-BLK-05, FR-BLK-06, FR-BLK-07, FR-SPC-01
#[test]
fn draws_every_block_type_and_hides_special_nodes() {
    let mut app = Harness::open("blocks.taskpaper", 40, 14);
    let buffer = screen(&mut app);
    let rule = |from: usize| "─".repeat(38 - from);
    let rows: Vec<String> = (2..13).map(|y| outline_line(&buffer, y)).collect();
    assert_eq!(
        rows,
        [
            "Kitchen:".to_owned(),
            "  Shopping list for the weekend.".to_owned(),
            "  Remember the coupons.".to_owned(),
            "  - Buy flour".to_owned(),
            "  Baking".to_owned(),
            "    - Preheat the oven".to_owned(),
            "  ↗ Recipe site".to_owned(),
            format!("  {}", rule(2)),
            r#"  print("knead")"#.to_owned(),
            r#"  print("bake")"#.to_owned(),
            "  - Clean up".to_owned(),
        ]
    );
    let section = &buffer[(4, 6)];
    assert!(
        section.modifier.contains(Modifier::BOLD),
        "section label is bold"
    );
    assert_eq!(
        section.fg,
        Color::LightMagenta,
        "sections use the secondary color"
    );
    let project = &buffer[(2, 2)];
    assert!(
        project.modifier.contains(Modifier::BOLD),
        "projects are bold"
    );
    assert_eq!(
        project.fg,
        Color::LightBlue,
        "projects use the primary color"
    );
    assert!(
        buffer[(4, 3)].modifier.contains(Modifier::DIM),
        "notes are dimmed"
    );
    let divider = &buffer[(4, 9)];
    assert_eq!(divider.fg, Color::DarkGray, "dividers use the rule color");
    assert!(
        divider.modifier.contains(Modifier::DIM),
        "dividers are dimmer than the header rule"
    );
    let link = &buffer[(6, 8)];
    assert!(
        link.modifier.contains(Modifier::UNDERLINED),
        "URL title is underlined"
    );
    assert_eq!(link.fg, Color::Cyan);
    for (x, y) in [(4, 10), (39, 10), (4, 11), (39, 11)] {
        assert_eq!(buffer[(x, y)].bg, Color::DarkGray, "code band at {x},{y}");
    }
    assert_eq!(
        buffer[(3, 10)].bg,
        Color::Reset,
        "the band starts at the text"
    );
    assert_eq!(focused_rows(&buffer), [2]);
}

// FR-NAV-01, FR-BLK-03
#[test]
fn focus_moves_by_block_and_marks_every_line_of_it() {
    let mut app = Harness::open("blocks.taskpaper", 40, 14);
    app.type_keys("j");
    assert_eq!(focused_rows(&screen(&mut app)), [3, 4]);
    app.type_keys("j");
    assert_eq!(focused_rows(&screen(&mut app)), [5]);
    app.type_keys("jjjjj");
    assert_eq!(focused_rows(&screen(&mut app)), [10, 11], "the code block");
    app.type_keys("j");
    assert_eq!(focused_rows(&screen(&mut app)), [12]);
    app.type_keys("j");
    assert_eq!(
        focused_rows(&screen(&mut app)),
        [12],
        "the special node is not a block"
    );
}

// FR-NAV-01
#[test]
fn scrolling_keeps_a_whole_multi_line_block_in_view() {
    let mut app = Harness::with_text("- a\n- b\nline 1\nline 2\nline 3\n- c\n- d\n- e", 20, 7);
    app.type_keys("jj");
    let buffer = screen(&mut app);
    let rows: Vec<String> = (2..6).map(|y| outline_line(&buffer, y)).collect();
    assert_eq!(rows, ["- b", "line 1", "line 2", "line 3"]);
    assert_eq!(focused_rows(&buffer), [3, 4, 5]);
}

// FR-NAV-01
#[test]
fn a_multi_line_block_that_fits_with_its_margins_does_not_scroll() {
    let mut app = Harness::with_text("- a\n- b\n- c\nl1\nl2\nl3\n- d\n- e\n- f\n- g\n- h", 20, 13);
    app.type_keys("jjj");
    let buffer = screen(&mut app);
    assert_eq!(outline_line(&buffer, 2), "- a");
    assert_eq!(focused_rows(&buffer), [5, 6, 7]);
}

#[test]
fn a_block_taller_than_the_view_shows_its_start() {
    let mut app = Harness::with_text("- a\nl1\nl2\nl3\nl4\nl5\n- b", 20, 6);
    app.type_keys("jj");
    assert_eq!(outline_line(&screen(&mut app), 4), "- b");
    app.type_keys("k");
    let buffer = screen(&mut app);
    let rows: Vec<String> = (2..5).map(|y| outline_line(&buffer, y)).collect();
    assert_eq!(rows, ["- a", "l1", "l2"]);
    assert_eq!(focused_rows(&buffer), [3, 4]);
}

// FR-DOC-05, D-27
#[test]
fn demo_shows_every_block_type() {
    let mut app = Harness::demo(60, 51);
    let text = app.render().to_string();
    for expected in ["↗ ", "─────", "✔ ", "Trip to Lisbon:"] {
        assert!(text.contains(expected), "demo lacks {expected:?}:\n{text}");
    }
    assert!(
        !text.contains("COMMENTS"),
        "a special node is visible:\n{text}"
    );
}
