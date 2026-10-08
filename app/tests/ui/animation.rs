//! Step 7.1: the landing flash where focus jumps, the highlight of what an undo, a paste or a
//! reload changed, the flash of a refused key, the spinner of a slow save and the `animations`
//! setting (FR-UI-11, UI-ANIM-01–03, UI-ANIM-08, ui.md §9). Step 7.2: the ease of a completed
//! block, the fade-in of a new root and of expanded rows, the fade-out of a status message and
//! the crossfade between previewed themes (UI-ANIM-04–07, UI-ANIM-09).

use std::{fs, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    buffer::Cell,
    style::{Color, Modifier},
};
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
    frame(&mut app);
    wait(&mut app, 200);
    assert_eq!(
        frame(&mut app),
        still("jx"),
        "nor does completing the focused block, once its text has eased"
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
/// off, after `act`.
fn full_and_off(text: &str, act: impl Fn(&mut Harness)) -> (Vec<Vec<Cell>>, Vec<Vec<Cell>>) {
    let [full, off] = ["full", "off"].map(|animations| {
        let settings = format!("theme = \"nord\"\nanimations = \"{animations}\"\n");
        let mut app =
            Harness::with_config(text, &settings, 60, 14).with_colors(ColorSupport::TrueColor);
        act(&mut app);
        frame(&mut app)
    });
    (full, off)
}

// UI-ANIM-01, D-80, ui.md §9.2
#[test]
fn a_jump_in_a_special_view_flashes_the_block_focus_lands_on_but_not_its_heading() {
    let text =
        "- Post office @start(2026-10-06)\n- Bank @due(2026-10-08)\n- Taxes @due(2026-10-10)\n";
    let last = |app: &mut Harness| {
        app.type_keys("gU");
        frame(app);
        wait(app, 250);
        app.type_keys("G");
    };
    let (full, off) = full_and_off(text, last);
    assert!(
        differ_in_row(&full, &off, 9),
        "Saturday's heading is in row 8"
    );
    let (full, off) = full_and_off(text, |app| {
        last(app);
        frame(app);
        wait(app, 250);
        app.type_keys("gg");
    });
    assert!(
        differ_in_row(&full, &off, 3),
        "Tomorrow's heading is in row 2, the block in row 3"
    );
}

// UI-ANIM-02, FR-MODE-07
#[test]
fn quick_add_highlights_the_block_it_adds() {
    let text = "INBOX:\n\t- old\n- loose\n";
    let (full, off) = full_and_off(text, |app| {
        app.type_keys("GqiIdea");
        app.press(KeyCode::Enter, KeyModifiers::NONE);
    });
    assert!(
        differ_in_row(&full, &off, 4),
        "the block added below `old`, away from the focus"
    );
}

/// The rows in which the two frames differ.
fn differing(left: &[Vec<Cell>], right: &[Vec<Cell>]) -> Vec<usize> {
    left.iter()
        .zip(right)
        .enumerate()
        .filter(|(_, (left, right))| left != right)
        .map(|(row, _)| row)
        .collect()
}

/// The app on `text` in Nord on a truecolor screen 60×10 with `animations`.
fn on(text: &str, animations: &str) -> Harness {
    let settings = format!("theme = \"nord\"\nanimations = \"{animations}\"\n");
    Harness::with_config(text, &settings, 60, 10).with_colors(ColorSupport::TrueColor)
}

/// The apps on `text` with animations full and off, each after `keys`, a frame and `millis`.
fn both(text: &str, keys: &str, millis: u64) -> (Harness, Harness) {
    let [mut full, mut off] = ["full", "off"].map(|animations| on(text, animations));
    for app in [&mut full, &mut off] {
        app.type_keys(keys);
        frame(app);
        wait(app, millis);
    }
    (full, off)
}

/// How far apart two RGB colors are, summed over their channels.
fn distance(left: Color, right: Color) -> u32 {
    match (left, right) {
        (Color::Rgb(red, green, blue), Color::Rgb(other_red, other_green, other_blue)) => [
            red.abs_diff(other_red),
            green.abs_diff(other_green),
            blue.abs_diff(other_blue),
        ]
        .into_iter()
        .map(u32::from)
        .sum(),
        _ => u32::MAX,
    }
}

// FR-UI-11, UI-ANIM-04, ui.md §9.2
#[test]
fn completing_a_project_eases_its_text_and_the_text_inside_it_into_the_done_style() {
    let text = "P:\n\t- a\n- b\n";
    let (mut full, mut off) = both(text, "", 0);
    for app in [&mut full, &mut off] {
        app.type_keys("x");
    }
    let (eased, done) = (frame(&mut full), frame(&mut off));
    assert_eq!(differing(&eased, &done), [2, 3], "P and the task inside it");
    let cell = &eased[2][2];
    assert!(
        !cell.modifier.contains(Modifier::DIM) && cell.modifier.contains(Modifier::CROSSED_OUT),
        "struck through at once, its dimming still to come: {cell:?}"
    );
    wait(&mut full, 100);
    let easing = frame(&mut full);
    assert_eq!(differing(&easing, &done), [2, 3]);
    assert_ne!(easing, eased, "on its way");
    wait(&mut full, 100);
    assert_eq!(frame(&mut full), done, "done after 200 ms");
    for app in [&mut full, &mut off] {
        app.type_keys("x");
    }
    let (eased, undone) = (frame(&mut full), frame(&mut off));
    assert_eq!(
        differing(&eased, &undone),
        [2, 3],
        "uncompleting eases back"
    );
    wait(&mut full, 200);
    assert_eq!(frame(&mut full), undone);
    let mut reduced = on(text, "reduced");
    reduced.type_keys("x");
    assert_eq!(frame(&mut reduced), done, "reduced keeps only the flashes");
}

// FR-UI-11, UI-ANIM-05, ui.md §9.2
#[test]
fn zooming_in_fades_the_pane_in_from_the_top_and_flashes_the_new_crumb() {
    let text = "A:\n\t- b\n\t- c\n\t- d\n- e\n";
    let (mut full, mut off) = both(text, ".", 30);
    let (fading, zoomed) = (frame(&mut full), frame(&mut off));
    assert_eq!(
        differing(&fading, &zoomed),
        [0, 2, 3, 4, 5],
        "the crumb and every row with text"
    );
    let text_fg = |frame: &[Vec<Cell>], row: usize| {
        frame[row]
            .iter()
            .find(|cell| cell.symbol() != " " && cell.symbol() != "▌")
            .map(|cell| cell.fg)
            .unwrap()
    };
    assert!(
        distance(text_fg(&fading, 3), text_fg(&zoomed, 3))
            < distance(text_fg(&fading, 5), text_fg(&zoomed, 5)),
        "the top is further in than the rows below it"
    );
    wait(&mut full, 100);
    let flashing = frame(&mut full);
    assert_eq!(
        differing(&flashing, &zoomed),
        [0, 3],
        "after 120 ms only the crumb and the landing flash on b are left"
    );
    let header: String = zoomed[0].iter().map(Cell::symbol).collect();
    let crumb = header.chars().position(|symbol| symbol == 'A').unwrap();
    let columns: Vec<usize> = (0..zoomed[0].len())
        .filter(|&column| flashing[0][column] != zoomed[0][column])
        .collect();
    assert_eq!(columns, [crumb], "the flash covers the new crumb alone");
    wait(&mut full, 120);
    assert_eq!(frame(&mut full), zoomed);
    let mut reduced = on(text, "reduced");
    reduced.type_keys(".");
    assert_eq!(
        differing(&frame(&mut reduced), &zoomed),
        [3],
        "reduced keeps only the landing flash"
    );
}

// FR-UI-11, UI-ANIM-06, ui.md §9.2
#[test]
fn expanding_a_block_fades_in_the_rows_it_shows() {
    let text = "A:\n\t- b\n\t- c\n- d\n";
    let (mut full, mut off) = both(text, "h", 250);
    for app in [&mut full, &mut off] {
        app.type_keys("l");
    }
    let expanded = frame(&mut off);
    assert_eq!(differing(&frame(&mut full), &expanded), [3, 4], "b and c");
    wait(&mut full, 120);
    assert_eq!(frame(&mut full), expanded);
}

/// The status line of an app 10 rows tall.
fn status(app: &mut Harness) -> String {
    line(app.render().buffer(), 9)
}

/// The app on a line in `colors` with `animations`, wide enough for the status line to show a
/// notice.
fn wide(animations: &str, colors: ColorSupport) -> Harness {
    let settings = format!("theme = \"nord\"\nanimations = \"{animations}\"\n");
    Harness::with_config("- a\n", &settings, 120, 10).with_colors(colors)
}

// FR-UI-11, UI-ANIM-07, ui.md §9.2
#[test]
fn a_status_message_fades_out_when_its_time_is_up() {
    let mut app = wide("full", ColorSupport::TrueColor);
    app.type_keys("gi");
    assert!(
        status(&mut app).ends_with("· No INBOX project in the Navigator"),
        "{}",
        status(&mut app)
    );
    let shown = app.render().buffer().clone();
    wait(&mut app, 10_000);
    assert!(
        status(&mut app).ends_with("· No INBOX project in the Navigator"),
        "kept while it fades"
    );
    assert_eq!(
        app.tui.deadline(),
        Some(Duration::from_millis(10_000) + Duration::from_micros(16_667))
    );
    wait(&mut app, 150);
    let fading = app.render().buffer().clone();
    let x = u16::try_from(status(&mut app).chars().count() - 2).unwrap();
    assert!(
        distance(fading[(x, 9)].fg, fading[(x, 9)].bg)
            < distance(shown[(x, 9)].fg, shown[(x, 9)].bg),
        "nearer the background half way"
    );
    wait(&mut app, 150);
    assert!(!status(&mut app).contains("No INBOX"), "gone after 300 ms");
    assert_eq!(app.tui.deadline(), None);
    for (animations, colors) in [
        ("reduced", ColorSupport::TrueColor),
        ("full", ColorSupport::Ansi),
    ] {
        let mut app = wide(animations, colors);
        app.type_keys("gi");
        wait(&mut app, 10_000);
        assert!(
            !status(&mut app).contains("No INBOX"),
            "it vanishes: {animations}, {colors:?}"
        );
        assert_eq!(app.tui.deadline(), None, "{animations}, {colors:?}");
    }
}

// FR-UI-11, UI-ANIM-09, FR-UI-06, ui.md §9.2
#[test]
fn moving_in_the_theme_picker_crossfades_to_the_previewed_theme() {
    let (mut full, mut off) = both("- a\n", "", 0);
    for app in [&mut full, &mut off] {
        app.press(KeyCode::Char('t'), KeyModifiers::CONTROL);
        frame(app);
        app.press(KeyCode::Down, KeyModifiers::NONE);
    }
    let previewed = frame(&mut off);
    let crossfading = frame(&mut full);
    assert_ne!(crossfading, previewed, "the old theme's colors linger");
    wait(&mut full, 150);
    assert_eq!(frame(&mut full), previewed, "for 150 ms");
    for app in [&mut full, &mut off] {
        app.press(KeyCode::Esc, KeyModifiers::NONE);
    }
    let back = frame(&mut off);
    assert_ne!(frame(&mut full), back, "Esc fades back");
    wait(&mut full, 150);
    assert_eq!(frame(&mut full), back);
    let mut ansi = colored("full", ColorSupport::Ansi);
    let mut plain = colored("off", ColorSupport::Ansi);
    for app in [&mut ansi, &mut plain] {
        app.press(KeyCode::Char('t'), KeyModifiers::CONTROL);
        frame(app);
        app.press(KeyCode::Down, KeyModifiers::NONE);
    }
    assert_eq!(
        frame(&mut ansi),
        frame(&mut plain),
        "16 colors switch at once"
    );
}

// FR-UI-11, UI-ANIM-09, FR-SET-02, ui.md §9.2
#[test]
fn cycling_the_theme_in_the_settings_window_crossfades_to_it() {
    let (mut full, mut off) = both("- a\n", "g,", 250);
    for app in [&mut full, &mut off] {
        app.type_keys("l");
    }
    // Each app writes its own settings file, whose path the window shows, so only the colors
    // compare.
    let colors = |frame: Vec<Vec<Cell>>| -> Vec<(Color, Color)> {
        frame
            .iter()
            .flatten()
            .map(|cell| (cell.fg, cell.bg))
            .collect()
    };
    let cycled = colors(frame(&mut off));
    assert_ne!(
        colors(frame(&mut full)),
        cycled,
        "the old theme's colors linger"
    );
    wait(&mut full, 150);
    assert_eq!(colors(frame(&mut full)), cycled, "for 150 ms");
}

// FR-UI-11, UI-ANIM-06, FR-KEY-05
#[test]
fn a_click_on_the_fold_marker_of_another_pane_fades_in_the_rows_it_shows_there() {
    let (mut full, mut off) = both("A:\n\t- b\n- c\n", "h", 250);
    for app in [&mut full, &mut off] {
        app.press(KeyCode::Char('w'), KeyModifiers::CONTROL);
        app.type_keys("v");
        frame(app);
        wait(app, 250);
        app.click(5, 2);
    }
    let expanded = frame(&mut off);
    assert_eq!(
        differing(&frame(&mut full), &expanded),
        [2, 3],
        "the landing flash on A in the left pane and b fading in under it"
    );
    wait(&mut full, 120);
    assert_eq!(differing(&frame(&mut full), &expanded), [2]);
}
