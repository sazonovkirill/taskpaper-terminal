//! The Starred view: every starred block, opened with `g*` or from the Navigator (D-92).

use crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{FLAG, Harness, STAR, outline_line, outline_rows};

/// Starred blocks at several depths among blocks that are not.
const OUTLINE: &str = "\
Home:
\t- Water plants @star
\t- Paint the hall @due(2026-10-01)
\tErrands:
\t\t- Bank @star
- Plan the week @star
";

/// The rows on screen below the header that are not blank.
fn shown(app: &mut Harness) -> Vec<String> {
    outline_rows(app)
        .into_iter()
        .filter(|row| !row.is_empty())
        .collect()
}

// FR-TAG-13, D-92, D-42
#[test]
fn g_star_lists_every_starred_block_with_its_path_under_the_starred_header() {
    for star in [KeyModifiers::NONE, KeyModifiers::SHIFT] {
        let mut app = Harness::with_text(OUTLINE, 80, 10);
        app.type_keys("g");
        app.press(KeyCode::Char('*'), star);
        let buffer = app.render().buffer().clone();
        assert_eq!(outline_line(&buffer, 0), format!("{STAR} Starred"));
        assert_eq!(
            shown(&mut app),
            [
                format!("- Water plants {STAR}  Home"),
                format!("- Bank {STAR}  Home › Errands"),
                format!("- Plan the week {STAR}"),
            ],
            "{star:?}"
        );
    }
}

/// The pane's header after choosing the Navigator's fifth entry in `app`, past Today, Upcoming,
/// Overdue and Colors.
fn fifth_entry(app: &mut Harness) -> String {
    app.render();
    app.press(KeyCode::Char('g'), KeyModifiers::CONTROL);
    for _ in 0..4 {
        app.press(KeyCode::Down, KeyModifiers::NONE);
    }
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    outline_line(app.render().buffer(), 0)
}

// FR-TAG-13, D-92, navigation §6
#[test]
fn the_navigator_lists_starred_after_the_other_views_unless_its_setting_is_off() {
    let mut app = Harness::with_text(OUTLINE, 80, 16);
    assert_eq!(fifth_entry(&mut app), format!("{STAR} Starred"));
    let mut app = Harness::with_config(OUTLINE, "sidebar_starred = false\n", 80, 16);
    assert_eq!(
        fifth_entry(&mut app),
        format!("{FLAG} Flagged"),
        "the view after it"
    );
    app.type_keys("g*");
    let buffer = app.render().buffer().clone();
    assert_eq!(
        outline_line(&buffer, 0),
        format!("{STAR} Starred"),
        "g* still opens it"
    );
}
