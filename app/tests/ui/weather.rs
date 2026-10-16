//! The weather: after the app sits idle, leaves, snow or rain by season fall over the whole
//! screen until the next input, which does nothing else; a setting turns it off and Go to
//! Command starts it at once (FR-UI-12, D-129, ui.md §10). The tests run in October: autumn.

use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::buffer::Buffer;
use taskpaper_terminal::ui::Input;

use crate::support::{Harness, focused_rows};

const OUTLINE: &str = "- a\n- b\n- c\n";

const MINUTE: Duration = Duration::from_mins(1);

/// The idle time by default.
const IDLE: Duration = Duration::from_mins(30);

/// The app on [`OUTLINE`] with `settings` and full animations unless `settings` says otherwise.
fn app(settings: &str) -> Harness {
    Harness::with_config(
        OUTLINE,
        &format!("animations = \"full\"\n{settings}"),
        40,
        12,
    )
}

fn screen(app: &mut Harness) -> Buffer {
    app.render().buffer().clone()
}

/// Lets `duration` pass a frame at a time, as the event loop does while the weather plays.
fn wait_frames(app: &mut Harness, duration: Duration) {
    let frame = Duration::from_millis(50);
    let mut waited = Duration::ZERO;
    while waited < duration {
        app.wait(frame);
        waited += frame;
    }
}

// FR-UI-12, ui.md §10.1, §10.2
#[test]
fn after_thirty_idle_minutes_leaves_fall_over_the_screen() {
    let mut app = app("");
    let still = screen(&mut app);
    assert_eq!(app.tui.deadline(), Some(IDLE), "the idle timer");
    app.wait(29 * MINUTE + Duration::from_secs(59));
    assert_eq!(screen(&mut app), still, "not yet");
    app.wait(Duration::from_secs(1));
    assert_eq!(screen(&mut app), still, "the sky starts empty");
    wait_frames(&mut app, Duration::from_secs(5));
    assert_ne!(screen(&mut app), still, "leaves fall");
    assert!(app.tui.deadline().is_some(), "and keep falling");
}

// ui.md §10.1
#[test]
fn the_key_that_ends_the_weather_does_nothing_else() {
    let mut app = app("");
    let still = screen(&mut app);
    app.wait(IDLE);
    wait_frames(&mut app, Duration::from_secs(5));
    app.press(KeyCode::Char('j'), KeyModifiers::NONE);
    assert_eq!(screen(&mut app), still, "the app as it was");
    assert_eq!(focused_rows(&mut app), ["- a"], "focus did not move");
    app.press(KeyCode::Char('j'), KeyModifiers::NONE);
    assert_eq!(focused_rows(&mut app), ["- b"], "the next key acts");
}

// ui.md §10.1
#[test]
fn a_click_a_paste_and_a_resize_end_it_too_and_restart_the_idle_timer() {
    let mut app = app("");
    let resize = Input::Terminal(Event::Resize(40, 12));
    let paste = Input::Terminal(Event::Paste("x".into()));
    let mut now = Duration::ZERO;
    for input in [resize, paste] {
        app.wait(IDLE);
        wait_frames(&mut app, Duration::from_secs(1));
        now += IDLE + Duration::from_secs(1);
        assert!(
            app.tui.deadline() < Some(now + Duration::from_secs(1)),
            "it plays"
        );
        app.send(input);
        assert_eq!(app.tui.deadline(), Some(now + IDLE));
    }
    app.wait(IDLE);
    app.click(10, 3);
    assert_eq!(focused_rows(&mut app), ["- a"], "the click focused nothing");
}

// ui.md §10.1
#[test]
fn focus_changes_and_disk_results_neither_end_it_nor_restart_the_timer() {
    let mut app = app("");
    app.wait(29 * MINUTE);
    app.send(Input::Terminal(Event::FocusLost));
    app.settle();
    assert_eq!(app.tui.deadline(), Some(IDLE));
    app.wait(MINUTE);
    app.send(Input::Terminal(Event::FocusGained));
    app.settle();
    assert!(
        app.tui.deadline() < Some(IDLE + Duration::from_secs(1)),
        "it plays"
    );
}

// FR-UI-12, ui.md §10.1, N-PERF-03
#[test]
fn the_setting_off_reduced_animations_and_no_animations_keep_it_from_starting() {
    for settings in [
        "weather = false\n",
        "animations = \"reduced\"\n",
        "animations = \"off\"\n",
    ] {
        let mut app = Harness::with_config(
            OUTLINE,
            &format!(
                "{}{settings}",
                if settings.starts_with("animations") {
                    ""
                } else {
                    "animations = \"full\"\n"
                }
            ),
            40,
            12,
        );
        let still = screen(&mut app);
        assert_eq!(app.tui.deadline(), None, "{settings}");
        app.wait(2 * IDLE);
        assert_eq!(screen(&mut app), still, "{settings}");
    }
}

// ui.md §10.1
#[test]
fn the_idle_time_is_a_setting_in_minutes() {
    let app = app("weather_idle_minutes = 1\n");
    assert_eq!(app.tui.deadline(), Some(MINUTE));
}

// FR-UI-12, ui.md §10.1
#[test]
fn start_weather_animation_starts_it_at_once_even_with_the_setting_off() {
    let mut app = Harness::with_config(OUTLINE, "weather = false\n", 40, 12);
    let still = screen(&mut app);
    app.press(KeyCode::Char('p'), KeyModifiers::CONTROL);
    app.type_keys("start weather");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        screen(&mut app),
        still,
        "the palette closed and the sky is empty"
    );
    wait_frames(&mut app, Duration::from_secs(5));
    assert_ne!(screen(&mut app), still);
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(screen(&mut app), still);
}

// ui.md §10.1
#[test]
fn in_summer_the_command_says_there_is_no_weather() {
    let mut app = Harness::with_config(OUTLINE, "weather = false\n", 60, 12);
    app.wait(Duration::from_hours(270 * 24));
    app.press(KeyCode::Char('p'), KeyModifiers::CONTROL);
    app.type_keys("start weather");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    let buffer = screen(&mut app);
    let status = crate::support::line(&buffer, 11);
    assert!(status.ends_with("No weather in summer"), "{status}");
}
