//! Step 2.5: editing a block's text in place, moving between blocks with `↑`/`↓` (D-43).

use std::{fs, os::unix::fs::PermissionsExt as _, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{DUE, EDIT, Harness, focused_rows, line, outline_rows};

const GROCERIES: &str = "Groceries: @home\n\t- Milk\n\t- Bread\n\t\tWholegrain if they have it.\n\tDairy:\n\t\t- Cheese\nNotes about the week.\n";

/// The status line of `app` after a draw.
fn status_line(app: &mut Harness) -> String {
    let buffer = app.render().buffer().clone();
    line(&buffer, buffer.area.bottom() - 1)
}

/// The open file's text once the autosave delay has passed and its save is done.
fn saved(app: &mut Harness) -> String {
    app.wait(Duration::from_secs(1));
    app.settle();
    let path = app.path.as_ref().expect("a file was opened");
    fs::read_to_string(path).expect("the file is readable")
}

fn press(app: &mut Harness, code: KeyCode, times: usize) {
    for _ in 0..times {
        app.press(code, KeyModifiers::NONE);
    }
}

/// Checks that the terminal's cursor, the caret, is at column `x` of screen row `y`.
#[track_caller]
fn assert_caret(app: &mut Harness, x: u16, y: u16) {
    app.render();
    app.terminal.backend_mut().assert_cursor_position((x, y));
}

// FR-MODE-01, FR-MODE-02, FR-MODE-03, FR-DOC-02
#[test]
fn enter_edits_a_task_in_place_and_esc_returns_with_it_renamed_and_saved() {
    let mut app = Harness::open("groceries.taskpaper", 50, 10);
    app.settle();
    app.type_keys("j");
    press(&mut app, KeyCode::Enter, 1);
    assert!(status_line(&mut app).starts_with(&format!("{EDIT} · ")));
    assert_caret(&mut app, 10, 3);
    press(&mut app, KeyCode::Backspace, 4);
    app.type_keys("Oat milk");
    assert_eq!(outline_rows(&mut app)[1], "  - Oat milk");
    assert_caret(&mut app, 14, 3);
    assert_eq!(saved(&mut app), GROCERIES.replace("Milk", "Oat milk"));
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(status_line(&mut app), app.status(50).trim_end());
    assert_eq!(focused_rows(&mut app), ["  - Oat milk"]);
    app.type_keys("j");
    assert_eq!(focused_rows(&mut app), ["  - Bread"], "keys navigate again");
}

// FR-MODE-01, FR-MODE-04
#[test]
fn home_moves_the_caret_to_the_start_and_enter_ends_editing_with_a_new_block() {
    let mut app = Harness::open("groceries.taskpaper", 50, 10);
    app.type_keys("j");
    press(&mut app, KeyCode::Enter, 1);
    press(&mut app, KeyCode::Home, 1);
    assert_caret(&mut app, 6, 3);
    app.type_keys("Soy ");
    press(&mut app, KeyCode::Enter, 1);
    assert!(status_line(&mut app).starts_with(EDIT));
    assert_eq!(outline_rows(&mut app)[1..3], ["  - Soy Milk", "  -"]);
}

// FR-MODE-03
#[test]
fn text_keys_move_select_and_delete_by_grapheme_and_word() {
    let mut app = Harness::with_text("- one two three\n", 40, 5);
    press(&mut app, KeyCode::Enter, 1);
    app.press(KeyCode::Left, KeyModifiers::CONTROL);
    app.press(KeyCode::Char('w'), KeyModifiers::CONTROL);
    assert_eq!(outline_rows(&mut app)[0], "- one three");
    app.press(KeyCode::Char('b'), KeyModifiers::ALT);
    app.press(KeyCode::Right, KeyModifiers::SHIFT);
    app.press(KeyCode::Right, KeyModifiers::SHIFT);
    app.type_keys("O");
    assert_eq!(outline_rows(&mut app)[0], "- Oe three");
    press(&mut app, KeyCode::Home, 1);
    press(&mut app, KeyCode::Delete, 1);
    app.press(KeyCode::Char('f'), KeyModifiers::ALT);
    app.press(KeyCode::Backspace, KeyModifiers::ALT);
    assert_eq!(outline_rows(&mut app)[0], "-  three");
    app.press(KeyCode::Char('a'), KeyModifiers::CONTROL);
    app.type_keys("é😀");
    press(&mut app, KeyCode::Left, 1);
    press(&mut app, KeyCode::Backspace, 1);
    press(&mut app, KeyCode::End, 1);
    app.press(KeyCode::Home, KeyModifiers::SHIFT);
    app.type_keys("ok");
    app.press(KeyCode::Left, KeyModifiers::CONTROL | KeyModifiers::SHIFT);
    app.press(KeyCode::Right, KeyModifiers::CONTROL | KeyModifiers::SHIFT);
    app.press(KeyCode::End, KeyModifiers::SHIFT);
    app.press(KeyCode::Right, KeyModifiers::CONTROL);
    assert_eq!(outline_rows(&mut app)[0], "- ok");
    assert_caret(&mut app, 6, 2);
}

// FR-TAG-01, OQ-24
#[test]
fn tags_stay_hidden_and_follow_the_edited_text() {
    let mut app = Harness::with_text("- Call @due(2026-10-09) Ana @home\n", 40, 5);
    press(&mut app, KeyCode::Enter, 1);
    assert_eq!(outline_rows(&mut app)[0], "- Call Ana");
    app.type_keys(" back");
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(
        outline_rows(&mut app)[0],
        format!("- Call Ana back{:>23}", format!("{DUE} Fri")),
        "the due date shows only as its chip (FR-TAG-03)"
    );
    assert_eq!(saved(&mut app), "- Call Ana back @due(2026-10-09) @home\n");
}

// OQ-25
#[test]
fn an_at_sign_starting_a_word_is_refused_with_a_hint() {
    let mut app = Harness::with_text("- Mail\n", 100, 5);
    press(&mut app, KeyCode::Enter, 1);
    app.type_keys(" @me@x.org");
    assert_eq!(outline_rows(&mut app)[0], "- Mail me@x.org");
    assert!(
        status_line(&mut app).ends_with(" · Tags are set in Edit Node (e)"),
        "{}",
        status_line(&mut app)
    );
}

// OQ-27
#[test]
fn the_marker_follows_the_type_the_text_gives() {
    let mut app = Harness::with_text("Ideas\n", 40, 5);
    press(&mut app, KeyCode::Enter, 1);
    app.type_keys(":");
    let buffer = app.render().buffer().clone();
    assert_eq!(
        buffer[(2, 2)].style().fg,
        Some(ratatui::style::Color::LightBlue)
    );
    app.press(KeyCode::Char('a'), KeyModifiers::CONTROL);
    app.type_keys("- Call");
    assert_eq!(outline_rows(&mut app)[0], "- Call");
    assert_caret(&mut app, 8, 2);
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(saved(&mut app), "- Call\n");
}

// OQ-27, D-52
#[test]
fn a_dash_or_a_colon_typed_or_deleted_changes_the_type_both_ways() {
    let mut app = Harness::with_text("Ideas @section\n- Milk\nInbox:\n", 40, 6);
    press(&mut app, KeyCode::Enter, 1);
    press(&mut app, KeyCode::Home, 1);
    app.type_keys("- ");
    press(&mut app, KeyCode::Esc, 1);
    app.type_keys("j");
    press(&mut app, KeyCode::Enter, 1);
    press(&mut app, KeyCode::Home, 1);
    press(&mut app, KeyCode::Backspace, 1);
    press(&mut app, KeyCode::Esc, 1);
    app.type_keys("j");
    press(&mut app, KeyCode::Enter, 1);
    press(&mut app, KeyCode::Delete, 1);
    press(&mut app, KeyCode::Esc, 1);
    app.type_keys("k");
    press(&mut app, KeyCode::Enter, 1);
    app.type_keys(":");
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(outline_rows(&mut app)[..3], ["- Ideas", "Milk:", "Inbox"]);
    assert_eq!(
        saved(&mut app),
        "- Ideas @section\nMilk:\nInbox\n",
        "the task keeps the tag, which has no effect on it"
    );
    app.type_keys("k");
    press(&mut app, KeyCode::Enter, 1);
    press(&mut app, KeyCode::Home, 1);
    press(&mut app, KeyCode::Backspace, 1);
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(saved(&mut app), "Ideas @section\nMilk:\nInbox\n");
    let buffer = app.render().buffer().clone();
    assert!(
        buffer[(2, 2)]
            .style()
            .add_modifier
            .contains(ratatui::style::Modifier::ITALIC),
        "a section again, in bold italic"
    );
}

// FR-OPS-08, keybindings.md §3
#[test]
fn ctrl_z_undoes_typing_and_z_undoes_the_whole_session() {
    let mut app = Harness::open("groceries.taskpaper", 50, 10);
    app.type_keys("j");
    press(&mut app, KeyCode::Enter, 1);
    app.type_keys(" x");
    press(&mut app, KeyCode::Backspace, 1);
    app.press(KeyCode::Char('z'), KeyModifiers::CONTROL);
    assert_eq!(outline_rows(&mut app)[1], "  - Milk x");
    app.press(KeyCode::Char('y'), KeyModifiers::CONTROL);
    app.type_keys("2");
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(outline_rows(&mut app)[1], "  - Milk 2");
    app.type_keys("z");
    assert_eq!(outline_rows(&mut app)[1], "  - Milk");
    assert_eq!(saved(&mut app), GROCERIES);
    app.type_keys("Z");
    assert_eq!(outline_rows(&mut app)[1], "  - Milk 2");
}

// file-format §3.1
#[test]
fn an_emptied_parent_is_not_saved_until_leaving_gives_it_the_placeholder() {
    let mut app = Harness::with_text("Notes\n\t- a\n", 40, 5);
    app.settle();
    press(&mut app, KeyCode::Enter, 1);
    app.press(KeyCode::Char('a'), KeyModifiers::CONTROL);
    press(&mut app, KeyCode::Backspace, 1);
    assert_eq!(outline_rows(&mut app)[0], "");
    assert_eq!(saved(&mut app), "Notes\n\t- a\n", "saving waits");
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(outline_rows(&mut app)[0], "…");
    assert_eq!(saved(&mut app), "…\n\t- a\n");
}

// file-io.md §2
#[test]
fn ctrl_q_in_edit_mode_quits_and_saves_the_edit() {
    let mut app = Harness::with_text("- a\n", 40, 5);
    press(&mut app, KeyCode::Enter, 1);
    app.type_keys("b");
    app.press(KeyCode::Char('q'), KeyModifiers::CONTROL);
    app.settle();
    assert!(app.tui.should_quit());
    assert_eq!(app.tui.farewell(), None, "a saved quit has nothing to say");
    assert_eq!(
        fs::read_to_string(app.path.as_ref().unwrap()).unwrap(),
        "- ab\n"
    );
}

// file-io.md §1
#[test]
fn a_read_only_file_refuses_edit_mode() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("todo.taskpaper");
    fs::write(&path, "- Milk\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
    let mut app = Harness::open_path(dir, &path, 90, 4);
    press(&mut app, KeyCode::Enter, 1);
    app.type_keys("x");
    assert!(status_line(&mut app).ends_with("read-only · cannot edit a read-only file"));
    assert_eq!(outline_rows(&mut app), ["- Milk"], "x was not typed");
}

// FR-DOC-05, FR-MODE-02
#[test]
fn the_demo_has_a_task_to_rename() {
    let mut app = Harness::demo(50, 12);
    app.type_keys("j");
    press(&mut app, KeyCode::Enter, 1);
    app.type_keys(" to Lisbon");
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(outline_rows(&mut app)[1], "  - Book flights to Lisbon");
    assert!(saved(&mut app).contains("\t- Book flights to Lisbon\n"));
}

// FR-MODE-03, D-43, FR-DOC-02, D-52
#[test]
fn up_and_down_edit_the_block_above_and_below_at_the_caret_column() {
    let mut app = Harness::with_text("one\n- two\n---\nthree\n", 40, 8);
    press(&mut app, KeyCode::Enter, 1);
    press(&mut app, KeyCode::Down, 1);
    assert_eq!(focused_rows(&mut app), ["- two"]);
    assert_caret(&mut app, 7, 3);
    app.type_keys("!");
    press(&mut app, KeyCode::Down, 1);
    assert_eq!(
        focused_rows(&mut app),
        ["---"],
        "a divider is text, edited raw"
    );
    assert_caret(&mut app, 5, 4);
    press(&mut app, KeyCode::Down, 1);
    assert_eq!(focused_rows(&mut app), ["three"]);
    assert_caret(&mut app, 5, 5);
    app.type_keys("?");
    press(&mut app, KeyCode::Up, 3);
    assert!(status_line(&mut app).starts_with(EDIT));
    assert_caret(&mut app, 5, 2);
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(saved(&mut app), "one\n- two!\n---\nthr?ee\n");
    app.type_keys("z");
    assert_eq!(outline_rows(&mut app)[3], "three", "each block one step");
    assert_eq!(outline_rows(&mut app)[1], "- two!");
    app.type_keys("z");
    assert_eq!(outline_rows(&mut app)[1], "- two");
}

// D-42, D-43
#[test]
fn no_key_breaks_a_line_in_edit_mode() {
    let mut app = Harness::with_text("Ideas\n- t\n", 40, 6);
    press(&mut app, KeyCode::Enter, 1);
    app.press(KeyCode::Enter, KeyModifiers::SHIFT);
    app.press(KeyCode::Enter, KeyModifiers::ALT);
    app.press(KeyCode::Char('j'), KeyModifiers::CONTROL);
    app.type_keys("!");
    assert!(status_line(&mut app).starts_with(EDIT));
    assert_eq!(outline_rows(&mut app)[..2], ["Ideas!", "- t"]);
}

// FR-DOC-05, D-43
#[test]
fn the_demo_has_a_text_line_under_a_task_to_edit_up_from() {
    let mut app = Harness::demo(60, 12);
    app.type_keys("jjj");
    press(&mut app, KeyCode::Enter, 1);
    press(&mut app, KeyCode::Up, 1);
    app.type_keys("!");
    press(&mut app, KeyCode::Esc, 1);
    assert!(saved(&mut app).contains("\t- Reserve a hotel near Alfama! @priority(1)\n"));
}

// OQ-27, editing.md §2
#[test]
fn backspace_in_an_empty_task_takes_off_the_dash_before_deleting_the_block() {
    let mut app = Harness::with_text("Inbox:\n\t- \n", 40, 6);
    app.type_keys("j");
    press(&mut app, KeyCode::Enter, 1);
    press(&mut app, KeyCode::Backspace, 1);
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(
        saved(&mut app),
        "Inbox:\n\n",
        "the task became an empty text block"
    );
    press(&mut app, KeyCode::Enter, 1);
    press(&mut app, KeyCode::Backspace, 1);
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(saved(&mut app), "Inbox:\n", "a second Backspace deletes it");
}

// OQ-27, editing.md §2
#[test]
fn backspace_after_the_dash_of_a_task_reached_with_left_takes_the_dash_off() {
    let mut app = Harness::with_text("Inbox:\n\t- Milk\n", 40, 6);
    app.type_keys("j");
    press(&mut app, KeyCode::Enter, 1);
    press(&mut app, KeyCode::Left, 4);
    press(&mut app, KeyCode::Backspace, 1);
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(saved(&mut app), "Inbox:\n\tMilk\n");
}

// OQ-27, editing.md §2
#[test]
fn backspace_after_a_projects_colon_makes_it_text() {
    let mut app = Harness::with_text("Inbox: @home\n\t- Milk\n", 40, 6);
    press(&mut app, KeyCode::Enter, 1);
    assert_caret(&mut app, 7, 2);
    press(&mut app, KeyCode::End, 1);
    assert_caret(&mut app, 8, 2);
    press(&mut app, KeyCode::Backspace, 1);
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(outline_rows(&mut app)[0], "Inbox");
    assert_eq!(saved(&mut app), "Inbox @home\n\t- Milk\n");
}

// OQ-27, editing.md §2
#[test]
fn typing_as_editing_starts_on_a_project_adds_to_its_name() {
    let mut app = Harness::with_text("Inbox: @home\n", 40, 6);
    press(&mut app, KeyCode::Enter, 1);
    app.type_keys("es");
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(outline_rows(&mut app)[0], "Inboxes:");
    assert_eq!(saved(&mut app), "Inboxes: @home\n");
}

// D-50, editing.md §2.4
#[test]
fn ctrl_t_on_a_project_being_edited_leaves_no_colon_behind() {
    let mut app = Harness::with_text("Inbox: @home\n", 40, 6);
    press(&mut app, KeyCode::Enter, 1);
    app.press(KeyCode::Char('t'), KeyModifiers::CONTROL);
    app.press(KeyCode::Char('t'), KeyModifiers::CONTROL);
    app.press(KeyCode::Char('t'), KeyModifiers::CONTROL);
    app.press(KeyCode::Char('t'), KeyModifiers::CONTROL);
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(
        saved(&mut app),
        "Inbox: @home\n",
        "round the cycle and back"
    );
    press(&mut app, KeyCode::Enter, 1);
    app.press(KeyCode::Char('t'), KeyModifiers::CONTROL);
    press(&mut app, KeyCode::Esc, 1);
    assert_eq!(outline_rows(&mut app)[0], "Inbox");
    assert_eq!(saved(&mut app), "Inbox @home @section\n");
}

// FR-MODE-02, F-FILE-03
#[test]
fn a_tab_inside_the_text_shows_as_a_space_and_is_saved_as_a_tab() {
    let mut app = Harness::with_text("- Name\tPhone\n", 40, 5);
    assert_eq!(outline_rows(&mut app)[0], "- Name Phone");
    press(&mut app, KeyCode::Enter, 1);
    assert_eq!(outline_rows(&mut app)[0], "- Name Phone");
    assert_caret(&mut app, 14, 2);
    press(&mut app, KeyCode::Left, 5);
    app.type_keys("s");
    assert_eq!(outline_rows(&mut app)[0], "- Name sPhone");
    assert_caret(&mut app, 10, 2);
    assert_eq!(saved(&mut app), "- Name\tsPhone\n");
}

// FR-BLK-07, FR-NAV-05, FR-UI-01, D-76: a tab inside the text shows as a space wherever the
// text is drawn, in Go to Symbol's entries and their paths, the Navigator and a divider's title.
#[test]
fn a_tab_inside_the_text_shows_as_a_space_in_the_finders_the_navigator_and_a_divider() {
    let mut app = Harness::with_text("Work\tHome:\n\t- Call\tAnn\n---Next\tweek---\n", 40, 12);
    let rows = |app: &mut Harness| -> Vec<String> {
        let buffer = app.render().buffer().clone();
        (0..buffer.area.height).map(|y| line(&buffer, y)).collect()
    };
    assert_eq!(
        outline_rows(&mut app)[2],
        "───────────── Next week ──────────────"
    );
    app.press(KeyCode::Char('k'), KeyModifiers::CONTROL);
    assert_eq!(
        rows(&mut app)[5..8],
        [
            "│›                                     │",
            "│ ▸ Work Home                          │",
            "│ - Call Ann  Work Home                │",
        ]
    );
    press(&mut app, KeyCode::Esc, 1);
    app.press(KeyCode::Char('g'), KeyModifiers::CONTROL);
    app.type_keys("work");
    assert_eq!(
        rows(&mut app)[6..8],
        [
            "│  Projects                            │",
            "│▌ Work Home                         1 │",
        ]
    );
}

// FR-MODE-02, editing.md §1: text wider than the pane scrolls only as far as keeps the caret in
// view, so the caret moves through the part shown.
#[test]
fn the_caret_moves_through_a_long_line_shown_and_the_text_scrolls_only_past_its_edges() {
    let mut app = Harness::with_text("- abcdefghijklmnopqrstuvwxyz\n", 20, 6);
    press(&mut app, KeyCode::Enter, 1);
    assert_eq!(focused_rows(&mut app), ["- lmnopqrstuvwxyz"]);
    assert_caret(&mut app, 19, 2);
    press(&mut app, KeyCode::Left, 3);
    assert_eq!(
        focused_rows(&mut app),
        ["- lmnopqrstuvwxyz"],
        "the text stays while the caret moves"
    );
    assert_caret(&mut app, 16, 2);
    press(&mut app, KeyCode::Left, 20);
    assert_eq!(
        focused_rows(&mut app),
        ["- defghijklmnopqrs"],
        "past the left edge the text scrolls to the caret"
    );
    assert_caret(&mut app, 4, 2);
    press(&mut app, KeyCode::Right, 3);
    assert_eq!(focused_rows(&mut app), ["- defghijklmnopqrs"]);
    assert_caret(&mut app, 7, 2);
}

/// Opens a dialog or panel with a text field.
type Opens = fn(&mut Harness);

// FR-MODE-02, editing.md §1: the one-line fields of the dialogs and panels keep their text still
// too while the caret moves left through the part shown.
#[test]
fn every_text_field_moves_its_caret_through_the_text_shown() {
    let long = "a long text that is much wider than the room the field has for it";
    let open: [(&str, Opens); 7] = [
        ("date prompt", |app| app.type_keys("md")),
        ("Quick Add", |app| app.type_keys("qq")),
        ("Edit Node", |app| app.type_keys("e")),
        ("settings filter", |app| app.type_keys("g,/")),
        ("note", |app| app.type_keys("ne")),
        ("comment", |app| app.type_keys("Na")),
        ("Save As", |app| {
            app.press(KeyCode::Char('p'), KeyModifiers::CONTROL);
            app.type_keys("save as");
            press(app, KeyCode::Enter, 1);
            press(app, KeyCode::Backspace, 200);
        }),
    ];
    for (field, open) in open {
        let mut app = Harness::with_text("- Milk\n", 50, 14);
        open(&mut app);
        app.type_keys(long);
        app.render();
        let at_end = app.terminal.get_cursor_position().expect("a caret");
        press(&mut app, KeyCode::Left, 3);
        app.render();
        let moved = app.terminal.get_cursor_position().expect("a caret");
        assert_eq!(
            (moved.x, moved.y),
            (at_end.x - 3, at_end.y),
            "the {field}'s caret moves left"
        );
    }
}
