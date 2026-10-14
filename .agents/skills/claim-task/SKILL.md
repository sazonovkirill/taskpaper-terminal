---
name: claim-task
description: Claim a taskpaper-terminal task in Todoist at the start of an implementation session, either one the owner named or the next one from the backlog. Not for reviewer sessions.
---

# Claim a task

The rules are in [the agent workflow](../../../docs/quality/agent-workflow.md#backlog-and-task-selection); this is the procedure.

1. Run `bash app/scripts/validate.sh doctor`. If it fails, report what it printed to the owner and stop.
2. Make sure you are in your own worktree ([version-control practices](../../../docs/quality/version-control.md#worktrees-and-branches)).
3. Take a label: pick a short fictional name, run `openssl rand -hex 3`, and form `agent-<name>-<suffix>` from its real output. Search Todoist with the filter `@agent-<name>-<suffix>`; it must find nothing.
4. Choose the task. A task the owner named is the task. Otherwise, in the Todoist project `taskpaper-terminal`, resume an In Progress task with no `agent-` label and no unresolved blocker, or else take the Ready task with the highest priority, then the oldest, whose dependencies are accepted and which is not labelled WILL_NOT_FIX.
5. Claim: re-read the task. If it moved or carries another `agent-` label, pick another (ask the owner for a named task). Otherwise set its labels to its existing labels plus yours, move it to In Progress, and comment with your label, branch and absolute worktree path.
6. Confirm: re-read the task. If your label is gone or another `agent-` label appeared, remove yours if it remains, leave the task where it is and pick another; for a task the owner named, ask the owner instead.
7. Run `bash app/scripts/validate.sh test` for a baseline ([testing strategy](../../../docs/quality/testing-strategy.md#practices-for-agent-written-tests)), then record your plan on the task before changing code that spans modules or adds interfaces.
