//! Step 4.5: showing and hiding completed blocks.

use std::{fs, time::Duration};

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

/// The open file's text once the autosave delay has passed and its save is done.
fn saved(app: &mut Harness) -> String {
    app.wait(Duration::from_secs(1));
    app.settle();
    let path = app.path.as_ref().expect("a file was opened");
    fs::read_to_string(path).expect("the file is readable")
}

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
fn g_c_hides_done_blocks_with_their_children_and_shows_them_again() {
    let mut app = Harness::with_text(OUTLINE, 30, 10);
    assert_eq!(shown(&mut app), ALL, "completed blocks show by default");
    app.type_keys("j");
    app.type_keys("gc");
    assert_eq!(shown(&mut app), ["Home:", "  - Eggs", "- Call the bank"]);
    assert_eq!(
        focused_rows(&mut app),
        ["  - Eggs"],
        "the next visible block"
    );
    app.type_keys("gc");
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

const ERRANDS: &str = "\
Errands:
\t- Bank @done(2026-10-01)
\t- Post office
\t- Pharmacy @done(2026-10-02)
\t- Hardware store
";

// FR-DONE-02, FR-OPS-02, FR-OPS-03
#[test]
fn with_done_blocks_hidden_tab_and_alt_j_k_pass_over_them() {
    let mut app = Harness::with_text(ERRANDS, 30, 10);
    app.type_keys("gcG");
    app.press(KeyCode::Char('k'), KeyModifiers::ALT);
    assert_eq!(
        shown(&mut app),
        ["Errands:", "  - Hardware store", "  - Post office"],
        "one press moves it above the block shown above it"
    );
    assert_eq!(focused_rows(&mut app), ["  - Hardware store"]);
    app.press(KeyCode::Char('j'), KeyModifiers::ALT);
    app.press(KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(
        shown(&mut app),
        ["Errands:", "  - Post office", "    - Hardware store"],
        "under the block shown above it"
    );
    assert_eq!(focused_rows(&mut app), ["    - Hardware store"]);
    assert_eq!(
        saved(&mut app),
        "Errands:\n\t- Bank @done(2026-10-01)\n\t- Post office\n\t\t- Hardware store\n\t- Pharmacy @done(2026-10-02)\n"
    );
}

// FR-DONE-02, editing.md §3
#[test]
fn enter_on_a_block_whose_children_are_all_hidden_adds_its_next_sibling() {
    let mut app = Harness::with_text("- Pack\n\t- Socks @done\n- Leave\n", 30, 10);
    app.type_keys("gc");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    app.type_keys("Shoes");
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(shown(&mut app), ["- Pack", "- Shoes", "- Leave"]);
    assert_eq!(
        saved(&mut app),
        "- Pack\n\t- Socks @done\n- Shoes\n- Leave\n"
    );
}
