# Testing Strategy

- Status: Accepted
- Last updated: 2026-10-08

## Owner requirements

These are the owner's requirements. Everything else in this document serves them.

1. **Full and complete coverage, within reason.** All owned production behavior is tested. Exclusions are narrow, explicit and justified.
2. **Clear separation between unit tests and integration (UI) tests.** The two kinds live in different places, run as separately selectable suites and are never mixed in one test.
3. **Every type and standalone function has a complete set of unit tests** covering the happy path, error cases and edge cases.
4. **Tests verify the contract, not the implementation.** A test states what a unit promises to its callers through its public interface. Refactoring that keeps the contract must not require rewriting tests.
5. **Integration (UI) tests cover every user interaction with the app, within reason.** Every key binding, command, mode and screen a user can reach has at least one test driving it through the app's input handling and checking what the user sees and what is saved.
6. **Every test can run in parallel** with every other test.
7. **UI tests render through Ratatui's `TestBackend`**, so agents can see what the app shows without a real terminal.

## Test levels and layout

The crate is split so that everything except terminal setup is testable without a terminal:

- `app/src/lib.rs` holds all application logic; `app/src/main.rs` only sets up the terminal, runs the event loop and restores the terminal. Integration tests can only import a library crate, so logic never lives in `main.rs`.
- Application code follows the Elm Architecture split: input mapping (`KeyEvent` to message), a state update function (model plus message to new model and effects), and a view that renders the model into a `Frame` without changing it. File I/O and the clock are passed in, never reached for from inside logic.

There are exactly four kinds of tests:

| Kind | Where | What it exercises | Run with |
| --- | --- | --- | --- |
| Unit | `#[cfg(test)] mod tests` inside the file under test | One type or function through its contract | `cargo test --lib` |
| Integration (UI) | `app/tests/ui/` (one test crate, `main.rs` plus modules) | The whole app through key events, rendered in a `TestBackend` and checked on screen and on disk | `cargo test --test ui` |
| End-to-end smoke | `app/tests/e2e/` (one test crate) | The real binary in a pseudo-terminal | `cargo test --test e2e` |
| Scenario | `app/tests/scenarios/library/` (`.scenario` files run by one test crate) | What a real user does in one session of the real binary in a pseudo-terminal, and what they see and what is saved | `bash app/scripts/validate.sh scenarios` |

Unit tests never construct a `Terminal`. Integration tests never call private functions or inspect internal state; they use only what the library exports for driving the app. Shared integration helpers live in a module of the test crate, not in production code. Add a new integration file as a module of the existing crate rather than as a new top-level file under `tests/`, because every top-level file becomes its own binary and slows the build.

## Unit tests

- Every public type, every method and every standalone function has unit tests for its happy path, each documented error, and its edge cases: empty input, single element, boundaries, maximum depth, Unicode and wide characters, malformed input, and no-op operations.
- Test through the public contract of the unit. Private helpers are covered through the functions that use them; a helper that needs its own tests is a sign it should be its own unit.
- Assert the specific error variant (`assert!(matches!(result, Err(ParseError::UnexpectedIndent { .. })))`), not just that an error happened. Use `#[should_panic(expected = "...")]` only for documented panics, and always with `expected`.
- Return `Result<(), Box<dyn std::error::Error>>` from tests that would otherwise chain `unwrap()`, and use `?`.
- Name tests after the behavior, for example `parses_project_line_with_trailing_tags`. Give `assert!` a message when the failure would otherwise be unclear.
- When many cases share a shape, route them through one `check(input, expected)` helper or an `rstest` table so a contract change edits one place.
- Use `proptest` for invariants: parsing then serializing a `.taskpaper` file reproduces it, editing operations preserve the outline's structure, and no input makes the parser panic. Commit the `proptest-regressions/` files it writes.
- Test widgets by rendering into a buffer, with no terminal: `let mut buf = Buffer::empty(Rect::new(0, 0, w, h)); widget.render(buf.area, &mut buf);` then `assert_eq!(buf, expected)`, where `expected` is built with `Buffer::with_lines([...])`. `Buffer` equality compares styles, so set expected colors and modifiers with `expected.set_style(area, style)`. A `#[track_caller] fn test_case(widget, expected)` helper, as in Ratatui's own widget tests, keeps each case to one line.
- Build key events directly: `KeyEvent::from(KeyCode::Char('j'))`, `KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL)`. Test that release and repeat events are ignored where the app filters on `KeyEventKind::Press`.

## Integration (UI) tests

- Every user interaction the specs describe has at least one integration test: every key binding in every mode, every command, every screen and dialog, and the visible result of errors such as an unreadable or malformed file. When a spec adds or changes an interaction, its integration test lands in the same commit.
- A test starts the app on a fixture file in a temporary directory, feeds a sequence of key events through the app's input handling, draws into `Terminal::new(TestBackend::new(width, height))`, and checks both what the user sees and, where the interaction saves, the file on disk.
- Check screens with `terminal.backend().assert_buffer_lines([...])`, `assert_buffer(&expected)` and `assert_cursor_position(...)`, or with an `insta` snapshot: `insta::assert_snapshot!(terminal.backend())` for text, and `insta::assert_debug_snapshot!(terminal.backend().buffer())` when colors or modifiers matter, because the text snapshot drops styles.
- Use a fixed terminal size in every test. Test layout at a few sizes, including a very small terminal, with `TestBackend::resize`.
- Fixture `.taskpaper` files live under `app/tests/fixtures/` and are copied into the test's temporary directory before use; tests never modify fixtures in place.
- Every integration test draws through `TestBackend`; none uses `CrosstermBackend` or another real backend. Only the end-to-end smoke tests below run in a real (pseudo-)terminal.
- End-to-end smoke tests are few, and [scenarios](#scenarios) cover what a user does in a real terminal: start, one key round trip, save, quit with the right exit code, and the terminal restored afterwards. They run the real binary (`env!("CARGO_BIN_EXE_tpt")`) in a pseudo-terminal, for example with `termlens`, and every wait has a timeout. Logic that can be checked through `TestBackend` is not tested here.

## Scenarios

Scenarios check the app the way a user meets it (owner, 2026-10-07): the real binary in a pseudo-terminal that, like most terminals, does not speak the kitty keyboard protocol, driven by key presses and checked on the screen and in the saved file. They complement the integration tests, which remain the place to cover every interaction; a scenario covers a realistic sequence of them. There can be hundreds, because each is a short text file and the runner runs them in parallel.

- Each `.scenario` file under `app/tests/scenarios/library/<area>/` is one session. `app/tests/scenarios/main.rs` runs them all, starting the binary on a fresh file with `HOME` and every XDG folder in a temporary directory, `LANG=C.UTF-8` and `TERM=xterm-256color`, and reports each failing step with its line, the screen and the file. `bash app/scripts/validate.sh scenarios [filter...]` runs those whose path contains a filter; `all` runs every one.
- The language has one command per line. The first comment line is the title, saying what the user does and expects. `size W H` and `env NAME=VALUE` set the terminal and the environment; `config <<END` and `file <<END` give the settings file and the outline, verbatim up to the line `END`. Steps: `keys` presses keys separated by spaces, a token without `<>` one character at a time (`gg`), named keys as `<Enter>`, `<Esc>`, `<Tab>`, `<S-Tab>`, `<BS>`, `<Del>`, arrows (`<Up>`), `<Home>`, `<End>`, `<PgUp>`, `<PgDn>`, `<Space>`, `<F1>`…`<F12>`, `<lt>`, and with modifiers `<C-x>`, `<A-x>`, `<S-Up>`, `<C-Down>`, `<A-S-Left>`, `<S-F10>`; `type` types the rest of the line; `paste <<END` pastes; `see TEXT` waits until the screen shows the text, `hide TEXT` requires that the drawn screen does not, `status TEXT` waits for it on the status line; `resize W H`; `quit` presses `Ctrl+Q` and requires a clean exit; `saved <<END` waits until the file holds exactly those lines. `{today}`, `{today+N}` and `{today-N}` stand for dates. The runner pauses after `Esc` so the next key is not read as an `Alt` chord.
- A scenario that reproduces a known bug carries a `# BUG: <summary>` line: it is expected to fail and does not fail the run, and once it passes the run fails until the line goes, so a fixed bug stays fixed. Remove the line in the commit that fixes the bug.
- Agents explore the app by hand in tmux and turn what they find into scenarios, as the [`explore-app` skill](../../.agents/skills/explore-app/SKILL.md) describes.
- The binary reads the real clock, so dates are written as placeholders the runner fills in from today's date; that and the pause after `Esc` are the runner's only exceptions to [parallel execution](#parallel-execution). Waits poll the screen or the file with a timeout and never sleep. Check distinctive text and saved files, not columns or colors, which the integration tests check.

## Fuzzing

`app/tests/ui/fuzz.rs` feeds random sequences of key presses, pauses, pastes and resizes, drawn from every key the app binds and typical text, into the app through `TestBackend`, and checks invariants that hold whatever the keys do: the app does not panic while handling or drawing; the file it saves reopens and saves to the same text; and after `Esc` and one undo per step the saved file equals the one opened. `validate.sh all` runs a few dozen cases on a fixed seed, so it stays fast and deterministic. `bash app/scripts/validate.sh fuzz` runs a thousand cases in release mode on a fresh seed it prints (`PROPTEST_CASES` and `PROPTEST_RNG_SEED` override both); run it after changing input handling or editing, and commit the regression file proptest writes for a failure with the fix.

## Seeing the app without a terminal

Agents cannot look at a running terminal app, so they inspect it the way the integration tests do: drive it through `TestBackend` and read the rendered screen as text.

- To check what a screen looks like, write or extend an integration test that reaches it, then print the backend: `TestBackend` implements `Display`, which renders each row of the buffer as a quoted line, so `println!("{}", terminal.backend())` with `cargo test --test ui <name> -- --nocapture` shows the screen. Turn what you saw into assertions or a snapshot before committing; leftover `println!` calls are debug output.
- Read the committed `.snap` files under `app/tests/ui/snapshots/` to see what each screen looks like today, and read a failing `assert_buffer_lines` or snapshot diff as the visible difference between the expected and actual screen.
- Verify a visible change through `TestBackend`, not by running the binary in a terminal or asking the owner for a screenshot. If a behavior cannot be seen through `TestBackend`, that is a gap in what the library exports for driving the app; fix the gap.

## Traceability

Feature-complete means every Must requirement has a test that shows it working.

- An integration test names the requirement IDs it covers (`FR-NAV-03`, `F-FILE-04`, `N-REL-01`) in a comment on the test, for example `// FR-NAV-03, FR-NAV-04`. A unit test does the same when it is the main evidence for a requirement, such as a round-trip property test for F-FILE-03. A test that covers no requirement needs no reference.
- `bash app/scripts/validate.sh trace` lists every Must requirement in [functional-requirements.md](../functional-requirements.md) and [requirements.md](../requirements.md) that no test or source file names yet. It reports and does not fail validation; delivering the last feature of v1 includes bringing it to zero.

## Performance

`app/benches/perf.rs` checks the performance requirements N-PERF-01 and N-PERF-02 on a generated 100k-line, 10 MB outline: opening it to its first frame, and each common keystroke to its rendered frame, through the same `Tui` the binary runs. `bash app/scripts/validate.sh bench` runs it in release mode and fails when a scenario misses its budget; it is not part of `all`, because timings depend on the machine. `cargo test --all-targets`, and so `all`, runs every scenario once on a small outline without timing it, so the benchmark keeps building and running. Run `bench` before and after a change that could slow opening, editing or drawing, and record both results on the task.

## Parallel execution

Every test must pass when run in parallel with every other test, in any order, as threads in one process (`cargo test`).

- No shared mutable state between tests: no `static mut`, no global caches, no singletons that tests write to.
- Never use fixed paths, the home directory, or the real working directory. Every test that touches the filesystem creates its own `tempfile::TempDir` and keeps the guard alive until the test ends.
- Never set or remove environment variables in a test; `std::env::set_var` is unsound in multi-threaded programs. Pass configuration in as parameters, or set it on a child process with `Command::env`.
- Never sleep or read the wall clock. Inject time as a parameter or through a clock the test controls. Clippy's `disallowed-methods` in `app/clippy.toml` rejects `std::thread::sleep`, `Instant::now` and `SystemTime::now` everywhere.
- Tests are deterministic: no randomness without a fixed seed (proptest's own seeds and regression files excepted), and no dependence on terminal size, locale or timezone of the machine running them.
- `serial_test` is a last resort for a resource that truly cannot be isolated, and needs a comment explaining why.

## Snapshots

- Snapshot files (`.snap`) are committed and reviewed like code. Keep each snapshot small and named after the interaction it shows.
- Review every changed snapshot with `cargo insta review` before accepting it. Never bulk-accept (`cargo insta accept`, `INSTA_UPDATE=always`) without having read each diff and understood why it changed; an unexplained snapshot change is a bug until proven otherwise.
- Pending `.snap.new` files are never committed; `validate.sh static` fails while any exist.

## Coverage and test quality

- Every line of the library crate (`app/src/` except `main.rs`) is covered: `bash app/scripts/validate.sh coverage` runs `cargo llvm-cov` over all tests and fails below 100% line coverage. Only `main.rs`, which holds nothing but terminal setup and the event loop, is excluded. Any further exclusion is a narrow, commented addition to the script's ignore pattern that the owner approves.
- Coverage shows what is not tested; it does not prove that what is tested is checked.
- Mutation testing is a required step before review: `bash app/scripts/validate.sh mutants` runs `cargo-mutants` on the code changed since the branch left `origin/main`, with one job per two CPUs (set `CARGO_MUTANTS_JOBS` to override). A missed mutant is code that can be broken without any test failing. Fix missed mutants by adding assertions, not by excluding them; a mutant that cannot be caught (an equivalent mutant) is recorded on the task with the reason, and reviewers may dispute it.

## Practices for agent-written tests

Agents write most of this project's code and tests, which brings specific failure modes: tests that pass whether or not the feature works, tests edited to match buggy code, and special-cased production code.

- **Run the tests first.** Start every session by running `bash app/scripts/validate.sh test` to get a baseline and catch breakage left by earlier work before adding to it.
- **Write the test first and watch it fail.** For new behavior and for every bug fix, write the test, run it, and confirm it fails for the expected reason before implementing. A test that never failed proves nothing. Record the failing run on the task.
- **Existing tests are fixed points.** Never edit, weaken, skip or delete an existing test to get green. If a test is wrong, explain why and stop for the owner or the reviewers, as [AGENTS.md](../../AGENTS.md) requires.
- **No solutions shaped to the tests.** Implement behavior that is correct for all valid inputs. Hardcoded expected outputs, branches on fixture contents, custom `PartialEq` written to make an assertion pass and test-only behavior in production code are defects.
- **Reviewers check the tests, not only the code.** Reviewers look specifically for edited or deleted assertions, special-cased outputs, weakened comparisons, tests that cannot fail, and listed edge cases without a test.
- **Report evidence, not claims.** Record the exact commands run and their real exit status and output summary on the task. Never report a check that was not run on the final contents.
- **Keep the suite fast.** Fast, deterministic tests run on every change; anything slow or timing-dependent stays in the small end-to-end set so the feedback loop stays short.

## Sources

- Rust Book, test organization and writing tests: https://doc.rust-lang.org/book/ch11-03-test-organization.html, https://doc.rust-lang.org/book/ch11-01-writing-tests.html
- matklad, integration test layout and testing at boundaries: https://matklad.github.io/2021/02/27/delete-cargo-integration-tests.html, https://matklad.github.io/2021/05/31/how-to-test.html
- `std::env::set_var` safety: https://doc.rust-lang.org/std/env/fn.set_var.html
- tempfile, proptest, insta, cargo-llvm-cov, cargo-mutants: https://docs.rs/tempfile/latest/tempfile/, https://proptest-rs.github.io/proptest/intro.html, https://insta.rs/docs/, https://github.com/taiki-e/cargo-llvm-cov, https://mutants.rs/
- Ratatui testing: https://ratatui.rs/recipes/testing/snapshots/, https://ratatui.rs/concepts/application-patterns/the-elm-architecture/, https://ratatui.rs/tutorials/counter-app/basic-app/, https://docs.rs/ratatui/latest/ratatui/backend/struct.TestBackend.html, https://github.com/ratatui/ratatui/issues/1402
- termlens: https://github.com/vyncint/termlens
- Claude Code best practices: https://code.claude.com/docs/en/best-practices
- Simon Willison, red/green TDD and running tests first: https://simonwillison.net/guides/agentic-engineering-patterns/red-green-tdd/, https://simonwillison.net/guides/agentic-engineering-patterns/first-run-the-tests/
- Kent Beck, augmented coding: https://tidyfirst.substack.com/p/augmented-coding-beyond-the-vibes
- ImpossibleBench, reward hacking in coding agents: https://www.greaterwrong.com/posts/qJYMbrabcQqCZ7iqm/impossiblebench-measuring-reward-hacking-in-llm-coding-1
- Meta, mutation-guided LLM test generation: https://arxiv.org/html/2501.12862v1
- Martin Fowler, harness engineering: https://martinfowler.com/articles/harness-engineering.html
