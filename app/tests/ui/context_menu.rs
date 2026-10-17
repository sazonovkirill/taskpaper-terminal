//! Step 4.9: `\`, `Shift+F10` and the Menu key open a context menu of the commands for the focused
//! block, the selection or the focused Navigator entry, grouped, each with its keys, under the
//! focused row; typing filters, `Enter` runs the highlighted command and `Esc` closes it.

use std::{fs, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::Modifier;

use crate::support::{Harness, focused_rows, line};

const TEXT: &str = "Trip:\n\t- Book the hotel\n\t\tNear the station\n\t- Pack\nOther\n";

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

/// Whether a context menu is on screen.
fn open(app: &mut Harness) -> bool {
    screen(app).iter().any(|row| row.contains("┌Commands:"))
}

/// The saved file once autosave has run.
fn saved(app: &mut Harness) -> String {
    app.wait(Duration::from_secs(1));
    app.settle();
    fs::read_to_string(app.path.clone().expect("a file was opened")).expect("the file is saved")
}

// FR-KEY-06, ui.md §5, keybindings.md §2
#[test]
fn backslash_on_a_task_lists_its_commands_grouped_with_their_keys_under_it() {
    let mut app = Harness::with_text(TEXT, 70, 16);
    app.type_keys("j\\");
    let rows = screen(&mut app);
    assert_eq!(
        rows[3..9],
        [
            "▌   - Book the hotel",
            "      ┌Commands:────────────────────────────────────────┐",
            "    - │  › Edit                                  Enter  │",
            "  Othe│  ✎ Edit Node                                 e  │",
            "      │  ≡ Open Attachments                          &  │",
            "      │  › New Block Below                           o  │",
        ],
        "anchored at the task's text, below it"
    );
    assert_eq!(
        rows[14], "      └─────────────────────────────────────────────────┘",
        "no taller than the room below the task"
    );
    let buffer = app.render().buffer().clone();
    assert!(
        buffer[(8, 5)].modifier.contains(Modifier::REVERSED),
        "the first command is highlighted"
    );
}

// FR-KEY-06, keybindings.md §1, D-42
#[test]
fn shift_f10_and_the_menu_key_open_it_too_and_esc_closes_it() {
    let mut app = Harness::with_text(TEXT, 70, 16);
    app.press(KeyCode::F(10), KeyModifiers::SHIFT);
    assert!(open(&mut app));
    press(&mut app, KeyCode::Esc);
    assert!(!open(&mut app));
    press(&mut app, KeyCode::Menu);
    assert!(open(&mut app));
    press(&mut app, KeyCode::Esc);
    assert!(!open(&mut app));
    assert_eq!(focused_rows(&mut app), ["Trip:"], "the focus stays");
}

// FR-KEY-06, keybindings.md §4
#[test]
fn typing_filters_and_enter_runs_the_command_as_its_keys_do() {
    let mut app = Harness::with_text(TEXT, 70, 16);
    app.type_keys("j\\comp");
    let rows = screen(&mut app);
    assert_eq!(
        rows[4..7],
        [
            "      ┌Commands:────────────────────┐",
            "    - │  ✓ Complete  x, Ctrl+Enter  │",
            "  Othe└comp─────────────────────────┘",
        ]
    );
    press(&mut app, KeyCode::Enter);
    assert!(!open(&mut app));
    assert_eq!(
        saved(&mut app),
        "Trip:\n\t- Book the hotel @done(2026-10-05)\n\t\tNear the station\n\t- Pack\nOther\n",
        "dated with the frontend's date"
    );
    app.type_keys("\\zoom");
    press(&mut app, KeyCode::Enter);
    assert_eq!(screen(&mut app)[0], "  ⌂ › Trip › Book the hotel");
}

// FR-KEY-06, keybindings.md §4
#[test]
fn a_character_that_matches_nothing_is_not_typed() {
    let mut app = Harness::with_text(TEXT, 70, 16);
    app.type_keys("\\zq");
    assert!(
        screen(&mut app).iter().any(|row| row.contains("└z─")),
        "{:#?}",
        screen(&mut app)
    );
}

// FR-KEY-06, FR-OPS-07
#[test]
fn on_the_selection_its_commands_act_on_every_selected_block() {
    let mut app = Harness::with_text(TEXT, 70, 16);
    app.type_keys("j");
    app.press(KeyCode::Down, KeyModifiers::SHIFT);
    app.press(KeyCode::Down, KeyModifiers::SHIFT);
    app.type_keys("\\");
    assert!(
        !screen(&mut app).iter().any(|row| row.contains("Edit ")),
        "the focused block's own commands are left out"
    );
    app.type_keys("due today");
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        saved(&mut app),
        "Trip:\n\t- Book the hotel @due(2026-10-05)\n\t\tNear the station @due(2026-10-05)\n\
         \t- Pack @due(2026-10-05)\nOther\n"
    );
}

// FR-KEY-06, FR-UI-01
#[test]
fn on_a_navigator_entry_it_offers_going_there() {
    let mut app = Harness::with_text(TEXT, 90, 17);
    app.render();
    app.press(KeyCode::Char('g'), KeyModifiers::CONTROL);
    for _ in 0..7 {
        press(&mut app, KeyCode::Down);
    }
    app.press(KeyCode::F(10), KeyModifiers::SHIFT);
    let rows = screen(&mut app);
    assert_eq!(
        rows[4..13],
        [
            "      Near the statio│  ☼ Today                                     │",
            "    - Pack           │  » Upcoming                                  │",
            "  Other              │  ! Overdue                                   │",
            "                     │  ┌Commands:─────────────────────────────────────────┐",
            "                     │  │  › Go to Entry                            Enter  │",
            "                     │  │  › Open in New Pane   Alt+Enter, Ctrl+V, Ctrl+T  │",
            "                     │  │──────────────────────────────────────────────────│",
            "                     │  │  › Return to Outline                        Esc  │",
            "                     │  └──────────────────────────────────────────────────┘",
        ],
        "over the window, above the focused entry on its last row"
    );
    let buffer = app.render().buffer().clone();
    assert!(
        buffer[(21, 9)].modifier.contains(Modifier::DIM),
        "the window is dimmed behind the menu"
    );
    assert!(!buffer[(25, 8)].modifier.contains(Modifier::DIM));
    press(&mut app, KeyCode::Enter);
    assert!(!open(&mut app));
    assert_eq!(screen(&mut app)[0], "  ⌂ › Trip", "the window closed");
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(
        screen(&mut app)[4],
        "▌     Near the station",
        "the pane has the keys and j moved its focus"
    );
}

// FR-KEY-06, keybindings.md §4 and §6, D-86, D-101: `Ctrl+R` opens it in the Navigator, whose
// filter takes `\`, for terminals that take `Shift+F10` and send no Menu key.
#[test]
fn ctrl_r_opens_it_on_a_navigator_entry_where_backslash_is_typed() {
    let mut app = Harness::with_text(TEXT, 90, 17);
    app.render();
    app.press(KeyCode::Char('g'), KeyModifiers::CONTROL);
    press(&mut app, KeyCode::Char('\\'));
    assert!(!open(&mut app), "\\ is typed into the filter");
    press(&mut app, KeyCode::Backspace);
    app.press(KeyCode::Char('r'), KeyModifiers::CONTROL);
    assert!(open(&mut app));
    assert!(
        screen(&mut app)
            .iter()
            .any(|row| row.contains("› Go to Entry")),
        "on the focused entry"
    );
}

// FR-KEY-06, navigation.md §7
#[test]
fn in_the_today_view_it_leaves_out_what_the_view_does_not_take() {
    let mut app = Harness::with_text("- Call Ann @star\n", 90, 30);
    app.render();
    app.press(KeyCode::Char('g'), KeyModifiers::CONTROL);
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('\\'));
    let rows = screen(&mut app).join("\n");
    assert!(rows.contains("Complete"), "{rows}");
    assert!(!rows.contains("Indent"), "{rows}");
    assert!(!rows.contains("New Block Below"), "{rows}");
}

// FR-KEY-06, FR-KEY-04
#[test]
fn the_shortcuts_panel_lists_it() {
    let mut app = Harness::with_text(TEXT, 120, 70);
    press(&mut app, KeyCode::Char('?'));
    let rows = screen(&mut app);
    let at = rows
        .iter()
        .position(|row| row.ends_with("Go to Command      Ctrl+P, :"))
        .expect("Go to Command is listed");
    assert!(rows[at + 1].ends_with("  Keymap                g?, F1"));
    assert!(rows[at + 2].ends_with("  Open Settings             g,"));
    assert!(
        rows[at + 3].ends_with("  Context … \\, Shift+F10, Menu"),
        "{}",
        rows[at + 3]
    );
}
