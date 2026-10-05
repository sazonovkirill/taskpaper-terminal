//! Step 3.4: `@today` with its marker chip, color tags that tint the text with `c` and the `mc`
//! picker, and bookmarks with `mb`.

use std::{fs, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{buffer::Buffer, style::Color};

use crate::support::{Harness, line, outline_line, outline_rows};

/// The open file's text once the autosave delay has passed and its save is done.
fn saved(app: &mut Harness) -> String {
    app.wait(Duration::from_secs(1));
    app.settle();
    let path = app.path.as_ref().expect("a file was opened");
    fs::read_to_string(path).expect("the file is readable")
}

fn press(app: &mut Harness, code: KeyCode) {
    app.press(code, KeyModifiers::NONE);
}

/// The text color of the first character of outline row `y` after the gutter and `indent`.
fn text_color(buffer: &Buffer, y: u16, indent: u16) -> Color {
    buffer[(2 + indent, y)].fg
}

/// Every screen row, without trailing blanks.
fn screen(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    (buffer.area.top()..buffer.area.bottom())
        .map(|y| line(&buffer, y))
        .collect()
}

// FR-TAG-05, FR-TAG-06, D-26, FR-OPS-08
#[test]
fn c_colors_the_focused_task_red_then_orange_and_z_undoes_it() {
    let mut app = Harness::with_text("- Milk\n- Bread\n", 30, 5);
    app.type_keys("jc");
    let buffer = app.render().buffer().clone();
    assert_eq!(outline_line(&buffer, 3), "- Bread");
    assert_eq!(text_color(&buffer, 3, 2), Color::Red);
    assert_eq!(text_color(&buffer, 2, 2), Color::Reset);
    assert_eq!(saved(&mut app), "- Milk\n- Bread @red\n");
    app.type_keys("c");
    assert_eq!(saved(&mut app), "- Milk\n- Bread @orange\n");
    app.type_keys("z");
    assert_eq!(saved(&mut app), "- Milk\n- Bread @red\n");
}

// FR-TAG-05, D-5, keybindings.md §4, ui.md §1
#[test]
fn m_c_opens_the_color_picker_and_enter_gives_the_highlighted_color() {
    let mut app = Harness::with_text("- Milk @yellow @x\n", 30, 14);
    app.type_keys("mc");
    assert_eq!(
        screen(&mut app)[2..12],
        [
            "▎ - Milk",
            "          ┌Color:──┐",
            "          │ Red    │",
            "          │ Orange │",
            "          │ Yellow │",
            "          │ Green  │",
            "          │ Blue   │",
            "          │ Violet │",
            "          │ None   │",
            "          └────────┘",
        ]
    );
    let buffer = app.render().buffer().clone();
    assert!(
        buffer[(11, 6)]
            .modifier
            .contains(ratatui::style::Modifier::REVERSED),
        "the block's color is highlighted"
    );
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    assert!(!screen(&mut app).iter().any(|row| row.contains("Color:")));
    assert_eq!(saved(&mut app), "- Milk @x @green\n");
}

// FR-TAG-05, keybindings.md §4
#[test]
fn typing_in_the_picker_filters_it_and_esc_closes_it_unchanged() {
    let mut app = Harness::with_text("- Milk @blue\n", 30, 14);
    app.type_keys("mcno");
    let rows = screen(&mut app);
    assert!(
        rows.contains(&"           │ None │".to_owned()),
        "{rows:#?}"
    );
    assert!(
        rows.contains(&"           └no────┘".to_owned()),
        "{rows:#?}"
    );
    assert!(!rows.iter().any(|row| row.contains("Red")));
    press(&mut app, KeyCode::Esc);
    assert!(!screen(&mut app).iter().any(|row| row.contains("Color:")));
    app.type_keys("j");
    assert_eq!(saved(&mut app), "- Milk @blue\n");
    app.type_keys("mcnone");
    press(&mut app, KeyCode::Enter);
    assert_eq!(saved(&mut app), "- Milk\n");
}

// FR-TAG-04, ui.md §2
#[test]
fn m_t_marks_the_focused_block_for_today_with_a_star_chip() {
    let mut app = Harness::with_text("- Milk @due(2026-10-09)\n", 40, 4);
    app.type_keys("mt");
    assert_eq!(
        outline_rows(&mut app)[0],
        format!("{:<23}★ due Fri 9 Oct", "- Milk")
    );
    let buffer = app.render().buffer().clone();
    assert_eq!(buffer[(25, 2)].fg, Color::Yellow);
    assert_eq!(saved(&mut app), "- Milk @due(2026-10-09) @today\n");
    app.type_keys("mt");
    assert_eq!(
        outline_rows(&mut app)[0],
        format!("{:<25}due Fri 9 Oct", "- Milk")
    );
    assert_eq!(saved(&mut app), "- Milk @due(2026-10-09)\n");
}

// D-11
#[test]
fn m_b_bookmarks_the_focused_block_and_says_so() {
    let mut app = Harness::with_text("Plans:\n", 80, 4);
    app.type_keys("mb");
    let status = line(app.render().buffer(), 3);
    assert!(status.ends_with("· Bookmarked"), "{status}");
    assert_eq!(
        outline_rows(&mut app)[0],
        "Plans:",
        "no mark in the outline"
    );
    assert_eq!(saved(&mut app), "Plans: @bookmark\n");
    app.type_keys("mb");
    let status = line(app.render().buffer(), 3);
    assert!(status.ends_with("· Bookmark removed"), "{status}");
    assert_eq!(saved(&mut app), "Plans:\n");
}

// FR-DOC-05, FR-TAG-04, FR-TAG-05, D-11
#[test]
fn the_demo_shows_today_and_colors() {
    let mut app = Harness::demo(70, 40);
    let buffer = app.render().buffer().clone();
    let row = |text: &str| {
        (2..39)
            .find(|&y| outline_line(&buffer, y).starts_with(text))
            .unwrap_or_else(|| panic!("the demo has {text:?}"))
    };
    assert!(outline_line(&buffer, row("    - Phone charger")).ends_with('★'));
    assert_eq!(
        text_color(&buffer, row("  - Fix the broken contact form"), 4),
        Color::Red
    );
}
