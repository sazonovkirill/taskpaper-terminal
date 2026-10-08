//! The Flagged view (FR-TAG-14, D-93): every block with `@flag`, flat, in document order, chosen
//! by `gF` or in the Navigator.

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::{Color, Modifier};

use crate::support::{
    ARROW, BOOK, FLAG, Harness, OVERDUE, PALETTE, STAR, SUN, outline_line, outline_rows,
};

/// Flagged blocks at two depths, a done one among them, and a block that is not flagged.
const OUTLINE: &str = "\
Home:
\t- Water plants @flag
\tErrands:
\t\t- Post office @flag @done(2026-10-04)
- Plan the week
- Call Ann @flag
";

/// The rows on screen below the header that are not blank.
fn shown(app: &mut Harness) -> Vec<String> {
    outline_rows(app)
        .into_iter()
        .filter(|row| !row.is_empty())
        .collect()
}

/// The lines of the open Navigator, between its borders, without trailing blanks.
fn navigator(app: &mut Harness) -> Vec<String> {
    outline_rows(app)
        .into_iter()
        .filter_map(|row| {
            let (_, inside) = row.split_once('│')?;
            let (inside, _) = inside.rsplit_once('│')?;
            Some(inside.trim_end().to_owned())
        })
        .collect()
}

// FR-TAG-14, D-93, FR-DONE-02, navigation §7.1: `gF` lists the flagged blocks under a red flag,
// done ones per show-completed, and Back returns to the outline.
#[test]
fn g_shift_f_lists_the_flagged_blocks_with_their_paths_and_back_returns() {
    let mut app = Harness::with_text(OUTLINE, 60, 10);
    app.type_keys("gF");
    let buffer = app.render().buffer().clone();
    assert_eq!(outline_line(&buffer, 0), format!("{FLAG} Flagged"));
    assert_eq!(buffer[(2, 0)].fg, Color::Red, "the header's flag is red");
    assert!(
        buffer[(4, 0)].modifier.contains(Modifier::BOLD),
        "the header is bold"
    );
    assert_eq!(
        shown(&mut app),
        [
            format!("- Water plants {FLAG}  Home"),
            format!("✔ Post office {FLAG}  Home › Errands"),
            format!("- Call Ann {FLAG}"),
        ]
    );
    app.type_keys("gc");
    assert_eq!(
        shown(&mut app),
        [
            format!("- Water plants {FLAG}  Home"),
            format!("- Call Ann {FLAG}"),
        ],
        "completed blocks hidden"
    );
    app.type_keys("H");
    assert_eq!(outline_line(app.render().buffer(), 0), "⌂");
}

// FR-TAG-14, D-93, D-85, navigation §6: Flagged follows the other special views in the
// Navigator with how many open tasks it lists, and Enter shows it.
#[test]
fn the_navigator_lists_flagged_after_the_other_views_and_enter_shows_it() {
    let mut app = Harness::with_text(OUTLINE, 60, 13);
    app.press(KeyCode::Char('g'), KeyModifiers::CONTROL);
    assert_eq!(
        navigator(&mut app)[..8],
        [
            format!("▌ {SUN} Today"),
            format!("  {ARROW} Upcoming"),
            format!("  {OVERDUE} Overdue"),
            format!("  {PALETTE} Colors"),
            format!("  {BOOK} Logbook"),
            format!("  {STAR} Starred"),
            format!("{:44}2", format!("  {FLAG} Flagged")),
            String::new(),
        ]
    );
    for _ in 0..6 {
        app.press(KeyCode::Down, KeyModifiers::NONE);
    }
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        outline_line(app.render().buffer(), 0),
        format!("{FLAG} Flagged")
    );
    assert_eq!(shown(&mut app).len(), 3);
}

// D-93, ui.md §8: `sidebar_flagged = false` leaves the entry out while `gF` still opens the
// view, and Show Flagged in Navigator in the settings window turns it back on.
#[test]
fn the_flagged_entry_leaves_the_navigator_with_its_setting_off_and_g_shift_f_still_opens_it() {
    let mut app = Harness::with_config(OUTLINE, "sidebar_flagged = false\n", 70, 30);
    app.press(KeyCode::Char('g'), KeyModifiers::CONTROL);
    assert_eq!(
        navigator(&mut app)[..7],
        [
            format!("▌ {SUN} Today"),
            format!("  {ARROW} Upcoming"),
            format!("  {OVERDUE} Overdue"),
            format!("  {PALETTE} Colors"),
            format!("  {BOOK} Logbook"),
            format!("  {STAR} Starred"),
            String::new()
        ]
    );
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    app.type_keys("gF");
    assert_eq!(
        outline_line(app.render().buffer(), 0),
        format!("{FLAG} Flagged")
    );
    app.type_keys("g,jjjjjjjjjjj");
    let screen = outline_rows(&mut app).join("\n");
    assert!(
        screen.contains(&format!("{FLAG} Show Flagged in Navigator")),
        "{screen}"
    );
    assert!(screen.contains("List the Flagged view in the Navigator"));
    app.type_keys(" ");
    app.settle();
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(
        std::fs::read_to_string(app.config_file()).expect("written"),
        "sidebar_flagged = true\n"
    );
    app.press(KeyCode::Char('g'), KeyModifiers::CONTROL);
    assert_eq!(
        navigator(&mut app)[..7],
        [
            format!("  {SUN} Today"),
            format!("  {ARROW} Upcoming"),
            format!("  {OVERDUE} Overdue"),
            format!("  {PALETTE} Colors"),
            format!("  {BOOK} Logbook"),
            format!("  {STAR} Starred"),
            format!("{:44}2", format!("▌ {FLAG} Flagged"))
        ],
        "back on, focused as where the pane is"
    );
}
