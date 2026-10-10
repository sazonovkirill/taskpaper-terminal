//! The Edit Node dialog: `e` and `mi` open it on the focused block, showing every property and
//! tag; its keys change them in a draft that `Enter` writes as one step to undo.

use std::{fs, time::Duration};

use crossterm::event::{Event, KeyCode, KeyModifiers};
use taskpaper_terminal::ui::Input;

use crate::support::{Harness, line, outline_rows};

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

fn press(app: &mut Harness, code: KeyCode) {
    app.press(code, KeyModifiers::NONE);
}

/// Whether the dialog is on screen.
fn open(app: &mut Harness) -> bool {
    screen(app).iter().any(|row| row.contains("Edit Node"))
}

// FR-MODE-09, D-3, FR-TAG-01, FR-TAG-02, FR-SPC-01, editing.md §7
#[test]
fn e_shows_every_property_and_tag_of_the_focused_block_and_esc_closes_it() {
    let mut app = Harness::with_text(
        "- Milk @due(2026-10-08) @estimate(2h)\n\tLOG @hidden @log\n\t\tx\n- Bread\n",
        70,
        22,
    );
    app.type_keys("e");
    assert_eq!(
        screen(&mut app)[4..19],
        [
            "   ┌ Edit Node ───────────────────────────────────────────────────┐",
            "   │▌ Text     Milk                                               │",
            "   │  Type     - Task                                             │",
            "   │  Done     Not done                                           │",
            "   │  Parent   ⌂ Home                                             │",
            "   │  Start    None                                               │",
            "   │  Due      ⇥ Thu 8 Oct 2026 · Thu                             │",
            "   │  Starred  Not starred                                        │",
            "   │  Flagged  Not flagged                                        │",
            "   │  Tags     @estimate(2h) unsupported                          │",
            "   │           + Add tag                                          │",
            "   │  Hidden   @hidden @log · 2 lines                             │",
            "   │                                                              │",
            "   │ Type the text · Tab next · Enter save · Esc cancel           │",
            "   └──────────────────────────────────────────────────────────────┘",
        ]
    );
    press(&mut app, KeyCode::Esc);
    assert!(!open(&mut app));
    app.type_keys("mi");
    assert!(open(&mut app), "m i opens it too");
}

// FR-MODE-09, FR-TAG-01, FR-OPS-08, editing.md §7
#[test]
fn its_keys_change_each_property_and_enter_writes_them_as_one_step_to_undo() {
    let text = "- Milk @due(2026-10-08) @estimate(2h)\n- Bread\n";
    let mut app = Harness::with_text(text, 70, 22);
    app.type_keys("e oat");
    press(&mut app, KeyCode::Tab);
    app.type_keys("p");
    for _ in 0..3 {
        press(&mut app, KeyCode::Tab);
    }
    app.type_keys("2026-10-06");
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Backspace);
    press(&mut app, KeyCode::Tab);
    app.type_keys("*");
    press(&mut app, KeyCode::Tab);
    app.type_keys(" ");
    press(&mut app, KeyCode::Tab);
    press(&mut app, KeyCode::Delete);
    app.type_keys("@context(phone)");
    press(&mut app, KeyCode::Enter);
    let rows = screen(&mut app);
    assert!(
        rows.iter()
            .any(|row| row.contains("│  Tags     @context(phone)")),
        "{rows:#?}"
    );
    assert!(
        rows.contains(
            &"   │▌          + Add tag                                          │".to_owned()
        ),
        "the focus stays on + Add tag for the next one"
    );
    assert!(open(&mut app), "Enter in a field keeps the dialog open");
    press(&mut app, KeyCode::Enter);
    assert!(!open(&mut app));
    assert_eq!(
        saved(&mut app),
        "Milk oat: @start(2026-10-06) @star @flag @context(phone)\n- Bread\n"
    );
    app.type_keys("z");
    assert_eq!(saved(&mut app), text, "one step undoes it all");
}

// FR-MODE-09, editing.md §7
#[test]
fn a_field_says_why_enter_would_not_take_its_text_and_esc_leaves_it() {
    let mut app = Harness::with_text("- Milk\n", 70, 22);
    app.type_keys("e");
    for _ in 0..9 {
        press(&mut app, KeyCode::Tab);
    }
    app.press(KeyCode::BackTab, KeyModifiers::SHIFT);
    press(&mut app, KeyCode::Up);
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Down);
    app.type_keys("a b");
    press(&mut app, KeyCode::Enter);
    let rows = screen(&mut app);
    assert!(
        rows.iter().any(|row| row.contains("│ not a tag name")),
        "{rows:#?}"
    );
    press(&mut app, KeyCode::Esc);
    assert!(open(&mut app), "Esc leaves the field, the dialog open");
    press(&mut app, KeyCode::Esc);
    assert!(!open(&mut app));
    assert_eq!(saved(&mut app), "- Milk\n", "nothing changed");
}

// FR-MODE-09, editing.md §7, §5
#[test]
fn a_date_field_quotes_where_its_text_stops_being_a_date_as_the_date_prompt_does() {
    let mut app = Harness::with_text("- Milk\n", 70, 22);
    app.type_keys("e");
    for _ in 0..5 {
        press(&mut app, KeyCode::Tab);
    }
    app.type_keys("foo");
    let rows = screen(&mut app);
    assert!(
        rows.iter()
            .any(|row| row.contains("│ Not a date from “foo”")),
        "{rows:#?}"
    );
}

// FR-MODE-09, FR-OPS-05, D-56, editing.md §7
#[test]
fn choosing_a_parent_moves_the_block_with_its_subtree_under_it() {
    let mut app = Harness::with_text(
        "Home:\n\t- Paint the hall\nWork:\n\t- Write report\n\t\t- Draft\n",
        70,
        22,
    );
    app.type_keys("jjje");
    for _ in 0..3 {
        press(&mut app, KeyCode::Tab);
    }
    app.type_keys("home");
    let rows = screen(&mut app);
    assert!(rows.iter().any(|row| row.contains("┌Parent")), "{rows:#?}");
    press(&mut app, KeyCode::Enter);
    let rows = screen(&mut app);
    assert!(
        rows.iter().any(|row| row.contains("│▌ Parent   Home ")),
        "{rows:#?}"
    );
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        outline_rows(&mut app)[..5],
        [
            "Home:",
            "  - Paint the hall",
            "  - Write report",
            "    - Draft",
            "Work:",
        ]
    );
}

// FR-MODE-09, D-42, editing.md §7
#[test]
fn ctrl_t_cycles_the_type_from_the_text_row() {
    let mut app = Harness::with_text("- Milk\n", 70, 22);
    app.type_keys("e");
    app.press(KeyCode::Char('t'), KeyModifiers::CONTROL);
    let rows = screen(&mut app);
    assert!(
        rows.iter()
            .any(|row| row.contains("│  Type     ¶ Note") || row.contains("│  Type     Note")),
        "{rows:#?}"
    );
    press(&mut app, KeyCode::Enter);
    assert_eq!(saved(&mut app), "Milk\n");
}

// editing.md §7, ui.md §1
#[test]
fn a_narrow_dialog_keeps_the_parent_path_end_and_a_tag_marker_behind_an_ellipsis() {
    let mut app = Harness::with_text(
        "Alpha project with a long name:\n\tBeta subproject also long:\n\t\tGamma:\n\t\t\t- A task @tag(a very long value that goes on and on)\n",
        50,
        20,
    );
    app.type_keys("Ge");
    let screen = screen(&mut app);
    let row = |label: &str| {
        screen
            .iter()
            .find(|row| row.contains(&format!(" {label:<9}")))
            .cloned()
            .unwrap_or_default()
    };
    assert_eq!(
        row("Parent"),
        "│  Parent   … › Beta subproject also long › Gamma│"
    );
    assert_eq!(
        row("Tags"),
        "│  Tags     @tag(a very long value t… unsupported│"
    );
}

// editing.md §7, D-3
#[test]
fn focusing_the_last_row_of_a_short_dialog_scrolls_its_hidden_rows_into_view() {
    let mut app = Harness::with_text(
        "P:\n\t- a @x @y\n\t\tLOG @hidden\n\t\t\tsome text\n",
        80,
        14,
    );
    app.type_keys("je");
    press(&mut app, KeyCode::Up);
    for _ in 0..12 {
        press(&mut app, KeyCode::Tab);
    }
    let screen = screen(&mut app);
    assert!(
        screen
            .iter()
            .any(|row| row.contains("▌          + Add tag")),
        "{screen:#?}"
    );
    assert!(
        screen
            .iter()
            .any(|row| row.contains("Hidden   @hidden · 2 lines")),
        "{screen:#?}"
    );
}

// editing.md §7, OQ-25
#[test]
fn an_at_sign_typed_in_the_text_row_points_to_the_tags_rows() {
    let mut app = Harness::with_text("- Milk\n", 70, 22);
    app.type_keys("e @");
    let status = screen(&mut app).last().cloned().unwrap_or_default();
    assert!(
        status.contains("Tags are set in the Tags rows below"),
        "{status}"
    );
}

// editing.md §7
#[test]
fn renaming_a_tag_renames_it_in_place_and_keeps_a_duplicate_the_dialog_does_not_list() {
    let mut app = Harness::with_text(
        "P:\n\t- a @context(phone) @x @context(work) @y(1)\n",
        70,
        22,
    );
    app.type_keys("je");
    press(&mut app, KeyCode::Up);
    for _ in 0..8 {
        press(&mut app, KeyCode::Tab);
    }
    app.type_keys(" ");
    app.press(KeyCode::Char('a'), KeyModifiers::CONTROL);
    app.type_keys("place(home)");
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        saved(&mut app),
        "P:\n\t- a @place(home) @x @context(work) @y(1)\n"
    );
}

// FR-MODE-09, FR-OPS-06, editing.md §7
#[test]
fn lines_pasted_into_its_text_are_joined_with_spaces() {
    let mut app = Harness::with_text("- Milk\n", 70, 22);
    app.type_keys("e");
    app.send(Input::Terminal(Event::Paste(" oat\r\nalmond\n".to_owned())));
    press(&mut app, KeyCode::Enter);
    assert_eq!(saved(&mut app), "- Milk oat almond\n");
}
