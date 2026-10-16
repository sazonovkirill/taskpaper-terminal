//! Step 3.4: `@star` with its marker chip, color tags that tint the row's background with `c` and
//! the `mc` picker and go with `C` (D-48, D-138), bookmarks with `mb`, and flags with `mf` and `!`
//! (D-91).

use std::{fs, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{buffer::Buffer, style::Color};

use crate::support::{
    BAND_TEXT, BOOKMARK, CURRENT_LINE, DUE, FLAG, Harness, STAR, line, outline_line, outline_rows,
};

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

/// The background a `@red` block's row gets, and the one when it is focused (D-48): dark reds of
/// the 256 colors in Terminal Dark (D-119).
const RED_TINT: Color = Color::Indexed(52);
const FOCUSED_RED_TINT: Color = Color::Indexed(88);

/// The background of row `y`'s cells from column `from` to the right edge, if it is the same
/// throughout.
fn background(buffer: &Buffer, y: u16, from: u16) -> Option<Color> {
    let first = buffer[(from, y)].bg;
    (from..buffer.area.width)
        .all(|x| buffer[(x, y)].bg == first)
        .then_some(first)
}

/// Every screen row, without trailing blanks.
fn screen(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    (buffer.area.top()..buffer.area.bottom())
        .map(|y| line(&buffer, y))
        .collect()
}

// FR-TAG-05, FR-TAG-06, D-48, D-91, D-138, FR-OPS-08
#[test]
fn c_tints_the_focused_row_at_once_and_cycles_and_shift_c_removes_the_color() {
    let mut app = Harness::with_text("- Milk\n- Bread\n", 30, 5);
    app.type_keys("jc");
    let status = line(app.render().buffer(), 4);
    assert!(!status.ends_with(" c"), "no prefix is pending: {status}");
    let buffer = app.render().buffer().clone();
    assert_eq!(outline_line(&buffer, 3), "- Bread");
    assert_eq!(background(&buffer, 3, 2), Some(FOCUSED_RED_TINT));
    assert_eq!(
        buffer[(4, 3)].fg,
        BAND_TEXT,
        "text in no color of its own is light on the tint (D-125)"
    );
    app.type_keys("k");
    let buffer = app.render().buffer().clone();
    assert_eq!(background(&buffer, 3, 2), Some(RED_TINT));
    assert_eq!(background(&buffer, 2, 2), Some(CURRENT_LINE));
    assert_eq!(saved(&mut app), "- Milk\n- Bread @red\n");
    app.type_keys("jc");
    assert_eq!(saved(&mut app), "- Milk\n- Bread @yellow\n");
    app.type_keys("cc");
    assert_eq!(saved(&mut app), "- Milk\n- Bread @blue\n");
    app.press(KeyCode::Char('c'), KeyModifiers::SHIFT);
    assert_eq!(saved(&mut app), "- Milk\n- Bread\n");
    let buffer = app.render().buffer().clone();
    assert_eq!(background(&buffer, 3, 0), Some(CURRENT_LINE));
    app.type_keys("z");
    assert_eq!(saved(&mut app), "- Milk\n- Bread @blue\n");
}

// FR-TAG-05, D-138: `@orange` and `@violet` are kept as written but are no colors.
#[test]
fn orange_and_violet_tags_tint_nothing_and_stay_when_c_colors_the_block() {
    let mut app = Harness::with_text("- Milk @orange\n- Bread @violet\n", 30, 5);
    app.type_keys("j");
    let buffer = app.render().buffer().clone();
    assert_eq!(background(&buffer, 2, 2), Some(Color::Reset));
    assert_eq!(background(&buffer, 3, 2), Some(CURRENT_LINE));
    app.type_keys("kc");
    assert_eq!(saved(&mut app), "- Milk @orange @red\n- Bread @violet\n");
    app.type_keys("C");
    assert_eq!(saved(&mut app), "- Milk @orange\n- Bread @violet\n");
}

// FR-TAG-05, FR-OPS-07, D-48, D-138
#[test]
fn c_and_shift_c_color_every_selected_block_in_one_step() {
    let mut app = Harness::with_text("- Milk @blue\n- Bread @red\n- Eggs\n", 30, 6);
    app.type_keys("Jc");
    assert_eq!(
        saved(&mut app),
        "- Milk @yellow\n- Bread @yellow\n- Eggs\n",
        "after the focused block's red"
    );
    app.type_keys("C");
    assert_eq!(saved(&mut app), "- Milk\n- Bread\n- Eggs\n");
    app.type_keys("z");
    assert_eq!(saved(&mut app), "- Milk @yellow\n- Bread @yellow\n- Eggs\n");
    app.type_keys("z");
    assert_eq!(saved(&mut app), "- Milk @blue\n- Bread @red\n- Eggs\n");
}

// FR-TAG-05, D-48, keybindings.md §4, ui.md §1
#[test]
fn m_c_opens_the_color_picker_and_enter_gives_the_highlighted_color() {
    let mut app = Harness::with_text("- Milk @yellow @x\n", 30, 14);
    app.type_keys("mc");
    assert_eq!(
        screen(&mut app)[2..11],
        [
            "▌ - Milk",
            "",
            "        ┌Color:──────┐",
            "        │ ● 1 Red    │",
            "        │ ● 2 Yellow │",
            "        │ ● 3 Green  │",
            "        │ ● 4 Blue   │",
            "        │ ●   None   │",
            "        └────────────┘",
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
    app.type_keys("mc4");
    assert!(!screen(&mut app).iter().any(|row| row.contains("Color:")));
    assert_eq!(
        saved(&mut app),
        "- Milk @x @blue\n",
        "a number chooses at once"
    );
}

// FR-TAG-05, keybindings.md §4
#[test]
fn typing_in_the_picker_filters_it_and_esc_closes_it_unchanged() {
    let mut app = Harness::with_text("- Milk @blue\n", 30, 14);
    app.type_keys("mcno");
    let rows = screen(&mut app);
    assert!(
        rows.contains(&"         │ ●   None │".to_owned()),
        "{rows:#?}"
    );
    assert!(
        rows.contains(&"         └no────────┘".to_owned()),
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

// FR-TAG-04, D-47, D-88, D-130, ui.md §2
#[test]
fn star_key_stars_the_focused_block_with_a_star_after_its_text() {
    let mut app = Harness::with_text("- Milk @due(2026-10-09)\n", 40, 4);
    app.type_keys("s");
    assert_eq!(
        outline_rows(&mut app)[0],
        format!("- Milk {STAR}{:>30}", format!("{DUE} Fri"))
    );
    let buffer = app.render().buffer().clone();
    assert_eq!(buffer[(9, 2)].fg, Color::Yellow);
    assert_eq!(saved(&mut app), "- Milk @due(2026-10-09) @star\n");
    app.type_keys("s");
    assert_eq!(
        outline_rows(&mut app)[0],
        format!("{:<33}{DUE} Fri", "- Milk")
    );
    assert_eq!(saved(&mut app), "- Milk @due(2026-10-09)\n");
}

// D-11, D-47
#[test]
fn m_b_bookmarks_the_focused_block_and_says_so() {
    let mut app = Harness::with_text("Plans:\n", 80, 4);
    app.type_keys("mb");
    let status = line(app.render().buffer(), 3);
    assert!(status.ends_with("· Bookmarked"), "{status}");
    assert_eq!(
        outline_rows(&mut app)[0],
        format!("Plans: {BOOKMARK}"),
        "the bookmark icon follows the text (D-47)"
    );
    assert_eq!(saved(&mut app), "Plans: @bookmark\n");
    app.type_keys("mb");
    let status = line(app.render().buffer(), 3);
    assert!(status.ends_with("· Bookmark removed"), "{status}");
    assert_eq!(saved(&mut app), "Plans:\n");
    assert_eq!(outline_rows(&mut app)[0], "Plans:");
}

// FR-DOC-05, FR-TAG-04, FR-TAG-05, D-11, D-47, D-91
#[test]
fn the_demo_shows_today_and_colors() {
    let mut app = Harness::demo(70, 40);
    let buffer = app.render().buffer().clone();
    let row = |text: &str| {
        (2..39)
            .find(|&y| outline_line(&buffer, y).starts_with(text))
            .unwrap_or_else(|| panic!("the demo has {text:?}"))
    };
    assert_eq!(
        outline_line(&buffer, row("    - Phone charger")),
        format!("    - Phone charger {STAR}")
    );
    assert_eq!(
        outline_line(&buffer, row("Trip to Lisbon:")),
        format!("Trip to Lisbon: {BOOKMARK}")
    );
    assert_eq!(
        background(&buffer, row("  - Fix the broken contact form"), 4),
        Some(RED_TINT)
    );
}

// FR-TAG-05, D-91, ui.md §2
#[test]
fn m_f_and_bang_flag_the_focused_block_with_a_red_flag_after_its_text() {
    let mut app = Harness::with_text("- Milk @blue\n", 30, 4);
    app.type_keys("mf");
    let buffer = app.render().buffer().clone();
    assert_eq!(outline_line(&buffer, 2), format!("- Milk {FLAG}"));
    assert_eq!(buffer[(9, 2)].fg, Color::Red, "a red flag on a blue row");
    assert_eq!(saved(&mut app), "- Milk @blue @flag\n");
    app.type_keys("!");
    assert_eq!(outline_rows(&mut app)[0], "- Milk");
    assert_eq!(saved(&mut app), "- Milk @blue\n");
    app.type_keys("!");
    assert_eq!(saved(&mut app), "- Milk @blue @flag\n");
}
