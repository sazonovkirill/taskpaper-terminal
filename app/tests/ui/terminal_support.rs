//! Step 7.4: wide characters at their display width, and small terminals.

use std::{fs, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::buffer::CellWidth as _;

use crate::support::{DUE, Harness, NAVIGATE, STAR, START};

const NONE: KeyModifiers = KeyModifiers::NONE;

/// An outline of CJK, Hangul, emoji (one of them a ZWJ sequence) and combining marks.
const WIDE: &str = "日本語の仕事:
\t- 買い物 👨‍👩‍👧 family @star
\t- Cafe\u{301} e\u{301}clair @start(2026-10-06)
\tメモ 🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉 終わり
\t- 한국어 텍스트가 아주 길어서 줄바꿈이 필요합니다 @due(2026-10-07)
";

/// Row `y` as the terminal shows it, without trailing blanks: each cell's symbol, a wide one
/// hiding the cells it covers.
fn shown(app: &mut Harness, y: u16) -> String {
    // A full redraw, so the test terminal holds this frame alone, as a terminal would.
    app.terminal.clear().expect("clearing a test terminal");
    let buffer = app.render().buffer();
    let mut row = String::new();
    let mut x = buffer.area.left();
    while x < buffer.area.right() {
        let cell = &buffer[(x, y)];
        row.push_str(cell.symbol());
        x += cell.cell_width().max(1);
    }
    row.trim_end().to_owned()
}

/// Every row of the screen as [`shown`] reads it.
fn screen(app: &mut Harness) -> Vec<String> {
    let height = app.terminal.backend().buffer().area.height;
    (0..height).map(|y| shown(app, y)).collect()
}

// N-PORT-04, FR-UI-05
#[test]
fn wide_characters_take_their_display_width_and_wrap_without_splitting_a_grapheme() {
    let mut app = Harness::with_text(WIDE, 40, 12);
    assert_eq!(
        screen(&mut app)[2..10],
        [
            "▌ 日本語の仕事:",
            format!("    - 買い物 👨‍👩‍👧 family {STAR}").as_str(),
            format!("    - Cafe\u{301} e\u{301}clair             {START} tomorrow").as_str(),
            "    メモ 🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉 終わり",
            format!("    - 한국어 텍스트가 아주 길어서  {DUE} Wed").as_str(),
            "      줄바꿈이 필요합니다",
            "",
            "",
        ]
    );
    app.press(KeyCode::Char('z'), KeyModifiers::ALT);
    assert_eq!(
        screen(&mut app)[5..7],
        [
            "    メモ 🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉 終わり",
            format!("    - 한국어 텍스트가 아주 길어서… {DUE} Wed").as_str(),
        ],
        "with wrap off, text that does not fit ends in … before a whole grapheme"
    );
}

// N-PORT-04, FR-MODE-03: the caret moves a grapheme at a time, as wide as it is drawn.
#[test]
fn the_caret_moves_over_wide_text_by_grapheme_and_its_width() {
    let mut app = Harness::with_text("- 日本 👨‍👩‍👧 e\u{301}\n", 40, 6);
    app.press(KeyCode::Enter, NONE);
    let caret = |app: &mut Harness| {
        app.render();
        app.terminal
            .get_cursor_position()
            .expect("the test terminal has a cursor")
            .x
    };
    let mut columns = vec![caret(&mut app)];
    for _ in 0..6 {
        app.press(KeyCode::Left, NONE);
        columns.push(caret(&mut app));
    }
    assert_eq!(
        columns,
        [13, 12, 11, 9, 8, 6, 4],
        "é, space, family, space, 本, 日"
    );
    app.press(KeyCode::Right, NONE);
    app.press(KeyCode::Delete, NONE);
    app.press(KeyCode::End, NONE);
    app.press(KeyCode::Backspace, NONE);
    assert_eq!(shown(&mut app, 2), "▌ - 日 👨‍👩‍👧");
    app.press(KeyCode::Esc, NONE);
    app.wait(Duration::from_secs(1));
    app.settle();
    let path = app.path.as_ref().expect("a file was opened");
    assert_eq!(
        fs::read_to_string(path).expect("the file is readable"),
        "- 日 👨‍👩‍👧 \n"
    );
}

// N-PORT-04: a popup drawn over half of a wide character hides all of it, so its edge shows.
#[test]
fn a_popup_over_wide_characters_hides_those_it_cuts() {
    let mut app = Harness::with_text(&"日本語日本語日本語日本語日本語日本語\n".repeat(10), 40, 14);
    app.type_keys("mc");
    let rows = screen(&mut app);
    assert_eq!(
        rows[3..5],
        [
            "  日本語日本 ┌Color:──────┐ 本語日本語",
            "  日本語日本 │ ● 1 Red    │ 本語日本語"
        ]
    );
}

// N-PORT-04, FR-UI-02: a pane the screen's edge cuts shows no wide grapheme it cuts.
#[test]
fn a_pane_cut_by_the_screen_edge_hides_the_wide_grapheme_it_cuts() {
    let mut app = Harness::with_text("a日本語日本語日本語日本語\n", 70, 10);
    for keys in ["s", "s", "h", "h"] {
        app.press(KeyCode::Char('w'), KeyModifiers::CONTROL);
        app.type_keys(keys);
    }
    assert!(
        shown(&mut app, 2).ends_with("│  a日本語"),
        "the third pane's fourth grapheme would end past the screen: {:?}",
        shown(&mut app, 2)
    );
}

/// The screen sizes the small-terminal tests draw at: 80×24 and smaller, down to nothing.
const SIZES: [(u16, u16); 12] = [
    (80, 24),
    (79, 23),
    (60, 12),
    (40, 10),
    (39, 10),
    (40, 9),
    (30, 6),
    (20, 4),
    (10, 3),
    (5, 2),
    (1, 1),
    (0, 0),
];

// N-PORT-02, ui.md §1: every screen down to 0×0 draws without failing, the status line at the
// bottom and the focused block above it while there is a row for each.
#[test]
fn every_size_down_to_nothing_shows_the_status_line_and_the_focused_block() {
    for (width, height) in SIZES {
        let mut app = Harness::with_text(WIDE, width, height);
        let rows = screen(&mut app);
        let mode: String = NAVIGATE.chars().take(usize::from(width)).collect();
        if height > 0 {
            assert!(
                rows[usize::from(height) - 1].starts_with(&mode),
                "{width}×{height}: {rows:#?}"
            );
        }
        if height > 3 {
            assert!(
                rows[2].contains(
                    &"▌ 日本語の仕事:"
                        .chars()
                        .take(usize::from(width))
                        .collect::<String>()
                ),
                "{width}×{height}: {rows:#?}"
            );
        }
    }
}

// N-PORT-02: every overlay opens on every size without failing, and keys go on working.
#[test]
fn every_overlay_opens_on_every_size() {
    let overlays = [
        "?", "md", "gs", "gP", ":", "mc", "my", "mi", "\\", "g,", "\n/",
    ];
    for (width, height) in SIZES {
        for keys in overlays {
            let mut app = Harness::with_text(WIDE, width, height);
            for key in keys.chars() {
                match key {
                    '\n' => app.press(KeyCode::Enter, NONE),
                    key => app.type_keys(&key.to_string()),
                }
                app.render();
            }
            app.press(KeyCode::Esc, NONE);
            app.press(KeyCode::Esc, NONE);
            app.press(KeyCode::Char('q'), KeyModifiers::CONTROL);
            app.settle();
            assert!(app.tui.should_quit(), "{width}×{height} {keys:?}");
        }
    }
}
