//! Step 6.2: Choose Theme in Go to Command previews each theme as it is highlighted, `Enter`
//! keeps it and writes it to `config.toml`, `Esc` goes back; `COLORTERM` and `NO_COLOR` decide
//! what colors are drawn (FR-UI-06, FR-UI-08, N-UX-03, ui.md §3).

use std::fs;

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    style::{Color, Modifier},
};
use taskpaper_terminal::ui::ColorSupport;

use crate::support::{Harness, line};

const OUTLINE: &str = "Trip:\n\t- Book the hotel @red\n\tNear the station\n";

const SETTINGS: &str = "# Mine\nautosave_delay_ms = 500 # half a second\n";

const NORD_BACKGROUND: Color = Color::Rgb(0x2e, 0x34, 0x40);
const DRACULA_BACKGROUND: Color = Color::Rgb(0x28, 0x2a, 0x36);

fn press(app: &mut Harness, code: KeyCode) {
    app.press(code, KeyModifiers::NONE);
}

/// Opens the theme picker from Go to Command.
fn choose_theme(app: &mut Harness) {
    app.press(KeyCode::Char('p'), KeyModifiers::CONTROL);
    app.type_keys("choose theme");
    press(app, KeyCode::Enter);
}

/// The background of an empty cell at the bottom right of the outline.
fn background(app: &mut Harness) -> Color {
    let buffer = app.render().buffer();
    let area = buffer.area;
    buffer[(area.width - 2, area.height - 2)].bg
}

fn screen(buffer: &Buffer) -> Vec<String> {
    (buffer.area.top()..buffer.area.bottom())
        .map(|y| line(buffer, y))
        .collect()
}

// FR-UI-06, FR-UI-08, ui.md §3
#[test]
fn choose_theme_lists_the_themes_with_the_one_in_use_highlighted() {
    let mut app =
        Harness::with_config(OUTLINE, SETTINGS, 60, 24).with_colors(ColorSupport::TrueColor);
    assert_eq!(
        background(&mut app),
        Color::Reset,
        "Terminal Dark, in the terminal's own background, by default (D-63)"
    );
    choose_theme(&mut app);
    let buffer = app.render().buffer().clone();
    let rows: Vec<String> = screen(&buffer)[5..21]
        .iter()
        .map(|row| row.trim_end().to_owned())
        .collect();
    let names = [
        "Terminal Dark",
        "Catppuccin Mocha",
        "Catppuccin Latte",
        "Tokyo Night",
        "Dracula",
        "Nord",
        "Gruvbox Dark",
        "Rosé Pine",
        "Rosé Pine Dawn",
        "Kanagawa Wave",
        "Everforest Dark",
        "One Dark",
        "Solarized Light",
    ];
    let mut expected = vec![
        "┌Theme─────────────────────────────────────────────────────┐".to_owned(),
        format!("│›{:57}│", ""),
    ];
    expected.extend(names.map(|name| format!("│ ● {name:<55}│")));
    expected.push("└──────────────────────────────────────────────────────────┘".to_owned());
    assert_eq!(rows, expected);
    assert!(
        buffer[(3, 7)].modifier.contains(Modifier::REVERSED),
        "Terminal Dark is highlighted"
    );
}

// FR-UI-06, FR-SET-04
#[test]
fn highlighting_previews_a_theme_esc_goes_back_and_enter_keeps_and_saves_it() {
    let mut app =
        Harness::with_config(OUTLINE, SETTINGS, 60, 24).with_colors(ColorSupport::TrueColor);
    choose_theme(&mut app);
    app.type_keys("nord");
    assert_eq!(
        background(&mut app),
        NORD_BACKGROUND,
        "the highlighted theme shows"
    );
    press(&mut app, KeyCode::Esc);
    assert_eq!(background(&mut app), Color::Reset, "Esc goes back");
    assert_eq!(fs::read_to_string(app.config_file()).unwrap(), SETTINGS);
    choose_theme(&mut app);
    for _ in 0..4 {
        press(&mut app, KeyCode::Down);
    }
    assert_eq!(
        background(&mut app),
        DRACULA_BACKGROUND,
        "arrows preview too"
    );
    press(&mut app, KeyCode::Enter);
    app.settle();
    assert_eq!(background(&mut app), DRACULA_BACKGROUND, "Enter keeps it");
    assert!(
        !screen(app.render().buffer())
            .iter()
            .any(|row| row.contains("┌Theme")),
        "the picker closes"
    );
    assert_eq!(
        fs::read_to_string(app.config_file()).unwrap(),
        format!("{SETTINGS}theme = \"dracula\"\n"),
        "written to the settings file, which keeps its comments"
    );
}

// FR-UI-06, FR-SET-01
#[test]
fn the_theme_in_the_config_file_is_used_from_the_start() {
    let settings = "theme = \"nord\"\n";
    let mut app =
        Harness::with_config(OUTLINE, settings, 60, 8).with_colors(ColorSupport::TrueColor);
    let buffer = app.render().buffer().clone();
    assert_eq!(background(&mut app), NORD_BACKGROUND);
    assert_eq!(
        buffer[(2, 2)].fg,
        Color::Rgb(0x88, 0xc0, 0xd0),
        "a project in Nord's accent"
    );
    let status = line(&buffer, 7);
    assert_eq!(status, app.status(60).trim_end(), "no problems reported");
}

// N-UX-03, ui.md §3, D-63
#[test]
fn without_truecolor_a_truecolor_theme_is_drawn_as_terminal_dark() {
    for theme in ["nord", "solarized-light"] {
        let settings = format!("theme = \"{theme}\"\n");
        let mut app = Harness::with_config(OUTLINE, &settings, 60, 8);
        let buffer = app.render().buffer().clone();
        assert_eq!(background(&mut app), Color::Reset, "{theme}");
        assert_eq!(buffer[(2, 2)].fg, Color::LightBlue, "{theme}");
        assert_eq!(buffer[(6, 3)].bg, Color::Red, "{theme}: the red tag's row");
        assert_eq!(buffer[(4, 4)].symbol(), "N");
        assert_eq!(
            buffer[(4, 4)].fg,
            Color::Indexed(248),
            "{theme}: the note is dimmed"
        );
    }
}

// N-UX-03, ui.md §3
#[test]
fn no_color_draws_no_colors_only_modifiers() {
    let mut app =
        Harness::with_config(OUTLINE, "theme = \"nord\"\n", 60, 8).with_colors(ColorSupport::None);
    let buffer = app.render().buffer().clone();
    for cell in &buffer.content {
        assert_eq!((cell.fg, cell.bg), (Color::Reset, Color::Reset), "{cell:?}");
    }
    assert!(
        buffer[(2, 2)].modifier.contains(Modifier::BOLD),
        "the project is still bold"
    );
}

// FR-SET-04
#[test]
fn a_theme_that_cannot_be_saved_is_reported_and_kept_for_the_session() {
    let mut app =
        Harness::with_config(OUTLINE, "theme = \n", 140, 8).with_colors(ColorSupport::TrueColor);
    choose_theme(&mut app);
    app.type_keys("nord");
    press(&mut app, KeyCode::Enter);
    app.settle();
    assert_eq!(background(&mut app), NORD_BACKGROUND);
    let status = line(app.render().buffer(), 7);
    assert!(
        status.contains("config.toml: `theme` not saved: not valid TOML"),
        "{status}"
    );
    assert_eq!(fs::read_to_string(app.config_file()).unwrap(), "theme = \n");
    let mut app = Harness::with_text(OUTLINE, 140, 8);
    choose_theme(&mut app);
    press(&mut app, KeyCode::Enter);
    let status = line(app.render().buffer(), 7);
    assert!(
        status.contains("config.toml: `theme` not saved: no place for it"),
        "without a settings file: {status}"
    );
}
