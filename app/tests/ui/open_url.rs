//! Step 3.9: `gx` opens the focused block's URL in the system browser.

use std::time::Duration;

use crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Harness, line};

/// The status line of `app` after a draw.
fn status_line(app: &mut Harness) -> String {
    let buffer = app.render().buffer().clone();
    line(&buffer, buffer.area.bottom() - 1)
}

/// What the status line says after the file's path.
fn notice(app: &mut Harness) -> String {
    let status = status_line(app);
    let path = app.status(200);
    status
        .strip_prefix(path.trim_end())
        .unwrap_or(&status)
        .to_owned()
}

/// The status line's notice when opening `url` failed because the opener is missing.
fn failed(url: &str) -> String {
    format!(" · cannot open {url}: No such file or directory (os error 2)")
}

// FR-BLK-06, D-10
#[test]
fn g_x_opens_a_url_blocks_address_or_a_bare_url_in_the_text() {
    let mut app = Harness::with_text(
        "Guide @url(https://a.b/c)\n- Read https://x.y today\n- No link\n",
        120,
        6,
    );
    app.type_keys("gx");
    assert_eq!(notice(&mut app), failed("https://a.b/c"));
    app.type_keys("jgx");
    assert_eq!(notice(&mut app), failed("https://x.y"));
    app.wait(Duration::from_secs(10));
    app.type_keys("jgx");
    assert_eq!(notice(&mut app), "", "a block without a URL opens nothing");
}

// FR-BLK-06
#[test]
fn g_x_in_edit_mode_types_the_letters() {
    let mut app = Harness::with_text("- https://x.y\n", 60, 5);
    app.press(KeyCode::Enter, KeyModifiers::NONE);
    app.type_keys(" gx");
    let status = status_line(&mut app);
    assert!(status.starts_with("EDIT"), "{status}");
    assert!(!status.contains("cannot open"), "{status}");
}
