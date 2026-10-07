//! Step 4.6, D-79: the navigation sidebar, a window that `Ctrl+G` opens over the panes, and its
//! keys.

use std::fmt::Write as _;

use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::{buffer::Buffer, style::Modifier};
use taskpaper_terminal::ui::Input;

use crate::support::{BOOKMARK, Harness, NAVIGATE, STAR, SUN, line};

const OUTLINE: &str = "\
Work:
\t- Launch plan @bookmark
\tReports:
\t\t- Draft
Home:
\t- Water plants
";

/// How many columns of the window's inside its lines take: all but the one it keeps clear of its
/// right border.
const TEXT: u16 = 45;

fn app(width: u16) -> Harness {
    let mut app = Harness::with_text(OUTLINE, width, 16);
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

/// Where the window's top left corner is on screen, if it is open.
fn corner(buffer: &Buffer) -> Option<(u16, u16)> {
    let area = buffer.area;
    (0..area.height)
        .flat_map(|y| (0..area.width).map(move |x| (x, y)))
        .find(|&(x, y)| {
            buffer[(x, y)].symbol() == "┌" && columns(buffer, y, x + 1, x + 10) == "Navigator"
        })
}

/// The lines inside the open window, gutter included, or `None` while it is closed.
fn window(app: &mut Harness) -> Option<Vec<String>> {
    let buffer = app.render().buffer().clone();
    let (x, top) = corner(&buffer)?;
    let lines = (top + 1..buffer.area.height)
        .take_while(|&y| buffer[(x, y)].symbol() != "└")
        .map(|y| columns(&buffer, y, x + 1, x + 1 + TEXT))
        .collect();
    Some(lines)
}

/// The pane's rows, gutter included, header and rule first.
fn pane(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    (0..buffer.area.height - 1)
        .map(|y| columns(&buffer, y, 0, buffer.area.width))
        .collect()
}

fn press(app: &mut Harness, code: KeyCode) {
    app.press(code, KeyModifiers::NONE);
}

fn ctrl_g(app: &mut Harness) {
    app.press(KeyCode::Char('g'), KeyModifiers::CONTROL);
}

/// The window's lines as it opens before any key, `▌` marking the focused or current one, each
/// project's count of open tasks at the right (D-73).
fn listed(focused: Option<usize>) -> Vec<String> {
    [
        (format!("{SUN} Today"), None),
        (format!("{STAR} Starred"), None),
        (String::new(), None),
        ("Bookmarks".to_owned(), None),
        ("Launch plan".to_owned(), None),
        (String::new(), None),
        ("Projects".to_owned(), None),
        ("▾Work".to_owned(), Some(2)),
        ("  Reports".to_owned(), Some(1)),
        ("Home".to_owned(), Some(1)),
    ]
    .into_iter()
    .enumerate()
    .map(|(at, (text, count))| {
        // A top-level project's fold marker stands in the gutter's second column (D-61).
        let gutter = match (Some(at) == focused, text.starts_with(['▾', '▸'])) {
            (true, false) => "▌ ",
            (true, true) => "▌",
            (false, false) => "  ",
            (false, true) => " ",
        };
        let line = format!("{gutter}{text}");
        match count {
            Some(count) => format!("{line:44}{count}"),
            None => line.trim_end().to_owned(),
        }
    })
    .collect()
}

// FR-UI-01, ui.md §1, navigation §6, D-79
#[test]
fn the_pane_takes_the_screen_and_ctrl_g_opens_the_window_over_it() {
    let mut app = app(80);
    assert_eq!(window(&mut app), None, "closed at the start");
    assert_eq!(
        pane(&mut app)[..4],
        [
            "  ⌂",
            "─".repeat(80).as_str(),
            "▌ Work:",
            &format!("    - Launch plan {BOOKMARK}")
        ]
    );
    ctrl_g(&mut app);
    assert_eq!(window(&mut app), Some(listed(Some(0))));
    let buffer = app.render().buffer().clone();
    assert_eq!(
        corner(&buffer),
        Some((16, 3)),
        "centered below the pane's header"
    );
    assert!(
        buffer[(2, 2)].modifier.contains(Modifier::DIM),
        "the pane is dimmed behind it"
    );
    assert!(
        !buffer[(19, 4)].modifier.contains(Modifier::DIM),
        "it is not"
    );
    assert!(
        line(&buffer, 15).starts_with(&format!("{NAVIGATE} · ")),
        "the status line spans the screen"
    );
}

// FR-UI-01, D-79
#[test]
fn ctrl_g_and_esc_close_the_window_and_the_pane_has_the_keys() {
    let mut app = app(80);
    for close in [KeyCode::Esc, KeyCode::Char('g')] {
        ctrl_g(&mut app);
        let modifiers = match close {
            KeyCode::Esc => KeyModifiers::NONE,
            _ => KeyModifiers::CONTROL,
        };
        app.press(close, modifiers);
        assert_eq!(window(&mut app), None, "{close:?}");
    }
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(
        pane(&mut app)[3],
        format!("▌   - Launch plan {BOOKMARK}"),
        "j moves in the pane"
    );
}

// D-68
#[test]
fn the_window_opens_on_the_entry_of_the_panes_zoom_which_stays_marked() {
    let mut app = app(80);
    app.type_keys(".");
    ctrl_g(&mut app);
    assert_eq!(window(&mut app), Some(listed(Some(7))), "Work");
    press(&mut app, KeyCode::Esc);
    app.type_keys("j..");
    assert_eq!(pane(&mut app)[0], "  ⌂ › Work › Reports › Draft");
    ctrl_g(&mut app);
    assert_eq!(
        window(&mut app),
        Some(listed(Some(8))),
        "Reports, above the zoomed task"
    );
    press(&mut app, KeyCode::Up);
    let mut lines = listed(Some(7));
    lines[8] = format!("{:44}1", "▌   Reports");
    assert_eq!(
        window(&mut app),
        Some(lines),
        "↑ moves the focus, the current entry keeps its marker"
    );
    let buffer = app.render().buffer().clone();
    let (x, y) = corner(&buffer).expect("open");
    assert!(
        buffer[(x + 1, y + 9)].modifier.contains(Modifier::DIM),
        "a selected block's marker"
    );
}

// FR-UI-01, keybindings.md §4, D-86
#[test]
fn the_arrows_and_ctrl_n_and_ctrl_p_move_in_the_window_skipping_headings() {
    let mut app = app(80);
    ctrl_g(&mut app);
    press(&mut app, KeyCode::Down);
    assert_eq!(window(&mut app), Some(listed(Some(1))));
    press(&mut app, KeyCode::Down);
    assert_eq!(
        window(&mut app),
        Some(listed(Some(4))),
        "headings and blank lines are skipped"
    );
    app.press(KeyCode::Char('n'), KeyModifiers::CONTROL);
    assert_eq!(window(&mut app), Some(listed(Some(7))));
    app.press(KeyCode::Char('p'), KeyModifiers::CONTROL);
    press(&mut app, KeyCode::Up);
    press(&mut app, KeyCode::Up);
    assert_eq!(window(&mut app), Some(listed(Some(0))));
    press(&mut app, KeyCode::Esc);
    assert_eq!(pane(&mut app)[2], "▌ Work:", "the pane's focus stayed");
}

// keybindings.md §4, D-79: the window keeps the keys until it closes.
#[test]
fn the_keys_that_focus_a_pane_and_the_sidebars_old_keys_do_nothing() {
    let mut app = app(80);
    ctrl_g(&mut app);
    app.press(KeyCode::Char('1'), KeyModifiers::ALT);
    app.press(KeyCode::Left, KeyModifiers::CONTROL | KeyModifiers::SHIFT);
    app.press(KeyCode::Char('w'), KeyModifiers::CONTROL);
    press(&mut app, KeyCode::Left);
    assert_eq!(window(&mut app), Some(listed(Some(0))), "still open");
    press(&mut app, KeyCode::Esc);
    app.press(KeyCode::Char('b'), KeyModifiers::CONTROL);
    app.press(KeyCode::Char('0'), KeyModifiers::ALT);
    app.type_keys("g0");
    assert_eq!(window(&mut app), None, "Ctrl+B, Alt+0 and g0 open nothing");
}

// FR-UI-01, navigation §6
#[test]
fn enter_zooms_the_pane_to_the_entry_and_closes_the_window() {
    let mut app = app(80);
    ctrl_g(&mut app);
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    assert_eq!(window(&mut app), None);
    assert_eq!(pane(&mut app)[0], "  ⌂ › Work › Launch plan");
    ctrl_g(&mut app);
    assert_eq!(window(&mut app), Some(listed(Some(4))), "on its entry");
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        pane(&mut app)[..4],
        [
            "  ⌂ › Work › Reports",
            "─".repeat(80).as_str(),
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
    ctrl_g(&mut app);
    press(&mut app, KeyCode::Enter);
    assert_eq!(pane(&mut app)[0], format!("  {SUN} Today"));
    ctrl_g(&mut app);
    assert_eq!(window(&mut app), Some(listed(Some(0))), "on its entry");
}

// keybindings.md §4, D-86
#[test]
fn the_left_and_right_arrows_fold_project_entries() {
    let mut app = app(80);
    ctrl_g(&mut app);
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Left);
    let mut folded = listed(Some(7));
    folded[7] = format!("{:44}2", "▌▸Work");
    folded.remove(8);
    assert_eq!(window(&mut app), Some(folded));
    press(&mut app, KeyCode::Right);
    assert_eq!(window(&mut app), Some(listed(Some(7))));
    press(&mut app, KeyCode::Esc);
    assert_eq!(
        pane(&mut app)[3],
        format!("    - Launch plan {BOOKMARK}"),
        "the outline keeps its folds"
    );
}

#[test]
fn ctrl_q_quits_from_the_window() {
    let mut app = app(80);
    ctrl_g(&mut app);
    app.press(KeyCode::Char('q'), KeyModifiers::CONTROL);
    assert!(app.tui.should_quit());
}

// FR-OPS-06
#[test]
fn text_pasted_into_the_window_is_dropped() {
    let mut app = app(80);
    ctrl_g(&mut app);
    app.send(Input::Terminal(Event::Paste("- Eggs".to_owned())));
    press(&mut app, KeyCode::Esc);
    assert_eq!(pane(&mut app)[3], format!("    - Launch plan {BOOKMARK}"));
}

// ui.md §1
#[test]
fn a_narrow_screen_draws_the_window_as_wide_as_the_pane() {
    let mut app = app(30);
    ctrl_g(&mut app);
    let buffer = app.render().buffer().clone();
    assert_eq!(corner(&buffer), Some((0, 3)));
    assert_eq!(columns(&buffer, 3, 0, 30).chars().last(), Some('┐'));
    assert_eq!(
        columns(&buffer, 4, 0, 30),
        format!("│▌ {SUN} Today{:19}│", "")
    );
}

// FR-UI-01
#[test]
fn the_window_scrolls_to_keep_its_focused_entry_visible() {
    let projects = (1..=12)
        .map(|n| format!("P{n}:"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut app = Harness::with_text(&projects, 80, 10);
    app.render();
    ctrl_g(&mut app);
    assert_eq!(
        window(&mut app).expect("open"),
        [
            format!("▌ {SUN} Today"),
            format!("  {STAR} Starred"),
            String::new(),
            "  Projects".to_owned(),
            "  P1".to_owned(),
        ]
    );
    for _ in 0..6 {
        press(&mut app, KeyCode::Down);
    }
    assert_eq!(
        window(&mut app).expect("open"),
        ["  P3", "  P4", "▌ P5", "  P6", "  P7"]
    );
}

// D-73, D-67
#[test]
fn a_projects_count_of_open_tasks_follows_completing_them() {
    let mut app = app(80);
    app.type_keys("jx");
    ctrl_g(&mut app);
    let mut listed = listed(Some(0));
    listed[7] = format!("{:44}1", " ▾Work");
    assert_eq!(window(&mut app), Some(listed.clone()), "Launch plan done");
    press(&mut app, KeyCode::Esc);
    app.type_keys("jjx");
    ctrl_g(&mut app);
    listed[7] = " ▾Work".to_owned();
    listed[8] = "    Reports".to_owned();
    assert_eq!(window(&mut app), Some(listed), "no open tasks, no count");
}

const INBOXED: &str = "\
Work:
INBOX:
\t- Call the bank
";

// D-75
#[test]
fn the_inbox_project_is_listed_above_today_unless_the_setting_is_off() {
    let mut app = Harness::with_text(INBOXED, 80, 16);
    ctrl_g(&mut app);
    assert_eq!(
        window(&mut app).expect("open"),
        [
            format!("{:44}1", "▌ ⊔ INBOX"),
            format!("  {SUN} Today"),
            format!("  {STAR} Starred"),
            String::new(),
            "  Projects".to_owned(),
            "  Work".to_owned(),
        ]
    );
    let mut app = Harness::with_config(INBOXED, "special_inbox = false\n", 80, 16);
    ctrl_g(&mut app);
    assert_eq!(
        window(&mut app).expect("open"),
        [
            format!("▌ {SUN} Today"),
            format!("  {STAR} Starred"),
            String::new(),
            "  Projects".to_owned(),
            "  Work".to_owned(),
            format!("{:44}1", "  INBOX"),
        ]
    );
}

// D-77
#[test]
fn the_today_entry_leaves_the_window_with_its_setting_off_and_gt_still_opens_today() {
    let mut app = Harness::with_config(INBOXED, "sidebar_today = false\n", 80, 16);
    ctrl_g(&mut app);
    assert_eq!(
        window(&mut app).expect("open"),
        [
            format!("{:44}1", "▌ ⊔ INBOX"),
            format!("  {STAR} Starred"),
            String::new(),
            "  Projects".to_owned(),
            "  Work".to_owned(),
        ]
    );
    press(&mut app, KeyCode::Esc);
    app.type_keys("gt");
    assert_eq!(pane(&mut app)[0], format!("  {SUN} Today"));
    app.type_keys("g,jjjj ");
    app.settle();
    press(&mut app, KeyCode::Esc);
    ctrl_g(&mut app);
    assert_eq!(
        window(&mut app).expect("open")[1],
        format!("▌ {SUN} Today"),
        "back on, focused as where the pane is"
    );
    assert_eq!(
        std::fs::read_to_string(app.config_file()).expect("written"),
        "sidebar_today = true\n"
    );
}

// D-78
#[test]
fn g_i_zooms_to_the_inbox_and_says_so_when_there_is_none() {
    let mut app = Harness::with_text(INBOXED, 80, 10);
    app.type_keys("gi");
    assert_eq!(pane(&mut app)[0], "  ⌂ › INBOX", "the breadcrumbs");
    let mut app = Harness::with_text(OUTLINE, 100, 10);
    app.type_keys("gi");
    let status = line(app.render().buffer(), 9);
    assert!(
        status.ends_with("· No INBOX project in the Navigator"),
        "{status}"
    );
}

// D-86, ui.md §1
#[test]
fn typing_filters_the_window_shown_in_its_bottom_border_and_enter_goes_to_the_match() {
    let mut app = app(80);
    ctrl_g(&mut app);
    app.type_keys("rep");
    assert_eq!(
        window(&mut app),
        Some(vec![
            "  Projects".to_owned(),
            format!("{:44}2", "  Work"),
            format!("{:44}1", "▌   Reports"),
        ]),
        "the project above the match stays, without fold markers"
    );
    let buffer = app.render().buffer().clone();
    let (x, top) = corner(&buffer).expect("open");
    assert_eq!(columns(&buffer, top + 4, x, x + 6), "└rep──");
    press(&mut app, KeyCode::Backspace);
    app.type_keys("z");
    assert_eq!(window(&mut app), Some(vec![" Nothing matches".to_owned()]));
    for _ in 0..3 {
        press(&mut app, KeyCode::Backspace);
    }
    app.type_keys("home");
    press(&mut app, KeyCode::Enter);
    assert_eq!(window(&mut app), None);
    assert_eq!(pane(&mut app)[0], "  ⌂ › Home");
    ctrl_g(&mut app);
    assert_eq!(
        window(&mut app).map(|lines| lines.len()),
        Some(10),
        "the filter is gone"
    );
}
