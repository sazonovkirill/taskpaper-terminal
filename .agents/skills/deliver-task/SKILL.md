---
name: deliver-task
description: Publish reviewed, validated taskpaper-terminal commits to main and deliver the Todoist task. Use after the review-commit gate holds.
---

# Deliver a task

The rules are in [version-control practices](../../../docs/quality/version-control.md#publication) and [the agent workflow](../../../docs/quality/agent-workflow.md#validation-publication-and-delivery); this is the procedure.

1. Check in: re-read the task. If your label was removed or the task was moved, closed or deleted, stop and ask the owner.
2. `git fetch origin main && git rebase origin/main`. On a conflict, resolve it and review the affected commits again.
3. Compare `git show <reviewed> | git patch-id --stable` with the same for each rebased commit. Any difference needs a new review.
4. Run the validation for the change type on the rebased tip, then push it: `git push origin HEAD:main`. If `main` moved, go back to step 2. Never force.
5. Verify `origin/main` contains your commits, then `git -C <primary checkout> merge --ff-only origin/main` if the primary checkout is clean and on `main`.
6. Record on the task: old and new base, patch IDs, validation commands and exit codes, commit links, limitations. Move it to Done, leave it open, remove your label and add a release comment.
7. Clean up as version-control practices describe: remove a worktree you created and its branches; leave a tool-created worktree for the tool, and say so.
