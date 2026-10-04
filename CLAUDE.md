# Claude Instructions

Follow [AGENTS.md](AGENTS.md) and [the agent workflow](docs/quality/agent-workflow.md). Claude works either as the implementer of a task or as an independent reviewer of a proposed commit, alongside a separate fresh Codex reviewer.

As an implementer, create your worktree under `.worktrees/` as [version-control practices](docs/quality/version-control.md) describe and work only there.

As a reviewer, start in a fresh session without the implementer's conversation or the other reviewer's findings. Use the supplied task-and-criteria excerpt, allowed scope, applicable governing documents, relevant source and exact proposed diff. Do not read the task's comments or review records; if the excerpt is missing or insufficient, report that limitation to the implementer instead of fetching them. Review read-only; do not modify, stage, commit, publish or contact the owner. Review only the diff and what it touches. Report P0/P1 findings only with the proof the agent workflow's severity rules require, list P2/P3 separately without arguing for them, and do not re-raise registered findings or decisions without new proof. Finding nothing is a correct outcome: end with `VERDICT: HOLDS` or `VERDICT: FAILS`, what you checked and any limitations.
