//! The attachments panel: `n` and `N` open the focused block's note and comments on a panel
//! over the right side of the screen, `Tab` switches between them and `Esc` closes it; each kind
//! has a setting that turns it off; the note shows rendered, scrolls, and `e` edits it
//! (FR-ATT-03, FR-ATT-04, FR-ATT-06); the comments tab adds, edits and deletes comments
//! (FR-ATT-05).

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

// FR-ATT-03, attachments.md §3
#[test]
fn n_opens_the_note_tab_over_the_right_tab_switches_and_esc_closes() {
    let mut app = Harness::with_text(TEXT, 80, 12);
    let before = screen(&mut app);
    app.type_keys("n");
    assert_eq!(
        screen(&mut app)[1..11],
        [
            "────────────────────────────────────────┌ Milk ────────────────────────────────┐",
            "▌ - Milk ≡                              │ ≡ Note   ❞ Comments                  │",
            "  - Bread                               │                                      │",
            "                                        │ Full fat                             │",
            "                                        │ • not skimmed                        │",
            "                                        │                                      │",
            "                                        │                                      │",
            "                                        │                                      │",
            "                                        │ Tab other tab · e edit · Esc close   │",
            "                                        └──────────────────────────────────────┘",
        ]
    );
    app.type_keys("dd");
    press(&mut app, KeyCode::Tab);
    assert_eq!(
        screen(&mut app)[2..5],
        [
            "▌ - Milk ≡                              │ ≡ Note   ❞ Comments                  │",
            "  - Bread                               │                                      │",
            "                                        │ No comments                          │",
        ],
        "the keys behind it do nothing"
    );
    app.press(KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(
        screen(&mut app)[4],
        format!("{:40}│ Full fat{:29}│", "", "")
    );
    press(&mut app, KeyCode::Esc);
    assert_eq!(screen(&mut app), before);
}

// FR-ATT-03, attachments.md §3
#[test]
fn shift_n_opens_the_comments_tab_on_any_block_but_a_divider() {
    let mut app = Harness::with_text(
        "- Milk
---
",
        60,
        10,
    );
    app.type_keys("N");
    let rows = screen(&mut app);
    assert!(rows[1].contains("┌ Milk ─"), "{rows:?}");
    assert!(rows[4].contains("│ No comments"), "{rows:?}");
    press(&mut app, KeyCode::Esc);
    app.type_keys("jn");
    assert!(!screen(&mut app)[1].contains('┌'), "a divider has none");
}

// FR-ATT-06, attachments.md §6
#[test]
fn a_kind_turned_off_has_no_tab_or_key() {
    let mut app = Harness::with_config(TEXT, "attachments_notes = false\n", 80, 12);
    app.type_keys("n");
    assert!(!screen(&mut app)[1].contains('┌'), "Open Note is off");
    app.type_keys("N");
    let rows = screen(&mut app);
    assert!(rows[2].contains("│ ❞ Comments    "), "{rows:?}");
    assert!(rows[9].contains("│ a add · Esc close"), "{rows:?}");
    let mut off = Harness::with_config(
        TEXT,
        "attachments_notes = false\nattachments_comments = false\n",
        80,
        12,
    );
    off.type_keys("nN");
    assert!(!screen(&mut off)[1].contains('┌'));
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
        rows[4..6],
        [
            format!("{:40}│ # Rye{:32}│", "", ""),
            format!("{:40}│ - *seeded*{:27}│", "", ""),
        ]
    );
    assert!(rows[9].contains("│ Enter new line · Esc save"), "{rows:?}");
    app.terminal.backend_mut().assert_cursor_position((52, 5));
    press(&mut app, KeyCode::Esc);
    let rows = screen(&mut app);
    assert_eq!(
        rows[2..6],
        [
            format!("▌ - Bread ≡{:29}│ ≡ Note   ❞ Comments{:18}│", "", ""),
            format!("{:40}│{:38}│", "", ""),
            format!("{:40}│ Rye{:34}│", "", ""),
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

// FR-ATT-04, attachments.md §4, D-107
#[test]
fn j_k_and_g_scroll_a_long_note() {
    let note = (1..=20)
        .map(|n| format!("\t\tline {n}\n"))
        .collect::<Vec<_>>()
        .concat();
    let mut app = Harness::with_text(&format!("- Milk\n\tNOTE @hidden\n{note}"), 80, 12);
    app.type_keys("njj");
    assert_eq!(screen(&mut app)[4], format!("{:40}│ line 3{:31}│", "", ""));
    app.type_keys("G");
    assert_eq!(screen(&mut app)[8], format!("{:40}│ line 20{:30}│", "", ""));
    app.type_keys("kg");
    assert_eq!(screen(&mut app)[4], format!("{:40}│ line 1{:31}│", "", ""));
    app.press(KeyCode::Char('d'), KeyModifiers::CONTROL);
    assert_eq!(screen(&mut app)[4], format!("{:40}│ line 6{:31}│", "", ""));
    press(&mut app, KeyCode::Tab);
    screen(&mut app);
    press(&mut app, KeyCode::Tab);
    assert_eq!(
        screen(&mut app)[4],
        format!("{:40}│ line 1{:31}│", "", ""),
        "another tab starts at the top"
    );
}

// FR-ATT-02, FR-ATT-05, attachments.md §2, §5
#[test]
fn a_adds_a_stamped_comment_e_rewords_it_and_d_deletes_it_each_one_step_to_undo() {
    let mut app = Harness::with_text("- Milk\n\tCOMMENTS @hidden\n\t\tTyped by hand\n", 80, 12);
    app.type_keys("Na");
    app.type_keys("Bought **it**");
    let rows = screen(&mut app);
    assert_eq!(rows[4], format!("{:40}│ › Bought **it**{:22}│", "", ""));
    assert!(rows[9].contains("│ Enter add · Esc cancel"), "{rows:?}");
    app.terminal.backend_mut().assert_cursor_position((57, 4));
    press(&mut app, KeyCode::Enter);
    let rows = screen(&mut app);
    assert_eq!(
        rows[2..6],
        [
            format!("▌ - Milk ❞ 2{:28}│ ≡ Note   ❞ Comments{:18}│", "", ""),
            format!("{:40}│{:38}│", "", ""),
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

// FR-ATT-05, attachments.md §5, D-108
#[test]
fn the_scroll_keys_move_the_selected_comment() {
    let comments = (1..=8)
        .map(|n| format!("\t\tc{n}\n"))
        .collect::<Vec<_>>()
        .concat();
    let mut app = Harness::with_text(&format!("- Milk\n\tCOMMENTS @hidden\n{comments}"), 80, 12);
    app.type_keys("N");
    screen(&mut app);
    press(&mut app, KeyCode::PageDown);
    press(&mut app, KeyCode::Up);
    app.type_keys("d");
    press(&mut app, KeyCode::PageUp);
    app.type_keys("jd");
    press(&mut app, KeyCode::Esc);
    assert_eq!(
        saved(&mut app),
        "- Milk\n\tCOMMENTS @hidden\n\t\tc1\n\t\tc3\n\t\tc4\n\t\tc6\n\t\tc7\n\t\tc8\n"
    );
}
