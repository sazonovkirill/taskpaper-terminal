//! Step 4.7: the Today view, chosen in the Navigator, edited in place and left for the outline.

use std::time::Duration;

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::Modifier;

use crate::support::{
    DUE, Harness, OVERDUE, STAR, START, SUN, focused_rows, outline_line, outline_rows,
};

/// Blocks for today, 5 October 2026, at several depths, among blocks that are not.
const OUTLINE: &str = "\
Home:
\t- Water plants @star
\t- Paint the hall @due(2026-10-09)
\tErrands:
\t\t- Post office @start(2026-10-05)
\t\t- Bank @due(2026-10-01)
- Plan the week @start(2026-10-06)
";

/// Opens `text` on an 80×10 screen and chooses Today, the Navigator's first entry without an
/// inbox.
fn today(text: &str) -> Harness {
    let mut app = Harness::with_text(text, 80, 10);
    app.render();
    app.press(KeyCode::Char('g'), KeyModifiers::CONTROL);
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    app
}

/// The rows on screen below the header that are not blank.
fn shown(app: &mut Harness) -> Vec<String> {
    outline_rows(app)
        .into_iter()
        .filter(|row| !row.is_empty())
        .collect()
}

/// `text` with the chips `chips` at the right edge of a row of the 80-column outline.
fn row(text: &str, chips: &str) -> String {
    let width = 78 - text.chars().count();
    format!("{text}{chips:>width$}")
}

// FR-TAG-04, D-7, navigation §6, §7
#[test]
fn choosing_today_in_the_navigator_lists_the_blocks_for_today_with_their_paths() {
    let mut app = today(OUTLINE);
    let buffer = app.render().buffer().clone();
    assert_eq!(outline_line(&buffer, 0), format!("{SUN} Today"));
    assert!(
        buffer[(4, 0)].modifier.contains(Modifier::BOLD),
        "the header is bold"
    );
    assert_eq!(
        shown(&mut app),
        [
            format!("- Water plants {STAR}  Home"),
            row("- Post office  Home › Errands", &format!("{START} today")),
            row("- Bank  Home › Errands", &format!("{OVERDUE} 4 days ago")),
        ]
    );
    let path = buffer[(20, 2)].clone();
    assert_eq!(path.symbol(), "H");
    assert!(path.modifier.contains(Modifier::DIM), "the path is dimmed");
    assert_eq!(
        focused_rows(&mut app),
        [format!("- Water plants {STAR}  Home")]
    );
}

// D-64, navigation §7: `gt` shows the Today view in the focused pane as choosing it in the
// Navigator does, and Back returns to the outline.
#[test]
fn g_t_shows_the_today_view_and_back_returns() {
    let mut app = Harness::with_text(OUTLINE, 80, 10);
    app.type_keys("gt");
    let buffer = app.render().buffer().clone();
    assert_eq!(outline_line(&buffer, 0), format!("{SUN} Today"));
    assert_eq!(shown(&mut app).len(), 3);
    assert_eq!(
        focused_rows(&mut app),
        [format!("- Water plants {STAR}  Home")]
    );
    app.type_keys("H");
    let buffer = app.render().buffer().clone();
    assert_ne!(outline_line(&buffer, 0), format!("{SUN} Today"));
    assert_eq!(
        focused_rows(&mut app),
        [format!("  - Water plants {STAR}")],
        "the focused entry shows in the outline"
    );
}

// D-64, FR-NAV-06
#[test]
fn go_to_command_lists_go_to_today_with_its_key_and_gives_it() {
    let mut app = Harness::with_text(OUTLINE, 80, 10);
    app.type_keys(":go to today");
    let buffer = app.render().buffer().clone();
    let rows: Vec<String> = (0..buffer.area.height)
        .map(|y| crate::support::line(&buffer, y))
        .collect();
    assert!(
        rows.iter()
            .any(|row| row.contains(&format!("│ {SUN} Go to Today  gt "))),
        "{rows:#?}"
    );
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    let buffer = app.render().buffer().clone();
    assert_eq!(outline_line(&buffer, 0), format!("{SUN} Today"));
}

// D-7, FR-DONE-01, FR-TAG-04, FR-TAG-06
#[test]
fn block_commands_act_on_the_focused_entry_and_the_list_follows() {
    let mut app = today(OUTLINE);
    app.type_keys("jw");
    assert_eq!(shown(&mut app).len(), 2, "the post office starts tomorrow");
    assert_eq!(
        focused_rows(&mut app),
        [row(
            "- Bank  Home › Errands",
            &format!("{OVERDUE} 4 days ago")
        )]
    );
    app.type_keys("T");
    assert_eq!(
        focused_rows(&mut app),
        [row("- Bank  Home › Errands", &format!("{DUE} today"))]
    );
    app.type_keys("x");
    assert!(
        focused_rows(&mut app)[0].starts_with("✔ Bank"),
        "a done entry stays while completed blocks show"
    );
    app.type_keys("gc");
    assert_eq!(shown(&mut app), [format!("- Water plants {STAR}  Home")]);
    app.type_keys("*");
    assert_eq!(shown(&mut app), Vec::<String>::new());
    app.wait(Duration::from_secs(1));
    app.settle();
    let saved = std::fs::read_to_string(app.path.as_ref().expect("a file")).expect("saved");
    assert!(saved.contains("\t- Water plants\n"), "{saved}");
}

// FR-MODE-01, D-7
#[test]
fn enter_edits_an_entry_in_place_and_enter_again_stops_editing() {
    let mut app = today(OUTLINE);
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    app.type_keys(" now");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        focused_rows(&mut app),
        [format!("- Water plants now {STAR}  Home")]
    );
    assert_eq!(shown(&mut app).len(), 3, "no block was added");
}

// D-7, navigation §7: the classic and kitty variants of D-42
#[test]
fn alt_l_and_gz_go_from_an_entry_to_it_in_the_outline_and_back_returns() {
    for keys in [
        &[(KeyCode::Char('l'), KeyModifiers::ALT)][..],
        &[
            (KeyCode::Char('g'), KeyModifiers::NONE),
            (KeyCode::Char('z'), KeyModifiers::NONE),
        ],
    ] {
        let mut app = today(OUTLINE);
        app.type_keys("jj");
        for &(code, modifiers) in keys {
            app.press(code, modifiers);
        }
        let header = outline_line(app.render().buffer(), 0);
        assert_eq!(header, "⌂ › Home › Errands", "{keys:?}");
        assert_eq!(
            focused_rows(&mut app),
            [row("  - Bank", &format!("{OVERDUE} 4 days ago"))],
            "{keys:?}"
        );
        app.type_keys("H");
        assert_eq!(
            outline_line(app.render().buffer(), 0),
            format!("{SUN} Today")
        );
        app.type_keys("L");
        assert_eq!(outline_line(app.render().buffer(), 0), "⌂ › Home › Errands");
    }
}

// D-7, navigation §7
#[test]
fn keys_for_the_outlines_tree_do_nothing_on_an_entry() {
    let mut app = today(OUTLINE);
    let before = shown(&mut app);
    for keys in ["h", "l", "o", "gp", "gl", "-", "_"] {
        app.type_keys(keys);
        app.wait(Duration::from_secs(1));
        assert_eq!(shown(&mut app), before, "{keys}");
        assert_eq!(focused_rows(&mut app), [before[0].clone()], "{keys}");
    }
    app.press(KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(shown(&mut app), before);
}

// D-57, navigation §7: the classic and kitty variants of D-42
#[test]
fn alt_k_and_alt_j_reorder_the_entries_and_leave_the_file_as_it_is() {
    for (up, down) in [
        (KeyCode::Char('k'), KeyCode::Char('j')),
        (KeyCode::Up, KeyCode::Down),
    ] {
        let mut app = today(OUTLINE);
        let entries = shown(&mut app);
        app.type_keys("G");
        app.press(up, KeyModifiers::ALT);
        app.press(up, KeyModifiers::ALT);
        let reordered = [entries[2].clone(), entries[0].clone(), entries[1].clone()];
        assert_eq!(shown(&mut app), reordered, "{up:?}");
        assert_eq!(focused_rows(&mut app), [entries[2].clone()], "{up:?}");
        app.press(down, KeyModifiers::ALT);
        assert_eq!(
            shown(&mut app),
            [entries[0].clone(), entries[2].clone(), entries[1].clone()],
            "{down:?}"
        );
        app.wait(Duration::from_secs(5));
        app.settle();
        let path = app.path.clone().expect("a file");
        assert_eq!(
            std::fs::read_to_string(&path).expect("the file"),
            OUTLINE,
            "the file is not written"
        );
    }
}

// D-57, FR-UI-04, file-io.md §5
#[test]
fn the_order_of_the_entries_comes_back_on_the_next_launch() {
    let mut app = today(OUTLINE);
    app.type_keys("G");
    app.press(KeyCode::Char('k'), KeyModifiers::ALT);
    let expected = shown(&mut app);
    let mut app = app.reopen(80, 10);
    assert_eq!(shown(&mut app), expected, "the Today view");
    let path = app.path.clone().expect("a file");
    assert_eq!(std::fs::read_to_string(&path).expect("the file"), OUTLINE);
}

// D-7, FR-TAG-04
#[test]
fn the_list_changes_at_local_midnight() {
    let mut app = today("- a @star\n- b @due(2026-10-06)\n");
    assert_eq!(shown(&mut app), [format!("- a {STAR}")]);
    let deadline = app.tui.deadline().expect("a redraw at midnight");
    app.wait(deadline);
    assert_eq!(
        shown(&mut app),
        [format!("- a {STAR}"), row("- b", &format!("{DUE} today"))]
    );
}

// FR-TAG-04, FR-SET-03, D-88, ui.md §8
#[test]
fn turning_off_starred_items_in_today_leaves_only_dated_blocks_and_is_kept() {
    let text = "- a @star\n- b @due(2026-10-05)\n";
    let mut app = Harness::with_config(text, "", 80, 24);
    app.type_keys("gt");
    assert_eq!(
        shown(&mut app),
        [format!("- a {STAR}"), row("- b", &format!("{DUE} today"))]
    );
    app.type_keys("g,jjjjj");
    app.press(KeyCode::Char('l'), KeyModifiers::NONE);
    app.settle();
    let config = std::fs::read_to_string(app.config_file()).expect("the settings file");
    assert_eq!(config, "today_starred = false\n");
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(shown(&mut app), [row("- b", &format!("{DUE} today"))]);
    let mut app = Harness::with_config(text, &config, 80, 24);
    app.type_keys("gt");
    assert_eq!(
        shown(&mut app),
        [row("- b", &format!("{DUE} today"))],
        "off from the start"
    );
}
