//! Step 4.2: `Ctrl+P`, `gs` and `/` open Go to Symbol, a fuzzy finder over every block,
//! bookmarks first, then projects and sections; choosing a block zooms to its parent and focuses
//! it.

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::Modifier;

use crate::support::{Harness, focused_rows, line};

const TEXT: &str = "Home:\n\t- Paint the hall\n\tNotes @bookmark\nWork:\n\t- Write report\n\tLater @section\n\t\t- Call Ann\n";

/// Every screen row, without trailing blanks.
fn screen(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    (buffer.area.top()..buffer.area.bottom())
        .map(|y| line(&buffer, y))
        .collect()
}

fn press(app: &mut Harness, code: KeyCode) {
    app.press(code, KeyModifiers::NONE);
}

// FR-NAV-05, navigation.md §8, ui.md §1
#[test]
fn ctrl_p_lists_every_block_with_its_path_bookmarks_projects_and_sections_first() {
    let mut app = Harness::with_text(TEXT, 60, 16);
    app.press(KeyCode::Char('p'), KeyModifiers::CONTROL);
    assert_eq!(
        screen(&mut app)[4..14],
        [
            "┌Go to Symbol──────────────────────────────────────────────┐",
            "│›                                                         │",
            "│ Notes  Home                                              │",
            "│ Home                                                     │",
            "│ Work                                                     │",
            "│ Later  Work                                              │",
            "│ Paint the hall  Home                                     │",
            "│ Write report  Work                                       │",
            "│ Call Ann  Work › Later                                   │",
            "└──────────────────────────────────────────────────────────┘",
        ]
    );
    let buffer = app.render().buffer().clone();
    assert!(
        buffer[(2, 6)].modifier.contains(Modifier::REVERSED),
        "the first entry is highlighted"
    );
    assert!(
        buffer[(9, 6)].modifier.contains(Modifier::DIM),
        "its path is dim"
    );
    app.terminal.backend_mut().assert_cursor_position((3, 5));
}

// FR-NAV-05, navigation.md §8
#[test]
fn typing_filters_and_enter_zooms_to_the_parent_and_focuses_the_block() {
    let mut app = Harness::with_text(TEXT, 60, 16);
    app.press(KeyCode::Char('p'), KeyModifiers::CONTROL);
    app.type_keys("rep");
    assert_eq!(
        screen(&mut app)[7..10],
        [
            "┌Go to Symbol──────────────────────────────────────────────┐",
            "│› rep                                                     │",
            "│ Write report  Work                                       │",
        ]
    );
    app.terminal.backend_mut().assert_cursor_position((6, 8));
    press(&mut app, KeyCode::Enter);
    let rows = screen(&mut app);
    assert_eq!(rows[0], "  ⌂ › Work");
    assert_eq!(focused_rows(&mut app), ["  - Write report"]);
}

// keybindings.md §2 and §4
#[test]
fn g_s_and_slash_open_it_arrows_move_and_esc_closes_it() {
    for keys in ["gs", "/"] {
        let mut app = Harness::with_text(TEXT, 60, 16);
        app.type_keys(keys);
        assert!(screen(&mut app)[4].contains("Go to Symbol"), "{keys}");
        press(&mut app, KeyCode::Down);
        app.press(KeyCode::Char('n'), KeyModifiers::CONTROL);
        press(&mut app, KeyCode::Up);
        press(&mut app, KeyCode::Down);
        app.press(KeyCode::Char('p'), KeyModifiers::CONTROL);
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);
        assert_eq!(screen(&mut app)[0], "  ⌂", "{keys}: Work is at home");
        assert_eq!(focused_rows(&mut app), ["Work:"], "{keys}");
        app.type_keys(keys);
        app.type_keys("x");
        press(&mut app, KeyCode::Backspace);
        press(&mut app, KeyCode::Esc);
        let rows = screen(&mut app);
        assert!(
            !rows.iter().any(|row| row.contains("Go to Symbol")),
            "{rows:#?}"
        );
        assert_eq!(
            focused_rows(&mut app),
            ["Work:"],
            "{keys}: Esc goes nowhere"
        );
    }
}

// navigation.md §8
#[test]
fn a_query_that_matches_nothing_says_so() {
    let mut app = Harness::with_text(TEXT, 60, 16);
    app.press(KeyCode::Char('p'), KeyModifiers::CONTROL);
    app.type_keys("qqq");
    assert_eq!(
        screen(&mut app)[7..11],
        [
            "┌Go to Symbol──────────────────────────────────────────────┐",
            "│› qqq                                                     │",
            "│ No matches                                               │",
            "└──────────────────────────────────────────────────────────┘",
        ]
    );
}

// ui.md §1
#[test]
fn a_long_list_scrolls_with_the_highlight_in_a_box_at_most_80_by_20() {
    let text = (1..=30).fold(String::new(), |text, n| text + &format!("- Task {n}\n"));
    let mut app = Harness::with_text(&text, 100, 30);
    app.press(KeyCode::Char('p'), KeyModifiers::CONTROL);
    let rows = screen(&mut app);
    let top = rows
        .iter()
        .position(|row| row.contains("Go to Symbol"))
        .unwrap();
    assert_eq!(rows[top].chars().count(), 10 + 80);
    assert!(rows[top + 19].contains('└'), "{rows:#?}");
    for _ in 0..20 {
        press(&mut app, KeyCode::Down);
    }
    let rows = screen(&mut app);
    assert!(rows[top + 2].contains("│ Task 7 "), "{rows:#?}");
    assert!(rows[top + 18].contains("│ Task 23 "), "{rows:#?}");
}
