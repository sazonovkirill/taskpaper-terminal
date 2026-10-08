//! Step 5.1: panes side by side, each with its own zoom, focus and history, opened from the
//! focused block, Go to Symbol or the sidebar, focused, closed and resized with keys, the strip
//! scrolling to keep the focused pane in view.

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::Modifier;

use crate::support::{Harness, line};

const OUTLINE: &str = "\
Work:
\t- Report
\t- Slides @done
Home:
\t- Water plants
";

/// Every screen row, without trailing blanks.
fn screen(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    (buffer.area.top()..buffer.area.bottom())
        .map(|y| line(&buffer, y))
        .collect()
}

fn ctrl(app: &mut Harness, c: char) {
    app.press(KeyCode::Char(c), KeyModifiers::CONTROL);
}

fn alt(app: &mut Harness, c: char) {
    app.press(KeyCode::Char(c), KeyModifiers::ALT);
}

/// Presses `Ctrl+W`, then types `keys`.
fn window(app: &mut Harness, keys: &str) {
    ctrl(app, 'w');
    app.type_keys(keys);
}

/// The column of each pane divider on the outline's first row.
fn dividers(app: &mut Harness) -> Vec<usize> {
    let buffer = app.render().buffer().clone();
    line(&buffer, 2)
        .chars()
        .enumerate()
        .filter(|&(_, c)| c == '│')
        .map(|(x, _)| x)
        .collect()
}

/// The two-pane screen after opening the focused `Work` in a new pane on an 80-column screen.
const WORK_BESIDE: [&str; 9] = [
    "  ⌂                                    │  ⌂ › Work",
    "───────────────────────────────────────│────────────────────────────────────────",
    "  Work:                                │  Work:",
    "    - Report                           │▌   - Report",
    "    ✔ Slides                           │    ✔ Slides",
    "  Home:                                │",
    "    - Water plants                     │",
    "                                       │",
    "                                       │",
];

// FR-UI-02, navigation.md §5, keybindings.md "View, panes and panels"
#[test]
fn ctrl_backslash_opens_the_focused_block_zoomed_in_a_new_pane_to_the_right() {
    let mut app = Harness::with_text(OUTLINE, 80, 10);
    ctrl(&mut app, '\\');
    assert_eq!(
        screen(&mut app)[..9],
        WORK_BESIDE,
        "only the focused pane marks its focused block"
    );
    let buffer = app.render().buffer().clone();
    assert!(
        buffer[(2, 0)].modifier.contains(Modifier::DIM),
        "the unfocused pane's breadcrumbs are dim"
    );
    assert!(
        !buffer[(45, 0)].modifier.contains(Modifier::DIM),
        "the focused pane's breadcrumbs are not"
    );
}

// FR-UI-02, keybindings.md "View, panes and panels", D-42
#[test]
fn ctrl_w_v_opens_the_focused_block_in_a_new_pane_too() {
    let mut app = Harness::with_text(OUTLINE, 80, 10);
    window(&mut app, "v");
    assert_eq!(screen(&mut app)[..9], WORK_BESIDE);
}

// FR-UI-02, navigation.md §5
#[test]
fn ctrl_w_s_opens_the_current_location_in_a_new_pane_with_the_same_focus() {
    let mut app = Harness::with_text(OUTLINE, 80, 10);
    app.type_keys("jjj");
    window(&mut app, "s");
    let screen = screen(&mut app);
    assert_eq!(screen[0], "  ⌂                                    │  ⌂");
    assert_eq!(
        screen[5], "  Home:                                │▌ Home:",
        "the new pane focuses the same block"
    );
}

// FR-UI-02, navigation.md §5
#[test]
fn a_leaf_opens_in_a_new_pane_zoomed_into_it() {
    let mut app = Harness::with_text(OUTLINE, 80, 10);
    app.type_keys("j");
    ctrl(&mut app, '\\');
    let screen = screen(&mut app);
    assert_eq!(
        screen[0],
        "  ⌂                                    │  ⌂ › Work › Report"
    );
    assert_eq!(
        screen[2],
        "  Work:                                │▌ - Report"
    );
}

// FR-UI-02, FR-UI-03, navigation.md §5, keybindings.md "View, panes and panels"
#[test]
fn alt_digits_g_digits_and_ctrl_w_h_l_focus_panes_by_number_and_side() {
    let mut app = Harness::with_text(OUTLINE, 80, 10);
    ctrl(&mut app, '\\');
    let focused = |app: &mut Harness| {
        let screen = screen(app);
        (screen[2].starts_with('▌'), screen[3].contains("│▌"))
    };
    assert_eq!(focused(&mut app), (false, true));
    alt(&mut app, '1');
    assert_eq!(focused(&mut app), (true, false), "Alt+1 focuses the first");
    app.type_keys("g2");
    assert_eq!(focused(&mut app), (false, true), "g2 focuses the second");
    window(&mut app, "h");
    assert_eq!(focused(&mut app), (true, false), "Ctrl+W h the left one");
    window(&mut app, "l");
    assert_eq!(focused(&mut app), (false, true), "Ctrl+W l the right one");
    alt(&mut app, '3');
    window(&mut app, "l");
    assert_eq!(
        focused(&mut app),
        (false, true),
        "no pane 3, none further right"
    );
}

// FR-UI-02, navigation.md §5
#[test]
fn each_pane_keeps_its_own_zoom_focus_and_history() {
    let mut app = Harness::with_text(OUTLINE, 80, 10);
    window(&mut app, "s");
    app.type_keys("jjj");
    app.press(KeyCode::Right, KeyModifiers::ALT);
    alt(&mut app, '1');
    app.type_keys("j");
    let screen = screen(&mut app);
    assert_eq!(
        screen[0],
        "  ⌂                                    │  ⌂ › Home"
    );
    assert_eq!(
        screen[3],
        "▌   - Report                           │    - Water plants"
    );
    app.press(KeyCode::Left, KeyModifiers::ALT);
    assert_eq!(
        self::screen(&mut app)[0],
        "  ⌂                                    │  ⌂ › Home",
        "the first pane has no history to go back to"
    );
    alt(&mut app, '2');
    app.press(KeyCode::Left, KeyModifiers::ALT);
    assert_eq!(
        self::screen(&mut app)[0],
        "  ⌂                                    │  ⌂",
        "the second goes back along its own"
    );
}

// FR-UI-02, navigation.md §5
#[test]
fn an_edit_in_one_pane_shows_in_every_pane_and_folds_are_shared() {
    let mut app = Harness::with_text(OUTLINE, 80, 10);
    window(&mut app, "s");
    app.type_keys("j");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    app.type_keys("!");
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    app.type_keys("k ");
    let screen = screen(&mut app);
    assert_eq!(
        screen[2],
        "  Work: ▸ 1                            │▌ Work: ▸ 1"
    );
    assert_eq!(screen[3], "  Home:                                │  Home:");
    alt(&mut app, '1');
    app.press(KeyCode::Char(' '), KeyModifiers::NONE);
    assert_eq!(
        self::screen(&mut app)[3],
        "    - Report!                          │    - Report!"
    );
}

// FR-UI-02, FR-VIEW-04, navigation.md §5
#[test]
fn showing_completed_blocks_is_per_pane_and_a_new_pane_starts_with_its_opener_s() {
    let mut app = Harness::with_text(OUTLINE, 80, 10);
    ctrl(&mut app, 'o');
    window(&mut app, "s");
    assert_eq!(
        screen(&mut app)[4],
        "  Home:                                │  Home:",
        "both hide the done Slides"
    );
    ctrl(&mut app, 'o');
    let screen = screen(&mut app);
    assert_eq!(
        screen[4],
        "  Home:                                │    ✔ Slides"
    );
}

// FR-UI-02, navigation.md §5
#[test]
fn deleting_a_block_another_pane_is_zoomed_into_moves_that_pane_to_its_parent() {
    let mut app = Harness::with_text(OUTLINE, 80, 10);
    app.type_keys("j");
    ctrl(&mut app, '\\');
    alt(&mut app, '1');
    app.type_keys("dd");
    let screen = screen(&mut app);
    assert_eq!(
        screen[0],
        "  ⌂                                    │  ⌂ › Work"
    );
    assert_eq!(
        screen[3],
        "▌   ✔ Slides                           │    ✔ Slides"
    );
    app.press(KeyCode::Char('u'), KeyModifiers::NONE);
    assert_eq!(
        self::screen(&mut app)[0],
        "  ⌂                                    │  ⌂ › Work",
        "undo brings the block back without moving the pane again"
    );
}

// FR-UI-02, navigation.md §5, keybindings.md "View, panes and panels"
#[test]
fn ctrl_w_c_closes_the_focused_pane_focusing_its_neighbor_but_not_the_last() {
    let mut app = Harness::with_text(OUTLINE, 80, 10);
    ctrl(&mut app, '\\');
    window(&mut app, "c");
    let screen = screen(&mut app);
    assert_eq!(screen[0], "  ⌂");
    assert_eq!(screen[2], "▌ Work:");
    window(&mut app, "c");
    assert_eq!(self::screen(&mut app)[2], "▌ Work:", "the last pane stays");
}

// FR-UI-02, ui.md §5
#[test]
fn the_context_menu_closes_a_pane() {
    let mut app = Harness::with_text(OUTLINE, 80, 12);
    ctrl(&mut app, '\\');
    app.type_keys("\\close");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(screen(&mut app)[0], "  ⌂");
}

// FR-UI-03, navigation.md §5, keybindings.md "View, panes and panels"
#[test]
fn ctrl_w_angles_and_alt_shift_h_l_narrow_and_widen_the_focused_pane() {
    let mut app = Harness::with_text(OUTLINE, 80, 10);
    window(&mut app, "s");
    assert_eq!(dividers(&mut app), [39]);
    window(&mut app, "<");
    assert_eq!(
        dividers(&mut app),
        [43],
        "the focused right pane narrows by 4"
    );
    alt(&mut app, 'H');
    assert_eq!(dividers(&mut app), [47]);
    alt(&mut app, 'L');
    assert_eq!(dividers(&mut app), [43]);
    window(&mut app, ">");
    assert_eq!(dividers(&mut app), [39]);
    for _ in 0..4 {
        window(&mut app, "<");
        app.render();
    }
    assert_eq!(dividers(&mut app), [49], "no narrower than 30 columns");
}

// FR-UI-03, navigation.md §5
#[test]
fn panes_wider_than_the_screen_scroll_sideways_to_keep_the_focused_one_in_view() {
    let mut app = Harness::with_text(OUTLINE, 80, 10);
    window(&mut app, "s");
    window(&mut app, "s");
    let screen = screen(&mut app);
    assert_eq!(
        screen[2], "                   │  Work:                      │▌ Work:",
        "three 30-column panes need 90 columns, so the first is partly off screen"
    );
    alt(&mut app, '1');
    assert_eq!(
        self::screen(&mut app)[2],
        "▌ Work:                      │  Work:                      │  Work:"
    );
    alt(&mut app, '2');
    assert_eq!(
        self::screen(&mut app)[2],
        "  Work:                      │▌ Work:                      │  Work:",
        "a pane already in view does not scroll the strip"
    );
}

// FR-UI-02, navigation.md §2, §5
#[test]
fn each_pane_keeps_how_far_it_is_scrolled() {
    let text = (1..=20)
        .map(|i| format!("- Task {i:02}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut app = Harness::with_text(&text, 80, 10);
    app.type_keys("G");
    app.render();
    app.type_keys("kkk");
    assert_eq!(
        screen(&mut app)[2],
        "  - Task 14",
        "moving back up in view keeps the scroll"
    );
    window(&mut app, "s");
    assert_eq!(
        screen(&mut app)[2],
        "  - Task 14                            │  - Task 13",
        "the new pane scrolls only as far as its focus needs"
    );
}

// FR-UI-02, ui.md §1
#[test]
fn a_screen_40_by_11_still_shows_more_than_one_pane() {
    let mut app = Harness::with_text(OUTLINE, 40, 11);
    ctrl(&mut app, '\\');
    assert_eq!(dividers(&mut app), [9]);
}

// FR-UI-02, FR-NAV-05, navigation.md §8, keybindings.md §4
#[test]
fn alt_enter_ctrl_v_and_ctrl_t_open_the_highlighted_symbol_in_a_new_pane() {
    for key in [
        (KeyCode::Enter, KeyModifiers::ALT),
        (KeyCode::Char('v'), KeyModifiers::CONTROL),
        (KeyCode::Char('t'), KeyModifiers::CONTROL),
    ] {
        let mut app = Harness::with_text(OUTLINE, 80, 10);
        ctrl(&mut app, 'k');
        app.type_keys("water");
        app.press(key.0, key.1);
        let screen = screen(&mut app);
        assert_eq!(
            screen[0],
            "  ⌂                                    │  ⌂ › Home › Water plants"
        );
        assert_eq!(
            screen[2],
            "  Work:                                │▌ - Water plants"
        );
    }
}

// FR-UI-02, FR-UI-05, keybindings.md §4
#[test]
fn alt_enter_ctrl_v_and_ctrl_t_open_the_focused_sidebar_entry_in_a_new_pane() {
    for key in [
        (KeyCode::Enter, KeyModifiers::ALT),
        (KeyCode::Char('v'), KeyModifiers::CONTROL),
        (KeyCode::Char('t'), KeyModifiers::CONTROL),
    ] {
        let mut app = Harness::with_text(OUTLINE, 100, 10);
        app.render();
        ctrl(&mut app, 'g');
        for _ in 0..6 {
            app.press(KeyCode::Down, KeyModifiers::NONE);
        }
        app.press(key.0, key.1);
        let screen = screen(&mut app);
        assert_eq!(
            screen[..4],
            [
                "  ⌂                                              │  ⌂ › Home",
                "─────────────────────────────────────────────────│──────────────────────────────────────────────────",
                "  Work:                                          │  Home:",
                "    - Report                                     │▌   - Water plants",
            ]
        );
    }
}

// FR-UI-02, ui.md §1
#[test]
fn a_screen_smaller_than_40_by_10_shows_only_the_focused_pane() {
    let mut app = Harness::with_text(OUTLINE, 39, 10);
    ctrl(&mut app, '\\');
    assert_eq!(
        screen(&mut app)[..5],
        [
            "  ⌂ › Work",
            "───────────────────────────────────────",
            "  Work:",
            "▌   - Report",
            "    ✔ Slides",
        ]
    );
}

// FR-UI-03, navigation.md §5
#[test]
fn a_pane_scrolled_wholly_out_of_view_is_not_drawn() {
    let mut app = Harness::with_text(OUTLINE, 80, 10);
    for _ in 0..3 {
        window(&mut app, "s");
    }
    assert_eq!(
        screen(&mut app)[0],
        "                   │  ⌂                          │  ⌂",
        "four 30-column panes need 120 columns: the first is gone, the second shows its last 20"
    );
}

// FR-UI-02, FR-UI-03, ui.md §1
#[test]
fn a_pane_is_never_wider_than_the_room_left_by_the_shortcuts_panel() {
    let mut app = Harness::with_text(OUTLINE, 80, 12);
    app.type_keys("?");
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        screen(&mut app)[2],
        "▌ Work:                                         │  Stop Editing             Esc",
        "the one pane takes the 48 columns left of the panel"
    );
    app.terminal.backend_mut().assert_cursor_position((6, 2));
}

// FR-UI-04, file-io.md §5, navigation.md §5
#[test]
fn quitting_and_reopening_the_file_brings_its_panes_back() {
    let mut app = Harness::with_text(OUTLINE, 80, 10);
    ctrl(&mut app, '\\');
    assert_eq!(screen(&mut app)[..9], WORK_BESIDE);
    let mut app = app.reopen(80, 10);
    assert_eq!(
        screen(&mut app)[..9],
        WORK_BESIDE,
        "the panes, their zooms and focus"
    );
    let kept = std::fs::read_to_string(&app.views).expect("the layout is kept");
    assert!(kept.contains("outline.taskpaper"), "{kept}");
}

// FR-UI-04, file-io.md §5
#[test]
fn each_save_keeps_the_layout() {
    let mut app = Harness::with_text(OUTLINE, 80, 10);
    ctrl(&mut app, '\\');
    app.type_keys("x");
    assert!(!app.views.exists(), "nothing kept before the save");
    app.wait(std::time::Duration::from_secs(1));
    app.settle();
    let kept = std::fs::read_to_string(&app.views).expect("the layout is kept");
    assert!(kept.contains("\"zoom\""), "{kept}");
}

// FR-DOC-05, file-io.md §5
#[test]
fn the_temporary_file_keeps_no_layout() {
    let mut app = Harness::demo(80, 10);
    ctrl(&mut app, '\\');
    ctrl(&mut app, 'q');
    app.settle();
    assert!(app.tui.should_quit());
    assert!(!app.views.exists());
}

// FR-UI-04, FR-TAG-04, file-io.md §5
#[test]
fn reopening_focuses_the_today_entry_that_was_focused() {
    let text = "Errands:\n\t- Post office @star\n\t- Dentist @due(2026-10-05)\n";
    let focused = |app: &mut Harness| -> Vec<String> {
        screen(app)
            .into_iter()
            .filter(|row| row.starts_with('▌'))
            .collect()
    };
    let mut app = Harness::with_text(text, 80, 10);
    app.render();
    ctrl(&mut app, 'g');
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    app.type_keys("j");
    let expected = focused(&mut app);
    assert!(expected[0].contains("Dentist"), "{expected:?}");
    let mut app = app.reopen(80, 10);
    assert_eq!(focused(&mut app), expected);
}
