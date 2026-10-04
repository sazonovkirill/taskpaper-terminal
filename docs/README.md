# Docs

Requirements, specifications and reference material for taskpaper-terminal.

## Vision

taskpaper-terminal is a keyboard-driven terminal editor for [TaskPaper](https://www.taskpaper.com) files that behaves like an infinite outliner in the spirit of [Workflowy](https://workflowy.com):

- The `.taskpaper` file on disk is the source of truth; the app reads and writes plain TaskPaper text.
- Every line is a node (project, task, or note) and indentation defines the tree.
- Nodes can be expanded, collapsed, and zoomed into, so any branch can become the whole screen.
- Tags (`@done`, `@due(2026-10-04)`, ...) are metadata: hidden from the text, set through commands, and searchable.

## Contents

| Document | Status | What it holds |
|---|---|---|
| [functional-requirements.md](functional-requirements.md) | Accepted | What the app does, as decided by the owner. Overrides every other document here. |
| [requirements.md](requirements.md) | Accepted | Non-functional requirements (performance, reliability, portability) and file-handling requirements the functional requirements leave open. |
| [open-questions.md](open-questions.md) | Living | Undecided choices, each with a proposed default, plus the ones already decided. |
| [specs/](specs/README.md) | Accepted | How each area works. |
| [reference/](reference/README.md) | Reference | TaskPaper format and Workflowy features, sourced from official docs. |
| [quality/](quality/README.md) | Accepted | How agents work on this project. |

Draft documents become Accepted once the owner has reviewed them. Where a draft disagrees with an accepted document, the accepted document wins and the draft is a bug.
