//! Quick Add: adding a block to a project from a panel at the bottom (FR-MODE-07, FR-MODE-08).

use std::{fs, time::Duration};

use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::style::Modifier;

use taskpaper_terminal::ui::Input;

use crate::support::{Harness, line};

const OUTLINE: &str = "INBOX:\n\t- old\nWork:\n\tLaunch:\n\t\t- plan\nTODAY:\n- loose\n";

fn press(app: &mut Harness, code: KeyCode) {
    app.press(code, KeyModifiers::NONE);
}

/// Every screen row, without trailing blanks.
fn screen(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    (buffer.area.top()..buffer.area.bottom())
        .map(|y| line(&buffer, y))
        .collect()
}

/// The open file's text once the autosave delay has passed and its save is done.
fn saved(app: &mut Harness) -> String {
    app.wait(Duration::from_secs(1));
    app.settle();
    let path = app.path.as_ref().expect("a file was opened");
    fs::read_to_string(path).expect("the file is readable")
}

/// The screen's width: as wide as it gets without the sidebar, so the status line has room
/// for its notice.
const WIDTH: u16 = 79;

/// The status line of a screen 14 rows tall.
fn status(app: &mut Harness) -> String {
    line(app.render().buffer(), 13)
}

/// A border's top across the screen, with `title`.
fn top(title: &str) -> String {
    let rule = "─".repeat(usize::from(WIDTH) - 2 - title.chars().count());
    format!("┌{title}{rule}┐")
}

/// A row inside a border across the screen.
fn inside(text: &str) -> String {
    format!("│{text:<width$}│", width = usize::from(WIDTH) - 2)
}

fn bottom_border() -> String {
    format!("└{}┘", "─".repeat(usize::from(WIDTH) - 2))
}

// FR-MODE-07, editing.md §6, D-96, D-97
#[test]
fn q_q_adds_a_task_with_its_dates_to_the_project_picked_with_slash() {
    let mut app = Harness::with_text(OUTLINE, WIDTH, 14);
    app.type_keys("qq");
    assert_eq!(
        screen(&mut app)[9..13],
        [
            top(" Quick Add "),
            inside(" -"),
            inside(" Type the text"),
            bottom_border(),
        ]
    );
    let buffer = app.render().buffer().clone();
    let dimmed = |x, y| buffer[(x, y)].modifier.contains(Modifier::DIM);
    assert!(dimmed(4, 2) && dimmed(0, 13), "the outline and status line");
    assert!(!dimmed(0, 9) && !dimmed(2, 10), "the panel");
    app.type_keys("Call Bob tomorrow due fri");
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        screen(&mut app)[11],
        inside(" Choose a project with /"),
        "Enter needs a project"
    );
    app.type_keys(" /la");
    assert_eq!(
        screen(&mut app)[5..9],
        [
            top("Project"),
            inside("› la"),
            inside(" ▸ Launch  Work"),
            bottom_border(),
        ]
    );
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        screen(&mut app)[9..12],
        [
            top(" Quick Add → Work › Launch "),
            inside(" - Call Bob tomorrow due fri"),
            inside(" Start Tue 6 Oct · Due Fri 9 Oct"),
        ]
    );
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        screen(&mut app)[2..13],
        [
            "▌ INBOX:".to_owned(),
            "    - old".to_owned(),
            "  Work:".to_owned(),
            "    Launch:".to_owned(),
            "      - plan".to_owned(),
            format!("      - Call Bob{:>63}", "↦ tomorrow ⇥ Fri"),
            "  TODAY:".to_owned(),
            "  - loose".to_owned(),
            String::new(),
            String::new(),
            String::new(),
        ],
        "the panel closed and the focus stayed"
    );
    assert!(
        status(&mut app).ends_with("· Added to Launch"),
        "{}",
        status(&mut app)
    );
    assert_eq!(
        saved(&mut app),
        "INBOX:\n\t- old\nWork:\n\tLaunch:\n\t\t- plan\n\t\t- Call Bob @start(2026-10-06) \
         @due(2026-10-09)\nTODAY:\n- loose\n"
    );
    app.type_keys("z");
    assert_eq!(saved(&mut app), OUTLINE, "one step to undo");
}

// FR-MODE-08, editing.md §6, D-126
#[test]
fn words_naming_a_past_day_stay_in_the_text_unless_due_comes_before_them() {
    let mut app = Harness::with_text(OUTLINE, WIDTH, 14);
    app.type_keys("qiReview last week sales due yesterday");
    assert_eq!(screen(&mut app)[11], inside(" Due Sun 4 Oct"));
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        saved(&mut app),
        "INBOX:\n\t- old\n\t- Review last week sales @due(2026-10-04)\nWork:\n\tLaunch:\n\t\t- \
         plan\nTODAY:\n- loose\n"
    );
}

// FR-MODE-07, FR-MODE-08, editing.md §6, D-96, D-98
#[test]
fn q_i_and_q_t_start_at_the_inbox_and_today_and_backspace_makes_a_note() {
    let mut app = Harness::with_text(OUTLINE, WIDTH, 14);
    app.type_keys("qi");
    assert_eq!(screen(&mut app)[9], top(" Quick Add → INBOX "));
    press(&mut app, KeyCode::Backspace);
    app.type_keys("Idea");
    assert_eq!(screen(&mut app)[10], inside(" Idea"));
    press(&mut app, KeyCode::Enter);
    assert!(status(&mut app).ends_with("· Added to INBOX"));
    app.type_keys("qtx");
    press(&mut app, KeyCode::Esc);
    assert_eq!(screen(&mut app)[12], "", "Esc closed the panel");
    app.type_keys("qty");
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        saved(&mut app),
        "INBOX:\n\t- old\n\tIdea\nWork:\n\tLaunch:\n\t\t- plan\nTODAY:\n\t- y\n- loose\n"
    );
}

// FR-MODE-08, D-78, D-98
#[test]
fn q_i_and_q_t_say_when_their_project_is_missing_or_turned_off() {
    let mut app = Harness::with_config(OUTLINE, "special_today = false\n", WIDTH, 14);
    app.type_keys("qt");
    assert_eq!(screen(&mut app)[9], "", "nothing opened");
    assert!(status(&mut app).ends_with("· No TODAY project"));
    let mut app = Harness::with_text("- loose\n", WIDTH, 14);
    app.type_keys("qi");
    let said = status(&mut app);
    assert!(said.contains("· No INBOX project"), "{said}");
    app.type_keys("qt");
    assert!(status(&mut app).ends_with("· No TODAY project"));
}

// editing.md §6, keybindings.md §4: the panel takes the standard text keys, pasting included.
#[test]
fn text_pasted_into_the_terminal_or_with_ctrl_v_is_typed_into_the_panel() {
    let mut app = Harness::with_text(OUTLINE, WIDTH, 14);
    app.type_keys("jyqi");
    app.send(Input::Terminal(Event::Paste(
        "Renew\npassport\n".to_owned(),
    )));
    assert_eq!(screen(&mut app)[10], inside(" - Renew passport"));
    app.type_keys(" ");
    app.press(KeyCode::Char('v'), KeyModifiers::CONTROL);
    assert_eq!(
        screen(&mut app)[10],
        inside(" - Renew passport old"),
        "the text of the block y copied"
    );
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        saved(&mut app),
        "INBOX:\n\t- old\n\t- Renew passport old\nWork:\n\tLaunch:\n\t\t- plan\nTODAY:\n- loose\n"
    );
}
