//! Step 3.7: `mi` shows the focused block's tags, unsupported ones marked, and the hidden data
//! it owns; any key closes it.

use crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{DUE, Harness, line};

/// Every screen row, without trailing blanks.
fn screen(app: &mut Harness) -> Vec<String> {
    let buffer = app.render().buffer().clone();
    (buffer.area.top()..buffer.area.bottom())
        .map(|y| line(&buffer, y))
        .collect()
}

// D-3, FR-TAG-01, FR-SPC-01, special-nodes.md §1
#[test]
fn m_i_lists_the_tags_and_hidden_data_and_a_key_closes_it() {
    let mut app = Harness::with_text(
        "- Milk @due(2026-10-08) @estimate(2h)\n\tLOG @hidden @log\n\t\tx\n- Bread\n",
        44,
        10,
    );
    app.type_keys("mi");
    assert_eq!(
        screen(&mut app)[2..8],
        [
            &format!("▌ - Milk{:>36}", format!("{DUE} Thu")),
            "  -┌Details:────────────────────────────┐",
            "   │ @due(2026-10-08)                   │",
            "   │ @estimate(2h) unsupported          │",
            "   │ Hidden data @hidden @log · 2 lines │",
            "   └────────────────────────────────────┘",
        ]
    );
    app.type_keys("j");
    let rows = screen(&mut app);
    assert!(
        !rows.iter().any(|row| row.contains("Details:")),
        "{rows:#?}"
    );
    assert_eq!(rows[2], format!("▌ - Milk{:>36}", format!("{DUE} Thu")));
    app.type_keys("jmi");
    let rows = screen(&mut app);
    assert!(
        rows.contains(&"                 │ No tags │".to_owned()),
        "{rows:#?}"
    );
    app.press(KeyCode::Esc, KeyModifiers::NONE);
    assert!(!screen(&mut app).iter().any(|row| row.contains("Details:")));
}
