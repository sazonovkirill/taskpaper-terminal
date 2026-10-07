//! Step 6.4: the settings window, opened with `g,` or Open Settings, lists the settings by group;
//! every change applies at once and is written to `config.toml`, which keeps its comments
//! (FR-SET-02, FR-SET-03, FR-SET-04, ui.md §8).

use std::{fs, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::{Color, Modifier};
use taskpaper_terminal::ui::ColorSupport;

use crate::support::{Harness, line};

const OUTLINE: &str = "- buy milk and eggs from the shop\n";

const SETTINGS: &str = "# Mine\nicons = \"none\" # boxes otherwise\n";

const NORD_BACKGROUND: Color = Color::Rgb(0x2e, 0x34, 0x40);
const DRACULA_BACKGROUND: Color = Color::Rgb(0x28, 0x2a, 0x36);

fn press(app: &mut Harness, code: KeyCode) {
    app.press(code, KeyModifiers::NONE);
}

/// A 70×20 app on [`OUTLINE`] with `settings` as its settings file, its window opened with `g,`.
fn opened(settings: &str) -> Harness {
    let mut app = Harness::with_config(OUTLINE, settings, 70, 20);
    app.type_keys("g,");
    app
}

/// The rows of the settings window on screen, from its top border to its bottom one, from its
/// left border on.
fn window(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    let rows: Vec<String> = (0..buffer.area.height).map(|y| line(&buffer, y)).collect();
    let Some((top, left)) = rows.iter().enumerate().find_map(|(y, row)| {
        row.find("┌ Settings")
            .map(|at| (y, row[..at].chars().count()))
    }) else {
        return Vec::new();
    };
    let rows: Vec<String> = rows
        .iter()
        .skip(top)
        .map(|row| row.chars().skip(left).collect())
        .collect();
    let bottom = rows
        .iter()
        .position(|row| row.starts_with('└'))
        .expect("the window has a bottom border");
    rows.into_iter().take(bottom + 1).collect()
}

/// The screen row of the settings window's top border.
fn window_top(app: &mut Harness) -> u16 {
    let buffer = app.render().buffer().clone();
    (0..buffer.area.height)
        .find(|&y| line(&buffer, y).contains("┌ Settings"))
        .expect("the window is open")
}

/// The settings file's text.
fn config(app: &Harness) -> String {
    fs::read_to_string(app.config_file()).expect("the settings file exists")
}

/// The background of an empty cell at the bottom right of the screen.
fn background(app: &mut Harness) -> Color {
    let buffer = app.render().buffer();
    let area = buffer.area;
    buffer[(area.width - 2, area.height - 2)].bg
}

/// Goes to the Files group's settings.
fn files(app: &mut Harness) {
    press(app, KeyCode::Tab);
    press(app, KeyCode::Char('j'));
    press(app, KeyCode::Tab);
}

// FR-SET-02, FR-SET-03, ui.md §7, §8
#[test]
fn g_comma_opens_the_window_with_the_appearance_settings_and_esc_closes_it() {
    let mut app = opened(SETTINGS);
    let file = app.config_file();
    let footer = format!("│ Settings file: {}", file.display());
    let rows = window(&mut app);
    assert_eq!(
        rows[..9],
        [
            "┌ Settings ──────────────────────────────────────────────────────────┐",
            "│ Appearance    │   ● Theme                           Terminal Dark  │",
            "│ Files         │     The colors of the whole app                    │",
            "│               │ * ◇ Icons                                    none  │",
            "│               │     Nerd Font icons; choose none if they show as bo│",
            "│               │   ↩ Word wrap                                  on  │",
            "│               │     Wrap long text, or end it in … on one line     │",
            "│               │   ● Color tags                               icon  │",
            "│               │     A colored flag after the text, or a tinted line│",
        ]
    );
    assert!(
        rows[10].starts_with("│ Tab: groups/settings · Enter: change"),
        "{}",
        rows[10]
    );
    assert!(
        rows[11].starts_with(&footer[..footer.len().min(68)]),
        "{}",
        rows[11]
    );
    assert_eq!(rows.len(), 13, "as tall as the Files group needs");
    press(&mut app, KeyCode::Esc);
    assert_eq!(window(&mut app), Vec::<String>::new());
}

// FR-SET-02, FR-NAV-06
#[test]
fn open_settings_in_go_to_command_opens_the_window() {
    let mut app = Harness::with_config(OUTLINE, SETTINGS, 70, 20);
    app.press(KeyCode::Char('p'), KeyModifiers::CONTROL);
    app.type_keys("open settings");
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        window(&mut app)[1],
        "│ Appearance    │   ● Theme                           Terminal Dark  │"
    );
}

// FR-SET-03, ui.md §8
#[test]
fn tab_switches_to_the_groups_where_j_shows_the_files_settings() {
    let mut app = opened(SETTINGS);
    press(&mut app, KeyCode::Tab);
    press(&mut app, KeyCode::Char('j'));
    let buffer = app.render().buffer().clone();
    let files_row = (0..buffer.area.height)
        .find(|&y| line(&buffer, y).contains("│ Files "))
        .expect("the groups are shown");
    let x = (0..buffer.area.width)
        .find(|&x| buffer[(x, files_row)].symbol() == "F")
        .expect("a group");
    assert!(
        buffer[(x, files_row)].modifier.contains(Modifier::REVERSED),
        "the groups have the keys"
    );
    press(&mut app, KeyCode::Tab);
    let rows = window(&mut app);
    assert_eq!(
        rows[1..9],
        [
            "│ Appearance    │   ◷ Autosave delay                        1000 ms  │",
            "│ Files         │     How long after the last change the file is save│",
            "│               │   ↺ Backups                                    on  │",
            "│               │     Keep a copy of each file as it was each day    │",
            "│               │   ▭ Backup folder                         default  │",
            "│               │     Where the copies go; empty for the default fold│",
            "│               │   ↺ Backups kept                               30  │",
            "│               │     How many daily copies of each file are kept    │",
        ]
    );
}

// ui.md §8
#[test]
fn a_short_window_scrolls_to_the_highlighted_setting_which_is_reversed() {
    let mut app = Harness::with_config(OUTLINE, SETTINGS, 70, 12);
    app.type_keys("g,");
    files(&mut app);
    app.type_keys("jjj");
    let rows = window(&mut app);
    assert_eq!(
        rows[1..7],
        [
            "│ Appearance    │   ↺ Backups                                    on  │",
            "│ Files         │     Keep a copy of each file as it was each day    │",
            "│               │   ▭ Backup folder                         default  │",
            "│               │     Where the copies go; empty for the default fold│",
            "│               │   ↺ Backups kept                               30  │",
            "│               │     How many daily copies of each file are kept    │",
        ]
    );
    let top = window_top(&mut app);
    let buffer = app.render().buffer().clone();
    let reversed = |row: u16| {
        buffer[(30, top + row)]
            .modifier
            .contains(Modifier::REVERSED)
    };
    assert_eq!(
        (1..7).map(reversed).collect::<Vec<_>>(),
        [false, false, false, false, true, true]
    );
}

// FR-UI-06, FR-SET-02, FR-SET-04, ui.md §3, §8
#[test]
fn the_theme_picker_previews_the_highlighted_theme_and_enter_keeps_and_writes_it() {
    let mut app = opened(SETTINGS).with_colors(ColorSupport::TrueColor);
    press(&mut app, KeyCode::Enter);
    assert!(
        app.render().to_string().contains("┌Theme"),
        "the value picker opens over the window"
    );
    app.type_keys("nord");
    assert_eq!(
        background(&mut app),
        NORD_BACKGROUND,
        "previewed behind the window"
    );
    press(&mut app, KeyCode::Esc);
    assert_ne!(background(&mut app), NORD_BACKGROUND, "Esc goes back");
    assert_eq!(config(&app), SETTINGS);
    press(&mut app, KeyCode::Enter);
    app.type_keys("drac");
    press(&mut app, KeyCode::Enter);
    app.settle();
    assert_eq!(background(&mut app), DRACULA_BACKGROUND);
    assert_eq!(
        window(&mut app)[1],
        "│ Appearance    │ * ● Theme                                 Dracula  │"
    );
    assert_eq!(config(&app), format!("{SETTINGS}theme = \"dracula\"\n"));
}

// FR-SET-02, FR-SET-04, ui.md §8
#[test]
fn l_and_h_cycle_a_choice_and_r_resets_it_each_written_at_once() {
    let mut app = opened(SETTINGS).with_colors(ColorSupport::TrueColor);
    press(&mut app, KeyCode::Char('l'));
    app.settle();
    assert_eq!(
        config(&app),
        format!("{SETTINGS}theme = \"catppuccin-mocha\"\n")
    );
    press(&mut app, KeyCode::Char('h'));
    press(&mut app, KeyCode::Char('h'));
    app.settle();
    assert_eq!(
        config(&app),
        format!("{SETTINGS}theme = \"solarized-light\"\n")
    );
    press(&mut app, KeyCode::Char('r'));
    app.settle();
    assert_eq!(
        config(&app),
        format!("{SETTINGS}theme = \"terminal-dark\"\n")
    );
    assert!(
        window(&mut app)[1].starts_with("│ Appearance    │   ● Theme"),
        "no longer marked"
    );
}

// FR-UI-10, FR-SET-04, ui.md §7, §8
#[test]
fn icons_switch_at_once_behind_the_window() {
    let mut app = opened(SETTINGS);
    press(&mut app, KeyCode::Char('j'));
    let status = |app: &mut Harness| {
        let buffer = app.render().buffer().clone();
        line(&buffer, buffer.area.height - 1)
    };
    assert!(status(&mut app).starts_with("↕ NAVIGATE"));
    press(&mut app, KeyCode::Char('l'));
    app.settle();
    assert!(
        status(&mut app).starts_with("\u{f01bf} NAVIGATE"),
        "Nerd Font icons now"
    );
    assert_eq!(config(&app), "# Mine\nicons = \"nerd\" # boxes otherwise\n");
    assert!(window(&mut app)[3].starts_with("│               │   \u{f0832} Icons"));
}

// FR-UI-05, FR-SET-03, FR-SET-04
#[test]
fn word_wrap_toggles_at_once_and_is_kept_for_the_next_start() {
    let mut app = Harness::with_config(OUTLINE, "", 30, 20);
    app.type_keys("g,jj");
    press(&mut app, KeyCode::Char(' '));
    app.settle();
    assert_eq!(config(&app), "word_wrap = false\n");
    press(&mut app, KeyCode::Esc);
    let buffer = app.render().buffer().clone();
    assert_eq!(line(&buffer, 2), "▌ - buy milk and eggs from th…");
    let mut app = Harness::with_config(OUTLINE, "word_wrap = false\n", 30, 20);
    let buffer = app.render().buffer().clone();
    assert_eq!(
        line(&buffer, 2),
        "▌ - buy milk and eggs from th…",
        "off from the start"
    );
}

// FR-TAG-05, FR-SET-03, FR-SET-04, D-74, ui.md §2, §8
#[test]
fn color_tags_switch_from_a_flag_to_a_tinted_line_and_are_kept_for_the_next_start() {
    let text = "- milk @red\n";
    let mut app = Harness::with_config(text, "", 30, 20);
    let buffer = app.render().buffer().clone();
    assert_eq!(line(&buffer, 2), "▌ - milk ●", "a flag by default");
    assert_eq!(
        (buffer[(9, 2)].fg, buffer[(9, 2)].bg),
        (Color::Red, Color::Black)
    );
    app.type_keys("g,jjj");
    press(&mut app, KeyCode::Char('l'));
    app.settle();
    assert_eq!(config(&app), "color_tags = \"line highlight\"\n");
    press(&mut app, KeyCode::Esc);
    let tinted = |app: &mut Harness| {
        let buffer = app.render().buffer().clone();
        (line(&buffer, 2), buffer[(9, 2)].bg)
    };
    assert_eq!(tinted(&mut app), ("▌ - milk".to_owned(), Color::Red));
    let mut app = Harness::with_config(text, "color_tags = \"line highlight\"\n", 30, 20);
    assert_eq!(
        tinted(&mut app),
        ("▌ - milk".to_owned(), Color::Red),
        "tinted from the start"
    );
}

// FR-SET-02, FR-DOC-02, file-io.md §2: try it, change the autosave delay.
#[test]
fn the_autosave_delay_takes_a_number_of_milliseconds_and_saves_that_long_after_a_change() {
    let mut app = opened(SETTINGS);
    files(&mut app);
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        window(&mut app)[10],
        "│ Autosave delay: 1000                                               │"
    );
    press(&mut app, KeyCode::Backspace);
    press(&mut app, KeyCode::Backspace);
    press(&mut app, KeyCode::Backspace);
    press(&mut app, KeyCode::Enter);
    let rows = window(&mut app);
    assert_eq!(
        rows[9], "│ Autosave delay must be a whole number, at least 100                │",
        "1 ms is refused"
    );
    assert_eq!(
        rows[10],
        "│ Autosave delay: 1                                                  │"
    );
    let top = window_top(&mut app);
    assert_eq!(
        app.terminal.get_cursor_position().map(|p| (p.x, p.y)).ok(),
        Some((19, top + 10)),
        "the caret ends the prompt"
    );
    app.type_keys("300");
    press(&mut app, KeyCode::Enter);
    app.settle();
    assert_eq!(
        config(&app),
        format!("{SETTINGS}autosave_delay_ms = 1300\n")
    );
    assert_eq!(
        window(&mut app)[1],
        "│ Appearance    │ * ◷ Autosave delay                        1300 ms  │"
    );
    press(&mut app, KeyCode::Esc);
    app.type_keys("x");
    app.wait(Duration::from_millis(1000));
    app.settle();
    assert_eq!(
        fs::read_to_string(app.path.as_ref().unwrap()).unwrap(),
        OUTLINE,
        "not yet"
    );
    app.wait(Duration::from_millis(300));
    app.settle();
    assert_eq!(
        fs::read_to_string(app.path.as_ref().unwrap()).unwrap(),
        "- buy milk and eggs from the shop @done(2026-10-05)\n"
    );
}

// FR-DOC-07, FR-SET-03, FR-SET-04, file-io.md §2
#[test]
fn backups_turn_off_and_take_a_folder_and_a_number_and_reset_removes_the_folder() {
    let mut app = opened(SETTINGS);
    files(&mut app);
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Enter);
    app.type_keys("~/copies");
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Backspace);
    press(&mut app, KeyCode::Backspace);
    app.type_keys("5");
    press(&mut app, KeyCode::Enter);
    app.settle();
    assert_eq!(
        config(&app),
        format!("{SETTINGS}\n[backups]\nenabled = false\ndir = \"~/copies\"\nkeep = 5\n")
    );
    assert_eq!(
        window(&mut app)[5],
        "│               │ * ▭ Backup folder                        ~/copies  │"
    );
    press(&mut app, KeyCode::Esc);
    app.type_keys("g,");
    files(&mut app);
    app.type_keys("jjj");
    let folder = &window(&mut app)[5];
    assert!(
        folder.starts_with("│               │ * ▭ Backup folder") && folder.ends_with("/copies  │"),
        "reopened, the window shows the folder in use: {folder}"
    );
    press(&mut app, KeyCode::Char('k'));
    press(&mut app, KeyCode::Char('r'));
    app.settle();
    assert_eq!(
        config(&app),
        format!("{SETTINGS}\n[backups]\nenabled = false\nkeep = 5\n")
    );
    app.type_keys("kr");
    app.settle();
    assert!(config(&app).contains("enabled = true\n"));
    assert_eq!(
        window(&mut app)[5],
        "│               │   ▭ Backup folder                         default  │"
    );
}

// FR-DOC-07, file-io.md §2
#[test]
fn backups_turned_back_on_copy_the_file_at_the_next_save() {
    let mut app = Harness::with_config(OUTLINE, "[backups]\nenabled = false\n", 70, 20);
    app.type_keys("g,");
    files(&mut app);
    app.type_keys("j ");
    press(&mut app, KeyCode::Esc);
    assert!(!app.backups.exists(), "none made at the start");
    app.type_keys("x");
    app.wait(Duration::from_secs(1));
    app.settle();
    assert!(app.backups.exists(), "made before the save");
}

// ui.md §8
#[test]
fn slash_filters_the_settings_by_name() {
    let mut app = opened(SETTINGS);
    app.type_keys("/back");
    let rows = window(&mut app);
    assert_eq!(
        rows[1],
        "│ Files         │   ↺ Backups                                    on  │"
    );
    assert_eq!(
        rows[10],
        "│ /back                                                              │"
    );
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        window(&mut app)[10],
        "│ Backup folder:                                                     │"
    );
    press(&mut app, KeyCode::Esc);
    app.type_keys("/zz");
    assert_eq!(
        window(&mut app)[1],
        "│               │ No settings match                                  │"
    );
    press(&mut app, KeyCode::Esc);
    assert_eq!(
        window(&mut app)[1],
        "│ Appearance    │   ● Theme                           Terminal Dark  │"
    );
}

// FR-SET-04, ui.md §8
#[test]
fn a_change_that_cannot_be_written_shows_in_the_window_and_the_status_line_and_holds() {
    let mut app = Harness::with_config(OUTLINE, "theme = \n", 140, 20);
    app.hide_sidebar();
    app.type_keys("g,jj ");
    app.settle();
    let buffer = app.render().buffer().clone();
    let status = line(&buffer, 19);
    assert!(
        status.contains("config.toml: `word_wrap` not saved: not valid TOML"),
        "{status}"
    );
    assert!(
        window(&mut app)[9].starts_with("│ `word_wrap` not saved: not valid TOML"),
        "{:?}",
        window(&mut app)[9]
    );
    assert!(window(&mut app)[5].contains("off"), "kept for the session");
    assert_eq!(config(&app), "theme = \n");
    let mut app = Harness::with_text(OUTLINE, 140, 20);
    app.hide_sidebar();
    app.type_keys("g,");
    assert!(
        window(&mut app)[11]
            .starts_with("│ No settings file: neither $XDG_CONFIG_HOME nor a home folder is set"),
        "{:?}",
        window(&mut app)[11]
    );
}

// FR-KEY-04, FR-SET-02
#[test]
fn the_shortcuts_panel_lists_open_settings() {
    let mut app = Harness::with_text(OUTLINE, 120, 50);
    app.type_keys("?");
    app.render();
    app.type_keys(")");
    let screen = app.render().to_string();
    assert!(screen.contains("Open Settings"), "{screen}");
    assert!(screen.contains("g,"), "{screen}");
}

// FR-DOC-05, FR-SET-02
#[test]
fn the_demo_says_where_the_settings_are() {
    let app = Harness::demo(60, 20);
    let text = fs::read_to_string(app.path.as_ref().unwrap()).unwrap();
    assert!(text.contains("Press g, for the settings"), "{text}");
}
