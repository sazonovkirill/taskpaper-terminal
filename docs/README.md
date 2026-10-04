# Docs

Requirements and specs for taskpaper-terminal live here.

## Vision

taskpaper-terminal is a keyboard-driven terminal editor for [TaskPaper](https://www.taskpaper.com) files that behaves like an infinite outliner in the spirit of Workflowy:

- The `.taskpaper` file on disk is the source of truth; the app reads and writes plain TaskPaper text.
- Every line is a node (project, task, or note) and indentation defines the tree.
- Nodes can be expanded, collapsed, and zoomed into, so any branch can become the whole screen.
- Tags (`@done`, `@due(2026-10-04)`, ...) are first-class: visible, editable, and searchable.

## TaskPaper format (summary)

- **Project**: a line ending with `:` (tags may follow).
- **Task**: a line starting with `- `.
- **Note**: any other line.
- **Tag**: `@name` or `@name(value)` anywhere in a line.
- Hierarchy is expressed with tab indentation.

## Next

Specs to be written here: data model, keybindings, editing behaviour, search and filtering, file I/O.
