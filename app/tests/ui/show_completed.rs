//! Step 4.5: showing and hiding completed blocks.

use crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Harness, focused_rows, outline_rows};

const OUTLINE: &str = "\
Home:
\t- Milk @done(2026-10-01)
\t\tthe organic one
\t- Eggs
Errands: @done
\t- Post office
- Call the bank
";

/// The rows on screen that are not blank.
fn shown(app: &mut Harness) -> Vec<String> {
    outline_rows(app)
        .into_iter()
        .filter(|row| !row.is_empty())
        .collect()
}

const ALL: [&str; 7] = [
    "Home:",
    "  ✔ Milk",
    "    the organic one",
    "  - Eggs",
    "Errands:",
    "  - Post office",
    "- Call the bank",
];

// FR-DONE-02, navigation §2
#[test]
fn ctrl_o_hides_done_blocks_with_their_children_and_shows_them_again() {
    let mut app = Harness::with_text(OUTLINE, 30, 10);
    assert_eq!(shown(&mut app), ALL, "completed blocks show by default");
    app.type_keys("j");
    app.press(KeyCode::Char('o'), KeyModifiers::CONTROL);
    assert_eq!(shown(&mut app), ["Home:", "  - Eggs", "- Call the bank"]);
    assert_eq!(
        focused_rows(&mut app),
        ["  - Eggs"],
        "the next visible block"
    );
    app.press(KeyCode::Char('o'), KeyModifiers::CONTROL);
    assert_eq!(shown(&mut app), ALL);
    assert_eq!(focused_rows(&mut app), ["  - Eggs"]);
}

// FR-DONE-02, FR-DONE-01
#[test]
fn g_c_hides_done_blocks_and_a_block_completed_then_leaves_the_view() {
    let mut app = Harness::with_text(OUTLINE, 30, 10);
    app.type_keys("gc");
    assert_eq!(shown(&mut app), ["Home:", "  - Eggs", "- Call the bank"]);
    app.type_keys("jx");
    assert_eq!(shown(&mut app), ["Home:", "- Call the bank"]);
    assert_eq!(focused_rows(&mut app), ["- Call the bank"]);
    app.type_keys("gc");
    assert_eq!(shown(&mut app)[3], "  ✔ Eggs");
}
