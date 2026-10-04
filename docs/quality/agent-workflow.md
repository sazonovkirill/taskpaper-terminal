# Agent Workflow

- Status: Accepted
- Last updated: 2026-10-04

## Authority and boundaries

The owner starts Claude or Codex agent sessions manually and tells each one which task to work on: either a specific Todoist task or "pick the next task from the Todoist backlog". Either tool may implement. The implementing agent chooses implementation details and atomic commit boundaries, obtains independent fresh Codex and Claude reviews for non-trivial changes, and commits and pushes scoped, validated work to `main` without further approval. Human review before publication is optional unless a task requires it. This authority covers the assigned task, not arbitrary changes outside its scope.

[AGENTS.md](../../AGENTS.md) and [version-control practices](version-control.md) remain governing constraints. Routine designs within accepted specs are delegated to the implementing agent. Record plans for changes spanning modules, adding public interfaces or introducing abstractions before implementation. Escalate unresolved product choices, changes to accepted specs or quality requirements, and scope expansions requiring owner judgment. An explicit task instruction takes precedence over this workflow.

## Backlog and task selection

Todoist is the backlog and the task record. The backlog is the Todoist project `taskpaper-terminal`:

- **Inbox** is owner staging. Never claim tasks from it unless the owner names one.
- **Ready** holds tasks agents may claim.
- **In Progress** holds tasks in work, including blocked ones.
- **Done** holds delivered tasks awaiting owner acceptance. They stay open.

Tasks elsewhere in Todoist do not authorize project work. An actionable Ready task authorizes its scoped work, as does a task the owner names in the session, wherever it sits. Infer routine acceptance details from accepted specs and record them on the task; ask before inventing material product requirements.

When asked to pick the next task, first resume an In Progress task with no `agent-` label and no unresolved blocker. Otherwise choose the runnable Ready task with the highest priority, p1 through p4, then the oldest creation time. A task is not runnable while a dependency named in its description remains unaccepted.

### Claims

Several agents work in parallel, each on a different task in its own worktree. All agents act through the owner's Todoist account, so each one marks its claims with its own label. At the start of a session, pick a short fictional name and take the label `agent-<name>-<suffix>`, where `<suffix>` is the output of `openssl rand -hex 3`, so that parallel sessions get distinct labels. Run the command; never invent the suffix or copy one from an example. Confirm that the Todoist filter for your label, such as `@agent-juniper-<suffix>`, finds nothing. A label that is not one of the account's personal labels exists only while a task carries it, so agents never create or delete labels. A label update replaces the task's whole label list, so always send the task's other labels with yours.

1. **Claim.** Re-read the task. If it has left the section it was selected from or carries another `agent-` label, select another task; for a task the owner named, ask the owner instead. Otherwise add your label, move the task to In Progress and add a comment naming your label, branch and absolute worktree path.
2. **Confirm.** Re-read the task. A concurrent claim can overwrite or join your label. If your label is gone or another `agent-` label is present, remove yours if it remains, leave the task where it is and select another task; for a task the owner named, ask the owner instead. An owner-approved takeover replaces the previous label with yours and adds a comment citing the approval.
3. **Check in.** Before freezing an increment for review, committing, pushing and delivering, re-read the task. If your label was removed or the owner moved, closed or deleted the task, stop at a recoverable checkpoint and ask the owner. Closure or deletion before delivery is not acceptance.
4. **Release.** Remove your label and add a release comment when delivering the task or stopping work on it. When you release an unblocked task without delivery, move it back to Ready, or to Inbox if it was claimed from Inbox.

Keep the task record current in Todoist comments: plan, scope decisions, effort raises, review evidence, validation, blockers, delivery summary and commit links. Before pausing or ending a session, record the branch, absolute worktree path, changed paths and whether work remains uncommitted or unpushed. Do not maintain competing Markdown task lists or agent-memory project state.

### Coordination between agents

When Todoist comments are not enough, for example overlapping or dependent work between agents or a blocker another task must resolve, add a comment on every affected task linking the others, naming the conflict and the next action. Create follow-ups for out-of-scope work in the Inbox section for owner triage. Agents never move their own follow-ups to Ready.

## Blockers, decisions and rework

The owner is reachable in the agent session they started. Ask there when a decision cannot be resolved from task instructions, accepted specs and delegated judgment. Include the question, reasonable options, a recommendation and the impact of waiting. Silence, elapsed time and another agent's opinion are not approval. Record the answer on the task and lasting decisions in `docs/` before resuming. Changed requirements return through validation and review.

When work cannot proceed, keep the task in In Progress and record the exact blocker, preserved worktree state and next action. Never overwrite an owner's move of active work; ask for clarification instead.

To resume or rework a task, use its existing record and its worktree if it still exists; otherwise start a fresh worktree from the current `origin/main`. Reconcile the recorded state with the checkout, staged changes, local and remote commits, and recorded validation before continuing. Resume existing work rather than duplicating commits, comments or delivery updates. Never spin indefinitely through review disagreements, authentication failures or unavailable tools: raise effort as [AGENTS.md](../../AGENTS.md) describes, then record the blocker and report it.

## Review and commit gate

Independent review is required for every non-trivial commit, code or documentation. A trivial change may be committed without it: one that is small and mechanical, whose correctness is evident from the diff and the deterministic checks, such as a typo, a version bump or formatting. A change to app behavior, interfaces, file handling, tests or governing rules is never trivial. When unsure, treat the change as non-trivial. Validation and the task record apply either way; record why a change was treated as trivial.

For every non-trivial commit, including later fixes:

1. **Ground first.** Prepare one coherent increment and run the validation for its change type. For code changes that means `bash app/scripts/validate.sh all` and `bash app/scripts/validate.sh mutants`, with every missed mutant fixed or recorded as equivalent ([testing strategy](testing-strategy.md)). Every commit must leave the project valid. Deterministic checks, not reviewers, settle anything a tool can decide, such as formatting, lint, build and tests.
2. **Freeze and brief.** Freeze the base revision and proposed contents, then give a fresh Codex reviewer and a fresh Claude reviewer the same brief (below). Reviews are read-only and independent: neither inherits the implementer's conversation or sees the other reviewer's findings before reporting.
3. **Record.** After both reviewers report, record on the task each reviewer's model, effort and session identifier when available, the base revision, a reproducible identifier of the reviewed contents such as the proposed Git tree, and the findings.
4. **Adjudicate against source.** Apply the severity rules below. The implementer fixes verified P0/P1 findings; reviewers only report. Resolve disputes by checking the code, tests or accepted requirements, never by vote or by asking a model which argument is right. To verify a disputed P0/P1, send it to a fresh reviewer from the other model family, told to refute it and to default to refuted when uncertain. Record each finding's disposition and reason on the task.
5. **Re-review only the delta.** When contents change, rerun affected checks and brief fresh reviewers on the diff since the last round, the findings it fixes and the two registers, not the whole increment again.
6. **Commit** when both reviewers' latest round holds no unrefuted P0/P1, required checks pass, and the staged contents match the reviewed snapshot. Reviewer unavailability or incomplete inspection blocks the commit; never substitute self-review.

For a multi-commit feature, also review the aggregate change with fresh Codex and Claude sessions before declaring delivery. Any resulting fix goes through the same gate. Independent review supplements tests; it does not replace local validation.

### Severity

- **P0/P1 (critical/major):** a functional bug, data loss, security flaw, explicit violation of the task's acceptance criteria or governing documents, or a logical contradiction. Admissible only with a proof: an execution trace or test case naming concrete inputs, expected result and actual result, or for documents, the conflicting rules quoted with a concrete scenario in which an agent following them fails. A P0/P1 without a proof is demoted to P3 without follow-up questions.
- **P2/P3 (minor/nitpick):** style, naming, wording, readability, optional hardening and anything outside the diff's scope. These never block a commit or start another round and are not debated. Record them on the task; turn worthwhile ones into follow-ups.

### Modern Rust

The project wants modern code. Reviewers pay extra attention to whether the diff uses the latest stable Rust language features, standard-library APIs and current APIs of its dependencies at their locked versions. Code that uses a deprecated or superseded API, or a hand-written workaround for something the language, `std` or a dependency now provides, is a violation of this rule and a P1. The proof names the outdated code, the stable replacement, and the Rust release or crate version that provides it, which must be no newer than the project's toolchain and `Cargo.lock`. Implementers check the same before freezing an increment. Both check against the sources actually in use rather than memory: dependencies under `~/.cargo/registry/src/` at the versions in `app/Cargo.lock`, and the standard library under `$(rustc --print sysroot)/lib/rustlib/src/rust/library` (the pinned toolchain's `rust-src` component).

### Taste and authority

Reviewers from different model families have different preferences. Settle disagreements in this order: verified facts, tests and tool output; then the task's acceptance criteria and the governing documents; then the implementer's choice. A preference not written in the governing documents is P3 by definition, however strongly argued. Approve a change that meets its criteria and clearly improves the project even if it is not how the reviewer would write it. When the same preference keeps coming back across tasks, ask the owner once and record the decision in the governing documents, where it becomes a rule rather than a taste.

### Reviewer brief

Restate the constraints and definition of done at the top of every brief, and put the artifact last:

```
Determine whether this change meets its acceptance criteria and governing documents.
Review only the diff and what it touches; not the codebase as a whole. No nitpicks,
no pedantic opinions, no scope creep. If there isn't anything major, just say so:
finding nothing is a correct and expected outcome.

Pay extra attention to whether the diff uses the latest stable Rust and dependency
APIs; an outdated API with a stable replacement is a P1 (see Modern Rust).

Report each P0/P1 with its proof (see docs/quality/agent-workflow.md, Severity).
Do not raise findings listed in the registers below without new proof.
End with `VERDICT: HOLDS` or `VERDICT: FAILS`, plus a list of what you checked.

<rejected findings register>
<deliberate decisions register>
<task and acceptance criteria>
<frozen diff>
```

From the second round on, carry two registers in every brief: **rejected findings** (what was raised and dismissed, and why) and **deliberate decisions** (intentional choices, accepted tradeoffs and owner decisions). Deduplicate new findings against everything raised so far, not only what was accepted.

### Termination

Run at most three review rounds per increment. Stop soliciting new findings when any of these holds:

- A round produces no proof-carrying P0/P1.
- A round repeats an earlier finding or would undo an earlier fix. Stop and put both positions to the owner.
- Fewer than about one in five raised P0/P1 findings survive verification.

After a stop, fix any verified P0/P1 findings and confirm only those fixes in one more round within the cap; that round may not raise unrelated findings. If verified P0/P1 findings remain after the third round, report them to the owner as a blocker instead of starting a fourth. Never add a round because a round produced only P2/P3 findings, never ask a reviewer whether review should continue, and never rerun the same reviewers to break a stall.

### Launching reviewers

`bash app/scripts/review.sh <effort> <base> <revision> <brief-file>` does all of this: it checks the revision out into a frozen worktree, appends the complete diff since the base to the brief, runs both reviewers in parallel at `<effort>` (the Claude reviewer at `max` for Codex's `ultra` or `persistent`), and prints their verdicts and the reviewed tree. The [`review-commit` skill](../../.agents/skills/review-commit/SKILL.md) walks through the gate around it.

The script launches reviewers from the frozen worktree with the prompt on standard input, at no less than your own current effort. Both commands keep reviewers away from Todoist, so they cannot read task comments. The Codex reviewer can still reach other configured MCP servers; its prompt forbids reading task records or other reviewers' output. The Claude reviewer cannot run Git, so the prompt contains the complete diff under review, including new files, and `--add-dir` lets it read the dependency and standard-library sources outside the worktree; the Codex sandbox can already read them. Substitute newer models when they replace these:

- Codex: `codex exec --model gpt-5.5 --config model_reasoning_effort="<effort>" --config mcp_servers.todoist.enabled=false --sandbox read-only --ephemeral -`
- Claude: `claude --model claude-opus-5-5 --effort <effort> --print --permission-mode dontAsk --tools Read,Grep,Glob --strict-mcp-config --no-session-persistence --add-dir ~/.cargo/registry/src "$(rustc --print sysroot)/lib/rustlib/src/rust/library"`

## Validation, publication and delivery

Code changes require `bash app/scripts/validate.sh all` (format, lint, cargo-deny, tests and the coverage gate) and `bash app/scripts/validate.sh mutants` to pass for the exact contents being committed, before pushing. There is no GitHub CI. Record the command and exit status on the task. Investigate failures and review and validate any corrective commit through the same gate. Do not hide failures with weaker tests, repeated blind retries or results from earlier contents.

Documentation-only work needs local review and `git diff --check` before staging plus `git diff --cached --check` after staging.

Standing authorization covers pushing to `main`. Follow [version-control practices](version-control.md) for rebasing and publication. Parallel agents may advance `main` while a review is in progress; a clean replay of an unchanged patch keeps its reviews.

After acceptance checks, required independent review, durable documentation, recorded validation and verified publication, move the Todoist task to Done and leave it open. Record the delivery summary, commit links and limitations on the task and release the claim. Agents never close Todoist tasks; the owner closing a delivered task is acceptance. If the owner requests rework, claim the task again and continue on its existing record.

After delivering, the agent retires its worktree as [version-control practices](version-control.md#cleanup-and-clean-state) describe and stops unless the owner asked it to continue with further tasks.

## Prerequisites

Agents need the Todoist MCP server (`https://ai.todoist.net/mcp`) registered under the name `todoist` and signed in for both Claude Code and Codex, the `codex` and `claude` CLIs for independent reviewers, `openssl`, `jq` for the Claude Code hook, the tracked Git hooks enabled once per clone with `git config core.hooksPath .githooks`, and a stable Rust toolchain with `rustfmt`, `clippy`, `llvm-tools` and `rust-src` (`rustup toolchain install` from the repository installs the pinned toolchain with them), and `cargo install --locked cargo-llvm-cov cargo-mutants cargo-deny`. Install Rust through rustup with its proxies first on `PATH`; a Homebrew `rust` ahead of them bypasses the toolchain pin. Start every session with `bash app/scripts/validate.sh doctor`, which checks everything above except whether you are signed in to Todoist and prints how to fix whatever is missing; stop and report to the owner if it fails. Keep credentials out of Git, task records and model prompts.
