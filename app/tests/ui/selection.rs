//! Step 3.5: selecting blocks with `Shift`+arrows, `v`, `V` and `Ctrl+A`, clearing with `Esc`,
//! every command acting on the selection, `J`/`K` moving it and Group grouping it.

use std::{fs, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Harness, NAVIGATE, focused_rows, line, outline_rows, selected_rows};

const ERRANDS: &str =
    "Errands:\n\t- Post office\n\t- Bank\n\t- Pharmacy\n\t- Bakery\n\t- Hardware store\nLater\n";

/// The open file's text once the autosave delay has passed and its save is done.
fn saved(app: &mut Harness) -> String {
    app.wait(Duration::from_secs(1));
    app.settle();
    let path = app.path.as_ref().expect("a file was opened");
    fs::read_to_string(path).expect("the file is readable")
}

fn shift(app: &mut Harness, code: KeyCode) {
    app.press(code, KeyModifiers::SHIFT);
}

// FR-OPS-01, FR-OPS-07, FR-DONE-01, FR-OPS-08
#[test]
fn shift_down_selects_five_tasks_and_x_completes_them_at_once() {
    let mut app = Harness::with_text(ERRANDS, 40, 10);
    app.type_keys("j");
    app.press(KeyCode::Down, KeyModifiers::SHIFT);
    app.press(KeyCode::Down, KeyModifiers::SHIFT);
    app.press(KeyCode::Down, KeyModifiers::SHIFT);
    shift(&mut app, KeyCode::Down);
    assert_eq!(
        selected_rows(&mut app),
        ["  - Post office", "  - Bank", "  - Pharmacy", "  - Bakery"]
    );
    assert_eq!(focused_rows(&mut app), ["  - Hardware store"]);
    app.type_keys("x");
    assert_eq!(
        saved(&mut app),
        ERRANDS
            .replace("\t- Post office", "\t- Post office @done(2026-10-05)")
            .replace("\t- Bank", "\t- Bank @done(2026-10-05)")
            .replace("\t- Pharmacy", "\t- Pharmacy @done(2026-10-05)")
            .replace("\t- Bakery", "\t- Bakery @done(2026-10-05)")
            .replace("\t- Hardware store", "\t- Hardware store @done(2026-10-05)")
    );
    app.type_keys("z");
    assert_eq!(saved(&mut app), ERRANDS, "one step to undo");
}

// FR-OPS-01
#[test]
fn shift_up_extends_upward_and_shift_down_shrinks_the_run() {
    let mut app = Harness::with_text(ERRANDS, 40, 10);
    app.type_keys("G");
    shift(&mut app, KeyCode::Up);
    shift(&mut app, KeyCode::Up);
    shift(&mut app, KeyCode::Up);
    assert_eq!(
        selected_rows(&mut app),
        ["  - Bakery", "  - Hardware store", "Later"]
    );
    assert_eq!(focused_rows(&mut app), ["  - Pharmacy"]);
    shift(&mut app, KeyCode::Down);
    assert_eq!(
        selected_rows(&mut app),
        ["  - Hardware store", "Later"],
        "back down shrinks the run"
    );
}

// FR-OPS-02, FR-OPS-07, D-42, D-139: `J`/`K` move the selection as `Alt+j`/`Alt+k` do, whether
// a terminal sends the capital or, under the kitty protocol, the letter with `Shift`.
#[test]
fn shift_j_and_shift_k_move_the_selection_down_and_up() {
    let mut app = Harness::with_text(ERRANDS, 40, 10);
    app.type_keys("j");
    shift(&mut app, KeyCode::Down);
    app.type_keys("J");
    assert_eq!(
        outline_rows(&mut app)[1..4],
        ["  - Pharmacy", "  - Post office", "  - Bank"]
    );
    app.press(KeyCode::Char('j'), KeyModifiers::SHIFT);
    assert_eq!(
        outline_rows(&mut app)[1..5],
        ["  - Pharmacy", "  - Bakery", "  - Post office", "  - Bank"]
    );
    app.type_keys("K");
    app.press(KeyCode::Char('k'), KeyModifiers::SHIFT);
    assert_eq!(saved(&mut app), ERRANDS, "back where they were");
    assert_eq!(
        selected_rows(&mut app),
        ["  - Post office"],
        "the run stays"
    );
    assert_eq!(focused_rows(&mut app), ["  - Bank"]);
}

// FR-OPS-01
#[test]
fn v_picks_blocks_one_by_one_ctrl_a_takes_all_and_esc_clears() {
    let mut app = Harness::with_text(ERRANDS, 40, 10);
    app.type_keys("jvjjv");
    assert_eq!(selected_rows(&mut app), ["  - Post office"]);
    assert_eq!(focused_rows(&mut app), ["  - Pharmacy"]);
    app.type_keys("j");
    assert_eq!(
        selected_rows(&mut app),
        ["  - Post office", "  - Pharmacy"],
        "moving keeps the selection"
    );
    app.type_keys("T");
    assert_eq!(
        saved(&mut app),
        ERRANDS
            .replace("\t- Post office", "\t- Post office @due(2026-10-05)")
            .replace("\t- Pharmacy", "\t- Pharmacy @due(2026-10-05)")
    );
    app.press(KeyCode::Char('a'), KeyModifiers::CONTROL);
    assert_eq!(selected_rows(&mut app).len(), 6);
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(selected_rows(&mut app), Vec::<String>::new());
    assert_eq!(focused_rows(&mut app), ["  - Bakery"]);
}

/// The status line's text.
fn status_line(app: &mut Harness) -> String {
    let buffer = app.render().buffer().clone();
    line(&buffer, buffer.area.bottom() - 1)
}

// FR-OPS-01, FR-OPS-07, D-135
#[test]
fn shift_v_selects_from_where_it_began_to_the_focus_until_a_command_acts() {
    let mut app = Harness::with_text(ERRANDS, 40, 10);
    app.type_keys("jjjVkk");
    assert!(status_line(&mut app).starts_with("↕ VISUAL"));
    assert_eq!(selected_rows(&mut app), ["  - Bank", "  - Pharmacy"]);
    assert_eq!(focused_rows(&mut app), ["  - Post office"]);
    app.type_keys("jjj");
    assert_eq!(selected_rows(&mut app), ["  - Pharmacy"]);
    assert_eq!(focused_rows(&mut app), ["  - Bakery"]);
    app.type_keys("T");
    assert!(
        status_line(&mut app).starts_with(NAVIGATE),
        "the command ends it"
    );
    app.type_keys("j");
    let selected = selected_rows(&mut app);
    let texts: Vec<_> = selected
        .iter()
        .map(|row| row.trim_end_matches("⇥ today").trim_end())
        .collect();
    assert_eq!(texts, ["  - Pharmacy", "  - Bakery"], "the selection stays");
    assert_eq!(
        saved(&mut app),
        ERRANDS
            .replace("\t- Pharmacy", "\t- Pharmacy @due(2026-10-05)")
            .replace("\t- Bakery", "\t- Bakery @due(2026-10-05)")
    );
    app.type_keys("ggVj");
    assert_eq!(selected_rows(&mut app), ["Errands:"]);
    app.type_keys("V");
    assert!(status_line(&mut app).starts_with(NAVIGATE));
    assert_eq!(
        selected_rows(&mut app),
        Vec::<String>::new(),
        "V again clears"
    );
    app.type_keys("Vk");
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    assert!(status_line(&mut app).starts_with(NAVIGATE));
    assert_eq!(selected_rows(&mut app), Vec::<String>::new(), "so does Esc");
}

// FR-OPS-04, FR-OPS-08, D-139
#[test]
fn group_from_go_to_command_groups_the_selection_under_a_new_block_to_name() {
    let mut app = Harness::with_text(ERRANDS, 40, 10);
    app.type_keys("j");
    app.press(KeyCode::Down, KeyModifiers::SHIFT);
    app.type_keys(":group");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        outline_rows(&mut app)[..4],
        ["Errands:", "", "    - Post office", "    - Bank"]
    );
    app.type_keys("Town");
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(
        saved(&mut app),
        ERRANDS.replace(
            "\t- Post office\n\t- Bank\n",
            "\tTown\n\t\t- Post office\n\t\t- Bank\n"
        )
    );
    app.type_keys("z");
    assert_eq!(saved(&mut app), ERRANDS, "one step to undo");
}

// FR-OPS-02, FR-OPS-03, FR-OPS-06, FR-OPS-07
#[test]
fn structure_keys_move_and_delete_the_whole_selection() {
    let mut app = Harness::with_text(ERRANDS, 40, 10);
    app.type_keys("jj");
    app.press(KeyCode::Down, KeyModifiers::SHIFT);
    app.press(KeyCode::Char('k'), KeyModifiers::ALT);
    assert_eq!(
        outline_rows(&mut app)[1..4],
        ["  - Bank", "  - Pharmacy", "  - Post office"]
    );
    app.press(KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(
        outline_rows(&mut app)[1..3],
        ["  - Bank", "  - Pharmacy"],
        "the first of the list cannot indent, so neither does the one after it"
    );
    app.press(KeyCode::Char('j'), KeyModifiers::ALT);
    app.press(KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(
        outline_rows(&mut app)[1..4],
        ["  - Post office", "    - Bank", "    - Pharmacy"]
    );
    app.press(KeyCode::BackTab, KeyModifiers::SHIFT);
    app.type_keys("dd");
    assert_eq!(
        saved(&mut app),
        "Errands:\n\t- Post office\n\t- Bakery\n\t- Hardware store\nLater\n"
    );
    assert_eq!(focused_rows(&mut app), ["  - Bakery"]);
}

// FR-OPS-07, FR-TAG-04, FR-TAG-05, D-11, D-48, D-138
#[test]
fn metadata_keys_mark_every_selected_block() {
    let mut app = Harness::with_text(ERRANDS, 40, 10);
    app.type_keys("j");
    app.press(KeyCode::Down, KeyModifiers::SHIFT);
    app.type_keys("mc1");
    app.type_keys("s");
    app.type_keys("b");
    assert_eq!(
        saved(&mut app),
        ERRANDS
            .replace("\t- Post office", "\t- Post office @red @star @bookmark")
            .replace("\t- Bank", "\t- Bank @red @star @bookmark")
    );
}

// FR-OPS-07, FR-TAG-03
#[test]
fn m_d_dates_every_selected_block_with_the_date_typed() {
    let mut app = Harness::with_text(ERRANDS, 40, 10);
    app.type_keys("j");
    app.press(KeyCode::Down, KeyModifiers::SHIFT);
    app.type_keys("mdfri");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        saved(&mut app),
        ERRANDS
            .replace("\t- Post office", "\t- Post office @due(2026-10-09)")
            .replace("\t- Bank", "\t- Bank @due(2026-10-09)")
    );
}
