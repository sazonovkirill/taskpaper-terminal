//! The attachments panel: `n` and `N` open the focused block's note and, below it, its comments
//! on a panel down the right side of the screen, and `Esc` closes it; each kind has a setting that
//! turns it off; the note shows rendered and `e` edits it (FR-ATT-03, FR-ATT-04, FR-ATT-06); the
//! comments are added, edited and deleted (FR-ATT-05); the scroll keys go through the selected
//! item and then select the next (D-109).

use std::{fs, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Harness, line};

/// Every screen row, without trailing blanks.
fn screen(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    (buffer.area.top()..buffer.area.bottom())
        .map(|y| line(&buffer, y))
        .collect()
}

/// The saved file once autosave has run.
fn saved(app: &mut Harness) -> String {
    app.wait(Duration::from_secs(1));
    app.settle();
    fs::read_to_string(app.path.clone().expect("a file was opened")).expect("the file is saved")
}

fn press(app: &mut Harness, code: KeyCode) {
    app.press(code, KeyModifiers::NONE);
}

const TEXT: &str = "- Milk\n\tNOTE @hidden\n\t\tFull fat\n\t\t- not skimmed\n- Bread\n";

// FR-ATT-03, attachments.md §3, D-109
#[test]
fn n_opens_the_note_then_the_comments_down_the_right_and_esc_closes() {
    let mut app = Harness::with_text(TEXT, 80, 12);
    let before = screen(&mut app);
    app.type_keys("n");
    assert_eq!(
        screen(&mut app)[..11],
        [
            "  ⌂                                     ┌ Milk ────────────────────────────────┐",
            "────────────────────────────────────────│                               ≡ Note │",
            "▌ - Milk                                │ Full fat                             │",
            "  - Bread                               │ • not skimmed                        │",
            "                                        │                                      │",
            "                                        │                           ❞ Comments │",
            "                                        │ No comments                          │",
            "                                        │                                      │",
            "                                        │                                      │",
            "                                        │ e edit · a comment · Esc close       │",
            "                                        └──────────────────────────────────────┘",
        ]
    );
    let shown = screen(&mut app);
    app.type_keys("dd");
    press(&mut app, KeyCode::Tab);
    assert_eq!(screen(&mut app), shown, "the keys behind it do nothing");
    press(&mut app, KeyCode::Esc);
    assert_eq!(screen(&mut app), before);
}

// FR-ATT-03, attachments.md §3
#[test]
fn shift_n_opens_the_panel_on_any_block_but_a_divider() {
    let mut app = Harness::with_text(
        "- Milk
---
",
        60,
        10,
    );
    app.type_keys("N");
    let rows = screen(&mut app);
    assert!(rows[0].contains("┌ Milk ─"), "{rows:?}");
    assert!(rows[2].contains("│ No note"), "{rows:?}");
    assert!(rows[5].contains("│ No comments"), "{rows:?}");
    press(&mut app, KeyCode::Esc);
    app.type_keys("jn");
    assert!(!screen(&mut app)[0].contains('┌'), "a divider has none");
}

// FR-ATT-06, attachments.md §6
#[test]
fn a_kind_turned_off_has_no_section_or_key() {
    let mut app = Harness::with_config(TEXT, "attachments_notes = false\n", 80, 12);
    app.type_keys("n");
    assert!(!screen(&mut app)[0].contains('┌'), "Open Note is off");
    app.type_keys("N");
    let rows = screen(&mut app);
    assert!(rows[1].ends_with("    ❞ Comments │"), "{rows:?}");
    assert!(rows[2].contains("│ No comments    "), "{rows:?}");
    assert!(rows[9].contains("│ a add · Esc close"), "{rows:?}");
    let mut off = Harness::with_config(
        TEXT,
        "attachments_notes = false\nattachments_comments = false\n",
        80,
        12,
    );
    off.type_keys("nN");
    assert!(!screen(&mut off)[0].contains('┌'));
}

// FR-ATT-04, attachments.md §4
#[test]
fn e_edits_the_note_and_esc_saves_it_as_one_step_to_undo() {
    let mut app = Harness::with_text("- Bread\n", 80, 12);
    app.type_keys("ne# Rye");
    press(&mut app, KeyCode::Enter);
    app.type_keys("- *seeded*");
    let rows = screen(&mut app);
    assert_eq!(
        rows[2..4],
        [
            format!("▌ - Bread{:31}│ # Rye{:32}│", "", ""),
            format!("{:40}│ - *seeded*{:27}│", "", ""),
        ]
    );
    assert!(rows[9].contains("│ Enter new line · Esc save"), "{rows:?}");
    app.terminal.backend_mut().assert_cursor_position((52, 3));
    press(&mut app, KeyCode::Esc);
    let rows = screen(&mut app);
    assert_eq!(
        rows[1..4],
        [
            format!("{}│{:>37} │", "─".repeat(40), "≡ Note"),
            format!("▌ - Bread{:31}│ Rye{:34}│", "", ""),
            format!("{:40}│ • seeded{:29}│", "", ""),
        ]
    );
    press(&mut app, KeyCode::Esc);
    assert_eq!(
        saved(&mut app),
        "- Bread\n\tNOTE @hidden\n\t\t# Rye\n\t\t\u{200B}- *seeded*\n"
    );
    app.type_keys("z");
    assert_eq!(saved(&mut app), "- Bread\n");
}

// FR-ATT-03, FR-ATT-04, attachments.md §3, §4, D-109
#[test]
fn the_scroll_keys_go_through_a_long_note_and_then_select_the_comment() {
    let note = (1..=20)
        .map(|n| format!("\t\tline {n}\n"))
        .collect::<Vec<_>>()
        .concat();
    let mut app = Harness::with_text(
        &format!("- Milk\n\tNOTE @hidden\n{note}\tCOMMENTS @hidden\n\t\tc\n"),
        80,
        12,
    );
    app.type_keys("n");
    screen(&mut app);
    app.type_keys("jj");
    assert_eq!(
        screen(&mut app)[1],
        format!("{:40}│ line 2{:31}│", "─".repeat(40), "")
    );
    app.type_keys("G");
    let rows = screen(&mut app);
    assert_eq!(rows[7], format!("{:40}│ c{:36}│", "", ""), "{rows:?}");
    assert!(rows[9].contains("│ a add · e edit · d del"), "{rows:?}");
    app.type_keys("k");
    assert_eq!(
        screen(&mut app)[1],
        format!("{}│{:>37} │", "─".repeat(40), "≡ Note"),
        "a taller item shows from its start"
    );
    app.press(KeyCode::Char('d'), KeyModifiers::CONTROL);
    assert_eq!(
        screen(&mut app)[1],
        format!("{:40}│ line 7{:31}│", "─".repeat(40), "")
    );
    app.type_keys("k");
    assert_eq!(
        screen(&mut app)[1],
        format!("{:40}│ line 6{:31}│", "─".repeat(40), ""),
        "up through the note while its start is out of view"
    );
    app.press(KeyCode::Char('u'), KeyModifiers::CONTROL);
    assert_eq!(
        screen(&mut app)[1],
        format!("{}│{:>37} │", "─".repeat(40), "≡ Note"),
        "no further than its start"
    );
    app.press(KeyCode::Char('d'), KeyModifiers::CONTROL);
    app.type_keys("jjj");
    app.press(KeyCode::Char('d'), KeyModifiers::CONTROL);
    let rows = screen(&mut app);
    assert_eq!(rows[7], format!("{:40}│ line 20{:30}│", "", ""), "{rows:?}");
    assert!(rows[9].contains("│ e edit · a comment"), "{rows:?}");
    app.type_keys("j");
    let rows = screen(&mut app);
    assert_eq!(rows[7], format!("{:40}│ c{:36}│", "", ""), "{rows:?}");
    app.type_keys("g");
    assert_eq!(
        screen(&mut app)[1],
        format!("{}│{:>37} │", "─".repeat(40), "≡ Note")
    );
}

// FR-ATT-02, FR-ATT-05, attachments.md §2, §5
#[test]
fn a_adds_a_stamped_comment_e_rewords_it_and_d_deletes_it_each_one_step_to_undo() {
    let mut app = Harness::with_text("- Milk\n\tCOMMENTS @hidden\n\t\tTyped by hand\n", 80, 12);
    app.type_keys("Na");
    app.type_keys("Bought **it**");
    let rows = screen(&mut app);
    assert_eq!(rows[5], format!("{:40}│ › Bought **it**{:22}│", "", ""));
    assert!(rows[9].contains("│ Enter add · Esc cancel"), "{rows:?}");
    app.terminal.backend_mut().assert_cursor_position((57, 5));
    press(&mut app, KeyCode::Enter);
    let rows = screen(&mut app);
    assert_eq!(
        rows[2..7],
        [
            format!("▌ - Milk{:32}│ No note{:30}│", "", ""),
            format!("{:40}│{:38}│", "", ""),
            format!("{:40}│{:>37} │", "", "❞ Comments"),
            format!("{:40}│ 2026-10-05 09:30 Bought it{:11}│", "", ""),
            format!("{:40}│ Typed by hand{:24}│", "", ""),
        ]
    );
    assert!(
        rows[9].contains("│ a add · e edit · d del · Esc close"),
        "{rows:?}"
    );
    press(&mut app, KeyCode::PageDown);
    press(&mut app, KeyCode::PageUp);
    app.type_keys("je");
    press(&mut app, KeyCode::End);
    app.type_keys("!");
    press(&mut app, KeyCode::Enter);
    app.type_keys("gaignored");
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Esc);
    assert_eq!(
        saved(&mut app),
        "- Milk\n\tCOMMENTS @hidden\n\t\t[2026-10-05 09:30] Bought **it**\n\t\tTyped by hand!\n"
    );
    app.type_keys("Ndd");
    press(&mut app, KeyCode::Esc);
    assert_eq!(
        saved(&mut app),
        "- Milk\n",
        "the last comment takes its node"
    );
    app.type_keys("z");
    assert_eq!(
        saved(&mut app),
        "- Milk\n\tCOMMENTS @hidden\n\t\tTyped by hand!\n"
    );
    app.type_keys("zzz");
    assert_eq!(
        saved(&mut app),
        "- Milk\n\tCOMMENTS @hidden\n\t\tTyped by hand\n"
    );
}

// FR-ATT-05, attachments.md §5, D-109
#[test]
fn the_scroll_keys_move_the_selected_comment_and_keep_it_in_view() {
    let comments = (1..=8)
        .map(|n| format!("\t\tc{n}\n"))
        .collect::<Vec<_>>()
        .concat();
    let mut app = Harness::with_text(&format!("- Milk\n\tCOMMENTS @hidden\n{comments}"), 80, 12);
    app.type_keys("NG");
    let rows = screen(&mut app);
    assert_eq!(rows[7], format!("{:40}│ c8{:35}│", "", ""), "{rows:?}");
    let buffer = app.render().buffer().clone();
    assert_ne!(buffer[(42, 7)].bg, buffer[(42, 6)].bg, "c8 is selected");
    for _ in 0..7 {
        app.type_keys("k");
        screen(&mut app);
    }
    app.type_keys("d");
    screen(&mut app);
    press(&mut app, KeyCode::PageDown);
    press(&mut app, KeyCode::Down);
    app.type_keys("d");
    screen(&mut app);
    press(&mut app, KeyCode::PageUp);
    app.type_keys("jd");
    press(&mut app, KeyCode::Esc);
    assert_eq!(
        saved(&mut app),
        "- Milk\n\tCOMMENTS @hidden\n\t\tc2\n\t\tc3\n\t\tc6\n\t\tc7\n\t\tc8\n"
    );
}
