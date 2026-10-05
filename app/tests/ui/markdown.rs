//! Step 3.10: Markdown headings and links in text blocks, drawn with their markers hidden in
//! navigate mode and raw in edit mode.

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::{Color, Modifier};

use crate::support::{Harness, line, outline_rows};

// FR-FMT-03
#[test]
fn text_blocks_draw_headings_and_links_without_markers_and_edit_mode_shows_them() {
    let mut app = Harness::with_text(
        "## Plan\n\tSee [the guide](https://a.b) or <https://c.d>\n- Task [x](https://e.f)\n",
        60,
        6,
    );
    assert_eq!(
        outline_rows(&mut app)[..3],
        [
            "Plan",
            "  See the guide or https://c.d",
            "- Task [x](https://e.f)"
        ]
    );
    let buffer = app.render().buffer().clone();
    let heading = &buffer[(2, 2)];
    assert!(
        heading.modifier.contains(Modifier::BOLD),
        "a heading is bold"
    );
    assert_eq!(
        heading.fg,
        Color::LightMagenta,
        "level 2 in the headers color"
    );
    let title = &buffer[(8, 3)];
    assert!(
        title.modifier.contains(Modifier::UNDERLINED),
        "a link's title"
    );
    assert_eq!(title.fg, Color::Cyan, "in the link color");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(outline_rows(&mut app)[0], "## Plan");
    app.press(KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(
        outline_rows(&mut app)[..2],
        ["Plan", "  See [the guide](https://a.b) or <https://c.d>"]
    );
}

// FR-FMT-03, FR-NAV-03
#[test]
fn a_zoomed_headings_breadcrumb_hides_its_hashes() {
    let mut app = Harness::with_text("# Plan\n\t- a\n", 40, 5);
    app.type_keys(".");
    let buffer = app.render().buffer().clone();
    assert_eq!(line(&buffer, 0), "  ⌂ › Plan");
}
