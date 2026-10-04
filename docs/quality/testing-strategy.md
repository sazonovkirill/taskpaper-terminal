# Testing Strategy

- Status: Accepted
- Last updated: 2026-10-04

## Owner requirements

These are the owner's requirements. Everything else in this document serves them.

1. **Full and complete coverage, within reason.** All owned production behavior is tested. Exclusions are narrow, explicit and justified.
2. **Clear separation between unit tests and integration (UI) tests.** The two kinds live in different places, run as separately selectable suites and are never mixed in one test.
3. **Every type and standalone function has a complete set of unit tests** covering the happy path, error cases and edge cases.
4. **Tests verify the contract, not the implementation.** A test states what a unit promises to its callers through its public interface. Refactoring that keeps the contract must not require rewriting tests.
5. **Integration (UI) tests cover every user interaction with the app, within reason.** Every key binding, command, mode and screen a user can reach has at least one test driving it through the app's input handling and checking what the user sees and what is saved.
6. **Every test can run in parallel** with every other test.

## Test levels and layout

The crate is split so that everything except terminal setup is testable without a terminal:

- `app/src/lib.rs` holds all application logic; `app/src/main.rs` only sets up the terminal, runs the event loop and restores the terminal. Integration tests can only import a library crate, so logic never lives in `main.rs`.
- Application code follows the Elm Architecture split: input mapping (`KeyEvent` to message), a state update function (model plus message to new model and effects), and a view that renders the model into a `Frame` without changing it. File I/O and the clock are passed in, never reached for from inside logic.

There are exactly three kinds of tests:

| Kind | Where | What it exercises | Run with |
| --- | --- | --- | --- |
| Unit | `#[cfg(test)] mod tests` inside the file under test | One type or function through its contract | `cargo test --lib` |
| Integration (UI) | `app/tests/ui/` (one test crate, `main.rs` plus modules) | The whole app through key events, rendered in a `TestBackend` and checked on screen and on disk | `cargo test --test ui` |
| End-to-end smoke | `app/tests/e2e/` (one test crate) | The real binary in a pseudo-terminal | `cargo test --test e2e` |

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
- End-to-end smoke tests are few: start, one key round trip, save, quit with the right exit code, and the terminal restored afterwards. They run the real binary (`env!("CARGO_BIN_EXE_taskpaper-terminal")`) in a pseudo-terminal, for example with `termlens`, and every wait has a timeout. Logic that can be checked through `TestBackend` is not tested here.

## Parallel execution

Every test must pass when run in parallel with every other test, in any order, as threads in one process (`cargo test`).

- No shared mutable state between tests: no `static mut`, no global caches, no singletons that tests write to.
- Never use fixed paths, the home directory, or the real working directory. Every test that touches the filesystem creates its own `tempfile::TempDir` and keeps the guard alive until the test ends.
- Never set or remove environment variables in a test; `std::env::set_var` is unsound in multi-threaded programs. Pass configuration in as parameters, or set it on a child process with `Command::env`.
- Never sleep or read the wall clock. Inject time as a parameter or through a clock the test controls.
- Tests are deterministic: no randomness without a fixed seed (proptest's own seeds and regression files excepted), and no dependence on terminal size, locale or timezone of the machine running them.
- `serial_test` is a last resort for a resource that truly cannot be isolated, and needs a comment explaining why.

## Snapshots

- Snapshot files (`.snap`) are committed and reviewed like code. Keep each snapshot small and named after the interaction it shows.
- Review every changed snapshot with `cargo insta review` before accepting it. Never bulk-accept (`cargo insta accept`, `INSTA_UPDATE=always`) without having read each diff and understood why it changed; an unexplained snapshot change is a bug until proven otherwise.
- Pending `.snap.new` files are never committed.

## Coverage and test quality

- Pure logic (model, parser and serializer, update functions, input mapping) has 100% line coverage. Rendering and terminal glue are covered by widget and integration tests to the extent a `TestBackend` can reach; only `main.rs` terminal setup is excluded.
- Measure with `cargo llvm-cov`. Coverage shows what is not tested; it does not prove that what is tested is checked.
- Use `cargo-mutants` to check that tests actually assert behavior: a missed mutant is code that can be broken without any test failing. Run it on the changed code before review, and fix missed mutants in the changed code by adding assertions, not by excluding them.

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
