//! The attachments panel: `n` and `N` open the focused block's note and comments on a panel
//! over the right side of the screen, `Tab` switches between them and `Esc` closes it; each kind
//! has a setting that turns it off (FR-ATT-03, FR-ATT-06).

use crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Harness, line};

/// Every screen row, without trailing blanks.
fn screen(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    (buffer.area.top()..buffer.area.bottom())
        .map(|y| line(&buffer, y))
        .collect()
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
            "▌ - Milk                                │ ≡ Note   ❞ Comments                  │",
            "  - Bread                               │                                      │",
            "                                        │ Full fat                             │",
            "                                        │ - not skimmed                        │",
            "                                        │                                      │",
            "                                        │                                      │",
            "                                        │                                      │",
            "                                        │ Tab other tab · Esc close            │",
            "                                        └──────────────────────────────────────┘",
        ]
    );
    app.type_keys("jdd");
    press(&mut app, KeyCode::Tab);
    assert_eq!(
        screen(&mut app)[2..5],
        [
            "▌ - Milk                                │ ≡ Note   ❞ Comments                  │",
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
    assert!(rows[9].contains("│ Esc close"), "{rows:?}");
    let mut off = Harness::with_config(
        TEXT,
        "attachments_notes = false\nattachments_comments = false\n",
        80,
        12,
    );
    off.type_keys("nN");
    assert!(!screen(&mut off)[1].contains('┌'));
}
