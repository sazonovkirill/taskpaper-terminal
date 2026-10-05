//! Step 1.6: zoom, breadcrumbs and zoom history.

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::Modifier;

use crate::support::{Harness, focused_rows, line, outline_rows};

const OUTLINE: &str = "\
Shop:
\t- Milk
\t\torganic
\t- Eggs
Work:
\t- Report
\t\t- Draft
\tIdeas:
";

fn app(width: u16) -> Harness {
    Harness::with_text(OUTLINE, width, 11)
}

/// The rows on screen below the header that are not blank.
fn shown(app: &mut Harness) -> Vec<String> {
    outline_rows(app)
        .into_iter()
        .filter(|row| !row.is_empty())
        .collect()
}

/// The breadcrumb header, the pane's first row.
fn header(app: &mut Harness) -> String {
    line(app.render().buffer(), 0)
}

#[track_caller]
fn assert_focus(app: &mut Harness, expected: &str) {
    assert_eq!(focused_rows(app), [expected]);
}

// FR-NAV-03
#[test]
fn the_header_shows_home_until_zoomed() {
    let mut app = app(30);
    assert_eq!(header(&mut app), "⌂");
    assert!(
        app.render().buffer()[(0, 0)]
            .modifier
            .contains(Modifier::BOLD),
        "the focused pane's header is highlighted"
    );
    assert_eq!(shown(&mut app)[0], "Shop:");
}

// FR-NAV-03
#[test]
fn zooming_in_shows_the_block_as_a_title_over_its_children() {
    let mut app = app(30);
    app.type_keys(".");
    assert_eq!(header(&mut app), "⌂ › Shop");
    assert_eq!(shown(&mut app), ["Shop", "- Milk", "  organic", "- Eggs"]);
    assert_focus(&mut app, "- Milk");
    let title = app.render().buffer()[(2, 2)].clone();
    assert!(title.modifier.contains(Modifier::BOLD), "the title is bold");
}

// FR-NAV-03
#[test]
fn every_zoom_key_zooms_in_and_breadcrumbs_show_the_path() {
    let mut app = app(30);
    app.type_keys("jjjj");
    app.press(KeyCode::Char('l'), KeyModifiers::ALT);
    assert_eq!(header(&mut app), "⌂ › Work");
    app.press(KeyCode::Right, KeyModifiers::ALT);
    assert_eq!(header(&mut app), "⌂ › Work › Report");
    app.type_keys("gz");
    assert_eq!(header(&mut app), "⌂ › Work › Report › Draft");
    assert_eq!(shown(&mut app), ["Draft"]);
    assert_focus(&mut app, "Draft");
}

// FR-NAV-03
#[test]
fn zooming_into_a_leaf_focuses_the_title_and_zooming_on_it_does_nothing() {
    let mut app = app(30);
    app.type_keys("jjj.");
    assert_eq!(shown(&mut app), ["Eggs"]);
    assert_focus(&mut app, "Eggs");
    app.type_keys(".");
    assert_eq!(header(&mut app), "⌂ › Shop › Eggs");
}

// FR-NAV-03
#[test]
fn every_zoom_out_key_goes_to_the_parent_and_focuses_where_you_were() {
    let mut app = app(30);
    app.type_keys("jjjj...");
    assert_eq!(header(&mut app), "⌂ › Work › Report › Draft");
    app.type_keys(">");
    assert_eq!(header(&mut app), "⌂ › Work › Report");
    assert_focus(&mut app, "- Draft");
    app.press(KeyCode::Char('h'), KeyModifiers::ALT);
    assert_eq!(header(&mut app), "⌂ › Work");
    assert_focus(&mut app, "- Report");
    app.press(KeyCode::Left, KeyModifiers::ALT);
    assert_eq!(header(&mut app), "⌂");
    assert_focus(&mut app, "Work:");
    app.type_keys("gu");
    assert_eq!(header(&mut app), "⌂", "home has no parent");
    app.type_keys(".gu");
    assert_focus(&mut app, "Work:");
}

// FR-NAV-03
#[test]
fn g_h_and_ctrl_quote_go_home() {
    let mut app = app(30);
    app.type_keys("jjjj..");
    app.type_keys("gh");
    assert_eq!(header(&mut app), "⌂");
    assert_focus(&mut app, "  - Report");
    app.type_keys("jj.");
    app.press(KeyCode::Char('\''), KeyModifiers::CONTROL);
    assert_eq!(header(&mut app), "⌂");
}

// FR-NAV-04
#[test]
fn back_and_forward_walk_the_zoom_history() {
    let mut app = app(30);
    app.type_keys("jjjj..");
    app.type_keys("H");
    assert_eq!(header(&mut app), "⌂ › Work");
    app.type_keys("H");
    assert_eq!(header(&mut app), "⌂");
    app.type_keys("H");
    assert_eq!(header(&mut app), "⌂", "nothing before the start");
    app.type_keys("L");
    assert_eq!(header(&mut app), "⌂ › Work");
    app.press(KeyCode::Right, KeyModifiers::ALT | KeyModifiers::SHIFT);
    assert_eq!(header(&mut app), "⌂ › Work › Report");
    app.type_keys("L");
    assert_eq!(
        header(&mut app),
        "⌂ › Work › Report",
        "nothing after the end"
    );
    app.press(KeyCode::Left, KeyModifiers::ALT | KeyModifiers::SHIFT);
    assert_eq!(header(&mut app), "⌂ › Work");
    app.type_keys("gh");
    app.type_keys("L");
    assert_eq!(
        header(&mut app),
        "⌂",
        "a new zoom drops the forward history"
    );
}

// FR-NAV-03
#[test]
fn breadcrumbs_too_wide_are_cut_from_the_left() {
    let mut app = app(22);
    app.type_keys("jjjj...");
    assert_eq!(header(&mut app), "… › Report › Draft");
    let mut narrow = app_at_draft(9);
    assert_eq!(header(&mut narrow), "… › Draft");
}

fn app_at_draft(width: u16) -> Harness {
    let mut app = app(width);
    app.type_keys("jjjj...");
    app
}

// FR-NAV-02, FR-NAV-03
#[test]
fn folding_works_under_the_zoom_and_leaves_the_title_alone() {
    let mut app = app(30);
    app.type_keys("jjjj.");
    app.type_keys("_");
    assert_eq!(shown(&mut app), ["Work", "- Report ▸ 1", "Ideas:"]);
    app.type_keys("gp");
    assert_focus(&mut app, "Work");
    app.type_keys("h'");
    assert_eq!(shown(&mut app), ["Work", "- Report ▸ 1", "Ideas:"]);
    app.type_keys("\"");
    assert_eq!(
        shown(&mut app).len(),
        4,
        "the subtree below the title expands"
    );
    app.type_keys("\"");
    assert_eq!(shown(&mut app), ["Work", "- Report ▸ 1", "Ideas:"]);
    app.press(KeyCode::Down, KeyModifiers::CONTROL);
    assert_focus(&mut app, "Work");
    app.type_keys("l");
    assert_focus(&mut app, "- Report ▸ 1");
    app.type_keys("+");
    assert_eq!(shown(&mut app).len(), 4);
    app.type_keys("gh");
    assert_eq!(shown(&mut app)[..2], ["Shop:", "  - Milk"]);
}

// FR-NAV-03
#[test]
fn a_collapsed_block_shows_its_children_when_zoomed() {
    let mut app = app(30);
    app.type_keys("h.");
    assert_eq!(shown(&mut app).len(), 4);
    app.type_keys("gu");
    assert_eq!(shown(&mut app)[0], "Shop: ▸ 2");
}

// FR-NAV-03
#[test]
fn zooming_resets_nothing_in_an_empty_document() {
    let mut app = Harness::with_text("", 20, 4);
    for keys in [".", ">", "gh", "H", "L"] {
        app.type_keys(keys);
    }
    assert_eq!(header(&mut app), "⌂");
    assert_eq!(focused_rows(&mut app), Vec::<String>::new());
}
