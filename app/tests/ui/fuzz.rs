//! A seeded key-sequence fuzzer: random sequences of the keys a user can press, with pauses,
//! pastes and resizes, checked against invariants that hold whatever the keys do
//! (docs/quality/testing-strategy.md, Fuzzing). `validate.sh fuzz` runs many more cases with a
//! fresh seed.

use std::{fmt, fs, time::Duration};

use crossterm::event::{Event, KeyCode, KeyModifiers};
use proptest::{
    prelude::*,
    sample::select,
    test_runner::{Config, RngSeed},
};
use ratatui::{Terminal, backend::TestBackend};
use taskpaper_terminal::ui::Input;

use crate::support::Harness;

/// An outline with every kind of block and metadata the app shows differently.
const OUTLINE: &str = "\
INBOX:
\t- Call the plumber @due(2026-10-05)
Kitchen: @home
\tShopping list for the weekend.
\t- Buy flour @start(2026-10-04) @star
\tBaking @section
\t\t- Preheat the oven @flag @red
\t\t\t- Check the **thermometer**
\tRecipe site @url(https://example.com/bread)
\t---
\t---Later---
\t- Post the letter @done(2026-10-01)
Errands: @priority(1)
\t- Call @alice about the lease @due(2026-10-03)
\t- Café crème ☕ 日本語 @bookmark
\tmail me@example.com about Proj:@x
";

/// One thing the user does.
#[derive(Clone)]
enum Action {
    Key(KeyCode, KeyModifiers),
    /// Lets this long pass without input, so that pending key sequences time out.
    Wait,
    Paste(&'static str),
    Resize(u16, u16),
}

impl fmt::Debug for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Key(code, modifiers) if modifiers.is_empty() => write!(f, "{code}"),
            Self::Key(code, modifiers) => write!(f, "{modifiers}+{code}"),
            Self::Wait => write!(f, "wait"),
            Self::Paste(text) => write!(f, "paste {text:?}"),
            Self::Resize(width, height) => write!(f, "resize {width}x{height}"),
        }
    }
}

/// The characters of navigate mode's keys and of typical text.
const CHARS: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 \
    '\".,<>[]{}=-+_*!?/:\\@()#`~$%^&|;é日🙂";

fn key(code: KeyCode) -> Action {
    Action::Key(code, KeyModifiers::NONE)
}

fn action() -> impl Strategy<Value = Action> {
    let chars: Vec<char> = CHARS.chars().collect();
    let named = [
        KeyCode::Enter,
        KeyCode::Esc,
        KeyCode::Esc,
        KeyCode::Tab,
        KeyCode::BackTab,
        KeyCode::Backspace,
        KeyCode::Delete,
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::PageUp,
        KeyCode::PageDown,
    ];
    // Every control letter but `q`, which quits and would end the sequence.
    let control: Vec<char> = ('a'..='z')
        .filter(|&c| c != 'q')
        .chain(['\'', '\\'])
        .collect();
    let alt: Vec<char> = "hjklvzVb1238`".chars().collect();
    let arrows = [KeyCode::Up, KeyCode::Down, KeyCode::Left, KeyCode::Right];
    let held = [
        KeyModifiers::SHIFT,
        KeyModifiers::CONTROL,
        KeyModifiers::ALT,
        KeyModifiers::ALT | KeyModifiers::SHIFT,
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    ];
    prop_oneof![
        40 => select(chars).prop_map(|c| key(KeyCode::Char(c))),
        12 => select(named.to_vec()).prop_map(key),
        5 => select(control).prop_map(|c| Action::Key(KeyCode::Char(c), KeyModifiers::CONTROL)),
        3 => select(alt).prop_map(|c| Action::Key(KeyCode::Char(c), KeyModifiers::ALT)),
        3 => (select(arrows.to_vec()), select(held.to_vec()))
            .prop_map(|(code, modifiers)| Action::Key(code, modifiers)),
        1 => Just(Action::Key(KeyCode::F(10), KeyModifiers::SHIFT)),
        1 => Just(Action::Wait),
        1 => select(vec!["milk", "- eggs\n- bread", "Trip:\n\t- pack @due(2026-10-09)", "日本語 🙂"])
            .prop_map(Action::Paste),
        1 => select(vec![(100, 30), (60, 12), (24, 6), (140, 40), (10, 3), (1, 1)])
            .prop_map(|(width, height)| Action::Resize(width, height)),
    ]
}

/// The seed and case count, unless `PROPTEST_RNG_SEED` and `PROPTEST_CASES` choose others, so
/// that `validate.sh all` runs the same sequences every time.
fn config() -> Config {
    let base = Config::default();
    Config {
        cases: if std::env::var_os("PROPTEST_CASES").is_some() {
            base.cases
        } else {
            24
        },
        rng_seed: if std::env::var_os("PROPTEST_RNG_SEED").is_some() {
            base.rng_seed
        } else {
            RngSeed::Fixed(0x7470_7466)
        },
        ..base
    }
}

/// The outlines a sequence starts on: a rich one, an empty file and a single task.
fn outline() -> impl Strategy<Value = &'static str> {
    select(vec![OUTLINE, OUTLINE, "", "- One task\n"])
}

/// Starts the app on `outline` and does `actions`, drawing after each one as the event loop
/// does, until they end or the app quits.
fn play(outline: &str, actions: &[Action]) -> Harness {
    // An empty settings file, so that settings changed in the app are saved for the next launch.
    let mut app = Harness::with_config(outline, "", 100, 30);
    for action in actions {
        if app.tui.should_quit() {
            break;
        }
        match action {
            Action::Key(code, modifiers) => app.press(*code, *modifiers),
            Action::Wait => app.wait(Duration::from_millis(1100)),
            Action::Paste(text) => app.send(Input::Terminal(Event::Paste((*text).to_owned()))),
            Action::Resize(width, height) => {
                app.terminal.backend_mut().resize(*width, *height);
                app.send(Input::Terminal(Event::Resize(*width, *height)));
            }
        }
        app.settle();
        app.render();
    }
    app
}

/// Leaves whatever is open, as a user does with `Esc`, waiting out any pending key sequence.
fn back_to_navigate(app: &mut Harness) {
    for _ in 0..4 {
        app.wait(Duration::from_millis(1100));
        app.press(KeyCode::Esc, KeyModifiers::NONE);
    }
    app.settle();
}

/// Columns and rows enough for every outline a sequence makes in one pane, so that neither
/// scrolling nor wrap, which a key toggles for the session only, changes what shows.
const WIDE: u16 = 400;
const TALL: u16 = 400;

/// The screen above the status line on a new terminal of [`WIDE`] by [`TALL`], with every block
/// expanded from home in the focused pane, without the focus and selection markers: fold state,
/// focus and selection are not saved.
fn whole_outline(app: &mut Harness) -> Vec<String> {
    app.terminal = Terminal::new(TestBackend::new(WIDE, TALL)).expect("a test terminal");
    app.send(Input::Terminal(Event::Resize(WIDE, TALL)));
    back_to_navigate(app);
    app.type_keys("gh+");
    app.settle();
    let buffer = app.render().buffer().clone();
    (0..TALL - 1)
        .map(|y| crate::support::line(&buffer, y).replace('▌', " "))
        .collect()
}

/// The text of the file after leaving what is open and quitting, as the next launch reads it.
fn saved_after_quitting(mut app: Harness) -> String {
    back_to_navigate(&mut app);
    let path = app.path.clone().expect("a file was opened");
    let app = app.reopen(100, 30);
    let text = fs::read_to_string(&path).expect("the saved file reads");
    drop(app);
    text
}

proptest! {
    #![proptest_config(config())]

    // N-REL-01, F-FILE-03: whatever the keys, the app neither panics nor saves a file that
    // reads back as another outline: the next launch shows what the user left.
    #[test]
    fn any_keys_save_an_outline_the_next_launch_shows_the_same(
        outline in outline(),
        actions in proptest::collection::vec(action(), 1..80),
    ) {
        let mut app = play(outline, &actions);
        prop_assume!(!app.tui.should_quit());
        let left = whole_outline(&mut app);
        let mut next = app.reopen(100, 30);
        prop_assert_eq!(whole_outline(&mut next), left);
    }

    // FR-OPS-08: undoing every step returns the file to what was opened.
    #[test]
    fn undoing_every_step_restores_the_file(
        outline in outline(),
        actions in proptest::collection::vec(action(), 1..80),
    ) {
        let mut app = play(outline, &actions);
        prop_assume!(!app.tui.should_quit());
        back_to_navigate(&mut app);
        for _ in 0..=actions.len() {
            app.press(KeyCode::Char('z'), KeyModifiers::NONE);
            app.settle();
        }
        prop_assert_eq!(saved_after_quitting(app), outline);
    }
}
