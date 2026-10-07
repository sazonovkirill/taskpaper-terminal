//! Step 4.6: the navigation sidebar on the left, its keys and the 80-column rule.

use std::fmt::Write as _;

use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::buffer::Buffer;
use taskpaper_terminal::ui::Input;

use crate::support::{BOOKMARK, Harness, NAVIGATE, STAR, line};

const OUTLINE: &str = "\
Work:
\t- Launch plan @bookmark
\tReports:
\t\t- Draft
Home:
\t- Water plants
";

/// The sidebar's width on an 80-column screen, its rule included.
const SIDEBAR: u16 = 24;

fn app(width: u16) -> Harness {
    let mut app = Harness::with_text(OUTLINE, width, 10);
    app.render();
    app
}

/// The text of row `y` from column `from` to `to`, without trailing blanks.
fn columns(buffer: &Buffer, y: u16, from: u16, to: u16) -> String {
    let mut text = String::new();
    for x in from..to {
        let _ = write!(text, "{}", buffer[(x, y)].symbol());
    }
    text.trim_end().to_owned()
}

/// The sidebar's lines as drawn, gutter included, without its rule.
fn sidebar(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    let mut lines: Vec<String> = (0..buffer.area.height - 1)
        .map(|y| columns(&buffer, y, 0, SIDEBAR - 1))
        .collect();
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}

/// The pane's rows right of the sidebar, gutter included, header and rule first.
fn pane(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    (0..buffer.area.height - 1)
        .map(|y| columns(&buffer, y, SIDEBAR, buffer.area.width))
        .collect()
}

fn press(app: &mut Harness, code: KeyCode) {
    app.press(code, KeyModifiers::NONE);
}

fn alt_0(app: &mut Harness) {
    app.press(KeyCode::Char('0'), KeyModifiers::ALT);
}

fn ctrl_b(app: &mut Harness) {
    app.press(KeyCode::Char('b'), KeyModifiers::CONTROL);
}

/// The sidebar's lines as listed before any key, `▌` marking the focused one.
fn listed(focused: Option<usize>) -> Vec<String> {
    [
        "Bookmarks".to_owned(),
        format!("{BOOKMARK} Launch plan"),
        format!("{STAR} Today"),
        String::new(),
        "Projects".to_owned(),
        "▾Work".to_owned(),
        "  Reports".to_owned(),
        "Home".to_owned(),
    ]
    .into_iter()
    .enumerate()
    .map(|(at, text)| {
        // A top-level project's fold marker stands in the gutter's second column (D-61).
        let gutter = match (Some(at) == focused, text.starts_with(['▾', '▸'])) {
            (true, false) => "▌ ",
            (true, true) => "▌",
            (false, false) => "  ",
            (false, true) => " ",
        };
        format!("{gutter}{text}").trim_end().to_owned()
    })
    .collect()
}

// FR-UI-01, ui.md §1, navigation §6
#[test]
fn the_sidebar_lists_bookmarks_today_and_projects_left_of_the_pane() {
    let mut app = app(80);
    assert_eq!(sidebar(&mut app), listed(None));
    let buffer = app.render().buffer().clone();
    assert_eq!(
        buffer[(SIDEBAR - 1, 0)].symbol(),
        "│",
        "a rule on its right edge"
    );
    assert_eq!(
        pane(&mut app)[..4],
        [
            "  ⌂",
            "─".repeat(56).as_str(),
            "▌ Work:",
            &format!("    - Launch plan {BOOKMARK}")
        ]
    );
    assert!(
        line(&buffer, 9).starts_with(&format!("{NAVIGATE} · ")),
        "the status line spans the screen"
    );
}

// FR-UI-01, ui.md §1
#[test]
fn below_80_columns_the_sidebar_hides_and_cannot_take_the_keys() {
    let mut app = app(79);
    assert_eq!(line(app.render().buffer(), 0), "  ⌂");
    alt_0(&mut app);
    press(&mut app, KeyCode::Char('j'));
    let buffer = app.render().buffer().clone();
    assert_eq!(
        line(&buffer, 3),
        format!("▌   - Launch plan {BOOKMARK}"),
        "j moved the pane's focus"
    );
    assert_eq!(line(&buffer, 0), "  ⌂", "still hidden");
}

// FR-UI-01
#[test]
fn ctrl_b_hides_and_shows_the_sidebar() {
    let mut app = app(80);
    ctrl_b(&mut app);
    assert_eq!(
        line(app.render().buffer(), 2),
        "▌ Work:",
        "the pane takes the screen"
    );
    ctrl_b(&mut app);
    assert_eq!(sidebar(&mut app), listed(None));
}

// FR-UI-01, keybindings.md §2, §4, D-42
#[test]
fn alt_0_and_g_0_focus_the_sidebar_where_j_k_and_the_arrows_move() {
    let mut app = app(80);
    alt_0(&mut app);
    assert_eq!(sidebar(&mut app), listed(Some(1)));
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(sidebar(&mut app), listed(Some(2)));
    press(&mut app, KeyCode::Down);
    assert_eq!(
        sidebar(&mut app),
        listed(Some(5)),
        "headings and blank lines are skipped"
    );
    press(&mut app, KeyCode::Char('k'));
    press(&mut app, KeyCode::Up);
    assert_eq!(sidebar(&mut app), listed(Some(1)));
    assert_eq!(pane(&mut app)[2], "▌ Work:", "the pane's focus stays");
    press(&mut app, KeyCode::Esc);
    assert_eq!(sidebar(&mut app), listed(None));
    app.type_keys("g0");
    assert_eq!(sidebar(&mut app), listed(Some(1)));
}

// FR-UI-02, keybindings.md §4, D-42
#[test]
fn g_1_and_alt_1_give_the_keys_from_the_sidebar_to_the_pane() {
    let mut app = app(80);
    app.type_keys("g0");
    app.type_keys("g1");
    assert_eq!(sidebar(&mut app), listed(None), "the sidebar lets go");
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(
        pane(&mut app)[3],
        "▌   - Launch plan ⚑".replace('⚑', BOOKMARK),
        "j moves in the pane"
    );
    alt_0(&mut app);
    app.press(KeyCode::Char('1'), KeyModifiers::ALT);
    assert_eq!(sidebar(&mut app), listed(None));
}

// FR-UI-01
#[test]
fn alt_0_shows_a_hidden_sidebar() {
    let mut app = app(80);
    ctrl_b(&mut app);
    alt_0(&mut app);
    assert_eq!(sidebar(&mut app), listed(Some(1)));
}

// FR-UI-01, navigation §6
#[test]
fn enter_zooms_the_pane_to_the_entry_and_gives_it_the_keys() {
    let mut app = app(80);
    alt_0(&mut app);
    press(&mut app, KeyCode::Enter);
    assert_eq!(pane(&mut app)[0], "  ⌂ › Work › Launch plan");
    assert_eq!(sidebar(&mut app), listed(None));
    app.type_keys("g0jjj");
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        pane(&mut app)[..4],
        [
            "  ⌂ › Work › Reports",
            "─".repeat(56).as_str(),
            "  Reports:",
            "▌   - Draft"
        ]
    );
    press(&mut app, KeyCode::Char('k'));
    assert_eq!(pane(&mut app)[2], "▌ Reports:", "k moves in the pane");
}

// D-7, navigation §6, §7
#[test]
fn enter_on_today_shows_the_today_view_in_the_pane() {
    let mut app = app(80);
    alt_0(&mut app);
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Enter);
    assert_eq!(pane(&mut app)[0], format!("  {STAR} Today"));
    assert_eq!(sidebar(&mut app), listed(None));
}

// keybindings.md §4
#[test]
fn h_and_l_and_the_arrows_fold_project_entries() {
    let mut app = app(80);
    alt_0(&mut app);
    app.type_keys("jjh");
    let mut folded = listed(Some(5));
    folded[5] = "▌▸Work".to_owned();
    folded.remove(6);
    assert_eq!(sidebar(&mut app), folded);
    press(&mut app, KeyCode::Char('l'));
    assert_eq!(sidebar(&mut app), listed(Some(5)));
    press(&mut app, KeyCode::Left);
    assert_eq!(sidebar(&mut app), folded);
    press(&mut app, KeyCode::Right);
    assert_eq!(sidebar(&mut app), listed(Some(5)));
    assert_eq!(
        pane(&mut app)[3],
        format!("    - Launch plan {BOOKMARK}"),
        "the outline keeps its folds"
    );
}

// FR-UI-01
#[test]
fn ctrl_b_in_the_sidebar_hides_it_and_gives_the_pane_the_keys() {
    let mut app = app(80);
    alt_0(&mut app);
    ctrl_b(&mut app);
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(
        line(app.render().buffer(), 3),
        format!("▌   - Launch plan {BOOKMARK}")
    );
}

#[test]
fn ctrl_q_quits_from_the_sidebar() {
    let mut app = app(80);
    alt_0(&mut app);
    app.press(KeyCode::Char('q'), KeyModifiers::CONTROL);
    assert!(app.tui.should_quit());
}

// FR-OPS-06
#[test]
fn text_pasted_into_the_focused_sidebar_is_dropped() {
    let mut app = app(80);
    alt_0(&mut app);
    app.send(Input::Terminal(Event::Paste("- Eggs".to_owned())));
    assert_eq!(pane(&mut app)[3], format!("    - Launch plan {BOOKMARK}"));
}

// FR-UI-01, ui.md §1
#[test]
fn a_screen_narrowed_below_80_columns_gives_the_keys_back_to_the_pane() {
    let mut app = app(80);
    alt_0(&mut app);
    app.terminal.backend_mut().resize(60, 10);
    app.render();
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(
        line(app.render().buffer(), 3),
        format!("▌   - Launch plan {BOOKMARK}")
    );
    app.terminal.backend_mut().resize(80, 10);
    assert_eq!(sidebar(&mut app), listed(None));
}

// FR-UI-01
#[test]
fn the_sidebar_scrolls_to_keep_its_focused_entry_visible() {
    let projects = (1..=12)
        .map(|n| format!("P{n}:"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut app = Harness::with_text(&projects, 80, 6);
    app.render();
    assert_eq!(
        sidebar(&mut app)[..3],
        [
            format!("  {STAR} Today"),
            String::new(),
            "  Projects".to_owned()
        ]
    );
    alt_0(&mut app);
    app.type_keys("jjjjj");
    assert_eq!(sidebar(&mut app), ["  P3", "  P4", "▌ P5", "  P6", "  P7"]);
}

// D-54, ui.md §1
#[test]
fn the_date_prompt_spans_the_screen_below_the_sidebar() {
    let mut app = app(80);
    app.type_keys("md");
    let buffer = app.render().buffer().clone();
    assert!(
        line(&buffer, 5).starts_with("┌ Due date ─"),
        "{}",
        line(&buffer, 5)
    );
    assert!(line(&buffer, 8).ends_with('┘'));
    assert_eq!(line(&buffer, 8).chars().count(), 80);
    assert_eq!(
        sidebar(&mut app)[..2],
        listed(None)[..2],
        "the sidebar stays above it"
    );
}

// FR-UI-01, FR-KEY-04, D-28, ui.md §1
#[test]
fn the_shortcuts_panel_opens_at_the_right_with_the_sidebar_at_the_left() {
    let mut app = Harness::with_text(OUTLINE, 100, 10);
    app.type_keys("?");
    let buffer = app.render().buffer().clone();
    assert_eq!(columns(&buffer, 0, 0, 25), "  Bookmarks             │");
    assert_eq!(columns(&buffer, 2, 25, 68), "▌ Work:");
    assert_eq!(columns(&buffer, 0, 68, 100), "│Shortcuts");
}
