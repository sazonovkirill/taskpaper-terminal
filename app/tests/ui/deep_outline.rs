//! An outline thousands of levels deep, which no step of the app may go through a level at a time
//! on the stack (N-REL-01).

use std::{fmt::Write as _, fs, thread, time::Duration};

use crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Harness, line};

/// Far less stack than the app's main thread gets, so that a walk taking stack for each level
/// overflows on [`LEVELS`] levels as it would on a few more on the main thread.
const STACK: usize = 128 * 1024;

/// How deep the opened outline is; the test doubles it.
const LEVELS: usize = 2_000;

/// A chain of tasks, each the only child of the one before it, ending in a project, then
/// a copy of it below that project when `doubled`.
fn chain(doubled: bool) -> String {
    let mut text = String::new();
    for depth in 0..LEVELS * if doubled { 2 } else { 1 } {
        let level = depth % LEVELS + 1;
        let tabs = "\t".repeat(depth);
        let line = if level == LEVELS {
            writeln!(text, "{tabs}End:")
        } else {
            writeln!(text, "{tabs}- {level}")
        };
        line.expect("writing to a String cannot fail");
    }
    text
}

/// The open file's text once the autosave delay has passed and its save is done.
fn saved(app: &mut Harness) -> String {
    app.wait(Duration::from_secs(1));
    app.settle();
    let path = app.path.as_ref().expect("a file was opened");
    fs::read_to_string(path).expect("the file is readable")
}

/// Presses `code`, then draws a frame.
fn press(app: &mut Harness, code: KeyCode) {
    app.press(code, KeyModifiers::NONE);
    app.render();
}

/// Types `keys`, then draws a frame.
fn keys(app: &mut Harness, keys: &str) {
    app.type_keys(keys);
    app.render();
}

// N-REL-01, FR-OPS-06, FR-OPS-02
#[test]
fn an_outline_thousands_of_levels_deep_opens_shows_edits_and_saves() {
    thread::Builder::new()
        .stack_size(STACK)
        .spawn(|| {
            let mut app = Harness::with_text(&chain(false), 60, 12);
            app.render();
            // Copies the whole chain and pastes it below its last block, as the report did.
            keys(&mut app, "ggyGp");
            press(&mut app, KeyCode::Tab);
            assert_eq!(saved(&mut app), chain(true));
            keys(&mut app, "gs");
            let screen = app.render().buffer().clone();
            assert!(
                (0..12).any(|y| line(&screen, y).contains("End  1 › 2 › 3")),
                "Go to Symbol lists the projects with the blocks above them"
            );
            press(&mut app, KeyCode::Esc);
            for view in ["gP", "gm", "gF", "gT", "gU"] {
                keys(&mut app, view);
                press(&mut app, KeyCode::Esc);
                keys(&mut app, "H");
            }
            app.press(KeyCode::Char('g'), KeyModifiers::CONTROL);
            press(&mut app, KeyCode::Esc);
            keys(&mut app, "gg.gh_+--==\"\"");
            keys(&mut app, "G");
            press(&mut app, KeyCode::BackTab);
            press(&mut app, KeyCode::Tab);
            press(&mut app, KeyCode::Enter);
            keys(&mut app, "!");
            press(&mut app, KeyCode::Esc);
            keys(&mut app, "ggDz");
            let mut edited = chain(true);
            let last = edited.rfind("End:").expect("the chain ends in the project");
            edited.replace_range(last.., "End!:\n");
            assert_eq!(saved(&mut app), edited);
            let mut app = app.reopen(60, 12);
            keys(&mut app, "G");
        })
        .expect("a thread")
        .join()
        .expect("the app does not overflow its stack");
}
