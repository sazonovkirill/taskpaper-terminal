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
8. **UI-independent core.** All core functions of the app, the data model and the business logic, are independent of the UI. The terminal UI is one frontend; it must be feasible to run the same core behind a different UI later, such as a web or desktop app, without changing the core.
9. **SOLID design.** The code follows the SOLID principles, applied the Rust way as [Design principles](#design-principles) describes. This is the owner's main code quality requirement (owner, 2026-10-04).
10. **Clear abstractions and interfaces, clean boundaries.** Every component hides its internals behind a small, documented interface, and components meet only at those interfaces.
11. **One clear responsibility per component.** Every module, type and function has one responsibility that can be stated in a sentence.
12. **Modular code.** Features live in cohesive modules that can be understood, tested and changed on their own.

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
- Each layer is a module tree (`document/`, `app/`, `ui/`). Only what another layer or the integration tests need is `pub`; everything else is private or `pub(crate)`. The owner chose one crate: the separation between core and UI exists at the module level, and reviewers enforce it. Splitting into a Cargo workspace requires a new owner decision.
- Round-tripping is a design constraint, not a feature: the document model keeps whatever it needs, such as unknown syntax, original whitespace and tag order, so that saving an unchanged file reproduces it byte for byte ([AGENTS.md](../../AGENTS.md#2-correct-not-just-green)).

### UI-independent core

Document and App together are the core. Every behavior of the app, including editing, navigation, folding, zooming, undo, search, filtering, loading and saving, lives in the core and works the same behind any frontend. The terminal UI only translates input into messages and renders what the core exposes.

- **Messages are user intents, not input events.** The core accepts messages such as "move to next item", "indent", "zoom into this item" or "set the filter to `@due`", never key codes or mouse events. Key bindings and keymaps belong to the frontend, so a web or desktop UI can bind the same intents differently.
- **The core exposes what to show, not how.** It provides read-only queries for everything a frontend renders: the visible items with their depth, kind, text, tags and fold state, the selection and cursor, the current mode, the zoom path and status or error messages. It knows nothing of cells, colors, styles, borders or Ratatui types.
- **Layout and presentation belong to the frontend.** Text wrapping, Unicode display width, scrolling to keep the cursor visible, themes and widgets live in the UI. When a behavior depends on the viewport, such as page up and page down, the frontend passes the size it has as part of the message.
- **Effects are data.** The core returns effects such as "save this text to this path" and never performs I/O, reads the clock or blocks. The frontend executes them and reports results back as messages, so the core can run in a single-threaded, async or WebAssembly host.
- **No terminal assumptions.** The core has no global or static mutable state, no direct access to the environment, the terminal or the process, and no dependency that only works in a terminal.
- **Test the core without the UI.** Unit tests cover every core behavior by sending messages to the core and checking its queries and effects, so the same contract holds for any future frontend. UI tests check the key bindings and what is drawn on top of it.

## Design principles

Requirements 9 to 12 apply at every level, from functions to layers. Rust has no classes or inheritance, so SOLID is applied through modules, visibility, enums, plain functions and, where they earn their place, traits. These principles do not override [AGENTS.md](../../AGENTS.md#3-design-before-implementation): no speculative abstraction, no trait with one implementation, rule of three. Good design here means simple, well-separated code, not more layers.

### SOLID in this codebase

- **Single responsibility.** Each module, type and function has one reason to change, stated in its doc comment. A module's `//!` comment says in one sentence what it is responsible for and which decision it hides, such as the `.taskpaper` syntax or the key bindings. If that sentence needs "and" to join unrelated jobs, split the module. Parsing, editing operations, application state, key mapping and rendering are separate responsibilities, and so are policy (what should happen) and mechanism (how it is done).
- **Open/closed.** Adding a feature adds code in the place that owns it instead of editing unrelated code. A closed set of variants, such as block kinds, modes or messages, is an enum handled with exhaustive `match`, so adding a variant makes the compiler list every site that needs a decision. Avoid `_` arms on the project's own enums where a new variant would need its own handling. Shared hubs such as `app::update` and `ui::keymap` dispatch to per-feature functions or modules and do not grow feature logic inline. A trait is the extension point only when two or more implementations exist or a real seam needs one.
- **Liskov substitution.** Every trait implementation honors the contract the trait documents, including its errors and edge cases, so callers never check which implementation they hold. When a trait has several implementations, the same contract tests run against each of them ([testing strategy](testing-strategy.md)).
- **Interface segregation.** Interfaces are small and specific. A function takes the narrowest input it needs, such as `&Document` or a node identifier instead of the whole `State`, and a trait has only the methods all of its callers use. Do not hand a component data it should not touch.
- **Dependency inversion.** Policy does not depend on mechanism. The core reaches files, the clock and the terminal only through messages and effects-as-data ([UI-independent core](#ui-independent-core)), and inner layers define the types that outer layers adapt to. Inside the core, dependencies are passed in as parameters, never reached through globals, statics or the environment.

### Interfaces and boundaries

- **Deep modules.** A module's interface is much simpler than what it does: a few well-named functions and types that hide a design decision, such as the tree representation or the file format. Avoid shallow modules whose interface is as complex as their implementation, pass-through functions that only forward to another call, and wrappers that add a name but no abstraction.
- **Minimal public surface.** Start private and widen visibility only when another module needs it: `pub(crate)` for use inside the crate, `pub` only for what another layer or the integration tests use. A module's root file (`mod.rs` or `<name>.rs`) declares its interface, re-exporting the types and functions callers use, so callers do not reach into its submodules.
- **No leaked representation.** An interface exposes operations and read-only queries, not internal collections or fields to mutate. Callers change state only through operations that keep its invariants, so a representation can change without touching them.
- **Data crosses boundaries as the inner layer's types.** Messages, effects, view queries and errors are defined by the layer that owns them. An outer layer converts its own types, such as key events or Ratatui widgets, at its edge.
- **No cycles.** Dependencies between sibling modules point one way, like the layers. When two modules need each other, the shared part moves into a third module, or one of them was not a separate responsibility.
- **Cohesive modules.** Code that changes together lives together. Organize modules by responsibility and feature, such as folding or tags, not by kind of code, such as a module of all helpers or all constants. No `utils` or `misc` dumping grounds.
- **Small, clear functions.** A function does one thing at one level of abstraction, and its name says what. Prefer early returns and `?` to deep nesting. Clippy's `too_many_lines`, `too_many_arguments` and `cognitive_complexity` lints flag functions to split ([Static analysis](#static-analysis)).

### Agent failure modes

Studies of AI-written code find more duplication, less reuse of existing code and more churn than in human-written code ([GitClear](https://arc.dev/developer-blog/impact-of-ai-on-code/), [LeadDev](https://leaddev.com/ai/code-maintainability-plummets-in-the-ai-coding-era)), and a reviewer asked to find gaps tends to push toward extra layers and defensive code ([Claude Code best practices](https://code.claude.com/docs/en/best-practices)). Implementers check their own diff for these before review, and reviewers look for them:

- A new helper, type or module that duplicates one that already exists, or a near copy of existing code with small changes
- A new pattern for something the codebase already does one way, such as error handling, naming or test layout
- Abstraction with no second use: a trait, generic parameter, builder, configuration option or layer for a single case
- Defensive checks for states the types already rule out, and fallbacks that hide failures
- Feature logic added inline to a hub or a long function instead of to the module that owns it
- Code that works around a design that no longer fits instead of first refactoring it ([AGENTS.md](../../AGENTS.md#4-minimal-focused-diffs))
- Dead code, commented-out code and comments that narrate the change

Code written before these rules is brought into line where a task changes that code, through a preparatory refactoring. Reviewers raise an existing violation only where the diff touches it.

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
- Keep modules, types and functions cohesive with one responsibility ([Design principles](#design-principles)). Clippy's `too_many_lines` (100 lines), `too_many_arguments` and `cognitive_complexity` lints flag functions to split; split them rather than allow the lint. `cognitive_complexity` is an approximation: when splitting would make a function harder to read, `#[expect]` it with a reason.
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
cognitive_complexity = "warn"
```

- `validate.sh` runs Clippy with `-D warnings`, so every warning fails validation and the baseline stays at zero warnings. Tests may use `unwrap`, `panic!` and indexing through Clippy's `allow-unwrap-in-tests`, `allow-panic-in-tests` and `allow-indexing-slicing-in-tests` settings in `app/clippy.toml`.
- Formatting is stock `rustfmt` with no configuration; `validate.sh static` checks it.
- An `#[allow]` or `#[expect]` is placed on the smallest item possible, never crate-wide, and carries a `reason = "..."`. Prefer `#[expect]`, which fails once the lint no longer fires. Allowing a pedantic lint project-wide requires an owner decision recorded here.
- `app/clippy.toml` lists `disallowed-methods` for sleeping and reading the clock, which belong to the frontend's edge ([UI-independent core](#ui-independent-core)). Frontend code that must read the clock opts out on that item with `#[expect(clippy::disallowed_methods, reason = "...")]`.
- `validate.sh static` also fails when a line of `src/document` that is not a comment contains the word `ratatui`, `crossterm`, `app` or `ui`, or such a line of `src/app` contains `ratatui`, `crossterm` or `ui`, so the layer direction above is checked, not only reviewed. It matches whole words in any import form, alias or `super::` path; name local values in those layers so they do not collide.
- New code uses the latest stable Rust and dependency APIs, as the agent workflow's [Modern Rust](agent-workflow.md#modern-rust) rule requires.

## Toolchain and dependencies

- `rust-toolchain.toml` at the repository root pins an exact stable release with the `rustfmt`, `clippy` and `llvm-tools` components. Bumping it is its own commit, which fixes any new lint findings. Agents use rustup so the pin takes effect.
- `cargo-deny`, configured in `app/deny.toml`, runs in `validate.sh static` and checks advisories, licenses (permissive licenses only, plus MPL-2.0 for `nucleo-matcher`, which [architecture.md §3](../specs/architecture.md) names; any other non-permissive license needs the owner), banned and duplicate crates, and that every crate comes from crates.io. A duplicate version that Ratatui or crossterm pulls in is listed under `skip` with a reason and removed once upstream converges. The crate sets `publish = false` until the owner chooses a license, so the license check covers dependencies only.
- Prefer `std`, Ratatui and crossterm when they cover the need. Add a dependency only for a demonstrated need, after checking its maintenance, license, size and transitive dependencies; record why on the task. Versions stay locked in `app/Cargo.lock` and every build uses `--locked`.
- No dependency may make network requests, collect data or write outside the files the user opens.

## Review expectations

Beyond correctness and the [testing strategy](testing-strategy.md), reviewers check:

- Layer boundaries and dependency direction, and that no logic leaked into `main.rs` or the view
- The [design principles](#design-principles): one responsibility per component, small interfaces, no leaked representation, no cycles, and none of the agent failure modes. A design finding is P1 only when it breaks a rule written in this document, with the proof the [severity rules](agent-workflow.md#severity) require; a view that a different design would be cleaner is P2
- That `.taskpaper` content the change does not understand survives a save
- Typed errors, no new panic paths, and no silently discarded errors
- Lint suppressions: each one narrow, justified and unavoidable
- Modern Rust and dependency APIs
- Simplicity and readability, with comments only where they explain why

## Sources

- Cargo `[lints]` table: https://doc.rust-lang.org/cargo/reference/manifest.html#the-lints-section
- Clippy lint groups and configuration: https://doc.rust-lang.org/clippy/lints.html, https://doc.rust-lang.org/clippy/lint_configuration.html
- `#[expect]` and lint reasons: https://doc.rust-lang.org/reference/attributes/diagnostics.html#lint-check-attributes
- Rust API Guidelines: https://rust-lang.github.io/api-guidelines/
- rustup toolchain files: https://rust-lang.github.io/rustup/overrides.html#the-toolchain-file
- cargo-deny: https://embarkstudios.github.io/cargo-deny/
- thiserror and anyhow: https://docs.rs/thiserror/latest/thiserror/, https://docs.rs/anyhow/latest/anyhow/
- Ratatui, the Elm Architecture: https://ratatui.rs/concepts/application-patterns/the-elm-architecture/
- SOLID: Robert C. Martin, *Agile Software Development: Principles, Patterns, and Practices* (2002)
- Deep modules and information hiding: John Ousterhout, *A Philosophy of Software Design*, 2nd ed. (2021)
- Preparatory refactoring ("make the change easy, then make the easy change"): Martin Fowler, *Refactoring*, 2nd ed. (2018)
- Claude Code best practices: https://code.claude.com/docs/en/best-practices
- Maintainability of AI-written code: https://arc.dev/developer-blog/impact-of-ai-on-code/, https://leaddev.com/ai/code-maintainability-plummets-in-the-ai-coding-era
