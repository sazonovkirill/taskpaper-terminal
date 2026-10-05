//! Step 1.4: blocks. Every line drawn as a block of its type, focus moving by block, and special
//! nodes never shown (D-43).

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

// FR-BLK-01, FR-BLK-02, FR-BLK-04, FR-BLK-06, FR-BLK-07, FR-SPC-01, D-43
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
            "  Recipe site".to_owned(),
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
    let text = &buffer[(4, 3)];
    assert_eq!(text.fg, Color::Gray, "text is dimmed");
    assert!(
        !text.modifier.contains(Modifier::DIM),
        "text is less dim than done tasks"
    );
    let divider = &buffer[(4, 9)];
    assert_eq!(divider.fg, Color::DarkGray, "dividers use the rule color");
    assert!(
        divider.modifier.contains(Modifier::DIM),
        "dividers are dimmer than the header rule"
    );
    let link = &buffer[(4, 8)];
    assert!(
        link.modifier.contains(Modifier::UNDERLINED),
        "URL title is underlined"
    );
    assert_eq!(link.fg, Color::Cyan);
    for (x, y) in [(4, 10), (39, 10), (4, 11)] {
        let cell = &buffer[(x, y)];
        assert_eq!(cell.bg, Color::Reset, "no code band at {x},{y}");
    }
    assert_eq!(buffer[(4, 10)].fg, Color::Gray, "a code line is text");
    assert_eq!(focused_rows(&buffer), [2]);
}

// FR-NAV-01, D-43
#[test]
fn focus_moves_by_block_one_line_at_a_time() {
    let mut app = Harness::open("blocks.taskpaper", 40, 14);
    for row in 3..=12 {
        app.type_keys("j");
        assert_eq!(focused_rows(&screen(&mut app)), [row]);
    }
    app.type_keys("j");
    assert_eq!(
        focused_rows(&screen(&mut app)),
        [12],
        "the special node is not a block"
    );
}

// FR-DOC-05, D-27
#[test]
fn demo_shows_every_block_type() {
    let mut app = Harness::demo(60, 51);
    let text = app.render().to_string();
    for expected in ["Lisbon travel guide", "─────", "✔ ", "Trip to Lisbon:"] {
        assert!(text.contains(expected), "demo lacks {expected:?}:\n{text}");
    }
    assert!(
        !text.contains("COMMENTS"),
        "a special node is visible:\n{text}"
    );
}
