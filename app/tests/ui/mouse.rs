//! Step 6.5: the mouse. A click focuses a block, places the caret in the text being edited,
//! toggles a fold marker, zooms to a crumb, goes to a sidebar entry or focuses a pane; Alt+click
//! opens in a new pane; a right-click opens the context menu at the pointer; the wheel scrolls
//! the pane or panel under the pointer.

use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::style::Modifier;

use crate::support::{Harness, focused_rows, line, outline_rows, selected_rows};

const OUTLINE: &str = "\
Work:
\t- Report
\t\tDraft
\t- Slides
Home:
\tGarden:
\t\t- Water plants
";

/// Every screen row, without trailing blanks.
fn screen(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    (buffer.area.top()..buffer.area.bottom())
        .map(|y| line(&buffer, y))
        .collect()
}

fn press(app: &mut Harness, code: KeyCode) {
    app.press(code, KeyModifiers::NONE);
}

fn alt_click(app: &mut Harness, x: u16, y: u16) {
    app.mouse(
        MouseEventKind::Down(MouseButton::Left),
        x,
        y,
        KeyModifiers::ALT,
    );
}

fn right_click(app: &mut Harness, x: u16, y: u16) {
    app.mouse(
        MouseEventKind::Down(MouseButton::Right),
        x,
        y,
        KeyModifiers::NONE,
    );
}

fn wheel(app: &mut Harness, x: u16, y: u16, down: bool) {
    let kind = if down {
        MouseEventKind::ScrollDown
    } else {
        MouseEventKind::ScrollUp
    };
    app.mouse(kind, x, y, KeyModifiers::NONE);
}

/// The outline of one pane without the sidebar, 50×12.
fn pane() -> Harness {
    let mut app = Harness::with_text(OUTLINE, 50, 12);
    app.hide_sidebar();
    app
}

/// The breadcrumb header's text, after the gutter.
fn header(app: &mut Harness) -> String {
    let buffer = app.render().buffer().clone();
    line(&buffer, 0).trim_start().to_owned()
}

/// The column of the context menu's top left corner, whose rows `menu` gives.
fn corner(menu: &[String]) -> Option<usize> {
    menu.first()?.chars().position(|c| c == '┌')
}

/// The rows of the open context menu, from its title down.
fn menu(app: &mut Harness) -> Vec<String> {
    screen(app)
        .into_iter()
        .skip_while(|row| !row.contains("┌Commands:"))
        .take_while(|row| !row.contains('└'))
        .collect()
}

// FR-KEY-05, ui.md §6
#[test]
fn a_click_focuses_the_block_under_the_pointer_and_ends_editing() {
    let mut app = pane();
    app.click(30, 6);
    assert_eq!(
        focused_rows(&mut app),
        ["Home:"],
        "the rest of the row counts too"
    );
    app.click(9, 4);
    assert_eq!(focused_rows(&mut app), ["    Draft"]);
    press(&mut app, KeyCode::Enter);
    app.click(1, 3);
    assert_eq!(
        focused_rows(&mut app),
        ["  - Report"],
        "the gutter counts too"
    );
    assert!(screen(&mut app)[11].contains("NAVIGATE"));
    app.click(5, 10);
    app.click(5, 11);
    app.mouse(
        MouseEventKind::Up(MouseButton::Left),
        30,
        6,
        KeyModifiers::NONE,
    );
    app.mouse(MouseEventKind::Moved, 30, 6, KeyModifiers::NONE);
    assert_eq!(
        focused_rows(&mut app),
        ["  - Report"],
        "nothing drawn there, or no click"
    );
}

// FR-KEY-05, ui.md §6
#[test]
fn a_click_in_the_text_being_edited_places_the_caret() {
    let mut app = pane();
    app.click(8, 3);
    press(&mut app, KeyCode::Enter);
    app.click(8, 3);
    app.type_keys("X");
    assert_eq!(outline_rows(&mut app)[1], "  - ReXport");
    app.click(2, 3);
    app.type_keys("Y");
    assert_eq!(
        outline_rows(&mut app)[1],
        "  - YReXport",
        "left of the text"
    );
    app.click(40, 3);
    app.type_keys("Z");
    assert_eq!(
        outline_rows(&mut app)[1],
        "  - YReXportZ",
        "right of the text"
    );
    assert!(screen(&mut app)[11].contains("EDIT"));
}

// FR-KEY-05, FR-NAV-02, ui.md §6, D-67
#[test]
fn a_click_on_a_fold_marker_toggles_the_fold() {
    let mut app = pane();
    app.click(8, 3);
    press(&mut app, KeyCode::Char('h'));
    assert_eq!(outline_rows(&mut app)[1], "  - Report ▸");
    app.click(8, 3);
    assert_eq!(
        outline_rows(&mut app)[2],
        "  - Slides",
        "a click on the text only focuses"
    );
    app.click(13, 3);
    assert_eq!(outline_rows(&mut app)[2], "    Draft");
    app.click(13, 3);
    assert_eq!(
        outline_rows(&mut app)[2],
        "    Draft",
        "no marker on an expanded block"
    );
}

// FR-KEY-05, navigation.md §4, ui.md §6
#[test]
fn a_click_on_a_crumb_zooms_the_pane_there_and_on_home_goes_home() {
    let mut app = pane();
    app.click(8, 4);
    press(&mut app, KeyCode::Char('.'));
    assert_eq!(header(&mut app), "⌂ › Work › Report › Draft");
    app.click(11, 0);
    assert_eq!(header(&mut app), "⌂ › Work › Report › Draft", "a separator");
    app.click(9, 0);
    assert_eq!(header(&mut app), "⌂ › Work");
    assert_eq!(
        focused_rows(&mut app),
        ["    Draft"],
        "the block zoomed out of"
    );
    app.click(2, 0);
    assert_eq!(header(&mut app), "⌂");
    press(&mut app, KeyCode::Char('H'));
    assert_eq!(header(&mut app), "⌂ › Work", "a step in the history");
}

// FR-KEY-05, FR-UI-03, navigation.md §5
#[test]
fn alt_click_opens_a_block_or_a_crumb_in_a_new_pane() {
    let mut app = Harness::with_text(OUTLINE, 80, 12);
    app.hide_sidebar();
    alt_click(&mut app, 8, 3);
    let rows = screen(&mut app);
    assert!(rows[0].ends_with("│  ⌂ › Work › Report"), "{rows:?}");
    alt_click(&mut app, 47, 0);
    let rows = screen(&mut app);
    assert!(rows[0].ends_with("│  ⌂ › Work"), "{rows:?}");
    assert_eq!(rows[1].matches('│').count(), 2, "three panes");
}

// FR-KEY-05, FR-UI-02, ui.md §6
#[test]
fn a_click_on_a_pane_header_or_into_another_pane_focuses_that_pane() {
    let mut app = Harness::with_text(OUTLINE, 80, 12);
    app.hide_sidebar();
    app.press(KeyCode::Char('\\'), KeyModifiers::CONTROL);
    let dimmed = |app: &mut Harness, x| {
        app.render().buffer()[(x, 0)]
            .modifier
            .contains(Modifier::DIM)
    };
    assert!(dimmed(&mut app, 2));
    app.click(20, 1);
    assert!(!dimmed(&mut app, 2), "the left pane has the keys");
    assert!(dimmed(&mut app, 42));
    app.click(50, 3);
    assert!(dimmed(&mut app, 2));
    assert!(!dimmed(&mut app, 42), "the right pane has the keys again");
}

// FR-KEY-05, FR-UI-01, navigation.md §6
#[test]
fn a_click_on_a_sidebar_entry_goes_there_and_on_its_fold_marker_folds_it() {
    let mut app = Harness::with_text(OUTLINE, 80, 12);
    let rows = screen(&mut app);
    assert!(rows[4].starts_with(" ▾Home "), "{rows:?}");
    app.click(10, 5);
    assert!(screen(&mut app)[0].ends_with("⌂ › Home › Garden"));
    app.click(1, 4);
    let rows = screen(&mut app);
    assert!(
        rows[4].starts_with("▌▸Home"),
        "the sidebar has the keys on Home: {rows:?}"
    );
    assert!(rows[5].starts_with("        "), "Garden folded away");
    alt_click(&mut app, 10, 3);
    let rows = screen(&mut app);
    assert!(rows[0].ends_with("│  ⌂ › Work"), "{rows:?}");
}

// FR-KEY-05, FR-KEY-06, ui.md §5
#[test]
fn a_right_click_on_a_block_opens_its_menu_at_the_pointer() {
    let mut app = Harness::with_text(OUTLINE, 100, 40);
    app.hide_sidebar();
    right_click(&mut app, 12, 5);
    let rows = menu(&mut app);
    assert_eq!(corner(&rows), Some(12), "{rows:?}");
    assert!(screen(&mut app)[5].starts_with("▌   - Slides"));
    assert!(rows.iter().any(|row| row.contains("Edit")));
    press(&mut app, KeyCode::Esc);
    assert_eq!(menu(&mut app), Vec::<String>::new());
    press(&mut app, KeyCode::Char('\\'));
    assert_eq!(
        corner(&menu(&mut app)),
        Some(6),
        "a menu opened by key opens at the focused block's text"
    );
}

// FR-KEY-05, FR-KEY-06, FR-OPS-01
#[test]
fn a_right_click_on_the_selection_opens_its_menu_and_elsewhere_drops_it() {
    let mut app = Harness::with_text(OUTLINE, 100, 40);
    app.hide_sidebar();
    app.type_keys("vjv");
    // The menu dims the focus and selection markers alike (D-69), so they are checked with it
    // closed.
    right_click(&mut app, 8, 3);
    assert!(
        menu(&mut app).iter().all(|row| !row.contains("Edit")),
        "the selection's menu"
    );
    press(&mut app, KeyCode::Esc);
    assert_eq!(selected_rows(&mut app), ["Work:"]);
    right_click(&mut app, 8, 6);
    assert!(menu(&mut app).iter().any(|row| row.contains("Edit")));
    press(&mut app, KeyCode::Esc);
    assert_eq!(selected_rows(&mut app), Vec::<String>::new());
    assert_eq!(focused_rows(&mut app), ["Home:"]);
}

// FR-KEY-05, FR-KEY-06, ui.md §5
#[test]
fn a_right_click_on_a_crumb_a_pane_header_or_a_sidebar_entry_opens_its_menu() {
    let mut app = Harness::with_text(OUTLINE, 80, 14);
    app.click(32, 3);
    press(&mut app, KeyCode::Char('.'));
    right_click(&mut app, 30, 0);
    let rows = menu(&mut app);
    assert!(rows.iter().any(|row| row.contains("Zoom Here")), "{rows:?}");
    assert!(rows.iter().any(|row| row.contains("Open in New Pane")));
    press(&mut app, KeyCode::Esc);
    right_click(&mut app, 70, 1);
    let rows = menu(&mut app);
    assert!(rows.iter().any(|row| row.contains("Zoom Out")), "{rows:?}");
    assert!(
        rows.iter()
            .any(|row| row.contains("Open Location in New Pane"))
    );
    press(&mut app, KeyCode::Esc);
    right_click(&mut app, 6, 4);
    let rows = menu(&mut app);
    assert!(
        rows.iter().any(|row| row.contains("Go to Entry")),
        "{rows:?}"
    );
}

// FR-KEY-05, ui.md §6
#[test]
fn the_wheel_scrolls_the_pane_under_the_pointer_until_a_key() {
    let text = (1..=30).fold(String::new(), |text, n| text + &format!("- Task {n}\n"));
    let mut app = Harness::with_text(&text, 50, 10);
    app.hide_sidebar();
    wheel(&mut app, 10, 5, true);
    wheel(&mut app, 10, 5, true);
    assert_eq!(outline_rows(&mut app)[0], "- Task 7");
    assert!(
        focused_rows(&mut app).is_empty(),
        "the focus stays on Task 1"
    );
    wheel(&mut app, 10, 5, false);
    assert_eq!(outline_rows(&mut app)[0], "- Task 4");
    for _ in 0..20 {
        wheel(&mut app, 10, 5, true);
    }
    assert_eq!(
        outline_rows(&mut app)[6],
        "- Task 30",
        "no further than the end"
    );
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(
        outline_rows(&mut app)[0],
        "- Task 1",
        "a key shows the focus again"
    );
    assert_eq!(focused_rows(&mut app), ["- Task 2"]);
    wheel(&mut app, 10, 5, true);
    app.click(10, 4);
    assert_eq!(focused_rows(&mut app), ["- Task 6"]);
    assert_eq!(
        outline_rows(&mut app)[0],
        "- Task 4",
        "a click keeps the view"
    );
}

// FR-KEY-05, ui.md §6
#[test]
fn the_wheel_scrolls_by_screen_lines_through_a_wrapped_block() {
    let long = "word ".repeat(60);
    let text = format!("- Task 1\n- {long}\n")
        + &(2..=20).fold(String::new(), |text, n| text + &format!("- Task {n}\n"));
    let mut app = Harness::with_text(&text, 50, 10);
    app.hide_sidebar();
    wheel(&mut app, 10, 5, true);
    let rows = outline_rows(&mut app);
    assert!(rows[0].trim_start().starts_with("word"), "{rows:?}");
    wheel(&mut app, 10, 5, false);
    assert_eq!(outline_rows(&mut app)[0], "- Task 1");
}

// FR-KEY-05, FR-KEY-04, D-55
#[test]
fn the_wheel_scrolls_the_shortcuts_panel_under_the_pointer() {
    let mut app = Harness::with_text(OUTLINE, 100, 12);
    press(&mut app, KeyCode::Char('?'));
    let before = screen(&mut app);
    assert!(before.iter().all(|row| !row.contains("↑ more")));
    wheel(&mut app, 90, 5, true);
    let after = screen(&mut app);
    assert!(after.iter().any(|row| row.contains("↑ more")), "{after:?}");
    assert_eq!(after[2].get(..40), before[2].get(..40), "the pane stays");
    wheel(&mut app, 90, 5, false);
    app.click(90, 5);
    wheel(&mut app, 5, 5, true);
    assert_eq!(
        screen(&mut app),
        before,
        "a click on the panel and the wheel on the sidebar do nothing"
    );
}

// FR-KEY-05, D-16
#[test]
fn the_mouse_does_nothing_while_a_popup_takes_the_keys() {
    let mut app = pane();
    press(&mut app, KeyCode::Char('\\'));
    let before = screen(&mut app);
    app.click(8, 6);
    right_click(&mut app, 8, 6);
    wheel(&mut app, 8, 6, true);
    assert_eq!(screen(&mut app), before);
}
