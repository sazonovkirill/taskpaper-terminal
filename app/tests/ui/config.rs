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
    app.hide_sidebar();
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
    app.type_keys("\\comp");
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
theme = \"nord\"
[keymap.navigate]
\"ctrl+d\" = \"toggle_done\"
q = \"frobnicate\"
";
    let mut app = Harness::with_config(OUTLINE, settings, 140, 8);
    let status = line(app.render().buffer(), 7);
    assert_eq!(
        status,
        format!(
            "{} · config.toml: unknown setting `theme`; [keymap.navigate] \"q\": no command \"frobnicate\" here",
            app.status(140).trim_end()
        )
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
        app.status(140).trim_end(),
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
