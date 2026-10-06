//! Step 1.2: the focused row, moving it, scrolling to keep it visible, the status line and
//! multi-key sequences.

use std::{fmt::Write as _, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
};

use crate::support::{BOOKMARK, CURRENT_LINE, Harness, focused_rows, line, outline_rows};

const NONE: KeyModifiers = KeyModifiers::NONE;

/// An outline of `n` tasks named `Task 01`, `Task 02`, …
fn tasks(n: usize) -> String {
    (1..=n).fold(String::new(), |mut text, i| {
        writeln!(text, "- Task {i:02}").expect("writing to a String cannot fail");
        text
    })
}

#[track_caller]
fn assert_focus(app: &mut Harness, expected: &str) {
    assert_eq!(focused_rows(app), [expected]);
}

/// The status line as drawn, without trailing blanks.
fn status_line(app: &mut Harness) -> String {
    let buffer = app.render().buffer().clone();
    line(&buffer, buffer.area.bottom() - 1)
}

/// The first row on screen.
fn top_row(app: &mut Harness) -> String {
    outline_rows(app).swap_remove(0)
}

// FR-NAV-01
#[test]
fn focus_starts_on_the_first_row_with_a_gutter_marker() {
    let mut app = Harness::with_text("A:\n\t- b\n", 12, 5);
    let mut expected = Buffer::with_lines([
        "  ⌂         ",
        "────────────",
        "▎ A:        ",
        "    - b     ",
        &app.status(12),
    ]);
    expected.set_style(Rect::new(2, 0, 10, 1), Style::new().bold());
    expected.set_style(Rect::new(0, 1, 12, 1), Style::new().dark_gray());
    expected.set_style(Rect::new(0, 2, 12, 1), Style::new().bg(CURRENT_LINE));
    expected.set_style(Rect::new(0, 2, 1, 1), Style::new().light_blue());
    expected.set_style(Rect::new(2, 2, 2, 1), Style::new().bold().light_blue());
    expected.set_style(Rect::new(0, 4, 8, 1), Style::new().bold());
    app.render().assert_buffer(&expected);
}

// FR-NAV-01
#[test]
fn j_k_and_arrows_move_focus_one_row_and_stop_at_the_ends() {
    let mut app = Harness::with_text(&tasks(3), 20, 12);
    app.type_keys("j");
    assert_focus(&mut app, "- Task 02");
    app.press(KeyCode::Down, NONE);
    assert_focus(&mut app, "- Task 03");
    app.type_keys("j");
    app.press(KeyCode::Down, NONE);
    assert_focus(&mut app, "- Task 03");
    app.type_keys("k");
    assert_focus(&mut app, "- Task 02");
    app.press(KeyCode::Up, NONE);
    assert_focus(&mut app, "- Task 01");
    app.type_keys("k");
    app.press(KeyCode::Up, NONE);
    assert_focus(&mut app, "- Task 01");
}

// FR-NAV-01
#[test]
fn g_g_home_shift_g_and_end_jump_to_the_first_and_last_rows() {
    let mut app = Harness::with_text(&tasks(30), 20, 8);
    app.press(KeyCode::Char('G'), KeyModifiers::SHIFT);
    assert_focus(&mut app, "- Task 30");
    assert_eq!(top_row(&mut app), "- Task 26");
    app.type_keys("gg");
    assert_focus(&mut app, "- Task 01");
    assert_eq!(top_row(&mut app), "- Task 01");
    app.press(KeyCode::End, NONE);
    assert_focus(&mut app, "- Task 30");
    app.press(KeyCode::Home, NONE);
    assert_focus(&mut app, "- Task 01");
}

// FR-NAV-01
#[test]
fn moving_down_scrolls_to_keep_two_rows_below_the_focus() {
    let mut app = Harness::with_text(&tasks(30), 20, 8);
    app.type_keys("jj");
    assert_eq!(top_row(&mut app), "- Task 01");
    app.type_keys("j");
    assert_focus(&mut app, "- Task 04");
    assert_eq!(
        outline_rows(&mut app),
        [
            "- Task 02",
            "- Task 03",
            "- Task 04",
            "- Task 05",
            "- Task 06"
        ]
    );
    app.type_keys("j");
    assert_eq!(top_row(&mut app), "- Task 03");
    app.type_keys("k");
    assert_eq!(top_row(&mut app), "- Task 02");
    app.type_keys("k");
    assert_focus(&mut app, "- Task 03");
    assert_eq!(top_row(&mut app), "- Task 01");
}

// FR-NAV-01
#[test]
fn page_keys_move_focus_and_view_by_the_outline_height() {
    let mut app = Harness::with_text(&tasks(30), 20, 8);
    app.render();
    app.press(KeyCode::PageDown, NONE);
    assert_focus(&mut app, "- Task 06");
    assert_eq!(
        outline_rows(&mut app),
        [
            "- Task 04",
            "- Task 05",
            "- Task 06",
            "- Task 07",
            "- Task 08"
        ]
    );
    app.press(KeyCode::Char('d'), KeyModifiers::CONTROL);
    assert_focus(&mut app, "- Task 11");
    assert_eq!(top_row(&mut app), "- Task 09");
    app.press(KeyCode::PageUp, NONE);
    assert_focus(&mut app, "- Task 06");
    assert_eq!(top_row(&mut app), "- Task 04");
    app.press(KeyCode::Char('u'), KeyModifiers::CONTROL);
    assert_focus(&mut app, "- Task 01");
    assert_eq!(top_row(&mut app), "- Task 01");
}

// FR-NAV-01
#[test]
fn paging_up_from_the_bottom_keeps_the_focused_row_in_place_on_screen() {
    let mut app = Harness::with_text(&tasks(30), 20, 13);
    app.press(KeyCode::Char('G'), NONE);
    assert_eq!(top_row(&mut app), "- Task 21");
    app.press(KeyCode::PageUp, NONE);
    assert_focus(&mut app, "- Task 20");
    assert_eq!(top_row(&mut app), "- Task 13");
}

// FR-NAV-01
#[test]
fn paging_stops_at_the_last_row_without_scrolling_past_it() {
    let mut app = Harness::with_text(&tasks(8), 20, 8);
    app.render();
    app.press(KeyCode::PageDown, NONE);
    app.press(KeyCode::PageDown, NONE);
    assert_focus(&mut app, "- Task 08");
    assert_eq!(
        outline_rows(&mut app),
        [
            "- Task 04",
            "- Task 05",
            "- Task 06",
            "- Task 07",
            "- Task 08"
        ]
    );
}

#[test]
fn the_status_line_shows_the_mode_and_the_path() {
    let mut app = Harness::with_text("- a\n", 200, 4);
    let path = app.path.clone().expect("a file was opened");
    assert_eq!(
        status_line(&mut app),
        format!("NAVIGATE · {}", path.display())
    );
}

#[test]
fn a_pending_prefix_shows_in_the_status_line_until_the_sequence_ends() {
    let mut app = Harness::demo(40, 4);
    app.type_keys("jg");
    let status: String = app.status(40).chars().take(38).collect();
    assert_eq!(status_line(&mut app), format!("{status} g"));
    app.type_keys("g");
    assert_eq!(status_line(&mut app), app.status(40).trim_end());
    assert_focus(&mut app, &format!("Trip to Lisbon: {BOOKMARK}"));
}

#[test]
fn a_pending_prefix_covers_the_end_of_a_long_path() {
    let mut app = Harness::with_text("- a\n", 16, 4);
    app.type_keys("g");
    let status: String = app.status(16).chars().take(14).collect();
    assert_eq!(status_line(&mut app), format!("{status} g"));
}

#[test]
fn a_sequence_times_out_after_one_second() {
    let mut app = Harness::with_text(&tasks(5), 20, 10);
    app.type_keys("jjj");
    assert_eq!(app.tui.deadline(), None);
    app.type_keys("g");
    assert_eq!(app.tui.deadline(), Some(Duration::from_secs(1)));
    app.wait(Duration::from_millis(999));
    app.type_keys("g");
    assert_eq!(app.tui.deadline(), None);
    assert_focus(&mut app, "- Task 01");

    app.type_keys("jjj");
    app.type_keys("g");
    app.wait(Duration::from_secs(1));
    assert_eq!(status_line(&mut app), app.status(20).trim_end());
    app.type_keys("g");
    assert_focus(&mut app, "- Task 04");
}

#[test]
fn an_empty_document_has_no_focus_and_ignores_motions() {
    let mut app = Harness::with_text("", 20, 5);
    for (code, modifiers) in [
        (KeyCode::Char('j'), NONE),
        (KeyCode::Char('k'), NONE),
        (KeyCode::Char('G'), KeyModifiers::SHIFT),
        (KeyCode::PageDown, NONE),
        (KeyCode::PageUp, NONE),
    ] {
        app.press(code, modifiers);
    }
    app.type_keys("gg");
    assert_eq!(focused_rows(&mut app), Vec::<String>::new());
    assert_eq!(outline_rows(&mut app), ["", ""]);
}

#[test]
fn a_one_row_terminal_shows_only_the_status_line() {
    let mut app = Harness::demo(12, 1);
    assert_eq!(status_line(&mut app), app.status(12));
    app.type_keys("j");
    app.press(KeyCode::PageDown, NONE);
    assert_eq!(status_line(&mut app), app.status(12));
    assert_eq!(focused_rows(&mut app), Vec::<String>::new());
}

#[test]
fn a_two_row_terminal_scrolls_one_row_at_a_time() {
    let mut app = Harness::with_text(&tasks(5), 12, 4);
    app.render();
    app.type_keys("j");
    assert_eq!(outline_rows(&mut app), ["- Task 02"]);
    assert_focus(&mut app, "- Task 02");
    app.press(KeyCode::PageDown, NONE);
    assert_focus(&mut app, "- Task 03");
    app.press(KeyCode::PageUp, NONE);
    assert_focus(&mut app, "- Task 02");
}

// D-30, D-39
#[test]
fn the_focused_block_gets_a_brighter_band_that_keeps_its_colors() {
    let mut app = Harness::with_text("- a\nfirst\nsecond\n- b\n", 16, 7);
    let unfocused = app.render().buffer().clone();
    app.type_keys("j");
    let navigate = app.render().buffer().clone();
    app.press(KeyCode::Enter, NONE);
    let edit = app.render().buffer().clone();
    for buffer in [&navigate, &edit] {
        assert_eq!([line(buffer, 3), line(buffer, 4)], ["▎ first", "  second"]);
        for y in 2..6 {
            let band = if y == 3 { CURRENT_LINE } else { Color::Reset };
            for x in 0..16 {
                let (cell, before) = (&buffer[(x, y)], &unfocused[(x, y)]);
                assert_eq!(cell.bg, band, "column {x}, row {y}");
                if x >= 2 {
                    assert_eq!(
                        (cell.fg, cell.modifier),
                        (before.fg, before.modifier),
                        "column {x}, row {y}"
                    );
                }
            }
        }
    }
}
