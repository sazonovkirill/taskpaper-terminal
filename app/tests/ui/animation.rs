//! Step 7.1: the landing flash where focus jumps, the highlight of what an undo, a paste or a
//! reload changed, the flash of a refused key, the spinner of a slow save and the `animations`
//! setting (FR-UI-11, UI-ANIM-01–03, UI-ANIM-08, ui.md §9).

use std::{fs, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{buffer::Cell, style::Color};
use taskpaper_terminal::ui::ColorSupport;

use crate::support::{Harness, line};

const OUTLINE: &str = "- a\n- b\n- c\n- d\n- e\n";

/// The screen row of each block of [`OUTLINE`], below the breadcrumb header and its rule.
const A: u16 = 2;
const E: u16 = 6;

/// The app on [`OUTLINE`] in Nord on a truecolor terminal, with `animations` set.
fn app(animations: &str) -> Harness {
    colored(animations, ColorSupport::TrueColor)
}

fn colored(animations: &str, colors: ColorSupport) -> Harness {
    let settings = format!("theme = \"nord\"\nanimations = \"{animations}\"\n");
    Harness::with_config(OUTLINE, &settings, 60, 10).with_colors(colors)
}

/// The cells of each row of a frame above the status line, which names a different temporary
/// file in each app.
fn frame(app: &mut Harness) -> Vec<Vec<Cell>> {
    let buffer = app.render().buffer();
    let area = buffer.area;
    (area.top()..area.bottom() - 1)
        .map(|y| {
            (area.left()..area.right())
                .map(|x| buffer[(x, y)].clone())
                .collect()
        })
        .collect()
}

/// The background of row `y` after the gutter.
fn background(app: &mut Harness, y: u16) -> Color {
    app.render().buffer()[(5, y)].bg
}

/// Whether the two frames differ in row `y` and only there.
fn differ_in_row(left: &[Vec<Cell>], right: &[Vec<Cell>], y: u16) -> bool {
    let differing: Vec<usize> = left
        .iter()
        .zip(right)
        .enumerate()
        .filter(|(_, (left, right))| left != right)
        .map(|(row, _)| row)
        .collect();
    differing == [usize::from(y)]
}

/// The frame of an app with animations off after `keys`.
fn still(keys: &str) -> Vec<Vec<Cell>> {
    let mut off = app("off");
    off.type_keys(keys);
    frame(&mut off)
}

fn wait(app: &mut Harness, millis: u64) {
    app.wait(Duration::from_millis(millis));
}

// FR-UI-11, UI-ANIM-01, ui.md §9.1
#[test]
fn shift_g_flashes_the_block_focus_lands_on_and_the_flash_fades_in_a_quarter_second() {
    let mut app = app("full");
    assert_eq!(app.tui.deadline(), None, "nothing moves");
    app.type_keys("G");
    assert_eq!(
        app.tui.deadline(),
        Some(Duration::from_micros(16_667)),
        "a frame is asked for to start the flash"
    );
    let landed = frame(&mut app);
    let still = still("G");
    assert!(differ_in_row(&landed, &still, E), "the landing is tinted");
    assert_eq!(
        app.tui.deadline(),
        Some(Duration::from_micros(16_667)),
        "the next frame is asked for at 60 fps"
    );
    wait(&mut app, 125);
    let fading = frame(&mut app);
    assert!(
        differ_in_row(&fading, &still, E),
        "half way it is still tinted"
    );
    assert_ne!(fading, landed, "but less");
    wait(&mut app, 125);
    assert_eq!(frame(&mut app), still, "after 250 ms it is gone");
    assert_eq!(app.tui.deadline(), None, "and nothing moves");
}

// UI-ANIM-01, ui.md §9.1
#[test]
fn moving_focus_one_block_does_not_flash() {
    let mut app = app("full");
    app.type_keys("j");
    assert_eq!(frame(&mut app), still("j"));
    assert_eq!(app.tui.deadline(), None);
    app.type_keys("x");
    assert_eq!(
        frame(&mut app),
        still("jx"),
        "nor does completing the focused block"
    );
    app.type_keys("dd");
    assert_eq!(
        frame(&mut app),
        still("jxdd"),
        "nor focus moving to the block after one deleted"
    );
    let empty = |animations: &str| {
        let settings = format!("theme = \"nord\"\nanimations = \"{animations}\"\n");
        let mut app =
            Harness::with_config("- a\n", &settings, 60, 10).with_colors(ColorSupport::TrueColor);
        app.type_keys("dd");
        frame(&mut app)
    };
    assert_eq!(
        empty("full"),
        empty("off"),
        "deleting the last block leaves no focus to flash"
    );
}

// UI-ANIM-01, ui.md §9.1
#[test]
fn zooming_in_and_focusing_another_pane_land_with_a_flash() {
    let outline = "A:\n\t- b\n- c\n";
    let start = |animations: &str| {
        let settings = format!("theme = \"nord\"\nanimations = \"{animations}\"\n");
        Harness::with_config(outline, &settings, 80, 10).with_colors(ColorSupport::TrueColor)
    };
    let (mut full, mut off) = (start("full"), start("off"));
    for app in [&mut full, &mut off] {
        app.type_keys(".");
    }
    assert_ne!(frame(&mut full), frame(&mut off), "zooming in");
    for app in [&mut full, &mut off] {
        app.press(KeyCode::Char('w'), KeyModifiers::CONTROL);
        app.type_keys("v");
        frame(app);
        wait(app, 250);
        app.type_keys("g1");
    }
    assert_ne!(frame(&mut full), frame(&mut off), "focusing the other pane");
}

// UI-ANIM-01, ui.md §9.1
#[test]
fn the_next_key_ends_a_flash_in_the_frame_it_draws() {
    let mut app = app("full");
    app.type_keys("G");
    frame(&mut app);
    wait(&mut app, 10);
    frame(&mut app);
    app.type_keys("k");
    assert_eq!(frame(&mut app), still("Gk"));
}

// UI-ANIM-01, ui.md §9.1
#[test]
fn reduced_and_sixteen_colors_flash_in_one_step_and_no_color_or_off_never_flash() {
    for (animations, colors) in [
        ("reduced", ColorSupport::TrueColor),
        ("full", ColorSupport::Ansi),
    ] {
        let mut app = colored(animations, colors);
        app.type_keys("G");
        let landed = background(&mut app, E);
        wait(&mut app, 200);
        assert_eq!(background(&mut app, E), landed, "{animations}, {colors:?}");
        wait(&mut app, 50);
        assert_ne!(background(&mut app, E), landed, "{animations}, {colors:?}");
    }
    for (animations, colors) in [
        ("full", ColorSupport::None),
        ("off", ColorSupport::TrueColor),
    ] {
        let mut app = colored(animations, colors);
        app.type_keys("G");
        let landed = background(&mut app, E);
        app.type_keys("gg");
        app.type_keys("G");
        assert_eq!(background(&mut app, E), landed, "{animations}, {colors:?}");
        assert_eq!(app.tui.deadline(), None, "{animations}, {colors:?}");
    }
}

// FR-UI-11, UI-ANIM-02, ui.md §9.1
#[test]
fn undo_highlights_the_block_it_brings_back() {
    let mut app = app("full");
    app.type_keys("dd");
    wait(&mut app, 250);
    app.type_keys("z");
    let restored = frame(&mut app);
    let still = still("ddz");
    assert!(
        differ_in_row(&restored, &still, A),
        "the block put back is tinted"
    );
    wait(&mut app, 250);
    assert_eq!(frame(&mut app), still);
}

// FR-UI-11, UI-ANIM-02, ui.md §9.1
#[test]
fn a_duplicate_highlights_the_copy_and_an_edit_of_the_focused_block_does_not() {
    let mut app = app("full");
    app.type_keys("D");
    let duplicated = frame(&mut app);
    let still_after = still("D");
    assert!(
        differ_in_row(&duplicated, &still_after, A + 1),
        "the copy is tinted"
    );
    wait(&mut app, 250);
    let mut off = colored("off", ColorSupport::TrueColor);
    off.type_keys("D");
    for app in [&mut app, &mut off] {
        app.press(KeyCode::Enter, KeyModifiers::NONE);
        app.type_keys("x");
        app.press(KeyCode::Esc, KeyModifiers::NONE);
    }
    assert_eq!(
        frame(&mut app),
        frame(&mut off),
        "typing in the focused block flashes nothing"
    );
}

// FR-UI-11, UI-ANIM-03, ui.md §9.1
#[test]
fn a_refused_key_flashes_the_focused_block_in_the_error_color() {
    let mut app = app("full");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    frame(&mut app);
    app.type_keys(" @");
    let refused = frame(&mut app);
    let mut off = colored("off", ColorSupport::TrueColor);
    off.press(KeyCode::Enter, KeyModifiers::NONE);
    off.type_keys(" @");
    let still = frame(&mut off);
    assert!(differ_in_row(&refused, &still, A), "the block is tinted");
    wait(&mut app, 150);
    assert_eq!(frame(&mut app), still, "for 150 ms");
}

/// The app with the given `animations` on [`OUTLINE`], after a change whose save started
/// `waited` ms ago and has not come back.
fn saving(animations: &str, waited: u64) -> Harness {
    let settings = format!("animations = \"{animations}\"\nautosave_delay_ms = 500\n");
    let mut app = Harness::with_config(OUTLINE, &settings, 60, 10);
    app.type_keys("dd");
    wait(&mut app, 500);
    wait(&mut app, waited);
    app
}

/// The status line's last character.
fn spinner(app: &mut Harness) -> String {
    let buffer = app.render().buffer().clone();
    let status = line(&buffer, buffer.area.bottom() - 1);
    status.chars().last().map(String::from).unwrap_or_default()
}

// UI-ANIM-08, ui.md §9.1
#[test]
fn a_save_still_running_after_300_ms_shows_a_spinner_that_turns_until_it_is_done() {
    let mut app = saving("full", 299);
    assert_eq!(spinner(&mut app), "●", "unsaved, and no spinner yet");
    assert_eq!(app.tui.deadline(), Some(Duration::from_millis(800)));
    wait(&mut app, 1);
    assert_eq!(spinner(&mut app), "⠋");
    assert_eq!(app.tui.deadline(), Some(Duration::from_millis(880)));
    wait(&mut app, 80);
    assert_eq!(spinner(&mut app), "⠙");
    assert_eq!(app.tui.deadline(), Some(Duration::from_millis(960)));
    app.settle();
    assert_eq!(spinner(&mut app), "r", "saved, and the path ends the line");
    assert_eq!(app.tui.deadline(), None);
    let mut reduced = saving("reduced", 400);
    assert_eq!(spinner(&mut reduced), "…", "a static spinner");
    assert_eq!(reduced.tui.deadline(), None, "that does not turn");
    let mut ascii = saving("full", 380).without_utf8();
    assert_eq!(spinner(&mut ascii), "/", "ASCII without UTF-8");
    let mut ascii = saving("reduced", 300).without_utf8();
    assert_eq!(spinner(&mut ascii), ".", "and its static ...");
}

// FR-UI-11, ui.md §9.3
#[test]
fn animations_off_from_go_to_command_is_written_to_the_config_file() {
    let mut app = app("full");
    app.type_keys("G");
    frame(&mut app);
    app.press(KeyCode::Char('p'), KeyModifiers::CONTROL);
    app.type_keys("animations: off");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(frame(&mut app), still("G"), "the flash running ends");
    app.type_keys("ggG");
    assert_eq!(frame(&mut app), still("G"), "and no other starts");
    assert_eq!(
        fs::read_to_string(app.config_file()).unwrap(),
        "theme = \"nord\"\nanimations = \"off\"\n"
    );
}

// FR-UI-11, FR-SET-04, ui.md §9.3
#[test]
fn an_animations_setting_that_is_not_a_level_is_reported() {
    let mut app = Harness::with_config(OUTLINE, "animations = \"slow\"\n", 120, 8);
    let status = line(app.render().buffer(), 7);
    assert_eq!(
        status,
        format!(
            "{} · config.toml: `animations` must be \"full\", \"reduced\" or \"off\"",
            app.status(120).trim_end()
        )
    );
}

/// The frames of the app on `text` in Nord on a truecolor screen 60×14, with animations full and
/// off, after `keys` and then Enter when `enter` says so.
fn full_and_off(text: &str, keys: &str, enter: bool) -> (Vec<Vec<Cell>>, Vec<Vec<Cell>>) {
    let [full, off] = ["full", "off"].map(|animations| {
        let settings = format!("theme = \"nord\"\nanimations = \"{animations}\"\n");
        let mut app =
            Harness::with_config(text, &settings, 60, 14).with_colors(ColorSupport::TrueColor);
        app.type_keys(keys);
        if enter {
            app.press(KeyCode::Enter, KeyModifiers::NONE);
        }
        frame(&mut app)
    });
    (full, off)
}

// UI-ANIM-01, D-80, ui.md §9.2
#[test]
fn opening_a_special_view_flashes_the_block_focus_lands_on_but_not_its_heading() {
    let text =
        "- Post office @start(2026-10-06)\n- Bank @due(2026-10-08)\n- Taxes @due(2026-10-10)\n";
    let (full, off) = full_and_off(text, "gU", false);
    assert!(
        differ_in_row(&full, &off, 3),
        "Tomorrow's heading is in row 2, the block in row 3"
    );
    let (full, off) = full_and_off(text, "gUG", false);
    assert!(
        differ_in_row(&full, &off, 9),
        "Saturday's heading is in row 8"
    );
}

// UI-ANIM-02, FR-MODE-07
#[test]
fn quick_add_highlights_the_block_it_adds() {
    let text = "INBOX:\n\t- old\n- loose\n";
    let (full, off) = full_and_off(text, "GqiIdea", true);
    assert!(
        differ_in_row(&full, &off, 4),
        "the block added below `old`, away from the focus"
    );
}
