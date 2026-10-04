# Version-Control Practices

- Status: Accepted
- Last updated: 2026-10-04

## Worktrees and branches

This is a single-developer project with several agents working in parallel on the owner's Mac. Each agent works in its own Git worktree under `.worktrees/` in the primary checkout (ignored by Git) on its own short-lived branch, and never does implementation work in the primary checkout.

- Name branches `<type>/<short-slug>`, where `<type>` is one of `feat`, `fix`, `refactor`, `test`, `docs` or `chore`, for example `feat/fold-outline`. Before creating one, check that no local or remote branch has that name.
- Create the worktree from the latest `main`, with the branch name's `/` replaced by `-` as the directory:

  ```sh
  git fetch origin main
  git worktree add .worktrees/feat-fold-outline -b feat/fold-outline origin/main
  ```

- Each worktree builds into its own `app/target/`. Do not share a target directory between worktrees.
- Never modify, rebase, delete or clean up another agent's worktree or branch, and never discard uncommitted work you did not create.

## Commit messages

- Write the title as a plain imperative sentence describing the outcome, for example `Fold and unfold items with Tab`. No type prefixes.
- Add a body only when the title does not give enough context. Keep the whole message to four or five short sentences at most.
- Explain what the commit achieves and include essential rationale or constraints only when they help a future reader. Do not narrate the implementation or list changed files.
- Prefer small, atomic commits. Each commit is one coherent outcome, independently understandable, and leaves the project valid.

## Publication

Agents push directly to `main`; there are no pull requests unless the owner asks for one. History on `main` is linear: no merge commits and no squashing of individually reviewed commits.

1. Commit in your worktree once validation and any required reviews pass ([agent workflow](agent-workflow.md)).
2. Fetch and rebase onto the latest `main`:

   ```sh
   git fetch origin main
   git rebase origin/main
   ```

3. A replayed commit keeps its earlier reviews when the rebase applies without conflicts, its `git patch-id --stable` matches the reviewed patch, and the validation required for its change type passes on the rebased tip. Record the old and new base and matching patch IDs on the task. A conflict, changed patch or failed validation requires fresh independent reviews of each affected non-trivial commit before pushing.
4. Push as a fast-forward: `git push origin HEAD:main`. If the push is rejected because `main` advanced, go back to step 2. Never force-push `main`.
5. Verify `origin/main` contains your commits, then fast-forward the primary checkout with `git -C <primary checkout> merge --ff-only origin/main` when it is clean and on `main`; otherwise leave it for the owner.

To pause unfinished work across sessions, push the branch to `origin` under the same name and record it on the task. Rebase it onto `main` before resuming.

## Cleanup and clean state

- After your commits are on `origin/main`, remove your worktree with `git worktree remove .worktrees/<dir>` (or the tool that created it), delete the local branch with `git branch -d`, and delete the remote branch if you pushed one.
- Archive abandoned work recoverably, with a tag, before deleting its branch.
- The idle clean state has only `main` locally and on the remote, no worktrees under `.worktrees/`, no uncommitted changes, and matching local and remote `main`.
