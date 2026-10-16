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

// FR-MODE-07, editing.md §6, D-96, D-97, D-133
#[test]
fn q_adds_a_task_with_its_dates_to_the_inbox() {
    let mut app = Harness::with_text(OUTLINE, WIDTH, 14);
    app.type_keys("q");
    assert_eq!(
        screen(&mut app)[9..13],
        [
            top(" Quick Add → INBOX "),
            inside(" -"),
            inside(" Type the text"),
            bottom_border(),
        ]
    );
    let buffer = app.render().buffer().clone();
    let dimmed = |x, y| buffer[(x, y)].modifier.contains(Modifier::DIM);
    assert!(dimmed(4, 2) && dimmed(0, 13), "the outline and status line");
    assert!(!dimmed(0, 9) && !dimmed(2, 10), "the panel");
    app.type_keys("tomorrow");
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        screen(&mut app)[11],
        inside(" Type the text"),
        "Enter needs text besides the dates"
    );
    app.type_keys(" call Bob/Ann due fri");
    assert_eq!(
        screen(&mut app)[9..12],
        [
            top(" Quick Add → INBOX "),
            inside(" - tomorrow call Bob/Ann due fri"),
            inside(" Start Tue 6 Oct · Due Fri 9 Oct"),
        ]
    );
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        screen(&mut app)[2..13],
        [
            "▌ INBOX:".to_owned(),
            "    - old".to_owned(),
            format!("    - call Bob/Ann{:>61}", "↦ tomorrow ⇥ Fri"),
            "  Work:".to_owned(),
            "    Launch:".to_owned(),
            "      - plan".to_owned(),
            "  TODAY:".to_owned(),
            "  - loose".to_owned(),
            String::new(),
            String::new(),
            String::new(),
        ],
        "the panel closed and the focus stayed"
    );
    assert!(
        status(&mut app).ends_with("· Added to INBOX"),
        "{}",
        status(&mut app)
    );
    assert_eq!(
        saved(&mut app),
        "INBOX:\n\t- old\n\t- call Bob/Ann @start(2026-10-06) @due(2026-10-09)\nWork:\n\tLaunch:\
         \n\t\t- plan\nTODAY:\n- loose\n"
    );
    app.type_keys("z");
    assert_eq!(saved(&mut app), OUTLINE, "one step to undo");
}

// FR-MODE-08, editing.md §6, D-126
#[test]
fn words_naming_a_past_day_stay_in_the_text_unless_due_comes_before_them() {
    let mut app = Harness::with_text(OUTLINE, WIDTH, 14);
    app.type_keys("qReview last week sales due yesterday");
    assert_eq!(screen(&mut app)[11], inside(" Due Sun 4 Oct"));
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        saved(&mut app),
        "INBOX:\n\t- old\n\t- Review last week sales @due(2026-10-04)\nWork:\n\tLaunch:\n\t\t- \
         plan\nTODAY:\n- loose\n"
    );
}

// FR-MODE-07, editing.md §6, D-96
#[test]
fn backspace_at_the_start_makes_a_note_and_esc_adds_nothing() {
    let mut app = Harness::with_text(OUTLINE, WIDTH, 14);
    app.type_keys("q");
    press(&mut app, KeyCode::Backspace);
    app.type_keys("Idea");
    assert_eq!(screen(&mut app)[10], inside(" Idea"));
    press(&mut app, KeyCode::Enter);
    assert!(status(&mut app).ends_with("· Added to INBOX"));
    app.type_keys("qx");
    press(&mut app, KeyCode::Esc);
    assert_eq!(screen(&mut app)[12], "", "Esc closed the panel");
    assert_eq!(
        saved(&mut app),
        "INBOX:\n\t- old\n\tIdea\nWork:\n\tLaunch:\n\t\t- plan\nTODAY:\n- loose\n"
    );
}

// FR-MODE-07, D-78, D-133
#[test]
fn q_says_when_there_is_no_inbox_or_it_is_turned_off() {
    for (outline, config) in [
        (OUTLINE, "special_inbox = false\n"),
        ("TODAY:\n- loose\n", ""),
    ] {
        let mut app = Harness::with_config(outline, config, WIDTH, 14);
        app.type_keys("q");
        assert_eq!(screen(&mut app)[9], "", "{outline:?}: nothing opened");
        let said = status(&mut app);
        assert!(
            said.ends_with("· No INBOX project in the Navigator"),
            "{said}"
        );
    }
}

// editing.md §6, keybindings.md §4: the panel takes the standard text keys, pasting included.
#[test]
fn text_pasted_into_the_terminal_or_with_ctrl_v_is_typed_into_the_panel() {
    let mut app = Harness::with_text(OUTLINE, WIDTH, 14);
    app.type_keys("jyq");
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
