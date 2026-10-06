//! Step 4.8: `?` opens the shortcuts panel in the secondary sidebar on the right, listing the
//! most useful bindings of the current mode, grouped.

use crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Harness, line, selected_rows};

const OUTLINE: &str = "Home:\n\t- Milk\n\t- Eggs\n";

/// Every screen row, without trailing blanks.
fn screen(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    (buffer.area.top()..buffer.area.bottom())
        .map(|y| line(&buffer, y))
        .collect()
}

/// The text of each screen row from column `x`, without trailing blanks.
fn right_of(app: &mut Harness, x: u16) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    let area = buffer.area;
    (area.top()..area.bottom())
        .map(|y| {
            (x..area.right())
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect()
}

/// The panel at the right of a 80×30 screen, in navigate mode, before any `Alt` key.
const NAVIGATE: [&str; 29] = [
    "│Shortcuts",
    "│ Moving",
    "│  Next Block              j, ↓",
    "│  Previous Block          k, ↑",
    "│  Collapse                h, ←",
    "│  Expand                  l, →",
    "│  First Block         gg, Home",
    "│  Last Block            G, End",
    "│ Folding",
    "│  Toggle Fold         ', Space",
    "│  Expand All                 +",
    "│  Collapse All               _",
    "│ Zoom and History",
    "│  Zoom In  ., Alt+l, Alt+→, gz",
    "│  Zoom Out >, Alt+h, Alt+←, gu",
    "│  Back          H, Alt+Shift+←",
    "│  Forward       L, Alt+Shift+→",
    "│ Editing and Creating",
    "│  Edit                   Enter",
    "│  New Block Below            o",
    "│  New Block as Last Child    i",
    "│ Selection",
    "│  Extend Selection… J, Shift+↓",
    "│  Toggle Selected            v",
    "│ …",
    "│",
    "│ No Alt key seen yet: on macOS",
    "│ set Option as Meta or Esc+",
    "│ Kitty keyboard protocol: off",
];

// FR-KEY-04, D-28, ui.md §1, §4
#[test]
fn question_mark_opens_the_shortcuts_panel_at_the_right_and_closes_it_again() {
    let mut app = Harness::with_text(OUTLINE, 80, 30);
    let before = screen(&mut app);
    app.type_keys("?");
    assert_eq!(right_of(&mut app, 48)[..29], NAVIGATE);
    let rows = screen(&mut app);
    assert_eq!(
        rows[3],
        "    - Milk                                      │  Previous Block          k, ↑"
    );
    assert_eq!(rows[29], before[29], "the status line keeps the full width");
    app.type_keys("j");
    assert_eq!(
        right_of(&mut app, 48)[..29],
        NAVIGATE,
        "keys still navigate"
    );
    assert!(screen(&mut app)[3].starts_with("▌   - Milk "));
    app.type_keys("k?");
    assert_eq!(screen(&mut app), before);
}

// FR-KEY-04
#[test]
fn esc_closes_the_panel_before_clearing_the_selection() {
    let mut app = Harness::with_text(OUTLINE, 80, 30);
    app.type_keys("J?");
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    let rows = screen(&mut app);
    assert!(
        !rows.iter().any(|row| row.contains("Shortcuts")),
        "{rows:#?}"
    );
    assert_eq!(selected_rows(&mut app), ["Home:"], "the selection stays");
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(selected_rows(&mut app), Vec::<String>::new());
}

// FR-KEY-04: the panel follows the mode, and `?` is typed while editing.
#[test]
fn the_panel_lists_edit_modes_bindings_while_editing() {
    let mut app = Harness::with_text(OUTLINE, 80, 30);
    app.type_keys("j?");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    app.type_keys("?");
    assert_eq!(
        right_of(&mut app, 48)[..19],
        [
            "│Shortcuts",
            "│ Editing",
            "│  Stop Editing             Esc",
            "│  Stop Editing and Add … Enter",
            "│  Edit Previous Block        ↑",
            "│  Edit Next Block            ↓",
            "│  Command Menu               /",
            "│  Cycle Type            Ctrl+T",
            "│  Bold                  Ctrl+B",
            "│  Inline Code    Alt+`, Ctrl+E",
            "│ Structure",
            "│  Indent                   Tab",
            "│  Outdent            Shift+Tab",
            "│  Undo                  Ctrl+Z",
            "│  Redo    Ctrl+Shift+Z, Ctrl+Y",
            "│  Copy                  Ctrl+C",
            "│  Paste Below           Ctrl+V",
            "│ View and Panels",
            "│  Quit                  Ctrl+Q",
        ]
    );
    assert!(screen(&mut app)[3].contains("- Milk?"), "typed");
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(right_of(&mut app, 48)[1], "│ Moving", "still open");
}

// FR-KEY-04, ui.md §1: below 80 columns the panel covers the right of the outline.
#[test]
fn a_narrow_screen_draws_the_panel_over_the_outline_and_cuts_the_list_short() {
    let mut app = Harness::with_text("- A task with a rather long name\n", 60, 12);
    app.type_keys("?");
    assert_eq!(
        screen(&mut app)[..11],
        [
            "  ⌂                         │Shortcuts",
            "────────────────────────────│ Moving",
            "▌ - A task with a rather lon│  Next Block              j, ↓",
            "                            │  Previous Block          k, ↑",
            "                            │  Collapse                h, ←",
            "                            │  Expand                  l, →",
            "                            │ …",
            "                            │",
            "                            │ No Alt key seen yet: on macOS",
            "                            │ set Option as Meta or Esc+",
            "                            │ Kitty keyboard protocol: off",
        ]
    );
}

// ui.md §1: below 40×10 only the pane and the status line are drawn.
#[test]
fn a_tiny_screen_has_no_room_for_the_panel() {
    for (width, height) in [(39, 20), (60, 9)] {
        let mut app = Harness::with_text(OUTLINE, width, height);
        let before = screen(&mut app);
        app.type_keys("?");
        assert_eq!(screen(&mut app), before, "{width}×{height}");
    }
}

// ui.md §4, keybindings.md §1
#[test]
fn the_panel_notes_once_alt_arrives_and_whether_the_kitty_protocol_is_on() {
    let mut app = Harness::with_text(OUTLINE, 80, 30);
    app.tui = app.tui.with_kitty_keyboard(true);
    app.type_keys("?");
    app.press(KeyCode::Char('j'), KeyModifiers::ALT);
    let panel = right_of(&mut app, 48);
    assert_eq!(
        panel[26..],
        ["│ …", "│", "│ Kitty keyboard protocol: on", ""],
        "the Alt note is gone, so the list is longer"
    );
    assert_eq!(panel[25], "│  Indent                   Tab");
}

// D-54: the date prompt's panel spans the screen under the shortcuts panel.
#[test]
fn the_date_prompt_spans_the_screen_below_the_panel() {
    let mut app = Harness::with_text(OUTLINE, 80, 30);
    app.type_keys("?md");
    let rows = screen(&mut app);
    assert!(
        rows[25].starts_with('┌') && rows[25].ends_with('┐'),
        "{rows:#?}"
    );
    assert!(
        rows[24].ends_with("│ Kitty keyboard protocol: off"),
        "the shortcuts panel ends above it: {rows:#?}"
    );
}

// FR-KEY-04
#[test]
fn a_list_that_just_fits_is_not_cut_short() {
    let mut app = Harness::with_text(OUTLINE, 80, 24);
    app.type_keys("?");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    let panel = right_of(&mut app, 48);
    assert_eq!(panel[18..20], ["│  Quit                  Ctrl+Q", "│"]);
    assert_eq!(panel[22], "│ Kitty keyboard protocol: off");
}
