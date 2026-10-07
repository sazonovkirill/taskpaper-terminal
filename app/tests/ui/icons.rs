//! Step 6.3: Nerd Font icons by default, `icons = "none"` for the Unicode markers, and ASCII on a
//! locale that is not UTF-8 (FR-UI-09, FR-UI-10, ui.md §3, §7).

use crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Harness, line};

/// A project with a sub-project and a bookmark, a task for today due today, a done task and a
/// project to collapse.
const OUTLINE: &str = "\
Work: @bookmark
\t- Draft @today @due(2026-10-05)
\t- Sent @done
\tPlans:
\t\t- Ask
Later:
\t- Call
";

/// Each Nerd Font glyph the screen of [`OUTLINE`] shows, with the Unicode marker drawn in its
/// place when icons are off (ui.md §2, §7).
const NERD_TO_UNICODE: [(&str, &str); 6] = [
    ("\u{f02dc}", "⌂"),
    ("\u{f04ce}", "★"),
    ("\u{f00c0}", "⚑"),
    ("\u{f166c}", "⇥"),
    ("\u{f01bf}", "↕"),
    ("\u{f09de}", "●"),
];

/// Every screen row, without trailing blanks, the file's path, which differs between tests, as
/// `FILE`.
fn screen(app: &mut Harness) -> Vec<String> {
    let path = app.path.clone().expect("a file was opened");
    let buffer = app.render().buffer().clone();
    (buffer.area.top()..buffer.area.bottom())
        .map(|y| line(&buffer, y).replace(&path.display().to_string(), "FILE"))
        .collect()
}

/// The app on [`OUTLINE`] with `settings`, with `Later` collapsed and an unsaved change.
fn started(settings: &str) -> Harness {
    let mut app = Harness::with_config(OUTLINE, settings, 90, 12);
    // `h` on `Call`, the last block, goes to `Later`, and on `Later` collapses it; then the
    // file changes without being saved.
    app.type_keys("G");
    app.press(KeyCode::Char('h'), KeyModifiers::NONE);
    app.press(KeyCode::Char('h'), KeyModifiers::NONE);
    app.type_keys("gg");
    app.press(KeyCode::Char('j'), KeyModifiers::NONE);
    app.press(KeyCode::Char('x'), KeyModifiers::NONE);
    app.press(KeyCode::Char('x'), KeyModifiers::NONE);
    app
}

// FR-UI-09, FR-UI-10, ui.md §7
#[test]
fn icons_on_draw_nerd_glyphs_where_icons_off_draws_the_markers_in_the_same_layout() {
    let nerd = screen(&mut started("icons = \"nerd\"\n"));
    let unicode = screen(&mut started("icons = \"none\"\n"));
    for (glyph, _) in NERD_TO_UNICODE {
        assert!(
            nerd.iter().any(|row| row.contains(glyph)),
            "{glyph:?} drawn: {nerd:#?}"
        );
        assert!(
            !unicode.iter().any(|row| row.contains(glyph)),
            "{glyph:?} not drawn with icons off: {unicode:#?}"
        );
    }
    let swapped: Vec<String> = nerd
        .iter()
        .map(|row| {
            NERD_TO_UNICODE
                .iter()
                .fold(row.clone(), |row, (glyph, marker)| {
                    row.replace(glyph, marker)
                })
        })
        .collect();
    assert_eq!(swapped, unicode, "the same screen but for the glyphs");
}

// FR-UI-10, ui.md §3
#[test]
fn without_a_utf_8_locale_both_settings_draw_ascii() {
    for settings in ["icons = \"nerd\"\n", "icons = \"none\"\n"] {
        let mut app = started(settings).without_utf8();
        let rows = screen(&mut app);
        let markers = ['⌂', '✔', '★', '⚑', '▸', '▾', '⇥', '↕', '●'];
        assert!(!rows.join("").contains(markers), "{settings}: {rows:#?}");
        assert_eq!(rows[0], "  ~", "{settings}: {rows:#?}");
        assert_eq!(rows[2], "  Work: #", "{settings}: {rows:#?}");
        assert!(
            rows[3].starts_with("▌   - Draft * ") && rows[3].ends_with("@ today"),
            "{settings}: {rows:#?}"
        );
        assert_eq!(rows[4], "    [x] Sent", "{settings}: {rows:#?}");
        assert_eq!(rows[7], "  Later: > 1", "{settings}: {rows:#?}");
        assert_eq!(rows[11], "> NAVIGATE · FILE *", "{settings}: unsaved");
        app.press(KeyCode::Char('g'), KeyModifiers::CONTROL);
        let rows = screen(&mut app);
        assert!(!rows.join("").contains(markers), "{settings}: {rows:#?}");
        for entry in ["│▌ * Today ", "│  Work ", "│ vWork "] {
            assert!(
                rows.iter().any(|row| row.contains(entry)),
                "{settings}: {entry:?} in the sidebar's window: {rows:#?}"
            );
        }
    }
}

// ui.md §7, FR-KEY-04
#[test]
fn the_shortcuts_panel_says_how_to_turn_nerd_icons_off() {
    let note = [
        "Icons drawn as boxes? Set",
        "icons = \"none\" in config.toml",
    ];
    let nerd = "icons = \"nerd\"\n";
    let mut app = Harness::with_config(OUTLINE, nerd, 90, 60);
    app.type_keys("?");
    let rows = screen(&mut app);
    for line in note {
        assert!(rows.iter().any(|row| row.contains(line)), "{rows:#?}");
    }
    let mut app = Harness::with_config(OUTLINE, "icons = \"none\"\n", 90, 60);
    app.type_keys("?");
    let rows = screen(&mut app);
    assert!(!rows.iter().any(|row| row.contains(note[0])), "{rows:#?}");
    let mut app = Harness::with_config(OUTLINE, nerd, 90, 60).without_utf8();
    app.type_keys("?");
    let rows = screen(&mut app);
    assert!(
        !rows.iter().any(|row| row.contains(note[0])),
        "ASCII draws no Nerd glyphs: {rows:#?}"
    );
}
