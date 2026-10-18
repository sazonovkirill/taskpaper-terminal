//! D-69: the screen behind a dialog that takes the keys is dimmed; the dialog is not.

use std::fs;

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    buffer::{Buffer, Cell},
    style::Modifier,
};
use taskpaper_terminal::{app::Message, ui::Input};

use crate::support::Harness;

const GROCERIES: &str = "Groceries:\n\t- Milk\n\t- Eggs\n";

fn dimmed(buffer: &Buffer, x: u16, y: u16) -> bool {
    buffer[(x, y)].modifier.contains(Modifier::DIM)
}

/// Whether the status line is dimmed and something drawn, the dialog, is not.
fn dims_behind_a_dialog(app: &mut Harness) -> bool {
    let buffer = app.render().buffer().clone();
    let status = buffer.area.bottom() - 1;
    let undimmed = buffer
        .content
        .iter()
        .any(|cell| cell.symbol() != " " && !cell.modifier.contains(Modifier::DIM));
    dimmed(&buffer, 0, status) && undimmed
}

fn status_dimmed(app: &mut Harness) -> bool {
    let buffer = app.render().buffer().clone();
    dimmed(&buffer, 0, buffer.area.bottom() - 1)
}

/// The keys that open a dialog.
enum Open {
    Control(char),
    Keys(&'static str),
}

// ui.md §1, D-69
#[test]
fn every_dialog_dims_the_screen_behind_it() {
    let dialogs = [
        ("Go to Command", Open::Control('p')),
        ("Go to Symbol", Open::Control('k')),
        ("Go to Project", Open::Keys("gP")),
        ("Move to", Open::Keys("jgm")),
        ("theme chooser", Open::Control('t')),
        ("settings", Open::Keys("g,")),
        ("context menu", Open::Control('\\')),
        ("edit node", Open::Keys("e")),
        ("color picker", Open::Keys("mc")),
        ("type picker", Open::Keys("my")),
        ("date prompt", Open::Keys("md")),
    ];
    for (name, open) in dialogs {
        let mut app = Harness::with_text(GROCERIES, 80, 20);
        assert!(!status_dimmed(&mut app), "{name}: nothing dimmed before");
        match open {
            Open::Control(key) => app.press(KeyCode::Char(key), KeyModifiers::CONTROL),
            Open::Keys(keys) => app.type_keys(keys),
        }
        assert!(dims_behind_a_dialog(&mut app), "{name}");
        app.press(KeyCode::Esc, KeyModifiers::NONE);
        assert!(!status_dimmed(&mut app), "{name}: nothing dimmed after");
    }
}

// ui.md §1, D-69
#[test]
fn save_as_dims_the_screen_behind_it() {
    let mut app = Harness::with_text(GROCERIES, 80, 20);
    app.press(KeyCode::Char('p'), KeyModifiers::CONTROL);
    app.type_keys("save as");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    assert!(dims_behind_a_dialog(&mut app));
}

// ui.md §1, ui.md §4, D-69
#[test]
fn the_shortcuts_panel_and_the_command_menu_dim_nothing() {
    let mut app = Harness::with_text(GROCERIES, 80, 20);
    app.type_keys("?");
    assert!(!status_dimmed(&mut app), "shortcuts panel");
    app.type_keys("?ja/");
    let buffer = app.render().buffer().clone();
    let screen: String = buffer.content.iter().map(Cell::symbol).collect();
    assert!(screen.contains("Commands:"), "the command menu is open");
    assert!(!status_dimmed(&mut app), "command menu");
}

// F-FILE-07, D-69
#[test]
fn the_conflict_prompt_dims_the_status_line() {
    let mut app = Harness::with_text(GROCERIES, 140, 8);
    app.settle();
    fs::write(app.path.clone().expect("a file"), b"- Caf\xe9\n").unwrap();
    app.send(Input::Core(Message::CheckFile));
    app.settle();
    assert!(dims_behind_a_dialog(&mut app));
}
