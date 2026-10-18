//! Step 6.1: `config.toml` rebinds keys, and what the app ignores of it is reported once in the
//! status line (FR-SET-01, FR-SET-04, keybindings.md §5).

use std::time::Duration;

use crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Harness, line};

const OUTLINE: &str = "- Milk\n- Eggs\n";

const REBOUND: &str = "\
# Mine
[keymap.navigate]
\"ctrl+d\" = \"toggle_done\"
x = \"none\"
\"g x\" = \"Toggle Done\"

[keymap.edit]
\"ctrl+g\" = \"inline_code\"
";

// FR-SET-01, keybindings.md §5
#[test]
fn the_config_file_binds_keys_to_commands_and_unbinds_them() {
    let mut app = Harness::with_config(OUTLINE, REBOUND, 60, 8);
    app.press(KeyCode::Char('d'), KeyModifiers::CONTROL);
    app.type_keys("jx");
    app.wait(Duration::from_secs(2));
    app.settle();
    assert_eq!(
        std::fs::read_to_string(app.path.as_ref().unwrap()).unwrap(),
        "- Milk @done(2026-10-05)\n- Eggs\n",
        "Ctrl+D completes; x does nothing"
    );
    app.type_keys("gx");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    app.type_keys("a");
    app.press(KeyCode::Char('g'), KeyModifiers::CONTROL);
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    app.wait(Duration::from_secs(2));
    app.settle();
    assert_eq!(
        std::fs::read_to_string(app.path.as_ref().unwrap()).unwrap(),
        "- Milk @done(2026-10-05)\n- `Eggsa` @done(2026-10-05)\n",
        "gx completes; Ctrl+G toggles inline code in edit mode"
    );
    let status = line(app.render().buffer(), 7);
    assert_eq!(status, app.status(60).trim_end(), "no problems reported");
}

// FR-KEY-04, ui.md §4, keybindings.md §5
#[test]
fn the_shortcuts_panel_go_to_command_and_the_context_menu_show_the_config_files_keys() {
    let mut app = Harness::with_config(OUTLINE, REBOUND, 120, 60);
    app.type_keys("?");
    let buffer = app.render().buffer().clone();
    let rows: Vec<String> = (0..60).map(|y| line(&buffer, y)).collect();
    assert!(
        rows.iter()
            .any(|row| row.ends_with("Ctrl+D, gx, Ctrl+Enter")),
        "{rows:#?}"
    );
    app.type_keys("?:toggle done");
    let buffer = app.render().buffer().clone();
    let rows: Vec<String> = (0..60).map(|y| line(&buffer, y)).collect();
    assert!(
        rows.iter()
            .any(|row| row.contains("Toggle Done") && row.contains("Ctrl+D, gx, Ctrl+Enter")),
        "{rows:#?}"
    );
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    app.press(KeyCode::Char('\\'), KeyModifiers::CONTROL);
    app.type_keys("comp");
    let buffer = app.render().buffer().clone();
    let rows: Vec<String> = (0..60).map(|y| line(&buffer, y)).collect();
    assert!(
        rows.iter()
            .any(|row| row.contains("Ctrl+D, gx, Ctrl+Enter")),
        "{rows:#?}"
    );
}

// FR-SET-04
#[test]
fn what_the_config_file_sets_wrongly_is_reported_once_and_the_rest_applies() {
    let settings = "\
theme = \"neon\"
[keymap.navigate]
\"ctrl+d\" = \"toggle_done\"
q = \"frobnicate\"
";
    let mut app = Harness::with_config(OUTLINE, settings, 180, 8);
    let status = line(app.render().buffer(), 7);
    assert_eq!(
        status,
        app.status_with(180, " · config.toml: `theme` must be a built-in theme's name, such as \"nord\"; [keymap.navigate] \"q\": no command \"frobnicate\" here").trim_end().to_owned()
    );
    app.press(KeyCode::Char('d'), KeyModifiers::CONTROL);
    app.wait(Duration::from_secs(11));
    app.settle();
    assert_eq!(
        std::fs::read_to_string(app.path.as_ref().unwrap()).unwrap(),
        "- Milk @done(2026-10-05)\n- Eggs\n",
        "the rest of the file applies"
    );
    let status = line(app.render().buffer(), 7);
    assert_eq!(
        status,
        app.status(180).trim_end(),
        "reported once, for a while"
    );
}

// FR-SET-04
#[test]
fn a_config_file_that_is_not_toml_is_reported_and_the_app_starts_with_the_defaults() {
    let mut app = Harness::with_config(OUTLINE, "[keymap\n", 100, 8);
    let status = line(app.render().buffer(), 7);
    assert!(
        status.starts_with(&format!(
            "{} · config.toml: line 1: ",
            app.status(100).trim_end()
        )),
        "{status}"
    );
    app.type_keys("x");
    app.wait(Duration::from_secs(2));
    app.settle();
    assert_eq!(
        std::fs::read_to_string(app.path.as_ref().unwrap()).unwrap(),
        "- Milk @done(2026-10-05)\n- Eggs\n"
    );
}

// FR-DOC-02, FR-SET-01, file-io.md §2
#[test]
fn the_config_file_sets_the_autosave_delay() {
    let mut app = Harness::with_config(OUTLINE, "autosave_delay_ms = 5000\n", 60, 8);
    app.type_keys("x");
    app.wait(Duration::from_secs(4));
    app.settle();
    let path = app.path.clone().unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), OUTLINE, "not yet");
    app.wait(Duration::from_secs(1));
    app.settle();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "- Milk @done(2026-10-05)\n- Eggs\n"
    );
}

// FR-DOC-07, FR-SET-01, file-io.md §2
#[test]
fn the_config_file_moves_backups_to_a_folder_under_home_or_turns_them_off() {
    let mut app = Harness::with_config(OUTLINE, "[backups]\ndir = \"~/copies\"\n", 60, 8);
    app.settle();
    let copies = std::fs::read_dir(app.home.join("copies")).unwrap().count();
    assert_eq!(copies, 1, "one folder for the file");
    assert!(!app.backups.exists(), "not in the default folder");
    let mut app = Harness::with_config(OUTLINE, "[backups]\nenabled = false\n", 60, 8);
    app.settle();
    assert!(!app.backups.exists(), "no daily copy");
}

// keybindings.md §1, §5, D-53, D-136: a configured key that also starts a longer sequence acts
// once the sequence times out, or at once when a key follows that continues none, and that key
// then acts where it left the app.
#[test]
fn a_configured_key_that_starts_a_sequence_acts_when_it_ends() {
    let mut app = Harness::with_config(
        OUTLINE,
        "[keymap.navigate]\ng = \"New Block Below\"\n",
        60,
        8,
    );
    app.type_keys("gB");
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    app.type_keys("g");
    app.wait(Duration::from_secs(1));
    app.type_keys("C");
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    app.wait(Duration::from_secs(2));
    app.settle();
    assert_eq!(
        std::fs::read_to_string(app.path.as_ref().unwrap()).unwrap(),
        "- Milk\n- B\n- C\n- Eggs\n",
        "B goes into the block g added, and C into the one added once g timed out"
    );
}
