---
name: review-commit
description: Get the independent Claude review a non-trivial commit needs before it may be pushed in taskpaper-terminal, and adjudicate the findings. Use as the implementer, never as a reviewer.
---

# Review a commit

The rules are in [the review and commit gate](../../../docs/quality/agent-workflow.md#review-and-commit-gate); this is the procedure.

1. Prepare one coherent increment. Run `bash app/scripts/validate.sh all` and, for code, `bash app/scripts/validate.sh mutants`.
2. Freeze it without committing: `git add -A`, then `snapshot="$(git commit-tree "$(git write-tree)" -p HEAD -m 'Review snapshot')"`. The snapshot is a commit object no branch points to; its tree is the reviewed contents.
3. Write the brief to a file outside the repository. Start it with the reviewer brief from the agent workflow, then the two registers (both `none` in the first round), then the task excerpt: title, acceptance criteria, allowed scope, and the evidence you gathered (commands, exit codes, outputs). Do not include task comments or another reviewer's output. The script appends the diff.
4. Run `bash app/scripts/review.sh <effort> HEAD "$snapshot" <brief-file>` at no less than your own effort. It reviews a frozen worktree of the snapshot with a fresh Claude reviewer that can read the crate and standard-library sources, prints the verdict, the output path and the reviewed tree, and exits non-zero unless the reviewer produced a verdict.
5. Read the output. Check each P0/P1 against the code, tests or documents; send a disputed one, with your counter-evidence, to another fresh Claude reviewer told to refute it, without naming who wrote the code or raised the finding. Record on the task: model, effort, base, reviewed tree, findings and their dispositions.
6. Fix verified P0/P1 findings, rerun step 1's validation on the fixed contents, freeze a new snapshot, and rerun step 4 with the previous snapshot as the base, the fixed findings named in the brief and both registers filled in. Stop after three rounds or on any termination condition, and report what remains to the owner.
7. When the latest verdict carries no unrefuted P0/P1 and step 1's validation passed on exactly these contents, commit the staged contents with a short message ([commit messages](../../../docs/quality/version-control.md#commit-messages)) and check that `git rev-parse HEAD^{tree}` equals the reviewed tree.
8. For a feature spread over several commits, run steps 3 to 6 once more on the whole range (`review.sh <effort> <first-parent> HEAD …`) before delivering.
