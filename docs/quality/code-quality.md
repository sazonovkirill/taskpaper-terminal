# Code Quality and Style

- Status: Accepted
- Last updated: 2026-10-04

## Owner requirements

These are the owner's requirements. Everything else in this document serves them.

1. **Strict Clippy.** All default lints plus `clippy::pedantic`, configured in `app/Cargo.toml`, with warnings denied. Exceptions are narrow and justified.
2. **No panics in production code.** No `unwrap`, `panic!`, `todo!`, `unimplemented!` or unchecked indexing outside tests. `expect` is allowed only for a real invariant, and its message says why it cannot fail.
3. **Typed errors.** The library returns `thiserror` enums; `anyhow` is used only in `main.rs`.
4. **One crate with layered modules.** The TaskPaper document model knows nothing about Ratatui or crossterm, and dependencies point inward only.
5. **No unsafe code.** `unsafe_code` is forbidden crate-wide.
6. **Brief doc comments on every public item,** enforced by `missing_docs`.
7. **Reproducible tooling.** The Rust toolchain is pinned in `rust-toolchain.toml`, and `cargo-deny` checks licenses, advisories and duplicate dependencies as part of validation.

## Architecture

All logic lives in the library (`app/src/lib.rs` and its modules); `main.rs` only sets up the terminal, runs the event loop, restores the terminal and reports a fatal error, as the [testing strategy](testing-strategy.md#test-levels-and-layout) requires. The library has three layers:

| Layer | Owns | May depend on |
| --- | --- | --- |
| Document | The outline tree, nodes, tags, parsing and serializing `.taskpaper` text, and editing operations on the tree | `std` and small pure crates only |
| App | Application state, messages, the update function, modes, undo, search and filter state, and effects such as "save this file" | Document |
| UI | Mapping key events to messages, widgets and the view that renders the state into a `Frame` | App, Document, Ratatui, crossterm |

- Dependencies point inward: UI depends on App, App on Document, never the reverse. Document and App never import `ratatui` or `crossterm`; a key event becomes an app-level message at the UI boundary.
- The update function is pure: it takes the state and a message and returns the new state and effects. File I/O and the clock happen at the edge and are passed in, so every layer except terminal setup runs in tests without a terminal.
- The view renders state and never changes it. Widgets hold no application state of their own beyond what Ratatui's `StatefulWidget` state needs.
- Each layer is a module tree (`document/`, `app/`, `ui/`). Only what another layer or the integration tests need is `pub`; everything else is private or `pub(crate)`. Splitting into a Cargo workspace requires an owner decision.
- Round-tripping is a design constraint, not a feature: the document model keeps whatever it needs, such as unknown syntax, original whitespace and tag order, so that saving an unchanged file reproduces it byte for byte ([AGENTS.md](../../AGENTS.md#2-correct-not-just-green)).

## Types and state

- Make invalid states unrepresentable. Use enums for node kinds, modes and outcomes, and newtypes for identifiers and validated values (such as a tag name), instead of strings, booleans or bare integers whose meaning depends on context.
- Validate at the boundary where text enters the program (parsing, user input) and carry the validated type inward, so inner code does not re-check.
- Prefer owned value types, composition and explicit state transitions. Avoid `Rc<RefCell<_>>` and interior mutability unless the ownership genuinely requires it, with a comment saying why.
- Choose the narrowest visibility and the simplest signature that works: borrow (`&str`, `&[T]`, `impl Iterator`) in parameters, return owned values when the caller needs them, and avoid needless `clone`.
- Derive standard traits (`Debug`, `Clone`, `PartialEq`, `Eq`, `Default`) where they are meaningful; every public type implements `Debug`.

## Errors and panics

- Expected failures (an unreadable or malformed file, an invalid edit) are typed: each module that can fail has a `thiserror` enum whose variants name what went wrong and carry the context needed to explain it, such as the path or line number. No stringly-typed errors, no `Box<dyn Error>` in library signatures.
- Propagate with `?`. Never discard an error silently (`let _ =`, `.ok()`, an empty `match` arm) unless ignoring it is correct, with a comment saying why.
- Errors the user can recover from are shown in the UI and leave the outline and the file on disk intact. Only `main.rs` turns an error into a process exit, through `anyhow` with context.
- Production code does not panic on any input. `unwrap`, `panic!`, `todo!`, `unimplemented!`, `unreachable!` and indexing that can go out of bounds are not allowed outside tests; use `get`, iterators, pattern matching or a typed error. `expect` is allowed only where an invariant makes failure impossible, and its message states that invariant, for example `expect("parser always yields a root node")`.
- Integer arithmetic on indices and positions uses checked or saturating operations where overflow or underflow is possible.

## Comments and documentation

- Prefer clear names, small functions and simple control flow to comments. Comments explain why, never what ([AGENTS.md](../../AGENTS.md#4-minimal-focused-diffs)).
- Every public item has a `///` doc comment, enforced by `missing_docs`. Keep it brief: usually one sentence on what the item promises. Add `# Errors` and `# Panics` sections where Clippy's pedantic lints require them.
- Document invariants, units and TaskPaper format rules that the code relies on but cannot express in types.
- Stale comments are defects; update or delete them with the code they describe.

## Naming and size

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/naming.html) for naming and conversions (`as_`, `to_`, `into_`, `try_`, `iter`). Use the TaskPaper and Workflowy vocabulary the specs use: project, task, note, tag, outline, zoom, fold.
- Keep modules, types and functions cohesive with one responsibility. Clippy's pedantic `too_many_lines` (100 lines) and argument-count limits flag functions to split; split them rather than allow the lint.
- File length is a review signal, not a hard limit: a file that mixes responsibilities is split by responsibility, not by line count.

## Static analysis

Lints are configured in `app/Cargo.toml` so that every machine and both agents get the same results:

```toml
[lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"

[lints.clippy]
pedantic = { level = "warn", priority = -1 }
unwrap_used = "warn"
panic = "warn"
todo = "warn"
unimplemented = "warn"
indexing_slicing = "warn"
```

- `validate.sh` runs Clippy with `-D warnings`, so every warning fails validation and the baseline stays at zero warnings. Tests may use `unwrap`, `panic!` and indexing through Clippy's `allow-unwrap-in-tests`, `allow-panic-in-tests` and `allow-indexing-slicing-in-tests` settings in `app/clippy.toml`.
- Formatting is stock `rustfmt` with no configuration; `validate.sh static` checks it.
- An `#[allow]` or `#[expect]` is placed on the smallest item possible, never crate-wide, and carries a `reason = "..."`. Prefer `#[expect]`, which fails once the lint no longer fires. Allowing a pedantic lint project-wide requires an owner decision recorded here.
- New code uses the latest stable Rust and dependency APIs, as the agent workflow's [Modern Rust](agent-workflow.md#modern-rust) rule requires.

## Toolchain and dependencies

- `rust-toolchain.toml` at the repository root pins an exact stable release with the `rustfmt`, `clippy` and `llvm-tools-preview` components. Bumping it is its own commit, which fixes any new lint findings. Agents use rustup so the pin takes effect.
- `cargo-deny`, configured in `app/deny.toml`, runs in `validate.sh static` and checks advisories, licenses (permissive licenses only), banned and duplicate crates, and that every crate comes from crates.io.
- Prefer `std`, Ratatui and crossterm when they cover the need. Add a dependency only for a demonstrated need, after checking its maintenance, license, size and transitive dependencies; record why on the task. Versions stay locked in `app/Cargo.lock` and every build uses `--locked`.
- No dependency may make network requests, collect data or write outside the files the user opens.

## Review expectations

Beyond correctness and the [testing strategy](testing-strategy.md), reviewers check:

- Layer boundaries and dependency direction, and that no logic leaked into `main.rs` or the view
- That `.taskpaper` content the change does not understand survives a save
- Typed errors, no new panic paths, and no silently discarded errors
- Lint suppressions: each one narrow, justified and unavoidable
- Modern Rust and dependency APIs
- Simplicity and readability, with comments only where they explain why

## Adoption

The configuration this document requires (the `[lints]` table, `clippy.toml`, `rust-toolchain.toml`, `deny.toml` and the `cargo-deny` step in `validate.sh`) lands as its own task, which also brings the scaffold in `main.rs` into compliance. Until then, new code follows these rules and reviewers check them by hand.

## Sources

- Cargo `[lints]` table: https://doc.rust-lang.org/cargo/reference/manifest.html#the-lints-section
- Clippy lint groups and configuration: https://doc.rust-lang.org/clippy/lints.html, https://doc.rust-lang.org/clippy/lint_configuration.html
- `#[expect]` and lint reasons: https://doc.rust-lang.org/reference/attributes/diagnostics.html#lint-check-attributes
- Rust API Guidelines: https://rust-lang.github.io/api-guidelines/
- rustup toolchain files: https://rust-lang.github.io/rustup/overrides.html#the-toolchain-file
- cargo-deny: https://embarkstudios.github.io/cargo-deny/
- thiserror and anyhow: https://docs.rs/thiserror/latest/thiserror/, https://docs.rs/anyhow/latest/anyhow/
- Ratatui, the Elm Architecture: https://ratatui.rs/concepts/application-patterns/the-elm-architecture/
