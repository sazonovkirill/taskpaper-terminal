# Agent Operating Rules

The owner starts Claude or Codex agent sessions manually on their Mac; either tool may implement a task. Several agents work in parallel, each in its own worktree. This file defines the engineering rules for both tools. [The agent workflow](docs/quality/agent-workflow.md) defines task selection, claims, the task record, review gates, validation, escalation and delivery; [version-control practices](docs/quality/version-control.md) define worktrees, branches, commits, rebasing and publication; the [testing strategy](docs/quality/testing-strategy.md) defines what and how to test; [code quality and style](docs/quality/code-quality.md) defines architecture layers, error handling, lints and dependency rules.

## Reasoning effort

Agents choose their own reasoning effort for the work in front of them. An agent that notices it is not making significant progress, for example repeated failed attempts, a problem that resists diagnosis or reviews that keep finding real defects, raises its effort one level at a time without asking, up to the tool's highest level ("ultra"). Record each raise and its reason on the task. Reviewers run at no less than the implementer's current effort.

## Engineering guidelines

### 1. Context before code

This project's conventions override tool defaults. Before writing code:

- Read the files you will modify and at least one similar existing implementation. Match its patterns for error handling, naming, module layout and test structure, even if you would personally do it differently. If no similar implementation exists, use the accepted documents under `docs/` as the reference and establish the convention explicitly.
- Search for existing helpers before writing new ones. Use `rg` for the concept, not only the name. Duplicating a utility that already exists is a defect.
- Resolve unclear conventions from existing code and accepted documentation. If conflicting patterns require an owner decision, ask. Do not invent a third pattern.
- Never assume an API, feature flag or crate version exists. Verify it in `Cargo.toml`, `Cargo.lock` or the crate's documentation for that exact version.
- Write modern Rust: use the latest stable language features, standard-library APIs and current dependency APIs rather than deprecated or superseded ones. Reviewers check this specifically ([agent workflow](docs/quality/agent-workflow.md#modern-rust)).

#### Project facts

- **Product:** a feature-complete terminal clone of Workflowy that stores outlines as `.taskpaper` files. Requirements, specs and format references live in `docs/`.
- **Language and stack:** Rust (edition 2024) with [Ratatui](https://ratatui.rs) 0.30 and crossterm 0.29. The crate lives in `app/`; versions are pinned in `app/Cargo.lock`.
- **Commands:** from the repo root, `bash app/scripts/validate.sh all` runs the required validation (format check, Clippy with warnings denied, tests, 100% line coverage gate). `bash app/scripts/validate.sh mutants` mutation-tests the changed code and is required before review. Use `format` to apply `rustfmt`, `static` for format and lint only, `test` for tests only, or `coverage` for the coverage gate only. Run the app with `cargo run` from `app/`.

### 2. Correct, not just green

Passing tests is necessary, not sufficient. The goal is code a senior reviewer would approve.

- Never modify, skip, weaken or delete a test to make it pass unless the test is wrong. If you think it is wrong, explain why and stop.
- Never special-case production inputs, hardcode output to match test fixtures, or make production behavior detect a test environment.
- Fix root causes, not symptoms. If you do not understand why a bug happens, keep investigating instead of adding guards until it goes away.
- Avoid defensive clutter: do not handle errors you cannot meaningfully handle or add fallbacks that hide failures. Fail loudly at the right layer.
- Never lose or silently rewrite a user's `.taskpaper` file. Anything that writes to disk preserves content it does not understand.
- Run formatting, Clippy and tests, not only tests. Warnings count as failures.
- When done, report honestly what you verified, what you did not verify and any remaining doubts. Prefer explicit uncertainty to false confidence.

### 3. Design before implementation

For any change touching more than one module, adding a public interface or introducing a new abstraction:

- First write a short plan covering which components change, where the new code lives and why, the proposed interfaces and rejected alternatives. Record the plan on the task and proceed autonomously when it stays within the task and accepted design. Ask the owner when a material product decision or change to accepted constraints cannot be resolved from existing instructions; pause only the dependent work.
- Respect existing module boundaries and dependency direction. If the task seems to require breaking them, flag it instead of doing so silently.
- Prefer extending an existing abstraction to creating a parallel one.
- Do not add abstractions for hypothetical futures: no trait with one implementation, configuration for things that do not vary, or generic framework for one use case. Duplicate twice before abstracting, following the rule of three.
- Keep ownership clear: identify who creates, owns, mutates and drops each value or resource.

### 4. Minimal, focused diffs

The best change is the smallest one that fully solves the problem.

- Do exactly what was asked. Do not make drive-by refactors, renames, formatting changes, dependency bumps or unrelated improvements; mention them as suggestions instead.
- Do not add features, options, parameters or error cases nobody requested.
- Before finishing, reread the diff and remove anything not required, including dead code, unused imports, debug output, speculative helpers and comments that restate the code.
- Comments explain why, never what. Do not narrate changes in comments.
- Prefer a solution that deletes more code than it adds when both approaches fully solve the problem.
- If the diff is growing large, reassess scope and split coherent work into atomic commits. Escalate when completing the task requires an unapproved scope or design change.

### Definition of done

Apply each item that is relevant to the change type. Documentation-only validation is defined in the agent workflow.

- [ ] Follows the patterns of surrounding code; no duplicated helpers
- [ ] Builds on the existing design without speculative abstractions
- [ ] Handles edge cases at the correct layer without defensive clutter
- [ ] Behavior changes include unit and integration tests as the [testing strategy](docs/quality/testing-strategy.md) requires, covering behavior rather than implementation details
- [ ] `bash app/scripts/validate.sh all` and `bash app/scripts/validate.sh mutants` pass on the committed contents; no tests weakened
- [ ] Addresses the root cause rather than masking it
- [ ] Contains nothing beyond the requested task
- [ ] Summarizes what was verified, what was not verified and what remains uncertain

## Sources of truth

1. **Todoist is the backlog and the task record.** The Todoist project `taskpaper-terminal` holds tasks with status (section), priority, scope and acceptance criteria (description), and progress, review evidence, blockers and next actions (comments). Do not keep a competing task list in Markdown, local notes or agent memory.
2. **Markdown under `docs/` is authoritative durable knowledge.** Requirements, specs, the TaskPaper format reference, design decisions and workflow documentation live there. Link lasting decisions from the task record; do not leave them only in discussion.
3. **Local validation is authoritative.** Code changes need `bash app/scripts/validate.sh all` to pass for the exact committed contents. There is no GitHub CI. Documentation-only changes need local review and the `git diff --check` steps in the agent workflow.
4. **Review happens locally, not in pull requests.** Fresh Codex and Claude reviewers inspect each non-trivial commit. Agents do not open PRs unless explicitly asked, and have standing authorization to push scoped, reviewed, validated work to `main`.

## Working agreement

- Read the relevant task, docs and earlier records before changing code. Keep changes within the task's scope; record proposed scope or acceptance-criteria changes on the task before implementing them.
- Work only in your own worktree under `.worktrees/`, never in the primary checkout, and never touch another agent's worktree or branch.
- Keep `docs/` current when a change alters agreed behavior or design.
- Ask the owner in the agent session for unresolved decisions. Silence is not approval.

## Repository boundaries

- `app/`: the Rust crate, its source, tests and `scripts/`.
- `docs/`: requirements, specs, references and workflow documentation (`docs/quality/`); no task tracking.
- `.worktrees/`: agent worktrees, ignored by Git.
