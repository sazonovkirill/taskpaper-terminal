//! Step 1.5: folding, and parent and sibling motion.

use crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Harness, focused_rows, outline_rows};

const NONE: KeyModifiers = KeyModifiers::NONE;

const OUTLINE: &str = "\
Home:
\t- Milk
\t\torganic
\t- Eggs
Work:
\t- Report
\t\t- Draft
\t\t\t- Outline
\tIdeas:
- Loose @done
";

fn app() -> Harness {
    Harness::with_text(OUTLINE, 30, 13)
}

/// The rows on screen that are not blank.
fn shown(app: &mut Harness) -> Vec<String> {
    outline_rows(app)
        .into_iter()
        .filter(|row| !row.is_empty())
        .collect()
}

#[track_caller]
fn assert_focus(app: &mut Harness, expected: &str) {
    assert_eq!(focused_rows(app), [expected]);
}

// FR-NAV-02
#[test]
fn a_file_opens_fully_expanded() {
    let mut app = app();
    assert_eq!(
        shown(&mut app),
        [
            "  Home:",
            "  ☐ Milk",
            "      organic",
            "  ☐ Eggs",
            "  Work:",
            "  ☐ Report",
            "    ☐ Draft",
            "      ☐ Outline",
            "    Ideas:",
            "☑ Loose",
        ]
    );
}

// FR-NAV-02
#[test]
fn h_collapses_a_project() {
    let mut app = app();
    app.type_keys("h");
    assert_eq!(
        shown(&mut app)[..3],
        ["  Home: ▸ 2", "  Work:", "  ☐ Report"]
    );
    assert_focus(&mut app, "  Home: ▸ 2");
    app.press(KeyCode::Left, NONE);
    assert_focus(&mut app, "  Home: ▸ 2");
    assert_eq!(shown(&mut app)[0], "  Home: ▸ 2");
}

// FR-NAV-02
#[test]
fn a_collapsed_block_shows_how_many_children_it_hides() {
    let mut app = app();
    app.type_keys("j");
    app.press(KeyCode::Left, NONE);
    assert_eq!(
        shown(&mut app)[..3],
        ["  Home:", "  ☐ Milk ▸ 1", "  ☐ Eggs"]
    );
}

// FR-NAV-01, FR-NAV-02
#[test]
fn h_on_a_leaf_or_a_collapsed_block_goes_to_its_parent() {
    let mut app = app();
    app.type_keys("jj");
    assert_focus(&mut app, "      organic");
    app.type_keys("h");
    assert_focus(&mut app, "  ☐ Milk");
    app.type_keys("h");
    assert_focus(&mut app, "  ☐ Milk ▸ 1");
    app.type_keys("h");
    assert_focus(&mut app, "  Home:");
}

// FR-NAV-01, FR-NAV-02
#[test]
fn l_expands_then_goes_to_the_first_child() {
    let mut app = app();
    app.type_keys("h");
    app.type_keys("l");
    assert_eq!(shown(&mut app)[..2], ["  Home:", "  ☐ Milk"]);
    assert_focus(&mut app, "  Home:");
    app.press(KeyCode::Right, NONE);
    assert_focus(&mut app, "  ☐ Milk");
    app.type_keys("jjl");
    assert_focus(&mut app, "  ☐ Eggs");
}

// FR-NAV-02
#[test]
fn quote_and_space_toggle_the_fold() {
    let mut app = app();
    app.type_keys("'");
    assert_focus(&mut app, "  Home: ▸ 2");
    app.type_keys(" ");
    assert_focus(&mut app, "  Home:");
    app.type_keys("jjj");
    app.type_keys("'");
    assert_focus(&mut app, "  ☐ Eggs");
    assert_eq!(shown(&mut app).len(), 10, "a leaf has nothing to fold");
}

// FR-NAV-02
#[test]
fn double_quote_toggles_the_whole_subtree() {
    let mut app = app();
    app.type_keys("jjjj");
    app.type_keys("\"");
    assert_eq!(shown(&mut app)[4..], ["  Work: ▸ 2", "☑ Loose"]);
    app.type_keys("l");
    assert_eq!(
        shown(&mut app)[4..],
        ["  Work:", "  ☐ Report ▸ 1", "    Ideas:", "☑ Loose"]
    );
    app.type_keys("\"");
    assert_eq!(shown(&mut app)[4..], ["  Work: ▸ 2", "☑ Loose"]);
    app.type_keys("\"");
    assert_eq!(shown(&mut app).len(), 10);
}

// FR-NAV-02
#[test]
fn underscore_and_plus_collapse_and_expand_everything() {
    let mut app = app();
    app.type_keys("_");
    assert_eq!(shown(&mut app), ["  Home: ▸ 2", "  Work: ▸ 2", "☑ Loose"]);
    app.type_keys("+");
    assert_eq!(shown(&mut app).len(), 10);
}

// FR-NAV-02
#[test]
fn minus_and_equals_fold_one_level_at_a_time() {
    let mut app = app();
    app.type_keys("-");
    assert_eq!(shown(&mut app)[5..7], ["  ☐ Report", "    ☐ Draft ▸ 1"]);
    app.type_keys("-");
    assert_eq!(
        shown(&mut app),
        [
            "  Home:",
            "  ☐ Milk ▸ 1",
            "  ☐ Eggs",
            "  Work:",
            "  ☐ Report ▸ 1",
            "    Ideas:",
            "☑ Loose",
        ]
    );
    app.type_keys("-");
    app.type_keys("-");
    assert_eq!(shown(&mut app), ["  Home: ▸ 2", "  Work: ▸ 2", "☑ Loose"]);
    app.type_keys("=");
    assert_eq!(shown(&mut app)[4], "  ☐ Report ▸ 1");
    app.type_keys("=");
    assert_eq!(shown(&mut app)[6], "    ☐ Draft ▸ 1");
    app.type_keys("==");
    assert_eq!(shown(&mut app).len(), 10);
}

// FR-NAV-01, navigation §2
#[test]
fn focus_hidden_by_a_fold_moves_to_the_next_visible_block() {
    let mut app = app();
    app.type_keys("jjjjjjj");
    assert_focus(&mut app, "      ☐ Outline");
    app.type_keys("_");
    assert_focus(&mut app, "☑ Loose");
}

// FR-NAV-01, navigation §2
#[test]
fn without_a_next_visible_block_focus_moves_to_the_previous_one() {
    let mut app = Harness::with_text("A:\n\t- b\n\t\t- c\n", 30, 6);
    app.type_keys("jj");
    app.type_keys("_");
    assert_focus(&mut app, "  A: ▸ 1");
}

// FR-NAV-01
#[test]
fn g_p_goes_to_the_parent() {
    let mut app = app();
    app.type_keys("jjjjjjj");
    app.type_keys("gp");
    assert_focus(&mut app, "    ☐ Draft");
    app.type_keys("gpgp");
    assert_focus(&mut app, "  Work:");
    app.type_keys("gp");
    assert_focus(&mut app, "  Work:");
}

// FR-NAV-01
#[test]
fn g_l_goes_to_the_last_child() {
    let mut app = app();
    app.type_keys("jjjjh");
    app.type_keys("gl");
    assert_focus(&mut app, "    Ideas:");
    assert_eq!(shown(&mut app).len(), 10, "Work expanded again");
    app.type_keys("gl");
    assert_focus(&mut app, "    Ideas:");
}

// FR-NAV-01
#[test]
fn ctrl_arrows_move_between_siblings() {
    let mut app = app();
    app.press(KeyCode::Down, KeyModifiers::CONTROL);
    assert_focus(&mut app, "  Work:");
    app.press(KeyCode::Down, KeyModifiers::CONTROL);
    assert_focus(&mut app, "☑ Loose");
    app.press(KeyCode::Down, KeyModifiers::CONTROL);
    assert_focus(&mut app, "☑ Loose");
    app.press(KeyCode::Up, KeyModifiers::CONTROL);
    app.press(KeyCode::Up, KeyModifiers::CONTROL);
    assert_focus(&mut app, "  Home:");
    app.press(KeyCode::Up, KeyModifiers::CONTROL);
    assert_focus(&mut app, "  Home:");
    app.type_keys("j");
    app.press(KeyCode::Down, KeyModifiers::CONTROL);
    assert_focus(&mut app, "  ☐ Eggs");
    app.press(KeyCode::Down, KeyModifiers::CONTROL);
    assert_focus(&mut app, "  ☐ Eggs");
}

// FR-SPC-01, special-nodes §1
#[test]
fn a_block_whose_only_children_are_special_nodes_is_a_leaf() {
    let mut app = Harness::with_text("A:\n\t- a\n\t\tC @hidden @comments\n\t\t\tx\n", 30, 5);
    app.type_keys("j");
    app.type_keys("'");
    assert_focus(&mut app, "  ☐ a");
    app.type_keys("h");
    assert_focus(&mut app, "  A:");
}
