//! D-140: the Keymap command, `g?` or `F1`, opens a dialog over 90% of the screen that lists every
//! binding, grouped as a cheat sheet, until `Esc`, `q`, `F1` or `g?` closes it.

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use taskpaper_terminal::ui::widgets::KeymapDialog;

use crate::support::{Harness, line, outline_rows};

const OUTLINE: &str = "Home:\n\t- Milk\n\t- Eggs\n";

/// Every screen row, without trailing blanks.
fn screen(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    (buffer.area.top()..buffer.area.bottom())
        .map(|y| line(&buffer, y))
        .collect()
}

/// The text of each row of `area` of the screen.
fn cut(app: &mut Harness, area: Rect) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    area.rows()
        .map(|row| row.columns().map(|cell| buffer[cell].symbol()).collect())
        .collect()
}

/// Whether a line of `sheet` gives `name` the keys `keys`, beside it or on the line below.
fn lists(sheet: &str, name: &str, keys: &str) -> bool {
    let lines: Vec<&str> = sheet.lines().collect();
    lines.iter().enumerate().any(|(at, line)| {
        line.match_indices(&format!("  {name} ")).any(|(start, _)| {
            let after = line[start + name.len() + 2..].trim_start();
            after.starts_with(&format!("{keys} "))
                || lines.get(at + 1).is_some_and(|next| {
                    next.get(start..)
                        .is_some_and(|below| below.trim_start().starts_with(keys))
                })
        })
    })
}

fn press(app: &mut Harness, code: KeyCode) {
    app.press(code, KeyModifiers::NONE);
}

/// Where the dialog is on a 200×60 screen: 180×54 cells from (10, 3).
const WIDE: Rect = Rect::new(10, 3, 180, 54);

// FR-KEY-07, D-140
#[test]
fn the_dialog_is_90_percent_of_the_screen_centered() {
    assert_eq!(KeymapDialog::area(Rect::new(0, 0, 200, 60)), WIDE);
    assert_eq!(
        KeymapDialog::area(Rect::new(0, 0, 80, 24)),
        Rect::new(4, 1, 72, 22)
    );
}

// FR-KEY-07, D-140, ui.md §11
#[test]
fn g_question_mark_opens_the_keymap_over_every_pane_with_all_bindings_in_columns() {
    let mut app = Harness::with_text(OUTLINE, 260, 80);
    app.press(KeyCode::Char('\\'), KeyModifiers::CONTROL);
    app.type_keys("g?");
    let dialog = cut(&mut app, KeymapDialog::area(Rect::new(0, 0, 260, 80)));
    assert!(dialog[0].starts_with("┌ Keymap ───"), "{}", dialog[0]);
    assert!(dialog[0].ends_with("──┐"));
    let last = dialog.last().unwrap();
    assert!(last.ends_with("─ Esc: close ┘"), "it all fits: {last}");
    assert_eq!(
        dialog[1..4]
            .iter()
            .map(|row| &row[..row.char_indices().nth(46).unwrap().0])
            .collect::<Vec<_>>(),
        [
            "│ Moving                                      ",
            "│  Next Block                            j, ↓ ",
            "│  Previous Block                        k, ↑ ",
        ]
    );
    let sheet = dialog.join("\n");
    for heading in [
        "Selection",
        "Block Metadata",
        "View and Panels",
        "Edit Mode",
        "Navigator",
        "Pickers and Palettes",
        "Date Prompt",
        "Quick Add",
        "Edit Node",
        "Attachments Panel",
        "Settings Window",
        "Save As",
        "Conflict Prompt",
    ] {
        assert!(
            sheet.lines().any(|line| line
                .split("  ")
                .any(|cell| cell.trim_matches(['│', ' ']) == heading)),
            "{heading}"
        );
    }
    for (name, keys) in [
        ("Keymap", "g?, F1"),
        ("Toggle Section", "S"),
        ("Focus Pane 9", "Alt+9, g9"),
        ("Stop Editing and Add Next Block", "Enter"),
        ("Add Comment", "a"),
        ("Reset", "r"),
        ("Complete Folder", "Tab"),
        ("Keep Mine", "Ctrl+K"),
        ("Close", "Esc, q, F1, g?"),
        ("Quit, in Any Window", "Ctrl+Q"),
    ] {
        assert!(lists(&sheet, name, keys), "{name}: {keys}");
    }
    assert!(
        !sheet.contains("Start Weather Animation"),
        "a command without keys is in Go to Command alone"
    );
    assert_eq!(
        sheet.matches("Ctrl+Q").count(),
        3,
        "Quit in navigate mode, in edit mode, and once for every window"
    );
    let row = &screen(&mut app)[4];
    assert!(
        row.starts_with("    - Eggs   ┌ Keymap ─"),
        "over the panes: {row}"
    );
}

// FR-KEY-07, D-140
#[test]
fn the_keymap_scrolls_with_the_keys_of_the_outline_and_closes_with_esc_q_f1_or_g_question_mark() {
    let mut app = Harness::with_text(OUTLINE, 80, 24);
    let before = outline_rows(&mut app);
    let area = Rect::new(4, 1, 72, 22);
    press(&mut app, KeyCode::F(1));
    let top = cut(&mut app, area);
    assert_eq!(
        top[1],
        "│ Moving                                                               │"
    );
    assert!(
        top[21].ends_with("─ j/k: scroll · Esc: close ┘"),
        "{}",
        top[21]
    );
    app.type_keys("j");
    assert_eq!(cut(&mut app, area)[1], top[2], "a line down");
    app.type_keys("k");
    assert_eq!(cut(&mut app, area), top);
    app.press(KeyCode::Char('d'), KeyModifiers::CONTROL);
    let page = cut(&mut app, area);
    assert!(
        page[1].contains("Expand All"),
        "a page of the 20 lines shown down: {}",
        page[1]
    );
    app.press(KeyCode::Char('u'), KeyModifiers::CONTROL);
    assert_eq!(cut(&mut app, area), top, "and up");
    app.type_keys("G");
    let bottom = cut(&mut app, area);
    assert!(bottom[20].contains("Quit"), "the last line: {}", bottom[20]);
    app.type_keys("j");
    assert_eq!(cut(&mut app, area), bottom, "no further than the last line");
    app.type_keys("gg");
    assert_eq!(cut(&mut app, area), top);
    app.type_keys("xdd");
    assert_eq!(cut(&mut app, area), top, "other keys do nothing");
    for close in ["esc", "q", "f1", "g?"] {
        match close {
            "esc" => press(&mut app, KeyCode::Esc),
            "f1" => press(&mut app, KeyCode::F(1)),
            keys => app.type_keys(keys),
        }
        assert_eq!(outline_rows(&mut app), before, "{close} closes it");
        assert!(!screen(&mut app)[1].contains("Keymap"), "{close}");
        app.type_keys("jg?");
        assert_eq!(cut(&mut app, area), top, "it opens at the top again");
    }
}

// FR-KEY-07, D-140, FR-SET-01
#[test]
fn the_keymap_shows_the_config_files_bindings_and_opens_from_go_to_command() {
    let settings = "[keymap.navigate]\n\"ctrl+d\" = \"toggle_done\"\nx = \"none\"\n";
    let mut app = Harness::with_config(OUTLINE, settings, 200, 60);
    app.press(KeyCode::Char('p'), KeyModifiers::CONTROL);
    app.type_keys("keymap");
    press(&mut app, KeyCode::Enter);
    let sheet = cut(&mut app, WIDE).join("\n");
    assert!(
        sheet.contains("Toggle Done           Ctrl+D, Ctrl+Enter"),
        "{sheet}"
    );
    let page_down = sheet
        .lines()
        .find(|row| row.starts_with("│  Page Down"))
        .unwrap();
    assert!(
        page_down.starts_with("│  Page Down                           PgDn "),
        "Ctrl+D pages no more: {page_down}"
    );
}
