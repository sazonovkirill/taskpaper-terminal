//! The Edit Node dialog: `e` and `mi` open it on the focused block, showing every property and
//! tag; its keys change them in a draft that `Enter` writes as one step to undo.

use std::{fs, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};

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
